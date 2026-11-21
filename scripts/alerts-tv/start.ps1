# Starts the release build for the alerts screenshots with a generated
# settings file and alerts.json in scripts/alerts-tv/tmp (TRADETERM_SETTINGS;
# never the user's settings),
# with the right sidebar open on the Alerts panel. Alerts are seeded around the live BTCUSDT /
# ETHUSDT prices so some trigger right away:
#   1 BTCUSDT Greater Than (price - 1 %), Once only, toast   -> triggers
#   2 BTCUSDT Crossing (price + 0.05 %), Once per minute
#   3 BTCUSDT Entering Channel (+0.2 % .. +0.4 %), Once per bar
#   4 BTCUSDT Less Than (price - 0.5 %), stopped
#   5 ETHUSDT Less Than (price + 1 %), Once only, popup + toast (no tab:
#     evaluated on the mini-ticker feed)                      -> triggers
# and a horizontal line drawing at price - 0.25 %.
# Usage: start.ps1 -Theme Dark|Light
param(
    [ValidateSet('Dark', 'Light')] [string]$Theme = 'Dark',
    [string]$Exe = 'F:\charting\target-alerts\release\tradeterm.exe'
)
. (Join-Path $PSScriptRoot 'win.ps1')
New-Item -ItemType Directory -Force $Tmp | Out-Null
Get-ChildItem $Tmp -File | Remove-Item -Force

$btc = [double](Invoke-RestMethod 'https://api.binance.com/api/v3/ticker/price?symbol=BTCUSDT').price
$eth = [double](Invoke-RestMethod 'https://api.binance.com/api/v3/ticker/price?symbol=ETHUSDT').price
$now = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
function A($id, $sym, $op, $v, $trigger, [hashtable]$extra = @{}) {
    $a = [ordered]@{
        id = $id; symbol = $sym; interval = '1m'
        condition = [ordered]@{ op = $op; target = @{ Value = [math]::Round($v, 2) }; lower = 0.0; bars = 1 }
        trigger = $trigger; expiration = [int64]($now + 30 * 86400000)
        name = ''; message = ''; notify = @{ popup = $false; toast = $true; sound = $false }
        status = 'Active'; created_ms = [int64]($now - (10 - $id) * 60000); precision = 2
    }
    foreach ($k in $extra.Keys) { $a[$k] = $extra[$k] }
    $a
}
$a3 = A 3 'BTCUSDT' 'EnteringChannel' ($btc * 1.004) 'OncePerBar'
$a3.condition.lower = [math]::Round($btc * 1.002, 2)
$alerts = @(
    (A 1 'BTCUSDT' 'GreaterThan' ($btc * 0.99) 'OnlyOnce' @{ name = 'BTC above support' }),
    (A 2 'BTCUSDT' 'Crossing' ($btc * 1.0005) 'OncePerMinute'),
    $a3,
    (A 4 'BTCUSDT' 'LessThan' ($btc * 0.995) 'OncePerBar' @{ status = 'Stopped' }),
    (A 5 'ETHUSDT' 'LessThan' ($eth * 1.01) 'OnlyOnce' @{
        notify = @{ popup = $true; toast = $true; sound = $true }
        message = '{{ticker}} at {{close}} ({{interval}}) {{timenow}}' })
)
# Default messages as the app writes them.
foreach ($a in $alerts) {
    if ($a.message -eq '') {
        $v = '{0:N2}' -f $a.condition.target.Value
        $a.message = switch ($a.condition.op) {
            'GreaterThan' { "$($a.symbol) Greater Than $v" }
            'LessThan' { "$($a.symbol) Less Than $v" }
            'Crossing' { "$($a.symbol) Crossing $v" }
            'EnteringChannel' { "$($a.symbol) Entering Channel (Upper Bound: $v, Lower Bound: $('{0:N2}' -f $a.condition.lower))" }
        }
    }
}
@{ version = 1; next_id = 6; alerts = $alerts; log = @() } | ConvertTo-Json -Depth 10 |
    Set-Content (Join-Path $Tmp 'alerts.json') -Encoding utf8NoBOM
@{
    theme_mode = $Theme
    tabs = @(@{ symbol = 'BTCUSDT'; timeframe = '1m'; chart_type = 'Candles' })
    active_tab = 0
    right_panel = @{ open = 'Alerts'; width = 300 }
    # A horizontal line for "Add alert" on the floating toolbar.
    drawings = @{
        BTCUSDT = @(@{
            tool = 'HorizontalLine'
            points = @(@{ time_ms = [int64]($now - 1800000); price = [math]::Round($btc * 0.9975, 2) })
        })
    }
} | ConvertTo-Json -Depth 10 | Set-Content (Join-Path $Tmp 'settings.json') -Encoding utf8NoBOM

$env:TRADETERM_SETTINGS = Join-Path $Tmp 'settings.json'
$p = Start-Process -FilePath $Exe -PassThru -RedirectStandardOutput (Join-Path $Tmp 'app.log') -RedirectStandardError (Join-Path $Tmp 'err.log')
$p.Id | Set-Content (Join-Path $Tmp 'pid.txt')
$deadline = (Get-Date).AddSeconds(30)
while (@(App-Windows).Count -lt 1 -and (Get-Date) -lt $deadline) { Start-Sleep -Milliseconds 300 }
$ws = @(App-Windows)
[AW]::MoveWindow($ws[0].H, 0, 0, 1720, 900, $true) | Out-Null
[AW]::Front($ws[0].H)
"started pid $($p.Id), BTC $btc, ETH $eth, windows $($ws.Count)"
