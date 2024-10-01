/*!
# pxsum: Checksums.
*/

use crate::{
	PxImage,
	PxKind,
	PxsumError,
};
use std::{
	ffi::OsStr,
	fmt,
	fs::File,
	io::BufReader,
};



#[derive(Debug, Clone, Eq, PartialEq)]
/// # Checksum.
///
/// This struct serves as a reusable de/encoder of sorts for each worker
/// thread, helping to reduce the number of allocations made over the course of
/// the run.
pub(super) struct Checksum {
	/// # Image Path.
	src: String,

	/// # Checksum.
	chk: [u8; 32],

	/// # File Buffer.
	buf: Vec<u8>,
}

impl fmt::Display for Checksum {
	/// # Format.
	///
	/// This prints a pxsum/path pairing in the same style used by `md5sum`,
	/// `b3sum`, etc.: the hex hash + two spaces + the path.
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		if self.src.is_empty() { Ok(()) }
		else {
			// The hex first.
			let mut buf = [0_u8; 64];
			let chk = faster_hex::hex_encode(self.chk.as_slice(), buf.as_mut_slice())
				.map_err(|_| fmt::Error)?;
			f.write_str(chk)?;

			// Two spaces.
			f.write_str("  ")?;

			// The path.
			f.write_str(self.src.as_str())
		}
	}
}

impl Checksum {
	/// # Strictness Flag.
	///
	/// This bit is used to indicate that all pixel colors — even invisible
	/// ones — should be factored into the checksum.
	///
	/// When active, it is encoded into the first byte of the checksum so that
	/// subsequent verification can infer which mode to use.
	pub(super) const STRICT: u8 = 0b0000_0001;

	/// # New (Empty) Instance.
	///
	/// Return a fresh instance that can be reused for any number of new or
	/// existing checksums.
	///
	/// Note the `strict` flag is only relevant for `Checksum::crunch`;
	/// verification uses the strictness of the reference checksum(s) instead.
	pub(super) const fn new(strict: bool) -> Self {
		let mut chk = [0_u8; 32];
		if strict { chk[0] |= Self::STRICT; }

		Self {
			src: String::new(),
			chk,
			buf: Vec::new(),
		}
	}

	/// # Crunch a Checksum Given a File Path.
	///
	/// Replace `self` with a new checksum/path pairing.
	///
	/// ## Errors
	///
	/// This will return an error if the path is invalid or unreadable, or the
	/// data is missing or cannot be decoded into a valid RGBA image.
	pub(super) fn crunch<P>(&mut self, src: P) -> Result<(), PxsumError>
	where P: AsRef<OsStr> {
		src.as_ref().to_str().ok_or(PxsumError::Path).and_then(|s| self.set_path(s))?;
		let fmt = self.read_raw()?;
		self.chk = PxImage::new(self.buf.as_slice(), fmt)?.into_checksum(self.strict());

		Ok(())
	}

	/// # Return Checksum.
	pub(super) const fn chk(&self) -> [u8; 32] { self.chk }

	/// # Return Source Path.
	pub(super) fn src(&self) -> &str { &self.src }

	/// # Verify a Checksum.
	///
	/// Replace `self` with the checksum/path pairing stored in `line`, then
	/// recrunch the data to see if it's still a match (`true`).
	///
	/// ## Errors
	///
	/// In addition to the errors returnable by `Self::crunch`, this will
	/// fail if the line cannot be parsed.
	pub(super) fn verify_existing(&mut self, line: &str) -> Result<bool, PxsumError> {
		// Clear the current source path early in case the line is corrupt.
		self.src.truncate(0);

		// Split the two parts.
		let (a, mut b) = line.split_at_checked(64).ok_or(PxsumError::LineDecode)?;
		b = b.strip_prefix("  ").ok_or(PxsumError::LineDecode)?;

		// De-hex the checksum.
		faster_hex::hex_decode(a.as_bytes(), self.chk.as_mut_slice())
			.map_err(|_| PxsumError::LineDecode)?;

		// Now basically do the same thing as crunch, but use the result for
		// comparison instead of making any changes to `self`.
		self.set_path(b)?;
		let fmt = self.read_raw()?;

		// Do we have a match?
		let chk = PxImage::new(self.buf.as_slice(), fmt)?.into_checksum(self.strict());
		Ok(self.chk == chk)
	}

	/// # Read Source.
	///
	/// This attempts to read the source — a file or STDIN — into the reusable
	/// buffer.
	///
	/// ## Errors
	///
	/// This will return an error if the data cannot be read or winds up empty,
	/// but does not otherwise validate the raw bytes.
	fn read_raw(&mut self) -> Result<PxKind, PxsumError> {
		use std::io::Read;

		#[inline]
		/// # Digest Reader.
		///
		/// The STDIN and path-based reads differ in setup, but finish the same
		/// way. This method helps remove all that trailing redundancy.
		fn digest_reader<R: Read>(r: &mut R, buf: &mut Vec<u8>)
		-> Result<PxKind, PxsumError> {
			// Read just enough to guess the image format; if we can't do this
			// much there's no point in continuing!
			buf.resize(16_usize, 0_u8);
			r.read_exact(buf.as_mut_slice()).map_err(|_| PxsumError::Read)?;
			let fmt = PxKind::try_from_magic(buf.as_slice())?;

			// Finish the job!
			r.read_to_end(buf).map_err(|_| PxsumError::Read)?;
			Ok(fmt)
		}

		// Read from STDIN.
		if self.stdin() {
			crate::stdin().and_then(|mut r| digest_reader(&mut r, &mut self.buf))
		}
		// Read from file.
		else {
			// Open the file and obtain its size.
			let file = File::open(self.src.as_str()).map_err(|_| PxsumError::Read)?;
			let meta = file.metadata().map_err(|_| PxsumError::Read)?;
			let len = usize::try_from(meta.len()).map_err(|_| PxsumError::Read)?;

			// Easy errors.
			if len == 0 { return Err(PxsumError::NoData); }
			else if len < 16 { return Err(PxsumError::Decode); }

			// Reserve and read!
			if let Some(diff) = len.checked_sub(self.buf.capacity()) {
				self.buf.try_reserve_exact(diff).map_err(|_| PxsumError::Read)?;
			}
			digest_reader(&mut BufReader::new(file), &mut self.buf)
		}
	}

	/// # Set Path.
	///
	/// Replace `self.src` with the specified path, lightly normalizing it in
	/// the process.
	///
	/// ## Errors
	///
	/// If the path contains invalid UTF-8 sequences, Windows bullshit, or does
	/// not end with a "normal" component, an error will be returned instead.
	fn set_path(&mut self, path: &str) -> Result<(), PxsumError> {
		// First things first, destroy self.
		self.src.truncate(0);

		// Special case: STDIN.
		let path = path.trim();
		if path.is_empty() || path == "-" {
			self.src.push('-');
			return Ok(());
		}

		// Easy abort: unsupported extension.
		if ! crate::check_extension(path.as_bytes()) {
			return Err(PxsumError::Path);
		}

		// If the path has no directory at the start, add one for consistency.
		// The "last" variable will be called into use a little further on…
		let mut last =
			if
				! path.starts_with('/') &&
				! path.starts_with("./") &&
				! path.starts_with("../")
			{
				self.src.push_str("./");
				'/'
			}
			else { '?' };

		// Space shouldn't be a problem…
		if self.src.try_reserve(path.len()).is_err() {
			self.src.truncate(0);
			return Err(PxsumError::Path);
		}

		// Run character by character, but stop if there's an error.
		for c in path.chars() {
			// Collapse double-slashes.
			if last == '/' && c == '/' { continue; }

			// No backslashes or control characters are allowed.
			if c.is_control() || c == '\\' {
				self.src.truncate(0);
				return Err(PxsumError::Path);
			}

			// Keep it!
			last = c;
			self.src.push(c);
		}

		// Remove pointless /./ sequences.
		// TODO: use remove_matches once stable.
		while let Some(pos) = self.src.find("/./") {
			self.src.replace_range(pos..pos + 2, "");
		}

		// We're good so long as we don't have an impossible parent-of-root
		// situation.
		if self.src.starts_with("/../") {
			self.src.truncate(0);
			Err(PxsumError::Path)
		}
		else { Ok(()) }
	}
}

impl Checksum {
	/// # Source is STDIN?
	fn stdin(&self) -> bool { self.src.is_empty() || self.src == "-" }

	/// # Checksums in Strict Mode?
	const fn strict(&self) -> bool { Self::STRICT == self.chk[0] & Self::STRICT }
}



#[cfg(test)]
mod test {
	use super::*;

	/// # Verify List.
	///
	/// We have pre-generated manifests for our test assets in both loose and
	/// strict modes, but the paths need to be adjusted because of relativity.
	fn run_check(path: &str) {
		let Ok(list) = std::fs::read_to_string(path) else {
			panic!("Unable to read {path}.");
		};

		let mut chk = Checksum::new(false);
		for line in list.lines() {
			let line = line.trim();
			if line.is_empty() { continue; }

			// We need to adjust the paths.
			let new_line = line.replace("  ./assets/", "  ./skel/assets/");
			assert_eq!(
				chk.verify_existing(&new_line),
				Ok(true),
				"Verification line failed: {line}",
			);
		}
	}

	#[test]
	/// # Test Pixel Collection.
	///
	/// The documentation doesn't make it very clear, but `RgbaImage::into_vec`
	/// should yield all the pixel values (in RGBA order), same as if we were
	/// to slice and collect the `Pixels` iterator.
	///
	/// This makes hashing a lot more efficient, but we should test our
	/// understanding is correct!
	fn t_pixels() {
		use image::Pixel;

		// Any image will do.
		let img = image::open("skel/assets/carl.jpg")
			.expect("Failed to open carl.jpg")
			.into_rgba8();

		// Collected via iterator.
		let manual: Vec<u8> = img.pixels()
			.map(|p| p.channels())
			.flatten()
			.copied()
			.collect::<Vec<u8>>();

		// Stolen.
		let automatic = img.into_vec();

		// They should match!
		assert_eq!(manual, automatic);
	}

	#[test]
	/// # Check Loose Checksums.
	fn t_check() { run_check("skel/loose.chk"); }

	#[test]
	/// # Check Strict Checksums.
	fn t_check_strict() { run_check("skel/strict.chk"); }
}
