//! 档案（archives 表）相关的查询：档案 CRUD、扫描入库/批量 upsert、分组、
//! 压缩包页表缓存、缩略图路径登记与 LRU 淘汰。

use rusqlite::{OptionalExtension, Result};

use super::{
    archive_row, archive_row_with_remote_cover, log_and_skip, order_expr_for, path_is_within,
    ArchiveFilters, ArchiveRow, Database, GroupedArchiveRow, PageRow, ARCHIVE_COLUMNS,
};

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
    /// `read`: None=全部, "read"=已有阅读记录, "unread"=从未读过。
    #[allow(clippy::too_many_arguments)]
    pub fn list_archives_all(
        &self,
        search: Option<&str>,
        tag: Option<&str>,
        category_id: Option<i64>,
        added_from: Option<&str>,
        added_to: Option<&str>,
        read: Option<&str>,
        sort: &str,
        order: &str,
    ) -> Result<Vec<ArchiveRow>> {
        let conn = self.conn()?;
        let (join_clause, mut where_clause, params) =
            Self::build_archive_filters(&conn, search, tag, category_id, added_from, added_to)?;

        let read_clause = match read {
            Some("read") => " AND EXISTS (SELECT 1 FROM history hf WHERE hf.archive_id = a.id)",
            Some("unread") => {
                " AND NOT EXISTS (SELECT 1 FROM history hf WHERE hf.archive_id = a.id)"
            }
            _ => "",
        };
        where_clause.push_str(read_clause);

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
        sort: &str,
        order: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<GroupedArchiveRow>> {
        let conn = self.conn()?;
        let (join_clause, mut where_clause, mut params) =
            Self::build_archive_filters(&conn, search, tag, category_id, added_from, added_to)?;

        let read_clause = match read {
            Some("read") => " AND EXISTS (SELECT 1 FROM history hf WHERE hf.archive_id = a.id)",
            Some("unread") => {
                " AND NOT EXISTS (SELECT 1 FROM history hf WHERE hf.archive_id = a.id)"
            }
            _ => "",
        };
        where_clause.push_str(read_clause);

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

    pub fn update_archive_title(&self, id: i64, title: &str) -> Result<usize> {
        self.conn()?.execute(
            "UPDATE archives SET title = ?, title_auto = 0, updated_at = datetime('now') WHERE id = ?",
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
        for &id in &archive_ids[1..] {
            tx.execute(
                "UPDATE archives SET group_id = ?, updated_at = datetime('now') WHERE id = ?",
                (primary_id, id),
            )?;
        }

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
