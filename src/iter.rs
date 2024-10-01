/*!
# pxsum: Iterators.
*/



/// # Manifest Line Normalizer.
///
/// Manifests come in both standard and grouped flavors; this iterator is used
/// to map the latter into the former so we don't have to think too much about
/// it when reading/verifying.
///
/// This will trim and ignore empty lines automatically, but otherwise doesn't
/// go out of its way to ensure the data is properly formatted.
pub(super) struct ManifestLines<I: Iterator<Item=String>> {
	/// # Iterator.
	iter: I,

	/// # Group Checksum.
	buf: Option<String>,
}

impl<I: Iterator<Item=String>> ManifestLines<I> {
	/// # New.
	pub(super) const fn new(iter: I) -> Self {
		Self { iter, buf: None }
	}
}

impl<I: Iterator<Item=String>> Iterator for ManifestLines<I> {
	type Item = String;

	fn next(&mut self) -> Option<Self::Item> {
		use trimothy::TrimMut;

		loop {
			// Pull the next line.
			let mut line = self.iter.next()?;
			line.trim_end_mut();
			let len = line.len();

			// Skip empty lines.
			if len == 0 || line.trim_start().is_empty() { continue; }

			// Grouped: new checksum.
			if len == 64 && line.bytes().all(|b| b.is_ascii_hexdigit()) {
				self.buf.replace(line);
				continue;
			}

			// Grouped: new path.
			if line.starts_with("  ") {
				if let Some(chk) = self.buf.as_deref() {
					// Prepend the stored checksum to the path to make it a
					// proper line, then return it.
					line.insert_str(0, chk);
					return Some(line);
				}
			}

			self.buf = None;
			return Some(line);
		}
	}

	fn size_hint(&self) -> (usize, Option<usize>) { self.iter.size_hint() }
}



#[cfg(test)]
mod test {
	use super::*;
	use std::{
		fs::File,
		io::{
			BufRead,
			BufReader,
		},
	};
	use trimothy::TrimMut;

	/// # Reference Checksums.
	///
	/// Read the contents of a file into a vector (of lines), then sort and
	/// return it.
	fn run_read_reference(src: &str) -> Vec<String> {
		let Ok(file) = File::open(src) else { panic!("Unable to read {src}."); };
		let mut out = Vec::new();
		for mut line in BufReader::new(file).lines().map_while(Result::ok) {
			line.trim_mut();
			if ! line.is_empty() { out.push(line); }
		}

		// Sort.
		out.sort_unstable();

		// Return!
		out
	}

	#[test]
	fn t_manifest_lines() {
		let expected = run_read_reference("skel/loose.chk");
		let mut out = Vec::new();
		for src in ["skel/loose.chk", "skel/loose-g.chk"] {
			if let Ok(lines) = File::open(src).map(|f| BufReader::new(f).lines()) {
				out.truncate(0);
				for line in ManifestLines::new(lines.map_while(Result::ok)) {
					out.push(line);
				}
				out.sort_unstable();
				assert_eq!(out, expected, "Normalized {src} mismatches straight read!");
			}
			else {
				panic!("Unable to read {src}.");
			}
		}
	}

	#[test]
	fn t_manifest_lines_strict() {
		let expected = run_read_reference("skel/strict.chk");
		let mut out = Vec::new();
		for src in ["skel/strict.chk", "skel/strict-g.chk"] {
			if let Ok(lines) = File::open(src).map(|f| BufReader::new(f).lines()) {
				out.truncate(0);
				for line in ManifestLines::new(lines.map_while(Result::ok)) {
					out.push(line);
				}
				out.sort_unstable();
				assert_eq!(out, expected, "Normalized {src} mismatches straight read!");
			}
			else {
				panic!("Unable to read {src}.");
			}
		}
	}
}
