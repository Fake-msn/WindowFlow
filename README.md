# WindowFlow

![version](https://img.shields.io/badge/version-1.2.0-blue)
![platform](https://img.shields.io/badge/platform-Windows%2010%2F11-lightgrey)
![tech](https://img.shields.io/badge/Tauri-2.x-24C8DB)
![license](https://img.shields.io/badge/license-MIT-green)

<p align="right">
  <img src="Product_Definition.png" alt="WindowFlow Logo" width="280" />
</p>

智能桌面窗口管理工具 — 一键多屏幕窗口迁移、基于使用频率的智能推荐、本地工作流识别，数据全程本地加密存储。

## 下载安装

前往 [**Releases**](https://github.com/Fake-msn/WindowFlow/releases/latest) 或 [**下载页**](https://fake-msn.github.io/WindowFlow/) 获取最新版本：

| 安装包 | 说明 |
|--------|------|
| `WindowFlow_1.2.0_x64_en-US.msi` | MSI 安装程序（推荐） |

> 系统要求：Windows 10 / 11 (x64)，需 WebView2 运行时（Windows 11 已内置）。

## 核心特性

- **一键窗口迁移**：将多屏幕上的窗口快速迁移到指定屏幕，支持 DPI 感知缩放；迁移失败自动回滚到原始位置。
- **智能推荐**：在光标附近显示窗口组合建议，融合共现统计、停留时间、时间衰减权重、Apriori 频繁项集与 PrefixSpan 序列模式。
- **工作流识别**：本地规则引擎分析窗口使用模式，支持可选在线模型（OpenAI 兼容 API）增强推荐。
- **隐私优先**：本地处理为主，通过进程信息（HWND + PID）识别窗口而非截图；数据库经 SQLCipher 加密，密钥存于 Windows 凭据管理器。
- **深色/浅色主题**：支持深色、浅色及跟随系统主题切换。

## 技术栈

| 层级 | 技术 |
|------|------|
| 后端 | Rust |
| 桌面框架 | Tauri 2.x |
| UI | React + TypeScript + Tailwind CSS |
| 数据库 | SQLite + **SQLCipher 加密** |
| 推荐引擎 | 本地规则引擎（共现 / 时间衰减 / Apriori / PrefixSpan）+ 可选在线模型 |
| 平台 API | Win32、DWM、WinEvent Hook |

## 已实现功能

### 窗口管理
- 多屏幕窗口枚举与信息获取
- DPI 感知的窗口迁移与批量迁移
- **迁移失败自动回滚**（捕获并恢复窗口位置与显示状态）
- 窗口缩略图预览，带 **TTL + 容量上限 + 定期清理** 的外置缓存

### 进程监控
- 基于 **EVENT_SYSTEM_FOREGROUND 事件钩子** 的前台窗口追踪（事件驱动，零轮询空转）
- 窗口停留时间与切换频次统计
- 窗口销毁事件监听（EVENT_OBJECT_DESTROY）与自动清理
- 事件列表容量上限，防止内存无限增长

### 推荐引擎
- **本地规则推荐**：基于共现矩阵与停留时间的窗口组合推荐
- **时间衰减权重**：近期共现的组合权重更高（默认 72 小时半衰期）
- **Apriori 频繁项集挖掘**：识别经常一起使用的应用组合，生成"高频工作流"推荐
- **PrefixSpan 序列模式分析**：按操作序列预测最可能紧随其后的应用并排序
- **在线模型推荐**（可选）：通过 OpenAI 兼容 API 进行场景化窗口组合推荐
- 推荐列表数量与窗口数可配置；模型数据滚动清理

### 数据存储（加密）
- **SQLCipher 全库加密**，密钥经 Windows 凭据管理器安全存储（首次运行自动生成 256-bit 随机密钥）
- 数据库**外置**到 `%APPDATA%\WindowFlow`，独立于程序目录
- 窗口停留记录、共现统计、推荐设置、模型调用计数等持久化，启动加载 + 定期保存

### UI
- 浮动面板（毛玻璃效果）与窗口缩略图卡片
- Mission Control 风格窗口概览
- 设置面板（快捷键、推荐参数、外观、在线模型）
- 深色 / 浅色 / 跟随系统主题切换，设置项悬停提示

### 输入
- 全局快捷键触发面板（默认 `Ctrl+Shift+Space`）
- 鼠标侧键触发面板（前进 / 后退按钮可配置）

## 安全与隐私

### 数据加密
- SQLite 数据库使用 **SQLCipher** 加密（静态链接 OpenSSL）
- 加密密钥存储于 **Windows 凭据管理器**，不落盘为明文
- 数据库外置到用户数据目录，避免写入程序安装目录

### 本地数据处理
**存储内容**：进程名、PID、窗口标题哈希（SHA-256，不可逆）、时间戳、使用频率统计。
**不存储**：完整窗口标题、用户输入、截图 / 窗口内容、文件路径、网络活动。

### 在线模型（可选，默认关闭）
- 仅发送匿名化特征；用户可完全禁用
- 请求设置连接 / 读取超时；数据可配置保留周期与清理

### 运行时加固
- 焦点监控采用事件钩子替代轮询，降低 CPU 占用
- 进程名缓存减少系统调用频率
- 提供 MSI 安装包格式

## 架构概览

```
┌─────────────────────────────────────┐
│         UI 层 (Tauri + React)        │
│  浮动面板 / 缩略图预览 / 设置面板    │
└─────────────────────────────────────┘
              ↓
┌─────────────────────────────────────┐
│         服务层 (Rust)                │
│  窗口管理 / 进程监控 / 推荐引擎      │
│  算法(Apriori/PrefixSpan/时间衰减)   │
│  在线模型客户端 / 缩略图缓存         │
└─────────────────────────────────────┘
              ↓
┌─────────────────────────────────────┐
│       平台适配层 (Win32 / DWM)       │
└─────────────────────────────────────┘
              ↓
┌─────────────────────────────────────┐
│    数据存储层 (SQLCipher 加密)       │
│  %APPDATA%\WindowFlow\windowflow.db │
└─────────────────────────────────────┘
```

## 从源码构建

### 环境要求
- [Node.js](https://nodejs.org/) 20+
- [Rust](https://www.rust-lang.org/) 1.70+ 与 Tauri CLI（`cargo install tauri-cli`）
- Windows 10/11
- **SQLCipher 依赖**（从源码编译 OpenSSL 所需）：
  - [Strawberry Perl](https://strawberryperl.com/)
  - [NASM](https://www.nasm.us/)

### 开发

```bash
cd frontend && npm install   # 安装前端依赖
cd .. && cargo tauri dev     # 运行开发模式
```

### 打包（生成 MSI 安装包）

```bash
cargo tauri build
```

> **注意**：数据库启用 SQLCipher（静态链接 OpenSSL），打包时会从源码编译 OpenSSL，需 Perl 与 NASM 在 `PATH` 中。
> 若项目路径**包含空格**，须将 `CARGO_TARGET_DIR` 指向无空格路径（如 `C:\wf_target`），否则 OpenSSL 构建会因空格路径失败。

## 项目结构

```
WindowFlow/
├── frontend/                    # React + TypeScript 前端
│   └── src/
│       ├── components/          # 浮动面板 / 推荐 / 设置 / 缩略图等组件
│       ├── contexts/            # ThemeContext 主题
│       ├── App.tsx
│       └── main.tsx
├── src-tauri/                   # Rust 后端
│   └── src/
│       ├── platform/
│       │   └── windows.rs       # Win32/DWM 平台适配、窗口迁移、缩略图
│       ├── services/
│       │   ├── window_manager.rs   # 窗口管理 + 迁移回滚
│       │   ├── process_monitor.rs  # EVENT_SYSTEM_FOREGROUND 事件钩子
│       │   ├── recommendation.rs   # 推荐引擎
│       │   ├── algorithms.rs       # 时间衰减 / Apriori / PrefixSpan
│       │   ├── online_model.rs     # 在线模型客户端
│       │   ├── thumbnail_cache.rs  # 缩略图缓存
│       │   ├── crypto_key.rs       # SQLCipher 密钥（凭据管理器）
│       │   └── database.rs         # SQLCipher 加密数据库
│       ├── commands.rs          # Tauri 命令
│       ├── lib.rs / main.rs / types.rs
│       └── tauri.conf.json
├── .github/workflows/           # GitHub Pages 部署工作流
├── docs/                        # 设计与实现文档
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
| 在线模型 API Key / 端点 / 名称 | OpenAI 兼容 API 配置（可选） | — |

## 目标平台

- **Phase 1**：Windows 10/11 —— 已实现
- **Phase 2**：macOS —— 规划中
- **Phase 3**：Linux（X11/Wayland）—— 规划中

## 许可证

MIT License
