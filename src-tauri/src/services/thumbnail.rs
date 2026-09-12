use anyhow::Result;
use image::{io::Reader as ImageReader, ImageOutputFormat};
use std::path::Path;

pub struct ThumbnailGenerator {
    width: u32,
    height: Option<u32>,
    quality: u8,
}

impl ThumbnailGenerator {
    pub fn new(width: u32, quality: u8) -> Self {
        Self {
            width,
            height: None,
            quality,
        }
    }

    pub fn generate(&self, input: &[u8]) -> Result<Vec<u8>> {
        let img = ImageReader::new(std::io::Cursor::new(input))
            .with_guessed_format()?
            .decode()?;

        // 保持宽高比缩放到目标包围盒内（不变形）；裁切交给前端 CSS object-fit。
        let target_w = self.width;
        let target_h = self.height.unwrap_or((self.width as f64 * 1.5) as u32);
        let (src_w, src_h) = (img.width(), img.height());
        let scale = (target_w as f64 / src_w as f64)
            .min(target_h as f64 / src_h as f64)
            .min(1.0);
        let new_w = ((src_w as f64) * scale).max(1.0) as u32;
        let new_h = ((src_h as f64) * scale).max(1.0) as u32;
        // thumbnail() 是专为“大图快速小图”设计的单趟降采样，比全分辨率 Lanczos3
        // 重采样快一个量级，视觉差异在 300px 缩略图尺度上不可感知。
        let thumbnail = img.thumbnail(new_w, new_h);

        let mut output = Vec::new();
        thumbnail.write_to(
            &mut std::io::Cursor::new(&mut output),
            ImageOutputFormat::Jpeg(self.quality),
        )?;

        Ok(output)
    }

    pub fn generate_with_cache(
        &self,
        input: &[u8],
        cache_dir: &Path,
        cache_key: &str,
    ) -> Result<Vec<u8>> {
        let cache_path = cache_dir.join(format!("{}.jpg", cache_key));

        // Check if cached version exists
        if cache_path.exists() {
            return Ok(std::fs::read(&cache_path)?);
        }

        // Generate thumbnail
        let thumbnail = self.generate(input)?;

        // Save to cache (原子写：tmp+rename，避免并发请求互相读到半截文件)
        std::fs::create_dir_all(cache_dir)?;
        let tmp_path = cache_dir.join(format!("{}.jpg.tmp", cache_key));
        std::fs::write(&tmp_path, &thumbnail)?;
        std::fs::rename(&tmp_path, &cache_path)?;

        Ok(thumbnail)
    }
}

impl Default for ThumbnailGenerator {
    fn default() -> Self {
        Self::new(300, 88)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_rejects_undecodable_input_without_panicking() {
        // 解码器不支持的输入（如 image crate 没有 avif 解码器时的 avif 数据，或任意垃圾字节）
        // 必须以 Err 返回——路由层据此降级回原始图片，而不是 500。
        let gen = ThumbnailGenerator::default();
        assert!(gen.generate(b"this is definitely not an image").is_err());

        // AVIF 文件头（ftypavif）：在未启用 avif 解码器的构建下应走 Err 分支
        let avif_header = b"\x00\x00\x00\x20ftypavif\x00\x00\x00\x00avifmif1miaf".to_vec();
        assert!(gen.generate(&avif_header).is_err());
    }

    #[test]
    fn generate_with_cache_does_not_write_cache_on_failure() {
        let gen = ThumbnailGenerator::default();
        let dir = tempfile::tempdir().unwrap();
        let result = gen.generate_with_cache(b"garbage bytes", dir.path(), "page0");
        assert!(result.is_err());
        // 失败的生成不应留下缓存文件，否则后续会反复读到损坏的缩略图
        assert!(!dir.path().join("page0.jpg").exists());
    }
}
