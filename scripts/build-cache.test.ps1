# Read-only/WhatIf regression tests. Never invokes actual cache deletion.
$ErrorActionPreference = 'Stop'
$root = Join-Path ([IO.Path]::GetTempPath()) ('dustaway-build-cache-test-' + [guid]::NewGuid().ToString('N'))
$fixture = Join-Path $root 'workspace'
New-Item -ItemType Directory -Path "$fixture\scripts", "$fixture\target\debug", "$fixture\target\release", "$root\protected\incremental" -Force | Out-Null
Set-Content -LiteralPath "$root\.dustaway-test-only" -Value 'build-cache-script-test'
Set-Content -LiteralPath "$fixture\Cargo.toml" -Value '[workspace]'
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'build-cache.ps1') -Destination "$fixture\scripts\build-cache.ps1"
Set-Content -LiteralPath "$fixture\source.txt" -Value 'keep-source'
Set-Content -LiteralPath "$fixture\target\debug\cache.bin" -Value 'rebuildable'
Set-Content -LiteralPath "$fixture\target\release\app.exe" -Value 'keep-release'
Set-Content -LiteralPath "$root\protected\incremental\sentinel.txt" -Value 'keep-outside-target'
$script = "$fixture\scripts\build-cache.ps1"
$report = (& $script -Mode Report | Out-String)
if ($report -notmatch '2 files' -or $report -notmatch 'debug:' -or $report -notmatch 'release:') { throw "Invalid report: $report" }
$before = Get-Content -LiteralPath "$fixture\target\debug\cache.bin" -Raw
& $script -Mode Clean -Scope debug -WhatIf
if ((Get-Content -LiteralPath "$fixture\target\debug\cache.bin" -Raw) -ne $before) { throw 'WhatIf changed the cache' }
if (-not (Test-Path -LiteralPath "$fixture\source.txt") -or -not (Test-Path -LiteralPath "$fixture\target\release\app.exe")) { throw 'Non-selected content changed' }

$invalidRejected = $false
try { & $script -Scope '..' } catch { $invalidRejected = $true }
if (-not $invalidRejected) { throw 'Invalid scope accepted' }

# Make a separate workspace so no existing directory must be deleted to test
# an intermediate junction between target and the incremental subdirectory.
$linked = Join-Path $root 'linked-workspace'
New-Item -ItemType Directory -Path "$linked\scripts", "$linked\target" -Force | Out-Null
Set-Content -LiteralPath "$linked\Cargo.toml" -Value '[workspace]'
Copy-Item -LiteralPath $script -Destination "$linked\scripts\build-cache.ps1"
New-Item -ItemType Junction -Path "$linked\target\debug" -Target "$root\protected" | Out-Null
foreach ($scope in @('incremental', 'all')) {
    $rejected = $false
    try { & "$linked\scripts\build-cache.ps1" -Mode Report -Scope $scope | Out-Null }
    catch { $rejected = $_.Exception.Message -match 'reparse point' }
    if (-not $rejected) { throw "Junction accepted for scope $scope" }
}
if ((Get-Content -LiteralPath "$root\protected\incremental\sentinel.txt" -Raw).Trim() -ne 'keep-outside-target') { throw 'Protected content changed' }
Write-Output 'PASS: report, WhatIf, scope validation, intermediate and nested junction rejection.'
Write-Output "Small marked fixtures retained for inspection: $root"
