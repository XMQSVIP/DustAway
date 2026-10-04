param([Parameter(Mandatory=$true)][string]$Directory, [switch]$Restore)
$ErrorActionPreference = 'Stop'
$fixtureDirectory = [IO.Path]::GetFullPath($Directory)
$fixtureRoot = Split-Path -Parent $fixtureDirectory
$temporaryRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\')
if ((Split-Path -Parent $fixtureRoot).TrimEnd('\') -ine $temporaryRoot -or (Split-Path -Leaf $fixtureRoot) -notlike 'fuchen-portable-smoke-*') { throw 'Not an owned portable smoke fixture' }
$savedAcl = Join-Path $fixtureRoot 'original-acl.sddl'
if ($Restore) {
  $acl = [IO.Directory]::GetAccessControl($fixtureDirectory)
  $sddl = [IO.File]::ReadAllText($savedAcl)
  $acl.SetSecurityDescriptorSddlForm($sddl, [Security.AccessControl.AccessControlSections]::Access)
  [IO.Directory]::SetAccessControl($fixtureDirectory, $acl)
} else {
  $acl = [IO.Directory]::GetAccessControl($fixtureDirectory)
  [IO.File]::WriteAllText($savedAcl, $acl.Sddl)
  $sid = [Security.Principal.WindowsIdentity]::GetCurrent().User
  $deny = New-Object Security.AccessControl.FileSystemAccessRule($sid, 'Write', 'ContainerInherit,ObjectInherit', 'None', 'Deny')
  $acl.AddAccessRule($deny)
  [IO.Directory]::SetAccessControl($fixtureDirectory, $acl)
}
