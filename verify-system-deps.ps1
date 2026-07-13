# WindowFlow System Dependencies Verification
# Verify Visual Studio Build Tools, Windows SDK, WebView2

Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  WindowFlow System Dependencies Check" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

$allOk = $true
$warnings = @()
$errors = @()

# 1. Verify Visual Studio Build Tools
Write-Host "--- 1. Visual Studio Build Tools ---" -ForegroundColor Yellow

$vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
$vsFound = $false

if (Test-Path $vswhere) {
    Write-Host "INFO: vswhere.exe found" -ForegroundColor Gray
    
    try {
        $vsInstances = & $vswhere -all -format json -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 2>$null
        
        if ($vsInstances) {
            $instances = $vsInstances | ConvertFrom-Json
            
            if ($instances.Count -gt 0) {
                foreach ($inst in $instances) {
                    Write-Host "OK: $($inst.displayName)" -ForegroundColor Green
                    Write-Host "    Path: $($inst.installationPath)" -ForegroundColor Gray
                    Write-Host "    Version: $($inst.installationVersion)" -ForegroundColor Gray
                }
                $vsFound = $true
            }
        }
        
        if (-not $vsFound) {
            $allInstances = & $vswhere -all -format json 2>$null
            if ($allInstances) {
                $instances = $allInstances | ConvertFrom-Json
                foreach ($inst in $instances) {
                    Write-Host "WARN: $($inst.displayName) - Missing C++ workload" -ForegroundColor Yellow
                    $warnings += "Visual Studio installed but missing C++ workload"
                }
            } else {
                Write-Host "FAIL: Visual Studio not found" -ForegroundColor Red
                $errors += "Visual Studio Build Tools not installed"
                $allOk = $false
            }
        }
    } catch {
        Write-Host "FAIL: vswhere execution failed" -ForegroundColor Red
        $errors += "vswhere execution failed"
        $allOk = $false
    }
} else {
    Write-Host "FAIL: Visual Studio Build Tools not installed" -ForegroundColor Red
    Write-Host "      Download: https://visualstudio.microsoft.com/visual-cpp-build-tools/" -ForegroundColor Gray
    Write-Host "      Required: Desktop development with C++ workload" -ForegroundColor Gray
    $errors += "Visual Studio Build Tools not installed"
    $allOk = $false
}

# 2. Verify MSVC Compiler (cl.exe)
Write-Host ""
Write-Host "--- 2. MSVC Compiler (cl.exe) ---" -ForegroundColor Yellow

$clFound = $false

try {
    $clResult = cl.exe 2>&1
    if ($LASTEXITCODE -eq 0 -or $clResult -match "Microsoft") {
        Write-Host "OK: cl.exe in PATH" -ForegroundColor Green
        $clFound = $true
    }
} catch {}

if (-not $clFound) {
    $clPaths = @(
        "C:\Program Files\Microsoft Visual Studio\2022\*\VC\Tools\MSVC\*\bin\Hostx64\x64\cl.exe",
        "C:\Program Files (x86)\Microsoft Visual Studio\2022\*\VC\Tools\MSVC\*\bin\Hostx64\x64\cl.exe",
        "C:\Program Files\Microsoft Visual Studio\2019\*\VC\Tools\MSVC\*\bin\Hostx64\x64\cl.exe",
        "C:\Program Files (x86)\Microsoft Visual Studio\2019\*\VC\Tools\MSVC\*\bin\Hostx64\x64\cl.exe"
    )
    
    foreach ($pattern in $clPaths) {
        $found = Get-ChildItem -Path $pattern -ErrorAction SilentlyContinue | Select-Object -First 1
        if ($found) {
            Write-Host "OK: cl.exe found: $($found.FullName)" -ForegroundColor Green
            Write-Host "    WARN: Not in PATH, may need Developer Command Prompt" -ForegroundColor Yellow
            $clFound = $true
            $warnings += "cl.exe installed but not in PATH"
            break
        }
    }
    
    if (-not $clFound) {
        Write-Host "FAIL: cl.exe not found" -ForegroundColor Red
        $errors += "MSVC Compiler (cl.exe) not found"
        $allOk = $false
    }
}

# 3. Verify Windows SDK
Write-Host ""
Write-Host "--- 3. Windows SDK ---" -ForegroundColor Yellow

$sdkFound = $false
$sdkPaths = @(
    "C:\Program Files (x86)\Windows Kits\10\Include",
    "C:\Program Files\Windows Kits\10\Include"
)

foreach ($sdkPath in $sdkPaths) {
    if (Test-Path $sdkPath) {
        $sdkVersions = Get-ChildItem -Path $sdkPath -Directory -ErrorAction SilentlyContinue | 
                       Where-Object { $_.Name -match "^\d+\.\d+\.\d+" } |
                       Sort-Object Name -Descending
        
        if ($sdkVersions.Count -gt 0) {
            $latestSdk = $sdkVersions[0]
            Write-Host "OK: Windows SDK found: $($latestSdk.Name)" -ForegroundColor Green
            Write-Host "    Path: $sdkPath" -ForegroundColor Gray
            
            $versionParts = $latestSdk.Name.Split(".")
            if ($versionParts.Count -ge 3) {
                $build = [int]$versionParts[2]
                if ($build -ge 19041) {
                    Write-Host "OK: SDK version >= 10.0.19041" -ForegroundColor Green
                } else {
                    Write-Host "WARN: SDK version < 10.0.19041" -ForegroundColor Yellow
                    $warnings += "Windows SDK version old ($($latestSdk.Name)), recommend >= 10.0.19041"
                }
            }
            
            $sdkFound = $true
            break
        }
    }
}

if (-not $sdkFound) {
    Write-Host "FAIL: Windows SDK not found" -ForegroundColor Red
    $errors += "Windows SDK not installed"
    $allOk = $false
}

# 4. Verify WebView2 Runtime
Write-Host ""
Write-Host "--- 4. WebView2 Runtime ---" -ForegroundColor Yellow

$webview2Found = $false

$regPaths = @(
    "HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}",
    "HKLM:\SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}",
    "HKCU:\SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}"
)

foreach ($regPath in $regPaths) {
    try {
        $webview2 = Get-ItemProperty -Path $regPath -ErrorAction SilentlyContinue
        if ($webview2) {
            $version = $webview2.pv
            Write-Host "OK: WebView2 Runtime installed: $version" -ForegroundColor Green
            Write-Host "    Registry: $regPath" -ForegroundColor Gray
            $webview2Found = $true
            break
        }
    } catch {}
}

if (-not $webview2Found) {
    $webview2Paths = @(
        "${env:ProgramFiles(x86)}\Microsoft\EdgeWebView\Application\msedgewebview2.exe",
        "$env:ProgramFiles\Microsoft\EdgeWebView\Application\msedgewebview2.exe"
    )
    
    foreach ($wvPath in $webview2Paths) {
        if (Test-Path $wvPath) {
            Write-Host "OK: WebView2 Runtime found: $wvPath" -ForegroundColor Green
            $webview2Found = $true
            break
        }
    }
}

if (-not $webview2Found) {
    Write-Host "FAIL: WebView2 Runtime not found" -ForegroundColor Red
    Write-Host "      Download: https://developer.microsoft.com/en-us/microsoft-edge/webview2/" -ForegroundColor Gray
    $errors += "WebView2 Runtime not installed"
    $allOk = $false
}

# 5. Verify Rust toolchain
Write-Host ""
Write-Host "--- 5. Rust Toolchain ---" -ForegroundColor Yellow

$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"

try {
    $toolchains = rustup toolchain list 2>$null
    if ($toolchains) {
        Write-Host "OK: Installed toolchains:" -ForegroundColor Green
        foreach ($tc in ($toolchains -split "`n")) {
            $tc = $tc.Trim()
            if ($tc) {
                if ($tc -match "default") {
                    Write-Host "    $tc (default)" -ForegroundColor Green
                } else {
                    Write-Host "    $tc" -ForegroundColor Gray
                }
            }
        }
    } else {
        Write-Host "WARN: No Rust toolchain found" -ForegroundColor Yellow
        $warnings += "Rust toolchain may need reinstall"
    }
} catch {
    Write-Host "FAIL: rustup execution failed" -ForegroundColor Red
    $errors += "rustup execution failed"
    $allOk = $false
}

# 6. Verify Node.js toolchain
Write-Host ""
Write-Host "--- 6. Node.js Toolchain ---" -ForegroundColor Yellow

$env:Path = "C:\nodejs\node-v20.18.3-win-x64;$env:Path"

try {
    $nodeVer = node --version
    Write-Host "OK: Node.js: $nodeVer" -ForegroundColor Green
} catch {
    Write-Host "FAIL: Node.js not available" -ForegroundColor Red
    $errors += "Node.js not available"
    $allOk = $false
}

try {
    $npmVer = npm --version
    Write-Host "OK: npm: $npmVer" -ForegroundColor Green
} catch {
    Write-Host "FAIL: npm not available" -ForegroundColor Red
    $errors += "npm not available"
    $allOk = $false
}

# Summary
Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  Verification Summary" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

if ($errors.Count -gt 0) {
    Write-Host "Errors ($($errors.Count)):" -ForegroundColor Red
    foreach ($err in $errors) {
        Write-Host "  - $err" -ForegroundColor Red
    }
    Write-Host ""
}

if ($warnings.Count -gt 0) {
    Write-Host "Warnings ($($warnings.Count)):" -ForegroundColor Yellow
    foreach ($warn in $warnings) {
        Write-Host "  - $warn" -ForegroundColor Yellow
    }
    Write-Host ""
}

if ($allOk) {
    Write-Host "========================================" -ForegroundColor Green
    Write-Host "  All system dependencies verified!" -ForegroundColor Green
    Write-Host "========================================" -ForegroundColor Green
    Write-Host ""
    Write-Host "Ready to build WindowFlow:" -ForegroundColor Cyan
    Write-Host "  cd src-tauri" -ForegroundColor Gray
    Write-Host "  cargo tauri dev" -ForegroundColor Gray
} else {
    Write-Host "========================================" -ForegroundColor Red
    Write-Host "  Some system dependencies missing" -ForegroundColor Red
    Write-Host "========================================" -ForegroundColor Red
    Write-Host ""
    Write-Host "Install missing dependencies and retry" -ForegroundColor Yellow
    Write-Host ""
    Write-Host "Quick install guide:" -ForegroundColor Cyan
    Write-Host "  1. Visual Studio Build Tools:" -ForegroundColor Gray
    Write-Host "     https://visualstudio.microsoft.com/visual-cpp-build-tools/" -ForegroundColor Gray
    Write-Host "     Install 'Desktop development with C++' workload" -ForegroundColor Gray
    Write-Host ""
    Write-Host "  2. WebView2 Runtime:" -ForegroundColor Gray
    Write-Host "     https://developer.microsoft.com/en-us/microsoft-edge/webview2/" -ForegroundColor Gray
    Write-Host "     Download Evergreen Bootstrapper" -ForegroundColor Gray
}
