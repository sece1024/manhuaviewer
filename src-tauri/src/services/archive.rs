use anyhow::Result;

use super::is_image_file;

pub trait ArchiveReader {
    fn list_pages(&self) -> Result<Vec<String>>;
    fn extract_page(&self, page_name: &str) -> Result<Vec<u8>>;
    fn get_cover(&self) -> Result<Vec<u8>>;
}

// ZIP/CBZ Archive
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

            if is_image_file(&name) {
                pages.push(name);
            }
        }

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

// Folder Archive
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
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if is_image_file(name) {
                        pages.push(path.to_string_lossy().to_string());
                    }
                }
            }
        }

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

// RAR Archive (uses system unrar command)
pub struct RarArchive {
    path: String,
}

impl RarArchive {
    pub fn new(path: &str) -> Result<Self> {
        Ok(Self {
            path: path.to_string(),
        })
    }
}

impl ArchiveReader for RarArchive {
    fn list_pages(&self) -> Result<Vec<String>> {
        let output = std::process::Command::new("unrar")
            .args(["lb", &self.path])
            .output()?;

        if !output.status.success() {
            anyhow::bail!(
                "Failed to list archive: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        let stdout = String::from_utf8(output.stdout)?;
        let mut pages: Vec<String> = stdout
            .lines()
            .filter(|line| is_image_file(line))
            .map(|s| s.to_string())
            .collect();

        pages.sort_by(|a, b| natord::compare(a, b));
        Ok(pages)
    }

    fn extract_page(&self, page_name: &str) -> Result<Vec<u8>> {
        let temp_dir = tempfile::tempdir()?;

        let output = std::process::Command::new("unrar")
            .args([
                "x",
                &self.path,
                page_name,
                &temp_dir.path().to_string_lossy(),
                "-o+",
            ])
            .output()?;

        if !output.status.success() {
            anyhow::bail!(
                "Failed to extract: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        let extracted_path = temp_dir.path().join(page_name);
        if extracted_path.exists() {
            let buffer = std::fs::read(&extracted_path)?;
            return Ok(buffer);
        }

        anyhow::bail!("File not found after extraction: {}", page_name)
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

// 7Z Archive (uses system 7z command)
pub struct SevenZArchive {
    path: String,
}

impl SevenZArchive {
    pub fn new(path: &str) -> Result<Self> {
        Ok(Self {
            path: path.to_string(),
        })
    }
}

impl ArchiveReader for SevenZArchive {
    fn list_pages(&self) -> Result<Vec<String>> {
        let output = std::process::Command::new("7z")
            .args(["l", &self.path])
            .output()?;

        if !output.status.success() {
            anyhow::bail!(
                "Failed to list archive: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        let stdout = String::from_utf8(output.stdout)?;
        let mut pages = Vec::new();

        // Parse 7z output - skip header lines
        for line in stdout.lines().skip(20) {
            if line.is_empty() || line.starts_with("----") {
                continue;
            }
            // 7z output format: Date Time Attr Size Compressed Name
            if let Some(name) = line.split_whitespace().last() {
                if is_image_file(name) {
                    pages.push(name.to_string());
                }
            }
        }

        pages.sort_by(|a, b| natord::compare(a, b));
        Ok(pages)
    }

    fn extract_page(&self, page_name: &str) -> Result<Vec<u8>> {
        let temp_dir = tempfile::tempdir()?;

        let output = std::process::Command::new("7z")
            .args([
                "x",
                &self.path,
                &format!("-o{}", temp_dir.path().to_string_lossy()),
                page_name,
                "-y",
            ])
            .output()?;

        if !output.status.success() {
            anyhow::bail!(
                "Failed to extract: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        let extracted_path = temp_dir.path().join(page_name);
        if extracted_path.exists() {
            let buffer = std::fs::read(&extracted_path)?;
            return Ok(buffer);
        }

        anyhow::bail!("File not found after extraction: {}", page_name)
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

pub fn create_archive_reader(path: &str, archive_type: &str) -> Result<Box<dyn ArchiveReader>> {
    match archive_type {
        "zip" | "cbz" => Ok(Box::new(ZipArchive::new(path)?)),
        "folder" => Ok(Box::new(FolderArchive::new(path)?)),
        "rar" | "cbr" => {
            // Check if unrar is available
            if std::process::Command::new("unrar")
                .arg("--help")
                .output()
                .is_ok()
            {
                Ok(Box::new(RarArchive::new(path)?))
            } else {
                anyhow::bail!(
                    "RAR support requires unrar to be installed. Install with: brew install unrar"
                )
            }
        }
        "7z" => {
            // Check if 7z is available
            if std::process::Command::new("7z")
                .arg("--help")
                .output()
                .is_ok()
            {
                Ok(Box::new(SevenZArchive::new(path)?))
            } else {
                anyhow::bail!(
                    "7Z support requires p7zip to be installed. Install with: brew install p7zip"
                )
            }
        }
        _ => anyhow::bail!("Unsupported archive type: {}", archive_type),
    }
}
