use anyhow::Result;
use std::path::Path;
use walkdir::WalkDir;

use super::is_image_file;

/// 遍历发现的一个候选档案（压缩包，或含图片的目录），连同遍历阶段**顺手拿到**的元数据。
///
/// 为什么把 size/mtime 一起带出来：增量扫描的「未变化就跳过」判定只需要这两个数。
/// 以前每个条目要 `path.is_dir()`（1 次 stat）+ `fs::metadata()`（取大小，1 次）+
/// `fs::metadata()`（取 mtime，又 1 次），一共 3 次 syscall；而上万本的书库、或库放在
/// 网络盘/慢盘上时，重复 stat 就是扫描的主要耗时。`walkdir` 的 `DirEntry::metadata()`
/// 在 Windows 上直接取自目录枚举返回的 WIN32_FIND_DATA（不额外 stat），Unix 上是一次
/// lstat——一次就够。非候选文件（纯图片、说明文本等）则一次 stat 都不做。
pub struct Discovered {
    pub path: String,
    pub is_dir: bool,
    pub file_size: i64,
    pub file_mtime: i64,
}

/// 归档扩展名白名单（小写比较）。
fn is_archive_ext(ext: &std::ffi::OsStr) -> bool {
    matches!(
        ext.to_string_lossy().to_lowercase().as_str(),
        "zip" | "cbz" | "rar" | "cbr" | "7z"
    )
}

/// 从一次 `Metadata` 取「大小 + mtime 秒数」，与 `fs_ext` 的同一份计算保持一致。
fn size_and_mtime(m: &std::fs::Metadata) -> (i64, i64) {
    crate::services::fs_ext::size_and_mtime_secs(m)
}

/// 取候选条目的元数据。符号链接要跟随（与原实现 `fs::metadata` 的行为一致，
/// 指向压缩包/目录的链接仍按目标的大小与 mtime 判定是否更新）；普通条目直接用
/// walkdir 已经拿到的元数据，不再额外 stat。
fn entry_size_and_mtime(entry: &walkdir::DirEntry) -> (i64, i64) {
    if entry.file_type().is_symlink() {
        std::fs::metadata(entry.path())
            .map(|m| size_and_mtime(&m))
            .unwrap_or((0, 0))
    } else {
        entry
            .metadata()
            .map(|m| size_and_mtime(&m))
            .unwrap_or((0, 0))
    }
}

pub struct Scanner;

impl Scanner {
    pub fn new() -> Self {
        Self
    }

    pub fn scan_directory(&self, root_path: &str, depth: u32) -> Result<Vec<Discovered>> {
        let mut archives = Vec::new();

        let walker = if depth == 0 {
            WalkDir::new(root_path).max_depth(1)
        } else {
            WalkDir::new(root_path).max_depth(depth as usize)
        };

        for entry in walker {
            let entry = entry?;
            let path = entry.path();
            let file_type = entry.file_type();
            // file_type 由 read_dir 带回（Windows 上是 WIN32_FIND_DATA），不花 syscall。
            // 符号链接默认不跟随，只有链接才需要按目标再判一次目录。
            let is_dir = file_type.is_dir() || (file_type.is_symlink() && path.is_dir());

            let is_archive = !is_dir && path.extension().map(is_archive_ext).unwrap_or(false);
            if !is_dir && !is_archive {
                continue; // 非候选：不 stat
            }
            if is_dir && !self.is_image_folder(path) {
                continue; // 目录里没有图片：不是档案
            }

            let (file_size, file_mtime) = entry_size_and_mtime(&entry);
            archives.push(Discovered {
                path: path.to_string_lossy().to_string(),
                is_dir,
                file_size,
                file_mtime,
            });
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
        self.detect_archive_type_with(path, Path::new(path).is_dir())
    }

    /// 已知条目是不是目录时用这个版本，省掉 `detect_archive_type` 里的又一次 stat。
    pub fn detect_archive_type_with(&self, path: &str, is_dir: bool) -> String {
        let path = Path::new(path);

        if is_dir {
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
    use super::{derive_title, Scanner};
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

    /// 发现阶段只收「压缩包」与「含图片的目录」，并把大小/mtime 一并带出，
    /// 调用方无需再为每个档案 stat。
    #[test]
    fn scan_directory_reports_type_and_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("lib");
        let folder_book = root.join("folder-book");
        std::fs::create_dir_all(&folder_book).unwrap();
        std::fs::write(folder_book.join("p01.jpg"), b"x").unwrap();
        std::fs::create_dir_all(root.join("not-a-book")).unwrap(); // 无图片：不是档案
        std::fs::write(root.join("note.txt"), b"x").unwrap(); // 非归档扩展名
        let cbz = root.join("book.cbz");
        std::fs::write(&cbz, b"not a real zip").unwrap();

        let found = Scanner::new()
            .scan_directory(root.to_str().unwrap(), 1)
            .unwrap();
        let names: Vec<(String, bool)> = found
            .iter()
            .map(|d| {
                (
                    Path::new(&d.path)
                        .file_name()
                        .unwrap()
                        .to_string_lossy()
                        .to_string(),
                    d.is_dir,
                )
            })
            .collect();
        assert_eq!(
            names.len(),
            2,
            "只应发现 folder-book 与 book.cbz：{names:?}"
        );
        assert!(
            names.contains(&("folder-book".to_string(), true)),
            "{names:?}"
        );
        assert!(
            names.contains(&("book.cbz".to_string(), false)),
            "{names:?}"
        );

        let cbz_entry = found
            .iter()
            .find(|d| d.path == cbz.to_string_lossy())
            .unwrap();
        assert_eq!(cbz_entry.file_size, 14);
        assert!(cbz_entry.file_mtime > 0, "mtime 应为 Unix 秒数");
    }
}
