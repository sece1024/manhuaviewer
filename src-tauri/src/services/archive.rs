use anyhow::Result;
use std::path::Path;

pub trait ArchiveReader {
    fn list_pages(&self) -> Result<Vec<String>>;
    fn extract_page(&self, page_name: &str) -> Result<Vec<u8>>;
    fn get_cover(&self) -> Result<Vec<u8>>;
}

pub struct ZipArchive {
    path: String,
}

impl ZipArchive {
    pub fn new(path: &str) -> Result<Self> {
        Ok(Self {
            path: path.to_string(),
        })
    }
}

impl ArchiveReader for ZipArchive {
    fn list_pages(&self) -> Result<Vec<String>> {
        let file = std::fs::File::open(&self.path)?;
        let mut archive = zip::ZipArchive::new(file)?;
        
        let mut pages = Vec::new();
        for i in 0..archive.len() {
            let file = archive.by_index(i)?;
            let name = file.name().to_string();
            
            // Check if it's an image file
            if let Some(ext) = Path::new(&name).extension() {
                let ext = ext.to_string_lossy().to_lowercase();
                if matches!(ext.as_str(), "jpg" | "jpeg" | "png" | "gif" | "webp" | "bmp") {
                    pages.push(name);
                }
            }
        }
        
        // Natural sort
        pages.sort_by(|a, b| natord::compare(a, b));
        
        Ok(pages)
    }

    fn extract_page(&self, page_name: &str) -> Result<Vec<u8>> {
        let file = std::fs::File::open(&self.path)?;
        let mut archive = zip::ZipArchive::new(file)?;
        
        let mut file = archive.by_name(page_name)?;
        let mut buffer = Vec::new();
        std::io::Read::read_to_end(&mut file, &mut buffer)?;
        
        Ok(buffer)
    }

    fn get_cover(&self) -> Result<Vec<u8>> {
        let pages = self.list_pages()?;
        if let Some(first_page) = pages.first() {
            self.extract_page(first_page)
        } else {
            anyhow::bail!("No pages found in archive")
        }
    }
}

pub struct FolderArchive {
    path: String,
}

impl FolderArchive {
    pub fn new(path: &str) -> Result<Self> {
        Ok(Self {
            path: path.to_string(),
        })
    }
}

impl ArchiveReader for FolderArchive {
    fn list_pages(&self) -> Result<Vec<String>> {
        let mut pages = Vec::new();
        
        for entry in std::fs::read_dir(&self.path)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_file() {
                if let Some(ext) = path.extension() {
                    let ext = ext.to_string_lossy().to_lowercase();
                    if matches!(ext.as_str(), "jpg" | "jpeg" | "png" | "gif" | "webp" | "bmp") {
                        pages.push(path.to_string_lossy().to_string());
                    }
                }
            }
        }
        
        // Natural sort
        pages.sort_by(|a, b| natord::compare(a, b));
        
        Ok(pages)
    }

    fn extract_page(&self, page_name: &str) -> Result<Vec<u8>> {
        std::fs::read(page_name).map_err(Into::into)
    }

    fn get_cover(&self) -> Result<Vec<u8>> {
        let pages = self.list_pages()?;
        if let Some(first_page) = pages.first() {
            self.extract_page(first_page)
        } else {
            anyhow::bail!("No pages found in folder")
        }
    }
}

pub fn create_archive_reader(path: &str, archive_type: &str) -> Result<Box<dyn ArchiveReader>> {
    match archive_type {
        "zip" | "cbz" => Ok(Box::new(ZipArchive::new(path)?)),
        "folder" => Ok(Box::new(FolderArchive::new(path)?)),
        // TODO: Add RAR and 7Z support
        _ => anyhow::bail!("Unsupported archive type: {}", archive_type),
    }
}
