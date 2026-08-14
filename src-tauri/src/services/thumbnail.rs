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
        let thumbnail = img.resize(new_w, new_h, image::imageops::FilterType::Lanczos3);

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

        // Save to cache
        std::fs::create_dir_all(cache_dir)?;
        std::fs::write(&cache_path, &thumbnail)?;

        Ok(thumbnail)
    }
}

impl Default for ThumbnailGenerator {
    fn default() -> Self {
        Self::new(300, 88)
    }
}
