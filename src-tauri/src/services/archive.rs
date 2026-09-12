use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

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

/// 持久化解压目录中的签名文件：内容是档案签名 `mtime_secs:len`，用于失效检测。
const EXTRACT_MARKER: &str = ".mv_extracted";

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

/// 进程级工具路径缓存：探测要 spawn 一次 `--help` 子进程，而运行期内工具是否可用不会变；
/// 否则 RAR/7z 每读一页都会重复 spawn 一次探测进程（外加一次解压进程）。
fn tool_cache() -> &'static Mutex<HashMap<&'static str, Option<PathBuf>>> {
    static CACHE: OnceLock<Mutex<HashMap<&'static str, Option<PathBuf>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn resolve_tool_cached(name: &'static str, candidates: &[&str]) -> Option<PathBuf> {
    {
        let cache = tool_cache().lock().unwrap();
        if let Some(hit) = cache.get(name) {
            return hit.clone();
        }
    }
    let resolved = resolve_tool(name, candidates);
    tool_cache().lock().unwrap().insert(name, resolved.clone());
    resolved
}

/// 档案文件签名：(mtime 秒, 长度)。签名变化说明文件被替换/修改，需要重新解压。
fn archive_signature(path: &str) -> Option<(i64, u64)> {
    let md = fs::metadata(path).ok()?;
    let mtime = md
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs() as i64;
    Some((mtime, md.len()))
}

/// 整包解压串行化：并发首访同一档案时只允许一个线程真正解压，
/// 其余线程在锁内重新检查签名后直接命中缓存。
fn extract_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn read_extract_marker(dir: &Path) -> Option<(i64, u64)> {
    let content = fs::read_to_string(dir.join(EXTRACT_MARKER)).ok()?;
    let mut parts = content.trim().splitn(2, ':');
    let mtime = parts.next()?.parse().ok()?;
    let len = parts.next()?.parse().ok()?;
    Some((mtime, len))
}

fn write_extract_marker(dir: &Path, sig: (i64, u64)) -> Result<()> {
    fs::write(dir.join(EXTRACT_MARKER), format!("{}:{}", sig.0, sig.1))?;
    Ok(())
}

/// 页面名是否安全（可安全 join 进缓存目录，拒绝绝对路径与 `..` 逃逸）。
fn is_safe_page_name(name: &str) -> bool {
    let p = Path::new(name);
    !p.is_absolute()
        && p.components()
            .all(|c| !matches!(c, std::path::Component::ParentDir))
}

/// 整包解压后校验：目录内每个条目（含子目录递归）规范化后必须仍位于 `dir` 之内。
/// 外部解压工具遇到恶意条目名（`..`/绝对路径/symlink）时可能把文件写到缓存目录外，
/// 这里兜底拦截并清空缓存。设遍历上限防止病态归档拖死进程。
fn validate_extracted_tree(dir: &Path) -> Result<()> {
    let base = dir.canonicalize()?;
    let mut stack = vec![base.clone()];
    let mut checked = 0usize;
    while let Some(d) = stack.pop() {
        for entry in fs::read_dir(&d)? {
            let entry = entry?;
            let p = entry.path();
            let canon = p
                .canonicalize()
                .map_err(|e| anyhow::anyhow!("cannot resolve {}: {}", p.display(), e))?;
            if !canon.starts_with(&base) {
                anyhow::bail!("extracted entry escapes cache dir: {}", canon.display());
            }
            if entry.file_type()?.is_dir() {
                stack.push(p);
            }
            checked += 1;
            if checked > 100_000 {
                anyhow::bail!("extraction tree too large, aborting validation");
            }
        }
    }
    Ok(())
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
    /// 持久化解压缓存目录；Some 时首次访问整包解压到该目录，之后页面直接读盘。
    cache: Option<PathBuf>,
}

impl RarArchive {
    pub fn new(path: &str, unrar: PathBuf, cache: Option<PathBuf>) -> Result<Self> {
        Ok(Self {
            path: path.to_string(),
            unrar,
            cache,
        })
    }

    /// 若配置了缓存目录且签名不匹配（未解压 / 档案已变更），整包解压一次。
    fn ensure_extracted(&self) -> Result<()> {
        let Some(dir) = self.cache.as_deref() else {
            return Ok(());
        };
        let Some(sig) = archive_signature(&self.path) else {
            return Ok(());
        };

        let _guard = extract_lock().lock().unwrap();
        if read_extract_marker(dir) == Some(sig) {
            return Ok(());
        }
        if dir.exists() {
            let _ = fs::remove_dir_all(dir);
        }
        fs::create_dir_all(dir)?;
        let output = std::process::Command::new(&self.unrar)
            .args(["x", "-o+", "-y", &self.path, &dir.to_string_lossy()])
            .output()?;
        if !output.status.success() {
            anyhow::bail!(
                "Failed to extract archive: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        // 条目路径整体校验：异常即清掉缓存目录并报错，避免脏缓存被后续读取
        if let Err(e) = validate_extracted_tree(dir) {
            let _ = fs::remove_dir_all(dir);
            return Err(e);
        }
        write_extract_marker(dir, sig)?;
        Ok(())
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
            .filter(|line| is_safe_page_name(line) && is_image_file(line))
            .map(|s| s.to_string())
            .collect();

        pages.sort_by(|a, b| natord::compare(a, b));
        Ok(pages)
    }

    fn extract_page(&self, page_name: &str) -> Result<Vec<u8>> {
        // 路径安全：拒绝绝对路径与 `..` 逃逸。缓存命中与回退解压两条路径都必须先过这一关，
        // 否则恶意的归档条目名可让 join 后的路径写到临时目录之外。
        if !is_safe_page_name(page_name) {
            anyhow::bail!("Unsafe page name rejected: {}", page_name);
        }

        // 持久化解压缓存命中时直接读盘：无子进程、无 tempdir、无 O(N²) 顺解（solid 包）
        if self.cache.is_some() {
            self.ensure_extracted()?;
            if let Some(dir) = self.cache.as_deref() {
                let candidate = dir.join(page_name);
                if candidate.is_file() {
                    return Ok(fs::read(&candidate)?);
                }
                tracing::warn!(
                    "Page {} not found in extraction cache {}; falling back to targeted extract",
                    page_name,
                    dir.display()
                );
            }
        }

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
    /// 持久化解压缓存目录；Some 时首次访问整包解压到该目录，之后页面直接读盘。
    cache: Option<PathBuf>,
}

impl SevenZArchive {
    pub fn new(path: &str, sevenz: PathBuf, cache: Option<PathBuf>) -> Result<Self> {
        Ok(Self {
            path: path.to_string(),
            sevenz,
            cache,
        })
    }

    /// 若配置了缓存目录且签名不匹配（未解压 / 档案已变更），整包解压一次。
    fn ensure_extracted(&self) -> Result<()> {
        let Some(dir) = self.cache.as_deref() else {
            return Ok(());
        };
        let Some(sig) = archive_signature(&self.path) else {
            return Ok(());
        };

        let _guard = extract_lock().lock().unwrap();
        if read_extract_marker(dir) == Some(sig) {
            return Ok(());
        }
        if dir.exists() {
            let _ = fs::remove_dir_all(dir);
        }
        fs::create_dir_all(dir)?;
        let output = std::process::Command::new(&self.sevenz)
            .args([
                "x",
                "-y",
                &self.path,
                &format!("-o{}", dir.to_string_lossy()),
            ])
            .output()?;
        if !output.status.success() {
            anyhow::bail!(
                "Failed to extract archive: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        // 条目路径整体校验：与 RarArchive 同理
        if let Err(e) = validate_extracted_tree(dir) {
            let _ = fs::remove_dir_all(dir);
            return Err(e);
        }
        write_extract_marker(dir, sig)?;
        Ok(())
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
                if is_safe_page_name(name) && is_image_file(name) {
                    pages.push(name.to_string());
                }
            }
        }

        pages.sort_by(|a, b| natord::compare(a, b));
        Ok(pages)
    }

    fn extract_page(&self, page_name: &str) -> Result<Vec<u8>> {
        // 路径安全：与 RarArchive 同理，缓存与回退两条路径都先校验条目名。
        if !is_safe_page_name(page_name) {
            anyhow::bail!("Unsafe page name rejected: {}", page_name);
        }

        // 持久化解压缓存命中时直接读盘：无子进程、无 tempdir、无 O(N²) 顺解（solid 包）
        if self.cache.is_some() {
            self.ensure_extracted()?;
            if let Some(dir) = self.cache.as_deref() {
                let candidate = dir.join(page_name);
                if candidate.is_file() {
                    return Ok(fs::read(&candidate)?);
                }
                tracing::warn!(
                    "Page {} not found in extraction cache {}; falling back to targeted extract",
                    page_name,
                    dir.display()
                );
            }
        }

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
    create_archive_reader_impl(path, archive_type, None)
}

/// 带持久化解压缓存目录的版本：RAR/7z 首次访问时整包解压到 `cache_dir`，
/// 之后每页直接读盘（避免每页 spawn 子进程 + tempdir + 整包顺解）。zip/folder 不受影响。
pub fn create_archive_reader_with_cache(
    path: &str,
    archive_type: &str,
    cache_dir: Option<PathBuf>,
) -> Result<Box<dyn ArchiveReader>> {
    create_archive_reader_impl(path, archive_type, cache_dir)
}

fn create_archive_reader_impl(
    path: &str,
    archive_type: &str,
    cache_dir: Option<PathBuf>,
) -> Result<Box<dyn ArchiveReader>> {
    match archive_type {
        "zip" | "cbz" => Ok(Box::new(ZipArchive::new(path)?)),
        "folder" => Ok(Box::new(FolderArchive::new(path)?)),
        "rar" | "cbr" => {
            // Check if unrar is available（结果进程级缓存，避免每页重复探测）
            match resolve_tool_cached("unrar", WINDOWS_UNRAR_CANDIDATES) {
                Some(bin) => Ok(Box::new(RarArchive::new(path, bin, cache_dir)?)),
                None => anyhow::bail!(
                    "RAR support requires the unrar tool. Install it via Homebrew \
                     (macOS: brew install unrar), 7-Zip/WinRAR (Windows: put unrar.exe in PATH \
                     or install WinRAR), or apt (Linux: sudo apt install unrar)"
                ),
            }
        }
        "7z" => {
            // Check if 7z is available（结果进程级缓存，避免每页重复探测）
            match resolve_tool_cached("7z", WINDOWS_7Z_CANDIDATES) {
                Some(bin) => Ok(Box::new(SevenZArchive::new(path, bin, cache_dir)?)),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_tool_cached_is_deterministic_for_missing_tool() {
        // 未知工具应稳定返回 None（缓存命中后不再重复探测）
        let name = "manga-viewer-no-such-tool-xyz";
        assert!(resolve_tool_cached(name, &[]).is_none());
        assert!(resolve_tool_cached(name, &[]).is_none());
    }

    #[cfg(unix)]
    #[test]
    fn resolve_tool_cached_finds_real_tool_on_path() {
        // 用 /bin/true 这类接受 --help 的真实命令验证正路径缓存
        let a = resolve_tool_cached("true", &[]);
        let b = resolve_tool_cached("true", &[]);
        assert_eq!(a, b);
        assert!(a.is_some());
    }

    #[test]
    fn extract_marker_roundtrip_and_page_name_safety() {
        let dir = tempfile::tempdir().unwrap();
        write_extract_marker(dir.path(), (1_700_000_000, 42)).unwrap();
        assert_eq!(read_extract_marker(dir.path()), Some((1_700_000_000, 42)));

        assert!(is_safe_page_name("folder/img01.jpg"));
        assert!(is_safe_page_name("img01.jpg"));
        // 拒绝路径逃逸与绝对路径，防止整包解压缓存被用于任意路径读取
        assert!(!is_safe_page_name("../evil.jpg"));
        assert!(!is_safe_page_name("/etc/passwd"));
        assert!(!is_safe_page_name("a/../../b.jpg"));
    }
}
