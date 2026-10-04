# Project-generated files only. Default: report; deletion requires -Mode Clean.
# Examples are documented in docs/project-cleanup-report.md.
[CmdletBinding(SupportsShouldProcess = $true, ConfirmImpact = 'Medium')]
param(
    [ValidateSet('Report', 'Clean')][string]$Mode = 'Report',
    [ValidateSet('None', 'Debug', 'All')][string]$RustScope = 'Debug',
    [switch]$IncludeDependencies,
    [switch]$IncludeLogs,
    [switch]$IncludeBenchmarkFixtures,
    [switch]$IncludeBrowserTestAssets,
    [switch]$IncludeBrowserArchives,
    [switch]$CurrentVersionOnly,
    [switch]$MeasureSize,
    [switch]$SkipE2EFixtures
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 2.0
$workspace = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..')).TrimEnd('\')

function Assert-PlainAncestors([string]$Path) {
    $entry = Get-Item -LiteralPath $Path -Force
    while ($null -ne $entry) {
        if ($entry.Attributes -band [IO.FileAttributes]::ReparsePoint) {
            throw "Refusing linked path or ancestor: $($entry.FullName)"
        }
        $entry = if ($entry -is [IO.DirectoryInfo]) { $entry.Parent } else { $entry.Directory }
    }
}

Assert-PlainAncestors $workspace
foreach ($required in @('package.json', 'Cargo.toml')) {
    Assert-PlainAncestors (Join-Path $workspace $required)
}
$package = Get-Content -LiteralPath (Join-Path $workspace 'package.json') -Raw -Encoding UTF8 | ConvertFrom-Json
if ($package.name -ne 'dustaway') { throw 'This is not the DustAway workspace.' }

# Fast, read-only enumeration. Never traverse a reparse point, including pnpm links.
if (-not ('DustAwayProjectCleanupWalker' -as [type])) {
    Add-Type -TypeDefinition @'
using System;
using System.IO;
using System.Collections.Generic;
using System.ComponentModel;
using System.Runtime.InteropServices;
using System.Text.RegularExpressions;
public class DustAwayCleanupTree {
    public long Bytes;
    public long Files;
    public List<string> DirectoryLinks = new List<string>();
    public List<string> FileLinks = new List<string>();
}
public static class DustAwayProjectCleanupWalker {
    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    private struct FindData {
        public FileAttributes Attributes;
        public System.Runtime.InteropServices.ComTypes.FILETIME CreationTime;
        public System.Runtime.InteropServices.ComTypes.FILETIME AccessTime;
        public System.Runtime.InteropServices.ComTypes.FILETIME WriteTime;
        public uint SizeHigh;
        public uint SizeLow;
        public uint Reserved0;
        public uint Reserved1;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 260)] public string Name;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 14)] public string AlternateName;
    }
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern IntPtr FindFirstFileW(string path, out FindData data);
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool FindNextFileW(IntPtr handle, out FindData data);
    [DllImport("kernel32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool FindClose(IntPtr handle);
    private static string NativePath(string path) {
        if (path.StartsWith(@"\\?\")) return path;
        if (path.StartsWith(@"\\")) return @"\\?\UNC\" + path.Substring(2);
        return @"\\?\" + path;
    }
    public static DustAwayCleanupTree Scan(string path, bool allowLinks) {
        var result = new DustAwayCleanupTree();
        var pending = new Stack<string>();
        pending.Push(Path.GetFullPath(path));
        while (pending.Count > 0) {
            var directory = pending.Pop();
            if ((File.GetAttributes(NativePath(directory)) & FileAttributes.ReparsePoint) != 0)
                throw new IOException("Refusing nested link: " + directory);
            FindData entry;
            var handle = FindFirstFileW(NativePath(directory + @"\*"), out entry);
            if (handle == new IntPtr(-1)) {
                var error = Marshal.GetLastWin32Error();
                if (error == 2) continue;
                throw new IOException(directory + ": " + new Win32Exception(error).Message);
            }
            try {
                do {
                    if (entry.Name == "." || entry.Name == "..") continue;
                    var fullName = Path.Combine(directory, entry.Name);
                    bool isDirectory = (entry.Attributes & FileAttributes.Directory) != 0;
                    if ((entry.Attributes & FileAttributes.ReparsePoint) != 0) {
                        if (!allowLinks) throw new IOException("Refusing nested link: " + fullName);
                        if (isDirectory) result.DirectoryLinks.Add(fullName);
                        else result.FileLinks.Add(fullName);
                    } else if (isDirectory) pending.Push(fullName);
                    else { result.Bytes += ((long)entry.SizeHigh << 32) | entry.SizeLow; result.Files++; }
                } while (FindNextFileW(handle, out entry));
                var error = Marshal.GetLastWin32Error();
                if (error != 18) throw new IOException(directory + ": " + new Win32Exception(error).Message);
            } finally { FindClose(handle); }
            }
        return result;
    }
    public static void DeleteTree(string path) {
        // Use filesystem calls rather than allocating one PowerShell object per
        // file. Marked test fixtures can contain hundreds of thousands of files.
        var pending = new Stack<KeyValuePair<string, bool>>();
        pending.Push(new KeyValuePair<string, bool>(Path.GetFullPath(path), false));
        while (pending.Count > 0) {
            var selected = pending.Pop();
            var directory = NativePath(selected.Key);
            if ((File.GetAttributes(directory) & FileAttributes.ReparsePoint) != 0)
                throw new IOException("Refusing nested link: " + selected.Key);
            if (selected.Value) { Directory.Delete(directory, false); continue; }
            var children = Directory.GetDirectories(directory);
            foreach (var file in Directory.GetFiles(directory)) {
                try { File.Delete(file); }
                catch (UnauthorizedAccessException) {
                    var attributes = File.GetAttributes(file);
                    if ((attributes & FileAttributes.ReparsePoint) != 0 ||
                        (attributes & FileAttributes.ReadOnly) == 0) throw;
                    File.SetAttributes(file, attributes & ~FileAttributes.ReadOnly);
                    File.Delete(file);
                }
            }
            pending.Push(new KeyValuePair<string, bool>(selected.Key, true));
            foreach (var child in children)
                pending.Push(new KeyValuePair<string, bool>(child, false));
        }
    }
    public static List<string> HistoricalPayloads(string root, string[] excluded) {
        var result = new List<string>();
        var suffixes = new HashSet<string>(new string[] {
            ".exe", ".dll", ".pdb", ".rlib", ".rmeta", ".lib", ".obj", ".o", ".a",
            ".pak", ".asar", ".node", ".msi", ".msix", ".msixbundle", ".appx", ".appxbundle", ".cab", ".7z"
        }, StringComparer.OrdinalIgnoreCase);
        var pending = new Stack<DirectoryInfo>();
        pending.Push(new DirectoryInfo(root));
        while (pending.Count > 0) {
            var directory = pending.Pop();
            if ((directory.Attributes & FileAttributes.ReparsePoint) != 0)
                throw new IOException("Historical directory changed to a link: " + directory.FullName);
            foreach (var entry in directory.EnumerateFileSystemInfos()) {
                bool skip = false;
                foreach (var preserved in excluded) {
                    if (String.Equals(entry.FullName, preserved, StringComparison.OrdinalIgnoreCase) ||
                        entry.FullName.StartsWith(preserved + @"\", StringComparison.OrdinalIgnoreCase)) {
                        skip = true; break;
                    }
                }
                if (skip || (entry.Attributes & FileAttributes.ReparsePoint) != 0) continue;
                var child = entry as DirectoryInfo;
                if (child != null) {
                    if (!String.Equals(child.Name, ".git", StringComparison.OrdinalIgnoreCase)) pending.Push(child);
                    continue;
                }
                if (suffixes.Contains(entry.Extension) ||
                    (String.Equals(entry.Extension, ".zip", StringComparison.OrdinalIgnoreCase) &&
                     !Regex.IsMatch(entry.Name, "before|baseline|source|snapshot|backup|ssh|nsis", RegexOptions.IgnoreCase)))
                    result.Add(entry.FullName);
            }
        }
        return result;
    }
    public static bool ContainsGitMetadata(string root) {
        var pending = new Stack<DirectoryInfo>();
        pending.Push(new DirectoryInfo(root));
        while (pending.Count > 0) {
            var directory = pending.Pop();
            if (String.Equals(directory.Name, ".git", StringComparison.OrdinalIgnoreCase)) return true;
            if (File.Exists(Path.Combine(directory.FullName, ".git"))) return true;
            foreach (var child in directory.EnumerateDirectories()) {
                if ((child.Attributes & FileAttributes.ReparsePoint) != 0) continue;
                if (String.Equals(child.Name, ".git", StringComparison.OrdinalIgnoreCase)) return true;
                pending.Push(child);
            }
        }
        return false;
    }
}
'@
}

function Get-RustRoot {
    $logicalRoot = Join-Path $workspace 'target'
    $entry = Get-Item -LiteralPath $logicalRoot -Force -ErrorAction SilentlyContinue
    if ($null -eq $entry) { return $null }
    if (-not $entry.PSIsContainer) { throw 'target is not a directory.' }
    if ($entry.Attributes -band [IO.FileAttributes]::ReparsePoint) {
        # Never clean a linked cache: this script releases space in the workspace.
        return $null
    }
    Assert-PlainAncestors $logicalRoot
    return $logicalRoot
}

$candidates = New-Object 'System.Collections.Generic.List[object]'
function Add-Candidate([string]$RelativePath, [string]$Category, [bool]$AllowLinks = $false) {
    $path = Join-Path $workspace $RelativePath
    if (Test-Path -LiteralPath $path) {
        $candidates.Add([pscustomobject]@{ Path = $path; Boundary = $workspace;
            Relative = $RelativePath; Category = $Category; AllowLinks = $AllowLinks; Rust = $false })
    }
}

Add-Candidate 'dist' 'Frontend output'
Add-Candidate 'tsconfig.tsbuildinfo' 'TypeScript cache'
Add-Candidate 'src-tauri\gen' 'Generated Tauri schemas'
Add-Candidate 'src-tauri\binaries\dustaway-helper-x86_64-pc-windows-msvc.exe' 'Generated helper'
Add-Candidate 'scripts\analysis-benchmark\target' 'Benchmark build output'
Add-Candidate 'artifacts\progress-visible-20261002\build-target' 'Historical Rust build cache'
if ($IncludeBrowserTestAssets) {
    foreach ($name in @('brave', 'vivaldi', 'vivaldi-packed', 'downloads')) {
        Add-Candidate ('artifacts\browser-validation-0.6\' + $name) 'Optional downloaded test browsers'
    }
} elseif ($IncludeBrowserArchives) {
    foreach ($name in @('vivaldi-packed', 'downloads')) {
        Add-Candidate ('artifacts\browser-validation-0.6\' + $name) 'Redundant test browser downloads'
    }
}
if ($IncludeDependencies) {
    Add-Candidate 'node_modules' 'Reinstallable dependencies' $true
} else {
    Add-Candidate 'node_modules\.vite' 'Vite / Vitest cache'
    Add-Candidate 'node_modules\.vite-temp' 'Vite temporary config'
}

$artifactRoot = Join-Path $workspace 'artifacts'
if (Test-Path -LiteralPath $artifactRoot) {
    Assert-PlainAncestors $artifactRoot
    foreach ($entry in (Get-ChildItem -LiteralPath $artifactRoot -Force)) {
        if ($entry.Attributes -band [IO.FileAttributes]::ReparsePoint) {
            Write-Warning "Preserving linked artifact: $($entry.FullName)"
            continue
        }
        if ($entry.PSIsContainer -and $entry.Name -match '^dustaway-e2e-(?:(?:attachments|software)-)?[A-Za-z0-9]{6}$') {
            $marker = Join-Path $entry.FullName '.dustaway-test-only'
            if (Test-Path -LiteralPath $marker -PathType Leaf) {
                Assert-PlainAncestors $marker
                if ([IO.File]::ReadAllText($marker).Trim() -eq 'fixture-only') {
                    if (-not $SkipE2EFixtures) {
                        Add-Candidate ('artifacts\' + $entry.Name) 'Marked E2E fixture'
                    }
                    continue
                }
            }
            Write-Warning "Preserving fixture without the expected marker: $($entry.FullName)"
        } elseif ($IncludeBenchmarkFixtures -and $entry.PSIsContainer -and $entry.Name -match '^dustaway-analysis-bench-[A-Za-z0-9][A-Za-z0-9_.-]*$') {
            $marker = Join-Path $entry.FullName '.dustaway-test-only'
            if (Test-Path -LiteralPath $marker -PathType Leaf) {
                Assert-PlainAncestors $marker
                if ([IO.File]::ReadAllText($marker).Trim() -eq 'readonly-analysis-benchmark') {
                    Add-Candidate ('artifacts\' + $entry.Name) 'Optional synthetic benchmark dataset'
                    continue
                }
            }
            Write-Warning "Preserving benchmark without the expected marker: $($entry.FullName)"
        } elseif ($entry.PSIsContainer -and $entry.Name -match '^(?:scoped_dir[0-9]+_[0-9]+|msedgeedge_BITS_[0-9]+_[0-9]+)$') {
            Add-Candidate ('artifacts\' + $entry.Name) 'WebView / Edge test temporary files'
        } elseif ($IncludeLogs -and -not $entry.PSIsContainer -and $entry.Extension -eq '.log') {
            Add-Candidate ('artifacts\' + $entry.Name) 'Optional root build/test log'
        }
    }
}

$historyProtected = @()
if ($IncludeBenchmarkFixtures -and (Test-Path -LiteralPath $artifactRoot)) {
    # These older marked scan fixtures are produced by scan-performance.mjs and
    # the inspected Compare-ScanPerformance.mjs scripts. Keep parent reports.
    $performanceRoots = @(Get-ChildItem -LiteralPath $artifactRoot -Directory -Force |
        Where-Object { $_.Name -match '^dustaway-perf-[A-Za-z0-9][A-Za-z0-9_.-]*$' })
    foreach ($parent in Get-ChildItem -LiteralPath $artifactRoot -Directory -Force) {
        if ($parent.Attributes -band [IO.FileAttributes]::ReparsePoint) { continue }
        foreach ($relative in @('dustaway-perf-home', 'scan-performance\dustaway-perf-home', 'scan-performance-isolated\dustaway-perf-home')) {
            $nested = Join-Path $parent.FullName $relative
            if (Test-Path -LiteralPath $nested -PathType Container) {
                $performanceRoots += Get-Item -LiteralPath $nested -Force
            }
        }
    }
    foreach ($entry in $performanceRoots) {
        if ($entry.Attributes -band [IO.FileAttributes]::ReparsePoint) { continue }
        $marker = Join-Path $entry.FullName '.dustaway-test-only'
        if (Test-Path -LiteralPath $marker -PathType Leaf) {
            Assert-PlainAncestors $marker
            if ([IO.File]::ReadAllText($marker).Trim() -eq 'performance-fixture-only') {
                Add-Candidate ($entry.FullName.Substring($workspace.Length + 1)) 'Optional synthetic scan-performance dataset'
                continue
            }
        }
        Write-Warning "Preserving performance dataset without the expected marker: $($entry.FullName)"
    }
}
if ($CurrentVersionOnly -and (Test-Path -LiteralPath $artifactRoot)) {
    # Explicit historical environments inspected for this project. Reports in
    # their parent directories and project source backups remain available.
    foreach ($relative in @(
        'browser-validation-0.6\brave', 'browser-validation-0.6\vivaldi',
        'home-cleanup-real-20261001\downloads',
        'readonly-unlock-20261002\isolated-electron-runtime',
        'readonly-unlock-20261002\research\cherry-official',
        'qq-acceptance\nt-layout',
        'software-remote-acceptance\brave-1.96.60-unpack',
        'software-remote-acceptance\desktop-third-round',
        'software-remote-acceptance\firefox',
        'software-remote-acceptance\firefox-desktop-delete-20261001',
        'software-remote-acceptance\firefox-desktop-delete-20261001-v2',
        'software-remote-acceptance\firefox-desktop-real-bundle',
        'software-remote-acceptance\firefox-v2',
        'software-remote-acceptance\firefox-v2-version-corrected',
        'software-remote-acceptance\geckodriver-0.37.1',
        'software-remote-acceptance\msi', 'software-remote-acceptance\msi-attempt2',
        'software-remote-acceptance\msi-attempt3', 'software-remote-acceptance\msi-completion',
        'software-remote-acceptance\msi-final', 'software-remote-acceptance\msi-restart',
        'software-remote-acceptance\msix',
        'software-verification\capabilities-native-20261001',
        'software-verification\residual-v2-native-20261001',
        'software-verification\simple-uninstall-native',
        'software-verification\simple-uninstall-native-final',
        'software-verification\simple-uninstall-native-v2',
        'software-verification\status-refresh-native-20261001',
        'software-verification\wps-native-20261001'
    )) {
        $historicalRoot = Join-Path $artifactRoot $relative
        if (Test-Path -LiteralPath $historicalRoot -PathType Container) {
            Assert-PlainAncestors $historicalRoot
            if (-not [DustAwayProjectCleanupWalker]::ContainsGitMetadata($historicalRoot)) {
                Add-Candidate ('artifacts\' + $relative) 'Historical test environment'
            } else {
                Write-Verbose ('Preserving historical source/Git metadata; selecting only payload files: ' + $historicalRoot)
            }
        }
    }

    $portableRoot = Join-Path $artifactRoot 'portable'
    if (Test-Path -LiteralPath $portableRoot) {
        Assert-PlainAncestors $portableRoot
        foreach ($release in (Get-ChildItem -LiteralPath $portableRoot -Directory -Force)) {
            if ($release.Name -match '^.+_(?<version>[0-9]+\.[0-9]+\.[0-9]+)(?:_[A-Za-z0-9_-]+)?_windows-x64$' -and
                [version]$Matches.version -lt [version]$package.version) {
                Assert-PlainAncestors $release.FullName
                if (-not [DustAwayProjectCleanupWalker]::ContainsGitMetadata($release.FullName)) {
                    Add-Candidate ('artifacts\portable\' + $release.Name) 'Historical release'
                }
            }
        }
    }

    # Keep current desktop-test tooling and remote access assets. Preserve all
    # portable packages here; older versions were selected explicitly above.
    foreach ($name in @('portable', 'webdriver', 'openssh-offline', 'ssh-authorize',
        'ssh-connection-info', 'ssh-network-diagnostic', 'ssh-remote')) {
        $historyProtected += Join-Path $artifactRoot $name
    }
    $historyExcluded = @($historyProtected) + @($candidates | ForEach-Object Path)
    foreach ($entry in (Get-ChildItem -LiteralPath $artifactRoot -Directory -Force)) {
        if ($entry.Name -match '^dustaway-(?:e2e-|analysis-bench-)') {
            $historyExcluded += $entry.FullName
        }
    }
    foreach ($payload in [DustAwayProjectCleanupWalker]::HistoricalPayloads($artifactRoot, [string[]]$historyExcluded)) {
        Add-Candidate ($payload.Substring($workspace.Length + 1)) 'Historical binary or test package'
    }
}

# Options can select the same cache or historical release more than once.
$uniqueCandidates = New-Object 'System.Collections.Generic.List[object]'
$seenPaths = New-Object 'System.Collections.Generic.HashSet[string]' ([StringComparer]::OrdinalIgnoreCase)
$coveringDirectories = New-Object 'System.Collections.Generic.List[string]'
foreach ($candidate in ($candidates | Sort-Object @{e={$_.Path.Length}})) {
    if (-not $seenPaths.Add($candidate.Path)) {continue}
    $covered = $false
    foreach ($existing in $coveringDirectories) {
        if ($candidate.Path.StartsWith($existing + '\', [StringComparison]::OrdinalIgnoreCase)) {
            $covered = $true; break
        }
    }
    if (-not $covered) {
        $uniqueCandidates.Add($candidate)
        if (Test-Path -LiteralPath $candidate.Path -PathType Container) {$coveringDirectories.Add($candidate.Path)}
    }
}
$candidates = $uniqueCandidates

$rustRoot = if ($RustScope -ne 'None') { Get-RustRoot } else { $null }
if ($RustScope -ne 'None' -and $null -eq $rustRoot) {
    Write-Output 'Rust target is absent or linked elsewhere; excluded from workspace-drive cleanup.'
}
if ($null -ne $rustRoot) {
    $rustNames = @('debug', 'bench-million', 'tmp', '.rustc_info.json')
    if ($RustScope -eq 'All') { $rustNames += 'release' }
    foreach ($name in $rustNames) {
        $path = Join-Path $rustRoot $name
        if (Test-Path -LiteralPath $path) {
            $candidates.Add([pscustomobject]@{ Path = $path; Boundary = $rustRoot;
                Relative = ('target\' + $name); Category = 'Rust build output'; AllowLinks = $false; Rust = $true })
        }
    }
    Write-Output "Rust cache physical location: $rustRoot (root and junction are preserved)"
}

# Also protect generated paths if they become tracked source files in the future.
$trackedPaths = @()
if ((Test-Path -LiteralPath (Join-Path $workspace '.git')) -and (Get-Command git -ErrorAction SilentlyContinue)) {
    $trackedPaths = @(& git -C $workspace ls-files)
    if ($LASTEXITCODE -ne 0) { throw 'Cannot check tracked files; no deletion performed.' }
}

function Assert-Candidate($Candidate) {
    $fullPath = [IO.Path]::GetFullPath($Candidate.Path).TrimEnd('\')
    $boundary = [IO.Path]::GetFullPath($Candidate.Boundary).TrimEnd('\')
    if (-not $fullPath.StartsWith($boundary + '\', [StringComparison]::OrdinalIgnoreCase)) {
        throw "Selected path escapes its allowed directory: $fullPath"
    }
    Assert-PlainAncestors $fullPath
    if ((Resolve-Path -LiteralPath $fullPath).ProviderPath.TrimEnd('\') -ne $fullPath) {
        throw "Unexpected resolved path: $fullPath"
    }
    if ($Candidate.Rust -and (Get-RustRoot) -ne $Candidate.Boundary) {
        throw 'Rust target mapping changed. No further deletion performed.'
    }
    $relative = $Candidate.Relative.Replace('\', '/')
    foreach ($tracked in $trackedPaths) {
        if ($tracked -eq $relative -or $tracked.StartsWith($relative + '/', [StringComparison]::OrdinalIgnoreCase)) {
            throw "Preserving tracked file: $tracked"
        }
    }
}

# Root validation is always performed. Full tree checks precede actual deletion;
# opt into full traversal during previews, since test fixtures contain many files.
$scanTrees = $MeasureSize -or ($Mode -eq 'Clean' -and -not $WhatIfPreference)
$plan = @(
    foreach ($candidate in $candidates) {
        Write-Verbose ('Checking: ' + $candidate.Path)
        Assert-Candidate $candidate
        $entry = Get-Item -LiteralPath $candidate.Path -Force
        if ($entry.PSIsContainer -and $scanTrees) {
            $tree = [DustAwayProjectCleanupWalker]::Scan($candidate.Path, $candidate.AllowLinks)
            $bytes = $tree.Bytes; $files = $tree.Files
        } elseif ($entry.PSIsContainer) { $bytes = $null; $files = $null }
        else { $bytes = $entry.Length; $files = 1 }
        [pscustomobject]@{ Candidate = $candidate; Bytes = $bytes; Files = $files }
    }
)
$plan | Sort-Object Bytes -Descending | Select-Object @{n='MiB';e={if ($null -eq $_.Bytes) {'-'} else {[math]::Round($_.Bytes / 1MB, 2)}}},
    Files, @{n='Category';e={$_.Candidate.Category}}, @{n='Path';e={$_.Candidate.Path}} | Format-Table -AutoSize
[long]$totalBytes = 0
[long]$totalFiles = 0
foreach ($selection in $plan) { $totalBytes += $selection.Bytes; $totalFiles += $selection.Files }
if ($scanTrees) {
    Write-Output ('Selected: {0} paths; {1} regular files; {2:N3} GiB (logical file sizes).' -f $plan.Count, $totalFiles, ($totalBytes / 1GB))
} else {
    Write-Output ('Selected: {0} paths. Directory sizes are not measured in this quick preview; use -MeasureSize for full traversal.' -f $plan.Count)
}
if ($Mode -eq 'Report') {
    Write-Output 'Report only. Nothing deleted. Use -Mode Clean to execute; -WhatIf is also supported.'
    return
}

if (-not $WhatIfPreference) {
    # Do not kill processes. Stop before touching files if a build/test/app is active.
    $active = @(Get-CimInstance Win32_Process | Where-Object {
        $_.Name -in @('cargo.exe', 'rustc.exe', 'clippy-driver.exe', 'dustaway.exe', 'dustaway-analysis-bench.exe', 'r3-benchmark.exe', 'tauri-driver.exe', 'msedgedriver.exe') -or
        ($_.Name -in @('node.exe', 'msedge.exe') -and
            $_.CommandLine -and $_.CommandLine.IndexOf($workspace, [StringComparison]::OrdinalIgnoreCase) -ge 0) -or
        ($_.Name -eq 'node.exe' -and $_.CommandLine -match '(?:[/\\](?:vite|vitest|@tauri-apps)[/\\ .]|scripts[/\\][^"\s]*(?:e2e|check|acceptance|release|smoke))') -or
        ($_.ExecutablePath -and $_.ExecutablePath.StartsWith($workspace + '\', [StringComparison]::OrdinalIgnoreCase)) -or
        ($_.ExecutablePath -and $rustRoot -and $_.ExecutablePath.StartsWith($rustRoot + '\', [StringComparison]::OrdinalIgnoreCase))
    })
    if ($active.Count -gt 0) {
        throw ('Close project builds, tests and applications first: ' + (($active | ForEach-Object { "$($_.Name) PID=$($_.ProcessId)" }) -join ', '))
    }
}

foreach ($selection in $plan) {
    $candidate = $selection.Candidate
    if (-not $PSCmdlet.ShouldProcess($candidate.Path, 'Delete inspected project-generated files')) { continue }
    if (-not (Test-Path -LiteralPath $candidate.Path)) { continue }
    Assert-Candidate $candidate
    $entry = Get-Item -LiteralPath $candidate.Path -Force
    if ($entry.PSIsContainer) {
        if ($candidate.AllowLinks) {
            $tree = [DustAwayProjectCleanupWalker]::Scan($candidate.Path, $true)
            # pnpm links are unlinked themselves, never recursively followed.
            foreach ($link in $tree.DirectoryLinks) { [IO.Directory]::Delete($link, $false) }
            foreach ($link in $tree.FileLinks) { [IO.File]::Delete($link) }
            Assert-Candidate $candidate
        }
        # Ordinary trees were fully checked during preflight. DeleteTree checks
        # each directory again before traversing it, without re-reading every file.
        [DustAwayProjectCleanupWalker]::DeleteTree($candidate.Path)
    } else { Remove-Item -LiteralPath $candidate.Path -Force }
    Write-Output "Deleted: $($candidate.Path)"
}
if ($CurrentVersionOnly) {
    Write-Output 'Finished. Current releases, source, dependencies, project backups and retained reports are preserved.'
} else {
    Write-Output 'Finished. Source, lockfiles, release packages, backups and acceptance evidence are preserved.'
}
