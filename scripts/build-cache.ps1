# Reports project-local Cargo output; deletion is explicit and supports -WhatIf.
[CmdletBinding(SupportsShouldProcess = $true, ConfirmImpact = 'Medium')]
param(
    [ValidateSet('Report', 'Clean')][string]$Mode = 'Report',
    [ValidateSet('all', 'debug', 'incremental')][string]$Scope = 'all'
)
$ErrorActionPreference = 'Stop'
$workspace = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).ProviderPath
$target = Join-Path $workspace 'target'
if (-not (Test-Path -LiteralPath (Join-Path $workspace 'Cargo.toml'))) {
    throw 'Cargo workspace not found.'
}
if (-not (Test-Path -LiteralPath $target)) {
    Write-Output 'No local target directory. Nothing to clean.'
    return
}
$selected = switch ($Scope) {
    'debug' { Join-Path $target 'debug' }
    'incremental' { Join-Path $target 'debug\incremental' }
    default { $target }
}
if (-not (Test-Path -LiteralPath $selected)) {
    Write-Output "Nothing to clean: $selected"
    return
}
# Include every selected ancestor (e.g. debug for the incremental scope), not
# just target. A junction in an intermediate directory must also be rejected.
$ancestor = Get-Item -LiteralPath $selected -Force
while ($null -ne $ancestor) {
    if ($ancestor -isnot [IO.DirectoryInfo] -or ($ancestor.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
        throw "Refusing a reparse point or non-directory: $($ancestor.FullName)"
    }
    $ancestor = $ancestor.Parent
}
$selected = (Resolve-Path -LiteralPath $selected).ProviderPath
if ($selected -ne $target -and -not $selected.StartsWith($target + '\', [StringComparison]::OrdinalIgnoreCase)) {
    throw "Target escapes the workspace: $selected"
}
$pending = New-Object 'System.Collections.Generic.Stack[string]'
$pending.Push($selected)
$sizes = @{}
[long]$files = 0
while ($pending.Count -gt 0) {
    $directory = $pending.Pop()
    $directoryInfo = Get-Item -LiteralPath $directory -Force
    if ($directoryInfo.Attributes -band [IO.FileAttributes]::ReparsePoint) {
        throw "Refusing reparse point: $directory"
    }
    foreach ($entry in (Get-ChildItem -LiteralPath $directory -Force)) {
        if ($entry.Attributes -band [IO.FileAttributes]::ReparsePoint) {
            throw "Refusing reparse point: $($entry.FullName)"
        }
        if ($entry.PSIsContainer) { $pending.Push($entry.FullName) }
        else {
            $relative = $entry.FullName.Substring($selected.Length + 1)
            $group = if ($relative.Contains('\')) { $relative.Split('\')[0] } else { '(files)' }
            $sizes[$group] = [long]$sizes[$group] + $entry.Length
            $files++
        }
    }
}
[long]$total = 0
foreach ($group in ($sizes.Keys | Sort-Object)) {
    $total += $sizes[$group]
    Write-Output ('{0}: {1:N3} GiB' -f $group, ($sizes[$group] / 1GB))
}
Write-Output ('Selected: {0}; {1:N3} GiB; {2} files (logical sizes)' -f $selected, ($total / 1GB), $files)
if ($Mode -eq 'Report') { return }

# No process is killed. Fail closed if a compiler or project app is active.
$active = @(Get-Process | Where-Object {
    $_.ProcessName -in @('cargo', 'rustc', 'clippy-driver', 'dustaway', 'dustaway-analysis-bench', 'r3-benchmark') -or
    ($_.Path -and $_.Path.StartsWith($target + '\', [StringComparison]::OrdinalIgnoreCase))
})
if ($active.Count -gt 0) {
    throw ('Close builds and project applications first: ' + (($active | ForEach-Object { "$($_.ProcessName) PID=$($_.Id)" }) -join ', '))
}
if ($PSCmdlet.ShouldProcess($selected, 'Delete rebuildable Cargo output')) {
    Remove-Item -LiteralPath $selected -Recurse -Force
    Write-Output 'Cache removed. The next build will recreate it.'
}
