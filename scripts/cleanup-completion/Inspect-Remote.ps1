$ErrorActionPreference='Stop'
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6' -or $env:USERNAME -ne 'MSN'){throw 'Wrong host/user'}
$taskRoot=Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance'
$taskChrome=Join-Path $env:LOCALAPPDATA 'Google\Chrome\User Data\Default'
$taskCaches=@(foreach($taskLeaf in @('Cache\Cache_Data','Code Cache','GPUCache')){
 $taskPath=Join-Path $taskChrome $taskLeaf
 $taskFiles=@(if(Test-Path -LiteralPath $taskPath){Get-ChildItem -LiteralPath $taskPath -File -Recurse -ErrorAction Stop})
 [ordered]@{cache=$taskLeaf;files=$taskFiles.Count;bytes=($taskFiles | Measure-Object Length -Sum).Sum}
})
[ordered]@{
 host=$env:COMPUTERNAME; user=$env:USERNAME
 processes=@(Get-Process -Name chrome,dustaway,tauri-driver,msedgedriver -ErrorAction SilentlyContinue | Select-Object ProcessName,Id,SessionId)
 drivers=@(Get-ChildItem -LiteralPath $taskRoot -Filter msedgedriver.exe -File -Recurse | Select-Object FullName,@{n='Version';e={$_.VersionInfo.FileVersion}})
 node=(Test-Path -LiteralPath 'D:\nodejs\node.exe')
 chrome_caches=$taskCaches
 tasks=@(Get-ScheduledTask | Where-Object {$_.TaskName -like 'DustAway*' -and $_.State -eq 'Running'} | Select-Object TaskName,State)
} | ConvertTo-Json -Depth 5
