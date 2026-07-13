# WindowFlow 环境检查脚本
# 用于验证所有必需工具链是否已正确安装

Write-Host "=== WindowFlow 环境检查 ===" -ForegroundColor Green

$allChecksPassed = $true

# 1. 检查 Rust
Write-Host "`n[1/4] 检查 Rust 工具链..." -ForegroundColor Yellow
if (Get-Command rustc -ErrorAction SilentlyContinue) {
    $rustVersion = rustc --version
    Write-Host "✓ Rust: $rustVersion" -ForegroundColor Green
} else {
    Write-Host "✗ Rust 未安装" -ForegroundColor Red
    $allChecksPassed = $false
}

# 2. 检查 Cargo
Write-Host "`n[2/4] 检查 Cargo..." -ForegroundColor Yellow
if (Get-Command cargo -ErrorAction SilentlyContinue) {
    $cargoVersion = cargo --version
    Write-Host "✓ Cargo: $cargoVersion" -ForegroundColor Green
} else {
    Write-Host "✗ Cargo 未安装" -ForegroundColor Red
    $allChecksPassed = $false
}

# 3. 检查 Node.js
Write-Host "`n[3/4] 检查 Node.js..." -ForegroundColor Yellow
if (Get-Command node -ErrorAction SilentlyContinue) {
    $nodeVersion = node --version
    Write-Host "✓ Node.js: $nodeVersion" -ForegroundColor Green
} else {
    Write-Host "✗ Node.js 未安装" -ForegroundColor Red
    $allChecksPassed = $false
}

# 4. 检查 npm
Write-Host "`n[4/4] 检查 npm..." -ForegroundColor Yellow
if (Get-Command npm -ErrorAction SilentlyContinue) {
    $npmVersion = npm --version
    Write-Host "✓ npm: $npmVersion" -ForegroundColor Green
} else {
    Write-Host "✗ npm 未安装" -ForegroundColor Red
    $allChecksPassed = $false
}

# 总结
Write-Host "`n=== 检查结果 ===" -ForegroundColor Cyan
if ($allChecksPassed) {
    Write-Host "✓ 所有必需工具链已安装" -ForegroundColor Green
    Write-Host "可以开始构建 WindowFlow 项目" -ForegroundColor Green
    exit 0
} else {
    Write-Host "✗ 部分工具链缺失" -ForegroundColor Red
    Write-Host "请参考 TOOLCHAIN-REQUIREMENTS.md 安装缺失的工具" -ForegroundColor Yellow
    exit 1
}
