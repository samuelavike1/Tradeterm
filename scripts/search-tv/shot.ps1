# Screenshots the symbol search and the indicator picker (TradingView
# layout) in the release build, and measures the first search's latency.
# Runs the exe with TRADETERM_SETTINGS pointing at a copy of settings.json in
# scripts/search-tv/tmp (never the user's settings; the asset and symbol
# caches land there too). Input is synthetic and only sent while the app's
# window is in the foreground.
#
# Usage: shot.ps1 -Theme Dark|Light -Out <dir> [-Fresh]
#   -Fresh: delete the caches first (cold start, measures the download).
# Writes <Out>/search-tv-<theme>-{empty,btc,eth}.png and
# <Out>/picker-tv-<theme>-{technicals,favorites,search}.png, and prints the
# symbol-list and search timings from the app's log.
param(
    [ValidateSet('Dark', 'Light')] [string]$Theme = 'Dark',
    [Parameter(Mandatory)] [string]$Out,
    [string]$Exe = 'F:\charting\target-search\release\tradeterm.exe',
    [switch]$Fresh,
    [switch]$NoShots,
    [int]$Wait = 6
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
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, int x, int y, uint d, UIntPtr e);
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
if ($Fresh -and (Test-Path $tmp)) { Remove-Item -Recurse -Force $tmp }
New-Item -ItemType Directory -Force $tmp | Out-Null
New-Item -ItemType Directory -Force $Out | Out-Null
$settingsPath = Join-Path $tmp 'settings.json'
(Get-Content (Join-Path $PSScriptRoot 'settings.json') -Raw) -replace '"theme_mode": "Dark"', "`"theme_mode`": `"$Theme`"" |
    Set-Content $settingsPath -Encoding utf8NoBOM
$log = Join-Path $tmp 'app.log'
$env:TRADETERM_SETTINGS = $settingsPath
$env:RUST_LOG = 'info,tradeterm=debug,tt_session=debug'
$p = Start-Process -FilePath $Exe -PassThru -RedirectStandardOutput $log -RedirectStandardError (Join-Path $tmp 'err.log')
$t = $Theme.ToLower()

function Fg {
    $f = [W]::GetForegroundWindow()
    if ($f -ne $script:h) { throw "app window lost the foreground (fg $f pid $([W]::Pid($f)), app $($script:h) pid $($p.Id))" }
}
function Keys([string]$k) { Fg; [System.Windows.Forms.SendKeys]::SendWait($k) }
function Click([double]$x, [double]$y) {
    Fg
    [W]::SetCursorPos([int]($ox + $x * $s), [int]($oy + $y * $s)) | Out-Null
    Start-Sleep -Milliseconds 150
    [W]::mouse_event(2, 0, 0, 0, [UIntPtr]::Zero); [W]::mouse_event(4, 0, 0, 0, [UIntPtr]::Zero)
}
function Shot([string]$name) {
    Fg
    $bmp = New-Object System.Drawing.Bitmap ([int]$script:cw), ([int]$script:ch)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.CopyFromScreen($script:ox, $script:oy, 0, 0, $bmp.Size); $g.Dispose()
    $path = Join-Path $Out "$name.png"
    $bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png); $bmp.Dispose()
    "saved $path"
}
try {
    $deadline = (Get-Date).AddSeconds(30)
    while ($p.MainWindowHandle -eq 0 -and (Get-Date) -lt $deadline) {
        Start-Sleep -Milliseconds 200; $p.Refresh()
    }
    $script:h = $p.MainWindowHandle
    [W]::MoveWindow($h, 20, 0, 1400, 900, $true) | Out-Null
    [W]::Front($h)
    Start-Sleep -Milliseconds 600
    $script:s = [W]::GetDpiForWindow($h) / 96.0
    $r = New-Object W+RECT; [W]::GetClientRect($h, [ref]$r) | Out-Null
    $pt = New-Object W+POINT; [W]::ClientToScreen($h, [ref]$pt) | Out-Null
    $script:ox = $pt.X; $script:oy = $pt.Y; $script:cw = $r.Right; $script:ch = $r.Bottom
    # First search as early as possible (latency), then the screenshots.
    Keys 'BTC'
    if ($NoShots) { Start-Sleep -Seconds 60; return }
    Start-Sleep -Seconds $Wait
    Shot "search-tv-$t-btc"
    Keys '^a'; Keys 'ETH'
    Start-Sleep -Seconds 3
    Shot "search-tv-$t-eth"
    Keys '{BACKSPACE}{BACKSPACE}{BACKSPACE}'
    Start-Sleep -Seconds 2
    Shot "search-tv-$t-empty"
    Keys '{ESC}'
    Start-Sleep -Milliseconds 600
    Keys '/'
    Start-Sleep -Seconds 2
    Shot "picker-tv-$t-technicals"
    # Favorites: the dialog (840 wide, centered, a tenth of the window
    # down); the item sits 166 px below its top, 120 in.
    $w = $script:cw / $s; $hh = $script:ch / $s
    Click (($w - 840) / 2 + 120) ($hh / 10 + 166)
    Start-Sleep -Seconds 1
    Shot "picker-tv-$t-favorites"
    Keys 'mov'
    Start-Sleep -Seconds 1
    Shot "picker-tv-$t-search"
    Keys '{ESC}'
} finally {
    Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue
    Start-Sleep -Milliseconds 500
    Get-Content $log -ErrorAction SilentlyContinue | Select-String 'symbol list|symbol search' | ForEach-Object { $_.Line -replace '\x1b\[[0-9;]*m', '' }
}
