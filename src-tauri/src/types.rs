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
        HWND(handle.0 as *mut _)
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

/// 推荐窗口组 - 2~5 个频繁共现的窗口，以堆叠缩略图展示
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecommendationGroup {
    pub windows: Vec<WindowHandle>,
    pub score: f32,
    pub label: String,
}

/// 单例推荐 - 按停留时间排序的单个应用
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SingletonRecommendation {
    pub hwnd: WindowHandle,
    pub process_name: String,
    pub dwell_time_secs: u64,
}

/// 推荐窗口信息 - 包含窗口详情
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecommendationWindowInfo {
    pub hwnd: WindowHandle,
    pub process_name: String,
    pub dwell_time_secs: u64,
}

/// 推荐组 - 一组相关的窗口
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecommendationGroupInfo {
    pub windows: Vec<RecommendationWindowInfo>,
    pub label: String,
}

/// 推荐响应 - 包含多个推荐组
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecommendationResponse {
    pub groups: Vec<RecommendationGroupInfo>,
}

/// 推荐设置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecommendationSettings {
    /// 显示几个推荐列表（1-3）
    pub list_count: usize,
    /// 每个列表最大窗口种类数（2-10）
    pub max_windows_per_list: usize,
    /// 常用组合：最小停留时间阈值（秒），默认600秒=10分钟
    pub common_combo_min_dwell_secs: u64,
    /// 常用组合：鼠标静止超时阈值（秒），超过此时间无鼠标移动则剔除，默认180秒=3分钟
    pub common_combo_mouse_idle_threshold_secs: u64,
    /// 最近使用：最大单次停留时间（秒），低于此值才算频繁切换，默认300秒=5分钟
    pub recent_max_dwell_secs: u64,
    /// 最近使用：最小切换次数，需要达到此次数才算频繁，默认3次
    pub recent_min_switch_count: u32,
    /// 窗口忽略清单（进程名列表）
    pub ignore_list: Vec<String>,
    /// 在线模型 API Key
    pub api_key: Option<String>,
    /// 在线模型 API 端点
    pub api_endpoint: Option<String>,
    /// 在线模型名称
    pub model_name: Option<String>,
}

impl Default for RecommendationSettings {
    fn default() -> Self {
        Self {
            list_count: 1,
            max_windows_per_list: 5,
            common_combo_min_dwell_secs: 600,      // 10分钟
            common_combo_mouse_idle_threshold_secs: 180,  // 3分钟
            recent_max_dwell_secs: 300,            // 5分钟
            recent_min_switch_count: 3,            // 3次
            ignore_list: Vec::new(),
            api_key: None,
            api_endpoint: None,
            model_name: None,
        }
    }
}

/// 鼠标活动事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MouseActivityEvent {
    /// 时间戳
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// 鼠标位置X
    pub x: i32,
    /// 鼠标位置Y
    pub y: i32,
    /// 当前焦点窗口句柄
    pub hwnd: WindowHandle,
}

/// 窗口停留记录（包含有效工作时间）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowDwellRecord {
    /// 窗口句柄
    pub hwnd: WindowHandle,
    /// 进程名
    pub process_name: String,
    /// 总停留时间（秒）
    pub total_dwell_secs: u64,
    /// 有效工作时间（秒）- 剔除鼠标静止时间
    pub effective_dwell_secs: u64,
    /// 切换次数（该窗口被切换到的次数）
    pub switch_count: u32,
    /// 单次停留时间列表（用于分析频繁切换）
    pub dwell_sessions: Vec<u64>,
    /// 最后活跃时间
    pub last_active: chrono::DateTime<chrono::Utc>,
}

/// 窗口统计信息（用于设置界面的忽略清单选择）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowStats {
    /// 进程名
    pub process_name: String,
    /// 总停留时间（秒）
    pub total_dwell_secs: u64,
    /// 有效工作时间（秒）
    pub effective_dwell_secs: u64,
    /// 切换次数
    pub switch_count: u32,
    /// 是否已在忽略清单中
    pub is_ignored: bool,
}

/// 窗口停留记录（用于数据库持久化）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DwellRecordRow {
    pub id: i64,
    pub hwnd: i64,
    pub process_name: String,
    pub dwell_secs: i64,
    pub switch_count: i32,
    pub last_active: i64,
    pub created_at: i64,
}

/// 共现矩阵记录（用于数据库持久化）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoOccurrenceRow {
    pub id: i64,
    pub process1: String,
    pub process2: String,
    pub count: i32,
    pub updated_at: i64,
}

/// 推荐设置（用于数据库持久化）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecommendationSettingsRow {
    pub id: i64,
    pub settings_json: String,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecommendationSource {
    LocalRule,
    OnlineModel,
}

/// 模型推荐结果（用于数据库持久化）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRecommendationRow {
    pub id: i64,
    pub scenario_type: String,
    pub window_combinations: String,
    pub confidence_score: f64,
    pub created_at: i64,
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
    // 鼠标侧键设置
    pub mouse_side_button_enabled: bool,
    pub mouse_side_button_xbutton1: bool,
    pub mouse_side_button_xbutton2: bool,
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
            mouse_side_button_enabled: true,
            mouse_side_button_xbutton1: true,
            mouse_side_button_xbutton2: true,
        }
    }
}
