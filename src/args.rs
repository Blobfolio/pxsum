/*!
# pxsum: Cli Arguments.
*/

use crate::PxsumError;
use dactyl::traits::BytesToUnsigned;
use std::{
	ffi::{
		OsString,
		OsStr,
	},
	num::NonZeroUsize,
	os::unix::ffi::{
		OsStringExt,
		OsStrExt,
	},
};
use trimothy::TrimMut;
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
		Self::from_iter(std::env::args_os().map(OsStringExt::into_vec).skip(1))
	}

	/// # From Arguments Iterator.
	///
	/// Any sort of `Vec`-iterating set of arguments will do.
	///
	/// This is a bit much, but just this side of "worth it" as we don't have
	/// too many arguments to worry about. So long as we're under the clippy
	/// line limit I think it's okay. Haha.
	fn from_iter<I>(raw: I) -> Result<(Self, Vec<OsString>), PxsumError>
	where I: Iterator<Item=Vec<u8>> {
		// So much setup!
		let mut flags = Self::PRINT_VALID | Self::PRINT_WARNINGS;
		let mut threads = std::thread::available_parallelism().unwrap_or(NonZeroUsize::MIN);
		let mut dirs: Vec<OsString> = Vec::new();
		let mut paths: Vec<OsString> = Vec::new();
		let mut last = CurrentKey::None;

		// Loop the loop!
		for mut src in raw {
			src.trim_mut();
			if src.is_empty() {
				last = CurrentKey::None;
				continue;
			}

			match src.as_slice() {
				// --bench
				[b'-', b'-', b'b', b'e', b'n', b'c', b'h', ] => { flags |= Self::PRINT_TIME; },

				// -c / --check
				[b'-', b'c'] |
				[b'-', b'-', b'c', b'h', b'e', b'c', b'k']  => { flags |= Self::CHECK; },

				// -g / --group-by-checksum
				[b'-', b'g'] |
				[b'-', b'-', b'g', b'r', b'o', b'u', b'p', b'-', b'b', b'y', b'-', b'c', b'h', b'e', b'c', b'k', b's', b'u', b'm']  => { flags |= Self::GROUP_BY_CHECKSUM; },

				// --no-warnings
				[b'-', b'-', b'n', b'o', b'-', b'w', b'a', b'r', b'n', b'i', b'n', b'g', b's']  => { flags &= ! Self::PRINT_WARNINGS; },

				// --only-dupes
				[b'-', b'-', b'o', b'n', b'l', b'y', b'-', b'd', b'u', b'p', b'e', b's']  => { flags |= Self::ONLY_DUPES; },

				// -q / --quiet
				[b'-', b'q'] |
				[b'-', b'-', b'q', b'u', b'i', b'e', b't']  => { flags &= ! Self::PRINT_VALID; },

				// --strict
				[b'-', b'-', b's', b't', b'r', b'i', b'c', b't']  => { flags |= Self::STRICT; },

				// -d / --dir
				[b'-', b'd'] |
				[b'-', b'-', b'd', b'i', b'r'] => {
					last = CurrentKey::Dir;
					continue;
				},

				// -d / --dir <DIR>
				[b'-', b'd', rest @ ..] |
				[b'-', b'-', b'd', b'i', b'r', b'=', rest @ ..] => {
					let rest = rest.trim_ascii_start();
					if rest.is_empty() {
						last = CurrentKey::Dir;
						continue;
					}

					// The value was included.
					src.drain(..src.len() - rest.len());
					dirs.push(OsString::from_vec(src));
				},

				// -j
				[b'-', b'j', rest @ ..]  => {
					let rest = rest.trim_ascii_start();
					if rest.is_empty() {
						last = CurrentKey::Threads;
						continue;
					}

					// The value was included.
					set_threads(&mut threads, rest);
				},

				// -h / --help
				[b'-', b'h'] |
				[b'-', b'-', b'h', b'e', b'l', b'p'] => return Err(PxsumError::PrintHelp),

				// -V / --version
				[b'-', b'V'] |
				[b'-', b'-', b'v', b'e', b'r', b's', b'i', b'o', b'n'] => return Err(PxsumError::PrintVersion),

				// Everything else!
				rest => match last {
					// Directory.
					CurrentKey::Dir => { dirs.push(OsString::from_vec(src)); },

					// Threads.
					CurrentKey::Threads => { set_threads(&mut threads, rest); },

					// Something else…
					CurrentKey::None => { paths.push(OsString::from_vec(src)); },
				},
			};

			// All but two cases require this to reset.
			last = CurrentKey::None;
		}

		// Finish up with some path work, unless -c/--check got set.
		if 0 == flags & Self::CHECK {
			// Go ahead and drop paths that don't have a proper extension.
			paths.retain(|p| crate::check_extension(p.as_bytes()));

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

	/// # Checksum w/ Invisible Pixels.
	const STRICT: u8 =            0b0000_1000;

	/// # Print Total Execution Time.
	const PRINT_TIME: u8 =        0b0001_0000;

	/// # Print Verified (OK) Files.
	const PRINT_VALID: u8 =       0b0010_0000;

	/// # Print Read/Decode/Formatting Warnings.
	const PRINT_WARNINGS: u8 =    0b0100_0000;

	get!(
		"Verification Mode", check, CHECK,
		"Group by Checksum", group_by_checksum, GROUP_BY_CHECKSUM,
		"Only Report (Grouped) Duplicates", only_dupes, ONLY_DUPES,
		"Strict Checksums", strict, STRICT,
		"Print Total Execution Time.", print_time, PRINT_TIME,
		"Print Verified (OK) Files.", print_valid, PRINT_VALID,
		"Print Image Warnings", print_warnings, PRINT_WARNINGS,
	);

	/// # Threads.
	pub(super) const fn threads(&self) -> NonZeroUsize { self.threads }
}



#[derive(Clone, Copy)]
/// # Options With Values.
///
/// This is a placeholder for 2-part key/value pairs so we know whether a given
/// arg applies to itself or a previously-defined key.
enum CurrentKey {
	/// # Not an option.
	None,

	/// # Directory.
	Dir,

	/// # Max Worker Threads.
	Threads,
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



#[cfg(test)]
mod test {
	use super::*;

	/// # Tests for Version/Help.
	///
	/// The version and help flags work the same way, causing parsing to abort
	/// with an error. This method consolidates the assertions to verify that.
	fn run_version_help(short: &[u8], long: &[u8], err: PxsumError) {
		// Straight cases.
		assert_eq!(
			Settings::from_iter(std::iter::once(short.to_vec())).expect_err("Help not detected."),
			err,
		);
		assert_eq!(
			Settings::from_iter(std::iter::once(long.to_vec())).expect_err("Help not detected."),
			err,
		);

		// Mixed args short.
		let many = [
			Vec::new(),
			b"skel/assets/carl.jpg".to_vec(),
			short.to_vec(),
			b"skel/assets/empty.jpg".to_vec(),
		];
		assert_eq!(
			Settings::from_iter(many.into_iter()).expect_err("Help not detected."),
			err,
		);

		// Mixed args long.
		let many = [
			Vec::new(),
			b"skel/assets/carl.jpg".to_vec(),
			long.to_vec(),
			b"skel/assets/empty.jpg".to_vec(),
		];
		assert_eq!(
			Settings::from_iter(many.into_iter()).expect_err("Help not detected."),
			err,
		);
	}

	#[test]
	fn t_settings_help() {
		run_version_help(b"-h", b"--help", PxsumError::PrintHelp);
	}

	#[test]
	fn t_settings_version() {
		run_version_help(b"-V", b"--version", PxsumError::PrintVersion);
	}

	#[test]
	fn t_settings_empty() {
		let (settings, paths) = Settings::from_iter([].into_iter())
			.expect("Zero args failed.");

		// Optionals should be off.
		assert!(! settings.check());
		assert!(! settings.group_by_checksum());
		assert!(! settings.only_dupes());
		assert!(! settings.print_time());
		assert!(! settings.strict());

		// The defaults.
		assert!(settings.print_valid());
		assert!(settings.print_warnings());
		assert_eq!(paths, &["-"]);
	}

	#[test]
	/// # Test Flags.
	///
	/// The flag-parsing is a little messy, so this helps make sure all
	/// variants are correctly detected and parsed.
	fn t_settings_flags() {
		macro_rules! toggle_flag {
			($flag:expr, $fn:ident, $val:literal) => (
				// Without It.
				let many = [
					Vec::new(),
					b"skel/assets/carl.jpg".to_vec(),
					b"skel/assets/empty.jpg".to_vec(),
				];
				let (settings, _) = Settings::from_iter(many.into_iter())
					.expect("Settings failed.");
				assert_eq!(settings.$fn(), $val);

				// With It.
				let many = [
					Vec::new(),
					b"skel/assets/carl.jpg".to_vec(),
					$flag,
					b"skel/assets/empty.jpg".to_vec(),
				];
				let (settings, _) = Settings::from_iter(many.into_iter())
					.expect("Settings failed.");
				assert_eq!(settings.$fn(), ! $val);
			);
		}

		toggle_flag!(b"--bench".to_vec(), print_time, false);

		toggle_flag!(b"-c".to_vec(), check, false);
		toggle_flag!(b"--check".to_vec(), check, false);

		toggle_flag!(b"-g".to_vec(), group_by_checksum, false);
		toggle_flag!(b"--group-by-checksum".to_vec(), group_by_checksum, false);

		toggle_flag!(b"--no-warnings".to_vec(), print_warnings, true);

		toggle_flag!(b"-q".to_vec(), print_valid, true);
		toggle_flag!(b"--quiet".to_vec(), print_valid, true);

		toggle_flag!(b"--strict".to_vec(), strict, false);

		// This one toggles two different options.
		toggle_flag!(b"--only-dupes".to_vec(), only_dupes, false);
		toggle_flag!(b"--only-dupes".to_vec(), group_by_checksum, false);
	}
}
