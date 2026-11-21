# Drives the app started by start.ps1. Steps (run in order):
#   main                  the app window (the panel is in its right sidebar)
#   move:X,Y  click:X,Y  dclick:X,Y  drag:X,Y,DX,DY   (client px)
#   alt-a  esc  enter  sleep:MS
#   shot:NAME             window -> <Out>/alerts-NAME.png
#   both:NAME             same as shot (kept for older step lists)
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
        'move' { Move-To $w $n[0] $n[1] }
        'click' { Click-At $w $n[0] $n[1] }
        'dclick' { Click-At $w $n[0] $n[1] 2 }
        'drag' { Drag-From $w $n[0] $n[1] $n[2] $n[3] }
        'alt-a' { Press $w 0x41 -Alt }
        'esc' { Press $w 0x1B }
        'enter' { Press $w 0x0D }
        'sleep' { Start-Sleep -Milliseconds $n[0] }
        'shot' { Shot-Window $w (Join-Path $Out "alerts-$arg.png") }
        'both' { Shot-Window $ws[0] (Join-Path $Out "alerts-$arg.png") }
        default { throw "unknown step $s" }
    }
}
