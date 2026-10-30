param(
  [ValidatePattern('^[\p{L}\p{N}_-]*$')]
  [string]$Variant = ''
)
$ErrorActionPreference = 'Stop'
function Get-Sha256([string]$path) {
  $stream = [IO.File]::OpenRead($path)
  $algorithm = [Security.Cryptography.SHA256]::Create()
  try { return [BitConverter]::ToString($algorithm.ComputeHash($stream)).Replace('-', '') }
  finally { $algorithm.Dispose(); $stream.Dispose() }
}
$projectRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$version = (Get-Content -LiteralPath (Join-Path $projectRoot 'package.json') -Encoding UTF8 -Raw | ConvertFrom-Json).version
$output = Join-Path $projectRoot 'artifacts/portable'
$name = if ($Variant) { "拂尘_${version}_${Variant}_windows-x64" } else { "拂尘_${version}_windows-x64" }
$folder = Join-Path $output $name
New-Item -ItemType Directory -Path $folder -Force | Out-Null
$sources = @{
    'LICENSE' = 'LICENSE'
    'licenses/THIRD-PARTY-NOTICES.txt' = 'THIRD-PARTY-NOTICES.txt'
    'licenses/inventory.json' = 'inventory.json'
    'docs/portable-readme.txt' = 'portable-readme.txt'
    'docs/user-guide.md' = 'user-guide.md'
    'docs/cleanup-compatibility.md' = 'cleanup-compatibility.md'
    'docs/current-capabilities.json' = 'current-capabilities.json'
    'docs/acceptance-ledger.json' = 'acceptance-ledger.json'
    'docs/acceptance-025.md' = 'acceptance-025.md'
    'docs/verification-025.md' = 'verification-025.md'
}
$expected = @('拂尘.exe', 'SHA256SUMS.txt') + @($sources.Values)
if (@(Get-ChildItem -LiteralPath $folder | Where-Object { $_.PSIsContainer -or $_.Name -notin $expected }).Count -gt 0) {
  throw "Output folder contains unexpected files; preserve them and choose a clean output folder: $folder"
}
Copy-Item -LiteralPath (Join-Path $projectRoot 'target/release/dustaway.exe') -Destination (Join-Path $folder '拂尘.exe')
foreach ($source in $sources.Keys) {
  Copy-Item -LiteralPath (Join-Path $projectRoot $source) -Destination (Join-Path $folder $sources[$source])
}
$checksums = foreach ($file in $expected | Where-Object { $_ -ne 'SHA256SUMS.txt' }) {
  $hash = Get-Sha256 (Join-Path $folder $file)
  "$hash  $file"
}
Set-Content -LiteralPath (Join-Path $folder 'SHA256SUMS.txt') -Value $checksums -Encoding UTF8
Write-Output "Portable folder: $folder"
