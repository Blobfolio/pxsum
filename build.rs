/*!
# pxsum: Build Script
*/

use dactyl::NiceU32;
use image::ImageFormat;
use std::{
	collections::BTreeSet,
	io::Write,
};



/// # Pre-Compute Extensions.
pub fn main() {
	// Collect the supported formats.
	let formats: Vec<ImageFormat> = ImageFormat::all()
		.filter(|f| f.can_read() && f.reading_enabled())
		.collect();

	// Build up a list of matching file extensions by length.
	let mut ext3: BTreeSet<u32> = BTreeSet::new();
	let mut ext4: BTreeSet<u32> = BTreeSet::new();
	for f in formats {
		for ext in f.extensions_str() {
			assert!(ext.is_ascii(), "Bug: extension is non-ascii: {ext}");

			match ext.as_bytes() {
				[a, b, c] => {
					ext3.insert(u32::from_le_bytes([
						b'.',
						a.to_ascii_lowercase(),
						b.to_ascii_lowercase(),
						c.to_ascii_lowercase(),
					]));
				},
				[a, b, c, d] => {
					ext4.insert(u32::from_le_bytes([
						a.to_ascii_lowercase(),
						b.to_ascii_lowercase(),
						c.to_ascii_lowercase(),
						d.to_ascii_lowercase(),
					]));
				},
				_ => panic!("Bug: file extension has unexpected length: {ext}"),
			}
		}
	}

	// Formats not in the image crate.
	ext3.insert(u32::from_le_bytes(*b".j2c"));
	ext3.insert(u32::from_le_bytes(*b".j2k"));
	ext3.insert(u32::from_le_bytes(*b".jp2"));
	ext3.insert(u32::from_le_bytes(*b".jpc"));
	ext3.insert(u32::from_le_bytes(*b".jxl"));
	ext4.insert(u32::from_le_bytes(*b"avif"));
	ext4.insert(u32::from_le_bytes(*b"jpg2"));

	// Build up a matching method we can use at runtime.
	let out = format!(
		r"
/// # Match Image Extension.
const fn check_extension(bytes: &[u8]) -> bool {{
	if let [.., 0..=46 | 48..=91 | 93..=255, b'.', a, b, c] = bytes {{
		matches!(
			u32::from_le_bytes([b'.', a.to_ascii_lowercase(), b.to_ascii_lowercase(), c.to_ascii_lowercase()]),
			{}
		)
	}}
	else if let [.., 0..=46 | 48..=91 | 93..=255, b'.', a, b, c, d] = bytes {{
		matches!(
			u32::from_le_bytes([a.to_ascii_lowercase(), b.to_ascii_lowercase(), c.to_ascii_lowercase(), d.to_ascii_lowercase()]),
			{}
		)
	}}
	else {{ false }}
}}",
		ext3.into_iter()
			.map(|n| NiceU32::with_separator(n, b'_'))
			.collect::<Vec<_>>()
			.join(" | "),
		ext4.into_iter()
			.map(|n| NiceU32::with_separator(n, b'_'))
			.collect::<Vec<_>>()
			.join(" | "),
	);

	let out_path = std::fs::canonicalize(std::env::var("OUT_DIR").expect("Missing OUT_DIR."))
		.expect("Missing OUT_DIR.")
		.join("pxsum-ext.rs");

	std::fs::File::create(out_path)
		.and_then(|mut f| f.write_all(out.as_bytes()).and_then(|_| f.flush()))
		.expect("Unable to write file.");
}
