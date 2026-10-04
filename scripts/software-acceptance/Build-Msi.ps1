# Builds a registry-only dedicated MSI with an HKCU payload. Does not install anything.
param([Parameter(Mandatory=$true)][string]$OutputDirectory,[ValidateSet('Original','Completion','Restart')][string]$Scenario='Original')
$ErrorActionPreference='Stop'
$taskOutput=[IO.Path]::GetFullPath($OutputDirectory)
New-Item -ItemType Directory -Path $taskOutput -Force | Out-Null
$taskSuffix=if($Scenario -eq 'Original'){''}else{$Scenario}
$taskMsi=Join-Path $taskOutput ('DustAwayAcceptance'+$taskSuffix+'.msi')
$taskIdentity=switch($Scenario){
 'Original' {@('0930','9301','9300','MSIInstalled','DustAway Acceptance MSI')}
 'Completion' {@('0931','9311','9310','MSICompletionInstalled','DustAway Acceptance MSI Completion')}
 'Restart' {@('0932','9321','9320','MSIRestartInstalled','DustAway Acceptance MSI Restart')}
}
if(Test-Path -LiteralPath $taskMsi){throw 'Refusing to replace an existing MSI'}
$taskInstaller=New-Object -ComObject WindowsInstaller.Installer
$taskDb=$taskInstaller.OpenDatabase($taskMsi,3)
function Add-Table([string]$Name,[string[]]$Columns,[string[]]$Types,[string[]]$Keys,[object[]]$Rows){
 $taskLines=@(($Columns -join "`t"),($Types -join "`t"),((@($Name)+$Keys) -join "`t"))
 foreach($taskRow in $Rows){$taskLines+=($taskRow -join "`t")}
 $taskIdt=$Name+'.idt'
 [IO.File]::WriteAllText((Join-Path $taskOutput $taskIdt),($taskLines -join "`r`n")+"`r`n",[Text.Encoding]::ASCII)
 $taskDb.GetType().InvokeMember('Import','InvokeMethod',$null,$taskDb,@($taskOutput,$taskIdt)) | Out-Null
}
Add-Table Property @('Property','Value') @('s72','l0') @('Property') @(
 @('ProductCode',('{7C7E25E8-73B7-4D77-B8E7-7B4BB565'+$taskIdentity[0]+'}')),
 @('UpgradeCode','{D8D3A777-E343-468C-A01E-1EBC81579306}'),
 @('ProductName',$taskIdentity[4]),@('ProductVersion','1.0.0'),
 @('ProductLanguage','1033'),@('Manufacturer','DustAway Acceptance Fixtures'),
 @('INSTALLLEVEL','1'),@('ARPNOMODIFY','1'),@('ARPNOREPAIR','1'),
 @('REBOOT','ReallySuppress'))
Add-Table Directory @('Directory','Directory_Parent','DefaultDir') @('s72','S72','l255') @('Directory') @(
 @('TARGETDIR','','SourceDir'),@('LocalAppDataFolder','TARGETDIR','.'),@('INSTALLDIR','LocalAppDataFolder','DustAwayAcceptance'))
Add-Table Component @('Component','ComponentId','Directory_','Attributes','Condition','KeyPath') @('s72','S38','s72','i2','S255','S72') @('Component') @(
 ,@('FixtureComponent',('{51F6C327-2C2F-4C1C-B188-4063437A'+$taskIdentity[1]+'}'),'INSTALLDIR','4','','FixtureRegistry'))
Add-Table Feature @('Feature','Feature_Parent','Title','Description','Display','Level','Directory_','Attributes') @('s38','S38','L64','L255','I2','i2','S72','i2') @('Feature') @(
 ,@('Main','','Dedicated acceptance fixture','','1','1','','0'))
Add-Table FeatureComponents @('Feature_','Component_') @('s38','s72') @('Feature_','Component_') @(
 ,@('Main','FixtureComponent'))
Add-Table Registry @('Registry','Root','Key','Name','Value','Component_') @('s72','i2','l255','L255','L0','s72') @('Registry') @(
 ,@('FixtureRegistry','1','Software\DustAwayAcceptance\7c7e25e8',$taskIdentity[3],'1','FixtureComponent'))
Add-Table Media @('DiskId','LastSequence','DiskPrompt','Cabinet','VolumeLabel','Source') @('i2','i4','L64','S255','S32','S72') @('DiskId') @(
 ,@('1','0','','','',''))
$taskActions=@(
 @('ValidateProductID','','700'),@('CostInitialize','','800'),@('FileCost','','900'),@('CostFinalize','','1000'),
 @('InstallValidate','','1400'),@('InstallInitialize','','1500'),@('ProcessComponents','','1600'),
 @('UnpublishFeatures','','1800'),@('RemoveRegistryValues','','2600'),@('WriteRegistryValues','','5000'),
 @('RegisterUser','','6000'),@('RegisterProduct','','6100'),@('PublishFeatures','','6300'),
 @('PublishProduct','','6400'),@('InstallFinalize','','6600'))
if($Scenario -eq 'Restart'){
 # A real 3010 path without a system restart: both package and backend suppress reboot.
 $taskActions+=,@('ScheduleReboot','REMOVE="ALL"','6700')
}
Add-Table InstallExecuteSequence @('Action','Condition','Sequence') @('s72','S255','I2') @('Action') $taskActions
Add-Table InstallUISequence @('Action','Condition','Sequence') @('s72','S255','I2') @('Action') @(
 @('CostInitialize','','800'),@('FileCost','','900'),@('CostFinalize','','1000'),@('ExecuteAction','','1300'))
$taskSummary=$taskDb.GetType().InvokeMember('SummaryInformation','GetProperty',$null,$taskDb,@(8))
foreach($taskPair in @(@(2,'Installation Database'),@(3,'DustAway isolated acceptance'),@(4,'DustAway Acceptance Fixtures'),@(7,'Intel;1033'),@(9,('{20D5B96D-796A-4D87-B04F-23D06E67'+$taskIdentity[2]+'}')),@(14,200),@(15,0),@(18,'DustAway acceptance builder'))){
 $taskSummary.GetType().InvokeMember('Property','SetProperty',$null,$taskSummary,$taskPair) | Out-Null
}
$taskSummary.GetType().InvokeMember('Persist','InvokeMethod',$null,$taskSummary,$null) | Out-Null
$taskDb.GetType().InvokeMember('Commit','InvokeMethod',$null,$taskDb,$null) | Out-Null
[Runtime.InteropServices.Marshal]::FinalReleaseComObject($taskSummary) | Out-Null
[Runtime.InteropServices.Marshal]::FinalReleaseComObject($taskDb) | Out-Null
[Runtime.InteropServices.Marshal]::FinalReleaseComObject($taskInstaller) | Out-Null
Get-FileHash -LiteralPath $taskMsi -Algorithm SHA256
