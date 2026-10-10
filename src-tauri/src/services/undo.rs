//! 撤销删除的短期缓冲。
//!
//! 「从库中移除」删的是数据库里的行（磁盘源文件不动），但外键级联会一并带走标签、分类、
//! 书签与阅读进度——恰恰是用户最舍不得重建的部分。所以删除必须给一次撤销机会，而它的
//! 有效期就是界面上那条提示的寿命。
//!
//! 放内存而不是数据库：撤销是对"刚刚发生的事"的补救，进程重启后不该还能撤销几分钟前
//! 的误删（那时的界面提示早已消失，用户也不会再期待它有效）。

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use crate::db::archives::ArchiveSnapshot;

// 最多同时保留几批删除（一批 = 一次单删或一次批量删）
const MAX_ENTRIES: usize = 20;
// 一批撤销的有效期
const TTL: Duration = Duration::from_secs(10 * 60);

struct Entry {
    token: String,
    snapshots: Vec<ArchiveSnapshot>,
    at: Instant,
}

#[derive(Default)]
pub struct UndoBuffer {
    entries: VecDeque<Entry>,
    counter: u64,
}

impl UndoBuffer {
    pub fn new() -> Self {
        Self::default()
    }

    /// 记下一批已删除的档案，返回撤销令牌。空批次不入队。
    pub fn push(&mut self, snapshots: Vec<ArchiveSnapshot>) -> String {
        self.push_at(snapshots, Instant::now())
    }

    /// 取走令牌对应的快照。**一次性**：撤销过一次的令牌立即失效，
    /// 否则重复撤销会撞上已恢复的 UNIQUE(path) 或产生重复的书签/进度写入。
    pub fn take(&mut self, token: &str) -> Option<Vec<ArchiveSnapshot>> {
        self.take_at(token, Instant::now())
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn push_at(&mut self, snapshots: Vec<ArchiveSnapshot>, now: Instant) -> String {
        self.prune_at(now);
        self.counter += 1;
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let token = format!("u{}-{}", self.counter, nanos);
        if !snapshots.is_empty() {
            self.entries.push_back(Entry {
                token: token.clone(),
                snapshots,
                at: now,
            });
            while self.entries.len() > MAX_ENTRIES {
                self.entries.pop_front();
            }
        }
        token
    }

    fn take_at(&mut self, token: &str, now: Instant) -> Option<Vec<ArchiveSnapshot>> {
        self.prune_at(now);
        let idx = self.entries.iter().position(|e| e.token == token)?;
        self.entries.remove(idx).map(|e| e.snapshots)
    }

    /// 丢掉过期的批次（时间与数量两条边界）
    fn prune_at(&mut self, now: Instant) {
        while let Some(front) = self.entries.front() {
            if now.saturating_duration_since(front.at) > TTL {
                self.entries.pop_front();
            } else {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(id: i64) -> ArchiveSnapshot {
        ArchiveSnapshot {
            id,
            title: format!("书{id}"),
            path: format!("/x/书{id}.cbz"),
            archive_type: "cbz".into(),
            page_count: 1,
            cover_image: None,
            remote_cover: None,
            file_size: 0,
            group_id: None,
            page_list_mtime: 0,
            file_mtime: 0,
            title_auto: 1,
            last_read_at: None,
            created_at: None,
            updated_at: None,
            tag_ids: Vec::new(),
            category_ids: Vec::new(),
            bookmarks: Vec::new(),
            reading: None,
            group_member_ids: Vec::new(),
        }
    }

    #[test]
    fn push_then_take_returns_the_batch() {
        let mut buf = UndoBuffer::new();
        let token = buf.push(vec![snap(1), snap(2)]);
        let taken = buf.take(&token).expect("应能取回");
        assert_eq!(taken.len(), 2);
        assert_eq!(taken[0].id, 1);
    }

    #[test]
    fn token_is_single_use() {
        let mut buf = UndoBuffer::new();
        let token = buf.push(vec![snap(1)]);
        assert!(buf.take(&token).is_some());
        assert!(buf.take(&token).is_none(), "同一令牌不能撤销两次");
        assert!(buf.is_empty());
    }

    #[test]
    fn unknown_token_yields_none() {
        let mut buf = UndoBuffer::new();
        buf.push(vec![snap(1)]);
        assert!(buf.take("nope").is_none());
    }

    #[test]
    fn empty_batch_is_not_stored() {
        let mut buf = UndoBuffer::new();
        let token = buf.push(Vec::new());
        assert!(buf.is_empty());
        assert!(buf.take(&token).is_none());
    }

    #[test]
    fn expired_batches_are_dropped() {
        let mut buf = UndoBuffer::new();
        let t0 = Instant::now();
        let token = buf.push_at(vec![snap(1)], t0);
        // 还没到期：仍在
        assert!(buf
            .take_at(&token, t0 + TTL - Duration::from_secs(1))
            .is_some());

        let token2 = buf.push_at(vec![snap(2)], t0);
        assert!(buf
            .take_at(&token2, t0 + TTL + Duration::from_secs(1))
            .is_none());
        assert!(buf.is_empty(), "过期批次应被清掉");
    }

    #[test]
    fn oldest_entries_are_evicted_by_count() {
        let mut buf = UndoBuffer::new();
        let t0 = Instant::now();
        let first = buf.push_at(vec![snap(1)], t0);
        for i in 0..MAX_ENTRIES {
            buf.push_at(vec![snap(100 + i as i64)], t0);
        }
        assert_eq!(buf.len(), MAX_ENTRIES);
        assert!(buf.take_at(&first, t0).is_none(), "最旧的一批应被挤出");
    }
}
