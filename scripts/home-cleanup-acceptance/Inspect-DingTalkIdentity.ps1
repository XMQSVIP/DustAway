$ErrorActionPreference='Stop'
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6'){throw 'Wrong host'}
$path='C:\Program Files (x86)\DingDing\main\current\DingTalk.exe'
$file=Get-Item -LiteralPath $path
$version=$file.VersionInfo
@{path=$path;fixed_version=('{0}.{1}.{2}.{3}' -f $version.FileMajorPart,$version.FileMinorPart,$version.FileBuildPart,$version.FilePrivatePart);display_version=$version.FileVersion;parent_attributes=[string](Get-Item -LiteralPath $file.DirectoryName).Attributes;time=(Get-Date -Format o)} | ConvertTo-Json
