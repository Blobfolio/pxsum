/*!
# pxsum
*/

#![forbid(unsafe_code)]

#![deny(
	clippy::allow_attributes_without_reason,
	clippy::correctness,
	unreachable_pub,
)]

#![warn(
	clippy::complexity,
	clippy::nursery,
	clippy::pedantic,
	clippy::perf,
	clippy::style,

	clippy::allow_attributes,
	clippy::clone_on_ref_ptr,
	clippy::create_dir,
	clippy::filetype_is_file,
	clippy::format_push_string,
	clippy::get_unwrap,
	clippy::impl_trait_in_params,
	clippy::lossy_float_literal,
	clippy::missing_assert_message,
	clippy::missing_docs_in_private_items,
	clippy::needless_raw_strings,
	clippy::panic_in_result_fn,
	clippy::pub_without_shorthand,
	clippy::rest_pat_in_fully_bound_structs,
	clippy::semicolon_inside_block,
	clippy::str_to_string,
	clippy::string_to_string,
	clippy::todo,
	clippy::undocumented_unsafe_blocks,
	clippy::unneeded_field_pattern,
	clippy::unseparated_literal_suffix,
	clippy::unwrap_in_result,

	macro_use_extern_crate,
	missing_copy_implementations,
	missing_docs,
	non_ascii_idents,
	trivial_casts,
	trivial_numeric_casts,
	unused_crate_dependencies,
	unused_extern_crates,
	unused_import_braces,
)]

#![expect(clippy::redundant_pub_crate, reason = "Unresolvable.")]

mod args;
mod chk;
mod error;
mod img;
mod iter;



use args::Settings;
use chk::Checksum;
use crossbeam_channel::Receiver;
use dactyl::NiceElapsed;
use error::PxsumError;
use fyi_msg::{
	Msg,
	MsgKind,
};
use img::{
	PxImage,
	PxKind,
};
use iter::ManifestLines;
use std::{
	borrow::Cow,
	collections::{
		BTreeMap,
		BTreeSet,
	},
	ffi::OsString,
	fs::File,
	io::{
		BufRead,
		BufReader,
	},
	num::{
		NonZeroU64,
		NonZeroUsize,
	},
	path::Path,
	sync::{
		Mutex,
		Once,
		atomic::{
			AtomicBool,
			AtomicU64,
			Ordering::{
				Relaxed,
				SeqCst,
			},
		},
	},
	thread,
	time::Instant,
};



// See build.rs.
include!(concat!(env!("OUT_DIR"), "/pxsum-ext.rs"));



/// # STDIN Used?
static STDIN_USED: Once = Once::new();



/// # Main.
fn main() {
	#[cold]
	/// # Print Execution Time.
	fn print_time(from: Instant) {
		Msg::done(format!("Finished in {}.", NiceElapsed::from(from)))
			.with_newline(true)
			.eprint();
	}

	// We don't know yet if time tracking will be needed, but have to get the
	// start time logged just in case.
	let now = Instant::now();
	let mut bench = false;

	// The _main() method does all the hard work, but some responses warrant
	// additional output.
	match _main(&mut bench) {
		Ok(()) => if bench { print_time(now); },
		Err(e @ (PxsumError::PrintHelp | PxsumError::PrintVersion)) => { println!("{e}"); },
		Err(e) => {
			// Print the message.
			let code = e.exit_code();
			Msg::new(
				if code == 1 { MsgKind::Error } else { MsgKind::Warning },
				e.to_string(),
			)
				.with_newline(true)
				.eprint();

			// Bench time?
			if bench { print_time(now); }

			// Exit appropriately.
			std::process::exit(code);
		},
	}
}

#[inline]
/// # Actual Main.
fn _main(print_time: &mut bool) -> Result<(), PxsumError> {
	// Parse CLI arguments.
	let (settings, paths) = Settings::new()?;

	// Note whether the user wants the execution time printed. Doing that now
	// ensures it'll happen even if we run into errors during processing.
	if settings.print_time() { *print_time = true; }

	// Verification mode.
	if settings.check() { verify_paths(&paths, settings) }
	// Regular ol' crunch.
	else { crunch_paths(&paths, settings) }
}

#[inline(never)]
/// # Crunch Paths.
///
/// Calculate and output new pxsum/path pairs.
fn crunch_paths(paths: &[OsString], settings: Settings)
-> Result<(), PxsumError> {
	/// # Anything?
	///
	/// A simple flag to indicate that we successfully crunched at least one
	/// image file.
	static ANY: AtomicBool = AtomicBool::new(false);

	/// # Paths by Checksum.
	///
	/// This is used for `-g`/`--group-by-checksum`.
	static GROUPED: Mutex<BTreeMap<[u8; 32], BTreeSet<String>>> = Mutex::new(BTreeMap::new());

	/// # Worker Callback.
	fn cb(rx: &Receiver::<&Path>, settings: &Settings) {
		let mut chk = Checksum::new(settings.strict());
		let print_warnings = settings.print_warnings();
		let group_by_checksum = settings.group_by_checksum();
		while let Ok(p) = rx.recv() {
			match chk.crunch(p) {
				Ok(()) =>
					// Collect the results for later.
					if group_by_checksum {
						let mut ptr = match GROUPED.lock() {
							Ok(guard) => guard,
							Err(poisoned) => poisoned.into_inner(),
						};
						ptr.entry(chk.chk())
							.or_default()
							.insert(chk.src().to_owned());
					}
					// Print now!
					else {
						ANY.store(true, Relaxed);
						println!("{chk}");
					},
				Err(PxsumError::Path | PxsumError::NoData) => {}, // Silently ignore.
				Err(_) => if print_warnings {
					let mut src = Cow::Borrowed(chk.src());
					if src.is_empty() { src = p.to_string_lossy(); }
					Msg::warning(format!(
						"Image could not be decoded.\n         \x1b[2m{src}\x1b[0m",
					)).eprint();
				},
			}
		}
	}

	#[cold]
	/// # Print Results Grouped by Checksum.
	fn print_grouped(only_dupes: bool) -> Result<(), PxsumError> {
		use std::io::Write;
		let mut any = false;
		let mut buf = [0_u8; 64];

		{
			let mut lock = std::io::stdout().lock();
			for (k, v) in GROUPED.lock().map_err(|_| PxsumError::JobServer)?.iter() {
				if ! only_dupes || 1 < v.len() {
					// Our buffer is the right size; this should never fail.
					if let Ok(chk) = faster_hex::hex_encode(k.as_slice(), buf.as_mut_slice()) {
						any = true;
						let _res = writeln!(&mut lock, "{chk}");
						for path in v {
							let _res = writeln!(&mut lock, "  {path}");
						}
					}
				}
			}
			let _res = lock.flush();
		}

		// Warnings?
		if any { Ok(()) }
		else if only_dupes { Err(PxsumError::NoDupes) }
		else { Err(PxsumError::Noop) }
	}

	// If there are fewer paths than threads, we can reduce the worker count.
	let mut threads = settings.threads();
	let Some(len) = NonZeroUsize::new(paths.len()) else { return Ok(()); };
	if len < threads { threads = len; }

	let (tx, rx) = crossbeam_channel::bounded::<&Path>(threads.get());
	thread::scope(#[inline(always)] |s| {
		// Set up the worker threads, either with or without progress.
		let mut workers = Vec::with_capacity(threads.get());
		for _ in 0..threads.get() {
			workers.push(s.spawn(#[inline(always)] || cb(&rx, &settings)));
		}

		// Broadcast the jobs!
		for p in paths { tx.send(p.as_ref()).map_err(|_| PxsumError::JobServer)?; }

		// Disconnect and wait for the threads to finish!
		drop(tx);
		for worker in workers { let _res = worker.join(); }

		// We're all good if we did at least one thing, but if not, emit an
		// error so we can let the user know.
		if settings.group_by_checksum() { print_grouped(settings.only_dupes()) }
		else if ANY.load(SeqCst) { Ok(()) }
		else { Err(PxsumError::Noop) }
	})
}

/// # STDIN Lock.
///
/// This method is used as a thin wrapper around STDIN to ensure the lock is
/// only requested once and only returned if it is being redirected.
///
/// This should hopefully help prevent indefinite hangs or other weirdness.
fn stdin() -> Result<std::io::StdinLock<'static>, PxsumError> {
	use std::io::IsTerminal;

	// If this is the first call, `available` should be `true` afterwards.
	let mut available = false;
	STDIN_USED.call_once(|| { available = true; });

	if available {
		let stdin = std::io::stdin();
		if stdin.is_terminal() { Err(PxsumError::Stdin) }
		else { Ok(stdin.lock()) }
	}
	else { Err(PxsumError::Stdin) }
}

#[inline(never)]
/// # Verify Paths.
///
/// Verify existing paths and maybe print their statuses.
fn verify_paths(paths: &[OsString], settings: Settings)
-> Result<(), PxsumError> {
	/// # Mismatched Path Count.
	static FAILED: AtomicU64 = AtomicU64::new(0);

	/// # Worker Callback.
	fn cb(rx: &Receiver::<String>, settings: &Settings) {
		let mut chk = Checksum::new(false);
		let print_valid =   settings.print_valid();
		let print_warnings = settings.print_warnings();

		while let Ok(line) = rx.recv() {
			match chk.verify_existing(line.as_str()) {
				Ok(true) => if print_valid {
					println!("{}: OK", chk.src());
				},
				Ok(false) => {
					FAILED.fetch_add(1, Relaxed);
					println!("{}: FAILED", chk.src());
				},
				Err(PxsumError::LineDecode | PxsumError::Path) => if print_warnings {
					Msg::warning(format!(
						"Malformed pxsum/path line.\n         \x1b[2m{line}\x1b[0m"
					)).eprint();
				},
				Err(e) => {
					FAILED.fetch_add(1, Relaxed);
					println!(
						"{}: FAILED ({})",
						chk.src(),
						if matches!(e, PxsumError::NoData) { "empty" }
						else if ! Path::new(chk.src()).exists() { "missing" }
						else { "read/decode" }
					);
				},
			}
		}
	}

	let threads = settings.threads();
	let (tx, rx) = crossbeam_channel::bounded::<String>(threads.get());
	thread::scope(#[inline(always)] |s| {
		// Set up the worker threads, either with or without progress.
		let mut workers = Vec::with_capacity(threads.get());
		for _ in 0..threads.get() {
			workers.push(s.spawn(#[inline(always)] || cb(&rx, &settings)));
		}

		// Broadcast the jobs!
		for p in paths {
			// Read from STDIN.
			if p == "-" {
				for line in ManifestLines::new(stdin()?.lines().map_while(Result::ok)) {
					tx.send(line).map_err(|_| PxsumError::JobServer)?;
				}
			}
			// Read from File.
			else {
				let mut read = ! settings.print_warnings();
				if let Ok(lines) = File::open(p).map(|f| BufReader::new(f).lines()) {
					for line in ManifestLines::new(lines.map_while(Result::ok)) {
						read = true;
						tx.send(line).map_err(|_| PxsumError::JobServer)?;
					}
				}

				// Well that didn't work!
				if ! read {
					Msg::warning(format!(
						"Invalid pxsum manifest.\n         \x1b[2m{}\x1b[0m",
						p.to_string_lossy(),
					)).eprint();
				}
			}
		}

		// Disconnect and wait for the threads to finish!
		drop(tx);
		for worker in workers { let _res = worker.join(); }

		// If any verifications failed, we want to print a warning and exit
		// with a non-zero code.
		NonZeroU64::new(FAILED.load(SeqCst)).map_or(
			Ok(()),
			|failed| Err(PxsumError::Failed(failed))
		)
	})
}
