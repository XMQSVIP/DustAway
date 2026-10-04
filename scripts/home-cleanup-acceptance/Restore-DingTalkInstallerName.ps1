$ErrorActionPreference='Stop'
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6'){throw 'Wrong host'}
$root=[IO.Path]::GetFullPath((Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20261001-home-cleanup'))
if([IO.File]::ReadAllText((Join-Path $root '.dustaway-test-only')) -ne 'DustAway.HomeCleanup.20261001'){throw 'Wrong environment'}
$source=Join-Path $root 'DingTalk.exe'
$name='DingTalk_v8.2.0.260119001.exe'
$destination=Join-Path $root $name
if([IO.Path]::GetDirectoryName($source) -ne $root -or [IO.Path]::GetDirectoryName($destination) -ne $root){throw 'Unexpected path'}
if(@(Get-CimInstance Win32_Process | Where-Object {$_.Name -match 'DingTalk|DingDing'}).Count){throw 'Close installer before renaming'}
if(Test-Path -LiteralPath $destination){throw 'Original-name copy already exists'}
$hash=(Get-FileHash -LiteralPath $source).Hash
if($hash -ne 'C4664B74F7B688FCBE30B951632EB6B0ABB820703908835695B5F82F8431CF56'){throw 'Unexpected installer'}
Rename-Item -LiteralPath $source -NewName $name
[ordered]@{time=(Get-Date -Format o);old_name='DingTalk.exe';new_name=$name;sha256=$hash;path=$destination;stage='official_filename_restored';installation_started=$false} | ConvertTo-Json | Tee-Object -FilePath (Join-Path $root 'DingTalk.rename-receipt.json')
