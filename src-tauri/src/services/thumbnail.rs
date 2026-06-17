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

        let height = self.height.unwrap_or(self.width);
        let thumbnail = img.resize(self.width, height, image::imageops::FilterType::Lanczos3);

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
        Self::new(300, 85)
    }
}
