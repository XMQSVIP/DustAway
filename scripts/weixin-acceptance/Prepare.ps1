$ErrorActionPreference='Stop'
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6'){throw 'Wrong host'}
$root='C:\Users\MSN\AppData\Local\DustAwayAcceptance\weixin-021'
$marker=Join-Path $root '.dustaway-test'
if(Test-Path -LiteralPath $root){
  if(-not (Test-Path -LiteralPath $marker) -or [IO.File]::ReadAllText($marker) -cne 'DustAway.Weixin.0.21'){throw 'Existing unowned directory'}
} else {
  New-Item -ItemType Directory -Path $root | Out-Null
  [IO.File]::WriteAllText($marker,'DustAway.Weixin.0.21')
}
[PSCustomObject]@{root=$root;processes=@(Get-Process -Name Weixin,WeChatAppEx,LogonUI -ErrorAction SilentlyContinue|Select-Object ProcessName,SessionId)}|ConvertTo-Json -Depth 3
