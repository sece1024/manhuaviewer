//! 标签（tags / archive_tags 表）相关的查询。

use rusqlite::{OptionalExtension, Result};
use std::collections::HashMap;

use super::{log_and_skip, Database, TagRow};

/// 跨机标签镜像的传输单元：按 (namespace, name) 对齐两台机器的标签——
/// **不传 ID**（两侧自增 ID 空间不同）。Serialize 供本机推送，Deserialize 供远端接收。
#[derive(
    Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct TagRef {
    #[serde(default)]
    pub namespace: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub color: String,
}

impl Database {
    pub fn list_tags(&self) -> Result<Vec<TagRow>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT t.id, t.namespace, t.name, t.color, COUNT(at.archive_id)
             FROM tags t
             LEFT JOIN archive_tags at ON at.tag_id = t.id
             GROUP BY t.id
             ORDER BY t.namespace, t.name",
        )?;
        let tags = stmt
            .query_map([], |row| {
                Ok(TagRow {
                    id: row.get(0)?,
                    namespace: row.get(1)?,
                    name: row.get(2)?,
                    color: row.get(3)?,
                    archive_count: row.get(4)?,
                })
            })?
            .filter_map(log_and_skip)
            .collect();
        Ok(tags)
    }

    pub fn create_tag(&self, namespace: &str, name: &str, color: &str) -> Result<i64> {
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO tags (namespace, name, color) VALUES (?, ?, ?)",
            (namespace, name, color),
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn update_tag(&self, id: i64, namespace: &str, name: &str, color: &str) -> Result<usize> {
        self.conn()?.execute(
            "UPDATE tags SET namespace = ?, name = ?, color = ? WHERE id = ?",
            (namespace, name, color, id),
        )
    }

    pub fn delete_tag(&self, id: i64) -> Result<usize> {
        self.conn()?.execute("DELETE FROM tags WHERE id = ?", [id])
    }

    pub fn assign_tag(&self, archive_id: i64, tag_id: i64) -> Result<usize> {
        self.conn()?.execute(
            "INSERT OR IGNORE INTO archive_tags (archive_id, tag_id) VALUES (?, ?)",
            (archive_id, tag_id),
        )
    }

    /// 按 (namespace, name) 幂等获取或创建标签并返回 id（供同步/回放元数据使用）。
    pub fn get_or_create_tag(&self, namespace: &str, name: &str, color: &str) -> Result<i64> {
        let conn = self.conn()?;
        let existing: Option<i64> = conn
            .query_row(
                "SELECT id FROM tags WHERE namespace = ? AND name = ?",
                (namespace, name),
                |r| r.get(0),
            )
            .optional()?;
        if let Some(id) = existing {
            return Ok(id);
        }
        conn.execute(
            "INSERT INTO tags (namespace, name, color) VALUES (?, ?, ?)",
            (namespace, name, color),
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn remove_tag(&self, archive_id: i64, tag_id: i64) -> Result<usize> {
        self.conn()?.execute(
            "DELETE FROM archive_tags WHERE archive_id = ? AND tag_id = ?",
            (archive_id, tag_id),
        )
    }

    /// 批量为多个档案分配标签，单事务执行
    pub fn batch_assign_tag(&self, archive_ids: &[i64], tag_id: i64) -> Result<usize> {
        if archive_ids.is_empty() {
            return Ok(0);
        }
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        let mut affected = 0;
        for &archive_id in archive_ids {
            affected += tx.execute(
                "INSERT OR IGNORE INTO archive_tags (archive_id, tag_id) VALUES (?, ?)",
                (archive_id, tag_id),
            )?;
        }
        tx.commit()?;
        Ok(affected)
    }

    /// 批量移除多个档案的标签，单事务执行
    pub fn batch_remove_tag(&self, archive_ids: &[i64], tag_id: i64) -> Result<usize> {
        if archive_ids.is_empty() {
            return Ok(0);
        }
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        let mut affected = 0;
        for &archive_id in archive_ids {
            affected += tx.execute(
                "DELETE FROM archive_tags WHERE archive_id = ? AND tag_id = ?",
                (archive_id, tag_id),
            )?;
        }
        tx.commit()?;
        Ok(affected)
    }

    pub fn list_namespaces(&self) -> Result<Vec<String>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT DISTINCT namespace FROM tags WHERE namespace != '' ORDER BY namespace",
        )?;
        let namespaces = stmt
            .query_map([], |row| row.get(0))?
            .filter_map(log_and_skip)
            .collect();
        Ok(namespaces)
    }

    pub fn get_archive_tags(&self, archive_id: i64) -> Result<Vec<TagRow>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT t.id, t.namespace, t.name, t.color
             FROM tags t
             JOIN archive_tags at ON at.tag_id = t.id
             WHERE at.archive_id = ?
             ORDER BY t.namespace, t.name",
        )?;

        let tags = stmt
            .query_map([archive_id], |row| {
                Ok(TagRow {
                    id: row.get(0)?,
                    namespace: row.get(1)?,
                    name: row.get(2)?,
                    color: row.get(3)?,
                    archive_count: 0,
                })
            })?
            .filter_map(log_and_skip)
            .collect();

        Ok(tags)
    }

    pub fn get_archive_tags_batch(
        &self,
        archive_ids: &[i64],
    ) -> Result<std::collections::HashMap<i64, Vec<TagRow>>> {
        if archive_ids.is_empty() {
            return Ok(std::collections::HashMap::new());
        }

        let placeholders: String = archive_ids
            .iter()
            .map(|_| "?")
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "SELECT at.archive_id, t.id, t.namespace, t.name, t.color
             FROM tags t
             JOIN archive_tags at ON at.tag_id = t.id
             WHERE at.archive_id IN ({})
             ORDER BY at.archive_id, t.namespace, t.name",
            placeholders
        );

        let conn = self.conn()?;
        let mut stmt = conn.prepare(&sql)?;
        let params: Vec<Box<dyn rusqlite::types::ToSql>> = archive_ids
            .iter()
            .map(|id| Box::new(*id) as Box<dyn rusqlite::types::ToSql>)
            .collect();
        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params.iter().map(|p| p.as_ref()).collect();

        let mut map: std::collections::HashMap<i64, Vec<TagRow>> = std::collections::HashMap::new();
        let rows = stmt.query_map(param_refs.as_slice(), |row| {
            Ok((
                row.get::<_, i64>(0)?,
                TagRow {
                    id: row.get(1)?,
                    namespace: row.get(2)?,
                    name: row.get(3)?,
                    color: row.get(4)?,
                    archive_count: 0,
                },
            ))
        })?;

        for row in rows.flatten() {
            map.entry(row.0).or_default().push(row.1);
        }

        Ok(map)
    }

    /// 单个标签的名称查询（供 OPDS 使用）
    pub fn get_tag_name(&self, tag_id: i64) -> Result<Option<String>> {
        self.conn()?
            .query_row("SELECT name FROM tags WHERE id = ?", [tag_id], |row| {
                row.get(0)
            })
            .optional()
    }

    /// 标签镜像源数据：**全部**档案按标题分组——无标签档案也返回空组，
    /// 远端据此把"本机已删光标签"的档案清干净（替换镜像必须拿到全量状态）。
    /// 同名多卷自然并集（跨机只以 title 为键，与拉取方向 apply_metadata 一致）。
    pub fn tag_mirror_by_title(&self) -> Result<HashMap<String, Vec<TagRef>>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT a.title, t.namespace, t.name, t.color
             FROM archives a
             LEFT JOIN archive_tags at ON at.archive_id = a.id
             LEFT JOIN tags t ON t.id = at.tag_id
             ORDER BY a.title, t.namespace, t.name",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        })?;
        let mut map: HashMap<String, Vec<TagRef>> = HashMap::new();
        for (title, namespace, name, color) in rows.filter_map(log_and_skip) {
            let group = map.entry(title).or_default();
            // 无标签档案的 LEFT JOIN 全列 NULL → 只登记空组；有名则入组
            if let Some(name) = name.filter(|n| !n.is_empty()) {
                group.push(TagRef {
                    namespace: namespace.unwrap_or_default(),
                    name,
                    color: color.unwrap_or_default(),
                });
            }
        }
        // 同名多卷的并集去重（行按 title 排序，跨卷拼接后需整体排序再 dedup）
        for group in map.values_mut() {
            group.sort();
            group.dedup();
        }
        Ok(map)
    }

    /// 标签镜像写入（**替换语义**）：按标题把档案标签整组替换为推送方的镜像——
    /// 推送方删掉的标签本机也删（启动同步的机器是标签权威方）。
    /// 未知标题跳过（对端可能有本机尚未下载的档案）；同名多卷全部替换为同一组。
    /// 返回 (替换的档案数, 跳过的未知标题数)。全程单事务，失败不留半套关联。
    pub fn mirror_tags_by_title(
        &self,
        titles: &HashMap<String, Vec<TagRef>>,
    ) -> Result<(usize, usize)> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        let mut matched = 0usize;
        let mut unknown = 0usize;
        for (title, tags) in titles {
            // 同名可能多卷；0 个 = 远端还没有这本 → 忽略
            let mut stmt = tx.prepare("SELECT id FROM archives WHERE title = ?")?;
            let ids: Vec<i64> = stmt
                .query_map([title], |r| r.get(0))?
                .filter_map(log_and_skip)
                .collect();
            drop(stmt);
            if ids.is_empty() {
                unknown += 1;
                continue;
            }
            // 按 (ns, name) 幂等建标签；已存在则保留本机原颜色（与拉取方向同规则）
            let mut tag_ids: Vec<i64> = Vec::with_capacity(tags.len());
            for t in tags {
                if t.name.is_empty() {
                    continue;
                }
                let existing: Option<i64> = tx
                    .query_row(
                        "SELECT id FROM tags WHERE namespace = ? AND name = ?",
                        (&t.namespace, &t.name),
                        |r| r.get(0),
                    )
                    .optional()?;
                let id = match existing {
                    Some(id) => id,
                    None => {
                        tx.execute(
                            "INSERT INTO tags (namespace, name, color) VALUES (?, ?, ?)",
                            (&t.namespace, &t.name, &t.color),
                        )?;
                        tx.last_insert_rowid()
                    }
                };
                if !tag_ids.contains(&id) {
                    tag_ids.push(id);
                }
            }
            for &archive_id in &ids {
                tx.execute(
                    "DELETE FROM archive_tags WHERE archive_id = ?",
                    [archive_id],
                )?;
                for &tag_id in &tag_ids {
                    tx.execute(
                        "INSERT OR IGNORE INTO archive_tags (archive_id, tag_id) VALUES (?, ?)",
                        (archive_id, tag_id),
                    )?;
                }
                matched += 1;
            }
        }
        tx.commit()?;
        Ok((matched, unknown))
    }
}
