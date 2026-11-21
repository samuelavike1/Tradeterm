# Screenshots ticker logos in the release build: the tab strip, the status
# line (Logo on) and the symbol search. Runs the exe with TRADETERM_SETTINGS
# pointing at a copy of settings.json in scripts/logos/tmp (never the user's
# settings; the asset cache, assets.json + logos/, lands there too). Input
# is synthetic and only sent while the app's window is in the foreground.
#
# Usage: shot.ps1 -Theme Dark|Light -Prefix <out path prefix>
# Writes <Prefix>-window.png and <Prefix>-search.png (whole client area).
param(
    [ValidateSet('Dark', 'Light')] [string]$Theme = 'Dark',
    [Parameter(Mandatory)] [string]$Prefix,
    [string]$Query = 'BTC',
    [string]$Exe = 'F:\charting\target-logos\release\tradeterm.exe',
    [int]$Wait = 10,
    [int]$SearchWait = 15
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
$p = Start-Process -FilePath $Exe -PassThru

function Fg {
    $f = [W]::GetForegroundWindow()
    if ($f -ne $script:h) { throw "app window lost the foreground (fg $f pid $([W]::Pid($f)), app $($script:h) pid $($p.Id))" }
}
function Full([string]$out) {
    $bmp = New-Object System.Drawing.Bitmap ([int]$script:cw), ([int]$script:ch)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.CopyFromScreen($script:ox, $script:oy, 0, 0, $bmp.Size); $g.Dispose()
    $bmp.Save($out, [System.Drawing.Imaging.ImageFormat]::Png); $bmp.Dispose()
    "saved $out (scale $script:s)"
}
try {
    $deadline = (Get-Date).AddSeconds(30)
    while ($p.MainWindowHandle -eq 0 -and (Get-Date) -lt $deadline) {
        Start-Sleep -Milliseconds 300; $p.Refresh()
    }
    $script:h = $p.MainWindowHandle
    [W]::MoveWindow($h, 40, 0, 1400, 900, $true) | Out-Null
    Start-Sleep -Seconds $Wait
    [W]::Front($h)
    Start-Sleep -Milliseconds 800
    $script:s = [W]::GetDpiForWindow($h) / 96.0
    $r = New-Object W+RECT; [W]::GetClientRect($h, [ref]$r) | Out-Null
    $pt = New-Object W+POINT; [W]::ClientToScreen($h, [ref]$pt) | Out-Null
    $script:ox = $pt.X; $script:oy = $pt.Y; $script:cw = $r.Right; $script:ch = $r.Bottom
    # Pointer over the title bar, so the status line shows the last bar.
    [W]::SetCursorPos([int]($ox + 900 * $s), [int]($oy + 10 * $s)) | Out-Null
    Start-Sleep -Milliseconds 500
    Full "$Prefix-window.png"
    Fg
    [System.Windows.Forms.SendKeys]::SendWait($Query)
    Start-Sleep -Seconds $SearchWait
    Fg
    Full "$Prefix-search.png"
} finally {
    Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue
}
