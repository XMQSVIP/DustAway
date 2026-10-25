param([Parameter(Mandatory=$true)][string]$ScriptFile,[string]$ParametersJson='{}',[string]$ParametersBase64='')
$ErrorActionPreference='Stop'
$knownHosts=Join-Path $env:USERPROFILE '.ssh/dustaway-desktop-24njtt6-known-hosts'
$key=Join-Path $env:USERPROFILE '.ssh/dustaway-desktop-24njtt6-ed25519'
$git='C:\Program Files\Git\usr\bin'
$fingerprint=& (Join-Path $git 'ssh-keygen.exe') -lf $knownHosts
if($LASTEXITCODE -ne 0 -or $fingerprint -notmatch [regex]::Escape('SHA256:qTe49uQ4Pc/szc7HFgm8ICcDMDO8laK/zLVPLhLsYIE')){throw 'Host fingerprint mismatch'}
$body=[IO.File]::ReadAllText((Resolve-Path -LiteralPath $ScriptFile).Path)
$arguments=if($ParametersBase64){$ParametersBase64}else{[Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes($ParametersJson))}
$source="`$ErrorActionPreference='Stop'; `$ProgressPreference='SilentlyContinue'; [Console]::OutputEncoding=[Text.UTF8Encoding]::new(`$false); `$a=@{}; (ConvertFrom-Json ([Text.Encoding]::UTF8.GetString([Convert]::FromBase64String('$arguments')))).PSObject.Properties | ForEach-Object {`$a[`$_.Name]=`$_.Value}; & {`n"+$body+"`n} @a"
$options=@('-F','/dev/null','-i',$key.Replace('\','/'),'-o','IdentitiesOnly=yes','-o','BatchMode=yes','-o','ConnectTimeout=8','-o','StrictHostKeyChecking=yes','-o',('UserKnownHostsFile='+$knownHosts.Replace('\','/')),'-o','GlobalKnownHostsFile=/dev/null','-o','HostKeyAlgorithms=ssh-ed25519')
$name='dustaway-ssh-'+[Guid]::NewGuid().ToString('N')+'.ps1'
$local=Join-Path ([IO.Path]::GetTempPath()) $name
$remote='C:/Users/MSN/AppData/Local/Temp/'+$name
try {
  [IO.File]::WriteAllText($local,$source,[Text.UTF8Encoding]::new($false))
  $hash=[BitConverter]::ToString([Security.Cryptography.SHA256]::Create().ComputeHash([IO.File]::ReadAllBytes($local))).Replace('-','')
  $unix='/'+$local.Substring(0,1).ToLower()+'/'+$local.Substring(3).Replace('\','/')
  & (Join-Path $git 'scp.exe') @options $unix ('MSN@100.69.113.49:'+$remote)
  if($LASTEXITCODE -ne 0){throw 'Script upload failed'}
  $wrapper="`$ErrorActionPreference='Stop'; `$ProgressPreference='SilentlyContinue'; if((Get-FileHash -LiteralPath '$remote').Hash -ne '$hash'){throw 'Transport script changed'}; try { & ([ScriptBlock]::Create([IO.File]::ReadAllText('$remote'))) } finally { Remove-Item -LiteralPath '$remote' }"
  $encoded=[Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($wrapper))
  & (Join-Path $git 'ssh.exe') @options '-l' 'MSN' '100.69.113.49' "powershell.exe -NoProfile -NonInteractive -OutputFormat Text -EncodedCommand $encoded"
  $result=$LASTEXITCODE
}finally {if(Test-Path -LiteralPath $local){Remove-Item -LiteralPath $local}}
exit $result
