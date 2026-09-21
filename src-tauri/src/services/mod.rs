pub mod archive;
pub mod backup;
pub mod cbz;
pub mod cleanup;
pub mod fs_ext;
pub mod metadata;
pub mod page_cache;
pub mod scanner;
pub mod thumb_cache;
pub mod thumbnail;

use std::path::Path;

pub const IMAGE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "gif", "webp", "bmp", "tiff", "avif"];

pub fn is_image_file(name: &str) -> bool {
    Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| {
            let ext = e.to_lowercase();
            IMAGE_EXTENSIONS.contains(&ext.as_str())
        })
        .unwrap_or(false)
}

/// 压缩包类型的档案（相对 folder：页面列表需要缓存 / 解压）。
pub fn is_compressed(archive_type: &str) -> bool {
    matches!(archive_type, "zip" | "rar" | "cbz" | "cbr" | "7z")
}
