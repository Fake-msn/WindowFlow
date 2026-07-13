# WindowFlow Toolchain Installation Script
# Usage: .\install-toolchain.ps1

$ErrorActionPreference = "Continue"

Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  WindowFlow Toolchain Installation" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# Set proxy
$env:HTTP_PROXY = "http://127.0.0.1:7890"
$env:HTTPS_PROXY = "http://127.0.0.1:7890"
Write-Host "INFO: Proxy set to http://127.0.0.1:7890" -ForegroundColor Green

# Set PATH
$env:Path = "$env:USERPROFILE\.cargo\bin;C:\nodejs\node-v20.18.3-win-x64;$env:Path"

# Get project root
$projectRoot = Split-Path -Parent $MyInvocation.MyCommand.Path

# Step 1: Verify base toolchain
Write-Host ""
Write-Host "--- Step 1: Verify base toolchain ---" -ForegroundColor Yellow

try {
    $rustVer = rustc --version
    Write-Host "OK: $rustVer" -ForegroundColor Green
} catch {
    Write-Host "FAIL: Rust not installed" -ForegroundColor Red
    exit 1
}

try {
    $cargoVer = cargo --version
    Write-Host "OK: $cargoVer" -ForegroundColor Green
} catch {
    Write-Host "FAIL: Cargo not installed" -ForegroundColor Red
    exit 1
}

try {
    $nodeVer = node --version
    Write-Host "OK: Node.js $nodeVer" -ForegroundColor Green
} catch {
    Write-Host "FAIL: Node.js not installed" -ForegroundColor Red
    exit 1
}

try {
    $npmVer = npm --version
    Write-Host "OK: npm $npmVer" -ForegroundColor Green
} catch {
    Write-Host "FAIL: npm not installed" -ForegroundColor Red
    exit 1
}

# Step 2: Install Tauri CLI
Write-Host ""
Write-Host "--- Step 2: Install Tauri CLI ---" -ForegroundColor Yellow

$tauriInstalled = $false
try {
    $tauriVer = cargo tauri --version 2>$null
    if ($LASTEXITCODE -eq 0) {
        Write-Host "SKIP: Tauri CLI already installed: $tauriVer" -ForegroundColor Green
        $tauriInstalled = $true
    }
} catch {}

if (-not $tauriInstalled) {
    Write-Host "INSTALL: Installing Tauri CLI..." -ForegroundColor Cyan
    Write-Host "  This may take 10-20 minutes..." -ForegroundColor Gray
    
    cargo install tauri-cli --version "^2"
    
    if ($LASTEXITCODE -eq 0) {
        Write-Host "OK: Tauri CLI installed successfully" -ForegroundColor Green
    } else {
        Write-Host "FAIL: Tauri CLI installation failed" -ForegroundColor Red
        Write-Host ""
        Write-Host "Trying alternative: npm install..." -ForegroundColor Yellow
        npm install -g @tauri-apps/cli
        
        if ($LASTEXITCODE -eq 0) {
            Write-Host "OK: Tauri CLI (npm) installed successfully" -ForegroundColor Green
        } else {
            Write-Host "FAIL: Tauri CLI installation failed" -ForegroundColor Red
        }
    }
}

# Step 3: Download Cargo dependencies
Write-Host ""
Write-Host "--- Step 3: Download Cargo dependencies ---" -ForegroundColor Yellow

$srcTauri = Join-Path $projectRoot "src-tauri"

if (Test-Path $srcTauri) {
    Write-Host "INSTALL: Downloading Cargo dependencies..." -ForegroundColor Cyan
    Write-Host "  This may take 10-30 minutes..." -ForegroundColor Gray
    
    Push-Location $srcTauri
    cargo fetch
    
    if ($LASTEXITCODE -eq 0) {
        Write-Host "OK: Cargo dependencies downloaded" -ForegroundColor Green
    } else {
        Write-Host "FAIL: Cargo dependencies download failed" -ForegroundColor Red
    }
    Pop-Location
} else {
    Write-Host "SKIP: src-tauri directory not found" -ForegroundColor Yellow
}

# Step 4: Install frontend dependencies
Write-Host ""
Write-Host "--- Step 4: Install frontend dependencies ---" -ForegroundColor Yellow

$frontend = Join-Path $projectRoot "frontend"
$nodeModules = Join-Path $frontend "node_modules"

if (Test-Path $nodeModules) {
    Write-Host "SKIP: Frontend dependencies already installed" -ForegroundColor Green
} else {
    Write-Host "INSTALL: Installing frontend dependencies..." -ForegroundColor Cyan
    
    Push-Location $frontend
    npm install
    
    if ($LASTEXITCODE -eq 0) {
        Write-Host "OK: Frontend dependencies installed" -ForegroundColor Green
    } else {
        Write-Host "FAIL: Frontend dependencies installation failed" -ForegroundColor Red
    }
    Pop-Location
}

# Final verification
Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  Installation Complete - Final Check" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

$allOk = $true

# Verify Tauri CLI
try {
    $tauriVer = cargo tauri --version 2>$null
    if ($LASTEXITCODE -eq 0) {
        Write-Host "OK: Tauri CLI: $tauriVer" -ForegroundColor Green
    } else {
        Write-Host "FAIL: Tauri CLI not available" -ForegroundColor Red
        $allOk = $false
    }
} catch {
    Write-Host "FAIL: Tauri CLI not available" -ForegroundColor Red
    $allOk = $false
}

# Verify Cargo dependencies
$cargoLock = Join-Path $srcTauri "Cargo.lock"
if (Test-Path $cargoLock) {
    Write-Host "OK: Cargo.lock generated" -ForegroundColor Green
} else {
    Write-Host "FAIL: Cargo.lock not found" -ForegroundColor Red
    $allOk = $false
}

# Verify frontend dependencies
if (Test-Path $nodeModules) {
    Write-Host "OK: node_modules installed" -ForegroundColor Green
} else {
    Write-Host "FAIL: node_modules not found" -ForegroundColor Red
    $allOk = $false
}

Write-Host ""
if ($allOk) {
    Write-Host "========================================" -ForegroundColor Green
    Write-Host "  All toolchains installed!" -ForegroundColor Green
    Write-Host "========================================" -ForegroundColor Green
    Write-Host ""
    Write-Host "Next: Run verification script" -ForegroundColor Cyan
    Write-Host "  .\verify-system-deps.ps1" -ForegroundColor Gray
} else {
    Write-Host "========================================" -ForegroundColor Red
    Write-Host "  Some toolchains failed" -ForegroundColor Red
    Write-Host "========================================" -ForegroundColor Red
    Write-Host ""
    Write-Host "Check errors above and retry" -ForegroundColor Yellow
}
