$ErrorActionPreference='Stop'
$root='C:\Users\MSN\AppData\Local\DustAwayAcceptance\weixin-021'
$target='C:\Program Files\DustAwayAcceptance'
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6' -or [IO.File]::ReadAllText((Join-Path $root '.dustaway-test')) -cne 'DustAway.Weixin.0.21'){throw 'Wrong host/marker'}
if((Get-Item -LiteralPath $target).Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'Linked install directory'}
if([IO.File]::ReadAllText((Join-Path $target '.dustaway-acceptance')) -cne 'DustAway.Weixin.20261003'){throw 'Old installation changed'}
$exe=Join-Path $target 'dustaway.exe'
$helper=Join-Path $target 'dustaway-helper.exe'
if((Get-Item -LiteralPath $exe).VersionInfo.ProductVersion -ne '0.20.0'){throw 'Expected 0.20.0 upgrade'}
if((Get-FileHash -LiteralPath $exe).Hash -ne '959BFEFC0A7A2B83B3DEA92C94F8FED185DBAE12A334F9FAE5725363F34CD0B7'){throw 'Old app changed'}
if((Get-FileHash -LiteralPath $helper).Hash -ne '27C262562D5B061568A5FE3E634FAB074BE78F115371A59ABE693DEB38481A30'){throw 'Old helper changed'}
if(Get-Process -Name dustaway -ErrorAction SilentlyContinue){throw 'App is running'}
$installer=Join-Path $root 'DustAway_0.21.0_x64-setup.exe'
if((Get-FileHash -LiteralPath $installer).Hash -ne 'INSTALLER_HASH'){throw 'Installer mismatch'}
$intent=Join-Path $root 'upgrade.started'
if(Test-Path -LiteralPath $intent){throw 'Previous upgrade requires verification; no replay'}
[IO.File]::WriteAllText($intent,'0.20.0 -> 0.21.0')
$job=Start-Process -FilePath $installer -ArgumentList ('/S /D='+$target) -WindowStyle Hidden -Wait -PassThru
if($job.ExitCode -ne 0){throw "Installer exit $($job.ExitCode)"}
$version=(Get-Item -LiteralPath $exe).VersionInfo.ProductVersion
if($version -ne '0.21.0'){throw 'Wrong installed version'}
if((Get-FileHash -LiteralPath $helper).Hash -ne 'HELPER_HASH'){throw 'Helper mismatch'}
if((Get-FileHash -LiteralPath $exe).Hash -ne 'APP_HASH'){throw 'App mismatch'}
[IO.File]::WriteAllText((Join-Path $target '.dustaway-acceptance'),'DustAway.Weixin.0.21')
$evidence=[PSCustomObject]@{exitCode=$job.ExitCode;from='0.20.0';version=$version;appSha256=(Get-FileHash -LiteralPath $exe).Hash;helperSha256=(Get-FileHash -LiteralPath $helper).Hash;interactiveUiVerified=$false;systemSettingsChanged=$false}|ConvertTo-Json
[IO.File]::WriteAllText((Join-Path $root 'installed-evidence.json'),$evidence)
$evidence
