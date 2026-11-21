# Runs the release build with a seeded settings file and screenshots its
# window. Usage: shot.ps1 -Settings <json> -Out <png> [-HoverX 0.5 -HoverY 0.4]
param(
    [Parameter(Mandatory)] [string]$Settings,
    [Parameter(Mandatory)] [string]$Out,
    [double]$HoverX = -1,
    [double]$HoverY = -1,
    [int]$Wait = 9
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class W {
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  public struct RECT { public int Left, Top, Right, Bottom; }
}
"@
[W]::SetProcessDPIAware() | Out-Null
$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$tmp = Join-Path $root 'target-shots'
New-Item -ItemType Directory -Force $tmp | Out-Null
$settingsPath = Join-Path $tmp 'settings.json'
Copy-Item $Settings $settingsPath -Force
$env:TRADETERM_SETTINGS = $settingsPath
$exe = 'F:\charting\target-s56\release\tradeterm.exe'
$p = Start-Process -FilePath $exe -PassThru
try {
    $deadline = (Get-Date).AddSeconds(30)
    while ($p.MainWindowHandle -eq 0 -and (Get-Date) -lt $deadline) {
        Start-Sleep -Milliseconds 300; $p.Refresh()
    }
    $h = $p.MainWindowHandle
    [W]::SetForegroundWindow($h) | Out-Null
    Start-Sleep -Seconds $Wait
    $r = New-Object W+RECT
    [W]::GetWindowRect($h, [ref]$r) | Out-Null
    if ($HoverX -ge 0 -and [W]::GetForegroundWindow() -eq $h) {
        $x = [int]($r.Left + ($r.Right - $r.Left) * $HoverX)
        $y = [int]($r.Top + ($r.Bottom - $r.Top) * $HoverY)
        [W]::SetCursorPos($x, $y) | Out-Null
        Start-Sleep -Milliseconds 300
        [W]::SetCursorPos($x + 2, $y + 1) | Out-Null
        Start-Sleep -Milliseconds 800
    }
    $w = $r.Right - $r.Left; $hh = $r.Bottom - $r.Top
    $bmp = New-Object System.Drawing.Bitmap $w, $hh
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.CopyFromScreen($r.Left, $r.Top, 0, 0, $bmp.Size)
    $bmp.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
    $g.Dispose(); $bmp.Dispose()
    "saved $Out ${w}x${hh}"
} finally {
    Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue
}
