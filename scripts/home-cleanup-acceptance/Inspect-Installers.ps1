$ErrorActionPreference='Stop'
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6'){throw 'Wrong host'}
$root=Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20261001-home-cleanup'
$files=@(Get-ChildItem -LiteralPath $root -File | Select-Object Name,Length,LastWriteTime)
$tasks=@(Get-ScheduledTask | Where-Object {$_.TaskName -like 'DustAway-HomeCleanup-*'} | ForEach-Object {$info=$_ | Get-ScheduledTaskInfo;[pscustomobject]@{name=$_.TaskName;state=[string]$_.State;last_result=$info.LastTaskResult}})
$launches=@(Get-ChildItem -LiteralPath $root -Filter '*.install-launch.json' | ForEach-Object {Get-Content -LiteralPath $_.FullName -Raw | ConvertFrom-Json})
$processes=@(Get-Process | Where-Object {$_.ProcessName -match '^(DingTalk|DingtalkLauncher|BaiduNetdisk|Weixin|WeChat|WeixinSetup|WeChatWin.*)$'} | Select-Object ProcessName,Id,SessionId,MainWindowTitle)
[ordered]@{time=(Get-Date -Format o);files=$files;tasks=$tasks;launches=$launches;processes=$processes} | ConvertTo-Json -Depth 5
