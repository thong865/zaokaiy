//! Media validation and processing.
//!
//! Images are sniffed by magic bytes (never trusting the filename or client MIME), auto-rotated
//! from EXIF, downscaled to at most `MAX_EDGE` px and re-encoded — which also strips EXIF/GPS
//! metadata, important for sellers photographing products at home. A small thumbnail is made
//! for grids. Animated GIFs are kept as-is so they keep animating. Videos are validated and
//! stored untouched (transcoding belongs in a background worker with ffmpeg).

use std::io::Cursor;

use bytes::Bytes;
use image::{codecs::jpeg::JpegEncoder, DynamicImage, ImageDecoder, ImageReader};

use crate::error::AppError;

pub const MAX_EDGE: u32 = 2000;
pub const THUMB_EDGE: u32 = 480;

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
        img.resize(edge, edge, image::imageops::FilterType::Lanczos3)
    } else {
        img
    }
}

/// CPU-heavy; call from `spawn_blocking`.
pub fn process(data: Bytes, max_image: usize, max_video: usize) -> Result<Processed, AppError> {
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
        });
    }

    if data.len() > max_image {
        return Err(AppError::bad(format!("image is larger than {} MB", max_image / 1_048_576)));
    }

    let mut decoder = ImageReader::new(Cursor::new(&data))
        .with_guessed_format()
        .map_err(|e| AppError::bad(format!("unreadable image: {e}")))?
        .into_decoder()
        .map_err(|e| AppError::bad(format!("unreadable image: {e}")))?;
    let orientation = decoder.orientation().ok();
    let mut img = DynamicImage::from_decoder(decoder)
        .map_err(|e| AppError::bad(format!("unreadable image: {e}")))?;
    if let Some(o) = orientation {
        img.apply_orientation(o);
    }

    let thumb = encode_jpeg(&fit(img.clone(), THUMB_EDGE), 78)?;

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
        });
    }

    let img = fit(img, MAX_EDGE);
    let (main, mime, ext) = if img.color().has_alpha() {
        (encode_png(&img)?, "image/png", "png")
    } else {
        (encode_jpeg(&img, 86)?, "image/jpeg", "jpg")
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
