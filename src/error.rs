/*!
# pxsum: Errors.
*/

use image::error::ImageError;
use std::{
	error::Error,
	fmt,
	num::NonZeroU64,
};



/// # Help Text.
///
/// It's long, but at least it's static!
const HELP: &str = concat!(
	r"
,_     _
 |\\_,-~/
 / _  _ |    ,--.
(  @  @ )   / ,-'
 \  _T_/-._( (
 /         `. \
|         _  \ |
 \ \ ,  /      |   ", "\x1b[38;5;199mpxsum\x1b[0;38;5;69m v", env!("CARGO_PKG_VERSION"), "\x1b[0m", r#"
  || |-_\__   /    Checksum decoded
 ((_/`(____,-'     image pixel data.

USAGE:
    pxsum [FLAGS] [OPTIONS] [FILE(S)]...

FLAGS:
        --bench           Print the total execution time before exiting.
    -c, --check           Read existing pxsum/path pairs from FILE(S) and
                          check if they still ring true. This takes priority
                          over crunch-specific options, like -d/--dir.
    -g, --group-by-checksum
                          Crunch as usual, but group the results by checksum.
                          Note this will delay output until the end of the run.
    -h, --help            Print help information and exit.
        --no-warnings     Suppress warnings related to image decoding when
                          crunching anew, and malformed check manifest lines
                          when -c/--check.
        --only-dupes      Same as -g/--group-by-checksum, but only checksums
                          with two or more matching images will be printed.
    -q, --quiet           Suppress OK messages in -c/--check mode.
        --strict          Include color data from invisible pixels in checksum
                          calculations.
    -V, --version         Print version information and exit.

OPTIONS:
    -d, --dir <DIR>       Recursively search <DIR> for image files and pxsum
                          them (along with any other FILE(S)). Has no effect
                          when -c/--check.
    -j <NUM>              Limit parallelization to this many threads (instead
                          of giving each logical core its own image to work
                          on). If negative, the value will be subtracted from
                          the total number of logical cores.

ARGS:
    [FILE(S)]...          One or more image file paths to checksum, or if
                          -c/--check, one or more text file paths containing
                          pxsums to verify.

                          With no FILE(S) or -, input is read from STDIN.

FORMATS:
    Only image paths with valid file extensions for the following formats are
    supported: AVIF, BMP, GIF, ICO, JPEG, JPEG 2000, JPEG XL, PNG, TIFF, WebP

EXIT CODES:
    0: Business as usual!
    1: Something blew up!
    2: No checksum/path pairs were outputted.
    3: One or more images failed to re-verify.
"#
);



#[derive(Debug, Clone, Copy, Eq, PartialEq)]
/// # Error Type.
///
/// Depending on the context, this enum can be used to indicate a show-stopping
/// error, a warning of some sort (that may or may not be used), or an abort
/// hint for "special" screens like Help and Version.
pub(super) enum PxsumError {
	/// # Image decode failed.
	Decode,

	/// # Job server failed.
	///
	/// This would trigger in the event a `tx.send()` request fails, but that
	/// shouldn't happen in practice.
	JobServer,

	/// # Malformed verification line.
	LineDecode,

	/// # Empty file/stream.
	NoData,

	/// # Nothing Doing.
	///
	/// This error is used when no paths were checksummed, allowing the program
	/// to exit with a different code.
	Noop,

	/// # Same as above, but in dupe mode.
	NoDupes,

	/// # Invalid path.
	Path,

	/// # Print Help.
	///
	/// Not an "error", per se, but demands early abort.
	PrintHelp,

	/// # Print Version.
	///
	/// Not an "error", per se, but demands early abort.
	PrintVersion,

	/// # Source read failed.
	Read,

	/// # STDIN read failed.
	///
	/// This error is used if STDIN is requested twice or is not redirected.
	Stdin,

	/// # Verification Failure(s).
	///
	/// This error is used to indicate the total number of verification
	/// failures, allowing the program to exit with a different code.
	Failed(NonZeroU64),
}

impl fmt::Display for PxsumError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		let string = match self {
			Self::Failed(n) => return write!(
				f,
				"{n} computed checksum{} did NOT match",
				if n.get() ==1 { "" } else { "s" }
			),
			Self::Decode => "Decoding failed.",
			Self::JobServer => "Job server choked!",
			Self::LineDecode => "Invalid pxsum line.",
			Self::NoData => "Empty input.",
			Self::NoDupes => "No duplicate images were found.",
			Self::Noop => "No pixel checksums were computed.",
			Self::Path => "Path is invalid.",
			Self::PrintHelp => HELP,
			Self::PrintVersion => concat!("pxsum v", env!("CARGO_PKG_VERSION")),
			Self::Read => "Unable to read source.",
			Self::Stdin => "Unable to read STDIN."
		};

		f.write_str(string)
	}
}

impl Error for PxsumError {}

impl From<ImageError> for PxsumError {
	#[inline]
	fn from(_src: ImageError) -> Self { Self::Decode }
}

impl PxsumError {
	/// # Exit Code.
	pub(super) const fn exit_code(self) -> i32 {
		match self {
			Self::PrintHelp | Self::PrintVersion => 0,
			Self::Noop | Self::NoDupes => 2,
			Self::Failed(_) => 3,
			_ => 1,
		}
	}
}
