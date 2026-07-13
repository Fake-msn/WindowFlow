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

    /// 智能迁移窗口到目标显示器
    /// 如果窗口最小化，先还原，迁移后再最小化回去
    pub fn smart_migrate_windows(
        &self,
        windows: &[WindowHandle],
        target_monitor: MonitorId,
    ) -> Result<Vec<SmartMigrateResult>, PlatformError> {
        let mut results = Vec::new();

        log::info!("Starting smart migration for {} windows to monitor {:?}", windows.len(), target_monitor);

        for (index, &hwnd) in windows.iter().enumerate() {
            log::info!("Processing window {}/{}: {:?}", index + 1, windows.len(), hwnd);
            
            let was_minimized = self.platform.is_window_minimized(hwnd)?;
            log::info!("Window {:?} minimized status: {}", hwnd, was_minimized);
            
            // 如果最小化，先还原并等待窗口完全显示
            if was_minimized {
                log::info!("Window {:?} was minimized, restoring before migration", hwnd);
                self.platform.restore_window(hwnd)?;
                // 增加等待时间，确保窗口完全还原
                std::thread::sleep(std::time::Duration::from_millis(500));
            }

            // 尝试迁移窗口（批量模式：不激活窗口，避免焦点干扰）
            log::info!("Attempting to move window {:?}", hwnd);
            let migrate_result = self.platform.move_window_internal(hwnd, target_monitor, false);

            if migrate_result.is_ok() {
                log::info!("Window {:?} migrated successfully", hwnd);
                // 如果之前最小化，迁移后再最小化回去
                if was_minimized {
                    log::info!("Window {:?} migrated, minimizing back", hwnd);
                    // 等待窗口稳定后再最小化
                    std::thread::sleep(std::time::Duration::from_millis(200));
                    let _ = self.platform.minimize_window(hwnd);
                }
                results.push(SmartMigrateResult {
                    hwnd,
                    success: true,
                    was_minimized,
                    error: None,
                });
            } else {
                let error_msg = migrate_result.unwrap_err().to_string();
                log::error!("Migration failed for {:?}: {}", hwnd, error_msg);
                results.push(SmartMigrateResult {
                    hwnd,
                    success: false,
                    was_minimized,
                    error: Some(error_msg),
                });
            }
            
            // 在窗口之间添加短暂延迟，避免冲突
            if index < windows.len() - 1 {
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }

        log::info!("Smart migration completed: {} total, {} successful", 
            windows.len(), results.iter().filter(|r| r.success).count());
        Ok(results)
    }
}

/// 智能迁移结果
#[derive(Debug, Clone, serde::Serialize)]
pub struct SmartMigrateResult {
    pub hwnd: WindowHandle,
    pub success: bool,
    pub was_minimized: bool,
    pub error: Option<String>,
}
