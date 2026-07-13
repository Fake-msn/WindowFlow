# WindowFlow 工具链需求评估

## 项目依赖分析

基于项目配置文件分析：
- **后端**: Tauri 2.0 + Rust (windows crate 0.57)
- **前端**: React 18.3 + TypeScript 5.5 + Vite 5.3 + Tailwind CSS 3.4
- **平台**: Windows (需要 Win32 API 支持)

## 必需工具链

### 1. Rust 工具链
- **用途**: 后端核心逻辑编译
- **组件**:
  - `rustc` - Rust 编译器
  - `cargo` - 包管理器和构建工具
  - `rustup` - 工具链管理器
- **版本要求**: stable 1.70+ (推荐 1.75+)
- **安装状态**: ❌ 未安装
- **关键依赖**:
  - tauri 2.0
  - windows 0.57 (Win32 API)
  - rusqlite 0.31 (SQLite)
  - tokio 1 (异步运行时)

### 2. Node.js 工具链
- **用途**: 前端 UI 构建和开发
- **组件**:
  - `node` - JavaScript 运行时
  - `npm` - 包管理器
- **版本要求**: Node.js 20.x LTS (20.11+)
- **安装状态**: ❌ 未安装
- **关键依赖**:
  - react 18.3
  - typescript 5.5
  - vite 5.3
  - tailwindcss 3.4
  - @tauri-apps/api 2.0

### 3. Tauri CLI
- **用途**: 构建和开发 Tauri 桌面应用
- **安装方式**: 通过 cargo install
- **命令**: `cargo install tauri-cli --version "^2.0"`
- **安装状态**: ❌ 未安装
- **用途**:
  - `cargo tauri dev` - 开发模式
  - `cargo tauri build` - 生产构建

### 4. 系统依赖 (Windows)
- **Visual Studio Build Tools**: C++ 编译工具
  - 需要安装 "Desktop development with C++" 工作负载
  - 包含 MSVC 编译器和 Windows SDK
- **WebView2**: Windows 10/11 通常已内置
  - Tauri 2.0 需要 WebView2 运行时
- **安装状态**: 需要验证

## 当前环境问题

1. **Rust 工具链缺失**
   - rustup 可能已安装但无活跃工具链
   - 需要安装 stable 工具链
   - 可能原因: 镜像源配置问题或网络问题

2. **Node.js 完全缺失**
   - 系统中未检测到 node 或 npm
   - 需要完整安装 Node.js 20.x LTS

3. **Tauri CLI 未安装**
   - 需要先安装 Rust 工具链
   - 然后通过 `cargo install tauri-cli` 安装

4. **环境变量未配置**
   - PATH 中缺少工具链路径
   - 需要重启终端或手动刷新

5. **Visual Studio Build Tools 未验证**
   - Rust 编译需要 MSVC 工具链
   - 需要验证是否已安装 "Desktop development with C++"

## 完整工具链需求清单

### 后端工具链 (Rust)

#### 1. Rust 工具链
```powershell
# 必需组件
rustc 1.70+          # Rust 编译器
cargo 1.70+          # 包管理器
rustup 1.25+         # 工具链管理器

# 安装命令
rustup default stable
```

#### 2. Rust 依赖 (src-tauri/Cargo.toml)
```toml
[dependencies]
tauri = "2.0"                    # Tauri 框架
tauri-plugin-shell = "2.0"       # Shell 插件
serde = "1"                      # 序列化
serde_json = "1"                 # JSON 处理
tokio = "1"                      # 异步运行时
chrono = "0.4"                   # 日期时间
rusqlite = "0.31"                # SQLite 数据库
sha2 = "0.10"                    # SHA256 哈希
log = "0.4"                      # 日志
env_logger = "0.11"              # 日志初始化
thiserror = "1.0"                # 错误处理
lazy_static = "1.4"              # 静态变量

[windows]
windows = "0.57"                 # Windows API
```

#### 3. Cargo 配置 (.cargo/config.toml)
```toml
[source.crates-io]
replace-with = 'ustc'

[source.ustc]
registry = "sparse+https://mirrors.ustc.edu.cn/crates.io-index/"

[net]
git-fetch-with-cli = true
```

### 前端工具链 (Node.js)

#### 1. Node.js 运行时
```powershell
# 必需版本
node 20.x LTS          # JavaScript 运行时
npm 10.x               # 包管理器

# 安装命令
# 下载: https://nodejs.org/dist/v20.11.0/node-v20.11.0-x64.msi
```

#### 2. 前端依赖 (frontend/package.json)
```json
{
  "dependencies": {
    "react": "^18.3.1",
    "react-dom": "^18.3.1",
    "@tauri-apps/api": "^2.0.0"
  },
  "devDependencies": {
    "typescript": "^5.5.3",
    "vite": "^5.3.1",
    "@vitejs/plugin-react": "^4.3.1",
    "tailwindcss": "^3.4.4",
    "postcss": "^8.4.38",
    "autoprefixer": "^10.4.19",
    "@types/react": "^18.3.3",
    "@types/react-dom": "^18.3.0"
  }
}
```

#### 3. 构建配置
- **Vite**: 开发服务器端口 1420，生产构建目标 chrome105
- **TypeScript**: ES2020 目标，严格模式
- **Tailwind CSS**: 扫描 `./src/**/*.{js,ts,jsx,tsx}`
- **PostCSS**: 自动应用 Tailwind 和 Autoprefixer

### Tauri CLI

#### 安装和使用
```powershell
# 安装 Tauri CLI
cargo install tauri-cli --version "^2.0"

# 开发模式
cargo tauri dev

# 生产构建
cargo tauri build
```

### 系统依赖 (Windows)

#### 1. Visual Studio Build Tools
```powershell
# 必需工作负载
"Desktop development with C++"

# 包含组件
- MSVC 编译器
- Windows SDK (10.0.19041+)
- CMake 工具

# 下载地址
https://visualstudio.microsoft.com/visual-cpp-build-tools/
```

#### 2. WebView2 运行时
```powershell
# Tauri 2.0 必需
# Windows 10/11 通常已内置
# 如需安装: https://developer.microsoft.com/en-us/microsoft-edge/webview2/
```

#### 3. Windows SDK
```powershell
# 版本要求: 10.0.19041 或更高
# 通过 Visual Studio Build Tools 安装
```

## 网络配置

### 代理设置
```powershell
# 如果需要通过代理访问
$env:HTTP_PROXY = "http://127.0.0.1:7890"
$env:HTTPS_PROXY = "http://127.0.0.1:7890"
```

### 镜像源配置

#### Rust (已配置)
```toml
# .cargo/config.toml
[source.crates-io]
replace-with = 'ustc'

[source.ustc]
registry = "sparse+https://mirrors.ustc.edu.cn/crates.io-index/"
```

#### npm (可选)
```powershell
# 使用淘宝镜像
npm config set registry https://registry.npmmirror.com
```

## 验证清单

### 环境验证脚本
```powershell
# 1. 检查 Rust
rustc --version
cargo --version

# 2. 检查 Node.js
node --version
npm --version

# 3. 检查 Tauri CLI
cargo tauri --version

# 4. 检查系统依赖
# Visual Studio Build Tools (手动检查)
# WebView2 (自动检测)
```

### 构建验证
```powershell
# 1. 后端编译
cd src-tauri
cargo check

# 2. 前端依赖安装
cd frontend
npm install

# 3. 完整构建
cargo tauri dev
```

## 常见问题

### 1. Rust 编译失败
- 检查 Visual Studio Build Tools 是否安装
- 确认 Windows SDK 版本 >= 10.0.19041
- 验证 `cl.exe` 在 PATH 中

### 2. Node.js 版本不兼容
- 确保使用 Node.js 20.x LTS
- 清理缓存: `npm cache clean --force`
- 重新安装: `rm -rf node_modules package-lock.json`

### 3. Tauri CLI 安装失败
- 检查网络连接或配置代理
- 使用镜像源加速
- 验证 Rust 工具链完整性: `rustup check`

### 4. WebView2 缺失
- Windows 10/11 通常已内置
- 如需安装，下载 Evergreen Bootstrapper
- 验证: `Get-ItemProperty -Path 'HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}'`

## 安装顺序建议

1. **Visual Studio Build Tools** (系统级)
2. **Rust 工具链** (用户级)
3. **Node.js 20.x LTS** (系统级)
4. **Tauri CLI** (用户级，通过 cargo)
5. **前端依赖** (项目级，通过 npm)

## 磁盘空间需求

- Rust 工具链: ~2 GB
- Node.js: ~200 MB
- 项目依赖: ~1 GB
- 构建产物: ~500 MB
- **总计**: ~3.7 GB

### 方案 A: 手动安装 (推荐)

#### 步骤 1: 安装 Rust
```powershell
# 使用官方安装器
Invoke-WebRequest -Uri https://win.rustup.rs/x86_64 -OutFile rustup-init.exe
.\rustup-init.exe -y
```

#### 步骤 2: 安装 Node.js
```powershell
# 下载安装包
Invoke-WebRequest -Uri https://nodejs.org/dist/v20.11.0/node-v20.11.0-x64.msi -OutFile node.msi
# 运行安装
Start-Process msiexec.exe -ArgumentList "/i node.msi /quiet /norestart" -Wait
```

#### 步骤 3: 安装 Tauri CLI
```powershell
cargo install tauri-cli --version "^2.0"
```

#### 步骤 4: 验证安装
```powershell
rustc --version
cargo --version
node --version
npm --version
```

### 方案 B: 使用国内镜像

如果官方源访问缓慢，可以使用：

#### Rust (USTC 镜像)
```powershell
$env:RUSTUP_DIST_SERVER = "https://mirrors.ustc.edu.cn/rust-static"
$env:RUSTUP_UPDATE_ROOT = "https://mirrors.ustc.edu.cn/rust-static/rustup"
rustup default stable
```

#### Node.js (npmmirror)
```powershell
# 下载链接
https://npmmirror.com/mirrors/node/v20.11.0/node-v20.11.0-x64.msi
```

### 方案 C: 离线安装

如果网络完全不可用：
1. 在有网络的机器上下载工具链安装包
2. 传输到目标机器
3. 运行离线安装程序

## 后续命令预防措施

为避免后续命令执行出错，建议：

1. **在执行任何构建命令前**，先验证工具链：
   ```powershell
   # 检查 Rust
   if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
       Write-Error "Rust 工具链未安装"
       exit 1
   }
   
   # 检查 Node.js
   if (-not (Get-Command node -ErrorAction SilentlyContinue)) {
       Write-Error "Node.js 未安装"
       exit 1
   }
   ```

2. **使用条件执行**：
   ```powershell
   # 仅在工具链可用时执行
   if (Get-Command cargo -ErrorAction SilentlyContinue) {
       cargo build
   } else {
       Write-Warning "跳过构建: Rust 工具链不可用"
   }
   ```

3. **创建环境检查脚本**：
   已创建 `check-environment.ps1` 用于验证所有必需工具

## 下一步行动

1. 用户需要手动安装 Rust 和 Node.js
2. 安装完成后重启终端
3. 运行 `check-environment.ps1` 验证环境
4. 然后可以继续执行构建任务

## 替代方案

如果无法安装工具链，可以：
- 仅完成代码编写，不执行编译
- 生成完整的实现计划文档
- 提供详细的构建指南供用户在自己的环境中执行
