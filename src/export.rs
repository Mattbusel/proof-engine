//! Writing pictures to disk: still frames in any common format, and
//! animated GIFs.
//!
//! The encoding is done by the [`image`] crate, so a frame can be saved as
//! PNG, JPEG, BMP, TGA or GIF just by choosing the file extension, and a run
//! of frames can be written as one looping GIF with [`GifRecorder`].
//!
//! Everything here works on plain RGBA8 buffers with the first row at the
//! top, so it needs no window and no GPU. Frames read back from OpenGL come
//! bottom row first; [`flip_rows`] turns them the right way up.
//!
//! ```rust
//! use proof_engine::export::{save_rgba, GifRecorder};
//! # let dir = std::env::temp_dir();
//! // A 64x32 horizontal ramp, saved as a PNG...
//! let (w, h) = (64u32, 32u32);
//! let ramp: Vec<u8> = (0..w * h)
//!     .flat_map(|i| { let v = ((i % w) * 4) as u8; [v, v, 255 - v, 255] })
//!     .collect();
//! save_rgba(dir.join("ramp.png"), w, h, &ramp).unwrap();
//!
//! // ...and as a three-frame looping GIF at 20 frames per second.
//! let mut gif = GifRecorder::create(dir.join("ramp.gif"), 1.0 / 20.0).unwrap();
//! for _ in 0..3 {
//!     gif.push(w, h, &ramp).unwrap();
//! }
//! assert_eq!(gif.finish().unwrap(), 3);
//! ```

use std::fs::File;
use std::io::{self, BufWriter};
use std::path::Path;

use image::codecs::gif::{GifEncoder, Repeat};
use image::{imageops, Delay, DynamicImage, Frame, ImageFormat, RgbaImage};

fn to_io(e: image::ImageError) -> io::Error {
    match e {
        image::ImageError::IoError(e) => e,
        other => io::Error::other(other),
    }
}

fn buffer(w: u32, h: u32, rgba: &[u8]) -> io::Result<RgbaImage> {
    let need = w as usize * h as usize * 4;
    if w == 0 || h == 0 || rgba.len() < need {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{w}x{h} RGBA needs {need} bytes, got {}", rgba.len()),
        ));
    }
    Ok(RgbaImage::from_raw(w, h, rgba[..need].to_vec()).expect("length checked above"))
}

/// Reverse the row order of an RGBA8 buffer in place.
///
/// OpenGL's `glReadPixels` returns the bottom row first; image files store
/// the top row first.
pub fn flip_rows(w: u32, h: u32, rgba: &mut [u8]) {
    let stride = w as usize * 4;
    let h = h as usize;
    for y in 0..h / 2 {
        let (top, bottom) = rgba.split_at_mut((h - 1 - y) * stride);
        top[y * stride..(y + 1) * stride].swap_with_slice(&mut bottom[..stride]);
    }
}

/// Scale an RGBA8 buffer to `width` pixels wide, keeping the aspect ratio.
///
/// Uses a Catmull-Rom filter, which keeps thin bright lines (most of what
/// this engine draws) crisp. Returns the new size and pixels; a buffer that
/// is already that width or narrower is returned unchanged.
pub fn scale_to_width(w: u32, h: u32, rgba: &[u8], width: u32) -> io::Result<(u32, u32, Vec<u8>)> {
    let img = buffer(w, h, rgba)?;
    if width == 0 || width >= w {
        return Ok((w, h, img.into_raw()));
    }
    let nh = ((h as u64 * width as u64 + w as u64 / 2) / w as u64).max(1) as u32;
    let out = imageops::resize(&img, width, nh, imageops::FilterType::CatmullRom);
    Ok((width, nh, out.into_raw()))
}

/// The format a path's extension asks for, if the `image` crate can write it.
pub fn format_for(path: &Path) -> Option<ImageFormat> {
    ImageFormat::from_path(path).ok().filter(|f| f.writing_enabled())
}

/// Save an RGBA8 buffer (top row first) to `path`.
///
/// The format comes from the extension: `.png`, `.jpg`/`.jpeg`, `.bmp`,
/// `.tga` or `.gif`. An unknown or missing extension writes a BMP, which is
/// what the engine always wrote before it could do anything else. Formats
/// without an alpha channel (JPEG, BMP) get the colour only.
pub fn save_rgba(path: impl AsRef<Path>, w: u32, h: u32, rgba: &[u8]) -> io::Result<()> {
    let path = path.as_ref();
    let img = DynamicImage::ImageRgba8(buffer(w, h, rgba)?);
    let format = format_for(path).unwrap_or(ImageFormat::Bmp);
    let img = match format {
        ImageFormat::Jpeg | ImageFormat::Bmp => DynamicImage::ImageRgb8(img.to_rgb8()),
        _ => img,
    };
    img.save_with_format(path, format).map_err(to_io)
}

/// Save an RGBA8 buffer as an opaque image: the alpha channel is ignored.
///
/// This is what frame capture uses. The alpha left in a window's back
/// buffer is whatever the last blend wrote, and is not meant to be seen.
pub fn save_opaque(path: impl AsRef<Path>, w: u32, h: u32, rgba: &[u8]) -> io::Result<()> {
    let mut img = buffer(w, h, rgba)?;
    for p in img.pixels_mut() {
        p.0[3] = 255;
    }
    save_rgba(path, w, h, img.as_raw())
}

/// Decode any supported image file into an RGBA8 buffer, top row first.
pub fn load_rgba(bytes: &[u8]) -> io::Result<(u32, u32, Vec<u8>)> {
    let img = image::load_from_memory(bytes).map_err(to_io)?.to_rgba8();
    Ok((img.width(), img.height(), img.into_raw()))
}

/// Writes frames into one looping animated GIF.
///
/// Every frame gets its own 256 colour palette, built with NeuQuant, so
/// smooth gradients survive better than they would with one palette for the
/// whole file. Frames are written as they arrive, so memory use stays at
/// one frame however long the recording is.
///
/// The file is complete once [`finish`](Self::finish) returns. Dropping the
/// recorder also finishes the file, but cannot report a write error.
pub struct GifRecorder {
    encoder: Option<GifEncoder<BufWriter<File>>>,
    delay_ms: u32,
    size: Option<(u32, u32)>,
    frames: usize,
}

impl GifRecorder {
    /// Default quantiser speed: 1 is best and slowest, 30 fastest.
    pub const DEFAULT_SPEED: i32 = 10;

    /// Start a GIF at `path` whose frames are shown for `frame_secs` each.
    ///
    /// GIF timing is in hundredths of a second, so the delay is rounded to
    /// the nearest 10 ms (and never below 20 ms, the fastest rate browsers
    /// honour).
    pub fn create(path: impl AsRef<Path>, frame_secs: f32) -> io::Result<Self> {
        Self::create_with_speed(path, frame_secs, Self::DEFAULT_SPEED)
    }

    /// As [`create`](Self::create), with the quantiser speed (1 to 30).
    pub fn create_with_speed(path: impl AsRef<Path>, frame_secs: f32, speed: i32) -> io::Result<Self> {
        let file = BufWriter::new(File::create(path)?);
        let mut encoder = GifEncoder::new_with_speed(file, speed.clamp(1, 30));
        encoder.set_repeat(Repeat::Infinite).map_err(to_io)?;
        let cs = (frame_secs.max(0.0) * 100.0).round().max(2.0) as u32;
        Ok(Self { encoder: Some(encoder), delay_ms: cs * 10, size: None, frames: 0 })
    }

    /// The delay given to each frame, in milliseconds.
    pub fn delay_ms(&self) -> u32 {
        self.delay_ms
    }

    /// Frames written so far.
    pub fn frames(&self) -> usize {
        self.frames
    }

    /// Append one RGBA8 frame, top row first. Alpha is ignored.
    ///
    /// Every frame must be the size of the first.
    pub fn push(&mut self, w: u32, h: u32, rgba: &[u8]) -> io::Result<()> {
        if *self.size.get_or_insert((w, h)) != (w, h) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("GIF frame is {w}x{h}, earlier frames were {:?}", self.size.unwrap()),
            ));
        }
        let mut img = buffer(w, h, rgba)?;
        for p in img.pixels_mut() {
            p.0[3] = 255;
        }
        let frame = Frame::from_parts(img, 0, 0, Delay::from_numer_denom_ms(self.delay_ms, 1));
        let enc = self.encoder.as_mut().ok_or_else(|| io::Error::other("GIF already finished"))?;
        enc.encode_frame(frame).map_err(to_io)?;
        self.frames += 1;
        Ok(())
    }

    /// Write the GIF trailer and close the file. Returns the frame count.
    pub fn finish(mut self) -> io::Result<usize> {
        if let Some(enc) = self.encoder.take() {
            // The trailer is written when the inner gif encoder drops; the
            // buffered file is flushed when its writer drops after that.
            drop(enc);
        }
        Ok(self.frames)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::AnimationDecoder;

    fn tmp(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join("proof_engine_export_tests");
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    /// Red on the top row, blue on the bottom row.
    fn two_rows(w: u32) -> Vec<u8> {
        let mut v = Vec::new();
        for _ in 0..w { v.extend_from_slice(&[255, 0, 0, 255]); }
        for _ in 0..w { v.extend_from_slice(&[0, 0, 255, 255]); }
        v
    }

    #[test]
    fn flip_rows_reverses_row_order() {
        let mut px = two_rows(3);
        flip_rows(3, 2, &mut px);
        assert_eq!(&px[0..4], &[0, 0, 255, 255]);
        assert_eq!(&px[12..16], &[255, 0, 0, 255]);
        // Odd heights leave the middle row alone.
        let mut odd: Vec<u8> = (0..3u8).flat_map(|r| [r, r, r, r]).collect();
        flip_rows(1, 3, &mut odd);
        assert_eq!(odd, vec![2, 2, 2, 2, 1, 1, 1, 1, 0, 0, 0, 0]);
    }

    #[test]
    fn png_round_trips_exactly() {
        let path = tmp("roundtrip.png");
        let px = two_rows(5);
        save_rgba(&path, 5, 2, &px).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(&bytes[1..4], b"PNG");
        let (w, h, back) = load_rgba(&bytes).unwrap();
        assert_eq!((w, h), (5, 2));
        assert_eq!(back, px);
    }

    #[test]
    fn extension_picks_the_format() {
        let px = two_rows(4);
        for (name, magic) in [("f.jpg", &b"\xFF\xD8"[..]), ("f.bmp", b"BM"), ("f.gif", b"GIF8"), ("f.unknownext", b"BM")] {
            let path = tmp(name);
            save_opaque(&path, 4, 2, &px).unwrap();
            let bytes = std::fs::read(&path).unwrap();
            assert!(bytes.starts_with(magic), "{name} starts with {:?}", &bytes[..4]);
            let (w, h, back) = load_rgba(&bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!((w, h), (4, 2));
            // Lossy formats get close; the top row stays red.
            assert!(back[0] > 200 && back[2] < 60, "{name}: {:?}", &back[..4]);
        }
    }

    #[test]
    fn opaque_save_drops_alpha() {
        let path = tmp("opaque.png");
        save_opaque(&path, 1, 1, &[10, 20, 30, 0]).unwrap();
        let (_, _, back) = load_rgba(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(back, vec![10, 20, 30, 255]);
    }

    #[test]
    fn short_buffers_are_an_error_not_a_panic() {
        assert!(save_rgba(tmp("short.png"), 4, 4, &[0; 10]).is_err());
        assert!(save_rgba(tmp("zero.png"), 0, 4, &[]).is_err());
    }

    #[test]
    fn scaling_keeps_the_aspect_ratio() {
        let px = vec![128u8; 40 * 20 * 4];
        let (w, h, out) = scale_to_width(40, 20, &px, 10).unwrap();
        assert_eq!((w, h), (10, 5));
        assert_eq!(out.len(), 10 * 5 * 4);
        assert!(out.iter().all(|&v| (126..=130).contains(&v)));
        // Never upscales.
        assert_eq!(scale_to_width(40, 20, &px, 80).unwrap().0, 40);
    }

    #[test]
    fn gif_recorder_writes_a_looping_animation() {
        let path = tmp("anim.gif");
        let (w, h) = (16u32, 8u32);
        let mut rec = GifRecorder::create(&path, 1.0 / 25.0).unwrap();
        assert_eq!(rec.delay_ms(), 40);
        for shade in [0u8, 120, 240] {
            let frame: Vec<u8> = (0..w * h).flat_map(|_| [shade, 255 - shade, 64, 255]).collect();
            rec.push(w, h, &frame).unwrap();
        }
        assert!(rec.push(8, 8, &[0; 256]).is_err(), "mismatched frame size is refused");
        assert_eq!(rec.finish().unwrap(), 3);

        let bytes = std::fs::read(&path).unwrap();
        assert!(bytes.starts_with(b"GIF89a"));
        assert_eq!(*bytes.last().unwrap(), 0x3B, "GIF trailer present");
        let dec = image::codecs::gif::GifDecoder::new(std::io::Cursor::new(&bytes)).unwrap();
        let frames = dec.into_frames().collect_frames().unwrap();
        assert_eq!(frames.len(), 3);
        for (i, f) in frames.iter().enumerate() {
            let (num, den) = f.delay().numer_denom_ms();
            assert_eq!(num / den, 40);
            assert_eq!(f.buffer().dimensions(), (w, h));
            let red = f.buffer().get_pixel(3, 3).0[0] as i32;
            assert!((red - [0, 120, 240][i]).abs() < 12, "frame {i} red {red}");
        }
    }
}
