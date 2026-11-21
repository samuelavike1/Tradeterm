# Screenshots for the right sidebar work (release build).
# Runs the exe with TRADETERM_SETTINGS pointing at a copy of settings.json in
# scripts/rightpanel/tmp (never the user's settings). Input is synthetic and only
# sent while the app's window is in the foreground.
#
# Usage: shot.ps1 -Theme Dark|Light -Prefix <out path prefix> -Steps <steps>
#   steps, ';'-separated, coordinates in logical px of the client area:
#     click:x,y   rclick:x,y   move:x,y   key:<SendKeys text>   wait:ms
#     shot:name              whole client area -> <Prefix>-name.png
#     shot:name@x,y,w,h      a region          -> <Prefix>-name.png
#   -Exe: tradeterm.exe (default) or the gallery example (-ExeArgs '--icons').
param(
    [ValidateSet('Dark', 'Light')] [string]$Theme = 'Dark',
    [Parameter(Mandatory)] [string]$Prefix,
    [string]$Steps = 'shot:window',
    [string]$Exe = 'F:\charting\target-sidebar\release\tradeterm.exe',
    [string[]]$ExeArgs = @(),
    [int]$W = 1400,
    [int]$H = 900,
    [int]$Wait = 8
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms
Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class W {
  [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref POINT p);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool MoveWindow(IntPtr h, int x, int y, int w, int hh, bool r);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int c);
  [DllImport("user32.dll")] public static extern bool BringWindowToTop(IntPtr h);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint p);
  [DllImport("user32.dll")] public static extern bool AttachThreadInput(uint a, uint b, bool f);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint dx, uint dy, uint d, UIntPtr e);
  [DllImport("kernel32.dll")] public static extern uint GetCurrentThreadId();
  public static uint Pid(IntPtr h) { uint p; GetWindowThreadProcessId(h, out p); return p; }
  public static void Front(IntPtr h) {
    IntPtr fg = GetForegroundWindow(); uint pp;
    uint ft = GetWindowThreadProcessId(fg, out pp), me = GetCurrentThreadId();
    AttachThreadInput(me, ft, true);
    ShowWindow(h, 6); ShowWindow(h, 9);
    BringWindowToTop(h); SetForegroundWindow(h);
    AttachThreadInput(me, ft, false);
  }
  public struct RECT { public int Left, Top, Right, Bottom; }
  public struct POINT { public int X, Y; }
}
"@
[W]::SetProcessDPIAware() | Out-Null
$tmp = Join-Path $PSScriptRoot 'tmp'
New-Item -ItemType Directory -Force $tmp | Out-Null
$settingsPath = Join-Path $tmp 'settings.json'
(Get-Content (Join-Path $PSScriptRoot 'settings.json') -Raw) -replace '"theme_mode": "Dark"', "`"theme_mode`": `"$Theme`"" |
    Set-Content $settingsPath -Encoding utf8NoBOM
$env:TRADETERM_SETTINGS = $settingsPath
if ($ExeArgs.Count -gt 0) {
    $p = Start-Process -FilePath $Exe -ArgumentList $ExeArgs -PassThru
} else {
    $p = Start-Process -FilePath $Exe -PassThru
}

function Fg {
    $f = [W]::GetForegroundWindow()
    if ($f -ne $script:h) { throw "app window lost the foreground (fg $f pid $([W]::Pid($f)), app $($script:h) pid $($p.Id))" }
}
function Shot([string]$name, [double[]]$r) {
    $s = $script:s
    if ($r) {
        $x = [int]($r[0] * $s); $y = [int]($r[1] * $s); $cw = [int]($r[2] * $s); $ch = [int]($r[3] * $s)
    } else { $x = 0; $y = 0; $cw = $script:cw; $ch = $script:ch }
    $bmp = New-Object System.Drawing.Bitmap $cw, $ch
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.CopyFromScreen($script:ox + $x, $script:oy + $y, 0, 0, $bmp.Size); $g.Dispose()
    $out = "$Prefix-$name.png"
    $bmp.Save($out, [System.Drawing.Imaging.ImageFormat]::Png); $bmp.Dispose()
    "saved $out (scale $s)"
}
function Pt([string]$v) {
    $a = $v.Split(',') | ForEach-Object { [double]$_ }
    [W]::SetCursorPos([int]($script:ox + $a[0] * $script:s), [int]($script:oy + $a[1] * $script:s)) | Out-Null
    Start-Sleep -Milliseconds 120
}
try {
    $deadline = (Get-Date).AddSeconds(30)
    while ($p.MainWindowHandle -eq 0 -and (Get-Date) -lt $deadline) {
        Start-Sleep -Milliseconds 300; $p.Refresh()
    }
    $script:h = $p.MainWindowHandle
    [W]::MoveWindow($h, 40, 0, $W, $H, $true) | Out-Null
    Start-Sleep -Seconds $Wait
    [W]::Front($h)
    Start-Sleep -Milliseconds 800
    $script:s = [W]::GetDpiForWindow($h) / 96.0
    $r = New-Object W+RECT; [W]::GetClientRect($h, [ref]$r) | Out-Null
    $pt = New-Object W+POINT; [W]::ClientToScreen($h, [ref]$pt) | Out-Null
    $script:ox = $pt.X; $script:oy = $pt.Y; $script:cw = $r.Right; $script:ch = $r.Bottom
    # Park the pointer over the title bar.
    Pt "700,10"
    foreach ($step in $Steps.Split(';')) {
        $kind, $arg = $step.Split(':', 2)
        switch ($kind) {
            'wait' { Start-Sleep -Milliseconds ([int]$arg) }
            'move' { Fg; Pt $arg }
            'click' { Fg; Pt $arg; [W]::mouse_event(2, 0, 0, 0, [UIntPtr]::Zero); Start-Sleep -Milliseconds 40; [W]::mouse_event(4, 0, 0, 0, [UIntPtr]::Zero); Start-Sleep -Milliseconds 500 }
            'rclick' { Fg; Pt $arg; [W]::mouse_event(8, 0, 0, 0, [UIntPtr]::Zero); Start-Sleep -Milliseconds 40; [W]::mouse_event(16, 0, 0, 0, [UIntPtr]::Zero); Start-Sleep -Milliseconds 500 }
            'key' { Fg; [System.Windows.Forms.SendKeys]::SendWait($arg); Start-Sleep -Milliseconds 400 }
            'shot' {
                if ($arg.Contains('@')) {
                    $n, $rr = $arg.Split('@', 2)
                    Shot $n ($rr.Split(',') | ForEach-Object { [double]$_ })
                } else { Shot $arg $null }
            }
        }
    }
} finally {
    Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue
}
