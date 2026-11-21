# Window helpers for the alerts screenshots (dot-sourced by start.ps1 /
# act.ps1). Input is synthetic and only sent while the app's window is in
# the foreground.
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
if (-not ('AW' -as [type])) {
Add-Type @"
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
public static class AW {
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc f, IntPtr l);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref POINT p);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool MoveWindow(IntPtr h, int x, int y, int w, int hh, bool r);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int c);
  [DllImport("user32.dll")] public static extern bool BringWindowToTop(IntPtr h);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint p);
  [DllImport("user32.dll")] public static extern bool AttachThreadInput(uint a, uint b, bool f);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, int x, int y, uint d, UIntPtr e);
  [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint f, UIntPtr e);
  [DllImport("kernel32.dll")] public static extern uint GetCurrentThreadId();
  public static List<IntPtr> Windows(uint pid) {
    var list = new List<IntPtr>();
    EnumWindows((h, l) => { uint p; GetWindowThreadProcessId(h, out p);
      if (p == pid && IsWindowVisible(h)) list.Add(h); return true; }, IntPtr.Zero);
    return list;
  }
  public static void Front(IntPtr h) {
    IntPtr fg = GetForegroundWindow(); uint pp;
    uint ft = GetWindowThreadProcessId(fg, out pp), me = GetCurrentThreadId();
    AttachThreadInput(me, ft, true);
    ShowWindow(h, 9); BringWindowToTop(h); SetForegroundWindow(h);
    AttachThreadInput(me, ft, false);
  }
  public struct RECT { public int Left, Top, Right, Bottom; }
  public struct POINT { public int X, Y; }
}
"@
}
[AW]::SetProcessDPIAware() | Out-Null
$Tmp = Join-Path $PSScriptRoot 'tmp'

function App-Process {
    $id = [int](Get-Content (Join-Path $Tmp 'pid.txt'))
    Get-Process -Id $id
}
# The app's windows: main (largest) and the alerts panel window.
function App-Windows {
    $p = App-Process
    $ws = [AW]::Windows([uint32]$p.Id) | ForEach-Object {
        $r = New-Object AW+RECT; [AW]::GetClientRect($_, [ref]$r) | Out-Null
        $pt = New-Object AW+POINT; [AW]::ClientToScreen($_, [ref]$pt) | Out-Null
        [pscustomobject]@{ H = $_; X = $pt.X; Y = $pt.Y; W = $r.Right; Hh = $r.Bottom }
    } | Where-Object { $_.W -gt 100 -and $_.Hh -gt 100 }
    $ws | Sort-Object { $_.W * $_.Hh } -Descending
}
function Main-Window { (App-Windows)[0] }
function Ensure-Fg($w) {
    if ([AW]::GetForegroundWindow() -ne $w.H) { [AW]::Front($w.H); Start-Sleep -Milliseconds 300 }
    if ([AW]::GetForegroundWindow() -ne $w.H) { throw 'app window is not in the foreground' }
}
function Move-To($w, [int]$x, [int]$y) {
    Ensure-Fg $w
    [AW]::SetCursorPos($w.X + $x, $w.Y + $y) | Out-Null; Start-Sleep -Milliseconds 120
}
function Click-At($w, [int]$x, [int]$y, [int]$n = 1) {
    Move-To $w $x $y
    for ($i = 0; $i -lt $n; $i++) {
        [AW]::mouse_event(2, 0, 0, 0, [UIntPtr]::Zero); Start-Sleep -Milliseconds 40
        [AW]::mouse_event(4, 0, 0, 0, [UIntPtr]::Zero); Start-Sleep -Milliseconds 60
    }
    Start-Sleep -Milliseconds 300
}
function Drag-From($w, [int]$x, [int]$y, [int]$dx, [int]$dy) {
    Move-To $w $x $y
    [AW]::mouse_event(2, 0, 0, 0, [UIntPtr]::Zero); Start-Sleep -Milliseconds 80
    for ($i = 1; $i -le 10; $i++) { Move-To $w ($x + $dx * $i / 10) ($y + $dy * $i / 10) }
    [AW]::mouse_event(4, 0, 0, 0, [UIntPtr]::Zero); Start-Sleep -Milliseconds 400
}
# Key with optional Alt (VK 0x12) / Ctrl (0x11).
function Press($w, [byte]$vk, [switch]$Alt, [switch]$Ctrl) {
    Ensure-Fg $w
    if ($Alt) { [AW]::keybd_event(0x12, 0, 0, [UIntPtr]::Zero) }
    if ($Ctrl) { [AW]::keybd_event(0x11, 0, 0, [UIntPtr]::Zero) }
    [AW]::keybd_event($vk, 0, 0, [UIntPtr]::Zero); Start-Sleep -Milliseconds 30
    [AW]::keybd_event($vk, 0, 2, [UIntPtr]::Zero)
    if ($Ctrl) { [AW]::keybd_event(0x11, 0, 2, [UIntPtr]::Zero) }
    if ($Alt) { [AW]::keybd_event(0x12, 0, 2, [UIntPtr]::Zero) }
    Start-Sleep -Milliseconds 400
}
function Shot-Rect([int]$x, [int]$y, [int]$w, [int]$h, [string]$path) {
    $bmp = New-Object System.Drawing.Bitmap $w, $h
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.CopyFromScreen($x, $y, 0, 0, $bmp.Size); $g.Dispose()
    $bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png); $bmp.Dispose()
    "saved $path"
}
function Shot-Window($w, [string]$path) { Ensure-Fg $w; Shot-Rect $w.X $w.Y $w.W $w.Hh $path }
