$ErrorActionPreference='Stop'
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6' -or $env:USERNAME -ne 'MSN'){throw 'Wrong host/user'}
$taskPaths=@(
 @('npm', (Join-Path $env:LOCALAPPDATA 'npm-cache')),
 @('pip', (Join-Path $env:LOCALAPPDATA 'pip/Cache')),
 @('nuget', (Join-Path $env:LOCALAPPDATA 'NuGet/v3-cache')),
 @('vscode', (Join-Path $env:APPDATA 'Code')),
 @('vscode-insiders', (Join-Path $env:APPDATA 'Code - Insiders')),
 @('jetbrains', (Join-Path $env:LOCALAPPDATA 'JetBrains'))
)
[ordered]@{
 host=$env:COMPUTERNAME;user=$env:USERNAME
 directories=@(foreach($taskEntry in $taskPaths){[ordered]@{tool=$taskEntry[0];path=$taskEntry[1];exists=(Test-Path -LiteralPath $taskEntry[1]);children=@(if(Test-Path -LiteralPath $taskEntry[1]){Get-ChildItem -LiteralPath $taskEntry[1] -Force | Select-Object Name,PSIsContainer})}})
 tools=@(Get-Command node,python,pip,dotnet,code -ErrorAction SilentlyContinue | Select-Object Name,Source)
 processes=@(Get-Process -Name node,python,pythonw,pip,dotnet,MSBuild,devenv,Code,'Code - Insiders',idea64,pycharm64,webstorm64,dustaway,tauri-driver,msedgedriver -ErrorAction SilentlyContinue | Select-Object ProcessName,Id,SessionId)
} | ConvertTo-Json -Depth 5
