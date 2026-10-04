# Creates a signed dedicated package. No app or trusted certificate is installed locally.
param([Parameter(Mandatory=$true)][string]$OutputDirectory)
$ErrorActionPreference='Stop'
$taskOutput=[IO.Path]::GetFullPath($OutputDirectory)
New-Item -ItemType Directory -Path $taskOutput -Force | Out-Null
$taskContents=Join-Path $taskOutput 'contents'
New-Item -ItemType Directory -Path $taskContents -Force | Out-Null
$taskSdk='C:\Program Files (x86)\Windows Kits\10\bin\10.0.22621.0\x64'
$taskPackage=Join-Path $taskOutput 'DustAwayAcceptance.msix'
if(Test-Path -LiteralPath $taskPackage){throw 'Package already exists'}
$taskApp='internal static class App { public static int Main() { return 0; } }'
[IO.File]::WriteAllText((Join-Path $taskOutput 'App.cs'),$taskApp)
& 'C:\Windows\Microsoft.NET\Framework64\v4.0.30319\csc.exe' /nologo /target:winexe ('/out:'+(Join-Path $taskContents 'AcceptanceApp.exe')) (Join-Path $taskOutput 'App.cs')
if($LASTEXITCODE -ne 0){throw 'App build failed'}
Add-Type -AssemblyName System.Drawing
foreach($taskSize in @(44,50,150)){
 $taskBitmap=New-Object Drawing.Bitmap($taskSize,$taskSize)
 $taskGraphics=[Drawing.Graphics]::FromImage($taskBitmap)
 try{$taskGraphics.Clear([Drawing.Color]::RoyalBlue);$taskBitmap.Save((Join-Path $taskContents ('Logo'+$taskSize+'.png')),[Drawing.Imaging.ImageFormat]::Png)}finally{$taskGraphics.Dispose();$taskBitmap.Dispose()}
}
$taskManifest=@'
<?xml version="1.0" encoding="utf-8"?>
<Package xmlns="http://schemas.microsoft.com/appx/manifest/foundation/windows10" xmlns:uap="http://schemas.microsoft.com/appx/manifest/uap/windows10" xmlns:rescap="http://schemas.microsoft.com/appx/manifest/foundation/windows10/restrictedcapabilities" IgnorableNamespaces="uap rescap">
  <Identity Name="DustAway.Acceptance.7c7e25e8" Publisher="CN=DustAway Acceptance 7c7e25e8" Version="1.0.0.0" ProcessorArchitecture="x64" />
  <Properties><DisplayName>DustAway Acceptance MSIX</DisplayName><PublisherDisplayName>DustAway Acceptance Fixtures</PublisherDisplayName><Logo>Logo50.png</Logo></Properties>
  <Dependencies><TargetDeviceFamily Name="Windows.Desktop" MinVersion="10.0.19041.0" MaxVersionTested="10.0.26200.0" /></Dependencies>
  <Resources><Resource Language="en-us" /></Resources>
  <Applications><Application Id="App" Executable="AcceptanceApp.exe" EntryPoint="Windows.FullTrustApplication"><uap:VisualElements DisplayName="DustAway Acceptance MSIX" Description="Dedicated uninstall acceptance fixture" BackgroundColor="transparent" Square150x150Logo="Logo150.png" Square44x44Logo="Logo44.png" /></Application></Applications>
  <Capabilities><rescap:Capability Name="runFullTrust" /></Capabilities>
</Package>
'@
[IO.File]::WriteAllText((Join-Path $taskContents 'AppxManifest.xml'),$taskManifest,(New-Object Text.UTF8Encoding($false)))
& (Join-Path $taskSdk 'makeappx.exe') pack /d $taskContents /p $taskPackage
if($LASTEXITCODE -ne 0){throw 'MSIX packing failed'}
$taskCert=New-SelfSignedCertificate -Type Custom -Subject 'CN=DustAway Acceptance 7c7e25e8' -FriendlyName 'DustAway isolated acceptance temporary signer' -KeyUsage DigitalSignature -KeyExportPolicy NonExportable -CertStoreLocation 'Cert:\CurrentUser\My' -NotAfter (Get-Date).AddDays(2) -TextExtension @('2.5.29.37={text}1.3.6.1.5.5.7.3.3','2.5.29.19={text}')
try {
 Export-Certificate -Cert $taskCert -FilePath (Join-Path $taskOutput 'DustAwayAcceptance.cer') | Out-Null
 [IO.File]::WriteAllText((Join-Path $taskOutput 'signer-thumbprint.txt'),$taskCert.Thumbprint)
 & (Join-Path $taskSdk 'signtool.exe') sign /fd SHA256 /sha1 $taskCert.Thumbprint $taskPackage
 if($LASTEXITCODE -ne 0){throw 'MSIX signing failed'}
} finally {
 # The exact temporary local signer is removed after signing; its private key is never exported.
 Remove-Item -LiteralPath ('Cert:\CurrentUser\My\'+$taskCert.Thumbprint) -DeleteKey
}
Get-FileHash -LiteralPath $taskPackage -Algorithm SHA256
