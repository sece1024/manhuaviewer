use anyhow::Result;
use image::{io::Reader as ImageReader, ImageOutputFormat, DynamicImage, GenericImageView};
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
            quality 
        }
    }

    pub fn with_height(mut self, height: u32) -> Self {
        self.height = Some(height);
        self
    }

    pub fn generate(&self, input: &[u8]) -> Result<Vec<u8>> {
        let img = ImageReader::new(std::io::Cursor::new(input))
            .with_guessed_format()?
            .decode()?;
        
        let height = self.height.unwrap_or(self.width);
        let thumbnail = img.resize(
            self.width,
            height,
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

    pub fn generate_cover_thumbnail(
        &self,
        input: &[u8],
        max_width: u32,
        max_height: u32,
    ) -> Result<Vec<u8>> {
        let img = ImageReader::new(std::io::Cursor::new(input))
            .with_guessed_format()?
            .decode()?;
        
        // Calculate dimensions maintaining aspect ratio
        let (orig_width, orig_height) = img.dimensions();
        let ratio = f64::min(
            max_width as f64 / orig_width as f64,
            max_height as f64 / orig_height as f64,
        );
        
        let new_width = (orig_width as f64 * ratio) as u32;
        let new_height = (orig_height as f64 * ratio) as u32;
        
        let thumbnail = img.resize_exact(
            new_width,
            new_height,
            image::imageops::FilterType::Lanczos3,
        );
        
        let mut output = Vec::new();
        thumbnail.write_to(
            &mut std::io::Cursor::new(&mut output),
            ImageOutputFormat::Jpeg(self.quality),
        )?;
        
        Ok(output)
    }

    pub fn convert_format(&self, input: &[u8], format: ImageFormat) -> Result<Vec<u8>> {
        let img = ImageReader::new(std::io::Cursor::new(input))
            .with_guessed_format()?
            .decode()?;
        
        let mut output = Vec::new();
        match format {
            ImageFormat::Jpeg => {
                img.write_to(
                    &mut std::io::Cursor::new(&mut output),
                    ImageOutputFormat::Jpeg(self.quality),
                )?;
            },
            ImageFormat::Png => {
                img.write_to(
                    &mut std::io::Cursor::new(&mut output),
                    ImageOutputFormat::Png,
                )?;
            },
            ImageFormat::WebP => {
                img.write_to(
                    &mut std::io::Cursor::new(&mut output),
                    ImageOutputFormat::WebP,
                )?;
            },
        }
        
        Ok(output)
    }
}

pub enum ImageFormat {
    Jpeg,
    Png,
    WebP,
}

impl Default for ThumbnailGenerator {
    fn default() -> Self {
        Self::new(300, 85)
    }
}

// Utility functions for image processing
pub fn get_image_dimensions(input: &[u8]) -> Result<(u32, u32)> {
    let img = ImageReader::new(std::io::Cursor::new(input))
        .with_guessed_format()?
        .decode()?;
    
    Ok((img.width(), img.height()))
}

pub fn is_valid_image(input: &[u8]) -> bool {
    ImageReader::new(std::io::Cursor::new(input))
        .with_guessed_format()
        .ok()
        .and_then(|reader| reader.decode().ok())
        .is_some()
}

pub fn resize_image(input: &[u8], width: u32, height: u32) -> Result<Vec<u8>> {
    let img = ImageReader::new(std::io::Cursor::new(input))
        .with_guessed_format()?
        .decode()?;
    
    let resized = img.resize_exact(width, height, image::imageops::FilterType::Lanczos3);
    
    let mut output = Vec::new();
    resized.write_to(
        &mut std::io::Cursor::new(&mut output),
        ImageOutputFormat::Jpeg(85),
    )?;
    
    Ok(output)
}
