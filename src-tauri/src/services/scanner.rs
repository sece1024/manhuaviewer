use anyhow::Result;
use std::path::Path;
use walkdir::WalkDir;

use super::is_image_file;

pub struct Scanner;

impl Scanner {
    pub fn new() -> Self {
        Self
    }

    pub fn scan_directory(&self, root_path: &str, depth: u32) -> Result<Vec<String>> {
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
                if let Some(name) = entry.path().file_name().and_then(|n| n.to_str()) {
                    if is_image_file(name) {
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

/// 根据路径取倒数第 `level` 层组件作为标题。
/// 层级 1 = 最后一层（文件时用 file_stem 剥扩展名），层级 N = 往前第 N 层目录名。
/// level 超出路径实际层数时，回退到最后一层；调用方需保证 level >= 1。
pub fn derive_title(path: &Path, level: u32) -> String {
    let components: Vec<String> = path
        .components()
        .filter_map(|c| match c {
            std::path::Component::Normal(name) => Some(name.to_string_lossy().to_string()),
            _ => None,
        })
        .collect();

    let n = components.len();
    if n == 0 {
        return String::new();
    }

    let idx = if level as usize > n {
        1
    } else {
        level as usize
    };
    let name = &components[n - idx];

    if idx == 1 {
        std::path::Path::new(name)
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string()
    } else {
        name.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::derive_title;
    use std::path::Path;

    #[test]
    fn test_derive_title_level_1() {
        assert_eq!(
            derive_title(Path::new("/path/to/manhua01/第一章"), 1),
            "第一章"
        );
    }

    #[test]
    fn test_derive_title_level_2() {
        assert_eq!(
            derive_title(Path::new("/path/to/manhua01/第一章"), 2),
            "manhua01"
        );
    }

    #[test]
    fn test_derive_title_level_3() {
        assert_eq!(derive_title(Path::new("/path/to/manhua01/第一章"), 3), "to");
    }

    #[test]
    fn test_derive_title_strips_extension() {
        assert_eq!(derive_title(Path::new("/path/manhua01.cbz"), 1), "manhua01");
    }

    #[test]
    fn test_derive_title_deep_level_keeps_dir_name() {
        assert_eq!(derive_title(Path::new("/path/manhua01.cbz"), 2), "path");
    }

    #[test]
    fn test_derive_title_exceeds_depth_falls_back() {
        assert_eq!(derive_title(Path::new("/a/第一章"), 3), "第一章");
    }

    #[test]
    fn test_derive_title_single_component() {
        assert_eq!(derive_title(Path::new("/第一章"), 1), "第一章");
    }

    #[test]
    fn test_derive_title_empty_path() {
        assert_eq!(derive_title(Path::new(""), 1), "");
    }
}
