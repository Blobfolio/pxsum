/*!
# pxsum: Image.
*/

use crate::{
	Checksum,
	PxsumError,
};
use image::{
	DynamicImage,
	ImageFormat,
	ImageReader,
};
use std::{
	fs::File,
	io::Cursor,
	num::{
		NonZeroU32,
		NonZeroUsize,
		Wrapping,
	},
	path::Path,
};



/// # RGBA Pixel Size.
///
/// Each channel is one byte, four in total.
const RGBA_SIZE: NonZeroUsize = NonZeroUsize::new(4).unwrap();



#[derive(Debug, Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
/// # Image Kind.
///
/// This enum collects all of the supported image formats from all of the
/// third-party crates we're using for decoding.
pub(super) enum PxKind {
	/// # AVIF.
	Avif,

	/// # Bitmap.
	Bmp,

	/// # GIF.
	Gif,

	/// # Icon.
	Ico,

	/// # JPEG.
	Jpeg,

	/// # JPEG 2000.
	Jpeg2k,

	/// # JPEG XL.
	JpegXl,

	/// # PNG.
	Png,

	/// # Tiff(any?).
	Tiff,

	/// # WebP.
	WebP,
}

impl TryFrom<ImageFormat> for PxKind {
	type Error = PxsumError;

	fn try_from(src: ImageFormat) -> Result<Self, Self::Error> {
		match src {
			ImageFormat::Avif => Ok(Self::Avif),
			ImageFormat::Bmp => Ok(Self::Bmp),
			ImageFormat::Gif => Ok(Self::Gif),
			ImageFormat::Ico => Ok(Self::Ico),
			ImageFormat::Jpeg => Ok(Self::Jpeg),
			ImageFormat::Png => Ok(Self::Png),
			ImageFormat::Tiff => Ok(Self::Tiff),
			ImageFormat::WebP => Ok(Self::WebP),
			_ => Err(PxsumError::Decode),
		}
	}
}

impl PxKind {
	/// # Decode.
	fn decode(self, src: &[u8]) -> Result<DynamicImage, PxsumError> {
		use image::{AnimationDecoder, codecs};

		#[cold]
		/// # Decode AVIF.
		///
		/// Not a popular format, hence cold.
		fn decode_avif(src: &[u8]) -> Result<DynamicImage, PxsumError> {
			libavif::decode_rgb(src).ok()
				.and_then(|img| image::ImageBuffer::from_vec(
					img.width(),
					img.height(),
					img.to_vec(),
				))
				.map(DynamicImage::ImageRgba8)
				.ok_or(PxsumError::Decode)
		}

		#[cold]
		/// # Decode JPEG 2000.
		///
		/// Not a popular format, hence cold.
		fn decode_jpeg2k(src: &[u8]) -> Result<DynamicImage, PxsumError> {
			jpeg2k::Image::from_bytes(src).ok()
				.and_then(|img| DynamicImage::try_from(&img).ok())
				.ok_or(PxsumError::Decode)
		}

		#[cold]
		/// # Decode JPEG XL.
		///
		/// Not a popular format, hence cold.
		fn decode_jpegxl(src: &[u8]) -> Result<DynamicImage, PxsumError> {
			use jpegxl_rs::image::ToDynamic;
			jpegxl_rs::decoder_builder()
				.build()
				.and_then(|dec| dec.decode_to_image(src))
				.ok()
				.flatten()
				.ok_or(PxsumError::Decode)
		}

		// Most decoding is handled by the image crate.
		let fmt = match self {
			Self::Bmp => ImageFormat::Bmp,
			Self::Ico => ImageFormat::Ico,
			Self::Jpeg => ImageFormat::Jpeg,
			Self::Tiff => ImageFormat::Tiff,

			// Check for animation.
			Self::Gif =>
				// This decoder lacks a built-in method for animation-checking
				// so we have to decode and count the frames instead. Yuck!
				if codecs::gif::GifDecoder::new(Cursor::new(src))?.into_frames().take(2).count() == 1 {
					ImageFormat::Gif
				}
				else { return Err(PxsumError::Animation); },
			Self::Png =>
				if codecs::png::PngDecoder::new(Cursor::new(src)).and_then(|p| p.is_apng())? {
					return Err(PxsumError::Animation);
				}
				else { ImageFormat::Png },
			Self::WebP =>
				if codecs::webp::WebPDecoder::new(Cursor::new(src))?.has_animation() {
					return Err(PxsumError::Animation);
				}
				else { ImageFormat::WebP },

			// The image crate doesn't _really_ support AVIF yet, so we need to
			// step in for these.
			Self::Avif => return decode_avif(src),

			// JPEG 2000 does its own thing.
			Self::Jpeg2k => return decode_jpeg2k(src),

			// And so does JPEG XL.
			Self::JpegXl => return decode_jpegxl(src),
		};

		// Let the image crate sort it out.
		let mut dec = ImageReader::with_format(Cursor::new(src), fmt);
		dec.no_limits();
		dec.decode().map_err(Into::into)
	}

	/// # Try From File.
	///
	/// Read the first few bytes of the file and see what shakes out.
	pub(super) fn try_from_file<P: AsRef<Path>>(src: P) -> Option<Self> {
		use std::io::Read;
		let mut file = File::open(src).ok()?;
		let mut buf = [0_u8; 16];
		file.read_exact(&mut buf).ok()?;
		Self::try_from_magic(buf.as_slice()).ok()
	}

	/// # Guess Format.
	///
	/// Look for a known file signature in the first dozen bytes, similar to
	/// `image::guess_format`, but covering all (and only) the specific image
	/// formats we support.
	pub(super) const fn try_from_magic(src: &[u8]) -> Result<Self, PxsumError> {
		match src.first_chunk::<12>() {
			Some([0xff, 0xd8, 0xff, ..]) => Ok(Self::Jpeg),
			Some([0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, ..]) => Ok(Self::Png),
			Some([b'G', b'I', b'F', b'8', b'7' | b'9', b'a', ..]) => Ok(Self::Gif),
			Some([b'R', b'I', b'F', b'F', _, _, _, _, b'W', b'E', b'B', b'P']) => Ok(Self::WebP),
			Some([0x00, 0x00, 0x00, 0x20 | 0x1c, b'f', b't', b'y', b'p', b'a', b'v', b'i', b'f']) => Ok(Self::Avif),
			Some([0xff, 0x0a, ..] | [0x00, 0x00, 0x00, 0x0c, b'J', b'X', b'L', 0x20, 0x0d, 0x0a, 0x87, 0x0a]) => Ok(Self::JpegXl),
			Some([b'B', b'M', ..]) => Ok(Self::Bmp),
			Some([0x00, 0x00, 0x01, 0x00, ..]) => Ok(Self::Ico),
			Some([0x00, 0x00, 0x00, 0x0c, b'j', b'P', 0x20, 0x20, 0x0d, 0x0a, 0x87, 0x0a] | [0xff, b'O', 0xff, b'Q', ..]) => Ok(Self::Jpeg2k),
			Some([b'M', b'M', 0x00, b'*', ..] | [b'I', b'I', b'*', 0x00, ..]) => Ok(Self::Tiff),
			_ => Err(PxsumError::Decode),
		}
	}
}



/// # Image Wrapper.
///
/// This holds the pixel buffer for an image in RGBA format, along with
/// knowledge of the original storage to potentially avoid alpha traversal.
pub(super) struct PxImage {
	/// # Pixel Buffer.
	buf: Vec<u8>,

	/// # No Alpha Data (original type).
	no_alpha: bool,

	/// # Image Width.
	width: NonZeroU32,

	/// # Image Height.
	height: NonZeroU32,
}

impl PxImage {
	/// # Decode Image.
	///
	/// Decode the image from memory.
	///
	/// ## Errors
	///
	/// This will return an error if the image cannot be decoded or has an
	/// invalid pixel count.
	pub(super) fn new(src: &[u8], format: PxKind) -> Result<Self, PxsumError> {
		// Decode the image as-is.
		let img = format.decode(src)?;

		// Get width and height.
		let width = NonZeroU32::new(img.width()).ok_or(PxsumError::Dimensions)?;
		let height = NonZeroU32::new(img.height()).ok_or(PxsumError::Dimensions)?;

		// Figure out how many bytes the RGBA pixel data _should_ take up
		// given the dimensions.
		let expected_len: NonZeroUsize = {
			let w = NonZeroUsize::try_from(width).map_err(|_| PxsumError::Dimensions)?;
			let h = NonZeroUsize::try_from(height).map_err(|_| PxsumError::Dimensions)?;
			w.checked_mul(h)
				.and_then(|r| r.checked_mul(RGBA_SIZE))
				.ok_or(PxsumError::Dimensions)?
		};

		// If we know there's no alpha channel in the original, make a note of
		// it as it can save us some time later on.
		let no_alpha = matches!(img,
			DynamicImage::ImageLuma8(_) |
			DynamicImage::ImageRgb8(_) |
			DynamicImage::ImageLuma16(_) |
			DynamicImage::ImageRgb16(_) |
			DynamicImage::ImageRgb32F(_)
		);

		// Convert to RGBA and tease out just the pixel data.
		let buf: Vec<u8> = img.into_rgba8().into_vec();
		let len = buf.len();

		// Check the counts, but we're probably good if we made it this far.
		if len == expected_len.get() {
			Ok(Self { buf, no_alpha, width, height })
		}
		else if len == 0 { Err(PxsumError::NoData) }
		else { Err(PxsumError::Decode) }
	}

	/// # Hash Pixels.
	///
	/// Calculate and return a checksum of the pixel data.
	pub(super) fn into_checksum(self, strict: bool) -> [u8; 32] {
		// Destructure.
		let Self { mut buf, no_alpha, width, height } = self;

		// For loose comparisons, replace invisible pixels with their index so
		// color drift won't affect the checksum.
		if ! strict && ! no_alpha {
			let mut i = Wrapping(0_u32);
			for chunk in buf.chunks_exact_mut(RGBA_SIZE.get()) {
				if chunk[3] == 0 {
					chunk.copy_from_slice(i.0.to_le_bytes().as_slice());
				}
				i += 1;
			}
		}

		// Hash the pixel data!
		let mut hasher = blake3::Hasher::new();
		hasher.update(width.get().to_le_bytes().as_slice());
		hasher.update(height.get().to_le_bytes().as_slice());
		hasher.update(buf.as_slice());
		let mut chk = <[u8; 32]>::from(hasher.finalize());

		// Steal one bit from the first byte to serve as a strictness indicator.
		if strict { chk[0] |= Checksum::STRICT; }
		else { chk[0] &= ! Checksum::STRICT; }

		chk
	}
}



#[cfg(test)]
mod test {
	use super::*;

	/// # Test Assets / Expected Kinds.
	const KINDS: &[(&str, Option<PxKind>)] = &[
		("skel/assets/ace.jp2", Some(PxKind::Jpeg2k)),
		("skel/assets/ace.jxl", Some(PxKind::JpegXl)),
		("skel/assets/ace.webp", Some(PxKind::WebP)),
		("skel/assets/ash.jpg", Some(PxKind::Jpeg)),
		("skel/assets/atom.avif", Some(PxKind::Avif)),
		("skel/assets/atom.png", Some(PxKind::Png)),
		("skel/assets/carl.jpg", Some(PxKind::Jpeg)),
		("skel/assets/cmyk.JPG", Some(PxKind::Jpeg)),
		("skel/assets/dingo.png", Some(PxKind::Png)),
		("skel/assets/down_arrow.gif", Some(PxKind::Gif)),
		("skel/assets/empty.jpg", None),                 // Empty file.
		("skel/assets/firefox.png", Some(PxKind::Png)),
		("skel/assets/firefox.webp", Some(PxKind::WebP)),
		("skel/assets/firefox-compressed.png", Some(PxKind::Png)),
		("skel/assets/frolics.TIF", Some(PxKind::Tiff)),
		("skel/assets/herring.png", Some(PxKind::WebP)), // Misnamed file.
		("skel/assets/lenna.jpeg", Some(PxKind::Jpeg)),
		("skel/assets/poe.png", Some(PxKind::Png)),
		("skel/assets/santo.bmp", Some(PxKind::Bmp)),
		("skel/assets/santo.ico", Some(PxKind::Ico)),
		("skel/assets/statler.png", Some(PxKind::Png)),
		("skel/assets/statler.webp", Some(PxKind::WebP)),
		("skel/assets/waldorf.png", Some(PxKind::Png)),
	];

	/// # Our Types to Image Crate Types.
	const IMAGE_KINDS: [(PxKind, ImageFormat); 7] = [
		(PxKind::Bmp, ImageFormat::Bmp),
		(PxKind::Gif, ImageFormat::Gif),
		(PxKind::Ico, ImageFormat::Ico),
		(PxKind::Jpeg, ImageFormat::Jpeg),
		(PxKind::Png, ImageFormat::Png),
		(PxKind::Tiff, ImageFormat::Tiff),
		(PxKind::WebP, ImageFormat::WebP),
	];

	#[test]
	fn t_image_format() {
		for (kind, fmt) in IMAGE_KINDS {
			// The type should be decodeable!
			assert!(
				fmt.can_read() && fmt.reading_enabled(),
				"Cannot decode {fmt:?}.",
			);

			// And back-and-forth should wind up the same.
			let Ok(kind2) = PxKind::try_from(fmt) else {
				panic!("Image {fmt:?} could not be converted to kind.");
			};
			assert_eq!(kind, kind2, "{kind:?} <-> {fmt:?} not symmetrical.");
		}
	}

	#[test]
	fn t_guess() {
		use std::io::Read;

		let mut buf = [0_u8; 16];
		for (path, kind) in KINDS.iter().copied() {
			let Ok(mut file) = File::open(path) else {
				panic!("Unable to open {path}.");
			};
			buf.fill(0);
			assert!(
				file.read_exact(&mut buf).is_ok() || path == "skel/assets/empty.jpg",
				"Unable to read {path}."
			);
			assert_eq!(
				PxKind::try_from_magic(buf.as_slice()).ok(),
				kind,
				"Wrong type guessed for {path}!",
			);
		}
	}

	#[test]
	fn t_animation() {
		// Make sure these all fail.
		for (path, fmt) in [
			("skel/assets/flyfuk.gif", PxKind::Gif),
			("skel/assets/flyfuk.png", PxKind::Png),
			("skel/assets/flyfuk.webp", PxKind::WebP),
		] {
			let Ok(raw) = std::fs::read(path) else {
				panic!("Unable to open {path}.");
			};
			match PxImage::new(&raw, fmt) {
				Ok(_) => panic!("Animation should fail: {path}"),
				Err(e) => assert_eq!(e, PxsumError::Animation),
			}
		}
	}
}
