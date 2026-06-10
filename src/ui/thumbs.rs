//! Downscaled wallpaper previews for the webview.
//!
//! Wallpapers are typically multi-megapixel; decoding them at full size for a
//! 56px strip thumbnail (times N profiles) makes the Profiles view crawl. The
//! asset handler routes requests with a `?w=` query here: the image is decoded
//! once, downscaled to the requested width, re-encoded as JPEG, and cached
//! keyed by (path, file identity, width) so repeat renders are free.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

type CacheKey = (String, u64, u32);

static CACHE: OnceLock<Mutex<HashMap<CacheKey, Arc<Vec<u8>>>>> = OnceLock::new();

/// Loose upper bound on cached entries; cleared wholesale when exceeded
/// (entries are small JPEGs, this mostly guards against unbounded growth).
const MAX_ENTRIES: usize = 256;

/// A JPEG of `path` downscaled to at most `width` px wide, or None when the
/// file can't be read/decoded (caller falls back to a 404).
pub fn thumbnail_jpeg(path: &str, width: u32) -> Option<Arc<Vec<u8>>> {
    // File identity: mtime seconds + length. Changing the file invalidates.
    let meta = std::fs::metadata(path).ok()?;
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let identity = mtime.wrapping_mul(31).wrapping_add(meta.len());
    let key = (path.to_string(), identity, width);

    let cache = CACHE.get_or_init(Default::default);
    if let Some(hit) = cache.lock().unwrap().get(&key) {
        return Some(hit.clone());
    }

    let img = image::open(path).ok()?;
    let scaled = if img.width() > width {
        img.thumbnail(width, u32::MAX)
    } else {
        img
    };
    let rgb = scaled.to_rgb8();

    let mut buf = Vec::new();
    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, 85);
    encoder.encode_image(&rgb).ok()?;
    let bytes = Arc::new(buf);

    let mut cache = cache.lock().unwrap();
    if cache.len() >= MAX_ENTRIES {
        cache.clear();
    }
    cache.insert(key, bytes.clone());
    Some(bytes)
}
