# Temporary, standalone cleanup. Run manually with PowerShell.
# Deletes only G:\my_ai\DustAway\target. Does not require administrator rights.
[CmdletBinding()]
param([switch]$CheckOnly)

$ErrorActionPreference = 'Stop'
$workspace = 'G:\my_ai\DustAway'
$target = 'G:\my_ai\DustAway\target'
$expectedHash = '44A75B2F98E2548320E49C599F72826F37C5D98D432E091B286D72E0B1D6FE02'

try {
    if (-not (Test-Path -LiteralPath $target)) {
        Write-Host 'Nothing to clean: target is already absent.'
        exit 0
    }
    # Check every ancestor as well as the target before any recursive operation.
    foreach ($path in @('G:\', 'G:\my_ai', $workspace, $target)) {
        $item = Get-Item -LiteralPath $path -Force
        if (-not $item.PSIsContainer -or ($item.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
            throw "Not a regular directory: $path"
        }
    }
    $resolved = (Resolve-Path -LiteralPath $target).ProviderPath
    if (-not [string]::Equals($resolved, $target, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Unexpected resolved target: $resolved"
    }

    # Preserve and verify the usable delivery outside the build cache.
    $packages = @(Get-ChildItem -LiteralPath "$workspace\artifacts\portable" -File |
        Where-Object { $_.Name -like '*_0.18.0_analysis-million-test-20261002-r3_windows-x64.zip' })
    if ($packages.Count -ne 1 -or (Get-FileHash -LiteralPath $packages[0].FullName -Algorithm SHA256).Hash -ne $expectedHash) {
        throw 'The preserved R3 package is missing or its checksum does not match.'
    }
    $processes = @(Get-CimInstance Win32_Process | Where-Object {
        $_.Name -in @('cargo.exe', 'rustc.exe', 'dustaway.exe', 'dustaway-analysis-bench.exe')
    })
    if ($processes.Count -gt 0) {
        throw ('Close build/application processes first: ' + (($processes | ForEach-Object { "$($_.Name) (PID $($_.ProcessId))" }) -join ', '))
    }

    # Refuse junctions/symlinks anywhere inside the cache; never traverse them.
    $pending = New-Object 'System.Collections.Generic.Stack[string]'
    $pending.Push($target)
    [long]$bytes = 0
    [long]$files = 0
    while ($pending.Count -gt 0) {
        foreach ($entry in (Get-ChildItem -LiteralPath $pending.Pop() -Force)) {
            if ($entry.Attributes -band [IO.FileAttributes]::ReparsePoint) {
                throw "Reparse point found; no deletion performed: $($entry.FullName)"
            }
            if ($entry.PSIsContainer) { $pending.Push($entry.FullName) }
            else { $bytes += $entry.Length; $files++ }
        }
    }
    Write-Host ('Target: {0}; {1:N2} GiB; {2} files' -f $target, ($bytes / 1GB), $files)
    Write-Host ('Preserved package: ' + $packages[0].FullName)
    if ($CheckOnly) {
        Write-Host 'Checks passed. CheckOnly mode: nothing deleted.'
        exit 0
    }

    $before = (Get-PSDrive -Name G).Free
    Write-Host 'Deleting build cache. Keep this window open; this may take several minutes.'
    Remove-Item -LiteralPath $target -Recurse -Force
    if (Test-Path -LiteralPath $target) { throw 'Cleanup incomplete: target still exists.' }
    $after = (Get-PSDrive -Name G).Free
    Write-Host ('Done. G: free space {0:N2} -> {1:N2} GiB; change {2:N2} GiB.' -f ($before / 1GB), ($after / 1GB), (($after - $before) / 1GB))
    Write-Host 'Source, backups, node_modules and release packages were preserved. The next build will recreate its cache.'
} catch {
    Write-Error ('Cleanup stopped: ' + $_.Exception.Message) -ErrorAction Continue
    Write-Host 'If deletion had already started, some cache files may have been removed. Close applications and rerun.'
    exit 1
}
