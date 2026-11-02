//! 文件系统小工具：把「文件 mtime 秒数」这类在多个模块里重复手写的计算收敛到一处。

use std::path::Path;
use std::time::UNIX_EPOCH;

/// 从已经拿到的 `Metadata` 取 (大小, mtime 秒数)。
///
/// 扫描的增量判定同时要这两个数，用一次 stat 的 `Metadata` 就够了——分成两次
/// `fs::metadata()` 在慢盘/网络盘上会让每个档案多付一趟往返。
/// 时间早于 Unix 纪元等异常按 0 处理，与 `mtime_secs` 一致。
pub fn size_and_mtime_secs(m: &std::fs::Metadata) -> (i64, i64) {
    let mtime = m
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    (m.len() as i64, mtime)
}

/// 文件最后修改时间（秒）；文件缺失/不可读/时间早于 Unix 纪元等异常一律返回 0。
pub fn mtime_secs(path: &Path) -> i64 {
    std::fs::metadata(path)
        .map(|m| size_and_mtime_secs(&m).1)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::size_and_mtime_secs;

    #[test]
    fn size_and_mtime_are_read_from_one_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.bin");
        std::fs::write(&file, b"hello").unwrap();
        let meta = std::fs::metadata(&file).unwrap();
        let (size, mtime) = size_and_mtime_secs(&meta);
        assert_eq!(size, 5);
        assert!(mtime > 0, "mtime 应为 Unix 秒数，实际 {mtime}");
    }
}
