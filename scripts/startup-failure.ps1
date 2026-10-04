param([string]$Executable = 'target/debug/dustaway.exe', [string]$Output = 'artifacts/space-0.11')
$ErrorActionPreference = 'Stop'
# Only the guarded debug fixture backend consumes this directory. Release builds ignore it.
$fixtureRoot = Join-Path $env:TEMP ('dustaway-e2e-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $fixtureRoot | Out-Null
[IO.File]::WriteAllText((Join-Path $fixtureRoot '.dustaway-test-only'), 'fixture-only')
foreach ($name in @('cache','analysis','history.sqlite')) {
  New-Item -ItemType Directory -Path (Join-Path $fixtureRoot $name) | Out-Null
}
[IO.File]::WriteAllText((Join-Path $fixtureRoot 'cache/protected.txt'), 'keep')
$outputDirectory = [IO.Path]::GetFullPath($Output)
New-Item -ItemType Directory -Path $outputDirectory -Force | Out-Null
Add-Type -ReferencedAssemblies System.Drawing @'
using System;
using System.Text;
using System.Collections.Generic;
using System.Runtime.InteropServices;
public static class StartupProbe {
  public delegate bool Visitor(IntPtr h, IntPtr p);
  [DllImport("user32.dll")] public static extern bool EnumWindows(Visitor callback, IntPtr p);
  [DllImport("user32.dll")] public static extern bool EnumChildWindows(IntPtr parent, Visitor callback, IntPtr p);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint id);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr h, StringBuilder text, int length);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassNameW(IntPtr h, StringBuilder text, int length);
  [DllImport("user32.dll")] public static extern bool PostMessageW(IntPtr h, uint message, IntPtr w, IntPtr l);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out Rect r);
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr dc, uint flags);
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  [StructLayout(LayoutKind.Sequential)] public struct Rect { public int Left,Top,Right,Bottom; }
  public static IntPtr Find(uint pid) {
    IntPtr found=IntPtr.Zero;
    EnumWindows((h,p)=>{uint owner;GetWindowThreadProcessId(h,out owner);var cls=new StringBuilder(128);GetClassNameW(h,cls,128);if(owner==pid && cls.ToString()=="#32770") found=h;return true;},IntPtr.Zero);
    return found;
  }
  public static string Read(IntPtr h) {
    var parts=new List<string>();EnumChildWindows(h,(child,p)=>{var s=new StringBuilder(4096);GetWindowTextW(child,s,4096);parts.Add(s.ToString());return true;},IntPtr.Zero);return string.Join("\n",parts);
  }
  public static void Capture(IntPtr h,string path) {
    Rect r;GetWindowRect(h,out r);using(var bitmap=new System.Drawing.Bitmap(r.Right-r.Left,r.Bottom-r.Top)) {using(var g=System.Drawing.Graphics.FromImage(bitmap)) {var dc=g.GetHdc();try {if(!PrintWindow(h,dc,0))throw new Exception("PrintWindow failed");}finally{g.ReleaseHdc(dc);}}bitmap.Save(path,System.Drawing.Imaging.ImageFormat.Png);}
  }
}
'@
[StartupProbe]::SetProcessDPIAware() | Out-Null
$previousFixture = $env:DUSTAWAY_E2E_ROOT
try {
  $env:DUSTAWAY_E2E_ROOT = $fixtureRoot
  $application = Start-Process -FilePath ([IO.Path]::GetFullPath($Executable)) -WindowStyle Hidden -PassThru -RedirectStandardError (Join-Path $outputDirectory 'startup-stderr.log') -RedirectStandardOutput (Join-Path $outputDirectory 'startup-stdout.log')
  $deadline = [DateTime]::UtcNow.AddSeconds(20)
  $dialog = [IntPtr]::Zero
  while ($dialog -eq [IntPtr]::Zero -and -not $application.HasExited -and [DateTime]::UtcNow -lt $deadline) {
    Start-Sleep -Milliseconds 100
    $dialog = [StartupProbe]::Find($application.Id)
  }
  if ($dialog -eq [IntPtr]::Zero) { throw 'Expected a native startup failure dialog from the fixture executable' }
  $message = [StartupProbe]::Read($dialog)
  if ($message -notlike '*Windows 存储设置*' -or $message -notlike '*尚未启动清理*' -or $message -notlike '*database*') { throw "Unexpected dialog: $message" }
  [StartupProbe]::Capture($dialog, (Join-Path $outputDirectory 'startup-database-failure.png'))
  # Decline only this process-owned dialog; do not launch Windows Settings during the probe.
  [StartupProbe]::PostMessageW($dialog, 0x111, [IntPtr]7, [IntPtr]::Zero) | Out-Null
  if (-not $application.WaitForExit(10000)) { throw 'Application did not finish after closing recovery prompt' }
  if ([IO.File]::ReadAllText((Join-Path $fixtureRoot 'cache/protected.txt')) -ne 'keep') { throw 'Fixture changed' }
  if (-not (Test-Path -LiteralPath (Join-Path $fixtureRoot 'history.sqlite') -PathType Container)) { throw 'Database path was overwritten' }
  @{ fixture=$fixtureRoot; actualErrorPreserved=$true; nativeStorageRecoveryOffered=$true; noDeletion=$true; databaseNotRebuilt=$true; message=$message } |
    ConvertTo-Json | Set-Content -LiteralPath (Join-Path $outputDirectory 'startup-database-failure.json') -Encoding UTF8
  'PASS: native database failure recovery, original error, unchanged fixture and database path'
} finally {
  $env:DUSTAWAY_E2E_ROOT = $previousFixture
  if ($application -and -not $application.HasExited) { $application.Kill() }
}
