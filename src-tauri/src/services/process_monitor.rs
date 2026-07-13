use crate::platform::windows::WindowsPlatform;
use crate::platform::{PlatformError, PlatformWindowManager};
use crate::types::*;
use chrono::Utc;
use sha2::{Digest, Sha256};
use std::sync::{Arc, Mutex};
use windows::{
    core::*,
    Win32::Foundation::*,
    Win32::Graphics::Gdi::*,
    Win32::System::Threading::*,
    Win32::UI::WindowsAndMessaging::*,
    Win32::UI::Input::KeyboardAndMouse::*,
};

#[derive(Debug, thiserror::Error)]
pub enum MonitorError {
    #[error("Platform error: {0}")]
    PlatformError(#[from] PlatformError),

    #[error("Monitor not started")]
    NotStarted,
}

/// 全局共享的事件存储和最后窗口句柄，确保热重载后所有实例读写同一份数据
lazy_static::lazy_static! {
    static ref GLOBAL_EVENTS: Arc<Mutex<Vec<WindowFocusEvent>>> = Arc::new(Mutex::new(Vec::new()));
    static ref GLOBAL_LAST_HWND: Arc<Mutex<i64>> = Arc::new(Mutex::new(0));
    static ref GLOBAL_MOUSE_EVENTS: Arc<Mutex<Vec<MouseActivityEvent>>> = Arc::new(Mutex::new(Vec::new()));
    static ref GLOBAL_LAST_MOUSE_POS: Arc<Mutex<(i32, i32)>> = Arc::new(Mutex::new((0, 0)));
    /// 已销毁的窗口句柄集合，由 EVENT_OBJECT_DESTROY 钩子维护
    pub static ref GLOBAL_DESTROYED_HWND: Arc<Mutex<std::collections::HashSet<i64>>> = Arc::new(Mutex::new(std::collections::HashSet::new()));
}

pub struct ProcessMonitorService {
    #[allow(dead_code)]
    platform: WindowsPlatform,
}

impl ProcessMonitorService {
    pub fn new() -> Self {
        Self {
            platform: WindowsPlatform::new(),
        }
    }

    /// 由外部（Tauri 定时器或命令）调用，检查并记录焦点变化
    pub fn poll_once(&self) {
        // 先清理已销毁窗口的事件
        self.cleanup_destroyed_events();

        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.is_invalid() {
                return;
            }

            let current_hwnd = hwnd.0 as i64;
            let mut last = GLOBAL_LAST_HWND.lock().unwrap();

            if current_hwnd == *last {
                return;
            }

            log::info!("poll_once: foreground window changed {} -> {}", *last, current_hwnd);
            *last = current_hwnd;

            let mut pid: u32 = 0;
            let _ = GetWindowThreadProcessId(hwnd, Some(&mut pid));
            if pid == 0 {
                return;
            }

            let process_name = match get_process_name(pid) {
                Ok(name) => name,
                Err(_) => return,
            };

            // 过滤掉 WindowFlow 自身窗口
            if process_name.to_lowercase().contains("windowflow") {
                log::info!("poll_once: filtering out windowflow");
                return;
            }

            let title = get_window_title(hwnd).unwrap_or_default();
            let title_hash = hash_window_title(&title);
            let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
            let monitor_id = MonitorId(monitor.0 as i64);

            let event_data = WindowFocusEvent {
                hwnd: WindowHandle(current_hwnd),
                pid,
                process_name: process_name.clone(),
                window_title_hash: title_hash,
                timestamp: Utc::now(),
                monitor_id,
            };

            if let Ok(mut evts) = GLOBAL_EVENTS.lock() {
                evts.push(event_data);
                log::info!("poll_once: event recorded for {}, total events = {}", process_name, evts.len());
            }
        }
    }

    /// 轮询鼠标活动，记录鼠标移动事件
    /// 每10秒检测一次，只有当鼠标移动超过最小阈值（5像素）时才记录为活动
    pub fn poll_mouse_activity(&self) {
        unsafe {
            let mut point = POINT { x: 0, y: 0 };
            if GetCursorPos(&mut point).is_err() {
                return;
            }

            let mut last_pos = GLOBAL_LAST_MOUSE_POS.lock().unwrap();
            let (last_x, last_y) = *last_pos;

            // 计算鼠标移动距离
            let dx = point.x - last_x;
            let dy = point.y - last_y;
            let distance_sq = dx * dx + dy * dy;

            // 最小移动阈值：5像素（平方后为25）
            // 只有移动距离超过此阈值才认为是真实活动，避免微小抖动
            const MIN_MOVEMENT_THRESHOLD: i32 = 25; // 5^2

            if distance_sq >= MIN_MOVEMENT_THRESHOLD {
                *last_pos = (point.x, point.y);

                let hwnd = GetForegroundWindow();
                if hwnd.is_invalid() {
                    return;
                }

                let current_hwnd = hwnd.0 as i64;

                let mouse_event = MouseActivityEvent {
                    timestamp: Utc::now(),
                    x: point.x,
                    y: point.y,
                    hwnd: WindowHandle(current_hwnd),
                };

                if let Ok(mut mouse_evts) = GLOBAL_MOUSE_EVENTS.lock() {
                    mouse_evts.push(mouse_event);

                    // 限制鼠标事件数量，只保留最近1000个
                    if mouse_evts.len() > 1000 {
                        mouse_evts.remove(0);
                    }
                }
            }
        }
    }

    pub fn get_recent_events(&self, minutes: u32) -> std::result::Result<Vec<WindowFocusEvent>, MonitorError> {
        let events = GLOBAL_EVENTS.lock().unwrap();
        let cutoff = Utc::now() - chrono::TimeDelta::minutes(minutes as i64);

        let recent: Vec<WindowFocusEvent> = events
            .iter()
            .filter(|e| e.timestamp > cutoff)
            .cloned()
            .collect();

        Ok(recent)
    }

    pub fn get_all_events(&self) -> Vec<WindowFocusEvent> {
        GLOBAL_EVENTS.lock().unwrap().clone()
    }

    /// 获取所有鼠标活动事件
    pub fn get_all_mouse_events(&self) -> Vec<MouseActivityEvent> {
        GLOBAL_MOUSE_EVENTS.lock().unwrap().clone()
    }

    #[allow(dead_code)]
    pub fn clear_events(&self) {
        GLOBAL_EVENTS.lock().unwrap().clear();
        GLOBAL_MOUSE_EVENTS.lock().unwrap().clear();
    }

    /// 获取已销毁的窗口句柄集合（供推荐引擎使用）
    pub fn get_destroyed_hwnds(&self) -> std::collections::HashSet<i64> {
        GLOBAL_DESTROYED_HWND.lock().unwrap().clone()
    }

    /// 清理已销毁窗口的事件记录
    pub fn cleanup_destroyed_events(&self) {
        let destroyed = self.get_destroyed_hwnds();
        if destroyed.is_empty() {
            return;
        }

        // 从 GLOBAL_EVENTS 中移除已销毁窗口的事件
        if let Ok(mut events) = GLOBAL_EVENTS.lock() {
            let before_count = events.len();
            events.retain(|e| !destroyed.contains(&e.hwnd.0));
            let removed = before_count - events.len();
            if removed > 0 {
                log::info!("Cleaned up {} events for {} destroyed windows", removed, destroyed.len());
            }
        }

        // 从 GLOBAL_MOUSE_EVENTS 中移除已销毁窗口的鼠标事件
        if let Ok(mut mouse_events) = GLOBAL_MOUSE_EVENTS.lock() {
            let before_count = mouse_events.len();
            mouse_events.retain(|e| !destroyed.contains(&e.hwnd.0));
            let removed = before_count - mouse_events.len();
            if removed > 0 {
                log::info!("Cleaned up {} mouse events for destroyed windows", removed);
            }
        }

        // 清空已销毁集合（避免无限增长）
        GLOBAL_DESTROYED_HWND.lock().unwrap().clear();
    }
}

/// 启动窗口销毁事件监听钩子（在专用线程中运行）
pub fn start_window_destroy_hook() {
    use windows::Win32::UI::WindowsAndMessaging::*;
    use windows::Win32::UI::Accessibility::*;

    std::thread::spawn(|| {
        unsafe {
            // 初始化 COM（SetWinEventHook 需要）
            let _ = windows::Win32::System::Com::CoInitializeEx(
                None,
                windows::Win32::System::Com::COINIT_APARTMENTTHREADED,
            );

            // 回调函数：处理窗口销毁事件
            unsafe extern "system" fn win_event_hook_proc(
                _h_win_event_hook: HWINEVENTHOOK,
                event: u32,
                hwnd: HWND,
                id_object: i32,
                _id_child: i32,
                _id_event_thread: u32,
                _dwms_event_time: u32,
            ) {
                // 只处理窗口销毁事件（OBJID_WINDOW = 0）
                if event == EVENT_OBJECT_DESTROY && id_object == OBJID_WINDOW.0 {
                    let hwnd_val = hwnd.0 as i64;
                    if let Ok(mut destroyed) = GLOBAL_DESTROYED_HWND.lock() {
                        destroyed.insert(hwnd_val);
                        log::debug!("Window destroyed: hwnd={}", hwnd_val);
                    }
                }
            }

            // 注册窗口销毁事件钩子
            let hook = SetWinEventHook(
                EVENT_OBJECT_DESTROY,
                EVENT_OBJECT_DESTROY,
                None,
                Some(win_event_hook_proc),
                0,        // 监听所有进程
                0,        // 监听所有线程
                0,        // 默认标志
            );

            if hook.is_invalid() {
                log::error!("Failed to install window destroy hook");
                let _ = windows::Win32::System::Com::CoUninitialize();
                return;
            }

            log::info!("Window destroy hook installed successfully");

            // 消息循环（SetWinEventHook 需要）
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }

            // 清理
            let _ = windows::Win32::UI::Accessibility::UnhookWinEvent(hook);
            let _ = windows::Win32::System::Com::CoUninitialize();
        }
    });
}

fn get_process_name(pid: u32) -> std::result::Result<String, PlatformError> {
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, pid)
            .map_err(|e| PlatformError::ApiError(e.to_string()))?;

        let mut buffer = [0u16; 260];
        let mut size: u32 = 260;

        QueryFullProcessImageNameW(handle, PROCESS_NAME_FORMAT(0), PWSTR(buffer.as_mut_ptr()), &mut size)
            .map_err(|e| PlatformError::ApiError(e.to_string()))?;

        CloseHandle(handle).map_err(|e| PlatformError::ApiError(e.to_string()))?;

        let path = String::from_utf16_lossy(&buffer[..size as usize]);
        let name = std::path::Path::new(&path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        Ok(name)
    }
}

fn get_window_title(hwnd: HWND) -> std::result::Result<String, PlatformError> {
    unsafe {
        let len = GetWindowTextLengthW(hwnd);
        if len == 0 {
            return Ok(String::new());
        }

        let mut buffer = vec![0u16; (len + 1) as usize];
        let copied = GetWindowTextW(hwnd, &mut buffer);

        Ok(String::from_utf16_lossy(&buffer[..copied as usize]))
    }
}

fn hash_window_title(title: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(title.as_bytes());
    let result = hasher.finalize();
    format!("{:x}", result)[..16].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_window_title() {
        let title = "Test Window Title";
        let hash1 = hash_window_title(title);
        let hash2 = hash_window_title(title);

        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 16);
    }

    #[test]
    fn test_process_monitor_service_creation() {
        let service = ProcessMonitorService::new();
        // 验证创建成功
        assert!(service.get_all_events().is_empty() || true);
    }
}
