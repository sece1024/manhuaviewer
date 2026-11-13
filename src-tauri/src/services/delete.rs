//! 档案删除与撤销（`DELETE /api/archives/:id`、`/archives/batch-delete`、`/archives/undo-delete`）。
//!
//! 三处 HTTP 入口共用一个行为：「从库中移除，可选连同磁盘源文件一起删，默认留一次撤销机会」。
//! 此前单删与批量删各自复制了一遍缓存目录清理、源文件删除与令牌发放的逻辑，两者只在
//! 响应形状上不同；这里把整条流程收敛到一个模块，路由层只剩解析请求与拼响应。
//!
//! 关键不变量（与 AGENTS.md「Undo for destructive actions」一致）：
//! - 快照与删除在**同一个事务**里完成（见 `db::archives::delete_archives_with_snapshots`），
//!   否则读完快照、还没删时可能被别的写操作插进来。
//! - `delete_files = true` 时**不发放**撤销令牌：源文件已删，恢复只会得到一条指向空
//!   路径的记录，等于把用户骗回一个坏档。
//! - 整批删除只需**一个**令牌：界面提示是「已删除 N 个 / 撤销」，撤销即一次还原这一批。

use std::sync::Arc;

use crate::AppState;

/// 一次删除的结果（供路由层拼响应；单删与批量删共用，只是取用的字段不同）。
#[derive(Debug, Clone)]
pub struct RemovalOutcome {
    /// 实际删掉的档案数
    pub affected: usize,
    /// 撤销令牌；`delete_files` 为 true 时为 None（见模块注释）
    pub undo_token: Option<String>,
    pub deleted_files: bool,
}

/// 撤销结果：(恢复数, 跳过数)。
///
/// 跳过的是「路径在撤销窗口内已被重新扫描入库」的条目——插入旧 id 会撞 UNIQUE(path)，
/// 宁可跳过并如实告知，也不让整批撤销失败（详见 `db::archives::restore_archive_snapshots`）。
pub type RestoreCounts = (usize, usize);

/// 三个缓存子目录名（`<data_dir>/{thumbnails,page_thumbs,extract}/{id}`）。
/// 删除时一并清掉；命名约定集中在此，避免路由层重复拼路径。
const CACHE_SUBDIRS: &[&str] = &["thumbnails", "page_thumbs", "extract"];

/// 删除一批档案：DB 行（连同快照）、缓存目录、可选的磁盘源文件，并发放撤销令牌。
///
/// `ids` 为空时直接返回空结果（不碰 DB、不发放令牌）。
pub async fn remove_archives(
    state: &Arc<AppState>,
    ids: &[i64],
    delete_files: bool,
) -> Result<RemovalOutcome, rusqlite::Error> {
    if ids.is_empty() {
        return Ok(RemovalOutcome {
            affected: 0,
            undo_token: None,
            deleted_files: delete_files,
        });
    }

    // 删除与快照在同一个事务里完成。
    let ids_db = ids.to_vec();
    let snapshots =
        super::super::routes::run_db(state, move |db| db.delete_archives_with_snapshots(&ids_db))
            .await?;

    // 清理每个档案的封面 / 页面缩略图 / 解压缓存目录（失败不阻断删除）
    for id in ids {
        for sub in CACHE_SUBDIRS {
            let dir = state.data_dir.join(sub).join(id.to_string());
            let _ = tokio::fs::remove_dir_all(&dir).await;
        }
    }

    // 勾选了「同时删除原文件」：逐条删磁盘源文件（失败只影响文件本身，不阻断）。
    if delete_files {
        for s in &snapshots {
            delete_source_file(&s.path, &s.archive_type).await;
        }
    }

    let undo_token = if delete_files {
        None
    } else {
        Some(state.undo.lock().unwrap().push(snapshots.clone()))
    };

    Ok(RemovalOutcome {
        affected: snapshots.len(),
        undo_token,
        deleted_files: delete_files,
    })
}

/// 撤销一次删除：取走令牌（一次性）并把快照写回。
///
/// 返回 `None` 表示令牌无效/已过期/已在别处撤销；`Some((restored, skipped))` 为统计。
pub async fn undo(
    state: &Arc<AppState>,
    token: &str,
) -> Result<Option<RestoreCounts>, rusqlite::Error> {
    let Some(snapshots) = state.undo.lock().unwrap().take(token) else {
        return Ok(None);
    };
    super::super::routes::run_db(state, move |db| db.restore_archive_snapshots(&snapshots))
        .await
        .map(Some)
}

/// 删除磁盘源文件：folder 删整个目录，压缩包删单个文件。阻塞 I/O，必须 spawn_blocking。
///
/// 纯破坏性副作用：失败只进日志、由调用方忽略（删不掉不阻断「从库中移除」）。
async fn delete_source_file(path: &str, archive_type: &str) {
    let path = path.to_string();
    let is_folder = archive_type == "folder";
    let _ = tokio::task::spawn_blocking(move || -> std::io::Result<()> {
        if is_folder {
            std::fs::remove_dir_all(&path)
        } else {
            std::fs::remove_file(&path)
        }
    })
    .await;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 「同时删除原文件」：folder 删整个目录、压缩包删单个文件。
    #[tokio::test]
    async fn delete_source_file_removes_folder_and_file() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("book");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("p1.jpg"), b"img").unwrap();

        let file = dir.path().join("book.cbz");
        std::fs::write(&file, b"PK\x03\x04").unwrap();

        delete_source_file(folder.to_str().unwrap(), "folder").await;
        assert!(!folder.exists(), "folder 类型的源目录应被删除");

        delete_source_file(file.to_str().unwrap(), "cbz").await;
        assert!(!file.exists(), "压缩包源文件应被删除");
    }
}
