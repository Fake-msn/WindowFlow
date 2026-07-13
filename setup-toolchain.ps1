# WindowFlow 工具链安装脚本
# 使用国内镜像安装 Rust 和 Node.js

Write-Host "=== WindowFlow 工具链安装 ===" -ForegroundColor Green

# 1. 安装 Rust (使用 USTC 镜像)
Write-Host "`n[1/2] 安装 Rust 工具链..." -ForegroundColor Yellow
$env:RUSTUP_DIST_SERVER = "https://mirrors.ustc.edu.cn/rust-static"
$env:RUSTUP_UPDATE_ROOT = "https://mirrors.ustc.edu.cn/rust-static/rustup"

if (-not (Get-Command rustc -ErrorAction SilentlyContinue)) {
    Write-Host "正在通过 USTC 镜像安装 Rust..."
    rustup default stable
    Write-Host "Rust 安装完成" -ForegroundColor Green
} else {
    Write-Host "Rust 已安装: $(rustc --version)" -ForegroundColor Green
}

# 刷新环境变量
$env:Path = [System.Environment]::GetEnvironmentVariable("Path","Machine") + ";" + [System.Environment]::GetEnvironmentVariable("Path","User")

# 2. 安装 Node.js (使用 npmmirror)
Write-Host "`n[2/2] 安装 Node.js..." -ForegroundColor Yellow

if (-not (Get-Command node -ErrorAction SilentlyContinue)) {
    Write-Host "正在下载 Node.js v20.11.0 (从 npmmirror)..."
    $nodeUrl = "https://npmmirror.com/mirrors/node/v20.11.0/node-v20.11.0-win-x64.zip"
    $downloadPath = "$env:TEMP\node.zip"
    $extractPath = "$env:LOCALAPPDATA\nodejs"
    
    Invoke-WebRequest -Uri $nodeUrl -OutFile $downloadPath
    
    Write-Host "解压 Node.js..."
    if (Test-Path $extractPath) {
        Remove-Item -Recurse -Force $extractPath
    }
    Expand-Archive -Path $downloadPath -DestinationPath $extractPath
    
    $nodeFolder = Get-ChildItem -Path $extractPath -Directory | Select-Object -First 1
    $nodePath = $nodeFolder.FullName
    
    # 添加到 PATH
    $env:Path = "$nodePath;$env:Path"
    $currentPath = [Environment]::SetEnvironmentVariable("Path", "User")
    if ($currentPath -notlike "*$nodePath*") {
        [Environment]::SetEnvironmentVariable("Path", "$nodePath;$currentPath", "User")
    }
    
    Remove-Item $downloadPath -Force
    Write-Host "Node.js 安装完成" -ForegroundColor Green
} else {
    Write-Host "Node.js 已安装: $(node --version)" -ForegroundColor Green
}

# 刷新环境变量
$env:Path = [System.Environment]::GetEnvironmentVariable("Path","Machine") + ";" + [System.Environment]::GetEnvironmentVariable("Path","User")

# 3. 验证安装
Write-Host "`n[验证] 检查安装结果..." -ForegroundColor Yellow
Write-Host "Rust: $(rustc --version)" -ForegroundColor Cyan
Write-Host "Cargo: $(cargo --version)" -ForegroundColor Cyan
Write-Host "Node.js: $(node --version)" -ForegroundColor Cyan
Write-Host "npm: $(npm --version)" -ForegroundColor Cyan

Write-Host "`n=== 工具链安装完成 ===" -ForegroundColor Green
Write-Host "请重启终端以使用新安装的工具链" -ForegroundColor Yellow
