# WindowFlow

智能桌面窗口管理工具 — 一键多屏幕窗口迁移、基于使用频率的智能推荐、本地工作流识别。

## 核心特性

- **一键窗口迁移**：将多屏幕上的窗口快速迁移到指定屏幕，支持 DPI 感知缩放
- **智能推荐**：基于操作频率在光标附近显示窗口组合建议，自动识别工作流模式
- **工作流识别**：通过本地规则引擎分析窗口使用模式，支持可选在线模型增强推荐
- **隐私保护**：本地处理为主，通过进程信息（HWND + PID）识别窗口，不使用截图
- **深色/浅色主题**：支持深色、浅色及跟随系统主题切换

## 技术栈

| 层级 | 技术 |
|------|------|
| 后端 | Rust |
| 前端框架 | Tauri 2.x |
| UI | React + TypeScript + Tailwind CSS |
| 数据库 | SQLite（本地存储） |
| 推荐引擎 | 本地规则引擎 + 可选在线模型（OpenAI 兼容 API） |

## 目标平台

- **Phase 1**：Windows 10/11（已实现）
- **Phase 2**：macOS
- **Phase 3**：Linux（X11/Wayland）

## 架构概览

```
┌─────────────────────────────────────┐
│         UI 层 (Tauri 前端)          │
│  - 浮动面板                         │
│  - 窗口缩略图预览                   │
│  - 设置面板                         │
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
│  - SQLite（频率统计、推荐缓存）     │
│  - 内存缓存（实时数据）             │
└─────────────────────────────────────┘
```

## 已实现功能

### 窗口管理
- 多屏幕窗口枚举与信息获取
- DPI 感知的窗口迁移
- 窗口缩略图预览（DWM API）
- 窗口批量迁移

### 进程监控
- 前台窗口焦点变化追踪
- 窗口停留时间统计
- 窗口关闭事件监听（EVENT_OBJECT_DESTROY）
- 频繁切换检测

### 推荐引擎
- **本地规则推荐**：基于共现矩阵和停留时间统计的窗口组合推荐
- **在线模型推荐**：通过 OpenAI 兼容 API 进行场景化感知的窗口组合推荐
- 推荐列表数量与窗口数可配置
- 数据滚动清理（每 2 次模型调用后清理历史数据）

### 数据库
- 窗口停留记录（dwell_records）
- 窗口共现统计（co_occurrence）
- 推荐设置存储（recommendation_settings）
- 模型调用计数（model_call_count）

### UI
- 浮动面板（毛玻璃效果）
- 窗口缩略图卡片
- Mission Control 风格窗口概览
- 设置面板（快捷键、推荐、外观、性能）
- 深色/浅色/跟随系统主题切换
- 设置项悬停提示说明

### 输入
- 全局快捷键触发面板
- 鼠标侧键触发面板（前进/后退按钮可配置）

## 快速开始

### 环境要求

- [Node.js](https://nodejs.org/) 20+
- [Rust](https://www.rust-lang.org/) 1.70+
- Windows 10/11（Phase 1）

### 开发

```bash
# 安装前端依赖
cd frontend
npm install

# 构建前端
npm run build

# 运行 Tauri 开发模式
cd ..
cargo tauri dev
```

### 构建

```bash
# Debug 构建
cargo build --manifest-path src-tauri/Cargo.toml

# Release 构建
cargo build --release --manifest-path src-tauri/Cargo.toml
```

## 隐私保护

### 本地数据处理

**存储内容**：
- 进程名（如 `code.exe`、`chrome.exe`）
- PID（进程 ID）
- 窗口标题哈希（SHA-256，不可逆）
- 时间戳
- 使用频率统计

**不存储**：
- 完整窗口标题
- 用户输入内容
- 截图或窗口内容
- 文件路径
- 网络活动

### 在线模型（可选）

- 仅发送匿名化特征向量
- 用户可完全禁用在线功能
- 数据保留 30 天，可配置清理周期

## 项目结构

```
WindowFlow/
├── frontend/                # React + TypeScript 前端
│   ├── src/
│   │   ├── components/      # React 组件
│   │   │   ├── FloatingPanel.tsx
│   │   │   ├── RecommendationSection.tsx
│   │   │   ├── SettingsWindow.tsx
│   │   │   └── ...
│   │   ├── contexts/        # React Context
│   │   │   └── ThemeContext.tsx
│   │   ├── App.tsx
│   │   └── main.tsx
│   ├── package.json
│   └── vite.config.ts
├── src-tauri/               # Rust 后端
│   ├── src/
│   │   ├── platform/        # 平台适配层
│   │   │   └── windows.rs
│   │   ├── services/        # 核心服务
│   │   │   ├── window_manager.rs
│   │   │   ├── process_monitor.rs
│   │   │   ├── recommendation.rs
│   │   │   ├── online_model.rs
│   │   │   └── database.rs
│   │   ├── commands.rs      # Tauri 命令
│   │   ├── lib.rs
│   │   ├── main.rs
│   │   └── types.rs
│   ├── icons/               # 应用图标
│   ├── Cargo.toml
│   └── tauri.conf.json
├── docs/                    # 设计文档
└── README.md
```

## 配置说明

| 设置项 | 说明 | 默认值 |
|--------|------|--------|
| 快捷键 | 唤起面板的全局快捷键 | `Ctrl+Shift+Space` |
| 推荐列表数量 | 显示的推荐组合数量 | 1 |
| 每列表最大窗口数 | 每个推荐组合包含的窗口数 | 5 |
| 常用组合最小停留时间 | 判定为常用组合的停留阈值 | 30 秒 |
| 鼠标空闲超时阈值 | 判定鼠标空闲的时间阈值 | 60 秒 |
| 频繁切换最大停留时间 | 判定为频繁切换的停留上限 | 5 秒 |
| 频繁切换最小频率 | 判定为频繁切换的最小次数 | 3 |
| 在线模型 API Key | OpenAI 兼容 API 密钥 | — |
| 在线模型端点 | API 端点 URL | — |
| 在线模型名称 | 模型名称 | — |

## 许可证

MIT License