use anyhow::Result;
use std::path::PathBuf;

use super::is_image_file;

// Windows 上 unrar/7z 通常不在 PATH 里，需探测常见安装目录。
const WINDOWS_UNRAR_CANDIDATES: &[&str] = &[
    r"C:\Program Files\WinRAR\UnRAR.exe",
    r"C:\Program Files\WinRAR\Rar.exe",
    r"C:\Program Files (x86)\WinRAR\UnRAR.exe",
    r"C:\Program Files (x86)\WinRAR\Rar.exe",
];

const WINDOWS_7Z_CANDIDATES: &[&str] = &[
    r"C:\Program Files\7-Zip\7z.exe",
    r"C:\Program Files (x86)\7-Zip\7z.exe",
];

/// 解析外部工具路径：先查 PATH，再查 Windows 常见安装目录。
fn resolve_tool(exe: &str, _windows_candidates: &[&str]) -> Option<PathBuf> {
    if std::process::Command::new(exe)
        .arg("--help")
        .output()
        .is_ok()
    {
        return Some(PathBuf::from(exe));
    }
    #[cfg(windows)]
    {
        for candidate in _windows_candidates {
            if std::path::Path::new(candidate).exists() {
                return Some(PathBuf::from(candidate));
            }
        }
    }
    None
}

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
    unrar: PathBuf,
}

impl RarArchive {
    pub fn new(path: &str, unrar: PathBuf) -> Result<Self> {
        Ok(Self {
            path: path.to_string(),
            unrar,
        })
    }
}

impl ArchiveReader for RarArchive {
    fn list_pages(&self) -> Result<Vec<String>> {
        let output = std::process::Command::new(&self.unrar)
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

        let output = std::process::Command::new(&self.unrar)
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
    sevenz: PathBuf,
}

impl SevenZArchive {
    pub fn new(path: &str, sevenz: PathBuf) -> Result<Self> {
        Ok(Self {
            path: path.to_string(),
            sevenz,
        })
    }
}

impl ArchiveReader for SevenZArchive {
    fn list_pages(&self) -> Result<Vec<String>> {
        let output = std::process::Command::new(&self.sevenz)
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

        let output = std::process::Command::new(&self.sevenz)
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
            match resolve_tool("unrar", WINDOWS_UNRAR_CANDIDATES) {
                Some(bin) => Ok(Box::new(RarArchive::new(path, bin)?)),
                None => anyhow::bail!(
                    "RAR support requires the unrar tool. Install it via Homebrew \
                     (macOS: brew install unrar), 7-Zip/WinRAR (Windows: put unrar.exe in PATH \
                     or install WinRAR), or apt (Linux: sudo apt install unrar)"
                ),
            }
        }
        "7z" => {
            // Check if 7z is available
            match resolve_tool("7z", WINDOWS_7Z_CANDIDATES) {
                Some(bin) => Ok(Box::new(SevenZArchive::new(path, bin)?)),
                None => anyhow::bail!(
                    "7Z support requires the 7z tool. Install 7-Zip (Windows), \
                     Homebrew p7zip (macOS: brew install p7zip), or apt \
                     (Linux: sudo apt install p7zip-full), and make sure 7z is in PATH"
                ),
            }
        }
        _ => anyhow::bail!("Unsupported archive type: {}", archive_type),
    }
}
