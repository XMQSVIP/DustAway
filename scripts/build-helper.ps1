$ErrorActionPreference = 'Stop'
$taskRoot = Split-Path -Parent $PSScriptRoot
Push-Location $taskRoot
try {
    cargo build -p dustaway-helper --release
    if ($LASTEXITCODE -ne 0) { throw 'Helper build failed' }
    New-Item -ItemType Directory -Force -Path (Join-Path $taskRoot 'src-tauri\binaries') | Out-Null
    Copy-Item -LiteralPath (Join-Path $taskRoot 'target\release\dustaway-helper.exe') -Destination (Join-Path $taskRoot 'src-tauri\binaries\dustaway-helper-x86_64-pc-windows-msvc.exe') -Force
} finally { Pop-Location }
