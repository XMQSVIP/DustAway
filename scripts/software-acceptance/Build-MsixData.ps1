# Build only. Does not install a package, trust a certificate, or change developer mode.
# Build the fixed Rust example first; every packaging attempt requires a new artifacts directory.
param([Parameter(Mandatory=$true)][string]$OutputDirectory)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$taskWorkspace = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
$taskArtifacts = Join-Path $taskWorkspace 'artifacts'
$taskOutput = [IO.Path]::GetFullPath($OutputDirectory).TrimEnd('\')
if (-not [string]::Equals([IO.Path]::GetDirectoryName($taskOutput), $taskArtifacts, [StringComparison]::OrdinalIgnoreCase) -or
    [IO.Path]::GetFileName($taskOutput) -notmatch '^store-data-msix-[a-zA-Z0-9-]+$') {
    throw 'Output must be a new store-data-msix-* directory directly in workspace artifacts'
}
foreach ($taskAncestor in @($taskWorkspace, $taskArtifacts)) {
    $taskInfo = Get-Item -LiteralPath $taskAncestor
    if (($taskInfo.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) { throw 'Reparse point output ancestor refused' }
}
if (Test-Path -LiteralPath $taskOutput) { throw 'Output already exists; preserve the original attempt' }
$taskExe = Join-Path $taskWorkspace 'target\debug\examples\store_data_fixture.exe'
$taskManifest = Join-Path $PSScriptRoot 'StoreDataFixture.AppxManifest.xml'
$taskSdk = 'C:\Program Files (x86)\Windows Kits\10\bin\10.0.22621.0\x64'
foreach ($taskInput in @($taskExe, $taskManifest, (Join-Path $taskSdk 'makeappx.exe'), (Join-Path $taskSdk 'signtool.exe'))) {
    if (-not (Test-Path -LiteralPath $taskInput -PathType Leaf)) { throw "Missing input: $taskInput" }
}
$taskIdentity = ([xml][IO.File]::ReadAllText($taskManifest)).Package.Identity
if ($taskIdentity.Name -cne 'DustAway.Acceptance.Data.20261001' -or
    $taskIdentity.Publisher -cne 'CN=DustAway Data Acceptance 20261001' -or
    $taskIdentity.Version -cne '1.0.0.0') { throw 'Unexpected fixture package identity' }
New-Item -ItemType Directory -Path $taskOutput | Out-Null
$taskContents = Join-Path $taskOutput 'contents'
New-Item -ItemType Directory -Path $taskContents | Out-Null
Copy-Item -LiteralPath $taskExe -Destination (Join-Path $taskContents 'StoreDataFixture.exe')
Copy-Item -LiteralPath $taskManifest -Destination (Join-Path $taskContents 'AppxManifest.xml')
Add-Type -AssemblyName System.Drawing
foreach ($taskSize in @(44, 50, 150)) {
    $taskBitmap = New-Object Drawing.Bitmap($taskSize, $taskSize)
    $taskGraphics = [Drawing.Graphics]::FromImage($taskBitmap)
    try {
        $taskGraphics.Clear([Drawing.Color]::RoyalBlue)
        $taskBitmap.Save((Join-Path $taskContents ('Logo' + $taskSize + '.png')), [Drawing.Imaging.ImageFormat]::Png)
    } finally { $taskGraphics.Dispose(); $taskBitmap.Dispose() }
}
$taskPackage = Join-Path $taskOutput 'DustAwayAcceptanceData.msix'
& (Join-Path $taskSdk 'makeappx.exe') pack /d $taskContents /p $taskPackage *> (Join-Path $taskOutput 'pack.log')
if ($LASTEXITCODE -ne 0) { throw 'MSIX packing failed; see pack.log' }
$taskCert = New-SelfSignedCertificate -Type Custom -Subject $taskIdentity.Publisher -FriendlyName 'DustAway MSIX data fixture temporary signer' -KeyUsage DigitalSignature -KeyExportPolicy NonExportable -CertStoreLocation 'Cert:\CurrentUser\My' -NotAfter (Get-Date).AddDays(2) -TextExtension @('2.5.29.37={text}1.3.6.1.5.5.7.3.3', '2.5.29.19={text}')
$taskThumbprint = $taskCert.Thumbprint
try {
    Export-Certificate -Cert $taskCert -FilePath (Join-Path $taskOutput 'DustAwayAcceptanceData.cer') | Out-Null
    [IO.File]::WriteAllText((Join-Path $taskOutput 'signer-thumbprint.txt'), $taskThumbprint)
    & (Join-Path $taskSdk 'signtool.exe') sign /fd SHA256 /sha1 $taskThumbprint $taskPackage *> (Join-Path $taskOutput 'sign.log')
    if ($LASTEXITCODE -ne 0) { throw 'MSIX signing failed; see sign.log' }
} finally {
    # Remove only the signer this invocation created, including its nonexportable private key.
    Remove-Item -LiteralPath ('Cert:\CurrentUser\My\' + $taskThumbprint) -DeleteKey
}
if (Test-Path -LiteralPath ('Cert:\CurrentUser\My\' + $taskThumbprint)) { throw 'Temporary signer cleanup failed' }
$taskTrustStores = @('Cert:\CurrentUser\TrustedPeople', 'Cert:\CurrentUser\Root', 'Cert:\LocalMachine\TrustedPeople', 'Cert:\LocalMachine\Root')
foreach ($taskStore in $taskTrustStores) {
    if (Test-Path -LiteralPath ($taskStore + '\' + $taskThumbprint)) { throw 'Unexpected trust for the new signer' }
}
$taskReport = [ordered]@{
    time = (Get-Date).ToString('o')
    identity = $taskIdentity.Name
    publisher = $taskIdentity.Publisher
    version = $taskIdentity.Version
    package_sha256 = (Get-FileHash -LiteralPath $taskPackage -Algorithm SHA256).Hash
    executable_sha256 = (Get-FileHash -LiteralPath (Join-Path $taskContents 'StoreDataFixture.exe') -Algorithm SHA256).Hash
    source_sha256 = (Get-FileHash -LiteralPath (Join-Path $taskWorkspace 'crates\windows\examples\store_data_fixture.rs') -Algorithm SHA256).Hash
    manifest_sha256 = (Get-FileHash -LiteralPath $taskManifest -Algorithm SHA256).Hash
    public_certificate_sha256 = (Get-FileHash -LiteralPath (Join-Path $taskOutput 'DustAwayAcceptanceData.cer') -Algorithm SHA256).Hash
    signer_thumbprint = $taskThumbprint
    temporary_signer_and_private_key_removed = $true
    trusted_certificate_installed = $false
    package_installed = $false
    fixture_run = $false
    production_authority_granted = $false
}
$taskReport | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $taskOutput 'build-report.json') -Encoding UTF8
$taskReport | ConvertTo-Json -Depth 4
