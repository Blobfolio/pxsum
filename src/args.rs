/*!
# pxsum: Cli Arguments.
*/

use argyle::Argument;
use crate::PxsumError;
use dactyl::traits::BytesToUnsigned;
use std::{
	ffi::{
		OsString,
		OsStr,
	},
	num::NonZeroUsize,
	os::unix::ffi::OsStrExt,
};
use walkdir::WalkDir;



#[derive(Debug, Clone, Copy)]
/// # Runtime Settings.
pub(super) struct Settings {
	/// # Flags.
	flags: u8,

	/// # Max Parallelism.
	threads: NonZeroUsize,
}

impl Settings {
	/// # From CLI Arguments.
	pub(super) fn new() -> Result<(Self, Vec<OsString>), PxsumError> {
		let args = argyle::args()
			.with_keywords(include!(concat!(env!("OUT_DIR"), "/argyle.rs")));

		let mut flags = Self::PRINT_VALID | Self::PRINT_WARNINGS;
		let mut threads = std::thread::available_parallelism().unwrap_or(NonZeroUsize::MIN);
		let mut dirs: Vec<OsString> = Vec::new();
		let mut paths: Vec<OsString> = Vec::new();
		for arg in args {
			match arg {
				Argument::Key("--bench") => { flags |= Self::PRINT_TIME; },
				Argument::Key("-c" | "--check") => { flags |= Self::CHECK; },
				Argument::Key("-g" | "--group-by-checksum") => { flags |= Self::GROUP_BY_CHECKSUM; },
				Argument::Key("-h" | "--help") => return Err(PxsumError::PrintHelp),
				Argument::Key("--no-warnings") => { flags &= ! Self::PRINT_WARNINGS; },
				Argument::Key("--only-dupes") => { flags |= Self::ONLY_DUPES; },
				Argument::Key("-q" | "--quiet") => { flags &= ! Self::PRINT_VALID; },
				Argument::Key("--split-by-type") => { flags |= Self::SPLIT_BY_TYPE; },
				Argument::Key("--strict") => { flags |= Self::STRICT; },
				Argument::Key("-V" | "--version") => return Err(PxsumError::PrintVersion),

				Argument::KeyWithValue("-d" | "--dir", s) => {
					dirs.push(OsString::from(s));
				},
				Argument::KeyWithValue("-j", s) => {
					set_threads(&mut threads, s.as_bytes());
				},

				// Path!
				Argument::Path(s) => { paths.push(s); },

				// Mistake?
				Argument::Other(s) => return Err(PxsumError::InvalidCli(s)),
				Argument::InvalidUtf8(s) => return Err(PxsumError::InvalidCli(s.to_string_lossy().into_owned())),
				_ => {},
			}
		}

		// Finish up with some path work, unless -c/--check got set.
		if 0 == flags & Self::CHECK {
			// Note whether we're empty to start with.
			let empty =
				if paths.is_empty() { true }
				else {
					// Go ahead and drop paths without proper extensions.
					paths.retain(|p| crate::check_extension(p.as_bytes()));
					false
				};

			// And crawl any directories requested.
			for d in dirs {
				for e in WalkDir::new(d).follow_links(true).into_iter().flatten() {
					if
						! e.file_type().is_dir() &&
						crate::check_extension(e.path().as_os_str().as_bytes())
					{
						paths.push(e.into_path().into_os_string());
					}
				}
			}

			// If we weren't empty before but are now, return an error.
			if ! empty && paths.is_empty() { return Err(PxsumError::Noop); }
		}

		// Path touch-ups.
		if paths.is_empty() {
			paths.push(OsStr::new("-").to_owned());
			threads = NonZeroUsize::MIN;
		}
		else {
			paths.sort_unstable();
			paths.dedup();
		}

		// Done!
		Ok((Self { flags, threads }, paths))
	}
}

/// # Helper: Getters.
macro_rules! get {
	($($title:literal, $fn:ident, $flag:ident),+ $(,)*) => ($(
		#[doc = concat!("# ", stringify!($title), "?")]
		pub(super) const fn $fn(&self) -> bool {
			Self::$flag == self.flags & Self::$flag
		}
	)+);
}

impl Settings {
	/// # Verification Mode.
	const CHECK: u8 =             0b0000_0001;

	/// # Group Output by Checksum.
	const GROUP_BY_CHECKSUM: u8 = 0b0000_0010;

	/// # Only Report (Grouped) Dupes.
	const ONLY_DUPES: u8 =        0b0000_0110; // Implies GROUP_BY_CHECKSUM.

	/// # Split by Type.
	const SPLIT_BY_TYPE: u8 =     0b0000_1000;

	/// # Checksum w/ Invisible Pixels.
	const STRICT: u8 =            0b0001_0000;

	/// # Print Total Execution Time.
	const PRINT_TIME: u8 =        0b0010_0000;

	/// # Print Verified (OK) Files.
	const PRINT_VALID: u8 =       0b0100_0000;

	/// # Print Read/Decode/Formatting Warnings.
	const PRINT_WARNINGS: u8 =    0b1000_0000;

	get!(
		"Verification Mode", check, CHECK,
		"Group by Checksum", group_by_checksum, GROUP_BY_CHECKSUM,
		"Only Report (Grouped) Duplicates", only_dupes, ONLY_DUPES,
		"Split by Type (when grouping by checksum)", split_by_type, SPLIT_BY_TYPE,
		"Strict Checksums", strict, STRICT,
		"Print Total Execution Time.", print_time, PRINT_TIME,
		"Print Verified (OK) Files.", print_valid, PRINT_VALID,
		"Print Image Warnings", print_warnings, PRINT_WARNINGS,
	);

	/// # Threads.
	pub(super) const fn threads(&self) -> NonZeroUsize { self.threads }
}



/// # Set Threads.
///
/// This method parses the requested user value (in raw byte form) into a
/// `NonZeroUsize` and replaces the default `threads` value if smaller.
///
/// (If negative, `threads` is decreased accordingly.)
fn set_threads(threads: &mut NonZeroUsize, wanted: &[u8]) {
	let wanted = wanted.trim_ascii();
	if let Some(t) = wanted.strip_prefix(b"-").and_then(NonZeroUsize::btou) {
		*threads = threads.get().checked_sub(t.get())
			.and_then(NonZeroUsize::new)
			.unwrap_or(NonZeroUsize::MIN);
	}
	else if let Some(t) = NonZeroUsize::btou(wanted) {
		if t < *threads { *threads = t; }
	}
}
