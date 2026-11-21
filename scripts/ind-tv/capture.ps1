# Screenshots every tab of the indicator settings dialog (RSI, MACD, BB,
# Volume, EMA) and the picker, in one theme, into docs/specs/img/.
#   capture.ps1 -Theme dark|light
param([string]$Theme = 'dark')
$ErrorActionPreference = 'Stop'
$drive = Join-Path $PSScriptRoot 'drive.ps1'
$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$img = Join-Path $root 'docs\specs\img'
# Legend rows in the seeded layout (window 1400 x 900).
$legend = [ordered]@{
    rsi = @(95, 545); macd = @(110, 697); bb = @(90, 157); volume = @(75, 112); ema = @(90, 135)
}
$crop = '440,80,520,800'
& $drive start -Theme $Theme
try {
    foreach ($name in $legend.Keys) {
        $xy = $legend[$name]
        & $drive click -X $xy[0] -Y $xy[1] -Double
        & $drive shot -Out (Join-Path $img "ind-tv-$name-inputs-$Theme.png") -Crop $crop
        & $drive click -X 618 -Y 172
        & $drive shot -Out (Join-Path $img "ind-tv-$name-style-$Theme.png") -Crop $crop
        & $drive click -X 680 -Y 172
        & $drive shot -Out (Join-Path $img "ind-tv-$name-visibility-$Theme.png") -Crop $crop
        & $drive keys -Text '{ESC}'
    }
    & $drive click -X 512 -Y 54
    & $drive shot -Out (Join-Path $img "ind-tv-picker-$Theme.png") -Crop '270,80,860,660'
    & $drive keys -Text 'relative{DOWN}'
    & $drive shot -Out (Join-Path $img "ind-tv-picker-search-$Theme.png") -Crop '270,80,860,660'
    & $drive keys -Text '{ESC}'
} finally {
    & $drive stop
}
