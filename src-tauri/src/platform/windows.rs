use super::{PlatformError, PlatformWindowManager};
use crate::types::*;
use sha2::{Sha256, Digest};
use windows::{
    core::*,
    Win32::Foundation::*,
    Win32::Graphics::Gdi::*,
    Win32::UI::WindowsAndMessaging::*,
    Win32::UI::HiDpi::*,
    Win32::UI::Input::KeyboardAndMouse::*,
    Win32::System::Threading::*,
};

pub struct WindowsPlatform;

impl WindowsPlatform {
    pub fn new() -> Self {
        Self
    }

    pub fn move_window(&self, hwnd: WindowHandle, target_monitor: MonitorId) -> std::result::Result<(), PlatformError> {
        self.move_window_internal(hwnd, target_monitor, true)
    }

    /// 移动窗口（内部方法，支持批量模式）
    /// batch_mode: 批量迁移时不激活窗口，避免干扰后续窗口
    pub fn move_window_internal(&self, hwnd: WindowHandle, target_monitor: MonitorId, activate: bool) -> std::result::Result<(), PlatformError> {
        let hwnd_win: HWND = hwnd.into();

        unsafe {
            // 检查窗口是否可移动（跳过特殊窗口如桌面、任务栏）
            if !self.is_window_movable(hwnd_win) {
                log::info!("Window {:?} is not movable (special window), skipping", hwnd);
                return Ok(());
            }

            let target_monitor_win = HMONITOR(target_monitor.0 as *mut _);

            // 提前检查：窗口是否已经在目标显示器上
            let current_monitor = MonitorFromWindow(hwnd_win, MONITOR_DEFAULTTONEAREST);
            if current_monitor == target_monitor_win {
                log::info!("Window {:?} is already on target monitor, skipping", hwnd);
                return Ok(());
            }

            let mut rect = RECT { left: 0, top: 0, right: 0, bottom: 0 };
            GetWindowRect(hwnd_win, &mut rect)
                .map_err(|e| PlatformError::ApiError(e.to_string()))?;

            let source_monitor = MonitorFromWindow(hwnd_win, MONITOR_DEFAULTTONEAREST);

            let source_dpi = get_monitor_dpi(source_monitor)?;
            let target_dpi = get_monitor_dpi(target_monitor_win)?;

            let scale = target_dpi as f32 / source_dpi as f32;

            let mut target_info = MONITORINFOEXW::default();
            target_info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
            if !GetMonitorInfoW(target_monitor_win, &mut target_info.monitorInfo).as_bool() {
                return Err(PlatformError::ApiError("Failed to get monitor info".to_string()));
            }

            let width = (((rect.right - rect.left) as f32) * scale) as i32;
            let height = (((rect.bottom - rect.top) as f32) * scale) as i32;

            let work_width = target_info.monitorInfo.rcWork.right - target_info.monitorInfo.rcWork.left;
            let work_height = target_info.monitorInfo.rcWork.bottom - target_info.monitorInfo.rcWork.top;

            let x = target_info.monitorInfo.rcWork.left + (work_width - width) / 2;
            let y = target_info.monitorInfo.rcWork.top + (work_height - height) / 2;

            let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);

            // 检查窗口是否最大化，如果是则先还原
            let was_maximized = IsZoomed(hwnd_win).as_bool();
            if was_maximized {
                log::info!("Window {:?} is maximized, restoring before move", hwnd);
                let _ = ShowWindow(hwnd_win, SW_RESTORE);
                std::thread::sleep(std::time::Duration::from_millis(300));
            }

            // 只在非批量模式下激活窗口
            if activate {
                let _ = SetForegroundWindow(hwnd_win);
                std::thread::sleep(std::time::Duration::from_millis(100));
            }

            // 策略1: SetWindowPlacement（最可靠）
            log::info!("Strategy 1: SetWindowPlacement for {:?}", hwnd);
            let mut placement = WINDOWPLACEMENT {
                length: std::mem::size_of::<WINDOWPLACEMENT>() as u32,
                ..Default::default()
            };
            GetWindowPlacement(hwnd_win, &mut placement)
                .map_err(|e| PlatformError::ApiError(e.to_string()))?;

            placement.showCmd = SW_SHOWNORMAL.0 as u32;
            placement.rcNormalPosition = RECT {
                left: x,
                top: y,
                right: x + width,
                bottom: y + height,
            };

            if SetWindowPlacement(hwnd_win, &placement).is_ok() {
                std::thread::sleep(std::time::Duration::from_millis(300));
                if self.check_window_on_monitor(hwnd_win, target_monitor_win) {
                    log::info!("Window {:?} moved successfully via SetWindowPlacement", hwnd);
                    if was_maximized {
                        let _ = ShowWindow(hwnd_win, SW_MAXIMIZE);
                    }
                    return Ok(());
                }
            }
            log::warn!("SetWindowPlacement failed for {:?}, trying next strategy", hwnd);

            // 策略2: SetWindowPos + SWP_FRAMECHANGED
            log::info!("Strategy 2: SetWindowPos for {:?}", hwnd);
            let _ = SetWindowPos(
                hwnd_win,
                None,
                x, y, width, height,
                SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
            );

            std::thread::sleep(std::time::Duration::from_millis(300));
            if self.check_window_on_monitor(hwnd_win, target_monitor_win) {
                log::info!("Window {:?} moved successfully via SetWindowPos", hwnd);
                if was_maximized {
                    let _ = ShowWindow(hwnd_win, SW_MAXIMIZE);
                }
                return Ok(());
            }
            log::warn!("SetWindowPos failed for {:?}, trying next strategy", hwnd);

            // 策略3: MoveWindow
            log::info!("Strategy 3: MoveWindow for {:?}", hwnd);
            let _ = MoveWindow(hwnd_win, x, y, width, height, true);

            std::thread::sleep(std::time::Duration::from_millis(300));
            if self.check_window_on_monitor(hwnd_win, target_monitor_win) {
                log::info!("Window {:?} moved successfully via MoveWindow", hwnd);
                if was_maximized {
                    let _ = ShowWindow(hwnd_win, SW_MAXIMIZE);
                }
                return Ok(());
            }
            log::warn!("MoveWindow failed for {:?}, trying next strategy", hwnd);

            // 策略4: 隐藏→移动→显示
            log::info!("Strategy 4: hide-move-show for {:?}", hwnd);
            let was_visible = IsWindowVisible(hwnd_win).as_bool();

            let _ = ShowWindow(hwnd_win, SW_HIDE);
            std::thread::sleep(std::time::Duration::from_millis(150));

            let _ = SetWindowPos(
                hwnd_win,
                None,
                x, y, width, height,
                SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
            );
            std::thread::sleep(std::time::Duration::from_millis(150));

            if was_visible {
                let _ = ShowWindow(hwnd_win, SW_SHOW);
            } else {
                let _ = ShowWindow(hwnd_win, SW_SHOWNOACTIVATE);
            }
            std::thread::sleep(std::time::Duration::from_millis(300));

            if self.check_window_on_monitor(hwnd_win, target_monitor_win) {
                log::info!("Window {:?} moved successfully via hide-move-show", hwnd);
                if was_maximized {
                    let _ = ShowWindow(hwnd_win, SW_MAXIMIZE);
                }
                return Ok(());
            }
            log::warn!("hide-move-show failed for {:?}, trying next strategy", hwnd);

            // 策略5: 针对 CabinetWClass (explorer.exe 目录窗口) 的特殊处理
            let mut class_name_buf = [0u16; 256];
            let class_len = GetClassNameW(hwnd_win, &mut class_name_buf);
            let class_name = if class_len > 0 {
                String::from_utf16_lossy(&class_name_buf[..class_len as usize])
            } else {
                String::new()
            };

            if class_name == "CabinetWClass" {
                log::info!("Strategy 5: CabinetWClass special handling for {:?}", hwnd);
                // 先激活窗口
                let _ = SetForegroundWindow(hwnd_win);
                std::thread::sleep(std::time::Duration::from_millis(200));

                // 确定目标显示器相对于当前显示器的方向
                let source_rect = {
                    let mut r = RECT::default();
                    let _ = GetWindowRect(hwnd_win, &mut r);
                    r
                };
                let source_center_x = (source_rect.left + source_rect.right) / 2;
                let target_center_x = (target_info.monitorInfo.rcWork.left + target_info.monitorInfo.rcWork.right) / 2;

                // 模拟 Win+Shift+方向键
                let direction = if target_center_x > source_center_x {
                    VK_RIGHT
                } else if target_center_x < source_center_x {
                    VK_LEFT
                } else {
                    let source_center_y = (source_rect.top + source_rect.bottom) / 2;
                    let target_center_y = (target_info.monitorInfo.rcWork.top + target_info.monitorInfo.rcWork.bottom) / 2;
                    if target_center_y > source_center_y { VK_DOWN } else { VK_UP }
                };

                log::info!("Simulating Win+Shift+{:?} for CabinetWClass", direction);
                self.send_win_shift_arrow(direction);
                std::thread::sleep(std::time::Duration::from_millis(500));

                if self.check_window_on_monitor(hwnd_win, target_monitor_win) {
                    log::info!("Window {:?} moved successfully via Win+Shift+Arrow", hwnd);
                    if was_maximized {
                        let _ = ShowWindow(hwnd_win, SW_MAXIMIZE);
                    }
                    return Ok(());
                }
                log::warn!("Win+Shift+Arrow failed for {:?}", hwnd);
            }

            // 策略6: 发送 WM_MOVE 消息（针对其他特殊窗口）- 使用 PostMessage 避免阻塞
            log::info!("Strategy 6: PostMessage WM_MOVE for {:?}", hwnd);
            let _ = PostMessageW(
                Some(hwnd_win),
                WM_MOVE,
                WPARAM(0),
                LPARAM(((y as u32 as u64) << 16 | (x as u32 as u64)) as isize),
            );
            std::thread::sleep(std::time::Duration::from_millis(200));

            if self.check_window_on_monitor(hwnd_win, target_monitor_win) {
                log::info!("Window {:?} moved successfully via WM_MOVE", hwnd);
                if was_maximized {
                    let _ = ShowWindow(hwnd_win, SW_MAXIMIZE);
                }
                return Ok(());
            }
            log::warn!("WM_MOVE failed for {:?}, trying last resort", hwnd);

            // 策略7（最后手段）: 先强制还原，再用 SetWindowPos 不带 SWP_NOSIZE
            log::info!("Strategy 7 (last resort): force restore + SetWindowPos for {:?}", hwnd);
            let _ = ShowWindow(hwnd_win, SW_RESTORE);
            std::thread::sleep(std::time::Duration::from_millis(200));

            let _ = SetWindowPos(
                hwnd_win,
                Some(HWND_TOP),
                x, y, width, height,
                SWP_FRAMECHANGED | SWP_NOACTIVATE,
            );
            std::thread::sleep(std::time::Duration::from_millis(400));

            if self.check_window_on_monitor(hwnd_win, target_monitor_win) {
                log::info!("Window {:?} moved successfully via last resort", hwnd);
                if was_maximized {
                    let _ = ShowWindow(hwnd_win, SW_MAXIMIZE);
                }
                return Ok(());
            }

            // 所有策略都失败
            log::error!("All strategies failed for {:?}, window remains on original monitor", hwnd);
            Err(PlatformError::ApiError(
                "Window failed to move to target monitor after all attempts".to_string()
            ))
        }
    }

    /// 检查窗口是否已经在目标显示器上（只检查显示器归属，不检查精确位置）
    unsafe fn check_window_on_monitor(&self, hwnd_win: HWND, target_monitor_win: HMONITOR) -> bool {
        let current = MonitorFromWindow(hwnd_win, MONITOR_DEFAULTTONEAREST);
        current == target_monitor_win
    }

    /// 模拟 Win+Shift+方向键（用于移动特殊窗口如 CabinetWClass）
    unsafe fn send_win_shift_arrow(&self, vk_code: VIRTUAL_KEY) {
        // 使用 keybd_event 模拟按键（更简单可靠）
        // 按下 Win + Shift
        keybd_event(VK_LWIN.0 as u8, 0, KEYEVENTF_EXTENDEDKEY, 0);
        keybd_event(VK_SHIFT.0 as u8, 0, KEYEVENTF_EXTENDEDKEY, 0);
        
        // 按下方向键
        keybd_event(vk_code.0 as u8, 0, KEYEVENTF_EXTENDEDKEY, 0);
        
        // 释放方向键
        keybd_event(vk_code.0 as u8, 0, KEYEVENTF_EXTENDEDKEY | KEYEVENTF_KEYUP, 0);
        
        // 释放 Shift
        keybd_event(VK_SHIFT.0 as u8, 0, KEYEVENTF_EXTENDEDKEY | KEYEVENTF_KEYUP, 0);
        
        // 释放 Win
        keybd_event(VK_LWIN.0 as u8, 0, KEYEVENTF_EXTENDEDKEY | KEYEVENTF_KEYUP, 0);
    }

    /// 强制移动窗口到目标位置（使用 MoveWindow API）
    pub fn force_move_window(&self, hwnd: WindowHandle, target_monitor: MonitorId) -> std::result::Result<(), PlatformError> {
        let hwnd_win: HWND = hwnd.into();

        unsafe {
            let mut rect = RECT { left: 0, top: 0, right: 0, bottom: 0 };
            GetWindowRect(hwnd_win, &mut rect)
                .map_err(|e| PlatformError::ApiError(e.to_string()))?;

            let source_monitor = MonitorFromWindow(hwnd_win, MONITOR_DEFAULTTONEAREST);
            let target_monitor_win = HMONITOR(target_monitor.0 as *mut _);

            let source_dpi = get_monitor_dpi(source_monitor)?;
            let target_dpi = get_monitor_dpi(target_monitor_win)?;

            let scale = target_dpi as f32 / source_dpi as f32;

            let mut target_info = MONITORINFOEXW::default();
            target_info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
            if !GetMonitorInfoW(target_monitor_win, &mut target_info.monitorInfo).as_bool() {
                return Err(PlatformError::ApiError("Failed to get monitor info".to_string()));
            }

            let width = (((rect.right - rect.left) as f32) * scale) as i32;
            let height = (((rect.bottom - rect.top) as f32) * scale) as i32;

            let work_width = target_info.monitorInfo.rcWork.right - target_info.monitorInfo.rcWork.left;
            let work_height = target_info.monitorInfo.rcWork.bottom - target_info.monitorInfo.rcWork.top;

            let x = target_info.monitorInfo.rcWork.left + (work_width - width) / 2;
            let y = target_info.monitorInfo.rcWork.top + (work_height - height) / 2;

            let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);

            // 使用 MoveWindow API（不同于 SetWindowPos）
            MoveWindow(hwnd_win, x, y, width, height, true)
                .map_err(|e| PlatformError::ApiError(e.to_string()))?;

            Ok(())
        }
    }

    pub fn get_window_dpi(&self, hwnd: WindowHandle) -> std::result::Result<u32, PlatformError> {
        let hwnd_win: HWND = hwnd.into();

        unsafe {
            Ok(GetDpiForWindow(hwnd_win))
        }
    }

    /// 检查窗口是否最小化
    pub fn is_window_minimized(&self, hwnd: WindowHandle) -> std::result::Result<bool, PlatformError> {
        let hwnd_win: HWND = hwnd.into();
        unsafe {
            Ok(IsIconic(hwnd_win).as_bool())
        }
    }

    /// 检查窗口是否可移动（跳过特殊窗口如桌面、任务栏）
    pub fn is_window_movable(&self, hwnd_win: HWND) -> bool {
        unsafe {
            // 检查窗口是否有效
            if !IsWindow(Some(hwnd_win)).as_bool() {
                log::debug!("Window {:?} is not valid, skipping", hwnd_win);
                return false;
            }

            // 检查窗口是否可见
            if !IsWindowVisible(hwnd_win).as_bool() {
                log::debug!("Window {:?} not visible, skipping", hwnd_win);
                return false;
            }

            // 获取窗口类名，过滤系统级窗口
            let mut class_name_buf = [0u16; 256];
            let class_len = GetClassNameW(hwnd_win, &mut class_name_buf);
            if class_len > 0 {
                let class_name = String::from_utf16_lossy(&class_name_buf[..class_len as usize]);
                let class_lower = class_name.to_lowercase();
                
                // 只过滤明确的系统窗口类
                if class_lower == "progman"           // 桌面
                    || class_lower == "workerw"       // 桌面工作区
                    || class_lower == "shell_traywnd" // 任务栏
                    || class_lower == "shell_secondarystart" // 开始菜单
                {
                    log::debug!("Skipping system window class: {}", class_name);
                    return false;
                }
            }

            // 检查窗口标题（允许空标题，但过滤特殊标题）
            let mut title_buf = [0u16; 512];
            let copied = GetWindowTextW(hwnd_win, &mut title_buf);
            let title = String::from_utf16_lossy(&title_buf[..copied as usize]);

            // 跳过特殊窗口标题
            let title_lower = title.to_lowercase();
            if title_lower == "program manager"
                || title_lower == "开始"
                || title_lower == "start"
            {
                log::debug!("Skipping special window title: {}", title);
                return false;
            }

            // 排除工具窗口（小型浮动面板，如浮窗通知）
            let ex_style = GetWindowLongW(hwnd_win, GWL_EXSTYLE) as u32;
            if ex_style & (WS_EX_TOOLWINDOW.0 as u32) != 0 {
                log::debug!("Skipping tool window: {}", title);
                return false;
            }

            log::debug!("Window {:?} '{}' is movable", hwnd_win, title);
            true
        }
    }

    /// 还原窗口（从最小化状态恢复）
    pub fn restore_window(&self, hwnd: WindowHandle) -> std::result::Result<(), PlatformError> {
        let hwnd_win: HWND = hwnd.into();
        unsafe {
            let result = ShowWindow(hwnd_win, SW_RESTORE);
            if !result.as_bool() {
                log::warn!("ShowWindow(SW_RESTORE) returned false for hwnd {:?}", hwnd);
            }
            Ok(())
        }
    }

    /// 最小化窗口
    pub fn minimize_window(&self, hwnd: WindowHandle) -> std::result::Result<(), PlatformError> {
        let hwnd_win: HWND = hwnd.into();
        unsafe {
            let result = ShowWindow(hwnd_win, SW_MINIMIZE);
            if !result.as_bool() {
                log::warn!("ShowWindow(SW_MINIMIZE) returned false for hwnd {:?}", hwnd);
            }
            Ok(())
        }
    }

    /// 最大化窗口
    pub fn maximize_window(&self, hwnd: WindowHandle) -> std::result::Result<(), PlatformError> {
        let hwnd_win: HWND = hwnd.into();
        unsafe {
            let result = ShowWindow(hwnd_win, SW_MAXIMIZE);
            if !result.as_bool() {
                log::warn!("ShowWindow(SW_MAXIMIZE) returned false for hwnd {:?}", hwnd);
            }
            Ok(())
        }
    }

    /// 隐藏窗口
    pub fn hide_window(&self, hwnd: WindowHandle) -> std::result::Result<(), PlatformError> {
        let hwnd_win: HWND = hwnd.into();
        unsafe {
            let result = ShowWindow(hwnd_win, SW_HIDE);
            if !result.as_bool() {
                log::warn!("ShowWindow(SW_HIDE) returned false for hwnd {:?}", hwnd);
            }
            Ok(())
        }
    }

    /// 显示窗口
    pub fn show_window(&self, hwnd: WindowHandle) -> std::result::Result<(), PlatformError> {
        let hwnd_win: HWND = hwnd.into();
        unsafe {
            let result = ShowWindow(hwnd_win, SW_SHOW);
            if !result.as_bool() {
                log::warn!("ShowWindow(SW_SHOW) returned false for hwnd {:?}", hwnd);
            }
            Ok(())
        }
    }

    /// 获取窗口所在的显示器 ID
    pub fn get_window_monitor(&self, hwnd: WindowHandle) -> std::result::Result<MonitorId, PlatformError> {
        let hwnd_win: HWND = hwnd.into();
        unsafe {
            let monitor = MonitorFromWindow(hwnd_win, MONITOR_DEFAULTTONEAREST);
            Ok(MonitorId(monitor.0 as i64))
        }
    }
}

impl PlatformWindowManager for WindowsPlatform {
    fn enumerate_windows(&self) -> std::result::Result<Vec<WindowInfo>, PlatformError> {
        let mut windows = Vec::new();

        unsafe {
            EnumWindows(
                Some(enum_window_proc),
                LPARAM(&mut windows as *mut Vec<WindowInfo> as isize),
            )
            .map_err(|e| PlatformError::ApiError(e.to_string()))?;
        }

        Ok(windows)
    }

    fn enumerate_monitors(&self) -> std::result::Result<Vec<MonitorInfo>, PlatformError> {
        let mut monitors = Vec::new();

        unsafe {
            let result = EnumDisplayMonitors(
                Some(HDC::default()),
                None,
                Some(enum_monitor_proc),
                LPARAM(&mut monitors as *mut Vec<MonitorInfo> as isize),
            );
            if !result.as_bool() {
                return Err(PlatformError::ApiError("Failed to enumerate monitors".to_string()));
            }
        }

        Ok(monitors)
    }

    fn get_window_process_info(&self, hwnd: WindowHandle) -> std::result::Result<ProcessInfo, PlatformError> {
        let hwnd_win: HWND = hwnd.into();

        unsafe {
            let mut pid: u32 = 0;
            GetWindowThreadProcessId(hwnd_win, Some(&mut pid));

            let process_name = get_process_name(pid)?;
            let title = get_window_title(hwnd_win)?;
            let title_hash = hash_string(&title);

            Ok(ProcessInfo {
                pid,
                process_name,
                window_title_hash: title_hash,
            })
        }
    }
}

unsafe extern "system" fn enum_window_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let windows = &mut *(lparam.0 as *mut Vec<WindowInfo>);

    if !IsWindowVisible(hwnd).as_bool() {
        return BOOL(1);
    }

    let title = match get_window_title(hwnd) {
        Ok(t) => t,
        Err(_) => return BOOL(1),
    };

    if title.is_empty() {
        return BOOL(1);
    }

    let mut pid: u32 = 0;
    GetWindowThreadProcessId(hwnd, Some(&mut pid));

    let process_name = match get_process_name(pid) {
        Ok(name) => name,
        Err(_) => return BOOL(1),
    };

    let mut rect = RECT { left: 0, top: 0, right: 0, bottom: 0 };
    if GetWindowRect(hwnd, &mut rect).is_err() {
        return BOOL(1);
    }

    let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
    let monitor_id = MonitorId(monitor.0 as i64);

    let window_info = WindowInfo {
        hwnd: hwnd.into(),
        pid,
        process_name,
        window_title_hash: hash_string(&title),
        rect: WindowRect {
            left: rect.left,
            top: rect.top,
            right: rect.right,
            bottom: rect.bottom,
        },
        monitor_id,
        is_visible: true,
    };

    windows.push(window_info);
    BOOL(1)
}

unsafe extern "system" fn enum_monitor_proc(
    hmonitor: HMONITOR,
    _hdc: HDC,
    _lprect: *mut RECT,
    lparam: LPARAM,
) -> BOOL {
    let monitors = &mut *(lparam.0 as *mut Vec<MonitorInfo>);

    let mut info = MONITORINFOEXW::default();
    info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;

    if !GetMonitorInfoW(hmonitor, &mut info.monitorInfo as *mut MONITORINFO).as_bool() {
        return BOOL(1);
    }

    let dpi = get_monitor_dpi(hmonitor).unwrap_or(96);

    let name = String::from_utf16_lossy(
        &info.szDevice[..]
            .iter()
            .take_while(|&&c| c != 0)
            .copied()
            .collect::<Vec<u16>>(),
    );

    let monitor_info = MonitorInfo {
        id: MonitorId(hmonitor.0 as i64),
        name,
        rect: MonitorRect {
            left: info.monitorInfo.rcMonitor.left,
            top: info.monitorInfo.rcMonitor.top,
            right: info.monitorInfo.rcMonitor.right,
            bottom: info.monitorInfo.rcMonitor.bottom,
            work_left: info.monitorInfo.rcWork.left,
            work_top: info.monitorInfo.rcWork.top,
            work_right: info.monitorInfo.rcWork.right,
            work_bottom: info.monitorInfo.rcWork.bottom,
        },
        dpi,
        is_primary: (info.monitorInfo.dwFlags & MONITORINFOF_PRIMARY) != 0,
    };

    monitors.push(monitor_info);
    BOOL(1)
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

fn get_monitor_dpi(hmonitor: HMONITOR) -> std::result::Result<u32, PlatformError> {
    unsafe {
        let mut dpi_x: u32 = 96;
        let mut dpi_y: u32 = 96;

        GetDpiForMonitor(hmonitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y)
            .map_err(|e| PlatformError::ApiError(e.to_string()))?;

        Ok(dpi_x)
    }
}

fn hash_string(s: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(s.as_bytes());
    let result = hasher.finalize();
    format!("{:x}", result)[..16].to_string()
}

/// PrintWindow FFI binding (not available in windows crate v0.61)
/// BOOL PrintWindow(HWND hwnd, HDC hdcBlt, UINT nFlags);
/// PW_RENDERFULLCONTENT = 0x00000002
#[link(name = "user32")]
extern "system" {
    fn PrintWindow(hwnd: HWND, hdc: HDC, flags: u32) -> BOOL;
}

/// Capture window thumbnail as base64 PNG using PrintWindow
/// PrintWindow asks the target window to render itself into our DC,
/// so it works even when the window is occluded by our own window.
pub fn capture_window_thumbnail(hwnd: WindowHandle, max_width: u32, max_height: u32) -> std::result::Result<String, PlatformError> {
    use windows::Win32::Graphics::Gdi::*;

    let hwnd_win: HWND = hwnd.into();

    unsafe {
        // Check if window is minimized
        if IsIconic(hwnd_win).as_bool() {
            return Err(PlatformError::ApiError("Window is minimized".to_string()));
        }

        let mut rect = RECT::default();
        GetWindowRect(hwnd_win, &mut rect)
            .map_err(|e| PlatformError::ApiError(e.to_string()))?;

        let width = (rect.right - rect.left) as u32;
        let height = (rect.bottom - rect.top) as u32;

        if width == 0 || height == 0 {
            return Err(PlatformError::ApiError("Window has zero size".to_string()));
        }

        // Scale down to fit within max dimensions
        let scale = (max_width as f32 / width as f32)
            .min(max_height as f32 / height as f32)
            .min(1.0);
        let thumb_w = (width as f32 * scale) as i32;
        let thumb_h = (height as f32 * scale) as i32;

        // Create a memory DC and bitmap for the thumbnail
        let screen_dc = GetDC(None);
        let mem_dc = CreateCompatibleDC(Some(screen_dc));
        let bitmap = CreateCompatibleBitmap(screen_dc, thumb_w, thumb_h);

        if bitmap.is_invalid() {
            let _ = DeleteDC(mem_dc);
            let _ = ReleaseDC(None, screen_dc);
            return Err(PlatformError::ApiError("Failed to create compatible bitmap".to_string()));
        }

        let old_bitmap = SelectObject(mem_dc, bitmap.into());

        // Use PrintWindow - asks the target window to render into our DC
        // PW_RENDERFULLCONTENT (0x00000002) renders full content including non-client area
        let print_result = PrintWindow(hwnd_win, mem_dc, 0x00000002);

        let mut png_data = Vec::new();
        if print_result.as_bool() {
            // Extract bitmap data
            let mut bi: BITMAPINFOHEADER = BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: thumb_w,
                biHeight: -thumb_h, // top-down
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                biSizeImage: 0,
                biXPelsPerMeter: 0,
                biYPelsPerMeter: 0,
                biClrUsed: 0,
                biClrImportant: 0,
            };

            let pixel_data_size = (thumb_w * thumb_h * 4) as usize;
            let mut pixel_data = vec![0u8; pixel_data_size];

            let lines = GetDIBits(
                mem_dc,
                bitmap,
                0,
                thumb_h as u32,
                Some(pixel_data.as_mut_ptr() as *mut _),
                &mut bi as *mut _ as *mut BITMAPINFO,
                DIB_RGB_COLORS,
            );

            if lines > 0 {
                // Convert BGRA to RGBA
                for chunk in pixel_data.chunks_exact_mut(4) {
                    chunk.swap(0, 2); // B <-> R
                }

                // Encode as PNG
                let img = image::RgbaImage::from_raw(thumb_w as u32, thumb_h as u32, pixel_data);
                if let Some(img) = img {
                    let mut buf = std::io::Cursor::new(Vec::new());
                    let _ = img.write_to(&mut buf, image::ImageFormat::Png);
                    png_data = buf.into_inner();
                }
            }
        }

        // Cleanup GDI objects
        SelectObject(mem_dc, old_bitmap);
        let _ = DeleteObject(bitmap.into());
        let _ = DeleteDC(mem_dc);
        let _ = ReleaseDC(None, screen_dc);

        if png_data.is_empty() {
            return Err(PlatformError::ApiError("Failed to capture thumbnail".to_string()));
        }

        use base64::Engine;
        let b64 = base64::engine::general_purpose::STANDARD.encode(&png_data);
        Ok(format!("data:image/png;base64,{}", b64))
    }
}

/// Get current cursor position
pub fn get_cursor_position() -> std::result::Result<(i32, i32), PlatformError> {
    use windows::Win32::UI::WindowsAndMessaging::*;

    unsafe {
        let mut point = POINT::default();
        GetCursorPos(&mut point).map_err(|e| PlatformError::ApiError(e.to_string()))?;
        Ok((point.x, point.y))
    }
}
