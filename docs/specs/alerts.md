# Alerts (TradingView parity)

TradingView's price alerts as local alerts: no server, they are evaluated
while tradeterm runs. Reference measured on tradingview.com/chart (2026-11,
signed-in account, CSS px at 100 %, browser viewport 1440 × 900). The
reference session only opened panels, menus and the "Create alert" dialog
and cancelled it: no alert was created, edited or deleted on the account.

Screenshots: `img/tv-ref/tv-alerts-*.png` (TradingView), `img/alerts-*.png`
(tradeterm).

Code: `tt-session/src/alerts.rs` (model, evaluator, placeholders, storage),
`tt-session/src/alert_engine.rs` (background task, feeds),
`tt-brokers` `BinanceSource::run_mini_tickers` (lightweight feed),
`tt-chart/src/alert_lines.rs` (on-chart levels, price-scale "+"),
`tradeterm/src/alerts.rs` (alert center, notifications, chart glue),
`tradeterm/src/alert_dialog.rs`, `tradeterm/src/alerts_panel.rs`.
Tokens: `AL_*` in `tt-theme/src/metrics.rs`.

## 1. Alerts panel (right sidebar)

![TV panel](img/tv-ref/tv-alerts-panel-empty.png)

| Part | TradingView | tradeterm |
|---|---|---|
| Panel | 300 wide, white / panel bg, radius 4 on the left top | `AL_W_PANEL` 300; fills what the sidebar gives it |
| Tabs | Segmented "Alerts \| Log": 268 × 34, radius 8, bg #f2f2f2, 3 px inset; segments 130 × 28 radius 5, 14 / 500; selected white (raised) | same (`AL_H_SEGMENT`, `AL_R_SEGMENT`, `AL_H_SEGMENT_BUTTON`, `AL_R_SEGMENT_BUTTON`) |
| Toolbar (Alerts) | 38 px buttons, 28 px icons: "+" Create Alert (left); Search, Sort ("Sorted by date created (newest first)"), Options "…" (right) | same order; kit `IconButton` Lg |
| Sort menu | Symbol A→Z / Z→A, Name A→Z / Z→A, Message A→Z / Z→A, Date created oldest / newest, Time triggering oldest / newest (pairs separated) | same ten, checked = current |
| Options menu | Restart all inactive, Pause all, Delete all inactive; SHOW ALERTS All / Active only / Inactive only (counts); FILTER ALERTS Current symbol, Current time interval; SHOW ALERTS BY TYPE Price / Technicals / Watchlist; CUSTOMIZE LIST Name, Message, Source icon, Instrument logo, Last triggered; Alerts settings… | Restart all inactive, Stop all, Delete all inactive; Show alerts All / Active only / Inactive only (counts); Filter: Current symbol. (No technical / watchlist alerts, no list customization: every row shows name, message, status, last triggered.) |
| Empty state | 120 px illustration, 16 px text in a 260 column "Alerts notify you instantly when your conditions are met. Create one to get started.", dark 34 × 111 radius 8 "Create alert" button | same text and button; our alarm-clock icon |
| Row | symbol, condition / name, status; hover shows actions | status dot (active green, triggered blue, stopped grey), symbol + interval, name (or condition), message, status line ("Active · Expires Dec 21, 15:41", "Stopped — Triggered · Last …"); hover: Edit, Stop / Restart, Clone, Delete. Click shows the symbol; double-click edits |
| Log toolbar | broom (clear) left, "…" right | broom (Clear log) |
| Log empty | palm-tree illustration, "No alerts triggered yet! You will see a list here when they do." | same text, alarm-clock icon |
| Log row | — (none on the account) | alarm icon, symbol, local time; alert title; message. Hover × removes the entry; click shows the symbol |

![sort](img/tv-ref/tv-alerts-sort-menu.png) ![options](img/tv-ref/tv-alerts-options-menu.png)
![log](img/tv-ref/tv-alerts-log-empty.png)

## 2. "Create alert" dialog (Alt + A)

![TV dialog](img/tv-ref/tv-alerts-create-dialog.png)

TradingView today has no Settings / Message / Notifications tabs any more:
one page with summary rows that open sub-pages ("Edit message",
"Notifications") with a back arrow and their own Cancel / Apply. tradeterm
keeps the page and its rows but shows the sub-pages as the dialog's top
tabs (Settings / Message / Notifications), which the kit's comfortable
dialog already has; the summary rows switch tabs.

| Part | TradingView | tradeterm |
|---|---|---|
| Surface | 480 wide, radius 20, 90 % white, header 70 (title 20 / 600 "Create alert on" + symbol chip with logo and caret; template icon; close) | `AL_W_DIALOG` 480, comfortable header "Create alert on BTCUSDT" / "Edit alert on …" (no symbol switcher, no template button) |
| Body | 20 px padding; label column 131 (14, muted #707070), control column 305 | `AL_W_LABEL`, `AL_W_CONTROL` |
| Condition | Source select "Price" (305 × 34, radius 8, border #ebebeb); operator select with icon "Crossing"; "Value" select 148 + number field 148 with steppers; "+ Add condition" link | Source "Price" (or the drawing: "Trend Line"…); operator select; "Value" + number field (`AL_W_HALF`). No multi-condition. |
| Operators | Crossing, Crossing Up, Crossing Down, then "Show more": Greater Than, Less Than, Entering / Exiting / Inside / Outside Channel, Moving Up / Down, Moving Up % / Down % | all thirteen (line targets: the three crossings) |
| Channels | "Upper bound" / "Lower bound" rows, each Value + field | same |
| Moving | "1.00 in 1 bar"; Interval row "Same as chart · 30 minutes" | amount "in" bars "bar(s)"; Interval select (also shown for the per-bar triggers) |
| Divider | 1 px #f2f2f2 | `border` |
| Trigger | text button + menu: Once only / Once per bar / Once per bar close / Once per minute, each with a description | select with the same four |
| Expiration | "December 21, 2026 at 15:41 ▾": Open-ended (Won't expire), End of day, 1 week, 1 month, Custom date (calendar) | select: presets with their resolved date, Custom date (a `YYYY-MM-DD HH:MM` field); the long date below. Default 1 month like TradingView |
| Message | row "BTCUSD Crossing 57,667.32 ›" opens "Edit message": Alert name field, Message textarea, "Add placeholder" menu (General: Ticker, Exchange, Interval, Time, Timenow; Symbol data: Open, Close, High, Low, Volume; Currency) | row opens the Message tab: Alert name, Message, Add placeholder (the ten General + Symbol data ones), Reset to default |
| Notifications | row "App, Toasts ›" opens: Notify in app (mobile push), Show toast notification, Send email, Webhook URL, Play sound (sound + duration), Send plain text, Notification schedule | Show popup, Show toast notification, Play sound (Windows system sound). Email / webhook / mobile need a server |
| Footer | 67 tall, Cancel (outline) / Create (dark), 72 × 34 radius 8 | kit comfortable footer, Create / Save |

Default messages (TradingView's, reproduced): `BTCUSD Crossing 57,667.32`,
`BTCUSD Entering Channel (Upper Bound: 57,667.33, Lower Bound: 57,667.32)`,
`BTCUSD, 30 Moving Up 1.00 in 1 bar`. The message follows the condition
until the user edits it.

![operators](img/tv-ref/tv-alerts-operators.png) ![channel](img/tv-ref/tv-alerts-entering-channel.png)
![moving](img/tv-ref/tv-alerts-moving-up.png) ![trigger](img/tv-ref/tv-alerts-trigger-menu.png)
![expiration](img/tv-ref/tv-alerts-expiration-menu.png) ![message](img/tv-ref/tv-alerts-message.png)
![placeholders](img/tv-ref/tv-alerts-placeholders.png) ![notifications](img/tv-ref/tv-alerts-notifications.png)

Opened from: the panel's "+" and empty-state button, the app bar's
"Alert" (alarm-clock-plus, Alt + A), the price scale's "+" (at the
crosshair price), a click on an alert line (edit), and the floating
toolbar's "Add alert" on a horizontal line / ray, trend line, ray or
extended line (between Settings and Lock, as on TradingView).

## 3. Evaluation

- Price = last trade / close. Chart tabs forward their last price after each
  data wake-up (a set lookup per tab, a channel send when the symbol has
  alerts). Symbols with alerts and no live tab get one combined Binance
  `@miniTicker` connection (about 1 update / s per symbol), restarted only
  when that set changes.
- The evaluator runs on a background task, O(1) per tick per alert: a hash
  lookup by symbol, then per alert the previous price, the current bar
  (open time, OHLC from the ticks) and a ring of the last N closes (Moving).
- Crossings compare the previous and current price like Pine's
  `ta.crossover` / `ta.crossunder` (`prev <= v < cur`), so a jump over the
  level between two ticks (gap) counts. The first tick never crosses.
- Trigger: Once only (then "Stopped — Triggered"), Once per bar (first
  qualifying tick of each bar of the alert's interval), Once per bar close
  (evaluated on bar closes only; previous close → this close), Once per
  minute (at most every 60 s while the condition holds).
- A 1 s clock closes bars without trades and expires alerts ("Stopped —
  Expired").
- Drawn lines: a horizontal line is a constant; a trend line / ray /
  extended line is linear in time between its anchors and extends as the
  drawing does. Editing the drawing updates the alert.
- Placeholders: `{{ticker}}`, `{{exchange}}` (BINANCE), `{{interval}}`
  (TradingView's: minutes as a number, else `1D` / `1W` / `1M` / `30S`),
  `{{time}}` (bar open, UTC `2019-08-27T09:56:00Z`), `{{timenow}}`,
  `{{open}}` / `{{high}}` / `{{low}}` / `{{close}}` (of the alert's bar as
  seen from the ticks), `{{volume}}` (`NaN`: the feeds carry no bar volume).
  Unknown placeholders stay as written.

## 4. On the chart

- Each alert of the chart's symbol draws a dashed line across the price
  pane in the alert color (Settings › Alerts › Alert lines, default the
  accent blue), a solid price tag on the price axis and a small alarm-clock
  tag on the plot side of the axis. Inactive alerts are faded (hidden with
  "Only active alerts"). Channels draw both bounds; sloped lines and
  Moving operators draw nothing.
- Drag a line or its tag: the level moves (rounded to the symbol's
  decimals; channels move the dragged bound; the automatic message
  follows). Click: the edit dialog.
- The price scale's "+" (Settings › Scales › Plus button, on by default)
  follows the crosshair at the right edge of the plot; a click opens
  "Create alert" at that price (TradingView opens a menu whose first row is
  "Add alert on … at …"; with only alerts here, the dialog opens directly).

## 5. Storage

`alerts.json` next to `settings.json` (`TRADETERM_SETTINGS` moves both):
`{ version, next_id, alerts: [...], log: [...] }`, written on the settings
writer thread after every change. Loading is tolerant: missing → empty;
unparseable → moved aside (`alerts.json.corrupt-<ms>`); an alert this build
cannot read (or with a duplicate id) is kept as raw JSON and written back;
unreadable log entries are dropped. The log keeps the last 500 entries.

## 6. Not done (BACKLOG.md)

Multi-condition alerts, indicator / technical and watchlist alerts, the
symbol switcher and template in the dialog header, email / webhook / mobile
push / plain-text notifications and the notification schedule, sound choice
and duration (one system sound; Settings › Alerts volume not applied),
desktop (OS) toasts, the list's "Customize list" options and "Current time
interval" filter, alerts on indicator panes, "Moving" alerts seeded from the
chart's history (they wait for N bars of live data).

## 7. tradeterm screenshots

Release build, `scripts/alerts-tv/start.ps1` (seeded settings and alerts in
`scripts/alerts-tv/tmp`, the panel in its own window via
`TRADETERM_ALERTS_PANEL=1` until the right sidebar mounts it) and
`scripts/alerts-tv/act.ps1`.

| | Dark | Light |
|---|---|---|
| Triggered alert: popup, panel, alert lines | ![](img/alerts-dark-triggered-popup.png) | ![](img/alerts-light-triggered-popup.png) |
| Chart + Alerts panel | ![](img/alerts-dark-chart-panel.png) | ![](img/alerts-light-chart-panel.png) |
| Log | ![](img/alerts-dark-log.png) | ![](img/alerts-light-log.png) |
| Create alert | ![](img/alerts-dark-dialog-settings.png) | ![](img/alerts-light-dialog-settings.png) |
| Message tab | ![](img/alerts-dark-dialog-message.png) | ![](img/alerts-light-dialog-message.png) |
| Notifications tab | ![](img/alerts-dark-dialog-notifications.png) | ![](img/alerts-light-dialog-notifications.png) |

Dark only: operators ![](img/alerts-dark-dialog-operators.png), channel
![](img/alerts-dark-dialog-channel.png), moving
![](img/alerts-dark-dialog-moving.png), a live toast
![](img/alerts-dark-toast.png), the floating toolbar's "Add alert"
![](img/alerts-dark-drawing-toolbar.png) ![](img/alerts-dark-drawing-alert-tip.png)
and its dialog ![](img/alerts-dark-drawing-alert-dialog.png). Light only:
the panel's options menu ![](img/alerts-light-options.png).
