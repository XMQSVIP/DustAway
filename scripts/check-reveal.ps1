param([Parameter(Mandatory=$true)][string]$Path)
$ErrorActionPreference = 'Stop'
Add-Type @'
using System.Text;
using System.Runtime.InteropServices;
public static class RevealPath {
  [DllImport("kernel32.dll", CharSet=CharSet.Unicode)] public static extern uint GetLongPathNameW(string value, StringBuilder result, uint size);
}
'@
function Expand-RevealPath([string]$value) {
  $buffer = New-Object Text.StringBuilder 32768
  if ([RevealPath]::GetLongPathNameW($value, $buffer, 32768) -eq 0) { return $value }
  return $buffer.ToString()
}
$target = Expand-RevealPath ([IO.Path]::GetFullPath($Path))
$shell = New-Object -ComObject Shell.Application
# Shell selection completes asynchronously after the IPC request returns.
$deadline = (Get-Date).AddSeconds(10)
do {
  foreach ($window in $shell.Windows()) {
    try {
      foreach ($item in $window.Document.SelectedItems()) {
        if ((Expand-RevealPath $item.Path) -eq $target) { Write-Output 'SELECTED'; return }
      }
    } catch { }
  }
  Start-Sleep -Milliseconds 100
} while ((Get-Date) -lt $deadline)
throw 'Explorer did not select the requested fixture file within 10 seconds.'
