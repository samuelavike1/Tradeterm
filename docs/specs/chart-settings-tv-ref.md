# Chart Settings dialog: TradingView reference

Measured 2026-11-21 on tradingview.com/chart (dark theme, BTCUSD, CSS px,
`getComputedStyle` / `getBoundingClientRect` on the open dialog; viewport
1167 × 659). Screenshots: `img/tv-ref/tv-settings-*.png` (the browser pane
renders at about 0.69 scale, so they are smaller than CSS px). The dialog
was always closed with Cancel; the chart type was switched to capture each
type's Symbol page and set back to Candles / 5m afterwards.

Our implementation: `crates/tradeterm/src/chart_settings/` with the kit's
`SideNavFrame` (`crates/tt-ui/src/side_nav.rs`), comfortable density
(ui-guidelines §2.10). Our screenshots (release build, `scripts/cs-tv/shot.ps1`
with a seeded `TRADETERM_SETTINGS` file): `img/cs-tv-<page>-<theme>.png`
(`-2`: scrolled), `img/cs-tv-symbol-<type>-dark.png` per chart type,
`img/cs-tv-custom-interval-<theme>.png`.

## Frame

| Part | TradingView | Ours |
|---|---|---|
| Surface | 750 wide, radius 20, bg `rgba(19,19,19,.85)` + blur | `DialogOpts::comfortable().width(750)` |
| Header | 68 tall, "Settings" 20 / 600 at (20, 20), close 34² at right 17 | comfortable `DialogHeader` (draggable) |
| Height | fits the current page (Symbol/Candles 562, Canvas 958), capped by the window (dialog top 20, bottom 20) | `SideNavFrame` body fits the page, max = window − chrome |
| Left nav | column 236 wide, padding 8 10 0 20; items 206 × 40, radius 8, padding 0 16 0 12; icon 28² box; label 14 at x 64 | `SideNavFrame` nav (`C_W_SIDE_NAV`, `C_H_SIDE_NAV_ITEM`) |
| Nav selected | bg `#222` (dark), text white 600, icon white | `selection` bg, `text` 600 |
| Nav items | Symbol, Status line, Scales and lines, Canvas, Trading, Alerts, Events | the first four (Trading / Alerts / Events: no equivalent; session breaks live on Scales) |
| Page body | padding 16 20 16 10 (labels at x 246 = nav 236 + 10), scrolls | same |
| Section title | 11 px uppercase, `#8c8c8c`, 9 px under the body top / 25 px under the previous row; rows start 23 px under the title text | `SectionTitle` |
| Row | 50 tall (control 34 + 8 + 8) | `FormRow::comfortable` |
| Sub-row (Inputs under Titles, "Value according to scale" under Symbol) | 42 tall, indented 26 (checkbox + gap) or in the control column | `sub_row` |
| Label column | per page: widest label (+ 26 with a checkbox) + 20; a lone checkbox spans the row | measured per page |
| Controls | gap 8; swatch 33.6 × 34 radius 6 (24 swatch); line 74.6 × 34; select 100 / 150 / 180 × 34 radius 8; number field 100 or 150 × 34 with unit 8 px after | `ColorSwatchButton`, `LinePicker`, `Select::comfortable().width(..)`, `Field` |
| Footer | 66 tall, padding 16 20; Template ▾ 100 × 34 outline; Cancel (1 px white outline) / Ok (`#f2f2f2` fill) 34 tall, 16 px labels, gap 12 | `DialogFooter::comfortable`; we keep "Apply to all tabs" left, after Template |

## Symbol (per chart type)

Every type: section titled with the type name, then **DATA MODIFICATION**:
`Precision` [select 150: Default, 1, 0.1, …], `Timezone` [select 150].

| Type | Rows (TradingView order) | Screenshot |
|---|---|---|
| Candles / Hollow candles | [ ] Color bars based on previous close (lone; not on Hollow); [x] Body [up][down]; [x] Borders [up][down]; [x] Wick [up][down] | `tv-settings-symbol-candles.png` |
| Heikin Ashi | [ ] Real prices on price scale (instead of Heikin-Ashi price) (we have no equivalent); then as Candles | `tv-settings-symbol-heikin-ashi.png` |
| Bars | [ ] Color bars based on previous close; [ ] HLC bars; Up color [sw]; Down color [sw]; [x] Thin bars | `tv-settings-symbol-bars.png` |
| Line / Line with markers / Step line | Price source [150]; Line [Solid/Gradient 150][color(s)][width] (we: one `LinePicker`, no gradient) | `tv-settings-symbol-line.png` |
| Area | Price source [150]; Line [LinePicker]; Fill [top][bottom] | `tv-settings-symbol-area.png` |
| HLC area | [x] High line [LinePicker]; [x] Low line [LinePicker]; Close line [LinePicker]; Fill [high–close][close–low] | `tv-settings-symbol-hlc-area.png` |
| Baseline | Price source [150]; Top line [LinePicker]; Bottom line [LinePicker]; Fill top area [2 sw]; Fill bottom area [2 sw]; Base level [150] % | `tv-settings-symbol-baseline.png` |
| Columns | Price source [100]; [x] Color bars based on previous close; Up color; Down color | `tv-settings-symbol-columns.png` |
| High-low | [x] Body [sw]; [x] Borders [sw]; [x] Labels [sw] | – |
| Renko | Up bars / Down bars / Projection up bars / Projection down bars [fill][border]; [x] Wick [up][down]; Source [100]; Box size assignment method [150]; ATR length (or Box size / Percentage) [150] | `tv-settings-symbol-renko.png` |
| Line break | the four brick rows; Number of line [150] | – |

Volume candles needs a paid plan on TradingView (not captured); ours shows
the candle rows.

## Status line (`tv-settings-status-line*.png`)

**INSTRUMENT**: [x] Logo; [x] Title [select 150: Name / Ticker / …];
[x] Open market status; [x] Chart values; [x] Bar change values;
[ ] Volume; [ ] Last day change values; [x] Buy/sell buttons + description
(trading: not ours).
**INDICATORS**: [x] Titles; [x] Inputs (sub-row, indented 26); [x] Values;
[x] Background [opacity slider 148 × 10, transparent→color gradient,
12 px handle].

Ours also has Interval and Exchange under Title (sub-rows).

## Scales and lines (`tv-settings-scales*.png`)

**PRICE SCALE**: Currency and Unit [180: Always visible / Visible on mouse
over / Hidden]; Scale modes (A and L) [180]; [ ] Lock price to bar ratio
[number 150, disabled while off]; Scales placement [150: Auto / Left /
Right].
**PRICE LABELS & LINES**: [x] No overlapping labels; [x] Plus button (ⓘ);
[x] Countdown to bar close; Symbol [checkable select 180: Name, Value,
Line → "Value, line"][LinePicker of the price line] + sub-row [180: Value
according to scale / …]; Previous day close [select 180][LinePicker,
disabled when Hidden]; Indicators and financials [checkable 180: Name,
Value]; High and low [180][LinePicker]; Bid and ask [180][bid][ask] (ours
hidden until the feed has bid / ask).
**TIME SCALE**: [x] Day of week on labels; Date format [150]; Time hours
format [100]; [ ] Save chart left edge position when changing interval;
[ ] Allow extended zoom out (not ours).

Ours adds Average close price (after High and low) and Session breaks
(TradingView: Events tab).

## Canvas (`tv-settings-canvas*.png`)

**CHART BASIC STYLES**: Background [Solid/Gradient 150][color] (+ bottom
color for Gradient); [x] Vertical grid lines [line: color + style];
[x] Horizontal grid lines [line]; Crosshair [LinePicker]; Watermark
[select 180][color] (ours: [x] Watermark [color] with Ticker and interval /
Description sub-rows). Ours also has Pane separators [color].
**SCALES**: Text [color][size 100]; Lines [color].
**BUTTONS**: Navigation [180] (ours hidden); Pane [180].
**MARGINS**: Top [100] %; Bottom [100] %; Right [100] bars.

## Custom interval

`tv-custom-interval.png`. TradingView: 400 wide, the comfortable header
("Add custom interval", close ×), two 50 px rows with labels at x 20 and
controls at x 127: Type [select ~180: minutes / hours / days / …] and
Interval [number ~180, focused]; footer Cancel / Add (Add disabled while
empty), no Template. Ours matches: `DialogOpts::comfortable().width(400)`,
Type [`Select::comfortable` 180] then Interval [`Field` 180], footer
Cancel / Add.
