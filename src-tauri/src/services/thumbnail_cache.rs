use crate::types::WindowHandle;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

/// 缩略图有效期：15 秒（窗口内容会变化，短 TTL 保证新鲜度）
const TTL_SECS: u64 = 15;
/// 最大缓存文件数（容量限制）
const MAX_ENTRIES: usize = 100;

/// [T5] 缩略图缓存目录：%LOCALAPPDATA%\WindowFlow\thumbnails
/// 外置到独立位置，避免在程序安装目录写入（降低被杀软误判风险）。
fn cache_dir() -> PathBuf {
    let base = std::env::var("LOCALAPPDATA")
        .unwrap_or_else(|_| std::env::temp_dir().to_string_lossy().to_string());
    let dir = PathBuf::from(base).join("WindowFlow").join("thumbnails");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

fn cache_path(hwnd: i64, w: u32, h: u32) -> PathBuf {
    cache_dir().join(format!("{}_{}x{}.png", hwnd, w, h))
}

fn encode_data_url(bytes: &[u8]) -> String {
    use base64::Engine;
    format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    )
}

/// [T5] 获取缩略图：命中缓存且未过期则复用，否则重新捕获并写入外置缓存目录。
pub fn get_or_capture(hwnd: i64, w: u32, h: u32) -> Result<String, String> {
    let path = cache_path(hwnd, w, h);

    // 命中缓存且未过期
    if let Ok(meta) = std::fs::metadata(&path) {
        if let Ok(modified) = meta.modified() {
            if let Ok(age) = SystemTime::now().duration_since(modified) {
                if age < Duration::from_secs(TTL_SECS) {
                    if let Ok(bytes) = std::fs::read(&path) {
                        if !bytes.is_empty() {
                            return Ok(encode_data_url(&bytes));
                        }
                    }
                }
            }
        }
    }

    // 未命中/过期：重新捕获并写入缓存
    let bytes = crate::platform::windows::capture_window_thumbnail_png_bytes(WindowHandle(hwnd), w, h)
        .map_err(|e| e.to_string())?;
    let _ = std::fs::write(&path, &bytes);
    Ok(encode_data_url(&bytes))
}

/// [T5] 清理过期与超量的缩略图文件（由后台线程定期调用）。
pub fn cleanup() {
    let dir = cache_dir();
    let read = match std::fs::read_dir(&dir) {
        Ok(r) => r,
        Err(_) => return,
    };
    let now = SystemTime::now();
    let mut alive: Vec<(PathBuf, SystemTime)> = Vec::new();

    for entry in read.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("png") {
            continue;
        }
        let modified = match entry.metadata().ok().and_then(|m| m.modified().ok()) {
            Some(m) => m,
            None => continue,
        };
        // 过期文件直接删除
        let expired = now
            .duration_since(modified)
            .map(|a| a > Duration::from_secs(TTL_SECS))
            .unwrap_or(true);
        if expired {
            let _ = std::fs::remove_file(&path);
        } else {
            alive.push((path, modified));
        }
    }

    // 容量限制：仅保留最新 MAX_ENTRIES 个
    if alive.len() > MAX_ENTRIES {
        alive.sort_by(|a, b| b.1.cmp(&a.1)); // 新 -> 旧
        for (path, _) in alive.into_iter().skip(MAX_ENTRIES) {
            let _ = std::fs::remove_file(&path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_path_format() {
        let p = cache_path(12345, 160, 90);
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        assert_eq!(name, "12345_160x90.png");
    }

    #[test]
    fn test_encode_data_url() {
        let url = encode_data_url(&[1, 2, 3]);
        assert!(url.starts_with("data:image/png;base64,"));
    }

    #[test]
    fn test_cleanup_no_panic() {
        cleanup();
    }
}
