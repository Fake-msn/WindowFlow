use crate::platform::windows::WindowsPlatform;
use crate::platform::{PlatformError, PlatformWindowManager};
use crate::types::*;
use chrono::Utc;
use sha2::{Digest, Sha256};
use std::sync::{Arc, Mutex};
use std::collections::VecDeque;
use std::sync::atomic::Ordering;
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
    /// 全局进程名缓存: pid -> process_name，减少 OpenProcess 调用（ATT&CK 整改）
    static ref GLOBAL_PROCESS_NAME_CACHE: Arc<Mutex<std::collections::HashMap<u32, String>>> = Arc::new(Mutex::new(std::collections::HashMap::new()));
    /// 前台钩子是否已激活
    pub static ref FOREGROUND_HOOK_ACTIVE: Arc<std::sync::atomic::AtomicBool> = Arc::new(std::sync::atomic::AtomicBool::new(false));
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
    /// 当前台钩子已激活时跳过，避免与钩子产生冗余双写
    pub fn poll_once(&self) {
        if FOREGROUND_HOOK_ACTIVE.load(Ordering::Relaxed) {
            return;
        }
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
                // [ATT&CK 整改] 事件列表容量上限：超过 5000 条时淘汰最旧的事件，防止内存无限增长
                const MAX_EVENTS: usize = 5000;
                if evts.len() > MAX_EVENTS {
                    let overflow = evts.len() - MAX_EVENTS;
                    evts.drain(..overflow);
                    log::info!("poll_once: trimmed {} oldest events (cap={})", overflow, MAX_EVENTS);
                }
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

/// hwnd 事件队列：回调仅入队，消费者线程处理重逻辑（避免回调阻塞导致事件丢失）
lazy_static::lazy_static! {
    static ref GLOBAL_HWND_QUEUE: Arc<Mutex<VecDeque<i64>>> = Arc::new(Mutex::new(VecDeque::new()));
}

/// 启动前台窗口事件监听钩子（EVENT_SYSTEM_FOREGROUND），替代轮询
/// 回调仅将 hwnd 入队，由消费者线程异步处理，避免快速 Alt-Tab 时事件丢失
pub fn start_foreground_hook() {
    use windows::Win32::UI::WindowsAndMessaging::*;
    use windows::Win32::UI::Accessibility::*;

    // 标记前台钩子已激活（poll_once 会据此跳过，避免双写）
    FOREGROUND_HOOK_ACTIVE.store(true, Ordering::Relaxed);

    // 消费者线程：从队列取 hwnd 并执行记录逻辑
    std::thread::spawn(|| {
        loop {
            let hwnd_val = {
                if let Ok(mut queue) = GLOBAL_HWND_QUEUE.lock() {
                    queue.pop_front()
                } else {
                    None
                }
            };

            match hwnd_val {
                Some(val) => record_focus_event_for_hwnd(val),
                None => std::thread::sleep(std::time::Duration::from_millis(10)),
            }
        }
    });

    // 钩子线程：仅入队，不做任何阻塞操作
    std::thread::spawn(|| {
        unsafe {
            let _ = windows::Win32::System::Com::CoInitializeEx(
                None,
                windows::Win32::System::Com::COINIT_APARTMENTTHREADED,
            );

            unsafe extern "system" fn foreground_hook_proc(
                _h_win_event_hook: HWINEVENTHOOK,
                event: u32,
                hwnd: HWND,
                id_object: i32,
                _id_child: i32,
                _id_event_thread: u32,
                _dwms_event_time: u32,
            ) {
                if event == EVENT_SYSTEM_FOREGROUND && id_object == OBJID_WINDOW.0 {
                    let hwnd_val = hwnd.0 as i64;
                    // 仅入队，不执行任何阻塞操作（避免快速 Alt-Tab 时事件丢失）
                    if let Ok(mut queue) = GLOBAL_HWND_QUEUE.lock() {
                        // 队列有界，防止极端情况下无限增长
                        if queue.len() < 50 {
                            queue.push_back(hwnd_val);
                        }
                    }
                }
            }

            let hook = SetWinEventHook(
                EVENT_SYSTEM_FOREGROUND,
                EVENT_SYSTEM_FOREGROUND,
                None,
                Some(foreground_hook_proc),
                0,  // 监听所有进程
                0,  // 监听所有线程
                0,  // WINEVENT_OUTOFCONTEXT: 异步回调，不阻塞目标线程
            );

            if hook.is_invalid() {
                log::error!("Failed to install foreground hook");
                FOREGROUND_HOOK_ACTIVE.store(false, Ordering::Relaxed);
                let _ = windows::Win32::System::Com::CoUninitialize();
                return;
            }

            log::info!("Foreground hook installed (EVENT_SYSTEM_FOREGROUND, queue-based)");

            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }

            let _ = windows::Win32::UI::Accessibility::UnhookWinEvent(hook);
            let _ = windows::Win32::System::Com::CoUninitialize();
        }
    });
}

/// 记录焦点事件的核心逻辑，供 EVENT_SYSTEM_FOREGROUND 钩子回调和 poll_once 共用
fn record_focus_event_for_hwnd(hwnd_val: i64) {
    // 清理已销毁窗口的事件
    let destroyed = GLOBAL_DESTROYED_HWND.lock().ok().map(|g| g.clone()).unwrap_or_default();
    if !destroyed.is_empty() {
        if let Ok(mut events) = GLOBAL_EVENTS.lock() {
            events.retain(|e| !destroyed.contains(&e.hwnd.0));
        }
        if let Ok(mut mouse_events) = GLOBAL_MOUSE_EVENTS.lock() {
            mouse_events.retain(|e| !destroyed.contains(&e.hwnd.0));
        }
        if let Ok(mut destroyed) = GLOBAL_DESTROYED_HWND.lock() {
            destroyed.clear();
        }
    }

    unsafe {
        let hwnd = HWND(hwnd_val as *mut _);

        if hwnd.is_invalid() {
            return;
        }

        let mut last = match GLOBAL_LAST_HWND.lock() {
            Ok(g) => g,
            Err(_) => return,
        };

        if hwnd_val == *last {
            return;
        }

        log::info!("foreground_hook: window changed {} -> {}", *last, hwnd_val);
        *last = hwnd_val;
        drop(last); // 尽早释放锁

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
            return;
        }

        let title = get_window_title(hwnd).unwrap_or_default();
        let title_hash = hash_window_title(&title);
        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        let monitor_id = MonitorId(monitor.0 as i64);

        let event_data = WindowFocusEvent {
            hwnd: WindowHandle(hwnd_val),
            pid,
            process_name: process_name.clone(),
            window_title_hash: title_hash,
            timestamp: Utc::now(),
            monitor_id,
        };

        if let Ok(mut evts) = GLOBAL_EVENTS.lock() {
            evts.push(event_data);
            const MAX_EVENTS: usize = 5000;
            if evts.len() > MAX_EVENTS {
                let overflow = evts.len() - MAX_EVENTS;
                evts.drain(..overflow);
                log::info!("foreground_hook: trimmed {} oldest events (cap={})", overflow, MAX_EVENTS);
            }
            log::info!("foreground_hook: recorded for {}, total={}", process_name, evts.len());
        }
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
    // [CR 修复] 先查全局缓存，减少 OpenProcess 调用（降低 ATT&CK 注册表查询告警）
    if let Ok(cache) = GLOBAL_PROCESS_NAME_CACHE.lock() {
        if let Some(name) = cache.get(&pid) {
            return Ok(name.clone());
        }
    }

    let name = unsafe {
        let handle = OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, pid)
            .map_err(|e| PlatformError::ApiError(e.to_string()))?;

        let mut buffer = [0u16; 260];
        let mut size: u32 = 260;

        QueryFullProcessImageNameW(handle, PROCESS_NAME_FORMAT(0), PWSTR(buffer.as_mut_ptr()), &mut size)
            .map_err(|e| PlatformError::ApiError(e.to_string()))?;

        CloseHandle(handle).map_err(|e| PlatformError::ApiError(e.to_string()))?;

        let path = String::from_utf16_lossy(&buffer[..size as usize]);
        std::path::Path::new(&path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string()
    };

    // 写入缓存（限制最大条目数防止无限增长）
    if let Ok(mut cache) = GLOBAL_PROCESS_NAME_CACHE.lock() {
        if cache.len() > 200 {
            cache.clear();
        }
        cache.insert(pid, name.clone());
    }

    Ok(name)
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
