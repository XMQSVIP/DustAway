$ErrorActionPreference='Stop'
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6'){throw 'Wrong host'}
$taskSessions=Invoke-RestMethod 'http://127.0.0.1:4444/sessions'
$taskSession=$taskSessions.value[0].id
if(!$taskSession){throw 'No desktop session'}
$taskBody=@{script='return {uv:document.querySelector("[data-rule="+CSS.escape("dev.uv")+"]")?.outerHTML,dialogs:[...document.querySelectorAll("[role=dialog]")].map(e=>e.textContent),alerts:[...document.querySelectorAll("[role=alert]")].map(e=>e.textContent)}';args=@()} | ConvertTo-Json -Compress
Invoke-RestMethod -Method Post -Uri "http://127.0.0.1:4444/session/$taskSession/execute/sync" -ContentType 'application/json' -Body $taskBody | ConvertTo-Json -Depth 8
