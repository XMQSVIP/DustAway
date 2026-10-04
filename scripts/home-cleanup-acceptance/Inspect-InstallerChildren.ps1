$ErrorActionPreference='Stop'
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6'){throw 'Wrong host'}
$root=Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20261001-home-cleanup'
$first=Get-Content -LiteralPath (Join-Path $root 'DingTalk.install-launch.json') -Raw | ConvertFrom-Json
$started=[DateTimeOffset]::Parse($first.time).LocalDateTime.AddMinutes(-1)
$all=@(Get-CimInstance Win32_Process)
$found=@(foreach($p in $all){
 if($p.SessionId -ne 2 -or $p.CreationDate -lt $started){continue}
 $metadata=$null
 if($p.ExecutablePath){try{$metadata=(Get-Item -LiteralPath $p.ExecutablePath).VersionInfo}catch{}}
 [ordered]@{name=$p.Name;pid=$p.ProcessId;parent=$p.ParentProcessId;created=$p.CreationDate.ToString('o');path=$p.ExecutablePath;product=$metadata.ProductName;description=$metadata.FileDescription;company=$metadata.CompanyName}
})
[ordered]@{time=(Get-Date -Format o);processes=$found} | ConvertTo-Json -Depth 5
