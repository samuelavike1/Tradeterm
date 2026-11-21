# Drives the app started by start.ps1. Steps (run in order):
#   main | panel          target window for the next steps
#   move:X,Y  click:X,Y  dclick:X,Y  drag:X,Y,DX,DY   (client px)
#   alt-a  esc  enter  sleep:MS
#   shot:NAME             window -> <Out>/alerts-NAME.png
#   both:NAME             main + panel side by side -> <Out>/alerts-NAME.png
# Usage: act.ps1 -Out <dir> -Steps main,alt-a,shot:dialog
param(
    [Parameter(Mandatory)] [string[]]$Steps,
    [string]$Out = (Join-Path $PSScriptRoot '..\..\docs\specs\img')
)
. (Join-Path $PSScriptRoot 'win.ps1')
$ws = @(App-Windows)
$w = $ws[0]
foreach ($s in $Steps) {
    $cmd, $arg = $s -split ':', 2
    $n = if ($arg -match '^[-\d,]+$') { @($arg -split ',' | ForEach-Object { [int]$_ }) } else { @() }
    switch ($cmd) {
        'main' { $w = $ws[0] }
        'panel' { $w = $ws[1] }
        'move' { Move-To $w $n[0] $n[1] }
        'click' { Click-At $w $n[0] $n[1] }
        'dclick' { Click-At $w $n[0] $n[1] 2 }
        'drag' { Drag-From $w $n[0] $n[1] $n[2] $n[3] }
        'alt-a' { Press $w 0x41 -Alt }
        'esc' { Press $w 0x1B }
        'enter' { Press $w 0x0D }
        'sleep' { Start-Sleep -Milliseconds $n[0] }
        'shot' { Shot-Window $w (Join-Path $Out "alerts-$arg.png") }
        'both' {
            $m = $ws[0]; $p = $ws[1]
            Ensure-Fg $p; Ensure-Fg $m; Start-Sleep -Milliseconds 300
            $x = [math]::Min($m.X, $p.X); $y = [math]::Min($m.Y, $p.Y)
            $r = [math]::Max($m.X + $m.W, $p.X + $p.W); $b = [math]::Max($m.Y + $m.Hh, $p.Y + $p.Hh)
            Shot-Rect $x $y ($r - $x) ($b - $y) (Join-Path $Out "alerts-$arg.png")
        }
        default { throw "unknown step $s" }
    }
}
