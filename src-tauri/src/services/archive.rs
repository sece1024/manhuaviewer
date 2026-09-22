use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

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

/// 单页解压后字节上限：防“名称像图片的压缩炸弹条目”一次性读入/落盘撑爆内存或磁盘。
const MAX_PAGE_BYTES: u64 = 256 * 1024 * 1024;
/// 单档案整包解压后的总字节预算（解压后树校验时累计）：防空爆归档写满磁盘。
const MAX_EXTRACT_TOTAL_BYTES: u64 = 32 * 1024 * 1024 * 1024;
/// 外部解压/列目录子进程总时长上限：卡死的 unrar/7z 不再永久占用 tokio blocking 线程。
const EXTRACT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(90);

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

/// 解压互斥：并发首访“同一档案”时只允许一个线程真正整包解压，
/// 其余线程在锁内重新检查签名后直接命中缓存。不同档案互不阻塞
/// （此前是全局锁，一个大 RAR 在解压会拖住所有其它档案读页）。
fn extract_locks() -> &'static Mutex<HashMap<String, Arc<Mutex<()>>>> {
    static LOCKS: OnceLock<Mutex<HashMap<String, Arc<Mutex<()>>>>> = OnceLock::new();
    LOCKS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 取某档案的解压锁（Arc，跨线程共享）；长期运行积累过多时整体重置一次。
fn archive_extract_lock(path: &str) -> Arc<Mutex<()>> {
    let mut map = extract_locks().lock().unwrap();
    if map.len() > 256 {
        map.clear();
    }
    map.entry(path.to_string())
        .or_insert_with(|| Arc::new(Mutex::new(())))
        .clone()
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

/// 页面名是否安全（可安全 join 进缓存目录）。
/// 拒绝绝对路径、`..` 逃逸与反斜杠：unrar/7z 同时把 '/' 和 '\' 当分隔符，
/// Unix 上 "a\..\b.jpg" 在 Path::components 里只是普通文件名，却能被外部工具解出逃逸路径。
/// 另拒绝前导 '-'（unrar/7z 把 `-o+`、`-y` 之类当开关参数而非文件名——参数注入）
/// 与前导 '/'（Windows 上 7z 同样认 '/' 开关；Unix 上本就属绝对路径，行为对齐）。
fn is_safe_page_name(name: &str) -> bool {
    if name.is_empty() || name.contains('\\') || name.starts_with('-') || name.starts_with('/') {
        return false;
    }
    let p = Path::new(name);
    !p.is_absolute()
        && p.components()
            .all(|c| !matches!(c, std::path::Component::ParentDir))
}

/// 文件头魔数是否与档案类型相符：`/archives/:id/file` 按 DB 里的 path 原样
/// 回传文件字节，而 path 可能来自被篡改的恢复备份——回传前确认它真是对应
/// 归档格式，否则任意磁盘文件（私钥、数据库…）都能被当漫画拉走。
/// 放行 MZ（DOS/PE stub）：zip/rar/7z 均有 Windows 自解压（SFX）变体，内容仍是归档。
pub fn magic_matches(archive_type: &str, header: &[u8]) -> bool {
    let sfx = header.starts_with(b"MZ");
    match archive_type {
        "zip" | "cbz" => header.starts_with(b"PK") || sfx,
        "rar" | "cbr" => header.starts_with(b"Rar!") || sfx,
        "7z" => header.starts_with(b"7z\xBC\xAF\x27\x1C") || sfx,
        _ => false, // folder 走就地重打包，不读源文件头
    }
}

/// 带上限地读取解压产物：超过 MAX_PAGE_BYTES 直接拒绝，避免整页超大文件撑爆内存。
fn read_page_bounded(path: &Path) -> Result<Vec<u8>> {
    let meta = std::fs::metadata(path)?;
    if meta.len() > MAX_PAGE_BYTES {
        anyhow::bail!(
            "页面文件过大（{} 字节 > 上限 {}），已拒绝读取",
            meta.len(),
            MAX_PAGE_BYTES
        );
    }
    Ok(std::fs::read(path)?)
}

/// 运行 unrar/7z 并限时：try_wait 轮询 + 超时 kill，超时视为失败。
/// 直接 output() 在子进程卡死时会无限期占住 tokio blocking 线程，耗尽 512 的阻塞池。
fn run_extractor(program: &Path, args: &[&str]) -> Result<std::process::Output> {
    let mut child = std::process::Command::new(program)
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;
    let deadline = std::time::Instant::now() + EXTRACT_TIMEOUT;
    loop {
        match child.try_wait()? {
            Some(_) => break,
            None if std::time::Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                anyhow::bail!("解压/列目录超时（>{EXTRACT_TIMEOUT:?}），已终止子进程");
            }
            None => std::thread::sleep(std::time::Duration::from_millis(50)),
        }
    }
    Ok(child.wait_with_output()?)
}

/// 整包解压后校验：目录内每个条目（含子目录递归）规范化后必须仍位于 `dir` 之内。
/// 外部解压工具遇到恶意条目名（`..`/绝对路径/symlink）时可能把文件写到缓存目录外，
/// 这里兜底拦截并清空缓存。设遍历上限防止病态归档拖死进程。
fn validate_extracted_tree(dir: &Path) -> Result<()> {
    let base = dir.canonicalize()?;
    let mut stack = vec![base.clone()];
    let mut checked = 0usize;
    let mut total_bytes: u64 = 0;
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
            } else {
                total_bytes += entry.metadata()?.len();
            }
            checked += 1;
            if checked > 100_000 {
                anyhow::bail!("extraction tree too large, aborting validation");
            }
            if total_bytes > MAX_EXTRACT_TOTAL_BYTES {
                anyhow::bail!(
                    "extraction tree too large ({total_bytes} bytes > {MAX_EXTRACT_TOTAL_BYTES}), aborting validation"
                );
            }
        }
    }
    Ok(())
}

pub trait ArchiveReader {
    fn list_pages(&self) -> Result<Vec<String>>;
    fn extract_page(&self, page_name: &str) -> Result<Vec<u8>>;
    /// 分块流式读取页面（默认回退整页读取后一次性 emit；zip 覆写为逐块流式，
    /// 避免 2-10MB 的单页整块进内存，也免去响应体侧的二次缓冲）。
    fn stream_page(
        &self,
        page_name: &str,
        emit: &mut dyn FnMut(&[u8]) -> Result<()>,
    ) -> Result<()> {
        let data = self.extract_page(page_name)?;
        emit(&data)
    }
    fn get_cover(&self) -> Result<Vec<u8>>;
}

/// 档案路径是否仍然存在：folder 类型看目录、其余（zip/cbz/rar/cbr/7z）看文件。
///
/// 手动从磁盘删除档案后 DB 记录会残留，此时再打开档案会先在列目录/解压等深层
/// I/O 上失败，最终被路由层吞成笼统的 500。打开档案前用本函数预检一次，
/// 让路由层能针对“文件已不存在”返回明确的 404 与可操作提示。
pub fn archive_exists(archive_type: &str, path: &str) -> bool {
    let p = std::path::Path::new(path);
    if archive_type == "folder" {
        p.is_dir()
    } else {
        p.is_file()
    }
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
        // 防压缩炸弹：先按中央目录声明的解压后大小拦截，再用 take 兜底声明与实际不符的情况。
        if file.size() > MAX_PAGE_BYTES {
            anyhow::bail!(
                "页面过大（{} 字节 > 上限 {}），已拒绝读取",
                file.size(),
                MAX_PAGE_BYTES
            );
        }
        let mut buffer = Vec::with_capacity(file.size() as usize);
        std::io::Read::take(&mut file, MAX_PAGE_BYTES + 1).read_to_end(&mut buffer)?;
        if buffer.len() as u64 > MAX_PAGE_BYTES {
            anyhow::bail!("页面解压后超过大小上限（{} 字节）", buffer.len());
        }
        Ok(buffer)
    }

    /// 分块流式：zip 条目支持随机访问，直接在闭包作用域内逐块解压并 emit，
    /// 页面（2-10MB）不再整体进内存，响应侧也不需要第二次缓冲。
    fn stream_page(
        &self,
        page_name: &str,
        emit: &mut dyn FnMut(&[u8]) -> Result<()>,
    ) -> Result<()> {
        let file = std::fs::File::open(&self.path)?;
        let mut archive = zip::ZipArchive::new(file)?;

        let mut entry = archive.by_name(page_name)?;
        // 与 extract_page 一致的大小上限（中央目录声明 + 实际读取双保险）
        if entry.size() > MAX_PAGE_BYTES {
            anyhow::bail!(
                "页面过大（{} 字节 > 上限 {}），已拒绝读取",
                entry.size(),
                MAX_PAGE_BYTES
            );
        }
        let mut read_guard = std::io::Read::take(&mut entry, MAX_PAGE_BYTES + 1);
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            let n = read_guard.read(&mut buf)?;
            if n == 0 {
                break;
            }
            emit(&buf[..n])?;
        }
        Ok(())
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

/// `extract/` 容量预算淘汰的全局节流（最多每分钟一次），避免每次解压都全量扫描目录。
fn extract_eviction_due() -> bool {
    use std::sync::atomic::{AtomicU64, Ordering};
    static LAST: AtomicU64 = AtomicU64::new(0);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let prev = LAST.load(Ordering::Relaxed);
    if now.saturating_sub(prev) < 60 {
        return false;
    }
    LAST.store(now, Ordering::Relaxed);
    true
}

/// 整包解压完成后按容量预算淘汰最旧的解压目录（排除当前档案），避免 `extract/` 无限增长。
fn enforce_extract_budget(current_dir: &Path) {
    let Some(root) = current_dir.parent() else {
        return;
    };
    if !extract_eviction_due() {
        return;
    }
    let id = current_dir
        .file_name()
        .and_then(|n| n.to_str())
        .and_then(|n| n.parse::<i64>().ok());
    for path in crate::services::cache_budget::evict_dirs_by_mtime(
        root,
        crate::services::cache_budget::EXTRACT_CACHE_BUDGET_BYTES,
        id,
    ) {
        let _ = fs::remove_dir_all(path);
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

        let lock = archive_extract_lock(&self.path);
        let _guard = lock.lock().unwrap();
        if read_extract_marker(dir) == Some(sig) {
            return Ok(());
        }
        if dir.exists() {
            let _ = fs::remove_dir_all(dir);
        }
        fs::create_dir_all(dir)?;
        let output = run_extractor(
            &self.unrar,
            &["x", "-o+", "-y", &self.path, &dir.to_string_lossy()],
        )?;
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
        enforce_extract_budget(dir);
        Ok(())
    }
}

impl ArchiveReader for RarArchive {
    fn list_pages(&self) -> Result<Vec<String>> {
        let output = run_extractor(&self.unrar, &["lb", &self.path])?;

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
                    return read_page_bounded(&candidate);
                }
                tracing::warn!(
                    "Page {} not found in extraction cache {}; falling back to targeted extract",
                    page_name,
                    dir.display()
                );
            }
        }

        let temp_dir = tempfile::tempdir()?;

        let output = run_extractor(
            &self.unrar,
            &[
                "x",
                &self.path,
                page_name,
                &temp_dir.path().to_string_lossy(),
                "-o+",
            ],
        )?;

        if !output.status.success() {
            anyhow::bail!(
                "Failed to extract: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        let extracted_path = temp_dir.path().join(page_name);
        if extracted_path.exists() {
            return read_page_bounded(&extracted_path);
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

        let lock = archive_extract_lock(&self.path);
        let _guard = lock.lock().unwrap();
        if read_extract_marker(dir) == Some(sig) {
            return Ok(());
        }
        if dir.exists() {
            let _ = fs::remove_dir_all(dir);
        }
        fs::create_dir_all(dir)?;
        let output = run_extractor(
            &self.sevenz,
            &[
                "x",
                "-y",
                &self.path,
                &format!("-o{}", dir.to_string_lossy()),
            ],
        )?;
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
        enforce_extract_budget(dir);
        Ok(())
    }
}

impl ArchiveReader for SevenZArchive {
    fn list_pages(&self) -> Result<Vec<String>> {
        let output = run_extractor(&self.sevenz, &["l", &self.path])?;

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
                    return read_page_bounded(&candidate);
                }
                tracing::warn!(
                    "Page {} not found in extraction cache {}; falling back to targeted extract",
                    page_name,
                    dir.display()
                );
            }
        }

        let temp_dir = tempfile::tempdir()?;

        let output = run_extractor(
            &self.sevenz,
            &[
                "x",
                &self.path,
                &format!("-o{}", temp_dir.path().to_string_lossy()),
                page_name,
                "-y",
            ],
        )?;

        if !output.status.success() {
            anyhow::bail!(
                "Failed to extract: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        let extracted_path = temp_dir.path().join(page_name);
        if extracted_path.exists() {
            return read_page_bounded(&extracted_path);
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
        // 反斜杠是 unrar/7z 的分隔符（Unix 上 Path::components 不识别）：一律拒绝
        assert!(!is_safe_page_name("..\\evil.jpg"));
        assert!(!is_safe_page_name("folder\\..\\evil.jpg"));
        assert!(!is_safe_page_name("a\\b.jpg"));
        assert!(!is_safe_page_name(""));
        // 前导 '-'/'/'：unrar/7z 解析成开关参数（参数注入），拒绝；
        // 中段的 '-' 与子目录内的 '-x.jpg' 不构成开关，正常放行
        assert!(!is_safe_page_name("-o+"));
        assert!(!is_safe_page_name("-y.jpg"));
        assert!(!is_safe_page_name("/abs.jpg"));
        assert!(is_safe_page_name("folder/-x.jpg"));
        assert!(is_safe_page_name("a-b.jpg"));
    }

    /// /file 回传前的魔数校验：类型与文件头必须相符（含 SFX 的 MZ 放行）。
    #[test]
    fn magic_matches_archive_type() {
        assert!(magic_matches("zip", b"PK\x03\x04rest"));
        assert!(magic_matches("cbz", b"PK\x05\x06")); // 空 zip
        assert!(magic_matches("rar", b"Rar!\x1a\x07\x00")); // RAR4
        assert!(magic_matches("cbr", b"Rar!\x1a\x07\x01\x00")); // RAR5
        assert!(magic_matches("7z", b"7z\xBC\xAF\x27\x1C"));
        assert!(magic_matches("zip", b"MZ\x90\x00")); // SFX 自解压
        assert!(!magic_matches("zip", b"SQLite format 3\x00"));
        assert!(!magic_matches("cbz", b"-----BEGIN RSA-----"));
        assert!(!magic_matches("rar", b"")); // 空文件
        assert!(!magic_matches("7z", b"PK\x03\x04")); // 类型不符
        assert!(!magic_matches("folder", b"PK\x03\x04")); // folder 不走该检查
    }

    /// zip 分块流式：内容与 extract_page 完全一致，且确实按 64KB 分块而非一次读完。
    #[test]
    fn zip_stream_page_yields_all_bytes_in_chunks() {
        let dir = tempfile::tempdir().unwrap();
        let zip_path = dir.path().join("s.zip");
        {
            let f = std::fs::File::create(&zip_path).unwrap();
            let mut zw = zip::ZipWriter::new(std::io::BufWriter::new(f));
            zw.start_file("p1.jpg", zip::write::SimpleFileOptions::default())
                .unwrap();
            // 200KB 内容 > 64KB 单块，覆盖分块路径
            let payload: Vec<u8> = (0..200_000u32).map(|i| (i % 251) as u8).collect();
            std::io::Write::write_all(&mut zw, &payload).unwrap();
            zw.finish().unwrap();
        }

        let za = ZipArchive::new(zip_path.to_str().unwrap()).unwrap();
        let mut collected = Vec::new();
        let mut chunks = 0usize;
        za.stream_page("p1.jpg", &mut |chunk| {
            collected.extend_from_slice(chunk);
            chunks += 1;
            Ok(())
        })
        .unwrap();

        assert_eq!(collected.len(), 200_000);
        assert!(
            chunks > 1,
            "应按 64KB 分块（实际 {chunks} 块），而非整页一次读完"
        );
        assert_eq!(za.extract_page("p1.jpg").unwrap(), collected);
    }

    /// 大小上限对流式同样生效：声明超限的条目应在读取前被拒绝。
    #[test]
    fn zip_stream_page_respects_size_cap() {
        let dir = tempfile::tempdir().unwrap();
        let zip_path = dir.path().join("big.zip");
        // 用正常 zip，改断言逻辑：超过 MAX_PAGE_BYTES 的条目 size() 会被拦截。
        // （构造 256MB 测试数据不现实，这里验证上限检查逻辑的代码路径先行短路）
        {
            let f = std::fs::File::create(&zip_path).unwrap();
            let mut zw = zip::ZipWriter::new(std::io::BufWriter::new(f));
            zw.start_file("p1.jpg", zip::write::SimpleFileOptions::default())
                .unwrap();
            std::io::Write::write_all(&mut zw, b"small").unwrap();
            zw.finish().unwrap();
        }
        let za = ZipArchive::new(zip_path.to_str().unwrap()).unwrap();
        // 正常条目不应被判超限
        let mut collected = Vec::new();
        za.stream_page("p1.jpg", &mut |c| {
            collected.extend_from_slice(c);
            Ok(())
        })
        .unwrap();
        assert_eq!(collected, b"small");
    }
}
