# WindowFlow 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 构建一款智能桌面窗口管理软件,提供一键多屏幕窗口迁移、基于使用频率的智能推荐、以及本地 ML 工作流识别功能。

**Architecture:** 采用分层架构,包括平台适配层(封装 Win32 API)、服务层(窗口管理、进程监控、推荐引擎)、数据存储层(SQLite)和 UI 层(Tauri + React)。核心逻辑与平台 API 解耦,便于后续扩展到 macOS 和 Linux。

**Tech Stack:** Rust (后端), Tauri 2.x (桌面框架), React + TypeScript (前端), Tailwind CSS (样式), SQLite (本地数据库), Windows API (Win32, DWM)

---

## 项目初始化

### Task 1: 创建 Tauri 项目结构

**Files:**
- Create: `Cargo.toml`
- Create: `src-tauri/Cargo.toml`
- Create: `src-tauri/tauri.conf.json`
- Create: `package.json`
- Create: `vite.config.ts`
- Create: `tsconfig.json`

- [ ] **Step 1: 初始化 Rust 工作空间**

```bash
cargo init --name windowflow
```

- [ ] **Step 2: 添加 Tauri CLI 依赖**

编辑 `Cargo.toml`:
```toml
[workspace]
members = ["src-tauri"]

[package]
name = "windowflow"
version = "0.1.0"
edition = "2021"

[dependencies]
```

- [ ] **Step 3: 创建 Tauri 应用目录**

```bash
mkdir src-tauri
cd src-tauri
cargo init --lib
```

- [ ] **Step 4: 配置 Tauri Cargo.toml**

编辑 `src-tauri/Cargo.toml`:
```toml
[package]
name = "windowflow-app"
version = "0.1.0"
edition = "2021"

[lib]
name = "windowflow_lib"
crate-type = ["staticlib", "cdylib", "rlib"]

[build-dependencies]
tauri-build = { version = "2.0", features = [] }

[dependencies]
tauri = { version = "2.0", features = [] }
tauri-plugin-shell = "2.0"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
chrono = { version = "0.4", features = ["serde"] }
rusqlite = { version = "0.31", features = ["bundled"] }
sha2 = "0.10"
log = "0.4"
env_logger = "0.11"
thiserror = "1.0"

[target.'cfg(windows)'.dependencies]
windows = { version = "0.57", features = [
    "Win32_Foundation",
    "Win32_UI_WindowsAndMessaging",
    "Win32_Graphics_Gdi",
    "Win32_UI_HiDpi",
    "Win32_Devices_Display",
    "Win32_UI_Shell",
    "Win32_UI_Input_KeyboardAndMouse",
] }
```

- [ ] **Step 5: 创建 Tauri 配置文件**

创建 `src-tauri/tauri.conf.json`:
```json
{
  "productName": "WindowFlow",
  "version": "0.1.0",
  "identifier": "com.windowflow.app",
  "build": {
    "beforeDevCommand": "npm run dev",
    "devUrl": "http://localhost:1420",
    "beforeBuildCommand": "npm run build",
    "frontendDist": "../dist"
  },
  "app": {
    "withGlobalTauri": false,
    "windows": [
      {
        "title": "WindowFlow",
        "width": 800,
        "height": 600,
        "visible": false,
        "decorations": false,
        "transparent": true,
        "alwaysOnTop": true
      }
    ],
    "security": {
      "csp": null
    }
  },
  "bundle": {
    "active": true,
    "targets": "all",
    "icon": [
      "icons/32x32.png",
      "icons/128x128.png",
      "icons/128x128@2x.png",
      "icons/icon.icns",
      "icons/icon.ico"
    ]
  }
}
```

- [ ] **Step 6: 创建 Tauri 构建脚本**

创建 `src-tauri/build.rs`:
```rust
fn main() {
    tauri_build::build()
}
```

- [ ] **Step 7: 创建 Tauri 入口文件**

创建 `src-tauri/src/lib.rs`:
```rust
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            let window = app.get_webview_window("main").unwrap();
            
            // 隐藏主窗口,仅保留系统托盘
            window.hide().unwrap();
            
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

创建 `src-tauri/src/main.rs`:
```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    windowflow_lib::run()
}
```

- [ ] **Step 8: 初始化前端项目**

```bash
npm create vite@latest frontend -- --template react-ts
cd frontend
npm install
npm install -D tailwindcss postcss autoprefixer
npx tailwindcss init -p
```

- [ ] **Step 9: 配置 Tailwind CSS**

编辑 `frontend/tailwind.config.js`:
```javascript
/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {},
  },
  plugins: [],
}
```

编辑 `frontend/src/index.css`:
```css
@tailwind base;
@tailwind components;
@tailwind utilities;

body {
  margin: 0;
  padding: 0;
  background: transparent;
  overflow: hidden;
}
```

- [ ] **Step 10: 配置 Vite**

编辑 `frontend/vite.config.ts`:
```typescript
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
  envPrefix: ["VITE_", "TAURI_"],
  build: {
    target: process.env.TAURI_PLATFORM === "windows" ? "chrome105" : "safari13",
    minify: !process.env.TAURI_DEBUG ? "esbuild" : false,
    sourcemap: !!process.env.TAURI_DEBUG,
  },
});
```

- [ ] **Step 11: 创建前端入口文件**

编辑 `frontend/src/main.tsx`:
```typescript
import React from 'react'
import ReactDOM from 'react-dom/client'
import App from './App'
import './index.css'

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
)
```

创建 `frontend/src/App.tsx`:
```typescript
import { useEffect, useState } from 'react'
import './App.css'

function App() {
  const [isVisible, setIsVisible] = useState(false)

  useEffect(() => {
    // 监听来自 Rust 的消息
    const unlisten = window.__TAURI__.event.listen('show-panel', () => {
      setIsVisible(true)
    })

    return () => {
      unlisten.then(f => f())
    }
  }, [])

  if (!isVisible) return null

  return (
    <div className="w-screen h-screen flex items-center justify-center">
      <div className="bg-gray-900/85 backdrop-blur-xl rounded-xl p-6 shadow-2xl">
        <h1 className="text-white text-2xl font-bold">WindowFlow</h1>
        <p className="text-gray-300 mt-2">智能窗口管理</p>
      </div>
    </div>
  )
}

export default App
```

创建 `frontend/src/App.css`:
```css
#root {
  width: 100vw;
  height: 100vh;
}
```

- [ ] **Step 12: 验证项目结构**

```bash
# 在根目录
cargo build
cd frontend
npm run build
cd ..
cargo tauri dev
```

Expected: 应用启动,显示空白窗口(因为窗口设置为 visible: false)

- [ ] **Step 13: 提交初始项目结构**

```bash
git init
git add .
git commit -m "feat: initialize Tauri project with React frontend"
```

---

## 平台适配层 (Windows)

### Task 2: 定义核心数据类型

**Files:**
- Create: `src-tauri/src/types.rs`

- [ ] **Step 1: 创建类型定义文件**

创建 `src-tauri/src/types.rs`:
```rust
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[cfg(windows)]
use windows::Win32::Foundation::HWND;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WindowHandle(pub i64);

#[cfg(windows)]
impl From<HWND> for WindowHandle {
    fn from(hwnd: HWND) -> Self {
        WindowHandle(hwnd.0 as i64)
    }
}

#[cfg(windows)]
impl From<WindowHandle> for HWND {
    fn from(handle: WindowHandle) -> Self {
        HWND(handle.0 as isize)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MonitorId(pub i64);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl WindowRect {
    pub fn width(&self) -> i32 {
        self.right - self.left
    }

    pub fn height(&self) -> i32 {
        self.bottom - self.top
    }

    pub fn contains(&self, other: &WindowRect) -> bool {
        self.left <= other.left
            && self.top <= other.top
            && self.right >= other.right
            && self.bottom >= other.bottom
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
    pub work_left: i32,
    pub work_top: i32,
    pub work_right: i32,
    pub work_bottom: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowInfo {
    pub hwnd: WindowHandle,
    pub pid: u32,
    pub process_name: String,
    pub window_title_hash: String,
    pub rect: WindowRect,
    pub monitor_id: MonitorId,
    pub is_visible: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorInfo {
    pub id: MonitorId,
    pub name: String,
    pub rect: MonitorRect,
    pub dpi: u32,
    pub is_primary: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessInfo {
    pub pid: u32,
    pub process_name: String,
    pub window_title_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppUsageStats {
    pub process_name: String,
    pub total_focus_time: Duration,
    pub focus_count: u32,
    pub last_used: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recommendation {
    pub workflow_label: String,
    pub windows: Vec<WindowHandle>,
    pub score: f32,
    pub source: RecommendationSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecommendationSource {
    LocalRule,
    OnlineModel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowFocusEvent {
    pub hwnd: WindowHandle,
    pub pid: u32,
    pub process_name: String,
    pub window_title_hash: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub monitor_id: MonitorId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub hotkey: String,
    pub language: String,
    pub max_recommendations: u32,
    pub recommendation_timeout_ms: u32,
    pub enable_online_model: bool,
    pub data_retention_days: u32,
    pub auto_clean_data: bool,
    pub panel_opacity: f32,
    pub enable_animations: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            hotkey: "Ctrl+Shift+Space".to_string(),
            language: "zh-CN".to_string(),
            max_recommendations: 5,
            recommendation_timeout_ms: 500,
            enable_online_model: false,
            data_retention_days: 30,
            auto_clean_data: true,
            panel_opacity: 0.85,
            enable_animations: true,
        }
    }
}
```

- [ ] **Step 2: 在 lib.rs 中引入类型模块**

编辑 `src-tauri/src/lib.rs`:
```rust
mod types;

use tauri::Manager;

// ... 其余代码保持不变
```

- [ ] **Step 3: 验证编译**

```bash
cargo build
```

Expected: 编译成功,无错误

- [ ] **Step 4: 提交类型定义**

```bash
git add src-tauri/src/types.rs src-tauri/src/lib.rs
git commit -m "feat: add core type definitions"
```

---

### Task 3: 实现 Windows 窗口枚举

**Files:**
- Create: `src-tauri/src/platform/mod.rs`
- Create: `src-tauri/src/platform/windows.rs`

- [ ] **Step 1: 创建平台模块结构**

创建 `src-tauri/src/platform/mod.rs`:
```rust
#[cfg(windows)]
pub mod windows;

#[cfg(windows)]
pub use windows::WindowsPlatform;

use crate::types::{MonitorInfo, WindowInfo, WindowHandle, MonitorId};
use std::collections::HashMap;

pub trait PlatformWindowManager {
    fn enumerate_windows(&self) -> Result<Vec<WindowInfo>, PlatformError>;
    fn enumerate_monitors(&self) -> Result<Vec<MonitorInfo>, PlatformError>;
    fn get_window_process_info(&self, hwnd: WindowHandle) -> Result<crate::types::ProcessInfo, PlatformError>;
}

#[derive(Debug, thiserror::Error)]
pub enum PlatformError {
    #[error("Window not found")]
    WindowNotFound,
    
    #[error("Monitor not found")]
    MonitorNotFound,
    
    #[error("Access denied")]
    AccessDenied,
    
    #[error("API error: {0}")]
    ApiError(String),
}
```

- [ ] **Step 2: 实现 Windows 平台窗口枚举**

创建 `src-tauri/src/platform/windows.rs`:
```rust
use super::{PlatformError, PlatformWindowManager};
use crate::types::*;
use sha2::{Sha256, Digest};
use windows::{
    core::*,
    Win32::Foundation::*,
    Win32::Graphics::Gdi::*,
    Win32::UI::WindowsAndMessaging::*,
};

pub struct WindowsPlatform;

impl WindowsPlatform {
    pub fn new() -> Self {
        Self
    }
}

impl PlatformWindowManager for WindowsPlatform {
    fn enumerate_windows(&self) -> Result<Vec<WindowInfo>, PlatformError> {
        let mut windows = Vec::new();
        
        unsafe {
            EnumWindows(Some(enum_window_proc), LPARAM(&mut windows as *mut Vec<WindowInfo> as isize))
                .map_err(|e| PlatformError::ApiError(e.to_string()))?;
        }
        
        Ok(windows)
    }

    fn enumerate_monitors(&self) -> Result<Vec<MonitorInfo>, PlatformError> {
        let mut monitors = Vec::new();
        
        unsafe {
            EnumDisplayMonitors(
                HDC::default(),
                None,
                Some(enum_monitor_proc),
                LPARAM(&mut monitors as *mut Vec<MonitorInfo> as isize),
            )
            .map_err(|e| PlatformError::ApiError(e.to_string()))?;
        }
        
        Ok(monitors)
    }

    fn get_window_process_info(&self, hwnd: WindowHandle) -> Result<ProcessInfo, PlatformError> {
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
    
    // 跳过不可见窗口
    if !IsWindowVisible(hwnd).as_bool() {
        return TRUE;
    }
    
    // 获取窗口标题
    let title = match get_window_title(hwnd) {
        Ok(t) => t,
        Err(_) => return TRUE,
    };
    
    // 跳过无标题窗口
    if title.is_empty() {
        return TRUE;
    }
    
    // 获取进程信息
    let mut pid: u32 = 0;
    GetWindowThreadProcessId(hwnd, Some(&mut pid));
    
    let process_name = match get_process_name(pid) {
        Ok(name) => name,
        Err(_) => return TRUE,
    };
    
    // 获取窗口矩形
    let mut rect = RECT::default();
    if GetWindowRect(hwnd, &mut rect).is_err() {
        return TRUE;
    }
    
    // 获取监控器 ID
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
    TRUE
}

unsafe extern "system" fn enum_monitor_proc(hmonitor: HMONITOR, _hdc: HDC, lprect: *mut RECT, lparam: LPARAM) -> BOOL {
    let monitors = &mut *(lparam.0 as *mut Vec<MonitorInfo>);
    
    let mut info = MONITORINFOEXW::default();
    info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
    
    if GetMonitorInfoW(hmonitor, &mut info.monitorInfo).is_err() {
        return TRUE;
    }
    
    // 获取 DPI
    let dpi = get_monitor_dpi(hmonitor).unwrap_or(96);
    
    // 获取监控器名称
    let name = String::from_utf16_lossy(
        &info.szDevice[..]
            .iter()
            .take_while(|&&c| c != 0)
            .copied()
            .collect::<Vec<u16>>()
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
    TRUE
}

fn get_window_title(hwnd: HWND) -> Result<String, PlatformError> {
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

fn get_process_name(pid: u32) -> Result<String, PlatformError> {
    use windows::Win32::System::Threading::*;
    
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, pid)
            .map_err(|e| PlatformError::ApiError(e.to_string()))?;
        
        let mut buffer = [0u16; 260];
        let mut size = 260u32;
        
        QueryFullProcessImageNameW(handle, 0, PWSTR(buffer.as_mut_ptr()), &mut size)
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

fn get_monitor_dpi(hmonitor: HMONITOR) -> Result<u32, PlatformError> {
    use windows::Win32::UI::HiDpi::*;
    
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
    format!("{:x}", result)[..16].to_string() // 使用前 16 个字符
}
```

- [ ] **Step 3: 在 lib.rs 中引入平台模块**

编辑 `src-tauri/src/lib.rs`:
```rust
mod types;
mod platform;

use tauri::Manager;

// ... 其余代码保持不变
```

- [ ] **Step 4: 添加 Windows 系统依赖**

编辑 `src-tauri/Cargo.toml`,在 `[target.'cfg(windows)'.dependencies]` 下添加:
```toml
windows = { version = "0.57", features = [
    "Win32_Foundation",
    "Win32_UI_WindowsAndMessaging",
    "Win32_Graphics_Gdi",
    "Win32_UI_HiDpi",
    "Win32_System_Threading",
] }
```

- [ ] **Step 5: 验证编译**

```bash
cargo build
```

Expected: 编译成功

- [ ] **Step 6: 提交窗口枚举实现**

```bash
git add src-tauri/src/platform/ src-tauri/src/lib.rs src-tauri/Cargo.toml
git commit -m "feat: implement Windows window enumeration"
```

---

### Task 4: 实现窗口迁移与 DPI 感知

**Files:**
- Modify: `src-tauri/src/platform/windows.rs`
- Create: `src-tauri/src/services/window_manager.rs`

- [ ] **Step 1: 在 WindowsPlatform 中添加窗口迁移方法**

编辑 `src-tauri/src/platform/windows.rs`,在 `impl WindowsPlatform` 中添加:
```rust
impl WindowsPlatform {
    // ... 现有代码
    
    pub fn move_window(&self, hwnd: WindowHandle, target_monitor: MonitorId) -> Result<(), PlatformError> {
        let hwnd_win: HWND = hwnd.into();
        
        unsafe {
            // 获取当前窗口信息
            let mut rect = RECT::default();
            GetWindowRect(hwnd_win, &mut rect)
                .map_err(|e| PlatformError::ApiError(e.to_string()))?;
            
            // 获取源和目标监控器
            let source_monitor = MonitorFromWindow(hwnd_win, MONITOR_DEFAULTTONEAREST);
            let target_monitor_win = HMONITOR(target_monitor.0 as isize);
            
            // 获取 DPI
            let source_dpi = get_monitor_dpi(source_monitor)?;
            let target_dpi = get_monitor_dpi(target_monitor_win)?;
            
            // 计算缩放因子
            let scale = target_dpi as f32 / source_dpi as f32;
            
            // 获取目标监控器工作区域
            let mut target_info = MONITORINFOEXW::default();
            target_info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
            GetMonitorInfoW(target_monitor_win, &mut target_info.monitorInfo)
                .map_err(|e| PlatformError::ApiError(e.to_string()))?;
            
            // 计算新位置和尺寸
            let width = ((rect.right - rect.left) as f32 * scale) as i32;
            let height = ((rect.bottom - rect.top) as f32 * scale) as i32;
            
            let work_width = target_info.monitorInfo.rcWork.right - target_info.monitorInfo.rcWork.left;
            let work_height = target_info.monitorInfo.rcWork.bottom - target_info.monitorInfo.rcWork.top;
            
            // 居中放置
            let x = target_info.monitorInfo.rcWork.left + (work_width - width) / 2;
            let y = target_info.monitorInfo.rcWork.top + (work_height - height) / 2;
            
            // 设置 DPI 感知上下文
            SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)
                .map_err(|e| PlatformError::ApiError(e.to_string()))?;
            
            // 移动窗口
            SetWindowPos(
                hwnd_win,
                HWND::default(),
                x,
                y,
                width,
                height,
                SWP_NOZORDER | SWP_NOACTIVATE,
            )
            .map_err(|e| PlatformError::ApiError(e.to_string()))?;
            
            Ok(())
        }
    }
    
    pub fn get_window_dpi(&self, hwnd: WindowHandle) -> Result<u32, PlatformError> {
        use windows::Win32::UI::HiDpi::GetDpiForWindow;
        
        let hwnd_win: HWND = hwnd.into();
        
        unsafe {
            GetDpiForWindow(hwnd_win)
                .map_err(|e| PlatformError::ApiError(e.to_string()))
        }
    }
}
```

- [ ] **Step 2: 添加必要的 Windows API 导入**

在 `src-tauri/src/platform/windows.rs` 顶部添加:
```rust
use windows::{
    core::*,
    Win32::Foundation::*,
    Win32::Graphics::Gdi::*,
    Win32::UI::WindowsAndMessaging::*,
    Win32::UI::HiDpi::*,
};
```

- [ ] **Step 3: 创建窗口管理服务**

创建 `src-tauri/src/services/window_manager.rs`:
```rust
use crate::platform::windows::WindowsPlatform;
use crate::platform::{PlatformError, PlatformWindowManager};
use crate::types::*;

pub struct WindowManagerService {
    platform: WindowsPlatform,
}

impl WindowManagerService {
    pub fn new() -> Self {
        Self {
            platform: WindowsPlatform::new(),
        }
    }

    pub fn get_all_windows(&self) -> Result<Vec<WindowInfo>, PlatformError> {
        self.platform.enumerate_windows()
    }

    pub fn get_all_monitors(&self) -> Result<Vec<MonitorInfo>, PlatformError> {
        self.platform.enumerate_monitors()
    }

    pub fn migrate_window(
        &self,
        hwnd: WindowHandle,
        target_monitor: MonitorId,
    ) -> Result<(), PlatformError> {
        // 保存原始状态
        let original_info = self.get_window_info(hwnd)?;
        
        // 尝试迁移
        match self.platform.move_window(hwnd, target_monitor) {
            Ok(_) => Ok(()),
            Err(e) => {
                // 回滚失败(窗口可能已移动,无法完全恢复)
                log::error!("Migration failed, rollback not implemented: {}", e);
                Err(e)
            }
        }
    }

    pub fn migrate_windows(
        &self,
        windows: &[WindowHandle],
        target_monitor: MonitorId,
    ) -> Result<(), PlatformError> {
        for &hwnd in windows {
            self.migrate_window(hwnd, target_monitor)?;
        }
        Ok(())
    }

    pub fn get_window_info(&self, hwnd: WindowHandle) -> Result<WindowInfo, PlatformError> {
        let windows = self.platform.enumerate_windows()?;
        windows
            .into_iter()
            .find(|w| w.hwnd == hwnd)
            .ok_or(PlatformError::WindowNotFound)
    }

    pub fn verify_dpi_scaling(
        &self,
        hwnd: WindowHandle,
        expected_dpi: u32,
    ) -> Result<bool, PlatformError> {
        let actual_dpi = self.platform.get_window_dpi(hwnd)?;
        Ok(actual_dpi == expected_dpi)
    }
}
```

- [ ] **Step 4: 创建服务模块**

创建 `src-tauri/src/services/mod.rs`:
```rust
pub mod window_manager;

pub use window_manager::WindowManagerService;
```

- [ ] **Step 5: 在 lib.rs 中引入服务模块**

编辑 `src-tauri/src/lib.rs`:
```rust
mod types;
mod platform;
mod services;

use tauri::Manager;

// ... 其余代码保持不变
```

- [ ] **Step 6: 验证编译**

```bash
cargo build
```

Expected: 编译成功

- [ ] **Step 7: 提交窗口迁移实现**

```bash
git add src-tauri/src/platform/windows.rs src-tauri/src/services/ src-tauri/src/lib.rs
git commit -m "feat: implement window migration with DPI awareness"
```

---

由于实现计划内容较长,我将创建一个精简版的核心任务清单。完整计划包含以下主要模块:

1. **项目初始化** ✅ (已完成)
2. **平台适配层** ✅ (已完成)
3. **进程监控服务**
4. **推荐引擎**
5. **数据存储层**
6. **Tauri 命令接口**
7. **UI 浮动面板**
8. **集成测试**

让我继续创建剩余的核心任务。
