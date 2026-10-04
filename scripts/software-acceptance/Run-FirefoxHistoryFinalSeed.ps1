$ErrorActionPreference='Stop'
$taskRoot=[IO.Path]::GetFullPath($PSScriptRoot)
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6' -or $taskRoot -ne (Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20260930-software') -or [IO.File]::ReadAllText((Join-Path $taskRoot '.dustaway-isolated-acceptance')) -ne 'DustAway.Acceptance.20260930.7c7e25e8'){throw 'Wrong environment'}
try{
 $taskPrincipal=New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
 if($taskPrincipal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){throw 'Ordinary user required'}
 if(Get-Process -Name firefox,geckodriver,Un_A,Au_,FirefoxSetup,dustaway -ErrorAction SilentlyContinue){throw 'Prior process remains'}
 if(Get-NetTCPConnection -State Listen -LocalPort 4446,19838,2829 -ErrorAction SilentlyContinue){throw 'Prior driver remains'}
 $taskPrior=Get-Content -LiteralPath (Join-Path $taskRoot 'firefox-history-graceful-close.json') -Raw | ConvertFrom-Json
 if(-not $taskPrior.OriginalBrowserExited -or -not $taskPrior.NoForce){throw 'Prior browser close not verified'}
 $taskManifest=Get-Content -LiteralPath (Join-Path $taskRoot 'firefox-history-final-seed-manifest.json') -Raw | ConvertFrom-Json
 foreach($taskFile in $taskManifest.Files.PSObject.Properties){if($taskFile.Name -notmatch '^[A-Za-z0-9_.-]+$' -or (Get-FileHash -LiteralPath (Join-Path $taskRoot $taskFile.Name)).Hash -ne $taskFile.Value){throw 'Driver integrity mismatch'}}
 $taskExe=Join-Path $taskRoot 'firefox-real\application\firefox.exe'
 $taskVersion=[Diagnostics.FileVersionInfo]::GetVersionInfo($taskExe)
 if(('{0}.{1}.{2}.{3}' -f $taskVersion.FileMajorPart,$taskVersion.FileMinorPart,$taskVersion.FileBuildPart,$taskVersion.FilePrivatePart) -ne '140.17.0.1158'){throw 'Unexpected Firefox version'}
 $taskSig=Get-AuthenticodeSignature -LiteralPath $taskExe
 if($taskSig.Status -ne 'Valid' -or $taskSig.SignerCertificate.Subject -notmatch 'O=Mozilla Corporation'){throw 'Unexpected browser signature'}
 $taskRun=Start-Process -FilePath 'D:\nodejs\node.exe' -ArgumentList @('firefox-history-seed.mjs','firefox-history-evidence-final') -WorkingDirectory $taskRoot -WindowStyle Hidden -Wait -PassThru -RedirectStandardOutput (Join-Path $taskRoot 'firefox-history-final-seed-run.log') -RedirectStandardError (Join-Path $taskRoot 'firefox-history-final-seed-error.log')
 [pscustomobject]@{ExitCode=$taskRun.ExitCode;Finished=(Get-Date).ToString('o');OrdinaryUser=$true;FreshBrowserAfterVerifiedExit=$true;DeletionAttempted=$false} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskRoot 'firefox-history-final-seed-result.json') -Encoding UTF8
 exit $taskRun.ExitCode
}catch{$_ | Out-String | Add-Content -LiteralPath (Join-Path $taskRoot 'firefox-history-final-seed-error.log') -Encoding UTF8;exit 99}
