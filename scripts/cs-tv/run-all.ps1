# Shoots every chart Settings page in dark and light, plus the Symbol page
# for each chart type (dark), into docs/specs/img/cs-tv-*.png.
param([switch]$TypesOnly, [switch]$NoTypes)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$img = Join-Path $root 'docs\specs\img'
$tmp = Join-Path $PSScriptRoot 'tmp'
New-Item -ItemType Directory -Force $tmp | Out-Null
function Seed([string]$theme, [string]$type) {
    $f = Join-Path $tmp "seed-$theme-$type.json"
    @"
{
  "theme_mode": "$theme",
  "tabs": [ { "symbol": "BTCUSDT", "timeframe": "1h", "chart_type": "$type" } ]
}
"@ | Set-Content -Encoding utf8 $f
    $f
}
if (-not $TypesOnly) {
    foreach ($theme in @('Dark', 'Light')) {
        $t = $theme.ToLower()
        & (Join-Path $PSScriptRoot 'shot.ps1') -Settings (Seed $theme 'Candles') -Prefix (Join-Path $img "cs-tv") -Scroll
        Get-ChildItem $img -Filter 'cs-tv-*.png' | Where-Object { $_.Name -notmatch '-(dark|light)' -and $_.Name -notmatch '^cs-tv-symbol-' -and $_.Name -notmatch '^cs-tv-interval' } | ForEach-Object {
            $n = $_.BaseName
            $dest = if ($n -match '-2$') { ($n -replace '-2$', '') + "-$t-2.png" } else { "$n-$t.png" }
            Move-Item $_.FullName (Join-Path $img $dest) -Force
        }
        & (Join-Path $PSScriptRoot 'shot.ps1') -Settings (Seed $theme 'Candles') -Prefix (Join-Path $img "cs-tv-interval-$t") -Interval
    }
}
if (-not $NoTypes) {
    foreach ($type in @('Bars', 'HollowCandles', 'Line', 'Area', 'HlcArea', 'Baseline', 'Columns', 'HighLow', 'HeikinAshi', 'Renko', 'LineBreak')) {
        & (Join-Path $PSScriptRoot 'shot.ps1') -Settings (Seed 'Dark' $type) -Prefix (Join-Path $tmp "t") -Pages 0
        Move-Item (Join-Path $tmp 't-symbol.png') (Join-Path $img "cs-tv-symbol-$($type.ToLower())-dark.png") -Force
    }
}
