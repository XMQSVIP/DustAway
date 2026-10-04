$ErrorActionPreference='Stop'
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6'){throw 'Wrong host'}
$directory='D:\BaiduNetdiskDownload'
$names=@('amazon_twister_parser.py','py_project260907.zip')
$files=@(Get-ChildItem -LiteralPath $directory -File | Where-Object {$_.Name -eq $names[0] -or $_.Name -eq $names[1] -or $_.Name.StartsWith($names[1]+'.',[StringComparison]::OrdinalIgnoreCase)} | Select-Object Name,Length,CreationTimeUtc,LastWriteTimeUtc,Attributes)
[ordered]@{time=(Get-Date -Format o);directory=$directory;user_named_fixtures=$files;read_only=$true;contents_read=$false} | ConvertTo-Json -Depth 5
