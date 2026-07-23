use crate::services::{
    DatabaseService, ProcessMonitorService, RecommendationEngine, WindowManagerService,
};
use crate::types::*;
use serde::{Deserialize, Serialize};
use tauri::{State, AppHandle};
use std::sync::Mutex;

#[derive(Debug, thiserror::Error)]
pub enum CommandError {
    #[error("Window manager error: {0}")]
    WindowManager(String),

    #[error("Monitor error: {0}")]
    Monitor(String),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Recommendation error: {0}")]
    Recommendation(String),
}

impl From<CommandError> for String {
    fn from(err: CommandError) -> Self {
        err.to_string()
    }
}

impl From<crate::platform::PlatformError> for CommandError {
    fn from(err: crate::platform::PlatformError) -> Self {
        CommandError::WindowManager(err.to_string())
    }
}

impl From<crate::services::process_monitor::MonitorError> for CommandError {
    fn from(err: crate::services::process_monitor::MonitorError) -> Self {
        CommandError::Monitor(err.to_string())
    }
}

impl From<crate::services::database::DatabaseError> for CommandError {
    fn from(err: crate::services::database::DatabaseError) -> Self {
        CommandError::Database(err.to_string())
    }
}

impl From<crate::services::recommendation::RecommendationError> for CommandError {
    fn from(err: crate::services::recommendation::RecommendationError) -> Self {
        CommandError::Recommendation(err.to_string())
    }
}

pub struct AppState {
    pub window_manager: Mutex<WindowManagerService>,
    pub process_monitor: Mutex<ProcessMonitorService>,
    pub database: Mutex<Option<DatabaseService>>,
    pub recommendation_engine: Mutex<RecommendationEngine>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            window_manager: Mutex::new(WindowManagerService::new()),
            process_monitor: Mutex::new(ProcessMonitorService::new()),
            database: Mutex::new(None),
            recommendation_engine: Mutex::new(RecommendationEngine::new()),
        }
    }
}

#[derive(Serialize)]
pub struct WindowInfoResponse {
    pub hwnd: i64,
    pub pid: u32,
    pub process_name: String,
    pub window_title_hash: String,
    pub monitor_id: i64,
    pub is_visible: bool,
}

#[derive(Serialize)]
pub struct MonitorInfoResponse {
    pub id: i64,
    pub name: String,
    pub is_primary: bool,
    pub dpi: u32,
}

#[tauri::command]
pub fn get_all_windows(state: State<AppState>) -> Result<Vec<WindowInfoResponse>, String> {
    let window_manager = state.window_manager.lock().map_err(|e| e.to_string())?;
    let windows = window_manager
        .get_all_windows()
        .map_err(|e| e.to_string())?;

    Ok(windows
        .into_iter()
        .map(|w| WindowInfoResponse {
            hwnd: w.hwnd.0,
            pid: w.pid,
            process_name: w.process_name,
            window_title_hash: w.window_title_hash,
            monitor_id: w.monitor_id.0,
            is_visible: w.is_visible,
        })
        .collect())
}

#[tauri::command]
pub fn get_all_monitors(state: State<AppState>) -> Result<Vec<MonitorInfoResponse>, String> {
    let window_manager = state.window_manager.lock().map_err(|e| e.to_string())?;
    let monitors = window_manager
        .get_all_monitors()
        .map_err(|e| e.to_string())?;

    Ok(monitors
        .into_iter()
        .map(|m| MonitorInfoResponse {
            id: m.id.0,
            name: m.name,
            is_primary: m.is_primary,
            dpi: m.dpi,
        })
        .collect())
}

/// 在指定显示器上显示临时标识浮窗（Win32 分层窗口，点击穿透，不阻塞操作）
#[tauri::command]
pub fn flash_monitor(
    _app: AppHandle,
    state: State<AppState>,
    monitor_id: i64,
) -> Result<(), String> {
    use windows::core::w;
    use windows::Win32::UI::WindowsAndMessaging::*;
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;

    let window_manager = state.window_manager.lock().map_err(|e| e.to_string())?;
    let monitors = window_manager
        .get_all_monitors()
        .map_err(|e| e.to_string())?;

    let target = monitors
        .iter()
        .find(|m| m.id.0 == monitor_id)
        .ok_or_else(|| "Monitor not found".to_string())?;

    let monitor_number = target
        .name
        .rsplit("DISPLAY")
        .next()
        .unwrap_or("?")
        .to_string();

    let is_primary = target.is_primary;
    let label_text = if is_primary {
        format!("显示器 {} (主)", monitor_number)
    } else {
        format!("显示器 {}", monitor_number)
    };

    let badge_w: i32 = 200;
    let badge_h: i32 = 64;
    let x = target.rect.work_left + 24;
    let y = target.rect.work_top + 24;

    // 在独立线程中创建窗口，避免与主线程 WebView2 消息循环冲突
    std::thread::spawn(move || {
        let _ = std::panic::catch_unwind(move || {
            unsafe {
                let _ = windows::Win32::System::Com::CoInitializeEx(
                    None,
                    windows::Win32::System::Com::COINIT_APARTMENTTHREADED,
                );

                let class_name = w!("WFFlashBadge2");
                let hinstance = GetModuleHandleW(None).unwrap_or_default();

                // 注册带窗口过程的窗口类
                let wc = WNDCLASSEXW {
                    cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                    hInstance: hinstance.into(),
                    lpszClassName: class_name,
                    lpfnWndProc: Some(badge_wnd_proc),
                    hbrBackground: windows::Win32::Graphics::Gdi::HBRUSH::default(),
                    ..Default::default()
                };
                let reg_result = RegisterClassExW(&wc);
                if reg_result == 0 {
                    let err = windows::Win32::Foundation::GetLastError();
                    if err.0 != 1410 { // ERROR_CLASS_ALREADY_EXISTS
                        log::error!("RegisterClassExW failed: {:?}", err);
                        let _ = windows::Win32::System::Com::CoUninitialize();
                        return;
                    }
                }

                let hwnd = CreateWindowExW(
                    WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE,
                    class_name,
                    w!(""),
                    WS_POPUP,
                    x, y, badge_w, badge_h,
                    None, None, Some(hinstance.into()), None,
                );

                if hwnd.is_err() {
                    log::error!("Failed to create flash badge window");
                    let _ = windows::Win32::System::Com::CoUninitialize();
                    return;
                }
                let hwnd = hwnd.unwrap();
                log::info!("Created flash badge window {:?} at ({}, {})", hwnd, x, y);

                if let Err(e) = draw_badge(hwnd, &label_text, badge_w, badge_h, x, y) {
                    log::error!("draw_badge failed: {}", e);
                    let _ = DestroyWindow(hwnd);
                    let _ = UnregisterClassW(class_name, Some(hinstance.into()));
                    let _ = windows::Win32::System::Com::CoUninitialize();
                    return;
                }

                // 确保分层窗口可见
                let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
                log::info!("Flash badge window shown at ({}, {})", x, y);

                // 后台线程 2 秒后发送 WM_CLOSE
                let hwnd_raw = hwnd.0 as isize;
                std::thread::spawn(move || {
                    use windows::Win32::Foundation::{HWND, WPARAM, LPARAM};
                    use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_CLOSE};
                    std::thread::sleep(std::time::Duration::from_secs(2));
                    let h = HWND(hwnd_raw as *mut _);
                    log::info!("Sending WM_CLOSE to flash badge {:?}", h);
                    let _ = PostMessageW(Some(h), WM_CLOSE, WPARAM(0), LPARAM(0));
                });

                // 消息循环 - 关键：使用具体 hwnd 而非 None，避免窃取主线程消息
                let mut msg = MSG::default();
                while GetMessageW(&mut msg, Some(hwnd), 0, 0).as_bool() {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }

                log::info!("Flash badge message loop ended for {:?}", hwnd);
                let _ = UnregisterClassW(class_name, Some(hinstance.into()));
                let _ = windows::Win32::System::Com::CoUninitialize();
            }
        });
    });

    Ok(())
}

/// 浮窗窗口过程：处理 WM_DESTROY 退出消息循环
unsafe extern "system" fn badge_wnd_proc(
    hwnd: windows::Win32::Foundation::HWND,
    msg: u32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    use windows::Win32::UI::WindowsAndMessaging::*;
    match msg {
        WM_DESTROY => {
            PostQuitMessage(0);
            windows::Win32::Foundation::LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

/// 绘制标识浮窗内容（32位位图 + 文字），返回 Result 避免崩溃
unsafe fn draw_badge(hwnd: windows::Win32::Foundation::HWND, text: &str, w: i32, h: i32, pos_x: i32, pos_y: i32) -> Result<(), String> {
    use windows::core::w;
    use windows::Win32::Foundation::{HWND, RECT, POINT, SIZE, COLORREF};
    use windows::Win32::Graphics::Gdi::*;
    use windows::Win32::UI::WindowsAndMessaging::UpdateLayeredWindow;
    use windows::Win32::UI::WindowsAndMessaging::ULW_ALPHA;

    // 创建 32 位位图（支持 alpha 通道）
    let hdc_screen = GetDC(Some(HWND::default()));
    if hdc_screen.is_invalid() {
        return Err("GetDC failed".to_string());
    }
    let hdc_mem = CreateCompatibleDC(Some(hdc_screen));
    if hdc_mem.is_invalid() {
        ReleaseDC(Some(HWND::default()), hdc_screen);
        return Err("CreateCompatibleDC failed".to_string());
    }

    let bmi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: w,
            biHeight: -h,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            biSizeImage: 0,
            biXPelsPerMeter: 0,
            biYPelsPerMeter: 0,
            biClrUsed: 0,
            biClrImportant: 0,
        },
        ..Default::default()
    };

    let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
    let hbmp_result = CreateDIBSection(Some(hdc_mem), &bmi, DIB_RGB_COLORS, &mut bits, None, 0);
    if hbmp_result.is_err() || bits.is_null() {
        DeleteDC(hdc_mem);
        ReleaseDC(Some(HWND::default()), hdc_screen);
        return Err("CreateDIBSection failed".to_string());
    }
    let hbmp = hbmp_result.unwrap();

    let old_bmp = SelectObject(hdc_mem, hbmp.into());

    // 填充位图数据（BGRA 格式）- 使用预乘 alpha 值
    let pixels = std::slice::from_raw_parts_mut(bits as *mut u32, (w * h) as usize);
    let bg_color: u32 = 0xE61B1B1B;
    let bg_r = 0x1B_u32;
    let bg_g = 0x1B_u32;
    let bg_b = 0x1B_u32;

    for pixel in pixels.iter_mut() {
        *pixel = bg_color;
    }

    // 使用 GDI 绘制文字
    let font = CreateFontW(
        28, 0, 0, 0, FW_BOLD.0 as i32,
        0, 0, 0, DEFAULT_CHARSET,
        OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS,
        DEFAULT_QUALITY,
        (DEFAULT_PITCH.0 | FF_DONTCARE.0) as u32,
        w!("Microsoft YaHei"),
    );
    if font.is_invalid() {
        let _ = SelectObject(hdc_mem, old_bmp);
        let _ = DeleteObject(hbmp.into());
        let _ = DeleteDC(hdc_mem);
        let _ = ReleaseDC(Some(HWND::default()), hdc_screen);
        return Err("CreateFontW failed".to_string());
    }
    let old_font = SelectObject(hdc_mem, font.into());
    SetTextColor(hdc_mem, COLORREF(0x00FFFFFF));
    SetBkMode(hdc_mem, TRANSPARENT);

    let mut text_w: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let mut text_rect = RECT { left: 8, top: 0, right: w - 8, bottom: h };
    DrawTextW(hdc_mem, &mut text_w, &mut text_rect, DT_CENTER | DT_VCENTER | DT_SINGLELINE);

    let _ = SelectObject(hdc_mem, old_font);
    let _ = DeleteObject(font.into());

    // 修复 alpha 通道并使用预乘 alpha 格式
    for pixel in pixels.iter_mut() {
        let r = (*pixel >> 16) & 0xFF;
        let g = (*pixel >> 8) & 0xFF;
        let b = *pixel & 0xFF;

        let diff_r = if r > bg_r { r - bg_r } else { bg_r - r };
        let diff_g = if g > bg_g { g - bg_g } else { bg_g - g };
        let diff_b = if b > bg_b { b - bg_b } else { bg_b - b };
        let total_diff = diff_r + diff_g + diff_b;

        let alpha: u32 = if total_diff > 30 { 255 } else { 0xE6 };

        let pre_r = (r * alpha / 255) & 0xFF;
        let pre_g = (g * alpha / 255) & 0xFF;
        let pre_b = (b * alpha / 255) & 0xFF;

        *pixel = (alpha << 24) | (pre_b << 16) | (pre_g << 8) | pre_r;
    }

    // 通过 UpdateLayeredWindow 显示分层窗口
    let pt_src = POINT { x: 0, y: 0 };
    let size = SIZE { cx: w, cy: h };
    let pt_dst = POINT { x: pos_x, y: pos_y };
    
    let blend = BLENDFUNCTION {
        BlendOp: AC_SRC_OVER as u8,
        BlendFlags: 0,
        SourceConstantAlpha: 255,
        AlphaFormat: AC_SRC_ALPHA as u8,
    };

    let result = UpdateLayeredWindow(
        hwnd,
        None,
        Some(&pt_dst),
        Some(&size),
        Some(hdc_mem),
        Some(&pt_src),
        COLORREF(0),
        Some(&blend),
        ULW_ALPHA,
    );
    
    let _ = SelectObject(hdc_mem, old_bmp);
    let _ = DeleteObject(hbmp.into());
    let _ = DeleteDC(hdc_mem);
    let _ = ReleaseDC(Some(HWND::default()), hdc_screen);

    if result.is_err() {
        return Err(format!("UpdateLayeredWindow failed: {:?}", result.err()));
    }
    
    Ok(())
}

#[derive(Deserialize)]
pub struct MigrateWindowRequest {
    pub hwnd: i64,
    pub target_monitor_id: i64,
}

#[tauri::command]
pub async fn migrate_window(
    _state: State<'_, AppState>,
    request: MigrateWindowRequest,
) -> Result<(), String> {
    let hwnd = WindowHandle(request.hwnd);
    let target = MonitorId(request.target_monitor_id);

    // 使用 spawn_blocking 避免阻塞 Tauri 事件循环
    let result = tokio::task::spawn_blocking(move || {
        let window_manager = WindowManagerService::new();
        window_manager.migrate_window(hwnd, target)
    })
    .await
    .map_err(|e| format!("Migration task failed: {}", e))?;

    result.map_err(|e| e.to_string())?;

    Ok(())
}

#[derive(Deserialize)]
pub struct MigrateWindowsRequest {
    pub hwnds: Vec<i64>,
    pub target_monitor_id: i64,
}

#[tauri::command]
pub async fn migrate_windows(
    _state: State<'_, AppState>,
    request: MigrateWindowsRequest,
) -> Result<String, String> {
    let hwnds = request.hwnds;
    let target = MonitorId(request.target_monitor_id);

    let result = tokio::task::spawn_blocking(move || {
        let window_manager = WindowManagerService::new();
        let mut success_count = 0;
        let mut fail_count = 0;

        for &hwnd_raw in &hwnds {
            let hwnd = WindowHandle(hwnd_raw);
            match window_manager.migrate_window(hwnd, target) {
                Ok(_) => {
                    log::info!("migrate_window: hwnd={} success", hwnd_raw);
                    success_count += 1;
                }
                Err(e) => {
                    log::warn!("migrate_window: hwnd={} failed: {}", hwnd_raw, e);
                    fail_count += 1;
                }
            }
        }

        (success_count, fail_count)
    })
    .await
    .map_err(|e| format!("Migration task failed: {}", e))?;

    let (success_count, fail_count) = result;
    let summary = format!("迁移完成: {} 成功, {} 失败", success_count, fail_count);
    log::info!("{}", summary);
    Ok(summary)
}

#[tauri::command]
pub fn start_monitor(_state: State<AppState>) -> Result<(), String> {
    // 现在使用前端 polling 方式，无需启动后台线程
    Ok(())
}

#[tauri::command]
pub fn stop_monitor(_state: State<AppState>) -> Result<(), String> {
    // 现在使用前端 polling 方式，无需停止后台线程
    Ok(())
}

#[tauri::command]
pub fn get_recent_events(
    state: State<AppState>,
    minutes: u32,
) -> Result<Vec<WindowInfoResponse>, String> {
    let monitor = state.process_monitor.lock().map_err(|e| e.to_string())?;
    let events = monitor
        .get_recent_events(minutes)
        .map_err(|e| e.to_string())?;

    Ok(events
        .into_iter()
        .map(|e| WindowInfoResponse {
            hwnd: e.hwnd.0,
            pid: e.pid,
            process_name: e.process_name,
            window_title_hash: e.window_title_hash,
            monitor_id: e.monitor_id.0,
            is_visible: true,
        })
        .collect())
}

#[tauri::command]
pub fn init_database(state: State<AppState>, db_path: String) -> Result<(), String> {
    let db = DatabaseService::new(&db_path).map_err(|e| e.to_string())?;
    let mut db_opt = state.database.lock().map_err(|e| e.to_string())?;
    *db_opt = Some(db);

    Ok(())
}

#[tauri::command]
pub fn save_events_to_database(state: State<AppState>) -> Result<u64, String> {
    let monitor = state.process_monitor.lock().map_err(|e| e.to_string())?;
    let events = monitor.get_all_events();

    let db_opt = state.database.lock().map_err(|e| e.to_string())?;
    if let Some(db) = db_opt.as_ref() {
        let mut count: u64 = 0;
        for event in events {
            db.insert_focus_event(&event).map_err(|e| e.to_string())?;
            count += 1;
        }
        Ok(count)
    } else {
        Err("Database not initialized".to_string())
    }
}

#[tauri::command]
pub fn get_recommendations(
    state: State<AppState>,
    current_hwnd: i64,
    max_count: u32,
) -> Result<RecommendationResponse, String> {
    let monitor = state.process_monitor.lock().map_err(|e| e.to_string())?;
    let events = monitor.get_all_events();
    let mouse_events = monitor.get_all_mouse_events();
    let destroyed_hwnds = monitor.get_destroyed_hwnds();
    drop(monitor); // 释放锁

    // 优先使用指定的 hwnd，找不到则使用最近的事件
    let current_event = events
        .iter()
        .rev()
        .find(|e| e.hwnd.0 == current_hwnd)
        .or_else(|| events.last())
        .ok_or_else(|| {
            format!("No events available (total events: {})", events.len())
        })?;

    let mut engine = state
        .recommendation_engine
        .lock()
        .map_err(|e| e.to_string())?;
    engine.update_from_events(&events);

    let response = engine
        .generate_recommendations(current_event, max_count as usize, &mouse_events, &destroyed_hwnds)
        .map_err(|e| e.to_string())?;

    Ok(response)
}

/// 调试：获取监控器状态和事件数量
#[tauri::command]
pub fn get_monitor_status(state: State<AppState>) -> Result<serde_json::Value, String> {
    let monitor = state.process_monitor.lock().map_err(|e| e.to_string())?;
    let events = monitor.get_all_events();
    let recent = monitor.get_recent_events(5).unwrap_or_default();

    log::info!("get_monitor_status: total_events={}", events.len());

    Ok(serde_json::json!({
        "total_events": events.len(),
        "recent_5min": recent.len(),
        "is_running": true,
        "events": events.iter().rev().take(10).map(|e| {
            serde_json::json!({
                "hwnd": e.hwnd.0,
                "process_name": e.process_name,
                "timestamp": e.timestamp.to_rfc3339(),
            })
        }).collect::<Vec<_>>(),
    }))
}

/// 前端定时调用，检查并记录焦点窗口变化
#[tauri::command]
pub fn poll_focus_changes(state: State<AppState>) -> Result<usize, String> {
    let monitor = state.process_monitor.lock().map_err(|e| e.to_string())?;
    monitor.poll_once();
    Ok(monitor.get_all_events().len())
}

/// 获取最近的焦点事件（用于前端推荐板块展示）
#[tauri::command]
pub fn get_recent_focus_events(
    state: State<AppState>,
    limit: u32,
) -> Result<Vec<serde_json::Value>, String> {
    let monitor = state.process_monitor.lock().map_err(|e| e.to_string())?;
    let events = monitor.get_all_events();

    let recent: Vec<_> = events
        .iter()
        .rev()
        .take(limit as usize)
        .map(|e| {
            serde_json::json!({
                "hwnd": e.hwnd.0,
                "pid": e.pid,
                "process_name": e.process_name,
                "window_title_hash": e.window_title_hash,
                "timestamp": e.timestamp.to_rfc3339(),
                "monitor_id": e.monitor_id.0,
            })
        })
        .collect();

    Ok(recent)
}

#[tauri::command]
pub fn get_usage_stats(
    state: State<AppState>,
    days: u32,
) -> Result<Vec<serde_json::Value>, String> {
    let db_opt = state.database.lock().map_err(|e| e.to_string())?;
    if let Some(db) = db_opt.as_ref() {
        let stats = db.get_usage_stats(days).map_err(|e| e.to_string())?;
        Ok(stats
            .into_iter()
            .map(|s| {
                serde_json::json!({
                    "process_name": s.process_name,
                    "focus_count": s.focus_count,
                    "last_used": s.last_used.to_rfc3339(),
                })
            })
            .collect())
    } else {
        Err("Database not initialized".to_string())
    }
}

#[tauri::command]
pub fn cleanup_old_data(state: State<AppState>, retention_days: u32) -> Result<u64, String> {
    let db_opt = state.database.lock().map_err(|e| e.to_string())?;
    if let Some(db) = db_opt.as_ref() {
        let deleted = db.cleanup_old_data(retention_days).map_err(|e| e.to_string())?;
        Ok(deleted)
    } else {
        Err("Database not initialized".to_string())
    }
}

#[tauri::command]
pub fn get_window_thumbnail(hwnd: i64, max_width: u32, max_height: u32) -> Result<String, String> {
    // [T5] 经缓存层获取：命中未过期缓存则复用，否则重新捕获并写入外置缓存目录
    crate::services::thumbnail_cache::get_or_capture(hwnd, max_width, max_height)
}

#[derive(Serialize)]
pub struct CursorPositionResponse {
    pub x: i32,
    pub y: i32,
}

#[derive(Serialize)]
pub struct MonitorIdResponse {
    pub monitor_id: i64,
}

#[tauri::command]
pub fn get_cursor_position() -> Result<CursorPositionResponse, String> {
    let (x, y) = crate::platform::windows::get_cursor_position()
        .map_err(|e| e.to_string())?;
    Ok(CursorPositionResponse { x, y })
}

/// 获取当前窗口所在的显示器 ID
#[tauri::command]
pub fn get_current_window_monitor(app: tauri::AppHandle) -> Result<MonitorIdResponse, String> {
    use crate::platform::windows::WindowsPlatform;
    use tauri::Manager;
    
    // 获取主窗口
    let window = app.get_webview_window("main")
        .ok_or("Main window not found")?;
    
    // 获取窗口句柄
    let hwnd_raw = window.hwnd().map_err(|e| e.to_string())?;
    let hwnd = WindowHandle(hwnd_raw.0 as i64);
    
    // 获取所在显示器
    let platform = WindowsPlatform::new();
    let monitor_id = platform.get_window_monitor(hwnd)
        .map_err(|e| e.to_string())?;
    
    log::info!("Current window is on monitor: {:?}", monitor_id);
    
    Ok(MonitorIdResponse { monitor_id: monitor_id.0 })
}

/// 智能迁移窗口到目标显示器（处理最小化窗口）
#[tauri::command]
pub fn smart_migrate_windows(
    state: State<AppState>,
    request: MigrateWindowsRequest,
) -> Result<Vec<crate::services::window_manager::SmartMigrateResult>, String> {
    let window_manager = state.window_manager.lock().map_err(|e| e.to_string())?;
    let target = MonitorId(request.target_monitor_id);

    log::info!(
        "smart_migrate_windows: hwnds={:?}, target_monitor_id={}",
        request.hwnds,
        request.target_monitor_id
    );

    let hwnds: Vec<WindowHandle> = request.hwnds.into_iter().map(WindowHandle).collect();
    let results = window_manager
        .smart_migrate_windows(&hwnds, target)
        .map_err(|e| e.to_string())?;

    let success_count = results.iter().filter(|r| r.success).count();
    let fail_count = results.len() - success_count;
    log::info!("smart_migrate completed: {} success, {} failed", success_count, fail_count);

    Ok(results)
}

/// 获取窗口统计数据（用于设置界面的忽略清单选择）
#[tauri::command]
pub fn get_window_stats(state: State<AppState>) -> Result<Vec<WindowStats>, String> {
    let monitor = state.process_monitor.lock().map_err(|e| e.to_string())?;
    let events = monitor.get_all_events();

    let mut engine = state
        .recommendation_engine
        .lock()
        .map_err(|e| e.to_string())?;
    engine.update_from_events(&events);

    let settings = engine.get_settings().clone();
    let ignore_set: std::collections::HashSet<&str> = settings.ignore_list.iter().map(|s| s.as_str()).collect();

    // 统计每个进程的数据
    let mut process_stats: std::collections::HashMap<String, WindowStats> = std::collections::HashMap::new();

    for event in &events {
        let process_name = &event.process_name;
        let stats = process_stats.entry(process_name.clone()).or_insert_with(|| WindowStats {
            process_name: process_name.clone(),
            total_dwell_secs: 0,
            effective_dwell_secs: 0,
            switch_count: 0,
            is_ignored: ignore_set.contains(process_name.as_str()),
        });
        stats.switch_count += 1;
    }

    // 计算停留时间（简化版本，使用事件时间戳差值）
    for (process_name, stats) in &mut process_stats {
        let process_events: Vec<_> = events
            .iter()
            .filter(|e| &e.process_name == process_name)
            .collect();

        if process_events.len() >= 2 {
            let total_dwell: u64 = process_events
                .windows(2)
                .map(|w| {
                    let diff = w[1].timestamp - w[0].timestamp;
                    diff.num_seconds().max(0) as u64
                })
                .sum();
            stats.total_dwell_secs = total_dwell;
            stats.effective_dwell_secs = total_dwell; // 简化处理，暂不计算鼠标静止时间
        }
    }

    let mut result: Vec<WindowStats> = process_stats.into_values().collect();
    result.sort_by(|a, b| b.total_dwell_secs.cmp(&a.total_dwell_secs));

    Ok(result)
}

/// 获取推荐设置
#[tauri::command]
pub fn get_recommendation_settings(state: State<AppState>) -> Result<RecommendationSettings, String> {
    let engine = state
        .recommendation_engine
        .lock()
        .map_err(|e| e.to_string())?;
    Ok(engine.get_settings().clone())
}

/// 更新推荐设置
#[tauri::command]
pub fn update_recommendation_settings(
    state: State<AppState>,
    settings: RecommendationSettings,
) -> Result<(), String> {
    let mut engine = state
        .recommendation_engine
        .lock()
        .map_err(|e| e.to_string())?;
    engine.update_settings(settings);
    Ok(())
}

/// 动态更新全局快捷键
#[tauri::command]
pub fn update_hotkey(
    app: tauri::AppHandle,
    new_hotkey: String,
) -> Result<(), String> {
    use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState, Modifiers, Code};

    // 解析新快捷键（支持 Ctrl/Shift/Alt + Key 格式）
    let parts: Vec<&str> = new_hotkey.split('+').collect();
    if parts.len() < 2 {
        return Err("快捷键格式错误，请使用如 Ctrl+Shift+Key 格式".to_string());
    }

    let mut modifiers = Modifiers::empty();
    let key = parts.last().unwrap();

    for &part in &parts[..parts.len() - 1] {
        match part.to_lowercase().as_str() {
            "ctrl" => modifiers |= Modifiers::CONTROL,
            "shift" => modifiers |= Modifiers::SHIFT,
            "alt" => modifiers |= Modifiers::ALT,
            _ => return Err(format!("不支持的修饰键: {}", part)),
        }
    }

    // 映射键名到 Code
    let code = match key.to_lowercase().as_str() {
        "space" => Code::Space,
        "a" => Code::KeyA, "b" => Code::KeyB, "c" => Code::KeyC, "d" => Code::KeyD,
        "e" => Code::KeyE, "f" => Code::KeyF, "g" => Code::KeyG, "h" => Code::KeyH,
        "i" => Code::KeyI, "j" => Code::KeyJ, "k" => Code::KeyK, "l" => Code::KeyL,
        "m" => Code::KeyM, "n" => Code::KeyN, "o" => Code::KeyO, "p" => Code::KeyP,
        "q" => Code::KeyQ, "r" => Code::KeyR, "s" => Code::KeyS, "t" => Code::KeyT,
        "u" => Code::KeyU, "v" => Code::KeyV, "w" => Code::KeyW, "x" => Code::KeyX,
        "y" => Code::KeyY, "z" => Code::KeyZ,
        "0" => Code::Digit0, "1" => Code::Digit1, "2" => Code::Digit2, "3" => Code::Digit3,
        "4" => Code::Digit4, "5" => Code::Digit5, "6" => Code::Digit6, "7" => Code::Digit7,
        "8" => Code::Digit8, "9" => Code::Digit9,
        "f1" => Code::F1, "f2" => Code::F2, "f3" => Code::F3, "f4" => Code::F4,
        "f5" => Code::F5, "f6" => Code::F6, "f7" => Code::F7, "f8" => Code::F8,
        "f9" => Code::F9, "f10" => Code::F10, "f11" => Code::F11, "f12" => Code::F12,
        _ => return Err(format!("不支持的键: {}", key)),
    };

    // 注销当前所有已注册的快捷键（使用 unregister_all 确保清理干净）
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();
    log::info!("Unregistered all hotkeys");

    // 递增代数，使旧处理器失效（它们会检查代数是否匹配）
    use std::sync::atomic::Ordering;
    let new_gen = crate::SHORTCUT_GENERATION.fetch_add(1, Ordering::Relaxed) + 1;
    log::info!("Incremented shortcut generation to {}", new_gen);

    // 注册新快捷键
    let new_shortcut = Shortcut::new(Some(modifiers), code);
    let app_handle = app.clone();
    let hotkey_label = new_hotkey.clone();
    let _ = gs.on_shortcut(new_shortcut, move |_app, _shortcut, event| {
        if event.state() == ShortcutState::Pressed {
            // 检查代数是否匹配，避免旧处理器导致闪回
            let current_gen = crate::SHORTCUT_GENERATION.load(Ordering::Relaxed);
            if current_gen == new_gen {
                log::info!("Global shortcut triggered: {} (gen={})", hotkey_label, new_gen);
                // 使用统一的 toggle_panel 函数，确保防抖机制生效
                crate::toggle_panel(&app_handle);
            } else {
                log::debug!("Ignoring shortcut from old handler (gen={} vs current={})", new_gen, current_gen);
            }
        }
    });

    log::info!("Registered new hotkey: {} (gen={})", new_hotkey, new_gen);
    Ok(())
}

/// 更新鼠标侧键设置
#[tauri::command]
pub fn update_mouse_side_button(
    enabled: bool,
    xbutton1: bool,
    xbutton2: bool,
) -> Result<(), String> {
    // 调用 lib.rs 中的更新函数
    crate::update_mouse_config(enabled, xbutton1, xbutton2);
    log::info!("Mouse side button config updated: enabled={}, xbutton1={}, xbutton2={}", enabled, xbutton1, xbutton2);
    Ok(())
}

/// 保存推荐数据到数据库
#[tauri::command]
pub fn save_recommendation_data(state: State<AppState>) -> Result<(), String> {
    let db_opt = state.database.lock().map_err(|e| e.to_string())?;
    let db = db_opt.as_ref().ok_or("Database not initialized")?;

    let engine = state.recommendation_engine.lock().map_err(|e| e.to_string())?;
    engine.save_to_database(db).map_err(|e| e.to_string())?;

    log::info!("Recommendation data saved to database");
    Ok(())
}

/// 从数据库加载推荐数据
#[tauri::command]
pub fn load_recommendation_data(state: State<AppState>) -> Result<(), String> {
    let db_opt = state.database.lock().map_err(|e| e.to_string())?;
    let db = db_opt.as_ref().ok_or("Database not initialized")?;

    let mut engine = state.recommendation_engine.lock().map_err(|e| e.to_string())?;
    engine.load_from_database(db).map_err(|e| e.to_string())?;

    log::info!("Recommendation data loaded from database");
    Ok(())
}

/// 清理已销毁窗口的事件
#[tauri::command]
pub fn cleanup_destroyed_events(state: State<AppState>) -> Result<u64, String> {
    let monitor = state.process_monitor.lock().map_err(|e| e.to_string())?;
    let destroyed_hwnds = monitor.get_destroyed_hwnds();
    let count = destroyed_hwnds.len() as u64;

    if count > 0 {
        monitor.cleanup_destroyed_events();
        log::info!("Cleaned up {} destroyed window events", count);
    }

    Ok(count)
}

/// 清理旧的推荐数据（dwell_records 和 co_occurrence）
#[tauri::command]
pub fn cleanup_old_recommendation_data(
    state: State<AppState>,
    retention_days: u32,
) -> Result<u64, String> {
    let db_opt = state.database.lock().map_err(|e| e.to_string())?;
    let db = db_opt.as_ref().ok_or("Database not initialized")?;

    let dwell_deleted = db.cleanup_old_dwell_records(retention_days).map_err(|e| e.to_string())?;
    let co_deleted = db.cleanup_old_co_occurrence(retention_days).map_err(|e| e.to_string())?;

    let total = dwell_deleted + co_deleted;
    log::info!("Cleaned up {} old recommendation records (dwell: {}, co-occurrence: {})",
        total, dwell_deleted, co_deleted);

    Ok(total)
}

/// 查询 dwell_records 数据
#[tauri::command]
pub fn get_dwell_records(state: State<AppState>) -> Result<Vec<DwellRecordRow>, String> {
    let db_opt = state.database.lock().map_err(|e| e.to_string())?;
    let db = db_opt.as_ref().ok_or("Database not initialized")?;

    let records = db.get_dwell_records().map_err(|e| e.to_string())?;
    Ok(records)
}

/// 查询 co_occurrence 数据
#[tauri::command]
pub fn get_co_occurrence_data(state: State<AppState>) -> Result<Vec<CoOccurrenceRow>, String> {
    let db_opt = state.database.lock().map_err(|e| e.to_string())?;
    let db = db_opt.as_ref().ok_or("Database not initialized")?;

    let records = db.get_co_occurrence().map_err(|e| e.to_string())?;
    Ok(records)
}

/// 在线模型推荐响应
#[derive(Serialize)]
pub struct OnlineModelRecommendationResponse {
    pub scenario_type: String,
    pub window_combinations: Vec<String>,
    pub confidence_score: f64,
}

/// 获取在线模型推荐（异步命令，第三个推荐列表）
#[tauri::command]
pub async fn get_online_model_recommendations(
    state: State<'_, AppState>,
    max_windows: u32,
) -> Result<Vec<OnlineModelRecommendationResponse>, String> {
    // 获取设置（在作用域内完成所有锁操作，避免 MutexGuard 跨 await）
    let (api_key, api_endpoint, model_name, recent_max_dwell_secs) = {
        let engine = state.recommendation_engine.lock().map_err(|e| e.to_string())?;
        let settings = engine.get_settings().clone();
        let api_key = settings.api_key.clone().ok_or("API Key 未配置")?;
        let api_endpoint = settings.api_endpoint.clone().ok_or("API 端点未配置")?;
        let model_name = settings.model_name.clone().ok_or("模型名称未配置")?;
        let recent_max_dwell_secs = settings.recent_max_dwell_secs;
        (api_key, api_endpoint, model_name, recent_max_dwell_secs)
    };

    // 检查 API 配置
    if api_key.is_empty() || api_endpoint.is_empty() || model_name.is_empty() {
        return Err("在线模型配置不完整，请在设置中填写 API Key、端点和模型名称".to_string());
    }

    // 获取数据库（DatabaseService 内部是 Arc，clone 是廉价的）
    let db = {
        let db_opt = state.database.lock().map_err(|e| e.to_string())?;
        db_opt.as_ref().ok_or("Database not initialized")?.clone()
    };

    // 准备数据：只取最近7天的数据
    let events = {
        let monitor = state.process_monitor.lock().map_err(|e| e.to_string())?;
        monitor.get_all_events()
    };

    let one_week_ago = chrono::Utc::now() - chrono::TimeDelta::days(7);

    // 构建 dwell_records: (hwnd, process_name) -> total_dwell_secs
    let mut dwell_map: std::collections::HashMap<(i64, String), u64> = std::collections::HashMap::new();
    let sorted_events: Vec<_> = events.iter().filter(|e| e.timestamp > one_week_ago).collect();
    for w in sorted_events.windows(2) {
        let key = (w[0].hwnd.0, w[0].process_name.clone());
        let secs = (w[1].timestamp - w[0].timestamp).num_seconds().max(0) as u64;
        *dwell_map.entry(key).or_insert(0) += secs;
    }

    // 构建 co_occurrence: (app1, app2) -> count（30分钟窗口）
    let mut co_map: std::collections::HashMap<(String, String), u32> = std::collections::HashMap::new();
    let window = chrono::Duration::minutes(30);
    let mut i = 0;
    while i < sorted_events.len() {
        let current = sorted_events[i];
        let mut window_apps: std::collections::HashSet<String> = std::collections::HashSet::new();
        window_apps.insert(current.process_name.clone());
        let mut j = i + 1;
        while j < sorted_events.len() {
            let time_diff = sorted_events[j].timestamp - current.timestamp;
            if time_diff > window { break; }
            if sorted_events[j].process_name != current.process_name {
                window_apps.insert(sorted_events[j].process_name.clone());
            }
            j += 1;
        }
        let apps: Vec<String> = window_apps.into_iter().collect();
        for a in 0..apps.len() {
            for b in (a + 1)..apps.len() {
                let key = if apps[a] < apps[b] {
                    (apps[a].clone(), apps[b].clone())
                } else {
                    (apps[b].clone(), apps[a].clone())
                };
                *co_map.entry(key).or_insert(0) += 1;
            }
        }
        i = j.max(i + 1);
    }

    // 构建 frequent_switches: process_name -> switch_count
    let mut switch_map: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
    for ((_, process_name), secs) in dwell_map.iter() {
        if *secs < recent_max_dwell_secs {
            *switch_map.entry(process_name.clone()).or_insert(0) += 1;
        }
    }

    if dwell_map.is_empty() {
        return Err("暂无足够的窗口使用数据".to_string());
    }

    // 调用在线模型
    let service = crate::services::OnlineModelService::new(
        api_key.clone(),
        api_endpoint.clone(),
        model_name.clone(),
    );

    let recommendations = service
        .analyze_and_recommend(&dwell_map, &co_map, &switch_map)
        .await
        .map_err(|e| {
            log::error!("Online model call failed: {}", e);
            format!("在线模型调用失败: {}", e)
        })?;

    // 递增调用计数
    let call_count = db.increment_model_call_count().map_err(|e| e.to_string())?;
    log::info!("Model call count: {}", call_count);

    // 保存推荐结果到数据库
    for rec in &recommendations {
        let windows_json = serde_json::to_string(&rec.window_combinations).unwrap_or_default();
        let _ = db.save_model_recommendation(
            &rec.scenario_type,
            &windows_json,
            rec.confidence_score,
        );
    }

    // 滚动清理：每 2 次调用后清理前一次的输入数据（dwell_records + co_occurrence）
    // 只保留模型判断结果（model_recommendations）供后续排查
    if call_count % 2 == 0 {
        log::info!("Rolling cleanup: clearing dwell_records and co_occurrence (call_count={})", call_count);
        let _ = db.cleanup_old_dwell_records(0);
        let _ = db.cleanup_old_co_occurrence(0);
        // 只保留最近一次模型推荐结果
        let _ = db.cleanup_old_model_recommendations(2);
        let _ = db.reset_model_call_count();
        log::info!("Rolling cleanup completed");
    }

    // 构建响应，限制窗口数量
    let max_w = max_windows as usize;
    let response: Vec<OnlineModelRecommendationResponse> = recommendations
        .into_iter()
        .take(2)
        .map(|r| {
            let mut windows = r.window_combinations;
            windows.truncate(max_w);
            OnlineModelRecommendationResponse {
                scenario_type: r.scenario_type,
                window_combinations: windows,
                confidence_score: r.confidence_score,
            }
        })
        .collect();

    Ok(response)
}
