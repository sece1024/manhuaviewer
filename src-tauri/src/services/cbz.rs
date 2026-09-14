use anyhow::{bail, Result};
use chrono::{Datelike, Timelike};
use std::io::Write;
use std::path::Path;

use super::is_image_file;

/// 收集文件夹中的图片文件（仅当前目录，不递归）
fn collect_images(folder: &Path) -> Result<Vec<std::path::PathBuf>> {
    if !folder.exists() {
        bail!("路径不存在: {}", folder.display());
    }
    if !folder.is_dir() {
        bail!("路径不是文件夹: {}", folder.display());
    }

    let entries: Vec<_> = std::fs::read_dir(folder)?.filter_map(|e| e.ok()).collect();

    // 空文件夹检查
    if entries.is_empty() {
        bail!("无法打包：该文件夹为空");
    }

    let mut images: Vec<std::path::PathBuf> = entries
        .into_iter()
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.file_name()
                    .and_then(|n| n.to_str())
                    .map(is_image_file)
                    .unwrap_or(false)
        })
        .collect();

    // 无有效图片检查
    if images.is_empty() {
        bail!("无法打包：未在当前目录找到有效的漫画图片");
    }

    // 按自然排序
    images.sort_by(|a, b| {
        natord::compare(
            &a.file_name().unwrap_or_default().to_string_lossy(),
            &b.file_name().unwrap_or_default().to_string_lossy(),
        )
    });

    Ok(images)
}

/// 将文件夹中的图片打包为临时 CBZ 文件（供流式下载/同步），
/// 使用 Stored 方式（不压缩），与 pack_folder_to_cbz 同样的收集/排序逻辑。
/// 返回的 NamedTempFile 用 `keep()` 转成持久路径后交给调用方流式回传；
/// 若不 keep，drop 时自动删除。
pub fn pack_folder_to_tempfile(folder_path: &str) -> Result<tempfile::NamedTempFile> {
    let images = collect_images(Path::new(folder_path))?;

    let mut file = tempfile::Builder::new()
        .prefix("mv-sync-")
        .suffix(".cbz")
        .tempfile()?;
    {
        let mut zip = zip::ZipWriter::new(&mut file);
        let base_options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);

        for image_path in &images {
            let file_name = image_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let data = std::fs::read(image_path)?;
            // 把源图片的 mtime 写进 zip 条目，同步后时间信息得以保留
            // （zip 2.x 的 DateTime 只支持 1980..=2107，出界/失败则用默认时间）
            let options = std::fs::metadata(image_path)
                .and_then(|m| m.modified())
                .ok()
                .map(chrono::DateTime::<chrono::Utc>::from)
                .and_then(|dt| {
                    zip::DateTime::from_date_and_time(
                        dt.year().max(1980) as u16,
                        dt.month() as u8,
                        dt.day() as u8,
                        dt.hour() as u8,
                        dt.minute() as u8,
                        dt.second() as u8,
                    )
                    .ok()
                })
                .map(|dt| base_options.last_modified_time(dt))
                .unwrap_or(base_options);
            zip.start_file(&file_name, options)?;
            zip.write_all(&data)?;
        }
        zip.finish()?;
    }
    file.flush()?;
    file.reopen()?; // 位置指针回到 0，便于以只读句柄流式送出
    Ok(file)
}

/// 将文件夹中的图片打包为 CBZ 文件（同步，应在 spawn_blocking 中调用）
///
/// - `folder_path`: 漫画文件夹的绝对路径
/// - `output_dir`: CBZ 输出目录
///
/// 返回生成的 CBZ 文件的绝对路径
pub fn pack_folder_to_cbz(folder_path: &str, output_dir: &str) -> Result<String> {
    let folder = Path::new(folder_path);
    let out_dir = Path::new(output_dir);

    // 校验输出目录
    if !out_dir.exists() {
        std::fs::create_dir_all(out_dir)?;
    }

    let images = collect_images(folder)?;

    // 以文件夹名作为 CBZ 文件名
    let folder_name = folder
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let cbz_path = out_dir.join(format!("{}.cbz", folder_name));

    // 如果目标文件已存在，加上数字后缀避免覆盖
    let cbz_path = if cbz_path.exists() {
        let mut i = 1u32;
        loop {
            let candidate = out_dir.join(format!("{}_{}.cbz", folder_name, i));
            if !candidate.exists() {
                break candidate;
            }
            i += 1;
        }
    } else {
        cbz_path
    };

    // 创建 ZIP 文件（使用 Stored 方式，不压缩，保证读取性能）
    let file = std::fs::File::create(&cbz_path)?;
    let mut zip = zip::ZipWriter::new(file);
    let options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

    for image_path in &images {
        let file_name = image_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let data = std::fs::read(image_path)?;
        zip.start_file(&file_name, options)?;
        zip.write_all(&data)?;
    }

    zip.finish()?;

    Ok(cbz_path.to_string_lossy().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_pack_empty_folder() {
        let dir = TempDir::new().unwrap();
        let out = TempDir::new().unwrap();
        let result = pack_folder_to_cbz(dir.path().to_str().unwrap(), out.path().to_str().unwrap());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("该文件夹为空"));
    }

    #[test]
    fn test_pack_folder_no_images() {
        let dir = TempDir::new().unwrap();
        let out = TempDir::new().unwrap();
        // 只创建非图片文件
        std::fs::write(dir.path().join("readme.txt"), b"hello").unwrap();
        std::fs::create_dir(dir.path().join("subdir")).unwrap();
        let result = pack_folder_to_cbz(dir.path().to_str().unwrap(), out.path().to_str().unwrap());
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("未在当前目录找到有效的漫画图片"));
    }

    #[test]
    fn test_pack_folder_success() {
        let dir = TempDir::new().unwrap();
        let out = TempDir::new().unwrap();
        // 创建测试图片文件
        std::fs::write(dir.path().join("page01.jpg"), b"fake-jpg-data").unwrap();
        std::fs::write(dir.path().join("page02.png"), b"fake-png-data").unwrap();
        std::fs::write(dir.path().join("notes.txt"), b"not an image").unwrap();

        let cbz =
            pack_folder_to_cbz(dir.path().to_str().unwrap(), out.path().to_str().unwrap()).unwrap();
        assert!(cbz.ends_with(".cbz"));
        assert!(Path::new(&cbz).exists());

        // 验证 CBZ 内容
        let file = std::fs::File::open(&cbz).unwrap();
        let archive = zip::ZipArchive::new(file).unwrap();
        assert_eq!(archive.len(), 2); // 只有 2 张图片，txt 被过滤
    }

    /// 打包必须把源图片的 mtime 写进 zip 条目（同步时间信息保留的根基）。
    #[test]
    fn test_pack_tempfile_preserves_entry_mtime() {
        let dir = TempDir::new().unwrap();
        let img = dir.path().join("page01.jpg");
        std::fs::write(&img, b"fake-jpg-data").unwrap();
        let t = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
        {
            let f = std::fs::File::open(&img).unwrap();
            f.set_times(std::fs::FileTimes::new().set_modified(t))
                .unwrap();
        }

        let tmp = pack_folder_to_tempfile(dir.path().to_str().unwrap()).unwrap();
        let mut archive = zip::ZipArchive::new(tmp.reopen().unwrap()).unwrap();
        assert_eq!(archive.len(), 1);
        let zf = archive.by_index(0).unwrap();
        let dt = zf.last_modified().expect("条目应带时间");
        let expect = chrono::DateTime::<chrono::Utc>::from(t);
        assert_eq!(dt.year(), expect.year().max(1980) as u16);
        assert_eq!(dt.month(), expect.month() as u8);
        assert_eq!(dt.day(), expect.day() as u8);
        assert_eq!(dt.hour(), expect.hour() as u8);
        assert_eq!(dt.minute(), expect.minute() as u8);
        assert_eq!(dt.second(), expect.second() as u8);
    }

    #[test]
    fn test_pack_folder_duplicate_name() {
        let dir = TempDir::new().unwrap();
        let out = TempDir::new().unwrap();
        std::fs::write(dir.path().join("page01.jpg"), b"data").unwrap();

        let cbz1 =
            pack_folder_to_cbz(dir.path().to_str().unwrap(), out.path().to_str().unwrap()).unwrap();
        let cbz2 =
            pack_folder_to_cbz(dir.path().to_str().unwrap(), out.path().to_str().unwrap()).unwrap();
        assert_ne!(cbz1, cbz2); // 第二次应生成不同文件名
        assert!(Path::new(&cbz2).exists());
    }

    #[test]
    fn test_pack_nonexistent_folder() {
        let out = TempDir::new().unwrap();
        let result = pack_folder_to_cbz("/nonexistent/path", out.path().to_str().unwrap());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("路径不存在"));
    }
}
