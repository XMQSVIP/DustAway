$ErrorActionPreference='Stop'
$project=(Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$evidence=Join-Path $project 'artifacts/home-cleanup-real-20261001'
$destination=Join-Path $evidence 'performance-baseline-source'
if(Test-Path -LiteralPath $destination){throw 'Baseline snapshot already exists'}
New-Item -ItemType Directory -Path (Join-Path $destination 'crates'),(Join-Path $destination 'scripts') | Out-Null
foreach($name in @('core','windows','helper')){Copy-Item -LiteralPath (Join-Path $project ('crates/'+$name)) -Destination (Join-Path $destination 'crates') -Recurse}
Copy-Item -LiteralPath (Join-Path $project 'src-tauri') -Destination $destination -Recurse
Copy-Item -LiteralPath (Join-Path $project 'rules') -Destination $destination -Recurse
Copy-Item -LiteralPath (Join-Path $project 'Cargo.lock') -Destination $destination
Copy-Item -LiteralPath (Join-Path $project 'scripts/application-catalog-v3.json') -Destination (Join-Path $destination 'scripts')
$workspace=[IO.File]::ReadAllText((Join-Path $project 'Cargo.toml'))
[IO.File]::WriteAllText((Join-Path $destination 'Cargo.toml'),$workspace)
Add-Type -AssemblyName System.IO.Compression.FileSystem
$archive=[IO.Compression.ZipFile]::OpenRead((Join-Path $evidence 'before.zip'))
try {
 $entry=$archive.GetEntry('platform.rs')
 if(!$entry){throw 'Before snapshot missing'}
 [IO.Compression.ZipFileExtensions]::ExtractToFile($entry,(Join-Path $destination 'crates/windows/src/platform.rs'),$true)
} finally {$archive.Dispose()}
# The saved pre-change platform has no real acceptance field. Remove only that
# test-only initializer from shared fixture files so it can be compiled unchanged.
foreach($file in @('browser_acceptance.rs','application_acceptance_tests.rs','scan_tests.rs','scan_benchmark.rs')){
 $path=Join-Path $destination ('crates/windows/src/'+$file)
 $source=[IO.File]::ReadAllText($path)
 $source=[regex]::Replace($source,'(?m)^\s*real_app_acceptance: None,\r?\n','')
 [IO.File]::WriteAllText($path,$source)
}
$inputs=@(Get-ChildItem -LiteralPath $destination -Recurse -File | ForEach-Object {@{path=$_.FullName.Substring($destination.Length+1);sha256=(Get-FileHash -LiteralPath $_.FullName).Hash}})
@{scope='user_temp read-only scan; exact pre-change platform.rs, unchanged shared scanner/core; test-only initializer adaptation';files=$inputs;before_archive_sha256=(Get-FileHash -LiteralPath (Join-Path $evidence 'before.zip')).Hash} | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $evidence 'performance-baseline-source.json') -Encoding utf8
