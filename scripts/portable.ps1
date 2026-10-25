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
$expected = @('拂尘.exe', 'LICENSE', 'THIRD-PARTY-NOTICES.txt', 'inventory.json', 'portable-readme.txt', 'application-coverage-evidence.md', 'readonly-cache-evidence.md', 'verification-attachments.md', 'verification-qq-attachments.md', 'cleanup-compatibility.md', 'verification-result-clarity.md', 'SHA256SUMS.txt')
$expected += 'system-cleanup-acceptance.md'
$expected += 'verification-weixin-special.md'
$expected += 'verification-weixin-021.md'
$expected += 'verification-weixin-0211.md'
$expected += 'verification-coverage-022.md'
$expected += 'verification-coverage-023.md'
$expected += 'verification-clarity-0231.md'
$expected += 'verification-coverage-024.md'
$expected += 'current-capabilities.json'
if (@(Get-ChildItem -LiteralPath $folder | Where-Object { $_.PSIsContainer -or $_.Name -notin $expected }).Count -gt 0) {
  throw "Output folder contains unexpected files; preserve them and choose a clean output folder: $folder"
}
Copy-Item -LiteralPath (Join-Path $projectRoot 'target/release/dustaway.exe') -Destination (Join-Path $folder '拂尘.exe')
Copy-Item -LiteralPath (Join-Path $projectRoot 'docs/system-cleanup-acceptance.md') -Destination $folder
Copy-Item -LiteralPath (Join-Path $projectRoot 'docs/verification-weixin-special.md') -Destination $folder
Copy-Item -LiteralPath (Join-Path $projectRoot 'docs/verification-weixin-021.md') -Destination $folder
Copy-Item -LiteralPath (Join-Path $projectRoot 'docs/verification-weixin-0211.md') -Destination $folder
Copy-Item -LiteralPath (Join-Path $projectRoot 'docs/verification-coverage-022.md') -Destination $folder
Copy-Item -LiteralPath (Join-Path $projectRoot 'docs/verification-coverage-023.md') -Destination $folder
Copy-Item -LiteralPath (Join-Path $projectRoot 'docs/verification-clarity-0231.md') -Destination $folder
Copy-Item -LiteralPath (Join-Path $projectRoot 'docs/verification-coverage-024.md') -Destination $folder
Copy-Item -LiteralPath (Join-Path $projectRoot 'docs/current-capabilities.json') -Destination $folder
foreach ($source in @('LICENSE', 'licenses/THIRD-PARTY-NOTICES.txt', 'licenses/inventory.json', 'docs/portable-readme.txt', 'docs/application-coverage-evidence.md', 'docs/readonly-cache-evidence.md', 'docs/verification-attachments.md', 'docs/verification-qq-attachments.md', 'docs/cleanup-compatibility.md', 'docs/verification-result-clarity.md')) {
  Copy-Item -LiteralPath (Join-Path $projectRoot $source) -Destination $folder
}
$checksums = foreach ($file in $expected | Where-Object { $_ -ne 'SHA256SUMS.txt' }) {
  $hash = Get-Sha256 (Join-Path $folder $file)
  "$hash  $file"
}
Set-Content -LiteralPath (Join-Path $folder 'SHA256SUMS.txt') -Value $checksums -Encoding UTF8
Write-Output "Portable folder: $folder"
