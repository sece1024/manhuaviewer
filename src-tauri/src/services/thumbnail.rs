use anyhow::Result;
use image::{io::Reader as ImageReader, ImageOutputFormat};

pub struct ThumbnailGenerator {
    width: u32,
    quality: u8,
}

impl ThumbnailGenerator {
    pub fn new(width: u32, quality: u8) -> Self {
        Self { width, quality }
    }

    pub fn generate(&self, input: &[u8]) -> Result<Vec<u8>> {
        let img = ImageReader::new(std::io::Cursor::new(input))
            .with_guessed_format()?
            .decode()?;
        
        let thumbnail = img.resize(
            self.width,
            self.width,
            image::imageops::FilterType::Lanczos3,
        );
        
        let mut output = Vec::new();
        thumbnail.write_to(
            &mut std::io::Cursor::new(&mut output),
            ImageOutputFormat::Jpeg(self.quality),
        )?;
        
        Ok(output)
    }

    pub fn generate_from_path(&self, path: &str) -> Result<Vec<u8>> {
        let input = std::fs::read(path)?;
        self.generate(&input)
    }

    pub fn get_dimensions(&self, input: &[u8]) -> Result<(u32, u32)> {
        let img = ImageReader::new(std::io::Cursor::new(input))
            .with_guessed_format()?
            .decode()?;
        
        Ok((img.width(), img.height()))
    }
}

impl Default for ThumbnailGenerator {
    fn default() -> Self {
        Self::new(300, 85)
    }
}
