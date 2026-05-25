use std::path::Path;
use walkdir::WalkDir;
use anyhow::Result;

pub struct Scanner;

impl Scanner {
    pub fn new() -> Self {
        Self
    }

    pub fn scan_directory(
        &self,
        root_path: &str,
        depth: u32,
    ) -> Result<Vec<String>> {
        let mut archives = Vec::new();
        
        let walker = if depth == 0 {
            WalkDir::new(root_path).max_depth(1)
        } else {
            WalkDir::new(root_path).max_depth(depth as usize)
        };

        for entry in walker {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_dir() {
                // Check if it's a folder archive (contains images)
                if self.is_image_folder(path) {
                    archives.push(path.to_string_lossy().to_string());
                }
            } else if let Some(ext) = path.extension() {
                let ext = ext.to_string_lossy().to_lowercase();
                if matches!(ext.as_str(), "zip" | "cbz" | "rar" | "cbr" | "7z") {
                    archives.push(path.to_string_lossy().to_string());
                }
            }
        }

        Ok(archives)
    }

    fn is_image_folder(&self, path: &Path) -> bool {
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                if let Some(ext) = entry.path().extension() {
                    let ext = ext.to_string_lossy().to_lowercase();
                    if matches!(ext.as_str(), "jpg" | "jpeg" | "png" | "gif" | "webp" | "bmp") {
                        return true;
                    }
                }
            }
        }
        false
    }

    pub fn detect_archive_type(&self, path: &str) -> String {
        let path = Path::new(path);
        
        if path.is_dir() {
            return "folder".to_string();
        }
        
        if let Some(ext) = path.extension() {
            let ext = ext.to_string_lossy().to_lowercase();
            match ext.as_str() {
                "zip" => "zip".to_string(),
                "cbz" => "cbz".to_string(),
                "rar" => "rar".to_string(),
                "cbr" => "cbr".to_string(),
                "7z" => "7z".to_string(),
                _ => "unknown".to_string(),
            }
        } else {
            "unknown".to_string()
        }
    }
}
