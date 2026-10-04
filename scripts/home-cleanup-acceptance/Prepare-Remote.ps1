$ErrorActionPreference='Stop'
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6'){throw 'Wrong host'}
$base=Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance'
if([IO.File]::ReadAllText((Join-Path $base '20260930-software\.dustaway-isolated-acceptance')) -ne 'DustAway.Acceptance.20260930.7c7e25e8'){throw 'Existing acceptance identity missing'}
for($parent=Get-Item -LiteralPath $base;$null -ne $parent;$parent=$parent.Parent){if($parent.Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'Linked workspace refused'}}
$root=Join-Path $base '20261001-home-cleanup'
if(Test-Path -LiteralPath $root){throw 'Preparation already exists; inspect instead of repeating'}
New-Item -ItemType Directory -Path $root | Out-Null
[IO.File]::WriteAllText((Join-Path $root '.dustaway-test-only'),'DustAway.HomeCleanup.20261001')
[ordered]@{time=(Get-Date -Format o);host=$env:COMPUTERNAME;root=$root;created=$true} | ConvertTo-Json
