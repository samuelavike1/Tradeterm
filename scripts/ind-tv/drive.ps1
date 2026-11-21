# Drives a release tradeterm for the indicator dialog screenshots.
#   drive.ps1 start -Theme dark|light   (fresh copy of the seed settings)
#   drive.ps1 shot -Out <png>           (the window)
#   drive.ps1 click -X <px> -Y <px> [-Double]   (window-relative)
#   drive.ps1 keys -Text "{ESC}"        (SendKeys syntax)
#   drive.ps1 stop
# Settings go to target-ind-shots/ inside this worktree (never the user's).
# Input is only sent while the app's window is in the foreground.
param(
    [Parameter(Mandatory)] [string]$Cmd,
    [string]$Theme = 'dark',
    [string]$Out = '',
    [int]$X = 0,
    [int]$Y = 0,
    [switch]$Double,
    [string]$Text = '',
    # x,y,w,h of the window image to keep (shot)
    [string]$Crop = ''
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms
Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class W {
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  [DllImport("user32.dll")] public static extern void mouse_event(int f, int x, int y, int d, int e);
  [DllImport("user32.dll")] public static extern bool MoveWindow(IntPtr h, int x, int y, int w, int hh, bool r);
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr dc, uint f);
  [DllImport("user32.dll")] public static extern IntPtr WindowFromPoint(POINT p);
  [DllImport("user32.dll")] public static extern IntPtr GetAncestor(IntPtr h, uint f);
  [DllImport("user32.dll")] public static extern void keybd_event(byte k, byte s, int f, int e);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int c);
  public struct POINT { public int X, Y; }
  public struct RECT { public int Left, Top, Right, Bottom; }
}
"@
[W]::SetProcessDPIAware() | Out-Null
$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$tmp = Join-Path $root 'target-ind-shots'
New-Item -ItemType Directory -Force $tmp | Out-Null
$pidFile = Join-Path $tmp 'pid.txt'
$exe = 'F:\charting\target-ind-tv\release\tradeterm.exe'

function Get-App {
    $id = [int](Get-Content $pidFile)
    $p = Get-Process -Id $id
    return $p
}

# Other apps (other engineers' builds) may take the foreground: retry for a
# while, never send input elsewhere.
function Focus-App($p) {
    for ($i = 0; $i -lt 40; $i++) {
        # A lone Alt tap lifts Windows' foreground lock for this process.
        [W]::keybd_event(0x12, 0, 0, 0); [W]::keybd_event(0x12, 0, 2, 0)
        [W]::ShowWindow($p.MainWindowHandle, 9) | Out-Null
        [W]::SetForegroundWindow($p.MainWindowHandle) | Out-Null
        Start-Sleep -Milliseconds 250
        if ([W]::GetForegroundWindow() -eq $p.MainWindowHandle) { return }
        Start-Sleep -Milliseconds 500
    }
    throw 'tradeterm is not in the foreground; not sending input'
}

switch ($Cmd) {
    'start' {
        $settings = Join-Path $tmp "settings-$Theme.json"
        Copy-Item (Join-Path $PSScriptRoot "$Theme.json") $settings -Force
        $env:TRADETERM_SETTINGS = $settings
        $p = Start-Process -FilePath $exe -PassThru
        $deadline = (Get-Date).AddSeconds(30)
        while ($p.MainWindowHandle -eq 0 -and (Get-Date) -lt $deadline) {
            Start-Sleep -Milliseconds 300; $p.Refresh()
        }
        $p.Id | Set-Content $pidFile
        [W]::MoveWindow($p.MainWindowHandle, 40, 40, 1400, 900, $true) | Out-Null
        Start-Sleep -Seconds 6
        "started $($p.Id)"
    }
    'shot' {
        # PrintWindow renders this window only, whatever overlaps it.
        $p = Get-App
        Start-Sleep -Milliseconds 400
        $r = New-Object W+RECT
        [W]::GetWindowRect($p.MainWindowHandle, [ref]$r) | Out-Null
        $w = $r.Right - $r.Left; $h = $r.Bottom - $r.Top
        $bmp = New-Object System.Drawing.Bitmap $w, $h
        $g = [System.Drawing.Graphics]::FromImage($bmp)
        $dc = $g.GetHdc()
        [W]::PrintWindow($p.MainWindowHandle, $dc, 2) | Out-Null
        $g.ReleaseHdc($dc)
        if ($Crop) {
            $c = $Crop.Split(',') | ForEach-Object { [int]$_ }
            $rect = New-Object System.Drawing.Rectangle $c[0], $c[1], $c[2], $c[3]
            $part = $bmp.Clone($rect, $bmp.PixelFormat)
            $bmp.Dispose(); $bmp = $part
        }
        $bmp.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
        $g.Dispose(); $bmp.Dispose()
        "saved $Out ${w}x${h} at $($r.Left),$($r.Top)"
    }
    'click' {
        $p = Get-App
        $r = New-Object W+RECT
        $pt = New-Object W+POINT
        for ($i = 0; $i -lt 20; $i++) {
            Focus-App $p
            [W]::GetWindowRect($p.MainWindowHandle, [ref]$r) | Out-Null
            [W]::SetCursorPos($r.Left + $X, $r.Top + $Y) | Out-Null
            Start-Sleep -Milliseconds 150
            $pt.X = $r.Left + $X; $pt.Y = $r.Top + $Y
            $under = [W]::GetAncestor([W]::WindowFromPoint($pt), 2)
            if ($under -eq $p.MainWindowHandle -and [W]::GetForegroundWindow() -eq $p.MainWindowHandle) { break }
            Start-Sleep -Milliseconds 500
        }
        if ($under -ne $p.MainWindowHandle -or [W]::GetForegroundWindow() -ne $p.MainWindowHandle) {
            throw "another window is under the cursor ($under vs $($p.MainWindowHandle), fg $([W]::GetForegroundWindow())); not clicking"
        }
        $n = if ($Double) { 2 } else { 1 }
        for ($i = 0; $i -lt $n; $i++) {
            [W]::mouse_event(2, 0, 0, 0, 0); [W]::mouse_event(4, 0, 0, 0, 0)
            Start-Sleep -Milliseconds 60
        }
        Start-Sleep -Milliseconds 700
        "clicked $X,$Y"
    }
    'keys' {
        $p = Get-App
        Focus-App $p
        [System.Windows.Forms.SendKeys]::SendWait($Text)
        Start-Sleep -Milliseconds 600
        "sent"
    }
    'stop' {
        $p = Get-App
        Stop-Process -Id $p.Id -Force
        "stopped"
    }
}
