# Screenshots the bottom toolbar (date ranges, Go to, clock) in the release
# build: idle, after clicking range buttons, the Go to dialog (Date and
# Custom range tabs). Runs the exe with TRADETERM_SETTINGS pointing at a
# generated settings file in scripts/bottom/tmp (never the user's
# settings). Input is synthetic and only sent while the app's window is in
# the foreground.
#
# Usage: shot.ps1 -Theme Dark|Light -Out <dir> [-Ranges x1,x2,...] [-GoTo x]
#   -Ranges: client x of the 1D..All buttons (from an idle shot); without
#   it only the idle shot is taken.
param(
    [ValidateSet('Dark', 'Light')] [string]$Theme = 'Dark',
    [Parameter(Mandatory)] [string]$Out,
    [string]$Exe = 'F:\charting\target-bottom\release\tradeterm.exe',
    [int[]]$Ranges = @(),
    [int]$GoTo = 0,
    [string[]]$Keep = @('1D', '3M', 'YTD', 'All'),
    [string]$GoToDate = '2022-06-15',
    # The dialog's Custom range tab (client coordinates).
    [int]$TabX = 652,
    [int]$TabY = 172
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
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int c);
  [DllImport("user32.dll")] public static extern bool BringWindowToTop(IntPtr h);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint p);
  [DllImport("user32.dll")] public static extern bool AttachThreadInput(uint a, uint b, bool f);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, int x, int y, uint d, UIntPtr e);
  [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint f, UIntPtr e);
  [DllImport("kernel32.dll")] public static extern uint GetCurrentThreadId();
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
New-Item -ItemType Directory -Force $Out | Out-Null

$settings = @{
    theme_mode = $Theme
    tabs = @(@{ symbol = 'BTCUSDT'; timeframe = '1h'; chart_type = 'Candles' })
    active_tab = 0
}
$settingsPath = Join-Path $tmp "settings-$($Theme.ToLower()).json"
$settings | ConvertTo-Json -Depth 10 | Set-Content $settingsPath -Encoding utf8NoBOM
$env:TRADETERM_SETTINGS = $settingsPath
$p = Start-Process -FilePath $Exe -PassThru -RedirectStandardOutput (Join-Path $tmp 'app.log') -RedirectStandardError (Join-Path $tmp 'err.log')
$th = $Theme.ToLower()

function Fg {
    $f = [W]::GetForegroundWindow()
    if ($f -ne $script:h) { throw "app window lost the foreground" }
}
function MoveTo([int]$x, [int]$y) { Fg; [W]::SetCursorPos($ox + $x, $oy + $y) | Out-Null; Start-Sleep -Milliseconds 120 }
function Click([int]$x, [int]$y) {
    MoveTo $x $y
    [W]::mouse_event(2, 0, 0, 0, [UIntPtr]::Zero); Start-Sleep -Milliseconds 60
    [W]::mouse_event(4, 0, 0, 0, [UIntPtr]::Zero); Start-Sleep -Milliseconds 300
}
function Key([byte]$vk) {
    Fg
    [W]::keybd_event($vk, 0, 0, [UIntPtr]::Zero); Start-Sleep -Milliseconds 40
    [W]::keybd_event($vk, 0, 2, [UIntPtr]::Zero); Start-Sleep -Milliseconds 300
}
function Shot([string]$name) {
    Fg
    $bmp = New-Object System.Drawing.Bitmap ([int]$script:cw), ([int]$script:ch)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.CopyFromScreen($script:ox, $script:oy, 0, 0, $bmp.Size); $g.Dispose()
    $path = Join-Path $Out "bottom-$th-$name.png"
    $bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png); $bmp.Dispose()
    "saved $path"
}
$labels = @('1D', '5D', '1M', '3M', '6M', 'YTD', '1Y', '5Y', 'All')
try {
    $deadline = (Get-Date).AddSeconds(30)
    while ($p.MainWindowHandle -eq 0 -and (Get-Date) -lt $deadline) {
        Start-Sleep -Milliseconds 200; $p.Refresh()
    }
    $script:h = $p.MainWindowHandle
    [W]::MoveWindow($h, 20, 0, 1400, 900, $true) | Out-Null
    [W]::Front($h)
    Start-Sleep -Seconds 6
    $r = New-Object W+RECT; [W]::GetClientRect($h, [ref]$r) | Out-Null
    $pt = New-Object W+POINT; [W]::ClientToScreen($h, [ref]$pt) | Out-Null
    $script:ox = $pt.X; $script:oy = $pt.Y; $script:cw = $r.Right; $script:ch = $r.Bottom
    "client $($script:cw)x$($script:ch)"
    $bar = $script:ch - 19
    MoveTo 600 300
    Shot 'idle'
    for ($i = 0; $i -lt $Ranges.Count; $i++) {
        Click $Ranges[$i] $bar
        MoveTo 600 300
        Start-Sleep -Seconds 5
        if ($Keep -contains $labels[$i]) { Shot "range-$($labels[$i])" }
    }
    if ($Ranges.Count) {
        # Scroll the chart: the highlight goes.
        MoveTo 600 400
        [W]::mouse_event(2, 0, 0, 0, [UIntPtr]::Zero); Start-Sleep -Milliseconds 60
        for ($i = 1; $i -le 10; $i++) { MoveTo (600 + 15 * $i) 400 }
        [W]::mouse_event(4, 0, 0, 0, [UIntPtr]::Zero); Start-Sleep -Milliseconds 600
        Shot 'panned'
    }
    if ($GoTo) {
        MoveTo $GoTo $bar
        Start-Sleep -Milliseconds 1200
        Shot 'goto-tooltip'
        Click $GoTo $bar
        Start-Sleep -Milliseconds 800
        Shot 'goto-date'
        # Type a date (field focused), Enter: the chart centers it.
        Fg; [System.Windows.Forms.SendKeys]::SendWait('^a'); Start-Sleep -Milliseconds 200
        Fg; [System.Windows.Forms.SendKeys]::SendWait($GoToDate); Start-Sleep -Milliseconds 400
        Shot 'goto-typed'
        Fg; [System.Windows.Forms.SendKeys]::SendWait('{ENTER}')
        MoveTo 600 300
        Start-Sleep -Seconds 4
        Shot 'goto-date-applied'
        # Custom range tab (defaults to the visible span).
        Click $GoTo $bar
        Start-Sleep -Milliseconds 800
        Click $TabX $TabY
        Start-Sleep -Milliseconds 600
        Shot 'goto-custom-range'
        Key 0x1B
        Start-Sleep -Milliseconds 400
    }
}
finally {
    if (-not $p.HasExited -and -not $env:KEEP_OPEN) { $p.Kill() }
}
