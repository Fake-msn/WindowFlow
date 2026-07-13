# WindowFlow 实现任务清单

## Phase 1: 核心基础设施 (已完成)

- [x] Task 1: 创建 Tauri 项目结构
- [x] Task 2: 定义核心数据类型
- [x] Task 3: 实现 Windows 窗口枚举
- [x] Task 4: 实现窗口迁移与 DPI 感知

---

## Phase 2: 进程监控与数据存储

### Task 5: 实现进程监控服务

**Files:**
- Create: `src-tauri/src/services/process_monitor.rs`
- Modify: `src-tauri/src/services/mod.rs`

**关键实现:**
```rust
// 使用 SetWinEventHook 监听窗口焦点变化
// 记录 HWND + PID + 时间戳
// 计算窗口标题哈希(不存储完整标题)
// 数据写入 SQLite

pub struct ProcessMonitorService {
    db: DatabaseService,
    event_hook: Option<HWINEVENTHOOK>,
}

impl ProcessMonitorService {
    pub fn start(&mut self) -> Result<(), MonitorError>;
    pub fn stop(&mut self) -> Result<(), MonitorError>;
    pub fn get_recent_events(&self, minutes: u32) -> Result<Vec<WindowFocusEvent>, MonitorError>;
}
```

**测试:**
- 验证焦点事件捕获
- 验证数据哈希正确性
- 验证数据库写入

---

### Task 6: 实现 SQLite 数据存储

**Files:**
- Create: `src-tauri/src/services/database.rs`
- Modify: `src-tauri/src/services/mod.rs`

**关键实现:**
```rust
pub struct DatabaseService {
    conn: rusqlite::Connection,
}

impl DatabaseService {
    pub fn new(path: &str) -> Result<Self, DatabaseError>;
    pub fn init_tables(&self) -> Result<(), DatabaseError>;
    pub fn insert_focus_event(&self, event: &WindowFocusEvent) -> Result<(), DatabaseError>;
    pub fn get_usage_stats(&self, days: u32) -> Result<Vec<AppUsageStats>, DatabaseError>;
    pub fn cleanup_old_data(&self, retention_days: u32) -> Result<(), DatabaseError>;
}
```

**数据库表结构:**
```sql
CREATE TABLE focus_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    hwnd INTEGER NOT NULL,
    pid INTEGER NOT NULL,
    process_name TEXT NOT NULL,
    window_title_hash TEXT NOT NULL,
    timestamp INTEGER NOT NULL,
    monitor_id INTEGER NOT NULL
);

CREATE INDEX idx_timestamp ON focus_events(timestamp);
CREATE INDEX idx_process ON focus_events(process_name);
```

---

## Phase 3: 推荐引擎

### Task 7: 实现本地规则引擎

**Files:**
- Create: `src-tauri/src/services/recommendation.rs`
- Modify: `src-tauri/src/services/mod.rs`

**关键实现:**
```rust
pub struct RecommendationEngine {
    co_occurrence: CoOccurrenceMatrix,
    sequences: Vec<SequencePattern>,
}

impl RecommendationEngine {
    pub fn new() -> Self;
    pub fn update_from_events(&mut self, events: &[WindowFocusEvent]);
    pub fn generate_recommendations(
        &self,
        context: &UserContext,
        max_count: u32,
    ) -> Vec<Recommendation>;
}

// 频繁项挖掘
pub struct CoOccurrenceMatrix {
    matrix: HashMap<(String, String), u32>,
}

// 序列模式分析
pub struct SequencePattern {
    pub pattern: Vec<String>,
    pub support: u32,
    pub confidence: f32,
}
```

**算法:**
- Apriori 频繁项挖掘
- PrefixSpan 序列模式分析
- 时间衰减权重计算

---

### Task 8: 实现在线模型客户端 (可选)

**Files:**
- Create: `src-tauri/src/services/online_model.rs`
- Modify: `src-tauri/src/services/mod.rs`

**关键实现:**
```rust
pub struct OnlineModelClient {
    endpoint: String,
    timeout_ms: u32,
}

impl OnlineModelClient {
    pub fn new(endpoint: &str, timeout_ms: u32) -> Self;
    pub async fn predict(
        &self,
        input: OnlineModelInput,
    ) -> Result<OnlineModelOutput, ModelError>;
}

// 匿名化输入
pub struct OnlineModelInput {
    pub process_hashes: Vec<String>,
    pub time_features: TimeFeatures,
    pub window_title_hashes: Vec<String>,
    pub recent_sequence: Vec<String>,
}
```

**隐私保护:**
- 仅发送哈希值
- 不发送截图、完整进程名、用户输入

---

## Phase 4: Tauri 命令接口

### Task 9: 创建 Tauri 命令

**Files:**
- Create: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`

**关键实现:**
```rust
#[tauri::command]
async fn get_windows() -> Result<Vec<WindowInfo>, String>;

#[tauri::command]
async fn get_monitors() -> Result<Vec<MonitorInfo>, String>;

#[tauri::command]
async fn migrate_window(hwnd: i64, target_monitor: i64) -> Result<(), String>;

#[tauri::command]
async fn migrate_windows(windows: Vec<i64>, target_monitor: i64) -> Result<(), String>;

#[tauri::command]
async fn get_recommendations() -> Result<Vec<Recommendation>, String>;

#[tauri::command]
async fn show_recommendation_panel();

#[tauri::command]
async fn hide_recommendation_panel();
```

**注册命令:**
```rust
tauri::Builder::default()
    .invoke_handler(tauri::generate_handler![
        get_windows,
        get_monitors,
        migrate_window,
        migrate_windows,
        get_recommendations,
        show_recommendation_panel,
        hide_recommendation_panel,
    ])
```

---

## Phase 5: UI 浮动面板

### Task 10: 创建浮动面板组件

**Files:**
- Create: `frontend/src/components/RecommendationPanel.tsx`
- Create: `frontend/src/components/RecommendationCard.tsx`
- Create: `frontend/src/components/WindowThumbnail.tsx`

**关键实现:**
```typescript
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
  thumbnailUrl: string;
  title: string;
}
```

**视觉设计:**
- 半透明毛玻璃: `backdrop-filter: blur(20px)`
- 背景色: `rgba(30, 30, 30, 0.85)`
- 圆角: `border-radius: 12px`
- 阴影: `box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3)`

---

### Task 11: 实现窗口缩略图渲染

**Files:**
- Modify: `src-tauri/src/platform/windows.rs`
- Create: `frontend/src/hooks/useWindowThumbnail.ts`

**Windows DWM API:**
```rust
pub fn capture_window_thumbnail(
    hwnd: WindowHandle,
    width: u32,
    height: u32,
) -> Result<Vec<u8>, PlatformError> {
    // 使用 DwmRegisterThumbnail
    // 渲染到内存位图
    // 转换为 PNG 数据
}
```

**前端 Hook:**
```typescript
export function useWindowThumbnail(hwnd: number) {
  const [thumbnailUrl, setThumbnailUrl] = useState<string>('');
  
  useEffect(() => {
    window.__TAURI__.invoke('capture_thumbnail', { hwnd, width: 160, height: 90 })
      .then((data: number[]) => {
        const blob = new Blob([new Uint8Array(data)], { type: 'image/png' });
        setThumbnailUrl(URL.createObjectURL(blob));
      });
  }, [hwnd]);
  
  return thumbnailUrl;
}
```

---

### Task 12: 实现快捷键与面板触发

**Files:**
- Create: `frontend/src/hooks/useHotkey.ts`
- Modify: `frontend/src/App.tsx`

**关键实现:**
```typescript
export function useHotkey(combo: string, callback: () => void) {
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      // 解析 combo (如 "Ctrl+Shift+Space")
      // 检查是否匹配
      // 触发 callback
    };
    
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [combo, callback]);
}
```

---

## Phase 6: 集成与测试

### Task 13: 端到端集成

**Files:**
- Modify: `src-tauri/src/lib.rs`
- Modify: `frontend/src/App.tsx`

**关键步骤:**
1. 初始化所有服务
2. 启动进程监控
3. 注册 Tauri 命令
4. 连接前端与后端
5. 测试完整流程

---

### Task 14: 单元测试

**Files:**
- Create: `src-tauri/src/services/recommendation_test.rs`
- Create: `src-tauri/src/platform/windows_test.rs`

**测试用例:**
- 频繁项挖掘算法
- DPI 缩放计算
- 窗口枚举
- 数据库操作

---

### Task 15: UI 测试

**Files:**
- Create: `frontend/src/components/__tests__/RecommendationPanel.test.tsx`

**测试用例:**
- 面板显示/隐藏
- 推荐卡片渲染
- 快捷键响应
- 窗口选择

---

## 开发顺序建议

1. **Week 1**: Task 1-4 (核心基础设施) ✅
2. **Week 2**: Task 5-6 (进程监控与存储)
3. **Week 3**: Task 7-8 (推荐引擎)
4. **Week 4**: Task 9 (Tauri 命令)
5. **Week 5**: Task 10-12 (UI 浮动面板)
6. **Week 6**: Task 13-15 (集成与测试)

---

## 下一步行动

**立即可执行的任务:**

1. **Task 5**: 实现进程监控服务
   - 使用 SetWinEventHook 监听焦点变化
   - 创建 SQLite 数据库
   - 记录窗口事件

2. **Task 6**: 实现数据存储层
   - 创建数据库表
   - 实现 CRUD 操作
   - 添加数据清理功能

3. **Task 7**: 实现推荐引擎
   - 实现 Apriori 算法
   - 实现序列模式分析
   - 生成推荐结果

**建议:** 从 Task 5 开始,按顺序实现。每个 Task 完成后进行代码审查和测试。
