use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use std::cmp::Ordering;

/// 简单的“x.y.z”版本比较（忽略 pre-release 后缀与缺段，如 3.3.9 vs 3.3.10）。
pub fn compare_versions(a: &str, b: &str) -> Ordering {
    let nums = |s: &str| -> Vec<i64> {
        s.trim_start_matches('v')
            .split(['.', '-', '+'])
            .filter_map(|seg| seg.parse::<i64>().ok())
            .collect()
    };
    let (na, nb) = (nums(a), nums(b));
    for i in 0..na.len().max(nb.len()) {
        let x = na.get(i).copied().unwrap_or(0);
        let y = nb.get(i).copied().unwrap_or(0);
        match x.cmp(&y) {
            Ordering::Equal => continue,
            other => return other,
        }
    }
    Ordering::Equal
}

fn strip_v(tag: &str) -> String {
    tag.trim_start_matches('v').to_string()
}

/// GET /api/update/check —— 查询 GitHub Releases 最新版本并与本地版本比较。
/// 仅做“提示去下载页”，不做自动安装（后者需要签名密钥与发布基础设施）。
pub async fn update_check() -> Response {
    let local = env!("CARGO_PKG_VERSION").to_string();
    let url = "https://api.github.com/repos/sece1024/manhuaviewer/releases/latest";
    let result = tokio::task::spawn_blocking(move || {
        let output = std::process::Command::new("curl")
            .args([
                "-fsSL",
                "--max-time",
                "12",
                "-A",
                "manhuaviewer/update-check",
            ])
            .arg(url)
            .output()?;
        if !output.status.success() {
            anyhow::bail!("更新检查请求失败（可能离线）");
        }
        let root: serde_json::Value =
            serde_json::from_slice(&output.stdout).map_err(|e| anyhow::anyhow!(e.to_string()))?;
        let tag = root
            .get("tag_name")
            .and_then(|t| t.as_str())
            .unwrap_or_default()
            .to_string();
        let release_url = root
            .get("html_url")
            .and_then(|u| u.as_str())
            .unwrap_or_default()
            .to_string();
        Ok::<(String, String), anyhow::Error>((tag, release_url))
    })
    .await;

    match result {
        Ok(Ok((tag, release_url))) => {
            if tag.is_empty() {
                return super::error_response(
                    StatusCode::BAD_GATEWAY,
                    "未找到版本信息（GitHub 响应异常）",
                );
            }
            let latest = strip_v(&tag);
            let update_available = compare_versions(&latest, &local) == Ordering::Greater;
            Json(serde_json::json!({
                "current": local,
                "latest": latest,
                "update_available": update_available,
                "release_url": release_url,
            }))
            .into_response()
        }
        Ok(Err(e)) => super::error_response(StatusCode::BAD_GATEWAY, &e.to_string()),
        Err(e) => super::internal_error(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cmp::Ordering;

    #[test]
    fn compare_versions_basic() {
        assert_eq!(compare_versions("3.3.9", "3.3.10"), Ordering::Less);
        assert_eq!(compare_versions("3.4.0", "3.3.10"), Ordering::Greater);
        assert_eq!(compare_versions("3.3.9", "3.3.9"), Ordering::Equal);
        assert_eq!(compare_versions("v3.3.9", "3.3.9"), Ordering::Equal);
        // 长度不同按缺位补 0 比较
        assert_eq!(compare_versions("3.10", "3.9.1"), Ordering::Greater);
    }
}
