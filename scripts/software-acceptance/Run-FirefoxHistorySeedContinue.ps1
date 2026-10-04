$ErrorActionPreference='Stop'
$taskRoot=[IO.Path]::GetFullPath($PSScriptRoot)
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6' -or $taskRoot -ne (Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20260930-software') -or [IO.File]::ReadAllText((Join-Path $taskRoot '.dustaway-isolated-acceptance')) -ne 'DustAway.Acceptance.20260930.7c7e25e8'){throw 'Wrong environment'}
try {
$taskPrincipal=New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if($taskPrincipal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){throw 'Ordinary user required'}
$taskOriginal=Get-CimInstance Win32_Process -Filter 'ProcessId=11304'
if(-not $taskOriginal -or $taskOriginal.Name -ne 'firefox.exe' -or $taskOriginal.ExecutablePath -ne (Join-Path $taskRoot 'firefox-real\application\firefox.exe') -or $taskOriginal.SessionId -ne [Diagnostics.Process]::GetCurrentProcess().SessionId){throw 'Original test browser handle is absent or different'}
if((Get-Process -Name geckodriver -ErrorAction SilentlyContinue) -or (Get-NetTCPConnection -State Listen -LocalPort 4446,19838 -ErrorAction SilentlyContinue)){throw 'Prior driver/server still running'}
$taskListener=Get-NetTCPConnection -State Listen -LocalPort 2829
if($taskListener.LocalAddress -ne '127.0.0.1' -or $taskListener.OwningProcess -ne 10252){throw 'Expected original Marionette listener'}
$taskBrowser=Get-CimInstance Win32_Process -Filter 'ProcessId=10252'
if($taskBrowser.ParentProcessId -ne 11304 -or $taskBrowser.ExecutablePath -ne $taskOriginal.ExecutablePath -or $taskBrowser.SessionId -ne $taskOriginal.SessionId){throw 'Original Firefox launcher/browser relationship changed'}
if((Get-FileHash -LiteralPath (Join-Path $taskRoot 'firefox-history-seed.mjs')).Hash -ne '781D9F06BE99985C2218490C89F447481CAF96E785F063FC2D47EDD521CCB934'){throw 'Unexpected continuation source'}
$taskRun=Start-Process -FilePath 'D:\nodejs\node.exe' -ArgumentList @('firefox-history-seed.mjs','firefox-history-evidence','resume-existing-browser') -WorkingDirectory $taskRoot -WindowStyle Hidden -Wait -PassThru -RedirectStandardOutput (Join-Path $taskRoot 'firefox-history-seed-continue-run.log') -RedirectStandardError (Join-Path $taskRoot 'firefox-history-seed-continue-error.log')
[pscustomobject]@{ExitCode=$taskRun.ExitCode;Finished=(Get-Date).ToString('o');OriginalBrowserReconnected=$true;OrdinaryUser=$true} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskRoot 'firefox-history-seed-continue-result.json') -Encoding UTF8
exit $taskRun.ExitCode

}catch{$_ | Out-String | Add-Content -LiteralPath (Join-Path $taskRoot 'firefox-history-seed-preflight-error.log') -Encoding UTF8; exit 99}
