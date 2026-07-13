# WindowFlow 项目工具链需求评估报告

## 项目概述

**项目名称**: WindowFlow  
**项目类型**: Tauri 2.0 桌面应用  
**目标平台**: Windows (优先)  
**技术栈**: Rust + React + TypeScript

---

## 一、工具链需求清单

### 1.1 后端工具链 (Rust)

#### 必需组件

| 组件 | 版本要求 | 用途 | 状态 |
|------|---------|------|------|
| **rustc** | 1.70+ (推荐 1.75+) | Rust 编译器 | ✅ 已安装 (1.97.0) |
| **cargo** | 1.70+ | 包管理器 | ✅ 已安装 (1.97.0) |
| **rustup** | 1.25+ | 工具链管理器 | ✅ 已安装 |
| **tauri-cli** | 2.0+ | Tauri 开发工具 | ❌ 未安装 |

#### Cargo 依赖包

**核心依赖** (来自 `src-tauri/Cargo.toml`):

```toml
[dependencies]
tauri = "2.0"                    # Tauri 框架核心
tauri-plugin-shell = "2.0"       # Shell 插件
serde = "1"                      # 序列化框架
serde_json = "1"                 # JSON 处理
tokio = "1"                      # 异步运行时
chrono = "0.4"                   # 日期时间处理
rusqlite = "0.31"                # SQLite 数据库
sha2 = "0.10"                    # SHA-256 哈希
log = "0.4"                      # 日志框架
env_logger = "0.11"              # 日志初始化
thiserror = "1.0"                # 错误处理
lazy_static = "1.4"              # 静态变量

[windows]
windows = "0.57"                 # Windows API 绑定
```

**构建依赖**:

```toml
[build-dependencies]
tauri-build = "2.0"              # Tauri 构建脚本
```

**状态**: ❌ 未下载（需要网络访问 crates.io）

---

### 1.2 前端工具链 (Node.js)

#### 必需组件

| 组件 | 版本要求 | 用途 | 状态 |
|------|---------|------|------|
| **node** | 20.x LTS | JavaScript 运行时 | ✅ 已安装 (20.18.3) |
| **npm** | 10.x | 包管理器 | ✅ 已安装 (10.8.2) |

#### npm 依赖包

**生产依赖** (来自 `frontend/package.json`):

```json
{
  "dependencies": {
    "@tauri-apps/api": "^2.0.0",  // Tauri API 绑定
    "react": "^18.3.1",            // React 核心
    "react-dom": "^18.3.1"         // React DOM
  }
}
```

**开发依赖**:

```json
{
  "devDependencies": {
    "@types/react": "^18.3.3",           // React 类型定义
    "@types/react-dom": "^18.3.0",       // React DOM 类型
    "@vitejs/plugin-react": "^4.3.1",    // Vite React 插件
    "autoprefixer": "^10.4.19",          // CSS 自动前缀
    "postcss": "^8.4.38",                // CSS 后处理器
    "tailwindcss": "^3.4.4",             // Tailwind CSS
    "typescript": "^5.5.3",              // TypeScript 编译器
    "vite": "^5.3.1"                     // Vite 构建工具
  }
}
```

**状态**: ✅ 已安装 (136 packages)

---

### 1.3 系统依赖 (Windows)

#### 必需组件

| 组件 | 版本要求 | 用途 | 状态 |
|------|---------|------|------|
| **Visual Studio Build Tools** | 2019+ | C++ 编译工具链 | ❓ 未验证 |
| **Windows SDK** | 10.0.19041+ | Windows API 头文件 | ❓ 未验证 |
| **WebView2 Runtime** | 最新版 | Tauri 渲染引擎 | ❓ 未验证 |

#### Visual Studio Build Tools 必需工作负载

- **Desktop development with C++** (必需)
  - MSVC 编译器
  - Windows SDK
  - CMake 工具

---

### 1.4 构建配置

#### Cargo 配置 (`.cargo/config.toml`)

```toml
[source.crates-io]
replace-with = 'rsproxy-sparse'

[source.rsproxy-sparse]
registry = "sparse+https://rsproxy.cn/crates.io-index/"

[registries.rsproxy]
index = "https://rsproxy.cn/crates.io-index/"

[net]
git-fetch-with-cli = true
```

**状态**: ✅ 已配置 (使用 rsproxy 镜像)

#### Vite 配置 (`frontend/vite.config.ts`)

```typescript
{
  plugins: [react()],
  server: {
    port: 1420,
    strictPort: true
  },
  build: {
    target: "chrome105"
  }
}
```

**状态**: ✅ 已配置

#### TypeScript 配置 (`frontend/tsconfig.json`)

```json
{
  "compilerOptions": {
    "target": "ES2020",
    "module": "ESNext",
    "jsx": "react-jsx",
    "strict": true
  }
}
```

**状态**: ✅ 已配置

---

## 二、当前环境状态

### 2.1 已安装工具链

✅ **Rust 工具链**
- rustc: 1.97.0
- cargo: 1.97.0
- rustup: 已安装

✅ **Node.js 工具链**
- node: 20.18.3
- npm: 10.8.2

✅ **前端依赖**
- 136 packages 已安装
- 2 个安全漏洞（可修复）

✅ **构建配置**
- Cargo 镜像配置完成
- Vite 配置完成
- TypeScript 配置完成

### 2.2 缺失工具链

❌ **Tauri CLI**
- 状态: 未安装
- 原因: 网络连接问题 (无法访问 crates.io)
- 影响: 无法运行 `cargo tauri dev` 和 `cargo tauri build`

❌ **Cargo 依赖包**
- 状态: 未下载
- 原因: 网络连接问题
- 影响: 无法编译 Rust 后端

❓ **Visual Studio Build Tools**
- 状态: 未验证
- 影响: Rust 编译可能失败

❓ **Windows SDK**
- 状态: 未验证
- 影响: Rust 编译可能失败

❓ **WebView2 Runtime**
- 状态: 未验证
- 影响: Tauri 应用无法运行

---

## 三、缺失工具链详细分析

### 3.1 Tauri CLI

**重要性**: 🔴 高  
**用途**:
- `cargo tauri dev` - 开发模式
- `cargo tauri build` - 生产构建
- `cargo tauri info` - 环境检查

**安装命令**:
```powershell
cargo install tauri-cli --version "^2"
```

**当前问题**:
- 网络连接失败 (无法访问 index.crates.io:443)
- 已配置 rsproxy 镜像，但仍未生效

**解决方案**:
1. 使用代理: `$env:HTTPS_PROXY = "http://127.0.0.1:7890"`
2. 验证镜像配置
3. 手动下载安装

---

### 3.2 Cargo 依赖包

**重要性**: 🔴 高  
**用途**:
- 编译 Rust 后端
- 提供所有依赖库

**安装命令**:
```powershell
cd src-tauri
cargo build
```

**当前问题**:
- 与 Tauri CLI 相同的网络问题
- 需要下载约 200+ 个依赖包

**解决方案**:
- 同 Tauri CLI

---

### 3.3 Visual Studio Build Tools

**重要性**: 🔴 高  
**用途**:
- 提供 MSVC 编译器
- 编译 Rust 代码 (windows crate 需要)
- 链接 Windows API

**验证命令**:
```powershell
# 检查 cl.exe 是否在 PATH 中
cl.exe

# 检查 Visual Studio 安装
& "C:\Program Files (x86)\Microsoft Visual Studio\Installer\vswhere.exe" -all
```

**安装步骤**:
1. 下载 Visual Studio Build Tools 2022
2. 安装 "Desktop development with C++" 工作负载
3. 确保包含 Windows SDK 10.0.19041+

**下载地址**:
https://visualstudio.microsoft.com/visual-cpp-build-tools/

---

### 3.4 Windows SDK

**重要性**: 🔴 高  
**用途**:
- 提供 Windows API 头文件
- 链接系统库

**验证命令**:
```powershell
# 检查 SDK 版本
Get-ChildItem "C:\Program Files (x86)\Windows Kits\10\Include" | Select-Object Name
```

**安装**:
- 通常随 Visual Studio Build Tools 一起安装
- 确保版本 >= 10.0.19041

---

### 3.5 WebView2 Runtime

**重要性**: 🟡 中  
**用途**:
- Tauri 应用的渲染引擎
- 显示前端 UI

**验证命令**:
```powershell
# 检查 WebView2 是否已安装
Get-ItemProperty -Path 'HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}' -ErrorAction SilentlyContinue
```

**安装**:
- Windows 10/11 通常已内置
- 如需安装: https://developer.microsoft.com/en-us/microsoft-edge/webview2/

---

## 四、环境验证脚本

### 4.1 完整验证脚本

创建 `verify-toolchain.ps1`:

```powershell
# WindowFlow 工具链验证脚本

Write-Host "=== WindowFlow 工具链验证 ===" -ForegroundColor Cyan
Write-Host ""

# 1. Rust 工具链
Write-Host "1. Rust 工具链" -ForegroundColor Yellow
try {
    $rustc = rustc --version
    Write-Host "   ✅ rustc: $rustc" -ForegroundColor Green
} catch {
    Write-Host "   ❌ rustc 未安装" -ForegroundColor Red
}

try {
    $cargo = cargo --version
    Write-Host "   ✅ cargo: $cargo" -ForegroundColor Green
} catch {
    Write-Host "   ❌ cargo 未安装" -ForegroundColor Red
}

try {
    $tauri = cargo tauri --version
    Write-Host "   ✅ tauri-cli: $tauri" -ForegroundColor Green
} catch {
    Write-Host "   ❌ tauri-cli 未安装" -ForegroundColor Red
}

Write-Host ""

# 2. Node.js 工具链
Write-Host "2. Node.js 工具链" -ForegroundColor Yellow
try {
    $node = node --version
    Write-Host "   ✅ node: $node" -ForegroundColor Green
} catch {
    Write-Host "   ❌ node 未安装" -ForegroundColor Red
}

try {
    $npm = npm --version
    Write-Host "   ✅ npm: $npm" -ForegroundColor Green
} catch {
    Write-Host "   ❌ npm 未安装" -ForegroundColor Red
}

Write-Host ""

# 3. 系统依赖
Write-Host "3. 系统依赖" -ForegroundColor Yellow

# Visual Studio Build Tools
try {
    $vswhere = "C:\Program Files (x86)\Microsoft Visual Studio\Installer\vswhere.exe"
    if (Test-Path $vswhere) {
        $vs = & $vswhere -latest -property displayName
        Write-Host "   ✅ Visual Studio: $vs" -ForegroundColor Green
    } else {
        Write-Host "   ❓ Visual Studio Build Tools 未找到" -ForegroundColor Yellow
    }
} catch {
    Write-Host "   ❓ Visual Studio Build Tools 验证失败" -ForegroundColor Yellow
}

# WebView2
try {
    $webview2 = Get-ItemProperty -Path 'HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}' -ErrorAction SilentlyContinue
    if ($webview2) {
        Write-Host "   ✅ WebView2 Runtime 已安装" -ForegroundColor Green
    } else {
        Write-Host "   ❓ WebView2 Runtime 未找到" -ForegroundColor Yellow
    }
} catch {
    Write-Host "   ❓ WebView2 验证失败" -ForegroundColor Yellow
}

Write-Host ""

# 4. 依赖包
Write-Host "4. 依赖包" -ForegroundColor Yellow

# Cargo 依赖
if (Test-Path "src-tauri\Cargo.lock") {
    Write-Host "   ✅ Cargo 依赖已下载" -ForegroundColor Green
} else {
    Write-Host "   ❌ Cargo 依赖未下载" -ForegroundColor Red
}

# npm 依赖
if (Test-Path "frontend\node_modules") {
    Write-Host "   ✅ npm 依赖已安装" -ForegroundColor Green
} else {
    Write-Host "   ❌ npm 依赖未安装" -ForegroundColor Red
}

Write-Host ""
Write-Host "=== 验证完成 ===" -ForegroundColor Cyan
```

---

## 五、安装优先级

### 5.1 优先级分类

🔴 **高优先级** (必须立即安装)
1. Tauri CLI - 无法开发和构建
2. Cargo 依赖 - 无法编译后端
3. Visual Studio Build Tools - 可能编译失败

🟡 **中优先级** (建议安装)
4. WebView2 Runtime - 应用可能无法运行

🟢 **低优先级** (已安装或可选)
5. Rust 工具链 ✅
6. Node.js 工具链 ✅
7. 前端依赖 ✅

---

## 六、安装步骤

### 6.1 解决网络问题

**方案 1: 使用代理**
```powershell
$env:HTTP_PROXY = "http://127.0.0.1:7890"
$env:HTTPS_PROXY = "http://127.0.0.1:7890"
```

**方案 2: 验证镜像配置**
```powershell
# 检查 .cargo/config.toml 是否正确配置
cat .cargo\config.toml
```

### 6.2 安装 Tauri CLI

```powershell
# 设置代理
$env:HTTPS_PROXY = "http://127.0.0.1:7890"

# 安装 Tauri CLI
cargo install tauri-cli --version "^2"

# 验证安装
cargo tauri --version
```

### 6.3 下载 Cargo 依赖

```powershell
cd src-tauri
cargo build
```

### 6.4 验证 Visual Studio Build Tools

```powershell
# 检查 cl.exe
cl.exe

# 如果未找到，安装 Visual Studio Build Tools 2022
# 下载地址: https://visualstudio.microsoft.com/visual-cpp-build-tools/
```

### 6.5 验证 WebView2

```powershell
# 检查 WebView2
Get-ItemProperty -Path 'HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}' -ErrorAction SilentlyContinue

# 如果未找到，下载安装
# 下载地址: https://developer.microsoft.com/en-us/microsoft-edge/webview2/
```

---

## 七、后续命令执行预防

### 7.1 可能失败的命令

❌ **cargo tauri dev**
- 原因: Tauri CLI 未安装
- 解决: 先安装 Tauri CLI

❌ **cargo build**
- 原因: Cargo 依赖未下载
- 解决: 先运行 `cargo build` 下载依赖

❌ **cargo tauri build**
- 原因: Tauri CLI 未安装 + Cargo 依赖未下载
- 解决: 先安装所有缺失工具链

❌ **npm run build**
- 原因: 可能缺少 TypeScript 类型检查
- 解决: 确保前端依赖已安装

### 7.2 预防措施

1. **运行验证脚本**
   ```powershell
   .\verify-toolchain.ps1
   ```

2. **按优先级安装缺失工具链**
   - 先安装 Tauri CLI
   - 再下载 Cargo 依赖
   - 验证系统依赖

3. **测试构建**
   ```powershell
   # 测试后端编译
   cd src-tauri
   cargo check
   
   # 测试前端构建
   cd frontend
   npm run build
   
   # 测试完整构建
   cargo tauri build
   ```

---

## 八、总结

### 8.1 当前状态

- ✅ 基础工具链已安装 (Rust, Node.js)
- ✅ 前端依赖已安装
- ✅ 构建配置已完成
- ❌ Tauri CLI 未安装 (网络问题)
- ❌ Cargo 依赖未下载 (网络问题)
- ❓ 系统依赖未验证

### 8.2 下一步行动

1. **立即执行**:
   - 解决网络问题 (代理或镜像)
   - 安装 Tauri CLI
   - 下载 Cargo 依赖

2. **验证系统依赖**:
   - Visual Studio Build Tools
   - Windows SDK
   - WebView2 Runtime

3. **测试构建**:
   - 运行验证脚本
   - 测试后端编译
   - 测试前端构建
   - 测试完整构建

### 8.3 预计时间

- 安装 Tauri CLI: 5-10 分钟
- 下载 Cargo 依赖: 10-20 分钟
- 验证系统依赖: 5 分钟
- **总计**: 20-35 分钟

---

## 附录

### A. 相关文档

- [TOOLCHAIN-REQUIREMENTS.md](./TOOLCHAIN-REQUIREMENTS.md) - 原始工具链需求文档
- [CHECKPOINT-REVIEW.md](./CHECKPOINT-REVIEW.md) - 检查点审查报告
- [Tauri 官方文档](https://tauri.app/)
- [Rust 官方文档](https://www.rust-lang.org/)

### B. 下载链接

- [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)
- [WebView2 Runtime](https://developer.microsoft.com/en-us/microsoft-edge/webview2/)
- [Rust 安装器](https://rustup.rs/)
- [Node.js 下载](https://nodejs.org/)

### C. 镜像源

- **Rust crates**: https://rsproxy.cn/
- **npm 包**: https://registry.npmmirror.com/

---

**文档版本**: 1.0  
**创建时间**: 2026-07-12  
**最后更新**: 2026-07-12  
**作者**: AI Assistant
