mod types;
mod platform;
mod services;
mod commands;

use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Manager, Emitter, State,
};
use commands::AppState;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use tauri_plugin_global_shortcut::{Shortcut, Modifiers, Code};

/// 快捷键处理器代数（用于使旧处理器失效）
pub(crate) static SHORTCUT_GENERATION: AtomicU8 = AtomicU8::new(1);

/// 切换面板显示/隐藏（使用原子锁+冷却期，彻底避免闪回）
pub(crate) fn toggle_panel(app: &tauri::AppHandle) {
    use std::sync::atomic::AtomicBool;
    use std::sync::Mutex;
    use std::time::Instant;

    // 原子锁：防止并发切换
    static IS_TOGGLING: AtomicBool = AtomicBool::new(false);
    // 冷却期：防止 500ms 内重复触发
    static LAST_TOGGLE: OnceLock<Mutex<Instant>> = OnceLock::new();

    // 1. 尝试获取原子锁，失败则跳过
    if IS_TOGGLING
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        log::debug!("Toggle panel skipped (already toggling)");
        return;
    }

    // 2. 检查冷却期
    let last_toggle = LAST_TOGGLE.get_or_init(|| Mutex::new(Instant::now()));
    let cooldown_ok = if let Ok(mut last) = last_toggle.try_lock() {
        let elapsed = last.elapsed().as_millis();
        if elapsed < 500 {
            log::debug!("Toggle panel skipped (cooldown, {}ms < 500ms)", elapsed);
            IS_TOGGLING.store(false, Ordering::SeqCst);
            return;
        }
        *last = Instant::now();
        true
    } else {
        IS_TOGGLING.store(false, Ordering::SeqCst);
        return;
    };

    if !cooldown_ok {
        return;
    }

    // 3. 执行切换
    if let Some(window) = app.get_webview_window("main") {
        if window.is_visible().unwrap_or(false) {
            let _ = window.hide();
            log::info!("Panel hidden via hotkey/mouse");
        } else {
            let _ = window.show();
            let _ = window.set_focus();
            let _ = app.emit("show-panel", ());
            log::info!("Panel shown via hotkey/mouse");
        }
    }

    // 4. 释放原子锁
    IS_TOGGLING.store(false, Ordering::SeqCst);
}

/// 鼠标侧键设置（全局静态，供钩子回调读取）
struct MouseSideButtonConfig {
    enabled: AtomicBool,
    xbutton1: AtomicBool,
    xbutton2: AtomicBool,
}

static MOUSE_CONFIG: OnceLock<MouseSideButtonConfig> = OnceLock::new();

/// 将键名转换为键码
pub fn key_to_code(key: &str) -> u8 {
    match key.to_lowercase().as_str() {
        "space" => 0,
        "a" => 1, "b" => 2, "c" => 3, "d" => 4, "e" => 5,
        "f" => 6, "g" => 7, "h" => 8, "i" => 9, "j" => 10,
        "k" => 11, "l" => 12, "m" => 13, "n" => 14, "o" => 15,
        "p" => 16, "q" => 17, "r" => 18, "s" => 19, "t" => 20,
        "u" => 21, "v" => 22, "w" => 23, "x" => 24, "y" => 25,
        "z" => 26,
        "0" => 27, "1" => 28, "2" => 29, "3" => 30, "4" => 31,
        "5" => 32, "6" => 33, "7" => 34, "8" => 35, "9" => 36,
        "f1" => 37, "f2" => 38, "f3" => 39, "f4" => 40, "f5" => 41,
        "f6" => 42, "f7" => 43, "f8" => 44, "f9" => 45, "f10" => 46,
        "f11" => 47, "f12" => 48,
        _ => 0,
    }
}

/// 将键码转换为 Shortcut Code
pub fn code_to_shortcut(code: u8) -> Code {
    match code {
        0 => Code::Space,
        1 => Code::KeyA, 2 => Code::KeyB, 3 => Code::KeyC, 4 => Code::KeyD,
        5 => Code::KeyE, 6 => Code::KeyF, 7 => Code::KeyG, 8 => Code::KeyH,
        9 => Code::KeyI, 10 => Code::KeyJ, 11 => Code::KeyK, 12 => Code::KeyL,
        13 => Code::KeyM, 14 => Code::KeyN, 15 => Code::KeyO, 16 => Code::KeyP,
        17 => Code::KeyQ, 18 => Code::KeyR, 19 => Code::KeyS, 20 => Code::KeyT,
        21 => Code::KeyU, 22 => Code::KeyV, 23 => Code::KeyW, 24 => Code::KeyX,
        25 => Code::KeyY, 26 => Code::KeyZ,
        27 => Code::Digit0, 28 => Code::Digit1, 29 => Code::Digit2, 30 => Code::Digit3,
        31 => Code::Digit4, 32 => Code::Digit5, 33 => Code::Digit6, 34 => Code::Digit7,
        35 => Code::Digit8, 36 => Code::Digit9,
        37 => Code::F1, 38 => Code::F2, 39 => Code::F3, 40 => Code::F4,
        41 => Code::F5, 42 => Code::F6, 43 => Code::F7, 44 => Code::F8,
        45 => Code::F9, 46 => Code::F10, 47 => Code::F11, 48 => Code::F12,
        _ => Code::Space,
    }
}

fn init_mouse_config(enabled: bool, xbutton1: bool, xbutton2: bool) {
    let _ = MOUSE_CONFIG.set(MouseSideButtonConfig {
        enabled: AtomicBool::new(enabled),
        xbutton1: AtomicBool::new(xbutton1),
        xbutton2: AtomicBool::new(xbutton2),
    });
}

pub(crate) fn update_mouse_config(enabled: bool, xbutton1: bool, xbutton2: bool) {
    if let Some(cfg) = MOUSE_CONFIG.get() {
        cfg.enabled.store(enabled, Ordering::Relaxed);
        cfg.xbutton1.store(xbutton1, Ordering::Relaxed);
        cfg.xbutton2.store(xbutton2, Ordering::Relaxed);
    }
}

/// 安装 Win32 低级鼠标钩子，监听鼠标侧键 (XButton1=侧键1, XButton2=侧键2)
fn install_mouse_hook(app: tauri::AppHandle) {
    use windows::Win32::Foundation::{WPARAM, LPARAM, LRESULT};
    use windows::Win32::UI::WindowsAndMessaging::*;

    unsafe extern "system" fn mouse_hook_proc(
        ncode: i32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if ncode >= 0 && (wparam.0 as u32) == WM_XBUTTONDOWN {
            let ms = *(lparam.0 as *const MSLLHOOKSTRUCT);
            let xbutton = (ms.mouseData >> 16) & 0xFFFF;
            if let Some(cfg) = MOUSE_CONFIG.get() {
                if cfg.enabled.load(Ordering::Relaxed) {
                    let should_trigger = (xbutton == 1 && cfg.xbutton1.load(Ordering::Relaxed))
                        || (xbutton == 2 && cfg.xbutton2.load(Ordering::Relaxed));
                    if should_trigger {
                        log::info!("Mouse side button detected: XButton{}", xbutton);
                        if let Some(app) = get_app_handle() {
                            toggle_panel(&app);
                        }
                    }
                }
            }
        }
        CallNextHookEx(None, ncode, wparam, lparam)
    }

    set_app_handle(app);

    std::thread::spawn(move || {
        unsafe {
            let hook = SetWindowsHookExW(
                WH_MOUSE_LL,
                Some(mouse_hook_proc),
                None,
                0,
            );
            if hook.is_err() {
                log::error!("Failed to install mouse hook: {:?}", hook.err());
                return;
            }
            let hook = hook.unwrap();
            log::info!("Mouse hook installed successfully");

            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }

            let _ = UnhookWindowsHookEx(hook);
        }
    });
}

static GLOBAL_APP_HANDLE: OnceLock<tauri::AppHandle> = OnceLock::new();

fn set_app_handle(app: tauri::AppHandle) {
    let _ = GLOBAL_APP_HANDLE.set(app);
}

fn get_app_handle() -> Option<tauri::AppHandle> {
    GLOBAL_APP_HANDLE.get().cloned()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 初始化日志系统
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();

    log::info!("WindowFlow application starting...");

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
                commands::get_all_windows,
                commands::get_all_monitors,
                commands::migrate_window,
                commands::migrate_windows,
                commands::start_monitor,
                commands::stop_monitor,
                commands::get_recent_events,
                commands::init_database,
                commands::save_events_to_database,
                commands::get_recommendations,
                commands::get_monitor_status,
                commands::poll_focus_changes,
                commands::get_recent_focus_events,
                commands::get_usage_stats,
                commands::cleanup_old_data,
                commands::get_window_thumbnail,
                commands::get_cursor_position,
                commands::get_current_window_monitor,
                commands::smart_migrate_windows,
                commands::get_window_stats,
                commands::get_recommendation_settings,
                commands::update_recommendation_settings,
                commands::flash_monitor,
                commands::update_hotkey,
                commands::update_mouse_side_button,
                commands::save_recommendation_data,
                commands::load_recommendation_data,
                commands::cleanup_destroyed_events,
                commands::cleanup_old_recommendation_data,
                commands::get_dwell_records,
                commands::get_co_occurrence_data,
                commands::get_online_model_recommendations,
            ])
        .setup(|app| {
            let _window = app.get_webview_window("main").unwrap();

            let app_handle = app.handle().clone();

            // 注册默认全局快捷键 Ctrl+Shift+Space（仅响应按下事件）
            use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
            let shortcut = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::Space);
            let shortcut_handle = app_handle.clone();
            let gs = app_handle.global_shortcut();
            let my_gen = SHORTCUT_GENERATION.load(Ordering::Relaxed);
            let _ = gs.on_shortcut(shortcut, move |_app, _shortcut, event| {
                // 只在按下时触发，且代数匹配时才执行（避免旧处理器导致闪回）
                if event.state() == ShortcutState::Pressed {
                    let current_gen = SHORTCUT_GENERATION.load(Ordering::Relaxed);
                    if current_gen == my_gen {
                        log::info!("Global shortcut triggered: Ctrl+Shift+Space (gen={})", my_gen);
                        toggle_panel(&shortcut_handle);
                    } else {
                        log::debug!("Ignoring shortcut from old handler (gen={} vs current={})", my_gen, current_gen);
                    }
                }
            });
            log::info!("Global shortcut registered: Ctrl+Shift+Space (gen={})", my_gen);

            // 初始化鼠标侧键配置（默认全部开启）
            init_mouse_config(true, true, true);

            // 安装鼠标侧键钩子
            install_mouse_hook(app_handle.clone());

            // 启动窗口销毁事件监听钩子（用于主动清理已销毁窗口的事件）
            services::process_monitor::start_window_destroy_hook();
            log::info!("Window destroy hook started");

            // [ATT&CK 整改] 使用 EVENT_SYSTEM_FOREGROUND 事件钩子替代轮询
            // 焦点变化时立即触发回调，零延迟且无 CPU 空转
            services::process_monitor::start_foreground_hook();
            log::info!("Foreground hook started (replaces polling)");

            // [T6] 数据库外置：存放到 %APPDATA%\\WindowFlow（独立于程序安装目录，
            // 避免被杀毒软件误判为程序目录内的可疑写入行为）。数据库经 SQLCipher 加密。
            if let Ok(appdata) = std::env::var("APPDATA") {
                let data_dir = std::path::Path::new(&appdata).join("WindowFlow");
                if let Err(e) = std::fs::create_dir_all(&data_dir) {
                    log::error!("Failed to create data dir: {}", e);
                }
                let db_path = data_dir.join("windowflow.db");
                match services::database::DatabaseService::new(&db_path.to_string_lossy()) {
                    Ok(db) => {
                        let state: State<AppState> = app.state();
                        // 启动时从数据库加载历史推荐数据
                        if let Ok(mut engine) = state.recommendation_engine.lock() {
                            let _ = engine.load_from_database(&db);
                        }
                        if let Ok(mut db_opt) = state.database.lock() {
                            *db_opt = Some(db);
                        }
                        log::info!("[T6] Encrypted database initialized at {:?}", db_path);
                    }
                    Err(e) => log::error!("Failed to initialize database: {}", e),
                }
            }

            // [T6] 每 2 分钟持久化推荐数据到外置数据库
            let save_handle = app.handle().clone();
            std::thread::spawn(move || {
                loop {
                    std::thread::sleep(std::time::Duration::from_secs(120));
                    let state: State<AppState> = save_handle.state();
                    let events = {
                        match state.process_monitor.lock() {
                            Ok(m) => m.get_all_events(),
                            Err(_) => continue,
                        }
                    };
                    if events.is_empty() {
                        continue;
                    }
                    let db_clone = {
                        match state.database.lock() {
                            Ok(dbopt) => dbopt.clone(),
                            Err(_) => None,
                        }
                    };
                    if let Some(db) = db_clone {
                        if let Ok(mut engine) = state.recommendation_engine.lock() {
                            engine.update_from_events(&events);
                            if let Err(e) = engine.save_to_database(&db) {
                                log::error!("Periodic save failed: {}", e);
                            }
                        }
                    }
                }
            });

            // [T5] 缩略图缓存清理：启动清理一次 + 每 60 秒清理过期/超量缩略图
            services::thumbnail_cache::cleanup();
            std::thread::spawn(|| {
                loop {
                    std::thread::sleep(std::time::Duration::from_secs(60));
                    services::thumbnail_cache::cleanup();
                }
            });

            // 启动鼠标活动轮询定时器，每 10秒 检查鼠标位置变化
            let app_handle_mouse = app.handle().clone();
            std::thread::spawn(move || {
                loop {
                    let state: State<AppState> = app_handle_mouse.state();
                    if let Ok(monitor) = state.process_monitor.lock() {
                        monitor.poll_mouse_activity();
                    }
                    std::thread::sleep(std::time::Duration::from_secs(10));
                }
            });

            // 创建系统托盘菜单
            let show_panel = MenuItem::with_id(app, "show_panel", "显示面板", true, None::<&str>)?;
            let settings = MenuItem::with_id(app, "settings", "设置", true, None::<&str>)?;
            let about = MenuItem::with_id(app, "about", "关于", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;

            let menu = Menu::with_items(app, &[&show_panel, &settings, &about, &quit])?;

            // 创建系统托盘图标
            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .on_menu_event(move |app, event| {
                    match event.id.as_ref() {
                        "show_panel" => {
                            let window = app.get_webview_window("main").unwrap();
                            let _ = window.show();
                            let _ = window.set_focus();
                            let _ = app.emit("show-panel", ());
                        }
                        "settings" => {
                            let window = app.get_webview_window("main").unwrap();
                            let _ = window.show();
                            let _ = window.set_focus();
                            let _ = app.emit("show-settings", ());
                        }
                        "about" => {
                            let window = app.get_webview_window("main").unwrap();
                            let _ = window.show();
                            let _ = window.set_focus();
                            let _ = app.emit("show-about", ());
                        }
                        "quit" => {
                            std::process::exit(0);
                        }
                        _ => {}
                    }
                })
                .build(app)?;

            // 延迟发送显示面板事件，等待前端加载完成
            let app_handle = app.handle().clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(500));
                let _ = app_handle.emit("show-panel", ());
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
