# Screenshots the chart Settings dialog of the release build, every page.
# Runs the exe with TRADETERM_SETTINGS pointing at a copy of -Settings in
# scripts/cs-tv/tmp (never the user's settings). Input is synthetic and only
# sent while the app's window is in the foreground.
#
# Usage: shot.ps1 -Settings <json> -Prefix <out path prefix> [-Pages 0,1,2,3]
#   [-Scroll] [-Interval]
# Writes <Prefix>-<page>.png (and <Prefix>-<page>-2.png scrolled with
# -Scroll); -Interval shoots the custom interval dialog instead.
param(
    [Parameter(Mandatory)] [string]$Settings,
    [Parameter(Mandatory)] [string]$Prefix,
    [int[]]$Pages = @(0, 1, 2, 3),
    [switch]$Scroll,
    [switch]$Interval,
    [string]$Exe = 'F:\charting\target-cs-tv\release\tradeterm.exe',
    [int]$Wait = 7
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
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint x, uint y, int d, UIntPtr e);
  public struct RECT { public int Left, Top, Right, Bottom; }
  public struct POINT { public int X, Y; }
}
"@
[W]::SetProcessDPIAware() | Out-Null
$tmp = Join-Path $PSScriptRoot 'tmp'
New-Item -ItemType Directory -Force $tmp | Out-Null
$settingsPath = Join-Path $tmp 'settings.json'
Copy-Item $Settings $settingsPath -Force
$env:TRADETERM_SETTINGS = $settingsPath
$p = Start-Process -FilePath $Exe -PassThru

function Fg { if ([W]::GetForegroundWindow() -ne $script:h) { throw 'app window lost the foreground' } }
function Click([double]$lx, [double]$ly) {
    Fg
    [W]::SetCursorPos([int]($script:ox + $lx * $script:s), [int]($script:oy + $ly * $script:s)) | Out-Null
    Start-Sleep -Milliseconds 120
    [W]::mouse_event(2, 0, 0, 0, [UIntPtr]::Zero); [W]::mouse_event(4, 0, 0, 0, [UIntPtr]::Zero)
    Start-Sleep -Milliseconds 500
}
function Wheel([double]$lx, [double]$ly, [int]$ticks) {
    Fg
    [W]::SetCursorPos([int]($script:ox + $lx * $script:s), [int]($script:oy + $ly * $script:s)) | Out-Null
    for ($i = 0; $i -lt $ticks; $i++) { [W]::mouse_event(0x800, 0, 0, -120, [UIntPtr]::Zero); Start-Sleep -Milliseconds 40 }
    Start-Sleep -Milliseconds 500
}
function Shot([double]$lx, [double]$ly, [double]$lw, [double]$lh, [string]$out) {
    $x = [int]($script:ox + $lx * $script:s); $y = [int]($script:oy + $ly * $script:s)
    $w = [int]($lw * $script:s); $hh = [int]([math]::Min($lh * $script:s, $script:ch - $ly * $script:s))
    $bmp = New-Object System.Drawing.Bitmap $w, $hh
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.CopyFromScreen($x, $y, 0, 0, $bmp.Size)
    $bmp.Save($out, [System.Drawing.Imaging.ImageFormat]::Png)
    $g.Dispose(); $bmp.Dispose()
    "saved $out"
}
try {
    $deadline = (Get-Date).AddSeconds(30)
    while ($p.MainWindowHandle -eq 0 -and (Get-Date) -lt $deadline) {
        Start-Sleep -Milliseconds 300; $p.Refresh()
    }
    $script:h = $p.MainWindowHandle
    [W]::MoveWindow($h, 40, 20, 1500, 1080, $true) | Out-Null
    [W]::SetForegroundWindow($h) | Out-Null
    Start-Sleep -Seconds $Wait
    $script:s = [W]::GetDpiForWindow($h) / 96.0
    $r = New-Object W+RECT; [W]::GetClientRect($h, [ref]$r) | Out-Null
    $pt = New-Object W+POINT; [W]::ClientToScreen($h, [ref]$pt) | Out-Null
    $script:ox = $pt.X; $script:oy = $pt.Y; $script:ch = $r.Bottom
    $lw = $r.Right / $s; $lh = $r.Bottom / $s
    Fg
    [System.Windows.Forms.SendKeys]::SendWait('^k'); Start-Sleep -Milliseconds 700
    Fg
    if ($Interval) {
        [System.Windows.Forms.SendKeys]::SendWait('custom interval'); Start-Sleep -Milliseconds 500
    } else {
        [System.Windows.Forms.SendKeys]::SendWait('chart settings'); Start-Sleep -Milliseconds 500
    }
    Fg
    [System.Windows.Forms.SendKeys]::SendWait('{ENTER}'); Start-Sleep -Milliseconds 1200
    $top = $lh / 10.0
    if ($Interval) {
        $dw = 400; $dx = ($lw - $dw) / 2.0
        Shot ($dx - 8) ($top - 8) ($dw + 16) 320 "$Prefix.png"
        return
    }
    $dw = 750; $dx = ($lw - $dw) / 2.0
    $names = @('symbol', 'status-line', 'scales', 'canvas')
    foreach ($i in $Pages) {
        Click ($dx + 20 + 103) ($top + 68 + 8 + 40 * $i + 20)
        Shot ($dx - 8) ($top - 8) ($dw + 16) ($lh - $top + 8) "$Prefix-$($names[$i]).png"
        if ($Scroll) {
            Wheel ($dx + 500) ($top + 300) 15
            Shot ($dx - 8) ($top - 8) ($dw + 16) ($lh - $top + 8) "$Prefix-$($names[$i])-2.png"
        }
    }
} finally {
    Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue
}
