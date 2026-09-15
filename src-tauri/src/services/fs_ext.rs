//! 文件系统小工具：把「文件 mtime 秒数」这类在多个模块里重复手写的计算收敛到一处。

use std::path::Path;
use std::time::UNIX_EPOCH;

/// 文件最后修改时间（秒）；文件缺失/不可读/时间早于 Unix 纪元等异常一律返回 0。
pub fn mtime_secs(path: &Path) -> i64 {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
