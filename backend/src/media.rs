//! Media validation and processing.
//!
//! Images are sniffed by magic bytes (never trusting the filename or client MIME), checked against
//! a minimum resolution, auto-rotated from EXIF, downscaled to at most `MAX_EDGE` px and re-encoded
//! — which also strips EXIF/GPS metadata, important for sellers photographing products at home.
//!
//! For fast pages every image also gets:
//! * responsive **WebP** renditions (`VARIANT_WIDTHS`, never upscaled) for `srcset`, so a phone
//!   downloads a ~30 KB 640px file instead of a 1–2 MB original;
//! * a tiny blurred **placeholder** (inline data URI, ~0.5 KB) and a **dominant colour**, shown
//!   while the real image lazy-loads;
//! * a JPEG (or PNG with transparency) **fallback** at full quality for old browsers, zoom and sharing.
//!
//! Animated GIFs are kept as-is so they keep animating. Videos are validated and stored untouched
//! (transcoding belongs in a background worker with ffmpeg).

use std::io::Cursor;

use base64::{engine::general_purpose::STANDARD as B64, Engine};
use bytes::Bytes;
use image::{codecs::jpeg::JpegEncoder, imageops::FilterType, DynamicImage, GenericImageView, ImageDecoder, ImageReader};

use crate::error::AppError;

pub const MAX_EDGE: u32 = 2000;
pub const THUMB_EDGE: u32 = 480;
/// Widths of the WebP renditions used in `srcset` (only those smaller than the image are made,
/// plus one at the image's own width when it is below the largest).
pub const VARIANT_WIDTHS: [u32; 5] = [320, 640, 960, 1280, 1920];
/// Lossy WebP quality: visually lossless for product photos at ~25-35% of the JPEG size.
pub const WEBP_QUALITY: f32 = 82.0;
pub const JPEG_QUALITY: u8 = 85;
/// Transparent images keep a PNG fallback, capped smaller because PNG is heavy.
pub const ALPHA_FALLBACK_EDGE: u32 = 1280;

/// Limits concurrent image processing to the number of CPU cores.
pub async fn cpu_permit() -> tokio::sync::SemaphorePermit<'static> {
    static SEM: std::sync::OnceLock<tokio::sync::Semaphore> = std::sync::OnceLock::new();
    let n = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(2);
    SEM.get_or_init(|| tokio::sync::Semaphore::new(n)).acquire().await.expect("semaphore never closed")
}

pub struct Variant {
    pub width: u32,
    pub height: u32,
    pub data: Bytes,
}

pub struct Processed {
    pub kind: &'static str,
    pub mime: String,
    pub ext: &'static str,
    pub main: Bytes,
    pub thumb: Option<Bytes>,
    pub thumb_mime: &'static str,
    pub thumb_ext: &'static str,
    pub width: Option<i32>,
    pub height: Option<i32>,
    /// WebP renditions, largest first.
    pub variants: Vec<Variant>,
    /// `data:image/jpeg;base64,…` ~24px preview (blurred by the browser while loading).
    pub placeholder: String,
    /// `#rrggbb` average colour.
    pub color: String,
}

pub fn sniff(data: &[u8]) -> Option<(&'static str, &'static str, &'static str)> {
    let t = infer::get(data)?;
    let kind = match t.mime_type() {
        "image/jpeg" | "image/png" | "image/webp" | "image/gif" => "image",
        "video/mp4" | "video/webm" | "video/quicktime" => "video",
        _ => return None,
    };
    let ext = match t.mime_type() {
        "image/jpeg" => "jpg",
        "image/png" => "png",
        "image/webp" => "webp",
        "image/gif" => "gif",
        "video/mp4" => "mp4",
        "video/webm" => "webm",
        _ => "mov",
    };
    Some((kind, t.mime_type(), ext))
}

fn encode_jpeg(img: &DynamicImage, quality: u8) -> Result<Bytes, AppError> {
    let mut out = Vec::new();
    let rgb = img.to_rgb8();
    JpegEncoder::new_with_quality(&mut out, quality)
        .encode_image(&rgb)
        .map_err(|e| AppError::Internal(format!("jpeg encode: {e}")))?;
    Ok(Bytes::from(out))
}

fn encode_png(img: &DynamicImage) -> Result<Bytes, AppError> {
    let mut out = Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png)
        .map_err(|e| AppError::Internal(format!("png encode: {e}")))?;
    Ok(Bytes::from(out.into_inner()))
}

fn fit(img: DynamicImage, edge: u32) -> DynamicImage {
    if img.width() > edge || img.height() > edge {
        img.resize(edge, edge, FilterType::Lanczos3)
    } else {
        img
    }
}

fn encode_webp(img: &DynamicImage) -> Result<Bytes, AppError> {
    let (w, h) = img.dimensions();
    let mem = if img.color().has_alpha() {
        let rgba = img.to_rgba8();
        webp::Encoder::from_rgba(rgba.as_raw(), w, h).encode(WEBP_QUALITY)
    } else {
        let rgb = img.to_rgb8();
        webp::Encoder::from_rgb(rgb.as_raw(), w, h).encode(WEBP_QUALITY)
    };
    Ok(Bytes::copy_from_slice(&mem))
}

/// Target widths for an image `w` px wide: the standard widths below it, plus `w` itself when
/// it is smaller than the largest standard width (so the sharpest rendition is never upscaled).
pub fn variant_widths(w: u32) -> Vec<u32> {
    let mut out: Vec<u32> = VARIANT_WIDTHS.iter().copied().filter(|&v| v < w).collect();
    if w <= *VARIANT_WIDTHS.last().unwrap() && !out.contains(&w) {
        out.push(w);
    }
    if out.is_empty() {
        out.push(*VARIANT_WIDTHS.last().unwrap());
    }
    out.sort_unstable_by(|a, b| b.cmp(a));
    out
}

/// WebP renditions, largest first. Each step downsizes the previous one (fast, still sharp).
pub fn make_variants(img: &DynamicImage) -> Result<Vec<Variant>, AppError> {
    let mut out = Vec::new();
    let mut src = img.clone();
    for w in variant_widths(img.width()) {
        let h = ((img.height() as f64) * (w as f64) / (img.width() as f64)).round().max(1.0) as u32;
        if w < src.width() {
            // Catmull-Rom: nearly Lanczos-sharp for downscaling, ~2x faster.
            src = src.resize_exact(w, h, FilterType::CatmullRom);
        }
        out.push(Variant { width: src.width(), height: src.height(), data: encode_webp(&src)? });
    }
    Ok(out)
}

/// Tiny preview + average colour for lazy-loading placeholders.
pub fn placeholder_and_color(img: &DynamicImage) -> (String, String) {
    let tiny = img.thumbnail(24, 24);
    let mut buf = Vec::new();
    let ok = JpegEncoder::new_with_quality(&mut buf, 50).encode_image(&tiny.to_rgb8()).is_ok();
    let placeholder = if ok { format!("data:image/jpeg;base64,{}", B64.encode(&buf)) } else { String::new() };
    let px = img.resize_exact(1, 1, FilterType::Triangle).to_rgb8();
    let [r, g, b] = px.get_pixel(0, 0).0;
    (placeholder, format!("#{r:02x}{g:02x}{b:02x}"))
}

fn decode(data: &[u8]) -> Result<DynamicImage, AppError> {
    let mut decoder = ImageReader::new(Cursor::new(data))
        .with_guessed_format()
        .map_err(|e| AppError::bad(format!("unreadable image: {e}")))?
        .into_decoder()
        .map_err(|e| AppError::bad(format!("unreadable image: {e}")))?;
    let orientation = decoder.orientation().ok();
    let mut img = DynamicImage::from_decoder(decoder).map_err(|e| AppError::bad(format!("unreadable image: {e}")))?;
    if let Some(o) = orientation {
        img.apply_orientation(o);
    }
    Ok(img)
}

/// Responsive renditions for an already-stored image (backfill of older uploads). CPU-heavy.
pub fn derive(data: &[u8]) -> Result<(Vec<Variant>, String, String, u32, u32), AppError> {
    let img = decode(data)?;
    let (ph, color) = placeholder_and_color(&img);
    Ok((make_variants(&img)?, ph, color, img.width(), img.height()))
}

/// CPU-heavy; call from `spawn_blocking`. `min_edge`: minimum px on the shortest side (0 = off).
pub fn process(data: Bytes, max_image: usize, max_video: usize, min_edge: u32) -> Result<Processed, AppError> {
    let (kind, mime, ext) = sniff(&data).ok_or_else(|| {
        AppError::bad("unsupported file type — use JPEG, PNG, WebP, GIF, MP4, WebM or MOV")
    })?;

    if kind == "video" {
        if data.len() > max_video {
            return Err(AppError::bad(format!("video is larger than {} MB", max_video / 1_048_576)));
        }
        return Ok(Processed {
            kind,
            mime: mime.into(),
            ext,
            main: data,
            thumb: None,
            thumb_mime: "",
            thumb_ext: "",
            width: None,
            height: None,
            variants: Vec::new(),
            placeholder: String::new(),
            color: String::new(),
        });
    }

    if data.len() > max_image {
        return Err(AppError::bad(format!("image is larger than {} MB", max_image / 1_048_576)));
    }

    let img = decode(&data)?;
    if min_edge > 0 && img.width().min(img.height()) < min_edge {
        return Err(AppError::bad(format!(
            "image is too small ({}×{} px) — use at least {min_edge} px on the shortest side",
            img.width(),
            img.height()
        )));
    }
    let (placeholder, color) = placeholder_and_color(&img);
    let thumb = encode_jpeg(&fit(img.clone(), THUMB_EDGE), 80)?;

    // Keep animated GIFs untouched (they are usually small and must keep animating).
    if mime == "image/gif" {
        return Ok(Processed {
            kind,
            mime: mime.into(),
            ext,
            width: Some(img.width() as i32),
            height: Some(img.height() as i32),
            main: data,
            thumb: Some(thumb),
            thumb_mime: "image/jpeg",
            thumb_ext: "jpg",
            variants: Vec::new(),
            placeholder,
            color,
        });
    }

    let img = fit(img, MAX_EDGE);
    let variants = make_variants(&img)?;
    let (main, mime, ext) = if img.color().has_alpha() {
        (encode_png(&fit(img.clone(), ALPHA_FALLBACK_EDGE))?, "image/png", "png")
    } else {
        (encode_jpeg(&img, JPEG_QUALITY)?, "image/jpeg", "jpg")
    };
    Ok(Processed {
        kind,
        mime: mime.into(),
        ext,
        width: Some(img.width() as i32),
        height: Some(img.height() as i32),
        main,
        thumb: Some(thumb),
        thumb_mime: "image/jpeg",
        thumb_ext: "jpg",
        variants,
        placeholder,
        color,
    })
}

/// Best-effort poster frame for videos using the `ffmpeg` binary (if installed).
/// Returns a JPEG thumbnail, or None when ffmpeg is unavailable or fails.
pub async fn video_poster(data: &Bytes, ext: &str) -> Option<Bytes> {
    let dir = std::env::temp_dir();
    let id = uuid::Uuid::new_v4();
    let input = dir.join(format!("zk-{id}.{ext}"));
    let output = dir.join(format!("zk-{id}.jpg"));
    tokio::fs::write(&input, data).await.ok()?;
    let status = tokio::process::Command::new("ffmpeg")
        .args(["-loglevel", "error", "-y", "-ss", "0.5", "-i"])
        .arg(&input)
        .args(["-frames:v", "1", "-vf", &format!("scale='min({THUMB_EDGE},iw)':-2")])
        .arg(&output)
        .status()
        .await;
    let result = match status {
        Ok(s) if s.success() => tokio::fs::read(&output).await.ok().map(Bytes::from),
        _ => None,
    };
    let _ = tokio::fs::remove_file(&input).await;
    let _ = tokio::fs::remove_file(&output).await;
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgb, RgbImage, Rgba, RgbaImage};

    fn jpeg(w: u32, h: u32) -> Bytes {
        let img = RgbImage::from_fn(w, h, |x, y| Rgb([(x % 255) as u8, (y % 255) as u8, 120]));
        encode_jpeg(&DynamicImage::ImageRgb8(img), 90).unwrap()
    }

    #[test]
    fn widths() {
        assert_eq!(variant_widths(4000), vec![1920, 1280, 960, 640, 320]);
        assert_eq!(variant_widths(1500), vec![1500, 1280, 960, 640, 320]);
        assert_eq!(variant_widths(640), vec![640, 320]);
        assert_eq!(variant_widths(200), vec![200]);
    }

    #[test]
    fn process_makes_webp_renditions_placeholder_and_rejects_small() {
        let p = process(jpeg(2400, 1600), 20 << 20, 0, 500).unwrap();
        assert_eq!((p.width, p.height), (Some(2000), Some(1333)));
        let ws: Vec<u32> = p.variants.iter().map(|v| v.width).collect();
        assert_eq!(ws, vec![1920, 1280, 960, 640, 320]);
        assert_eq!(p.variants[3].height, 427);
        for v in &p.variants {
            assert_eq!(&v.data[0..4], b"RIFF");
            assert_eq!(&v.data[8..12], b"WEBP");
        }
        assert!(p.variants[4].data.len() < p.variants[0].data.len());
        assert!(p.placeholder.starts_with("data:image/jpeg;base64,") && p.placeholder.len() < 2000);
        assert!(p.color.starts_with('#') && p.color.len() == 7);
        assert_eq!(p.mime, "image/jpeg");

        let err = process(jpeg(480, 900), 20 << 20, 0, 500).err().unwrap();
        assert!(err.to_string().contains("too small (480×900 px)"), "{err}");
        assert!(process(jpeg(480, 900), 20 << 20, 0, 0).is_ok()); // check disabled
    }

    #[test]
    fn transparency_is_kept() {
        let img = RgbaImage::from_fn(900, 900, |x, _| Rgba([200, 10, 10, (x % 255) as u8]));
        let mut png = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(img).write_to(&mut png, image::ImageFormat::Png).unwrap();
        let p = process(Bytes::from(png.into_inner()), 20 << 20, 0, 500).unwrap();
        assert_eq!(p.mime, "image/png");
        let v = image::load_from_memory(&p.variants[0].data).unwrap();
        assert!(v.color().has_alpha());
    }
}
