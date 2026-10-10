//! 档案（archives 表）相关的查询：档案 CRUD、扫描入库/批量 upsert、分组、
//! 压缩包页表缓存、缩略图路径登记与 LRU 淘汰。

use rusqlite::{OptionalExtension, Result};

use super::{
    archive_row, archive_row_with_remote_cover, log_and_skip, order_expr_for, path_is_within,
    ArchiveFilters, ArchiveRow, Database, GroupedArchiveRow, PageRow, ARCHIVE_COLUMNS,
};

/// 「有效总页数」SQL 表达式：优先取 `archives.page_count`（扫描会刷新，文件补页后
/// 不会把旧的阅读位置误判成「已读完」），为 0/NULL（文件夹尚未数页）时退回 history
/// 自己记录的总页数，两者都未知则算 0 —— 0 页既不算「在读」也不算「已读完」。
///
/// 必须只有这一处定义：「在读」与「已读完」合起来必须恰好等于「有阅读记录」，
/// 一旦两边的表达式漂移，筛选结果就会自相矛盾（既漏又重）。
const EFFECTIVE_TOTAL_PAGES: &str = "COALESCE(NULLIF(a.page_count, 0), hf.total_pages, 0)";

/// 阅读状态筛选对应的 SQL 片段（`read` 查询参数）。
///
/// - `unread`：history 无行，从未读过；
/// - `in_progress`：读过但还不到末页（总页数未知时也归这里，见下）；
/// - `finished`：读过且最后记录的页已是末页；
/// - `read`：有阅读记录（= 在读 + 已读完）。这是本功能的旧取值，保留原语义以免
///   旧客户端（含局域网上的网页端缓存）行为突变；
/// - 其余/缺省：不过滤。
///
/// 三个精确取值必须严格划分「有阅读记录」这一个集合：`unread` 取补集，
/// `in_progress` 取「不是已读完」，于是 `in_progress ∪ finished = read` 恒成立。
/// 总页数未知（`EFFECTIVE_TOTAL_PAGES = 0`）时判为「在读」而不是「已读完」：
/// 宁可让用户在一读的集合里多看到一个已看完的条目，也不能让它在三个筛选下全都消失。
///
/// 纯函数（不碰连接、不碰文件系统），便于直接断言生成的 SQL 语义。
fn read_filter_clause(read: Option<&str>) -> String {
    let total = EFFECTIVE_TOTAL_PAGES;
    match read {
        Some("read") => {
            " AND EXISTS (SELECT 1 FROM history hf WHERE hf.archive_id = a.id)".to_string()
        }
        Some("unread") => {
            " AND NOT EXISTS (SELECT 1 FROM history hf WHERE hf.archive_id = a.id)".to_string()
        }
        Some("in_progress") => format!(
            " AND EXISTS (SELECT 1 FROM history hf WHERE hf.archive_id = a.id \
             AND ({total} <= 0 OR hf.page_index + 1 < {total}))"
        ),
        Some("finished") => format!(
            " AND EXISTS (SELECT 1 FROM history hf WHERE hf.archive_id = a.id \
             AND {total} > 0 AND hf.page_index + 1 >= {total})"
        ),
        _ => String::new(),
    }
}

/// 「标签状态」筛选对应的 SQL 片段（`tag_state` 查询参数）。
///
/// - `untagged`：一个标签都没有——书库里"还没整理过"的那批，也是整理模式的输入；
/// - `tagged`：至少有一个标签；
/// - 其余/缺省：不过滤。
///
/// 与搜索语法、标签过滤用的子查询别名（`atx`/`at_f`）区分开，避免同一条 WHERE 里
/// 多个标签子查询互相串味。
fn tag_state_clause(state: Option<&str>) -> &'static str {
    match state {
        Some("untagged") => {
            " AND NOT EXISTS (SELECT 1 FROM archive_tags atg WHERE atg.archive_id = a.id)"
        }
        Some("tagged") => {
            " AND EXISTS (SELECT 1 FROM archive_tags atg WHERE atg.archive_id = a.id)"
        }
        _ => "",
    }
}

/// 删除前的档案快照：撤销删除要把这一行连同它的标签/分类/书签/阅读进度**原样**放回去。
///
/// 为什么复用备份通道（`import_backup`）做不到这件事：备份以 path 为键，只恢复档案的
/// 四个字段与关联，用途是"导入到另一台机器"；而撤销必须精确——要还原原 id
/// （`archives.id` 是 AUTOINCREMENT，id 不会被复用，所以安全）、原始 created_at
/// （否则"添加时间"与侧栏日期树都会变）、手动封面 / 远程封面，以及合并组的归属。
///
/// 刻意不记 pages：压缩包的页面清单由 `page_cache::load_page_rows` 在缓存失效或为空时
/// 自动重建（见该函数），撤销时不需要把几百行页记录一起搬回去。
/// 也不记 thumbnail_path / thumb_accessed_at：封面缓存目录在删除时已被清掉，
/// 恢复时留空让缩略图按需重新生成，而不是指向一个不存在的目录。
#[derive(Debug, Clone)]
pub struct ArchiveSnapshot {
    pub id: i64,
    pub title: String,
    pub path: String,
    pub archive_type: String,
    pub page_count: i64,
    pub cover_image: Option<String>,
    pub remote_cover: Option<String>,
    pub file_size: i64,
    pub group_id: Option<i64>,
    pub page_list_mtime: i64,
    pub file_mtime: i64,
    pub title_auto: i64,
    pub last_read_at: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub tag_ids: Vec<i64>,
    pub category_ids: Vec<i64>,
    /// (page_index, created_at)
    pub bookmarks: Vec<(i64, Option<String>)>,
    /// (page_index, total_pages, updated_at)
    pub reading: Option<(i64, i64, Option<String>)>,
    /// 被删档案是合并组主档案时，成员的 group_id 会被 `ON DELETE SET NULL` 清掉；
    /// 记下这些成员，撤销时把归属还回去（否则合并组会被悄悄拆散）。
    pub group_member_ids: Vec<i64>,
}

impl Database {
    pub fn get_archive(&self, id: i64) -> Result<Option<ArchiveRow>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, title, path, archive_type, page_count, cover_image, file_size, thumbnail_path, group_id, created_at, updated_at FROM archives WHERE id = ?"
        )?;
        let mut rows = stmt.query_map([id], archive_row)?;

        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    /// 一次查询取回档案行与其远程封面 URL，避免封面请求两次往返 DB。
    pub fn get_archive_with_remote_cover(
        &self,
        id: i64,
    ) -> Result<Option<(ArchiveRow, Option<String>)>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, title, path, archive_type, page_count, cover_image, file_size, thumbnail_path, group_id, created_at, updated_at, remote_cover FROM archives WHERE id = ?"
        )?;
        let mut rows = stmt.query_map([id], archive_row_with_remote_cover)?;

        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    pub fn get_archive_by_path(&self, path: &str) -> Result<Option<ArchiveRow>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, title, path, archive_type, page_count, cover_image, file_size, thumbnail_path, group_id, created_at, updated_at FROM archives WHERE path = ?"
        )?;

        let mut rows = stmt.query_map([path], archive_row)?;

        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    /// 可批量转换为 CBZ 的档案：压缩包类型中非 cbz 的（zip/rar/cbr/7z）。
    pub fn list_convertible_archives(&self) -> Result<Vec<ArchiveRow>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, title, path, archive_type, page_count, cover_image, file_size, thumbnail_path, group_id, created_at, updated_at
             FROM archives
             WHERE archive_type IN ('zip', 'rar', 'cbr', '7z')
             ORDER BY id",
        )?;
        let rows = stmt
            .query_map([], archive_row)?
            .filter_map(log_and_skip)
            .collect();
        Ok(rows)
    }

    /// 指定 id 中可转换为 CBZ 的档案（供「仅转换选中项」使用）。
    pub fn list_convertible_archives_by_ids(&self, ids: &[i64]) -> Result<Vec<ArchiveRow>> {
        if ids.is_empty() {
            return Ok(vec![]);
        }
        let conn = self.conn()?;
        let placeholders = vec!["?"; ids.len()].join(",");
        let mut stmt = conn.prepare(&format!(
            "SELECT id, title, path, archive_type, page_count, cover_image, file_size, thumbnail_path, group_id, created_at, updated_at
             FROM archives
             WHERE archive_type IN ('zip', 'rar', 'cbr', '7z') AND id IN ({placeholders})
             ORDER BY id"
        ))?;
        let rows = stmt
            .query_map(rusqlite::params_from_iter(ids.iter()), archive_row)?
            .filter_map(log_and_skip)
            .collect();
        Ok(rows)
    }

    /// 格式转换成功后就地更新档案（保留 id，标签/历史/书签/分类/分组均不受影响）：
    /// 更新 path 与类型为 cbz、页数/大小/mtime，并清空封面缓存登记（下次访问重新生成）。
    pub fn update_archive_converted(
        &self,
        id: i64,
        new_path: &str,
        page_count: i64,
        file_size: i64,
        file_mtime: i64,
    ) -> Result<usize> {
        self.conn()?.execute(
            "UPDATE archives SET path = ?1, archive_type = 'cbz', page_count = ?2,
                file_size = ?3, file_mtime = ?4, thumbnail_path = NULL,
                updated_at = datetime('now')
             WHERE id = ?5",
            (new_path, page_count, file_size, file_mtime, id),
        )
    }

    // Page list cache operations (compressed archives only)
    pub fn get_page_list_mtime(&self, archive_id: i64) -> Result<Option<i64>> {
        self.conn()?
            .query_row(
                "SELECT page_list_mtime FROM archives WHERE id = ?",
                [archive_id],
                |row| row.get(0),
            )
            .optional()
    }

    pub fn get_pages(&self, archive_id: i64) -> Result<Vec<PageRow>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, archive_id, filename, filepath, sort_order
             FROM pages WHERE archive_id = ? ORDER BY sort_order",
        )?;
        let pages = stmt
            .query_map([archive_id], |row| {
                Ok(PageRow {
                    id: row.get(0)?,
                    archive_id: row.get(1)?,
                    filename: row.get(2)?,
                    filepath: row.get(3)?,
                    sort_order: row.get(4)?,
                })
            })?
            .filter_map(log_and_skip)
            .collect();
        Ok(pages)
    }

    /// Replace the cached page list for an archive and record the archive file
    /// mtime used to build it (used to detect staleness on later requests).
    pub fn save_pages(&self, archive_id: i64, pages: &[PageRow], mtime_secs: i64) -> Result<()> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM pages WHERE archive_id = ?", [archive_id])?;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO pages (archive_id, filename, filepath, sort_order) VALUES (?, ?, ?, ?)",
            )?;
            for p in pages {
                stmt.execute((archive_id, &p.filename, &p.filepath, p.sort_order))?;
            }
        }
        tx.execute(
            "UPDATE archives SET page_list_mtime = ? WHERE id = ?",
            (mtime_secs, archive_id),
        )?;
        tx.commit()?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn list_archives(
        &self,
        search: Option<&str>,
        tag: Option<&str>,
        category_id: Option<i64>,
        added_from: Option<&str>,
        added_to: Option<&str>,
        sort: &str,
        order: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<ArchiveRow>> {
        let conn = self.conn()?;
        let (join_clause, where_clause, mut params) =
            Self::build_archive_filters(&conn, search, tag, category_id, added_from, added_to)?;

        let order_clause = order_expr_for(sort);
        let direction = if order == "asc" { "ASC" } else { "DESC" };

        let sql = format!(
            "SELECT {} FROM archives a {} {} ORDER BY {} {} LIMIT ? OFFSET ?",
            ARCHIVE_COLUMNS, join_clause, where_clause, order_clause, direction
        );

        params.push(Box::new(limit));
        params.push(Box::new(offset));

        let mut stmt = conn.prepare(&sql)?;
        let archives = stmt
            .query_map(
                rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())),
                archive_row,
            )?
            .filter_map(log_and_skip)
            .collect();

        Ok(archives)
    }

    /// 拉取所有符合过滤条件的档案（不分页），供服务端分组后统一分页。
    /// `read`: None=全部, "unread"=从未读过, "in_progress"=读到一半, "finished"=已读完,
    /// "read"=有阅读记录（在读 + 已读完）。详见 [`read_filter_clause`]。
    /// `tag_state`: None=全部, "untagged"=未打标签, "tagged"=已打标签。详见 [`tag_state_clause`]。
    #[allow(clippy::too_many_arguments)]
    pub fn list_archives_all(
        &self,
        search: Option<&str>,
        tag: Option<&str>,
        category_id: Option<i64>,
        added_from: Option<&str>,
        added_to: Option<&str>,
        read: Option<&str>,
        tag_state: Option<&str>,
        sort: &str,
        order: &str,
    ) -> Result<Vec<ArchiveRow>> {
        let conn = self.conn()?;
        let (join_clause, mut where_clause, params) =
            Self::build_archive_filters(&conn, search, tag, category_id, added_from, added_to)?;

        where_clause.push_str(&read_filter_clause(read));
        where_clause.push_str(tag_state_clause(tag_state));

        let order_clause = order_expr_for(sort);
        let direction = if order == "asc" { "ASC" } else { "DESC" };

        let sql = format!(
            "SELECT {} FROM archives a {} {} ORDER BY {} {}",
            ARCHIVE_COLUMNS, join_clause, where_clause, order_clause, direction
        );

        let mut stmt = conn.prepare(&sql)?;
        let archives = stmt
            .query_map(
                rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())),
                archive_row,
            )?
            .filter_map(log_and_skip)
            .collect();

        Ok(archives)
    }

    /// 服务端分组 + 分页：在一次 SQL 查询内按（永久组 `group_id` / 自动组「同父目录 + 同标题」）
    /// 分组，取每组代表行与成员数，再对「组」排序并 `LIMIT/OFFSET`。
    ///
    /// 分组键由 SQL 标量函数 `archive_group_key` 计算（见 `db::mod`），与路由层
    /// `group_archives` 的规则一致：永久组的代表行取 `id == group_id` 的成员，自动组取
    /// 排序最靠前的成员；组在列表中的位置取组内成员的极值（升序 MIN / 降序 MAX），
    /// 保持「按首条成员位置」的既有语义。
    ///
    /// 调用方需保证 `sort != "random"`（随机排序依赖路由层的稳定洗牌，单独处理）。
    #[allow(clippy::too_many_arguments)]
    pub fn list_archives_grouped_page(
        &self,
        search: Option<&str>,
        tag: Option<&str>,
        category_id: Option<i64>,
        added_from: Option<&str>,
        added_to: Option<&str>,
        read: Option<&str>,
        tag_state: Option<&str>,
        sort: &str,
        order: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<GroupedArchiveRow>> {
        let conn = self.conn()?;
        let (join_clause, mut where_clause, mut params) =
            Self::build_archive_filters(&conn, search, tag, category_id, added_from, added_to)?;

        where_clause.push_str(&read_filter_clause(read));
        where_clause.push_str(tag_state_clause(tag_state));

        let sort_expr = order_expr_for(sort);
        let direction = if order == "asc" { "ASC" } else { "DESC" };
        // 组位置：升序用组内最小排序值、降序用最大，等价于「首条成员的位置」
        let group_agg = if order == "asc" { "MIN" } else { "MAX" };

        let sql = format!(
            "WITH base AS (
                 SELECT {cols},
                        archive_group_key(a.path, a.title, a.group_id) AS gkey,
                        {sort_expr} AS sortkey
                 FROM archives a {join} {where}
             ),
             ranked AS (
                 SELECT *,
                     ROW_NUMBER() OVER (
                         PARTITION BY gkey
                         ORDER BY (CASE WHEN group_id IS NOT NULL AND id = group_id THEN 0 ELSE 1 END),
                                  sortkey {dir}
                     ) AS rn,
                     COUNT(*) OVER (PARTITION BY gkey) AS cnt,
                     {agg}(sortkey) OVER (PARTITION BY gkey) AS group_sort
                 FROM base
             )
             SELECT id, title, path, archive_type, page_count, cover_image, file_size,
                    thumbnail_path, group_id, created_at, updated_at, cnt
             FROM ranked
             WHERE rn = 1
             ORDER BY group_sort {dir}, gkey
             LIMIT ? OFFSET ?",
            cols = ARCHIVE_COLUMNS,
            join = join_clause,
            where = where_clause,
            sort_expr = sort_expr,
            dir = direction,
            agg = group_agg,
        );

        params.push(Box::new(limit));
        params.push(Box::new(offset));

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt
            .query_map(
                rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())),
                |row| {
                    Ok(GroupedArchiveRow {
                        archive: archive_row(row)?,
                        chapter_count: row.get(11)?,
                    })
                },
            )?
            .filter_map(log_and_skip)
            .collect();

        Ok(rows)
    }

    /// 构造档案列表查询的 JOIN / WHERE 片段与参数（供 list_archives / list_archives_all /
    /// list_archives_grouped_page 共用）。
    /// `added_from`/`added_to`：按添加日期过滤的本地日期边界（from 含、to 不含）。
    fn build_archive_filters(
        conn: &rusqlite::Connection,
        search: Option<&str>,
        tag: Option<&str>,
        category_id: Option<i64>,
        added_from: Option<&str>,
        added_to: Option<&str>,
    ) -> Result<ArchiveFilters> {
        let mut where_clause = String::from("WHERE 1=1");
        let mut join_clause = String::new();
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if let Some(s) = search {
            if !s.is_empty() {
                // 解析搜索语法：tag:xxx（标签名或 ns:name）、-xxx（排除）、普通关键词（标题或标签名）
                for token in s.split_whitespace() {
                    if token.is_empty() {
                        continue;
                    }
                    if let Some(spec) = token.strip_prefix("tag:") {
                        let (ns, name) = match spec.split_once(':') {
                            Some((ns, name)) => (Some(ns), name),
                            None => (None, spec),
                        };
                        let mut cond = String::from(
                            "EXISTS (SELECT 1 FROM archive_tags atx JOIN tags tx ON tx.id = atx.tag_id WHERE atx.archive_id = a.id",
                        );
                        if let Some(ns) = ns {
                            cond.push_str(" AND tx.namespace = ?");
                            params.push(Box::new(ns.to_string()));
                        }
                        cond.push_str(" AND tx.name = ?)");
                        params.push(Box::new(name.to_string()));
                        where_clause.push_str(" AND ");
                        where_clause.push_str(&cond);
                    } else if let Some(ex) = token.strip_prefix('-') {
                        if !ex.is_empty() {
                            let pattern = format!("%{}%", ex);
                            where_clause.push_str(
                                " AND a.title NOT LIKE ? AND NOT EXISTS (SELECT 1 FROM archive_tags atx JOIN tags tx ON tx.id = atx.tag_id WHERE atx.archive_id = a.id AND tx.name LIKE ?)",
                            );
                            params.push(Box::new(pattern.clone()));
                            params.push(Box::new(pattern));
                        }
                    } else {
                        let pattern = format!("%{}%", token);
                        where_clause.push_str(
                            " AND (a.title LIKE ? OR EXISTS (SELECT 1 FROM archive_tags atx JOIN tags tx ON tx.id = atx.tag_id WHERE atx.archive_id = a.id AND tx.name LIKE ?))",
                        );
                        params.push(Box::new(pattern.clone()));
                        params.push(Box::new(pattern));
                    }
                }
            }
        }

        // 按标签过滤：支持 "namespace:name" 或 "name" 格式
        if let Some(t) = tag {
            if !t.is_empty() {
                join_clause.push_str(
                    " JOIN archive_tags at_f ON at_f.archive_id = a.id JOIN tags t_f ON t_f.id = at_f.tag_id",
                );
                if let Some((ns, name)) = t.split_once(':') {
                    where_clause.push_str(" AND t_f.namespace = ? AND t_f.name = ?");
                    params.push(Box::new(ns.to_string()));
                    params.push(Box::new(name.to_string()));
                } else {
                    where_clause.push_str(" AND t_f.name = ? AND t_f.namespace = ''");
                    params.push(Box::new(t.to_string()));
                }
            }
        }

        // 按分类过滤：静态分类走关联表 JOIN，动态分类（配置了 search）走标题匹配
        if let Some(cid) = category_id {
            let dynamic_search: Option<String> = conn
                .query_row("SELECT search FROM categories WHERE id = ?", [cid], |row| {
                    row.get(0)
                })
                .ok();
            match dynamic_search {
                Some(s) if !s.is_empty() => {
                    where_clause.push_str(" AND a.title LIKE ?");
                    params.push(Box::new(format!("%{}%", s)));
                }
                _ => {
                    join_clause.push_str(" JOIN archive_categories ac_f ON ac_f.archive_id = a.id");
                    where_clause.push_str(" AND ac_f.category_id = ?");
                    params.push(Box::new(cid));
                }
            }
        }

        // 按添加日期过滤：本地日期边界（from 含、to 不含）。created_at 存的是 UTC
        // 无时区标记串，用 SQLite 'utc' 修饰符把本地日期换算成 UTC 后做**裸列**范围
        // 比较——左值不包函数，仍能走 idx_archives_created_at 索引。
        if let Some(from) = added_from.filter(|s| !s.is_empty()) {
            where_clause.push_str(" AND a.created_at >= datetime(?, 'utc')");
            params.push(Box::new(from.to_string()));
        }
        if let Some(to) = added_to.filter(|s| !s.is_empty()) {
            where_clause.push_str(" AND a.created_at < datetime(?, 'utc')");
            params.push(Box::new(to.to_string()));
        }

        Ok((join_clause, where_clause, params))
    }

    pub fn list_archives_by_tag(
        &self,
        tag_id: i64,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<ArchiveRow>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT a.id, a.title, a.path, a.archive_type, a.page_count, a.cover_image, a.file_size, a.thumbnail_path, a.group_id, a.created_at, a.updated_at
             FROM archives a
             JOIN archive_tags at ON at.archive_id = a.id
             WHERE at.tag_id = ?
             ORDER BY a.updated_at DESC
             LIMIT ? OFFSET ?",
        )?;

        let archives = stmt
            .query_map(rusqlite::params![tag_id, limit, offset], archive_row)?
            .filter_map(log_and_skip)
            .collect();

        Ok(archives)
    }

    pub fn insert_archive(
        &self,
        title: &str,
        path: &str,
        archive_type: &str,
        page_count: i64,
        file_size: i64,
    ) -> Result<i64> {
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO archives (title, path, archive_type, page_count, file_size)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(path) DO NOTHING",
            (title, path, archive_type, page_count, file_size),
        )?;
        // ON CONFLICT DO NOTHING 吞掉并发重复插入，随后按 path 取回 id（可能已存在）。
        let id = conn.query_row("SELECT id FROM archives WHERE path = ?", [path], |row| {
            row.get::<_, i64>(0)
        })?;
        Ok(id)
    }

    pub fn delete_archive(&self, id: i64) -> Result<usize> {
        self.conn()?
            .execute("DELETE FROM archives WHERE id = ?", [id])
    }

    /// 增量扫描入库：按 path upsert（更新标题/类型/页数/大小/file_mtime），
    /// 不使用 INSERT OR REPLACE，避免级联删除 history 与标签/分类关联。返回该档案 id。
    pub fn upsert_scanned_archive(
        &self,
        title: &str,
        path: &str,
        archive_type: &str,
        page_count: i64,
        file_size: i64,
        file_mtime: i64,
    ) -> Result<i64> {
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO archives (title, path, archive_type, page_count, file_size, file_mtime, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, datetime('now'))
             ON CONFLICT(path) DO UPDATE SET
                title = excluded.title,
                archive_type = excluded.archive_type,
                page_count = excluded.page_count,
                file_size = excluded.file_size,
                file_mtime = excluded.file_mtime,
                updated_at = excluded.updated_at",
            (title, path, archive_type, page_count, file_size, file_mtime),
        )?;
        let id = conn.query_row("SELECT id FROM archives WHERE path = ?", [path], |row| {
            row.get::<_, i64>(0)
        })?;
        Ok(id)
    }

    /// 批量 upsert 扫描结果：单连接 + 单事务内完成全部写入，避免每次 upsert
    /// 都单独获取连接并在 `SELECT id` 上往返。`entries` 为 (title, path, archive_type,
    /// page_count, file_size, file_mtime)。
    pub fn batch_upsert_scanned_archives(
        &self,
        entries: &[(String, String, String, i64, i64, i64)],
    ) -> Result<()> {
        if entries.is_empty() {
            return Ok(());
        }
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        {
            let mut stmt = tx.prepare_cached(
                "INSERT INTO archives (title, path, archive_type, page_count, file_size, file_mtime, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, datetime('now'))
                 ON CONFLICT(path) DO UPDATE SET
                    title = excluded.title,
                    archive_type = excluded.archive_type,
                    page_count = excluded.page_count,
                    file_size = excluded.file_size,
                    file_mtime = excluded.file_mtime,
                    updated_at = excluded.updated_at",
            )?;
            for (title, path, archive_type, page_count, file_size, file_mtime) in entries {
                stmt.execute((
                    title.as_str(),
                    path.as_str(),
                    archive_type.as_str(),
                    *page_count,
                    *file_size,
                    *file_mtime,
                ))?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// 读取某根目录下所有档案的扫描元数据快照：(path, page_count, file_size, file_mtime)。
    pub fn scan_meta_for_root(&self, root: &str) -> Result<Vec<(String, i64, i64, i64)>> {
        let conn = self.conn()?;
        let mut stmt =
            conn.prepare("SELECT path, page_count, file_size, file_mtime FROM archives")?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })?;
        Ok(rows
            .filter_map(log_and_skip)
            .filter(|(path, _, _, _)| path_is_within(root, path))
            .collect())
    }

    /// 按 path 删除档案（供扫描清理孤儿档案；级联删除 pages/history/标签分类关联）。
    pub fn delete_archive_by_path(&self, path: &str) -> Result<usize> {
        self.conn()?
            .execute("DELETE FROM archives WHERE path = ?", [path])
    }

    /// 批量删除档案，单事务执行
    pub fn batch_delete_archives(&self, ids: &[i64]) -> Result<usize> {
        if ids.is_empty() {
            return Ok(0);
        }
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        let mut affected = 0;
        for &id in ids {
            affected += tx.execute("DELETE FROM archives WHERE id = ?", [id])?;
        }
        tx.commit()?;
        Ok(affected)
    }

    /// 在同一个事务里读取快照并删除单个档案（读与删之间不能有其它写操作插入）。
    pub fn delete_archive_with_snapshot(&self, id: i64) -> Result<Option<ArchiveSnapshot>> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        let snapshot = Self::snapshot_archive(&tx, id)?;
        if snapshot.is_none() {
            return Ok(None);
        }
        tx.execute("DELETE FROM archives WHERE id = ?", [id])?;
        tx.commit()?;
        Ok(snapshot)
    }

    /// 批量删除：整体一个事务，返回每个成功取到快照并删除的档案（顺序与 ids 一致）。
    pub fn delete_archives_with_snapshots(&self, ids: &[i64]) -> Result<Vec<ArchiveSnapshot>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        let mut snapshots = Vec::new();
        for &id in ids {
            if let Some(snapshot) = Self::snapshot_archive(&tx, id)? {
                tx.execute("DELETE FROM archives WHERE id = ?", [id])?;
                snapshots.push(snapshot);
            }
        }
        tx.commit()?;
        Ok(snapshots)
    }

    /// 撤销删除：把快照原样写回，单事务。
    ///
    /// 返回 (恢复数, 跳过数)。跳过的是"路径已被重新入库"的条目——删除后若恰好跑过扫描，
    /// 同一路径会带着新 id 回到库里，此时插入旧 id 会撞 UNIQUE(path)，整批撤销就会失败。
    /// 宁可跳过并如实告诉用户，也不要让撤销整体失败。
    pub fn restore_archive_snapshots(
        &self,
        snapshots: &[ArchiveSnapshot],
    ) -> Result<(usize, usize)> {
        if snapshots.is_empty() {
            return Ok((0, 0));
        }
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        let mut restored = 0;
        let mut skipped = 0;

        for snap in snapshots {
            let existing: Option<i64> = tx
                .query_row(
                    "SELECT id FROM archives WHERE path = ?",
                    [&snap.path],
                    |row| row.get(0),
                )
                .optional()?;
            if existing.is_some() {
                skipped += 1;
                continue;
            }

            tx.execute(
                "INSERT INTO archives (
                     id, title, path, archive_type, page_count, cover_image, remote_cover,
                     file_size, group_id, page_list_mtime, file_mtime, title_auto,
                     last_read_at, created_at, updated_at
                 ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                rusqlite::params![
                    snap.id,
                    snap.title,
                    snap.path,
                    snap.archive_type,
                    snap.page_count,
                    snap.cover_image,
                    snap.remote_cover,
                    snap.file_size,
                    snap.group_id,
                    snap.page_list_mtime,
                    snap.file_mtime,
                    snap.title_auto,
                    snap.last_read_at,
                    snap.created_at,
                    snap.updated_at,
                ],
            )?;

            for tag_id in &snap.tag_ids {
                tx.execute(
                    "INSERT OR IGNORE INTO archive_tags (archive_id, tag_id) VALUES (?, ?)",
                    (snap.id, tag_id),
                )?;
            }
            for category_id in &snap.category_ids {
                tx.execute(
                    "INSERT OR IGNORE INTO archive_categories (archive_id, category_id) VALUES (?, ?)",
                    (snap.id, category_id),
                )?;
            }
            for (page_index, created_at) in &snap.bookmarks {
                tx.execute(
                    "INSERT OR IGNORE INTO bookmarks (archive_id, page_index, created_at) VALUES (?, ?, ?)",
                    rusqlite::params![snap.id, page_index, created_at],
                )?;
            }
            if let Some((page_index, total_pages, updated_at)) = &snap.reading {
                tx.execute(
                    "INSERT OR REPLACE INTO history (archive_id, page_index, total_pages, updated_at)
                     VALUES (?, ?, ?, ?)",
                    rusqlite::params![snap.id, page_index, total_pages, updated_at],
                )?;
            }
            // 合并组归属：这些成员在删除主档案时被 SET NULL，现在恢复
            for member_id in &snap.group_member_ids {
                tx.execute(
                    "UPDATE archives SET group_id = ? WHERE id = ? AND group_id IS NULL",
                    (snap.id, member_id),
                )?;
            }
            restored += 1;
        }

        tx.commit()?;
        Ok((restored, skipped))
    }

    /// 读取一个档案的完整快照（不含 pages 与缩略图缓存字段，见 [`ArchiveSnapshot`]）。
    fn snapshot_archive(
        tx: &rusqlite::Transaction<'_>,
        id: i64,
    ) -> Result<Option<ArchiveSnapshot>> {
        let archive = tx
            .query_row(
                "SELECT id, title, path, archive_type, page_count, cover_image, remote_cover,
                        file_size, group_id, page_list_mtime, file_mtime, title_auto,
                        last_read_at, created_at, updated_at
                 FROM archives WHERE id = ?",
                [id],
                |row| {
                    Ok(ArchiveSnapshot {
                        id: row.get(0)?,
                        title: row.get(1)?,
                        path: row.get(2)?,
                        archive_type: row.get(3)?,
                        page_count: row.get(4)?,
                        cover_image: row.get(5)?,
                        remote_cover: row.get(6)?,
                        file_size: row.get(7)?,
                        group_id: row.get(8)?,
                        page_list_mtime: row.get(9)?,
                        file_mtime: row.get(10)?,
                        title_auto: row.get(11)?,
                        last_read_at: row.get(12)?,
                        created_at: row.get(13)?,
                        updated_at: row.get(14)?,
                        tag_ids: Vec::new(),
                        category_ids: Vec::new(),
                        bookmarks: Vec::new(),
                        reading: None,
                        group_member_ids: Vec::new(),
                    })
                },
            )
            .optional()?;
        let Some(mut snap) = archive else {
            return Ok(None);
        };

        let mut stmt = tx.prepare("SELECT tag_id FROM archive_tags WHERE archive_id = ?")?;
        snap.tag_ids = stmt
            .query_map([id], |row| row.get(0))?
            .filter_map(log_and_skip)
            .collect();
        drop(stmt);

        let mut stmt =
            tx.prepare("SELECT category_id FROM archive_categories WHERE archive_id = ?")?;
        snap.category_ids = stmt
            .query_map([id], |row| row.get(0))?
            .filter_map(log_and_skip)
            .collect();
        drop(stmt);

        let mut stmt = tx.prepare(
            "SELECT page_index, created_at FROM bookmarks WHERE archive_id = ? ORDER BY page_index",
        )?;
        snap.bookmarks = stmt
            .query_map([id], |row| Ok((row.get(0)?, row.get(1)?)))?
            .filter_map(log_and_skip)
            .collect();
        drop(stmt);

        snap.reading = tx
            .query_row(
                "SELECT page_index, total_pages, updated_at FROM history WHERE archive_id = ?",
                [id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;

        // 只有主档案（group_id = 自身 id）才需要记成员：删它会把成员的 group_id 置 NULL
        if snap.group_id == Some(id) {
            let mut stmt = tx.prepare("SELECT id FROM archives WHERE group_id = ?")?;
            snap.group_member_ids = stmt
                .query_map([id], |row| row.get(0))?
                .filter_map(log_and_skip)
                .collect();
        }

        Ok(Some(snap))
    }

    pub fn update_archive_title(&self, id: i64, title: &str) -> Result<usize> {
        self.conn()?.execute(            "UPDATE archives SET title = ?, title_auto = 0, updated_at = datetime('now') WHERE id = ?",
            (title, id),
        )
    }

    /// 列出需要按「初始标题层级」重生成的档案（自动派生标题且未被手动改名）
    pub fn list_auto_titled(&self) -> Result<Vec<(i64, String)>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare("SELECT id, path FROM archives WHERE title_auto = 1")?;
        let rows = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .filter_map(log_and_skip)
            .collect();
        Ok(rows)
    }

    /// 更新自动派生标题（保持 title_auto = 1）；title 未变化时不更新，返回是否变更。
    pub fn update_title_auto(&self, id: i64, title: &str) -> Result<bool> {
        let affected = self.conn()?.execute(
            "UPDATE archives SET title = ?, updated_at = datetime('now') WHERE id = ? AND title != ?",
            (title, id, title),
        )?;
        Ok(affected > 0)
    }

    /// 批量重生成自动标题：单连接 + 单事务 + 单条 prepared 语句，返回实际变更数。
    pub fn update_titles_auto(&self, entries: &[(i64, String)]) -> Result<usize> {
        if entries.is_empty() {
            return Ok(0);
        }
        let conn = self.conn()?;
        let tx = conn.unchecked_transaction()?;
        let mut changed = 0usize;
        {
            let mut stmt = tx.prepare_cached(
                "UPDATE archives SET title = ?1, updated_at = datetime('now') WHERE id = ?2 AND title != ?1",
            )?;
            for (id, title) in entries {
                changed += stmt.execute((title, id))?;
            }
        }
        tx.commit()?;
        Ok(changed)
    }

    /// 获取组内所有章节（按路径排序）
    pub fn get_group_chapters(&self, group_id: i64) -> Result<Vec<ArchiveRow>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, title, path, archive_type, page_count, cover_image, file_size, thumbnail_path, group_id, created_at, updated_at
             FROM archives WHERE group_id = ? ORDER BY path",
        )?;

        let archives = stmt
            .query_map([group_id], archive_row)?
            .filter_map(log_and_skip)
            .collect();

        Ok(archives)
    }

    /// 与给定档案**同父目录**的其他档案（含自己），按路径排序。
    ///
    /// 与永久合并组（`group_id`）不同：合并组可以跨目录，而真实系列的标题通常是
    /// 「xxx 01 / xxx 02 / xxx 03」，标题并不相同，书库的自动分组（同目录 + 同标题）
    /// 也并不到一起。阅读器要在末页续到下一卷，只能先由服务端把范围收窄到同目录，
    /// 再由前端做标题前缀 + 自然序的保守挑选。
    ///
    /// 这里刻意不引入任何标题猜测：判错的代价只应是「不显示下一卷按钮」，
    /// 绝不该是自动跳到一本无关的书，所以启发式只放在前端且仅用于提示。
    ///
    /// 目录比对的归一化（去掉尾部分隔符）与路由层 `parent` 过滤完全一致。
    pub fn get_siblings_in_dir(&self, id: i64) -> Result<Vec<ArchiveRow>> {
        let conn = self.conn()?;
        let path: Option<String> = conn
            .query_row("SELECT path FROM archives WHERE id = ?", [id], |row| {
                row.get(0)
            })
            .optional()?;
        let Some(path) = path else {
            return Ok(Vec::new());
        };
        let Some(parent) = std::path::Path::new(&path).parent() else {
            return Ok(Vec::new());
        };
        let parent = parent
            .to_string_lossy()
            .trim_end_matches(['/', '\\'])
            .to_string();

        let mut stmt = conn.prepare(&format!(
            "SELECT {} FROM archives a ORDER BY a.path",
            ARCHIVE_COLUMNS
        ))?;
        let archives: Vec<ArchiveRow> = stmt
            .query_map([], archive_row)?
            .filter_map(log_and_skip)
            .collect();

        Ok(archives
            .into_iter()
            .filter(|a| {
                std::path::Path::new(&a.path)
                    .parent()
                    .map(|p| p.to_string_lossy().trim_end_matches(['/', '\\']) == parent.as_str())
                    .unwrap_or(false)
            })
            .collect())
    }

    /// 按精确标题查询所有档案（供自动分组展开时拉取完整成员列表）
    pub fn get_archives_by_title(&self, title: &str) -> Result<Vec<ArchiveRow>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, title, path, archive_type, page_count, cover_image, file_size, thumbnail_path, group_id, created_at, updated_at
             FROM archives WHERE title = ? COLLATE NOCASE ORDER BY path",
        )?;

        let archives = stmt
            .query_map([title], archive_row)?
            .filter_map(log_and_skip)
            .collect();

        Ok(archives)
    }

    /// 合并多个档案：第一个为主档案，其余 group_id 设为主档案 id
    pub fn merge_archives(&self, archive_ids: &[i64]) -> Result<i64> {
        let primary_id = archive_ids[0];
        let conn = self.conn()?;
        let tx = conn.unchecked_transaction()?;

        // 主档案: group_id 设为自身 id
        tx.execute(
            "UPDATE archives SET group_id = ?, updated_at = datetime('now') WHERE id = ?",
            (primary_id, primary_id),
        )?;

        // 其余档案: group_id 设为主档案 id
        let mut set_group = tx.prepare_cached(
            "UPDATE archives SET group_id = ?, updated_at = datetime('now') WHERE id = ?",
        )?;
        for &id in &archive_ids[1..] {
            set_group.execute((primary_id, id))?;
        }
        drop(set_group);

        tx.commit()?;
        Ok(primary_id)
    }

    // Thumbnail cache operations
    pub fn set_thumbnail_path(&self, archive_id: i64, thumb_path: &str) -> Result<()> {
        self.conn()?.execute(
            "UPDATE archives SET thumbnail_path = ? WHERE id = ?",
            (thumb_path, archive_id),
        )?;
        Ok(())
    }

    /// 清除若干档案的 `thumbnail_path`（供封面缓存 LRU 淘汰后登记）。
    pub fn clear_thumbnail_paths(&self, ids: &[i64]) -> Result<()> {
        if ids.is_empty() {
            return Ok(());
        }
        let conn = self.conn()?;
        let placeholders = vec!["?"; ids.len()].join(",");
        conn.execute(
            &format!(
                "UPDATE archives SET thumbnail_path = NULL WHERE id IN ({})",
                placeholders
            ),
            rusqlite::params_from_iter(ids.iter()),
        )?;
        Ok(())
    }

    /// 记录一次缩略图访问（由路由层节流调用），供 LRU 按真实使用时间淘汰。
    pub fn touch_thumbnail_access(&self, archive_id: i64) -> Result<usize> {
        self.conn()?.execute(
            "UPDATE archives SET thumb_accessed_at = datetime('now') WHERE id = ?",
            [archive_id],
        )
    }

    /// 有缩略图缓存的档案，按「最近访问 → updated_at」倒序排列（最近使用的留在缓存）。
    pub fn get_cached_archive_ids(&self) -> Result<Vec<i64>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT a.id FROM archives a
             WHERE a.thumbnail_path IS NOT NULL
             ORDER BY COALESCE(a.thumb_accessed_at, a.updated_at) DESC",
        )?;
        let ids = stmt
            .query_map([], |row| row.get::<_, i64>(0))?
            .filter_map(log_and_skip)
            .collect();
        Ok(ids)
    }

    /// 所有存活档案的 id 集合，用于清理不再被任何档案引用的缓存目录
    /// （extract/thumbnails/page_thumbs 下的孤立子目录）。
    pub fn live_archive_ids(&self) -> Result<std::collections::HashSet<i64>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare("SELECT id FROM archives")?;
        let ids = stmt
            .query_map([], |row| row.get::<_, i64>(0))?
            .filter_map(log_and_skip)
            .collect();
        Ok(ids)
    }

    /// 设置/清除手动封面页（cover_image 存档案内页面名；None 恢复默认首页）。
    pub fn set_archive_cover(&self, id: i64, cover: Option<&str>) -> Result<usize> {
        let conn = self.conn()?;
        match cover {
            Some(c) => conn.execute(
                "UPDATE archives SET cover_image = ?1 WHERE id = ?2",
                (c, id),
            ),
            None => conn.execute("UPDATE archives SET cover_image = NULL WHERE id = ?", [id]),
        }
    }

    /// 读取远程封面 URL（无则 None）。
    pub fn get_remote_cover(&self, id: i64) -> Result<Option<String>> {
        self.conn()?
            .query_row(
                "SELECT remote_cover FROM archives WHERE id = ?",
                [id],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()
            .map(|v| v.flatten())
    }

    /// 设置/清除远程封面 URL（None 清除，恢复 页面封面/首页 的优先级）。
    pub fn set_remote_cover(&self, id: i64, url: Option<&str>) -> Result<usize> {
        let conn = self.conn()?;
        match url {
            Some(u) => conn.execute(
                "UPDATE archives SET remote_cover = ?1 WHERE id = ?2",
                (u, id),
            ),
            None => conn.execute("UPDATE archives SET remote_cover = NULL WHERE id = ?", [id]),
        }
    }

    /// 按添加日期聚合：年 → 月 → 数量，供侧栏"日期"树使用。
    /// 按**本机时区**分桶（created_at 为 UTC，datetime(..., 'localtime') 转本地后再取年月），
    /// 与前端卡片展示的本地日期一致；年降序（最新在前）、月升序（1~12）。
    /// 空库/日期异常的行（NULL、空串 → strftime 为 NULL）自动跳过，返回 `{"years":[]}`。
    pub fn added_tree(&self) -> Result<serde_json::Value> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT strftime('%Y', datetime(created_at, 'localtime')),
                    CAST(strftime('%m', datetime(created_at, 'localtime')) AS INTEGER),
                    COUNT(*)
             FROM archives
             WHERE created_at IS NOT NULL AND created_at != ''
             GROUP BY 1, 2
             ORDER BY 1 DESC, 2 ASC",
        )?;
        let rows: Vec<(String, i64, i64)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
            .filter_map(log_and_skip)
            .collect();

        // 行已按年降序、月升序排好：连续同年的行并入当前组
        let mut years: Vec<serde_json::Value> = Vec::new();
        let mut cur: Option<(i64, i64, Vec<serde_json::Value>)> = None;
        for (y_str, month, count) in rows {
            let Ok(year) = y_str.parse::<i64>() else {
                continue;
            };
            match &mut cur {
                Some((cy, total, months)) if *cy == year => {
                    *total += count;
                    months.push(serde_json::json!({ "month": month, "count": count }));
                }
                _ => {
                    if let Some((cy, total, months)) = cur.take() {
                        years.push(
                            serde_json::json!({ "year": cy, "count": total, "months": months }),
                        );
                    }
                    cur = Some((
                        year,
                        count,
                        vec![serde_json::json!({ "month": month, "count": count })],
                    ));
                }
            }
        }
        if let Some((cy, total, months)) = cur {
            years.push(serde_json::json!({ "year": cy, "count": total, "months": months }));
        }
        Ok(serde_json::json!({ "years": years }))
    }
}

#[cfg(test)]
mod added_date_tests {
    use super::*;

    fn setup() -> Database {
        // 与 db::tests::setup_test_db 同款：连接打开后 NamedTempFile 被 drop（Unix 上
        // 文件被 unlink，SQLite 连接仍持有 fd 可正常读写），测试结束自动回收。
        let temp_file = tempfile::NamedTempFile::new().unwrap();
        let db = Database::new(temp_file.path().to_str().unwrap()).unwrap();
        db.init().unwrap();
        db
    }

    /// 造一个指定 created_at 的档案（中间日期 12:00，任何时区下本地日期都同一天，
    /// 保证分桶断言与运行机时区无关）。
    fn insert_with_date(db: &Database, title: &str, date: &str) -> i64 {
        let id = db
            .upsert_scanned_archive(title, &format!("/x/{title}.cbz"), "cbz", 5, 10, 1)
            .unwrap();
        db.conn()
            .unwrap()
            .execute(
                "UPDATE archives SET created_at = ? WHERE id = ?",
                rusqlite::params![date, id],
            )
            .unwrap();
        id
    }

    /// 年/月分桶与计数：年降序（2026 在前）、月升序（3 月在 7 月前）。
    #[test]
    fn added_tree_buckets_by_year_and_month() {
        let db = setup();
        insert_with_date(&db, "春书", "2026-03-15 12:00:00");
        insert_with_date(&db, "夏书", "2026-07-02 12:00:00");
        insert_with_date(&db, "冬书", "2025-12-31 12:00:00");

        let tree = db.added_tree().unwrap();
        let years = tree["years"].as_array().unwrap();
        assert_eq!(years.len(), 2);
        assert_eq!(years[0]["year"], 2026);
        assert_eq!(years[0]["count"], 2);
        let months = years[0]["months"].as_array().unwrap();
        assert_eq!(months.len(), 2);
        assert_eq!(months[0]["month"], 3);
        assert_eq!(months[0]["count"], 1);
        assert_eq!(months[1]["month"], 7);
        assert_eq!(years[1]["year"], 2025);
        assert_eq!(years[1]["count"], 1);
    }

    /// 空库返回空 years 数组（前端据此隐藏"日期"段）。
    #[test]
    fn added_tree_empty_db_returns_empty_years() {
        let db = setup();
        let tree = db.added_tree().unwrap();
        assert!(tree["years"].as_array().unwrap().is_empty());
    }

    /// 日期范围过滤：from 含、to 不含；可只给单边边界；不给 = 全部。
    #[test]
    fn added_range_filter_selects_by_bounds() {
        let db = setup();
        insert_with_date(&db, "三月书", "2026-03-15 12:00:00");
        insert_with_date(&db, "四月书", "2026-04-15 12:00:00");

        let titles = |from: Option<&str>, to: Option<&str>| -> Vec<String> {
            db.list_archives(None, None, None, from, to, "created", "desc", 50, 0)
                .unwrap()
                .into_iter()
                .map(|a| a.title)
                .collect()
        };
        assert_eq!(
            titles(Some("2026-03-01"), Some("2026-04-01")),
            vec!["三月书"],
            "3月区间：from 含、to 不含"
        );
        assert_eq!(
            titles(Some("2026-04-01"), None),
            vec!["四月书"],
            "只给下界：不含更早的三月书"
        );
        assert_eq!(
            titles(None, Some("2026-04-01")),
            vec!["三月书"],
            "只给上界：不含四月书"
        );
        assert_eq!(titles(None, None).len(), 2, "无日期参数 = 全部");
    }
}

#[cfg(test)]
mod read_state_tests {
    use super::*;

    fn setup() -> Database {
        let temp_file = tempfile::NamedTempFile::new().unwrap();
        let db = Database::new(temp_file.path().to_str().unwrap()).unwrap();
        db.init().unwrap();
        db
    }

    /// 入库一个指定页数的档案（不写 history = 未读）。
    fn add(db: &Database, title: &str, page_count: i64) -> i64 {
        db.upsert_scanned_archive(title, &format!("/x/{title}.cbz"), "cbz", page_count, 10, 1)
            .unwrap()
    }

    fn set_page_count(db: &Database, id: i64, page_count: i64) {
        db.conn()
            .unwrap()
            .execute(
                "UPDATE archives SET page_count = ? WHERE id = ?",
                rusqlite::params![page_count, id],
            )
            .unwrap();
    }

    fn titles(db: &Database, read: Option<&str>) -> Vec<String> {
        db.list_archives_all(None, None, None, None, None, read, None, "title", "asc")
            .unwrap()
            .into_iter()
            .map(|a| a.title)
            .collect()
    }

    /// 断言两侧用同一比较器排序，避免依赖 SQLite 的排序规则。
    fn any_order(mut v: Vec<&str>) -> Vec<String> {
        v.sort();
        v.into_iter().map(String::from).collect()
    }

    /// 纯函数：未指定/未知取值不过滤；三个精确取值都引用同一份「有效总页数」表达式。
    #[test]
    fn read_filter_clause_shares_one_total_pages_expression() {
        assert!(read_filter_clause(None).is_empty());
        assert!(read_filter_clause(Some("all")).is_empty());
        assert!(read_filter_clause(Some("whatever")).is_empty());

        for value in ["in_progress", "finished"] {
            let clause = read_filter_clause(Some(value));
            assert!(
                clause.contains(EFFECTIVE_TOTAL_PAGES),
                "{value} 必须与 EFFECTIVE_TOTAL_PAGES 共用同一表达式，否则集合会既漏又重"
            );
        }
        assert!(read_filter_clause(Some("unread")).contains("NOT EXISTS"));
        // 旧取值保持「有阅读记录」语义，不加总页数条件
        let legacy = read_filter_clause(Some("read"));
        assert!(legacy.contains("EXISTS") && !legacy.contains("NOT EXISTS"));
        assert!(!legacy.contains(EFFECTIVE_TOTAL_PAGES));
    }

    /// 三种状态严格划分整个书库：未读 / 在读 / 已读完，且 在读+已读完 = 有阅读记录。
    #[test]
    fn read_state_partitions_library() {
        let db = setup();
        add(&db, "未读", 10);
        let reading = add(&db, "在读", 10);
        let finished = add(&db, "已读完", 10);
        let just_opened = add(&db, "刚翻开", 10);
        db.save_history(reading, 3, 10).unwrap();
        db.save_history(finished, 9, 10).unwrap();
        db.save_history(just_opened, 0, 10).unwrap();

        assert_eq!(titles(&db, Some("unread")), any_order(vec!["未读"]));
        assert_eq!(
            titles(&db, Some("in_progress")),
            any_order(vec!["在读", "刚翻开"]),
            "第 1 页关掉也算在读，不能算已读完"
        );
        assert_eq!(titles(&db, Some("finished")), any_order(vec!["已读完"]));

        let started = any_order(vec!["在读", "已读完", "刚翻开"]);
        assert_eq!(
            titles(&db, Some("read")),
            started,
            "旧取值 read 必须等于 在读 + 已读完"
        );
        assert_eq!(
            titles(&db, Some("read")).len(),
            titles(&db, Some("in_progress")).len() + titles(&db, Some("finished")).len(),
            "两个新取值必须无重叠地覆盖有阅读记录的集合"
        );
        assert_eq!(titles(&db, None).len(), 4, "缺省 = 全部");
    }

    /// 文件补页后（page_count 变大）旧进度不能被判成已读完。
    #[test]
    fn page_count_wins_over_stale_history_total() {
        let db = setup();
        let id = add(&db, "补页后", 20);
        db.save_history(id, 9, 10).unwrap(); // 客户端上次只知道 10 页

        assert!(titles(&db, Some("finished")).is_empty());
        assert_eq!(titles(&db, Some("in_progress")), any_order(vec!["补页后"]));
    }

    /// 总页数完全未知（文件夹未数页）时归「在读」，绝不让条目在三个筛选下全部消失。
    #[test]
    fn unknown_total_counts_as_in_progress() {
        let db = setup();
        let id = add(&db, "页数未知", 0);
        db.save_history(id, 0, 0).unwrap();

        assert_eq!(titles(&db, Some("unread")).len(), 0);
        assert_eq!(titles(&db, Some("finished")).len(), 0);
        assert_eq!(
            titles(&db, Some("in_progress")),
            any_order(vec!["页数未知"])
        );
    }

    /// page_count 为 0 但 history 记录了总页数时，用 history 的总页数判定读完。
    #[test]
    fn history_total_used_when_page_count_unknown() {
        let db = setup();
        let id = add(&db, "靠history判定", 5);
        set_page_count(&db, id, 0);
        db.save_history(id, 4, 5).unwrap();

        assert_eq!(
            titles(&db, Some("finished")),
            any_order(vec!["靠history判定"])
        );
    }
}

#[cfg(test)]
mod tag_state_tests {
    use super::*;

    fn setup() -> Database {
        let temp_file = tempfile::NamedTempFile::new().unwrap();
        let db = Database::new(temp_file.path().to_str().unwrap()).unwrap();
        db.init().unwrap();
        db
    }

    fn add(db: &Database, title: &str) -> i64 {
        db.upsert_scanned_archive(title, &format!("/x/{title}.cbz"), "cbz", 5, 10, 1)
            .unwrap()
    }

    fn titles(db: &Database, tag: Option<&str>, tag_state: Option<&str>) -> Vec<String> {
        db.list_archives_all(None, tag, None, None, None, None, tag_state, "title", "asc")
            .unwrap()
            .into_iter()
            .map(|a| a.title)
            .collect()
    }

    fn any_order(mut v: Vec<&str>) -> Vec<String> {
        v.sort();
        v.into_iter().map(String::from).collect()
    }

    /// 纯函数：缺省/未知取值不过滤；且绝不能复用搜索语法与标签过滤的子查询别名
    /// （`atx`/`at_f`）——同一条 WHERE 里共用别名会让两个标签条件互相串味。
    #[test]
    fn tag_state_clause_is_isolated_from_other_tag_subqueries() {
        assert!(tag_state_clause(None).is_empty());
        assert!(tag_state_clause(Some("all")).is_empty());
        assert!(tag_state_clause(Some("whatever")).is_empty());

        for value in ["untagged", "tagged"] {
            let clause = tag_state_clause(Some(value));
            assert!(
                !clause.contains("atx"),
                "{value} 不能复用搜索语法的别名 atx"
            );
            assert!(
                !clause.contains("at_f"),
                "{value} 不能复用标签过滤的别名 at_f"
            );
        }
        assert!(tag_state_clause(Some("untagged")).contains("NOT EXISTS"));
        assert!(tag_state_clause(Some("tagged")).contains("EXISTS"));
        assert!(!tag_state_clause(Some("tagged")).contains("NOT EXISTS"));
    }

    /// 未打标签 / 已打标签各自取到正确的集合
    #[test]
    fn tag_state_partitions_library() {
        let db = setup();
        let tagged = add(&db, "已整理");
        add(&db, "待整理A");
        add(&db, "待整理B");
        let tag = db.create_tag("", "动作", "#fff").unwrap();
        db.assign_tag(tagged, tag).unwrap();

        assert_eq!(
            titles(&db, None, Some("untagged")),
            any_order(vec!["待整理A", "待整理B"])
        );
        assert_eq!(titles(&db, None, Some("tagged")), any_order(vec!["已整理"]));
        assert_eq!(titles(&db, None, None).len(), 3, "缺省 = 全部");
    }

    /// 与标签过滤组合时语义正确：`tag=X & untagged` 必然为空（两条件互斥），
    /// `tag=X & tagged` 只剩带 X 的那本——整理模式正是靠这个组合找"还没打某个标签"的书
    #[test]
    fn tag_state_combines_with_tag_filter() {
        let db = setup();
        let with_x = add(&db, "有动作");
        let with_y = add(&db, "只有喜剧");
        add(&db, "没有标签");
        let action = db.create_tag("", "动作", "#fff").unwrap();
        let comedy = db.create_tag("", "喜剧", "#fff").unwrap();
        db.assign_tag(with_x, action).unwrap();
        db.assign_tag(with_y, comedy).unwrap();

        assert!(titles(&db, Some("动作"), Some("untagged")).is_empty());
        assert_eq!(
            titles(&db, Some("动作"), Some("tagged")),
            any_order(vec!["有动作"])
        );
        assert_eq!(
            titles(&db, Some("喜剧"), Some("tagged")),
            any_order(vec!["只有喜剧"])
        );
    }

    /// 标签被移除后重新回到「未打标签」
    #[test]
    fn removing_last_tag_moves_archive_back_to_untagged() {
        let db = setup();
        let id = add(&db, "来回");
        let tag = db.create_tag("", "临时", "#fff").unwrap();
        db.assign_tag(id, tag).unwrap();
        assert_eq!(titles(&db, None, Some("tagged")).len(), 1);

        db.remove_tag(id, tag).unwrap();
        assert_eq!(titles(&db, None, Some("untagged")), any_order(vec!["来回"]));
    }
}

#[cfg(test)]
mod sibling_tests {
    use super::*;

    fn setup() -> Database {
        let temp_file = tempfile::NamedTempFile::new().unwrap();
        let db = Database::new(temp_file.path().to_str().unwrap()).unwrap();
        db.init().unwrap();
        db
    }

    fn add(db: &Database, title: &str, path: &str) -> i64 {
        db.upsert_scanned_archive(title, path, "cbz", 5, 10, 1)
            .unwrap()
    }

    fn paths_of(db: &Database, id: i64) -> Vec<String> {
        db.get_siblings_in_dir(id)
            .unwrap()
            .into_iter()
            .map(|a| a.path)
            .collect()
    }

    /// 只收同父目录的档案：不带上层、不带子目录，且每个成员的兄弟集合一致（含自己）。
    #[test]
    fn siblings_are_scoped_to_the_same_directory() {
        let db = setup();
        let a = add(&db, "A", "/lib/series/A.cbz");
        let b = add(&db, "B", "/lib/series/B.cbz");
        add(&db, "C", "/lib/other/C.cbz");
        add(&db, "D", "/lib/series/sub/D.cbz");

        let expected = vec!["/lib/series/A.cbz", "/lib/series/B.cbz"];
        assert_eq!(paths_of(&db, a), expected);
        assert_eq!(paths_of(&db, b), expected, "同目录成员的兄弟集合应完全相同");
    }

    /// 真实系列的关键前提：标题不同（xxx 01 / xxx 02）也要能被同目录收进来，
    /// 否则「下一卷」永远只能靠合并组才成立。
    #[test]
    fn different_titles_in_one_directory_are_all_returned() {
        let db = setup();
        let v1 = add(&db, "系列 01", "/lib/系列/系列 01.cbz");
        add(&db, "系列 02", "/lib/系列/系列 02.cbz");
        add(&db, "系列 03", "/lib/系列/系列 03.cbz");

        assert_eq!(paths_of(&db, v1).len(), 3);
    }

    /// 尾部分隔符不影响目录归一化（与路由层 parent 过滤同一口径）。
    #[test]
    fn trailing_separator_is_normalized() {
        let db = setup();
        let a = add(&db, "A", "/lib/series/A.cbz");
        add(&db, "B", "/lib/series/B.cbz");

        assert_eq!(paths_of(&db, a).len(), 2);
    }

    #[test]
    fn missing_id_yields_empty_list() {
        let db = setup();
        add(&db, "A", "/lib/series/A.cbz");
        assert!(db.get_siblings_in_dir(9999).unwrap().is_empty());
    }
}

#[cfg(test)]
mod undo_tests {
    use super::*;

    fn setup() -> Database {
        let temp_file = tempfile::NamedTempFile::new().unwrap();
        let db = Database::new(temp_file.path().to_str().unwrap()).unwrap();
        db.init().unwrap();
        db
    }

    fn add(db: &Database, title: &str, path: &str) -> i64 {
        db.upsert_scanned_archive(title, path, "cbz", 10, 100, 5)
            .unwrap()
    }

    /// 删除 → 撤销：档案行（原 id、原始 created_at、手动封面）与标签/分类/书签/进度
    /// 必须一模一样地回来。这是"撤销"和"重新扫描入库"的本质区别。
    #[test]
    fn snapshot_restores_metadata_exactly() {
        let db = setup();
        let id = add(&db, "书A", "/x/a.cbz");
        db.save_history(id, 3, 10).unwrap();
        db.add_bookmark(id, 4).unwrap();
        let tag = db.create_tag("artist", "是谁", "#fff").unwrap();
        db.assign_tag(id, tag).unwrap();
        let cat = db.create_category("动作", "#0f0", false, "").unwrap();
        db.assign_category(id, cat).unwrap();
        // 手动封面 + 一个可辨认的 created_at（撤销后"添加时间"和日期树都不该变）
        db.conn()
            .unwrap()
            .execute(
                "UPDATE archives SET created_at = '2020-01-02 03:04:05', cover_image = 'p3.jpg' WHERE id = ?",
                [id],
            )
            .unwrap();

        let snap = db
            .delete_archive_with_snapshot(id)
            .unwrap()
            .expect("应取到快照");
        assert!(db.get_archive(id).unwrap().is_none(), "删除后应查不到");
        assert!(
            db.get_archive_tags(id).unwrap().is_empty(),
            "级联应清掉标签关联"
        );

        assert_eq!(db.restore_archive_snapshots(&[snap]).unwrap(), (1, 0));

        let row = db.get_archive(id).unwrap().expect("应恢复原行");
        assert_eq!(row.id, id, "应恢复原 id（AUTOINCREMENT 不复用，安全）");
        assert_eq!(row.title, "书A");
        assert_eq!(row.cover_image.as_deref(), Some("p3.jpg"));
        assert_eq!(row.created_at, "2020-01-02 03:04:05");
        assert_eq!(db.get_archive_tags(id).unwrap().len(), 1);
        assert_eq!(db.get_archive_categories(id).unwrap().len(), 1);
        assert_eq!(db.list_bookmarks(id).unwrap(), vec![4]);
        assert_eq!(
            db.get_history_for_archive(id).unwrap().unwrap().page_index,
            3
        );
    }

    /// 删除合并组主档案会把成员的 group_id 置 NULL（ON DELETE SET NULL）；
    /// 撤销必须把归属还回去，否则组会被悄悄拆散。
    #[test]
    fn snapshot_relinks_group_members() {
        let db = setup();
        let first = add(&db, "第一话", "/x/1.cbz");
        let second = add(&db, "第二话", "/x/2.cbz");
        db.merge_archives(&[first, second]).unwrap();
        assert_eq!(
            db.get_archive(second).unwrap().unwrap().group_id,
            Some(first)
        );

        let snap = db.delete_archive_with_snapshot(first).unwrap().unwrap();
        assert_eq!(
            db.get_archive(second).unwrap().unwrap().group_id,
            None,
            "删除主档案会 SET NULL 成员的 group_id"
        );

        assert_eq!(db.restore_archive_snapshots(&[snap]).unwrap(), (1, 0));
        assert_eq!(
            db.get_archive(second).unwrap().unwrap().group_id,
            Some(first),
            "撤销后成员应重新归组"
        );
        assert_eq!(
            db.get_archive(first).unwrap().unwrap().group_id,
            Some(first)
        );
    }

    /// 删除后路径恰好被重新扫描入库（新 id）：撤销应跳过而不是撞 UNIQUE(path) 整批失败。
    #[test]
    fn restore_skips_paths_that_came_back() {
        let db = setup();
        let old_id = add(&db, "旧", "/x/same.cbz");
        let snap = db.delete_archive_with_snapshot(old_id).unwrap().unwrap();
        let new_id = add(&db, "新", "/x/same.cbz");
        assert_ne!(old_id, new_id);

        assert_eq!(db.restore_archive_snapshots(&[snap]).unwrap(), (0, 1));
        // 库里仍是重新入库的那一条，没有被旧快照覆盖
        let row = db.get_archive(new_id).unwrap().unwrap();
        assert_eq!(row.title, "新");
        assert!(db.get_archive(old_id).unwrap().is_none());
    }

    /// 批量删除一次性拿到整批快照，撤销一次全部还原
    #[test]
    fn batch_delete_snapshots_restore_together() {
        let db = setup();
        let a = add(&db, "A", "/x/a.cbz");
        let b = add(&db, "B", "/x/b.cbz");
        let c = add(&db, "C", "/x/c.cbz");

        let snaps = db.delete_archives_with_snapshots(&[a, b, c]).unwrap();
        assert_eq!(snaps.len(), 3);
        assert_eq!(
            db.list_archives(None, None, None, None, None, "title", "asc", 50, 0)
                .unwrap()
                .len(),
            0
        );

        assert_eq!(db.restore_archive_snapshots(&snaps).unwrap(), (3, 0));
        assert_eq!(
            db.list_archives(None, None, None, None, None, "title", "asc", 50, 0)
                .unwrap()
                .len(),
            3
        );
    }

    /// 不存在的 id：不产生快照，也不报错（并发下可能已被别人删掉）
    #[test]
    fn deleting_missing_archive_yields_no_snapshot() {
        let db = setup();
        assert!(db.delete_archive_with_snapshot(999).unwrap().is_none());
        assert!(db
            .delete_archives_with_snapshots(&[998, 999])
            .unwrap()
            .is_empty());
    }
}
