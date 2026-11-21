param(
  [string]$Action = "shot",      # shot | click | dclick | rclick | move | rect | front | key | size
  [string]$Out = "",
  [int]$X = 0, [int]$Y = 0,
  [int]$W2 = 1280, [int]$H2 = 800,
  [string]$Keys = ""
)
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class W {
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(int f, int x, int y, int d, int e);
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  [DllImport("user32.dll")] public static extern bool MoveWindow(IntPtr h, int x, int y, int w, int hh, bool r);
  public struct RECT { public int L, T, R, B; }
}
"@
[W]::SetProcessDPIAware() | Out-Null
# Only this worktree's build (other engineers run their own tradeterm).
$p = Get-Process tradeterm | Where-Object { $_.MainWindowHandle -ne 0 -and $_.Path -like "*target-drawings-a*" } | Select-Object -First 1
if (-not $p) { "no window"; exit 1 }
$h = $p.MainWindowHandle
$r = New-Object W+RECT
[W]::GetWindowRect($h, [ref]$r) | Out-Null
function Front {
  if ([W]::GetForegroundWindow() -ne $h) { [W]::SetForegroundWindow($h) | Out-Null; Start-Sleep -Milliseconds 300 }
  if ([W]::GetForegroundWindow() -ne $h) { "not foreground"; exit 1 }
}
switch ($Action) {
  "rect" { "$($r.L) $($r.T) $($r.R) $($r.B)" }
  "size" { [W]::MoveWindow($h, 40, 40, $W2, $H2, $true) | Out-Null; "sized" }
  "front" { Front; "fg" }
  "move" { Front; [W]::SetCursorPos($r.L + $X, $r.T + $Y) | Out-Null; "moved" }
  "click" {
    Front
    [W]::SetCursorPos($r.L + $X, $r.T + $Y) | Out-Null; Start-Sleep -Milliseconds 80
    [W]::mouse_event(2,0,0,0,0); Start-Sleep -Milliseconds 40; [W]::mouse_event(4,0,0,0,0); "clicked"
  }
  "dclick" {
    Front
    [W]::SetCursorPos($r.L + $X, $r.T + $Y) | Out-Null; Start-Sleep -Milliseconds 80
    [W]::mouse_event(2,0,0,0,0); [W]::mouse_event(4,0,0,0,0); Start-Sleep -Milliseconds 60
    [W]::mouse_event(2,0,0,0,0); [W]::mouse_event(4,0,0,0,0); "dclicked"
  }
  "rclick" {
    Front
    [W]::SetCursorPos($r.L + $X, $r.T + $Y) | Out-Null; Start-Sleep -Milliseconds 80
    [W]::mouse_event(8,0,0,0,0); Start-Sleep -Milliseconds 40; [W]::mouse_event(16,0,0,0,0); "rclicked"
  }
  "drag" {
    # From (X, Y) by (W2, H2) in 20 steps.
    Front
    [W]::SetCursorPos($r.L + $X, $r.T + $Y) | Out-Null; Start-Sleep -Milliseconds 150
    [W]::mouse_event(2,0,0,0,0)
    for ($i = 1; $i -le 20; $i++) {
      [W]::SetCursorPos($r.L + $X + [int]($W2 * $i / 20), $r.T + $Y + [int]($H2 * $i / 20)) | Out-Null
      Start-Sleep -Milliseconds 30
    }
    [W]::mouse_event(4,0,0,0,0); "dragged"
  }
  "key" { Front; [System.Windows.Forms.SendKeys]::SendWait($Keys); "sent" }
  "shot" {
    $w = $r.R - $r.L; $hh = $r.B - $r.T
    $bmp = New-Object System.Drawing.Bitmap $w, $hh
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.CopyFromScreen($r.L, $r.T, 0, 0, $bmp.Size)
    $bmp.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
    "saved $w x $hh"
  }
}
