param(
  [Parameter(Mandatory=$true)][string]$Executable,
  [long]$WindowHandle = 0,
  [int]$Width = 0,
  [int]$Height = 0
)
$ErrorActionPreference = 'Stop'
Add-Type @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class WindowProbe {
  [StructLayout(LayoutKind.Sequential)] public struct Point { public int X, Y; }
  [StructLayout(LayoutKind.Sequential)] public struct Rect { public int Left, Top, Right, Bottom; }
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hwnd, out Rect rectangle);
  [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr hwnd, out Rect rectangle);
  [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr hwnd);
  [DllImport("user32.dll")] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr context);
  [DllImport("user32.dll")] public static extern IntPtr GetAncestor(IntPtr hwnd, uint flags);
  [DllImport("user32.dll", SetLastError = true)] public static extern bool SetWindowPos(IntPtr hwnd, IntPtr after, int x, int y, int width, int height, uint flags);
  public delegate bool EnumWindowProc(IntPtr hwnd, IntPtr parameter);
  [DllImport("user32.dll")] public static extern bool EnumChildWindows(IntPtr hwnd, EnumWindowProc callback, IntPtr parameter);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetClassName(IntPtr hwnd, StringBuilder name, int count);
  [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr hwnd, ref Point point);
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  [DllImport("kernel32.dll", CharSet = CharSet.Unicode)] public static extern uint GetLongPathNameW(string path, StringBuilder expanded, uint capacity);
  [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr hwnd);
  [DllImport("user32.dll")] public static extern bool IsZoomed(IntPtr hwnd);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint processId);
}
'@
[WindowProbe]::SetProcessDPIAware() | Out-Null
[void][WindowProbe]::SetThreadDpiAwarenessContext([IntPtr](-4))
$applicationPath = [IO.Path]::GetFullPath($Executable)
function Expand-ExecutablePath([string]$path) {
  $expanded = New-Object Text.StringBuilder 32768
  if ([WindowProbe]::GetLongPathNameW($path, $expanded, 32768) -eq 0) { throw 'Cannot resolve executable path' }
  return $expanded.ToString()
}
$applicationPath = Expand-ExecutablePath $applicationPath
$applicationName = [IO.Path]::GetFileNameWithoutExtension($applicationPath)
$application = @(Get-Process -Name $applicationName -ErrorAction SilentlyContinue | Where-Object { $_.Path -and (Expand-ExecutablePath $_.Path) -eq $applicationPath })
if ($application.Count -eq 0) { '{"running":false}'; exit 0 }
if ($application.Count -ne 1) { throw 'Expected one application process at the exact executable path' }
$handle = if ($WindowHandle -ne 0) { [IntPtr]$WindowHandle } else { $application[0].MainWindowHandle }
if ($handle -eq 0) { throw 'Application has no main window yet' }
$ownerProcessId = [uint32]0
[WindowProbe]::GetWindowThreadProcessId($handle, [ref]$ownerProcessId) | Out-Null
if ($ownerProcessId -ne $application[0].Id) { throw 'Window does not belong to the expected application' }
if ([WindowProbe]::GetAncestor($handle, 2) -ne $handle) { throw 'Expected a top-level application window' }
if ($Width -ne 0 -or $Height -ne 0) {
  if ($Width -lt 760 -or $Height -lt 580 -or $Width -gt 4096 -or $Height -gt 4096) { throw 'Requested logical client size is outside test limits' }
  if ([WindowProbe]::IsIconic($handle) -or [WindowProbe]::IsZoomed($handle)) { throw 'Restore the test window before resizing it' }
  $outer = New-Object WindowProbe+Rect
  $inner = New-Object WindowProbe+Rect
  if (-not [WindowProbe]::GetWindowRect($handle, [ref]$outer) -or -not [WindowProbe]::GetClientRect($handle, [ref]$inner)) { throw 'Cannot measure resize target' }
  $scale = [WindowProbe]::GetDpiForWindow($handle) / 96.0
  $physicalWidth = [int][Math]::Round($Width * $scale) + ($outer.Right - $outer.Left - $inner.Right)
  $physicalHeight = [int][Math]::Round($Height * $scale) + ($outer.Bottom - $outer.Top - $inner.Bottom)
  # Resize only the owning Tauri window; EdgeDriver's window/rect targets a Chromium child.
  # SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE keeps desktop position and focus unchanged.
  if (-not [WindowProbe]::SetWindowPos($handle, [IntPtr]::Zero, 0, 0, $physicalWidth, $physicalHeight, 22)) { throw 'Native window resize failed' }
  Start-Sleep -Milliseconds 100
}
$bounds = New-Object WindowProbe+Rect
$origin = New-Object WindowProbe+Point
if (-not [WindowProbe]::GetWindowRect($handle, [ref]$bounds) -or -not [WindowProbe]::ClientToScreen($handle, [ref]$origin)) { throw 'Window geometry is unavailable' }
$client = New-Object WindowProbe+Rect
if (-not [WindowProbe]::GetClientRect($handle, [ref]$client)) { throw 'Client geometry is unavailable' }
$children = [Collections.Generic.List[object]]::new()
$callback = [WindowProbe+EnumWindowProc]{ param($child, $parameter)
  $name = New-Object Text.StringBuilder 256
  [void][WindowProbe]::GetClassName($child, $name, 256)
  $rectangle = New-Object WindowProbe+Rect
  if ([WindowProbe]::GetWindowRect($child, [ref]$rectangle)) {
    $children.Add([pscustomobject]@{
      handle = $child.ToInt64().ToString()
      className = $name.ToString()
      x = $rectangle.Left - $origin.X
      y = $rectangle.Top - $origin.Y
      width = $rectangle.Right - $rectangle.Left
      height = $rectangle.Bottom - $rectangle.Top
    })
  }
  return $true
}
[void][WindowProbe]::EnumChildWindows($handle, $callback, [IntPtr]::Zero)
[pscustomobject]@{
  running = $true
  handle = $handle.ToInt64().ToString()
  nonClientTopPixels = $origin.Y - $bounds.Top
  minimized = [WindowProbe]::IsIconic($handle)
  maximized = [WindowProbe]::IsZoomed($handle)
  dpi = [WindowProbe]::GetDpiForWindow($handle)
  client = @{ x = $origin.X; y = $origin.Y; width = $client.Right; height = $client.Bottom }
  children = @($children.ToArray())
} | ConvertTo-Json -Compress -Depth 5
