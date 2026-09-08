use anyhow::Result;
use serde_json::Value;

/// 一次元数据候选项（来自第三方开放目录）。
#[derive(Debug, Clone, serde::Serialize)]
pub struct MetadataCandidate {
    /// 展示名（优先简体中文名，其次原名）
    pub title: String,
    /// 封面 URL（large）
    pub cover: Option<String>,
    /// 评分（0-10，可能缺失）
    pub score: Option<f64>,
    /// 来源侧唯一标识
    pub source_id: String,
    /// 前几个标签
    pub tags: Vec<String>,
}

fn text(obj: &Value, key: &str) -> Option<String> {
    obj.get(key).and_then(|v| v.as_str()).map(|s| s.to_string())
}

/// 解析 Bangumi 搜索响应（结构变化时容忍缺失，返回尽量多的候选）。
pub fn parse_bangumi_search(body: &str) -> Vec<MetadataCandidate> {
    let root: Value = match serde_json::from_str(body) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    let list = root
        .get("list")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    list.iter()
        .filter_map(|item| {
            let id = text(item, "id")?;
            let name = text(item, "name").unwrap_or_default();
            let name_cn = text(item, "name_cn").unwrap_or_default();
            let title = if !name_cn.is_empty() { name_cn } else { name };
            if title.is_empty() {
                return None;
            }
            let cover = item
                .get("images")
                .and_then(|im| text(im, "large"))
                .or_else(|| item.get("images").and_then(|im| text(im, "medium")));
            let score = item
                .get("rating")
                .and_then(|r| r.get("score"))
                .and_then(|s| s.as_f64());
            let tags = item
                .get("tags")
                .and_then(|t| t.as_array())
                .map(|arr| arr.iter().filter_map(|t| text(t, "name")).take(5).collect())
                .unwrap_or_default();
            Some(MetadataCandidate {
                title,
                cover,
                score,
                source_id: id,
                tags,
            })
        })
        .collect()
}

/// 简易 UTF-8 百分号编码（URL 路径段用）。
fn percent_encode(input: &str) -> String {
    let mut out = String::new();
    for b in input.as_bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*b as char)
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

/// 通过 Bangumi 公开搜索接口查找漫画候选。
/// 走系统 curl（与 RAR/7z 一致），避免为单次抓取引入 HTTP 依赖。
pub fn search_bangumi(query: &str) -> Result<Vec<MetadataCandidate>> {
    let q = percent_encode(query);
    let url = format!(
        "https://api.bgm.tv/search/subject/{}?type=2&responseGroup=small&max_results=12",
        q
    );
    let output = std::process::Command::new("curl")
        .args(["-fsSL", "--max-time", "15", "-A", "manhuaviewer/3.4"])
        .arg(&url)
        .output()?;
    if !output.status.success() {
        anyhow::bail!(
            "Bangumi 搜索失败: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(parse_bangumi_search(&String::from_utf8_lossy(
        &output.stdout,
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_bangumi_sample() {
        let body = r#"{"results":1,"list":[{"id":"123","name":"One Piece","name_cn":"海贼王","images":{"large":"https://lain.bgm.tv/pic/cover/l/xx.jpg"},"rating":{"score":8.5},"tags":[{"name":"漫画"},{"name":"热血"}]}]}"#;
        let items = parse_bangumi_search(body);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "海贼王");
        assert_eq!(items[0].source_id, "123");
        assert_eq!(
            items[0].cover.as_deref(),
            Some("https://lain.bgm.tv/pic/cover/l/xx.jpg")
        );
        assert_eq!(items[0].score, Some(8.5));
        assert_eq!(items[0].tags, vec!["漫画", "热血"]);
    }

    #[test]
    fn parse_handles_empty_and_garbage() {
        assert!(parse_bangumi_search("not json").is_empty());
        assert!(parse_bangumi_search(r#"{"list":[]}"#).is_empty());
    }

    #[test]
    fn percent_encode_keeps_safe_chars() {
        assert_eq!(percent_encode("海贼王"), "%E6%B5%B7%E8%B4%BC%E7%8E%8B");
        assert_eq!(percent_encode("one piece"), "one%20piece");
    }
}
