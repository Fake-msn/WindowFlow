# WindowFlow - 智能桌面窗口管理软件设计文档

## 项目概述

WindowFlow 是一款跨平台桌面窗口管理软件,提供一键多屏幕窗口迁移、基于使用频率的智能推荐、以及本地 ML 工作流识别功能。所有数据处理优先在本地完成,通过进程信息(HWND + PID)而非截图识别窗口,保护用户隐私。

### 核心特性

1. **一键窗口迁移**: 将多屏幕上的窗口快速迁移到指定屏幕
2. **智能推荐**: 基于操作频率在光标附近显示窗口组合建议
3. **工作流识别**: 通过本地规则引擎或可选在线模型识别操作模式
4. **隐私保护**: 本地处理为主,不使用截图识别窗口
5. **DPI 感知**: 使用原生 API 确保迁移后窗口正确缩放

### 技术栈

- **后端**: Rust
- **前端框架**: Tauri 2.x
- **UI**: React + TypeScript + Tailwind CSS
- **数据库**: SQLite (本地存储)
- **ML**: 本地规则引擎 + 可选在线模型

### 目标平台

- **Phase 1**: Windows 10/11 (优先实现)
- **Phase 2**: macOS
- **Phase 3**: Linux (X11/Wayland)

---

## 架构设计

### 分层架构

```
┌─────────────────────────────────────┐
│         UI 层 (Tauri 前端)          │
│  - 浮动面板                         │
│  - 窗口缩略图预览                   │
│  - 用户交互                         │
└─────────────────────────────────────┘
              ↓
┌─────────────────────────────────────┐
│      服务层 (Rust 后端)             │
│  - 窗口管理服务                     │
│  - 进程监控服务                     │
│  - 推荐引擎                         │
│  - 在线模型客户端 (可选)            │
└─────────────────────────────────────┘
              ↓
┌─────────────────────────────────────┐
│    平台适配层 (Platform Layer)      │
│  - Windows: Win32 API               │
│  - macOS: AppKit                    │
│  - Linux: X11/Wayland               │
└─────────────────────────────────────┘
              ↓
┌─────────────────────────────────────┐
│         数据存储层                  │
│  - SQLite (频率统计、推荐缓存)      │
│  - 内存缓存 (实时数据)              │
└─────────────────────────────────────┘
```

### 设计原则

1. **职责分离**: 每层只负责特定功能,通过接口通信
2. **平台抽象**: 核心逻辑与平台 API 解耦,便于扩展
3. **隐私优先**: 敏感数据本地处理,在线功能可选
4. **性能优化**: 延迟加载、缓存策略、异步处理

---

## 核心组件设计

### 1. 平台适配层 (Platform Abstraction Layer)

**职责**: 封装不同操作系统的窗口管理 API

**Windows 实现**:
```rust
// 窗口枚举
EnumWindows(callback, lParam) -> Vec<HWND>

// 窗口信息获取
GetWindowThreadProcessId(hwnd, &pid) -> DWORD
GetWindowText(hwnd, buffer, max_count) -> int
IsWindowVisible(hwnd) -> BOOL
GetWindowRect(hwnd, &rect) -> BOOL

// 窗口迁移
SetWindowPos(hwnd, hWndInsertAfter, x, y, cx, cy, uFlags)
SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)

// DPI 感知
GetDpiForWindow(hwnd) -> UINT
GetDpiForMonitor(hmonitor, dpiType, &dpiX, &dpiY)

// 缩略图 (DWM API)
DwmRegisterThumbnail(hwnd, hwndThumbnail, &hThumbnail)
DwmUpdateThumbnailProperties(hThumbnail, &properties)
DwmUnregisterThumbnail(hThumbnail)
```

**关键接口**:
```rust
pub trait PlatformWindowManager {
    fn enumerate_windows(&self) -> Result<Vec<WindowInfo>>;
    fn move_window(&self, hwnd: WindowHandle, target: MonitorId, pos: Position) -> Result<()>;
    fn get_monitor_dpi(&self, monitor: MonitorId) -> Result<u32>;
    fn get_window_thumbnail(&self, hwnd: WindowHandle, size: Size) -> Result<ThumbnailHandle>;
    fn get_window_process_info(&self, hwnd: WindowHandle) -> Result<ProcessInfo>;
}
```

### 2. 窗口管理服务 (Window Manager Service)

**职责**: 窗口迁移的核心逻辑

**功能**:
- 多屏幕检测与坐标映射
- DPI 感知迁移
- 窗口状态保存/恢复
- 批量迁移支持

**DPI 处理流程**:
```
1. 获取源屏幕 DPI (GetDpiForWindow)
2. 获取目标屏幕 DPI
3. 计算缩放因子: scale = target_dpi / source_dpi
4. 调整窗口尺寸: new_width = old_width * scale
5. 设置 DPI 感知上下文
6. 执行迁移 (SetWindowPos)
7. 验证窗口尺寸是否正确
```

**批量迁移**:
```rust
pub struct WindowMigrationPlan {
    pub windows: Vec<WindowHandle>,
    pub target_monitor: MonitorId,
    pub layout: LayoutStrategy, // 网格、层叠、自定义
}

pub fn migrate_windows(plan: WindowMigrationPlan) -> Result<()>;
```

### 3. 进程监控服务 (Process Monitor Service)

**职责**: 跟踪应用使用频率,不涉及截图

**实现**:
```rust
// Windows: SetWinEventHook 监听焦点变化
SetWinEventHook(
    EVENT_SYSTEM_FOREGROUND,
    EVENT_SYSTEM_FOREGROUND,
    hWinEventHookProc,
)

// 记录数据结构
pub struct WindowFocusEvent {
    pub hwnd: WindowHandle,
    pub pid: u32,
    pub process_name: String,
    pub window_title_hash: String, // 仅存储哈希
    pub timestamp: DateTime<Utc>,
    pub monitor_id: MonitorId,
}
```

**隐私保护**:
- ✅ 记录: 进程名、PID、窗口标题哈希、时间戳
- ❌ 不记录: 完整窗口标题、用户输入、截图、文件路径

**数据清理策略**:
- 默认保留 30 天数据
- 可配置清理周期
- 手动清理选项

### 4. 推荐引擎 (Recommendation Engine)

#### 本地规则引擎

**算法**:

1. **频繁项挖掘 (Apriori)**:
   ```rust
   // 找出常一起使用的应用组合
   pub struct CoOccurrenceMatrix {
       matrix: HashMap<(AppId, AppId), u32>,
   }
   
   impl CoOccurrenceMatrix {
       pub fn update(&mut self, app_a: AppId, app_b: AppId);
       pub fn get_frequent_pairs(&self, min_support: u32) -> Vec<(AppId, AppId)>;
   }
   ```

2. **序列模式分析 (PrefixSpan)**:
   ```rust
   // 识别操作序列
   pub struct SequencePattern {
       pub pattern: Vec<AppId>,
       pub support: u32,
       pub confidence: f32,
   }
   
   pub fn mine_sequences(events: &[WindowFocusEvent]) -> Vec<SequencePattern>;
   ```

3. **时间衰减**:
   ```rust
   // 最近使用的组合权重更高
   pub fn calculate_weight(timestamp: DateTime<Utc>, half_life_hours: f32) -> f32 {
       let age_hours = (Utc::now() - timestamp).num_hours() as f32;
       0.5_f32.powf(age_hours / half_life_hours)
   }
   ```

**推荐生成**:
```rust
pub struct Recommendation {
    pub workflow_label: String, // "开发工作流"、"文档编写"
    pub windows: Vec<WindowHandle>,
    pub score: f32,
    pub source: RecommendationSource, // LocalRule | OnlineModel
}

pub fn generate_recommendations(
    context: UserContext,
    co_occurrence: &CoOccurrenceMatrix,
    sequences: &[SequencePattern],
) -> Vec<Recommendation>;
```

#### 可选在线模型

**输入特征** (匿名化):
```rust
pub struct OnlineModelInput {
    pub process_hashes: Vec<String>, // 进程名哈希
    pub time_features: TimeFeatures, // 小时、星期几、工作日
    pub window_title_hashes: Vec<String>,
    pub recent_sequence: Vec<String>, // 最近 N 个应用哈希
}
```

**输出**:
```rust
pub struct OnlineModelOutput {
    pub workflow_label: String,
    pub recommended_apps: Vec<String>, // 应用哈希
    pub confidence: f32,
}
```

**隐私保护**:
- ✅ 发送: 匿名化特征向量
- ❌ 不发送: 截图、完整进程名、用户输入、文件路径
- ✅ 用户可完全禁用在线功能

### 5. UI 层 (Tauri 前端)

#### 浮动面板设计

**布局**:
```
┌─────────────────────────────────────┐
│  [推荐组合 1: 开发工作流]           │
│  ┌──────────┐ ┌──────────┐         │
│  │          │ │          │         │
│  │ 窗口1    │ │ 窗口2    │         │
│  │ 缩略图   │ │ 缩略图   │         │
│  │          │ │          │         │
│  │      [VS][Term]      │         │
│  └──────────┘ └──────────┘         │
│                                     │
│  [推荐组合 2: 文档编写]             │
│  ┌──────────┐ ┌──────────┐         │
│  │          │ │          │         │
│  │ 窗口3    │ │ 窗口4    │         │
│  │ 缩略图   │ │ 缩略图   │         │
│  │          │ │          │         │
│  │      [Word][PDF]     │         │
│  └──────────┘ └──────────┘         │
└─────────────────────────────────────┘
```

**视觉设计**:
- 半透明毛玻璃效果: `backdrop-filter: blur(20px)`
- 背景色: `rgba(30, 30, 30, 0.85)` (深色模式)
- 圆角: `border-radius: 12px`
- 阴影: `box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3)`
- 边框: `border: 1px solid rgba(255, 255, 255, 0.1)`

**窗口缩略图**:
- 尺寸: 160x90px (16:9)
- 重叠排列: 类似 Mission Control
- 悬停效果: 放大 1.05x, 显示完整标题
- 应用图标: 16x16px, 右下角集合显示

**交互**:
- 触发: 快捷键 (如 `Ctrl+Shift+Space`) 或鼠标手势
- 位置: 光标附近,自动调整避免超出屏幕
- 导航: 方向键切换,Enter 确认,Esc 取消
- 动画: 淡入淡出 200ms, 轻微缩放

**技术实现**:
```typescript
// React 组件结构
interface RecommendationPanel {
  recommendations: Recommendation[];
  onSelect: (rec: Recommendation) => void;
  position: { x: number; y: number };
}

interface RecommendationCard {
  workflowLabel: string;
  windows: WindowThumbnail[];
  appIcons: string[];
  score: number;
}

interface WindowThumbnail {
  hwnd: number;
  thumbnailUrl: string; // 来自 Rust 后端的缩略图
  title: string;
}
```

**缩略图渲染** (Windows):
```rust
// Rust 端: 使用 DWM API 生成缩略图
pub fn capture_window_thumbnail(hwnd: HWND, width: u32, height: u32) -> Result<Vec<u8>> {
    // 1. 注册缩略图
    let mut h_thumbnail = HTHUMBNAIL::default();
    DwmRegisterThumbnail(hwnd, hwnd_host, &mut h_thumbnail)?;
    
    // 2. 设置属性
    let props = DWM_THUMBNAIL_PROPERTIES {
        dwFlags: DWM_TNP_RECTDESTINATION | DWM_TNP_VISIBLE,
        rcDestination: RECT { left: 0, top: 0, right: width, bottom: height },
        fVisible: true,
        ..Default::default()
    };
    DwmUpdateThumbnailProperties(h_thumbnail, &props)?;
    
    // 3. 渲染到内存位图
    // 4. 转换为 PNG/RGB 数据
    // 5. 通过 Tauri IPC 传递给前端
}
```

---

## 数据流

### 主流程

```
1. 用户操作 → 窗口焦点变化
   ↓
2. 进程监控服务捕获事件 (HWND + PID + 时间戳)
   ↓
3. 更新频率统计 (SQLite)
   ↓
4. 推荐引擎分析:
   - 本地规则引擎 (实时, <100ms)
   - 在线模型 (可选, 异步, <500ms)
   ↓
5. 生成推荐结果 (Vec<Recommendation>)
   ↓
6. 查找对应窗口 (通过 PID 查找 HWND)
   ↓
7. UI 层渲染浮动面板:
   - 使用 DWM API 获取窗口缩略图
   - 渲染推荐卡片
   ↓
8. 用户选择 → 窗口管理服务执行迁移
   ↓
9. 验证迁移结果,更新统计
```

### 数据模型

```rust
// 窗口信息
pub struct WindowInfo {
    pub hwnd: WindowHandle,
    pub pid: u32,
    pub process_name: String,
    pub window_title_hash: String,
    pub rect: WindowRect,
    pub monitor_id: MonitorId,
    pub is_visible: bool,
}

// 监控器信息
pub struct MonitorInfo {
    pub id: MonitorId,
    pub name: String,
    pub rect: MonitorRect,
    pub dpi: u32,
    pub is_primary: bool,
}

// 频率统计
pub struct AppUsageStats {
    pub process_name: String,
    pub total_focus_time: Duration,
    pub focus_count: u32,
    pub last_used: DateTime<Utc>,
}

// 推荐缓存
pub struct RecommendationCache {
    pub context_hash: String,
    pub recommendations: Vec<Recommendation>,
    pub generated_at: DateTime<Utc>,
}
```

---

## 隐私保护策略

### 本地数据处理

**存储内容**:
- ✅ 进程名 (如 "code.exe", "chrome.exe")
- ✅ PID (进程 ID)
- ✅ 窗口标题哈希 (SHA-256, 不可逆)
- ✅ 时间戳
- ✅ 使用频率统计

**不存储**:
- ❌ 完整窗口标题
- ❌ 用户输入内容
- ❌ 截图或窗口内容
- ❌ 文件路径
- ❌ 网络活动

**加密**:
- SQLite 数据库使用 SQLCipher 加密
- 密钥存储在系统密钥链 (Windows Credential Manager)

### 在线模型 (可选)

**发送内容** (匿名化):
```json
{
  "process_hashes": ["a1b2c3d4", "e5f6g7h8"],
  "time_features": {
    "hour": 14,
    "day_of_week": 2,
    "is_workday": true
  },
  "window_title_hashes": ["x9y8z7w6"],
  "recent_sequence": ["a1b2c3d4", "i9j8k7l6", "e5f6g7h8"]
}
```

**不发送**:
- ❌ 原始进程名
- ❌ 完整窗口标题
- ❌ 截图
- ❌ 用户输入
- ❌ 文件路径
- ❌ IP 地址 (由服务器记录,非应用发送)

**用户控制**:
- 设置中可完全禁用在线功能
- 显示隐私政策说明
- 可随时删除本地数据

---

## 错误处理

### 窗口迁移失败

**场景**:
- 窗口被锁定 (如全屏游戏)
- 目标屏幕 DPI 不支持
- 权限不足

**处理**:
```rust
pub fn migrate_window(hwnd: HWND, target: MonitorId) -> Result<()> {
    // 1. 保存原始状态
    let original_state = save_window_state(hwnd)?;
    
    // 2. 尝试迁移
    match do_migration(hwnd, target) {
        Ok(_) => Ok(()),
        Err(e) => {
            // 3. 回滚
            restore_window_state(hwnd, original_state)?;
            
            // 4. 记录错误
            log_error(e);
            
            // 5. 通知用户
            show_notification("窗口迁移失败", "请手动调整窗口位置");
            
            Err(e)
        }
    }
}
```

### 在线模型不可用

**降级策略**:
- 超时: 500ms
- 自动切换到本地规则引擎
- 不阻塞核心功能
- 记录错误日志

### DPI 缩放异常

**检测**:
```rust
pub fn verify_dpi_scaling(hwnd: HWND, expected_size: Size) -> Result<()> {
    let actual_size = get_window_size(hwnd)?;
    let tolerance = 0.05; // 5% 容差
    
    if !size_within_tolerance(actual_size, expected_size, tolerance) {
        // 自动调整
        adjust_window_size(hwnd, expected_size)?;
        
        // 如果仍然异常,提示用户
        if !size_within_tolerance(get_window_size(hwnd)?, expected_size, tolerance) {
            show_notification("DPI 缩放异常", "请手动调整窗口大小");
        }
    }
    
    Ok(())
}
```

---

## 测试策略

### 单元测试

**频率统计算法**:
```rust
#[test]
fn test_co_occurrence_matrix() {
    let mut matrix = CoOccurrenceMatrix::new();
    matrix.update("code.exe", "chrome.exe");
    matrix.update("code.exe", "chrome.exe");
    matrix.update("code.exe", "terminal.exe");
    
    let pairs = matrix.get_frequent_pairs(2);
    assert_eq!(pairs.len(), 1);
    assert_eq!(pairs[0], ("code.exe", "chrome.exe"));
}
```

**DPI 计算**:
```rust
#[test]
fn test_dpi_scaling() {
    let source_dpi = 96;
    let target_dpi = 144;
    let scale = target_dpi as f32 / source_dpi as f32;
    
    let original_width = 800;
    let expected_width = (original_width as f32 * scale) as u32;
    assert_eq!(expected_width, 1200);
}
```

### 集成测试

**窗口迁移端到端**:
```rust
#[test]
fn test_window_migration_flow() {
    // 1. 创建测试窗口
    let hwnd = create_test_window();
    
    // 2. 获取目标屏幕
    let target_monitor = get_secondary_monitor();
    
    // 3. 执行迁移
    migrate_window(hwnd, target_monitor).unwrap();
    
    // 4. 验证位置
    let rect = get_window_rect(hwnd).unwrap();
    assert!(target_monitor.rect.contains(rect));
    
    // 5. 验证 DPI
    let dpi = get_window_dpi(hwnd).unwrap();
    assert_eq!(dpi, target_monitor.dpi);
}
```

### UI 测试

**浮动面板交互**:
```typescript
test('recommendation panel shows on hotkey', async () => {
  // 1. 触发快捷键
  await pressKeys(['Control', 'Shift', 'Space']);
  
  // 2. 验证面板显示
  const panel = await screen.findByRole('dialog');
  expect(panel).toBeInTheDocument();
  
  // 3. 验证推荐卡片
  const cards = await screen.findAllByTestId('recommendation-card');
  expect(cards.length).toBeGreaterThan(0);
});
```

---

## 性能优化

### 缩略图缓存

```rust
pub struct ThumbnailCache {
    cache: HashMap<HWND, ThumbnailData>,
    max_age: Duration,
}

impl ThumbnailCache {
    pub fn get(&mut self, hwnd: HWND) -> Option<&ThumbnailData> {
        let entry = self.cache.get_mut(&hwnd)?;
        if entry.age() > self.max_age {
            self.cache.remove(&hwnd);
            None
        } else {
            Some(entry)
        }
    }
}
```

### 推荐缓存

```rust
// 相同上下文 5 分钟内不重新计算
pub fn should_recalculate(context: &UserContext, last_calc: DateTime<Utc>) -> bool {
    let age = Utc::now() - last_calc;
    age.num_minutes() > 5
}
```

### 延迟加载

- 缩略图仅在面板可见时捕获
- 在线模型异步调用,不阻塞 UI
- 数据库查询使用索引优化

---

## 配置与设置

### 用户可配置项

```rust
pub struct AppConfig {
    // 通用设置
    pub hotkey: String, // 默认 "Ctrl+Shift+Space"
    pub language: String, // "zh-CN", "en-US"
    
    // 推荐设置
    pub max_recommendations: u32, // 默认 5
    pub recommendation_timeout_ms: u32, // 默认 500
    pub enable_online_model: bool, // 默认 false
    
    // 隐私设置
    pub data_retention_days: u32, // 默认 30
    pub auto_clean_data: bool, // 默认 true
    
    // UI 设置
    pub panel_opacity: f32, // 0.0 - 1.0, 默认 0.85
    pub thumbnail_size: ThumbnailSize, // Small, Medium, Large
    pub enable_animations: bool, // 默认 true
}
```

---

## 未来扩展

### Phase 2: macOS

- 使用 AppKit API 替换 Win32 API
- NSWindow 管理
- AppKit 缩略图 API

### Phase 3: Linux

- X11: Xlib API
- Wayland: Wayland Client API
- 处理不同桌面环境 (GNOME, KDE)

### 高级功能

- 窗口布局模板 (保存/恢复)
- 多用户配置切换
- 插件系统 (自定义推荐规则)
- 云同步 (可选,加密)

---

## 附录

### 术语表

- **HWND**: Windows 窗口句柄
- **PID**: 进程 ID
- **DPI**: 每英寸点数 (Dots Per Inch)
- **DWM**: 桌面窗口管理器 (Desktop Window Manager)
- **Per-Monitor DPI V2**: Windows 10+ 的 DPI 感知模式

### 参考资料

- [Windows DPI Awareness](https://docs.microsoft.com/en-us/windows/win32/hidpi/high-dpi-desktop-application-development-on-windows)
- [DWM Thumbnail API](https://docs.microsoft.com/en-us/windows/win32/dwm/thumbnail-ovw)
- [Tauri Documentation](https://tauri.app/)
- [Rust Book](https://doc.rust-lang.org/book/)
