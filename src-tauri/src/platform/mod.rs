#[cfg(windows)]
pub mod windows;

#[cfg(windows)]
pub use windows::WindowsPlatform;

use crate::types::{MonitorInfo, WindowInfo, WindowHandle, MonitorId};

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
