# Screenshots a rectangle drawn as on TradingView (handles, axis tags and
# bands, text on the middle line) in the release build, checks the resize
# pointer over each handle and resizes it via an edge and a corner.
# Runs the exe with TRADETERM_SETTINGS pointing at a generated settings file
# in scripts/rect-tv/tmp (never the user's settings). The rectangle ("Test",
# middle line on, today's defaults) and an old-format red rectangle are
# seeded at recent BTCUSDT 1h bars (from Binance's public klines). Input is
# synthetic and only sent while the app's window is in the foreground.
#
# Usage: shot.ps1 -Theme Dark|Light -Out <dir>
# Writes <Out>/rect-<theme>-{idle,selected,hover-edge,edge-resized,corner-resized}.png
param(
    [ValidateSet('Dark', 'Light')] [string]$Theme = 'Dark',
    [Parameter(Mandatory)] [string]$Out,
    [string]$Exe = 'F:\charting\target-rect\release\tradeterm.exe'
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
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
  [DllImport("user32.dll")] public static extern IntPtr LoadCursor(IntPtr h, int id);
  [DllImport("user32.dll")] public static extern bool GetCursorInfo(ref CURSORINFO ci);
  [DllImport("kernel32.dll")] public static extern uint GetCurrentThreadId();
  public static void Front(IntPtr h) {
    IntPtr fg = GetForegroundWindow(); uint pp;
    uint ft = GetWindowThreadProcessId(fg, out pp), me = GetCurrentThreadId();
    AttachThreadInput(me, ft, true);
    ShowWindow(h, 6); ShowWindow(h, 9);
    BringWindowToTop(h); SetForegroundWindow(h);
    AttachThreadInput(me, ft, false);
  }
  public static IntPtr Cursor() {
    CURSORINFO ci = new CURSORINFO(); ci.cbSize = Marshal.SizeOf(typeof(CURSORINFO));
    GetCursorInfo(ref ci); return ci.hCursor;
  }
  public struct RECT { public int Left, Top, Right, Bottom; }
  public struct POINT { public int X, Y; }
  [StructLayout(LayoutKind.Sequential)]
  public struct CURSORINFO { public int cbSize; public int flags; public IntPtr hCursor; public POINT pt; }
}
"@
[W]::SetProcessDPIAware() | Out-Null
$tmp = Join-Path $PSScriptRoot 'tmp'
New-Item -ItemType Directory -Force $tmp | Out-Null
New-Item -ItemType Directory -Force $Out | Out-Null

# Recent bars: the rectangle spans bars -40..-22 and the upper part of the
# visible range; the old red one sits lower right.
$k = Invoke-RestMethod 'https://api.binance.com/api/v3/klines?symbol=BTCUSDT&interval=1h&limit=60'
$hi = ($k | ForEach-Object { [double]$_[2] } | Measure-Object -Maximum).Maximum
$lo = ($k | ForEach-Object { [double]$_[3] } | Measure-Object -Minimum).Minimum
$t = { param($i) [int64]$k[$k.Count + $i][0] }
$pr = { param($f) [math]::Round($lo + $f * ($hi - $lo), 2) }
$settings = @{
    theme_mode = $Theme
    tabs = @(@{ symbol = 'BTCUSDT'; timeframe = '1h'; chart_type = 'Candles' })
    active_tab = 0
    drawings = @{
        BTCUSDT = @(
            @{
                tool = 'Rectangle'
                points = @(@{ time_ms = (& $t -40); price = (& $pr 0.85) }, @{ time_ms = (& $t -22); price = (& $pr 0.55) })
                props = @{ kind = 'Shape'; text = @{ on = $true; h = 'Right'; v = 'Middle' } }
                text = 'Test'
            },
            @{
                tool = 'Rectangle'
                points = @(@{ time_ms = (& $t -14); price = (& $pr 0.25) }, @{ time_ms = (& $t -6); price = (& $pr 0.1) })
                color = @(242, 54, 69, 255); width = 2.0; style = 'Solid'
            }
        )
    }
}
$settingsPath = Join-Path $tmp 'settings.json'
$settings | ConvertTo-Json -Depth 10 | Set-Content $settingsPath -Encoding utf8NoBOM
$env:TRADETERM_SETTINGS = $settingsPath
$p = Start-Process -FilePath $Exe -PassThru -RedirectStandardOutput (Join-Path $tmp 'app.log') -RedirectStandardError (Join-Path $tmp 'err.log')
$th = $Theme.ToLower()

function Fg {
    $f = [W]::GetForegroundWindow()
    if ($f -ne $script:h) { throw "app window lost the foreground" }
}
function Move([int]$x, [int]$y) { Fg; [W]::SetCursorPos($ox + $x, $oy + $y) | Out-Null; Start-Sleep -Milliseconds 120 }
function Click([int]$x, [int]$y) {
    Move $x $y
    [W]::mouse_event(2, 0, 0, 0, [UIntPtr]::Zero); Start-Sleep -Milliseconds 60
    [W]::mouse_event(4, 0, 0, 0, [UIntPtr]::Zero); Start-Sleep -Milliseconds 300
}
function Drag([int]$x, [int]$y, [int]$dx, [int]$dy) {
    Move $x $y
    [W]::mouse_event(2, 0, 0, 0, [UIntPtr]::Zero); Start-Sleep -Milliseconds 80
    for ($i = 1; $i -le 10; $i++) { Move ($x + $dx * $i / 10) ($y + $dy * $i / 10) }
    [W]::mouse_event(4, 0, 0, 0, [UIntPtr]::Zero); Start-Sleep -Milliseconds 300
}
function Grab {
    Fg
    $bmp = New-Object System.Drawing.Bitmap ([int]$script:cw), ([int]$script:ch)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.CopyFromScreen($script:ox, $script:oy, 0, 0, $bmp.Size); $g.Dispose()
    $bmp
}
function Shot([string]$name) {
    $bmp = Grab
    $path = Join-Path $Out "rect-$th-$name.png"
    $bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png); $bmp.Dispose()
    "saved $path"
}
# The blue rectangle's bounds: bluish pixels inside the plot.
function FindRect {
    $bmp = Grab
    $minx = 1e9; $miny = 1e9; $maxx = -1; $maxy = -1
    for ($y = 90; $y -lt $script:ch - 80; $y += 2) {
        for ($x = 70; $x -lt $script:cw - 160; $x += 2) {
            $c = $bmp.GetPixel($x, $y)
            if ($c.B -gt $c.R + 40 -and $c.B -gt $c.G + 25) {
                if ($x -lt $minx) { $minx = $x }; if ($x -gt $maxx) { $maxx = $x }
                if ($y -lt $miny) { $miny = $y }; if ($y -gt $maxy) { $maxy = $y }
            }
        }
    }
    $bmp.Dispose()
    if ($maxx -lt 0) { throw 'rectangle not found' }
    @{ L = $minx; T = $miny; R = $maxx; B = $maxy }
}
$cursors = @{
    ([W]::LoadCursor([IntPtr]::Zero, 32642)) = 'nwse'
    ([W]::LoadCursor([IntPtr]::Zero, 32643)) = 'nesw'
    ([W]::LoadCursor([IntPtr]::Zero, 32644)) = 'ew'
    ([W]::LoadCursor([IntPtr]::Zero, 32645)) = 'ns'
    ([W]::LoadCursor([IntPtr]::Zero, 32649)) = 'hand'
    ([W]::LoadCursor([IntPtr]::Zero, 32512)) = 'arrow'
    ([W]::LoadCursor([IntPtr]::Zero, 32515)) = 'cross'
}
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
    Move 300 ($script:ch - 120)
    Start-Sleep -Milliseconds 500
    $b = FindRect
    "rect at $($b.L),$($b.T) - $($b.R),$($b.B)"
    Shot 'idle'
    $cx = [int](($b.L + $b.R) / 2); $cy = [int](($b.T + $b.B) / 2)
    # Select on the border (TradingView selects by the outline).
    Click $b.L ($b.T + 10)
    Move 300 ($script:ch - 120)
    Start-Sleep -Milliseconds 400
    Shot 'selected'
    $handles = [ordered]@{
        'top-left' = @($b.L, $b.T); 'top-right' = @($b.R, $b.T)
        'bottom-left' = @($b.L, $b.B); 'bottom-right' = @($b.R, $b.B)
        'left' = @($b.L, $cy); 'right' = @($b.R, $cy)
        'top' = @($cx, $b.T); 'bottom' = @($cx, $b.B)
    }
    foreach ($n in $handles.Keys) {
        $q = $handles[$n]
        Move $q[0] $q[1]
        Start-Sleep -Milliseconds 250
        $c = [W]::Cursor()
        "cursor over $n handle: $($cursors[$c] ?? $c)"
    }
    Move $b.R $cy
    Start-Sleep -Milliseconds 250
    Shot 'hover-edge'
    # Resize via the right edge, then the top-left corner.
    Drag $b.R $cy 120 40
    Move 300 ($script:ch - 120)
    Start-Sleep -Milliseconds 400
    Shot 'edge-resized'
    Drag $b.L $b.T -80 -50
    Move 300 ($script:ch - 120)
    Start-Sleep -Milliseconds 400
    Shot 'corner-resized'
}
finally {
    if (-not $p.HasExited) { $p.Kill() }
}
