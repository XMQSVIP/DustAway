$ErrorActionPreference='Stop'
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6'){throw 'Wrong host'}
@(Get-CimInstance Win32_Process | Where-Object {$_.Name -match 'DingTalk|BaiduNetdisk|YunDetect|YunUtility' -or $_.ExecutablePath -like 'C:\Users\MSN\AppData\Roaming\baidu\BaiduNetdisk\*' -or $_.ExecutablePath -like 'C:\Program Files (x86)\DingDing\*'} | Select-Object Name,ProcessId,SessionId,ExecutablePath) | ConvertTo-Json -Depth 3
