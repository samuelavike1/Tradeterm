# Spec: chart settings, templates, layouts and persistence

Scope: the chart **Settings** dialog (every tab), each chart type's own style
options, chart-settings **templates**, named chart **layouts**, the **price
scale menu**, **time zone** and time formats, **watermark**, **session**
settings, and the app's general settings and persistence.

Out of scope here (other specs): drawing tools and their templates, indicator
dialogs and indicator templates, new chart-type *algorithms* beyond their
inputs (Kagi / Point & figure rendering), multi-chart grids, the trading
backend.

Goal: match TradingView (TV) closely, with every important feature fully
implemented: settings, customization and templates. Performance stays the
north star: no setting may add per-frame allocation or work when it is off.

Legend used in the tables:

- **TV default**: TradingView's default. Sources: the TradingView Advanced
  Charts `ChartPropertiesOverrides` reference (most reliable for values; the
  library's light-theme defaults), the TV help center, and observation of the
  TV web app. Values marked **(?)** were not confirmed by a source. Verify them
  in TV before treating them as final.
- **Ours**: what tradeterm should default to. "theme" means the value follows
  `tt_theme::Theme` (dark/light). It is stored as `None` so a theme switch
  recolors it.
- **Status**: `have` (done), `partial` (exists but incomplete), `missing`,
  `blocked` (needs a backend we don't have: broker, alerts, news or
  fundamentals).

---

## 1. Current state

### 1.1 What exists

| Area | Where | Notes |
|---|---|---|
| `ChartStyle`: one flat struct for all dialog options | `crates/tt-chart/src/style.rs:51-106` | `#[serde(default)]`, so partial or old files load (test at `style.rs:203-224`). |
| `ChartColors`: twelve `Option<Color32>` overrides | `style.rs:32-47` | `None` = theme color. Applied by `ChartStyle::theme()` (`style.rs:146-176`), which also recolors `volume_up/down`. |
| `ScaleButtons` (On hover / Always / Hidden) | `style.rs:6-29` | Matches TV's "Scale modes (A and L)" options. |
| Settings dialog with 4 pages | `crates/tradeterm/src/chart_settings.rs` | Pages: `Page` enum `:22-40`. Symbol `:335-360` (one "CANDLES" block for every chart type). Status line `:361-414`. Scales `:415-489`. Canvas `:490-552`. Footer with Reset / Cancel / Ok at `:573-600`. Live preview via `edit()` `:231-239`. Revert on close `:210-216`. |
| Dialog opened from the title-bar gear and the price-scale gear | `crates/tradeterm/src/app.rs:664-676`, `:848-856`; `crates/tt-chart-gpui/src/chart.rs:990-1000` | |
| One style shared by all tabs | `app.rs:638-661` (`set_chart_style` pushes it to every tab); `crates/tt-session/src/tab.rs:41-42,107` | TV keeps settings **per chart**, with "Apply to all". |
| Price-scale corner: A / L / % and the gear menu | `chart.rs:776-1011` | Gear menu: Reset price scale, Auto, Invert, the 4 modes, five label/line toggles, More settings…. |
| `ChartType` (15 types), `ScaleMode` (4 modes) | `crates/tt-chart/src/kind.rs:7-24`, `:133-141` | |
| Per-tab persistence | `crates/tt-session/src/settings.rs:86-121` (`TabSettings`) | Symbol, timeframe, chart type, scale mode, indicators, legend collapsed, invert, main collapsed. |
| App persistence: one JSON file, debounced 750 ms, atomic rename | `settings.rs:123-149` (`AppSettings`), `:209-315` (`SettingsStore`) | Theme mode, tabs, recent symbols, favorites, `chart_style`, drawings per symbol. |
| Migrations | `settings.rs:171-187` (`show_volume`), `app.rs:111-115` (old `up_color`/`down_color` → `chart_style.colors`) | Pattern: read the legacy field, never write it back. |

### 1.2 Missing or half-done (cite)

**Symbol tab / chart-type styles**

- One set of candle options drives every type. `body`, `borders`, `wick` and
  `color_by_prev_close` (`style.rs:70-73`) are read only by `candles()`
  (`draw.rs:527-569`) and `Scene::rising` (`draw.rs:298-304`, used by
  candles, HA and bars).
- Hollow candles ignore `borders` and `body`, and always color by previous
  close (`draw.rs:532-553`).
- Volume candles ignore every Symbol option and color by close vs open
  (`draw.rs:392-412`).
- Line, Line with markers and Step line: the color is hard-coded to
  `series_palette[0]` (`draw.rs:599-601`), the width to 1.5 (`:434,624`). They
  have no line style and no price source. The marker radius is hard-coded
  (`:420`).
- Area: the fill alpha (0.28/0.22) and the gradient to transparent are
  hard-coded (`draw.rs:603-613`).
- HLC area: colors come from theme up/down and alphas are hard-coded; widths
  are 1.0/1.5 (`draw.rs:438-450`).
- Baseline: the level is fixed at 50 % of the visible range (`draw.rs:456`).
  Fill alpha, line colors and the baseline color (`text_muted`) are fixed
  (`:470-490`).
- Columns: always "previous close" coloring with alpha 0.85, and no price
  source (`draw.rs:494-501`).
- High-low: one color (the line color). It has no borders and no H/L value
  labels (`draw.rs:504-510`).
- Renko: the box is fixed at ATR(14) (`derived.rs:30`, `tt-core/src/bricks.rs:110-116`).
  It has no Traditional or Percentage method, no source (close/OHLC), no wicks
  and no projection colors. Line break is fixed at 3 lines (`derived.rs:32`).
- Kagi and Point & figure don't exist (`kind.rs:7-24`, `BACKLOG.md`).
- Data modification is missing: no Precision override (precision comes from
  the symbol's tick size, `view.rs:665`), no Time zone, and no Session (not
  needed for 24/7 crypto, but the model should have it).

**Status line**: no Logo, no title format (description / ticker / both), no
open-market status, no last-day change, no indicator legend background or
opacity. The OHLC readout is at `draw.rs:1371-1416` and the legend at
`draw.rs:929-1092`.

**Scales and lines**

- There is no Currency and unit label, no Lock price to bar ratio, and no
  Scales placement (left / right / auto).
- No Plus button. Symbol label modes (name / value / line), Previous day close,
  High and low, Bid and ask and Pre/post market are missing. Indicator *name*
  labels are missing. These are listed in `BACKLOG.md` ("Price scale").
- The last-price line is always dotted, 1 px, in the candle color
  (`draw.rs:1265-1285`). TV lets you set its width and color.
- Time scale: no Date format and no 12/24-hour setting. The crosshair format
  and the 24h tick labels are hard-coded (`scale.rs:252-266`,
  `time_axis.rs:75-83`).
- The style has no "Save chart left edge position when changing intervals"
  setting.

**Canvas**

- Background is solid only.
- Grid: `dotted_grid` exists (`style.rs:66`) but has no UI. It applies to both
  directions, and TV has a style per direction.
- Pane separators have no color option.
- Crosshair: color only. The 1 px dashed 4/4 is hard-coded (`draw.rs:1316-1332`).
- No watermark. No Navigation buttons (no such overlay at all; `chart.rs`
  has only `pane_controls` `:1022`). Pane buttons have no visibility option.
- Margins exist (`style.rs:100-104`), but the Right margin applies only on
  reset (`view.rs:253`).

**Trading / Alerts / Events tabs**: none exist. Session breaks (TV puts them
on the Events tab) are missing.

**Templates**: none (`BACKLOG.md`: "Template menu"). The footer has only Reset
to defaults (`chart_settings.rs:580-586`).

**Layouts**: none. The working set of tabs is autosaved into `settings.json`,
but there are no named layouts, no Save as / Rename / Make a copy, and no
layout menu.

**Price scale menu** (`chart.rs:921-1000`) compared with TV's items:

- Missing: Lock price to bar ratio, Scale price chart only, Move scale to
  left/right.
- Labels and Lines should be two submenus, but we have a flat list of 5
  toggles.
- Missing: the Plus button entry.

**Persistence risks found**

1. A corrupt `settings.json` is ignored with a warning (`settings.rs:297-302`).
   On the next change, defaults are **written over it**, so the user's data is
   lost. Rename the bad file first.
2. Every `persist()` builds a full `AppSettings` snapshot. That includes
   cloning the whole drawings map (`app.rs:311-339`), then comparing it with
   `PartialEq` (`settings.rs:247-249`). This is O(total settings) per change
   and grows with layouts and templates. The write itself runs synchronously
   on the UI thread (`settings.rs:276-285`, `serde_json::to_vec_pretty` +
   `fs::write`).
3. There is no schema version field. Migrations are ad hoc.
4. `Color32` is serialized premultiplied by ecolor's serde. Low-alpha colors
   lose precision on a round trip. That matters once opacity is a user setting.

---

## 2. Field tables

Shared value types (defined in §3.2): `Color` = optional RGBA (`None` =
theme). `LineKind` = Solid | Dotted | Dashed (TV also has Large dashed and
Sparse dotted for drawings, but the chart settings use these three). `Width`
= 1–4 px. `PriceSource` = Open | High | Low | Close | (H+L)/2 | (H+L+C)/3 |
(O+H+L+C)/4 | (H+L+C+C)/4.

### 2.1 Symbol tab: one block per chart type

TV shows only the block for the chart's current type, then the "Data
modification" section. Each type keeps its own values, so switching type and
back keeps its colors.

#### Candles (also Heikin Ashi and Volume candles: same fields, separate instances)

| Field | Type | TV default | Ours | Status |
|---|---|---|---|---|
| Color bars based on previous close | bool | false | false | partial: one global field |
| Body (visible) | bool | true | true | partial |
| Body up / down color | Color | #089981 / #F23645 | theme up / down | partial |
| Borders (visible) | bool | true | true | partial |
| Border up / down color | Color | #089981 / #F23645 | follows body | partial |
| Wick (visible) | bool | true | true | partial |
| Wick up / down color | Color | #089981 / #F23645 | theme up_wick / down_wick | partial |

Volume candles: TV uses the same field set (`volCandlesStyle`). Today ours
ignores all of them (`draw.rs:392-412`).

#### Hollow candles

Fields are the same as Candles, without "Color bars based on previous close"
(the type defines its own coloring). TV `hollowCandleStyle` has `drawBody`,
`drawBorder` and `drawWick`, up/down/border/wick colors, all with the same
defaults as Candles.

#### Bars

| Field | Type | TV default | Ours | Status |
|---|---|---|---|---|
| Color bars based on previous close | bool | false | false | partial |
| HLC bars (don't draw the open tick) | bool | false | false | missing |
| Thin bars (1 px regardless of zoom) | bool | true | true | missing (ours scales with zoom) |
| Up color / down color | Color | #089981 / #F23645 | theme | partial |

#### Line, Line with markers, Step line (separate instances)

| Field | Type | TV default | Ours | Status |
|---|---|---|---|---|
| Price source | PriceSource | Close | Close | missing |
| Color | Color | #2962FF | theme `series_palette[0]` (#2962ff) | missing |
| Width | Width | 2 | 2 (today 1.5) | missing |
| Line style | LineKind | Solid | Solid | missing |

Line with markers: TV draws a marker on every point. The marker size isn't
exposed (?). Step line: same fields.

#### Area

| Field | Type | TV default | Ours | Status |
|---|---|---|---|---|
| Price source | PriceSource | Close | Close | missing |
| Line color | Color | #2962FF | theme palette[0] | missing |
| Line width | Width | 2 | 2 | missing |
| Line style | LineKind | Solid | Solid | missing |
| Fill top color | Color | rgba(41,98,255,0.28) | palette[0] @ 0.28 | missing |
| Fill bottom color | Color | #2962FF @ transparency 100 = fully transparent | palette[0] @ 0.0 | missing |

The library's `areaStyle` names are `color1` (top) and `color2` (bottom), and
`transparency` is 100. The web dialog shows two fill swatches (top, bottom).

#### HLC area

| Field | Type | TV default | Ours |
|---|---|---|---|
| High line: color, width, style | Color, Width, LineKind | #089981, 2, Solid | theme up, 2, Solid |
| Low line: color, width, style | | #F23645, 2, Solid | theme down, 2, Solid |
| Close line: color, width, style | | #868993, 2, Solid | theme text_muted (?) , 2, Solid |
| Fill high–close | Color | rgba(8,153,129,0.2) | up @ 0.2 |
| Fill close–low | Color | rgba(242,54,69,0.2) | down @ 0.2 |

All missing.

#### Baseline

| Field | Type | TV default | Ours | Status |
|---|---|---|---|---|
| Price source | PriceSource | Close | Close | missing |
| Base level | percent 0–100 (of the visible range, from the bottom) | 50 | 50 | partial: hard-coded |
| Baseline color | Color | #758696 | theme text_muted | missing |
| Top line color / width | Color, Width | #089981, 2 | theme up, 2 | missing |
| Bottom line color / width | Color, Width | #F23645, 2 | theme down, 2 | missing |
| Top fill 1 (near line) / 2 (near base) | Color | rgba(8,153,129,0.28) / rgba(8,153,129,0.05) | up @ .28 / .05 | missing |
| Bottom fill 1 (near base) / 2 (near line) | Color | rgba(242,54,69,0.05) / rgba(242,54,69,0.28) | down @ .05 / .28 | missing |

TV's base level percentage is measured on the visible price range (from the
bottom, according to TV's docs). Our 0.5 is the midpoint, so the direction
doesn't matter at 50. Implement it as `min + (max-min)*pct/100`.

#### Columns

| Field | Type | TV default | Ours | Status |
|---|---|---|---|---|
| Color bars based on previous close | bool | **true** | true | partial: fixed |
| Up / down color | Color | rgba(8,153,129,0.5) / rgba(242,54,69,0.5) | theme up/down @ 0.5 (today 0.85) | missing |
| Price source | PriceSource | Close | Close | missing |

#### High-low

| Field | Type | TV default | Ours | Status |
|---|---|---|---|---|
| Body color | Color | #2962FF | theme palette[0] | partial |
| Borders (visible) + color | bool, Color | true, #2962FF | true, palette[0] | missing |
| Labels (visible) + color: high/low values at each bar | bool, Color | true, #2962FF | true, palette[0] | missing |

#### Renko

Style:

| Field | Type | TV default | Ours |
|---|---|---|---|
| Up bars: color, border color | Color | #089981 / #089981 | theme up |
| Down bars: color, border color | Color | #F23645 / #F23645 | theme down |
| Projection up bars: color, border | Color | #a9dcc3 | mix(up, bg, 0.6) |
| Projection down bars: color, border | Color | #f5a6ae | mix(down, bg, 0.6) |
| Wick up / down color | Color | #089981 / #F23645 | theme |

Inputs (TV shows them in the same block):

| Field | Type | TV default | Notes |
|---|---|---|---|
| Source | Close \| OHLC | Close (?) | OHLC: highs/lows count toward brick formation and wicks |
| Box size assignment method | ATR \| Traditional \| Percentage (LTP) | ATR (?) | |
| ATR length | int ≥ 1 | 14 | ATR method only |
| Box size | price > 0 | 3 (?), in price units, rounded to the tick | Traditional only |
| Percentage | % > 0 | 1 | Percentage (LTP): percent of the last close, rounded to the tick and held constant. TV notes it repaints. |
| Wicks | bool | true (?) | A wick shows a move of more than 1 box but less than 2 against the trend. High wicks appear only on down bricks and low wicks only on up bricks. |

Status: style and inputs are missing (ATR 14 is hard-coded). Projection means
the forming brick on intraday data, drawn in the projection color.

#### Line break

| Field | Type | TV default | Ours |
|---|---|---|---|
| Up / down color, border up / down | Color | #089981 / #F23645 | theme |
| Projection up / down (fill, border) | Color | #a9dcc3 / #f5a6ae | as Renko |
| Number of lines | int 1–100 (?) | 3 | 3 (hard-coded today) |
| Source | Close | Close | Close only |

#### Kagi (type missing; spec the fields so the model is complete)

| Field | Type | TV default |
|---|---|---|
| Up (yang) line color / down (yin) line color | Color | #089981 / #F23645 |
| Projection up / down | Color | #a9dcc3 / #f5a6ae |
| Reversal method | ATR \| Traditional \| Percentage (LTP) | ATR (?) |
| ATR length | int | 14 |
| Reversal amount | price | 1 (?) |
| Percentage | % | 1 |
| Source | Close | Close (?) |

#### Point & figure (type missing)

| Field | Type | TV default |
|---|---|---|
| Up (X) / down (O) color, projection up / down | Color | #089981 / #F23645 / #a9dcc3 / #f5a6ae |
| Source | Close \| High/Low | Close (High/Low option **(?)**) |
| Box size assignment method | ATR \| Traditional \| Percentage (LTP) | ATR (?) |
| ATR length | int | 14 |
| Box size | price | 1 (?) |
| Percentage | % | 1 |
| Reversal amount | boxes, int | 3 (?) |
| One step back building | bool | false. Requires reversal 1. |

Not applicable (need tick data, see `BACKLOG.md`): Range, Volume footprint,
TPO, Session volume profile. TV `rangeStyle` / `svpStyle` exist and are listed
in the overrides reference for completeness only.

#### Data modification (section below the type block)

| Field | Type | TV default | Ours | Status |
|---|---|---|---|---|
| Session | Regular trading hours \| Extended trading hours (stocks, funds only; TV `sessionId` also has `24h`) | Regular | hidden for crypto. Store `Session::Regular`. | missing (model only) |
| Precision | Default \| 1 \| 0.1 \| … \| 0.0000000001 (10 decimals); fractional 1/2 … 1/320 for futures/bonds | Default (from the symbol) | Default | missing |
| Time zone | Exchange \| UTC \| (list, §2.6) | Exchange (for crypto, the exchange zone is UTC) | Exchange | missing |
| Adjust for dividends / contract changes (back-adjust) | bool | — | — | not applicable (crypto) |

Precision also applies to the status line values and price labels (TV help:
"price format for the price scale and status line values").

### 2.2 Status line tab

| Section | Field | Type | TV default | Ours (today) | Status |
|---|---|---|---|---|---|
| Symbol | Logo | bool | true (web) | true | missing. Needs logo source; show the base-asset badge from the tab pill (`ui::tab_pill`). |
| | Title | bool | true | true (`status_title`) | have |
| | Title format | Description \| Ticker \| Ticker and description | Description (web) (?); library `symbolTextSource` = "description" | Ticker | missing |
| | (Title) Show interval / exchange | bool, bool | true, true (library `showInterval` / `showExchange`) | we always show `symbol · timeframe · source` | missing |
| | Open market status | bool | true | true (crypto: always "open", green dot) | missing |
| | Chart values (OHLC) | bool | true (`showSeriesOHLC`) | true | have |
| | Bar change values | bool | true (`showBarChange`) | true | have |
| | Last day change values | bool | false (`showLastDayChange`) | false. Data: 24h ticker we already fetch for the tab strip. | missing |
| | Volume | bool | false (`showVolume`) | false | have |
| Indicators | Titles | bool | true | true | have |
| | Inputs (arguments) | bool | true | true | have |
| | Values | bool | true | true | have |
| | Background | bool | true | true | missing |
| | Background opacity | 0–100 % (TV slider; library `backgroundTransparency` 50) | 50 % transparency | 50 | missing |

### 2.3 Scales and lines tab

| Section | Field | Type | TV default | Ours | Status |
|---|---|---|---|---|---|
| Price scale | Currency and unit (label on the scale) | Always visible \| Visible on mouse over \| Hidden | Always visible (?) | Visible on mouse over (quote asset, e.g. "USDT") | missing |
| | Scale modes (A and L) | Visible on mouse over \| Always visible \| Hidden | Visible on mouse over | same | have (`ScaleButtons`) |
| | Lock price to bar ratio | bool + number (price units per bar) | false | false | missing |
| | Scales placement | Auto \| Left \| Right (library `priceScaleSelectionStrategyName`) | Auto | Auto (= right with a single scale) | missing |
| Price labels & lines | No overlapping labels | bool | true | true | have |
| | Plus button | bool | true | false until alerts or orders exist | blocked |
| | Countdown to bar close | bool | false in the library (`showCountdown`). The TV web default is **(?)**. | true | have |
| | Symbol (last value) | mode: Value, line \| Value \| Line \| Name, value, line \| Name, value \| Name \| Hidden (?); and value mode: "Value according to scale" \| "Price and percentage value" (library `seriesLastValueMode`) | Value, line; according to scale | Value, line | partial (two bools `last_price_label` / `last_price_line`) |
| | Symbol price line width / color | Width, Color | 1, "" (= bar color) | 1, bar color | missing |
| | Previous day close | mode (Value, line / Line / Value / Hidden) + color + width | Hidden (`showPrevClosePriceLine` false), #555555, 1 | Hidden | missing. Data: the daily candle or the 24h ticker open. |
| | Indicators and financials | mode: Value \| Name, value \| Name \| Hidden (?) | Value (`showStudyLastValue` true, `showStudyPlotLabels` false) | Value | partial (`indicator_value_labels`) |
| | High and low | mode (labels, lines, both, hidden) + color + width | Hidden | Hidden | missing. It covers the visible range high/low. |
| | Average close price | label/line bools + color + width | false / false | false | missing (library `highLowAvgPrice.averageClose*`) |
| | Bid and ask | mode + bid color + ask color + line style/width | Hidden; #2962FF / #F7525F; Dotted, 1 | Hidden | missing. Data: Binance `bookTicker` stream. |
| | Pre/post market | mode + colors | Value, line | n/a for crypto. Keep the model and hide the row. | missing |
| Time scale | Day of week on labels | bool | true | true | have |
| | Date format | see list below | "Mon 29 Sep '97" style (with weekday) | same | missing |
| | Time hours format | 24-hours \| 12-hours | 24-hours | 24-hours | missing |
| | Save chart left edge position when changing interval | bool | false (?) | false | missing |

Date format options. TV's dropdown shows a sample date. This set is from
observation of TV (?). Confirm the exact list in the app.

| Id | Example |
|---|---|
| `DayMonYy` (default) | 29 Sep '97 (with weekday: Mon 29 Sep '97) |
| `MonDdYyyy` | Sep 29, 1997 |
| `DdMonYyyy` | 29 Sep 1997 |
| `Iso` | 1997-09-29 |
| `YyMmDd` | 97-09-29 |
| `YyyySlash` | 1997/09/29 |
| `DdMmYyyyDot` | 29.09.1997 |
| `DdMmYyDot` | 29.09.97 |
| `DdMmYyyySlash` | 29/09/1997 |
| `DdMmYySlash` | 29/09/97 |
| `MmDdYyyySlash` | 09/29/1997 |
| `MmDdYySlash` | 09/29/97 |
| `DdMmYyyyDash` | 29-09-1997 |

Today's crosshair format (`scale.rs:250-266`) equals `DayMonYy` with the
weekday: `Fri 20 Nov '26  03:35`.

### 2.4 Canvas tab

| Section | Field | Type | TV default (library light) | Ours | Status |
|---|---|---|---|---|---|
| Chart basic styles | Background | Solid \| Gradient | Solid (the library says the dark theme uses a gradient) | Solid | partial |
| | Background color (solid) / gradient top + bottom | Color, Color | #ffffff | theme bg | partial (solid only) |
| | Grid lines | Vert and horz \| Vert only \| Horz only \| None | Vert and horz | Vert and horz | have (2 bools) |
| | Vertical grid color + style | Color, LineKind | rgba(42,46,57,0.06), Solid | theme grid, **Dotted** (our look) | partial (style global `dotted_grid`, no UI) |
| | Horizontal grid color + style | Color, LineKind | same | same | partial |
| | Pane separators color | Color | #EBEBEB | theme border | missing |
| | Crosshair color | Color | #9598A1 | theme crosshair | have |
| | Crosshair width | Width | 1 | 1 | missing |
| | Crosshair style | LineKind | Dashed | Dashed | missing |
| | Watermark (visible) | bool | false (?) | false | missing |
| | Watermark color (with opacity) | Color | rgba of text, low alpha (?) | text @ 0.06 | missing |
| | Watermark content | ticker, interval, description (library `IWatermarkApi`: ticker, interval, description visibility) | ticker + interval | `BTCUSDT, 1h` + description line "Bitcoin / TetherUS" | missing |
| Scales | Text color | Color | #131722 | theme text_muted | have |
| | Text size | 10 \| 11 \| 12 \| 13 \| 14 \| 16 \| 18 \| … (?) | 12 | 12.5 | have (`FONT_SIZES` `chart_settings.rs:152`) |
| | Lines color | Color | rgba(42,46,57,0) | theme border | have |
| Buttons | Navigation (zoom in/out, scroll left/right, reset) | Visible on mouse over \| Always visible \| Always invisible | Visible on mouse over | same | missing (no overlay) |
| | Pane (move up/down, collapse, maximize, remove) | same options | Visible on mouse over | same | partial (always on hover) |
| Margins | Top | % 0–… | 10 | 8 (keep, since we match TV's look today) **or** 10 to match TV | have |
| | Bottom | % | 8 | 8 | have |
| | Right | bars | 10 (?) | 5 | partial: only on reset |

The crosshair label backgrounds (`crosshairLabelBgColorDark` #363A45 /
`Light` #131722) aren't in the TV dialog. Keep them on the theme.

### 2.5 Trading, Alerts, Events tabs

Ship the tabs only when a field is usable. Model all fields now so the
templates format is stable.

**Trading.** Needs a broker; we have none. The whole tab is **blocked**.

| Field | Type | TV |
|---|---|---|
| Buy/sell buttons | bool | true |
| Instant order placement | bool | false (needs broker) |
| Play sound for executions + sound | bool, enum | |
| Notifications | Rejection only \| All events | |
| Positions: show + Profit & loss display | bool; Money \| Ticks \| Percent | |
| Reverse button | bool | |
| Orders: show | bool | |
| Brackets profit & loss: show + display | bool; Money \| Ticks \| Percent | |
| Executions: show | bool | |
| Execution labels | bool | |
| Extended price line for positions and orders | bool | |
| Orders and positions alignment | Right \| Left \| Center | Right |
| Orders / Executions / Positions on screenshots | bool × 3 | |

Library extras: `lineLength`, `lineWidth`, `extendLeft`.

**Alerts.** Needs an alerts engine. **Blocked** (the local alerts engine is a
future feature).

| Field | Type | TV |
|---|---|---|
| Alert lines: show + color | bool, Color | true |
| Only active alerts | bool | |
| Alert volume: on + level | bool, 0–100 | |
| Automatically hide toasts (20 s) | bool | |

**Events**

| Field | Type | TV | Ours |
|---|---|---|---|
| Ideas (show + filter) | bool, enum | | n/a (not a social platform) |
| Dividends, Splits, Earnings, Earnings break | bool | | n/a for crypto |
| Latest news, News notifications | bool | | blocked (no news feed) |
| **Session breaks** | bool + color + style + width | false; #4985e7; Dashed; 1 | **implement**: vertical lines at the start of each trading day in the chart's time zone, on intraday charts only |

TV also toggles Session breaks from the time-scale right-click menu.

### 2.6 Time zone menu

- **Where:** TV has a clock / zone button in the chart's bottom toolbar. It
  shows the current time and zone, e.g. `14:05:32 UTC+1`. The same setting is
  also in Settings › Symbol › Time zone. We'll put the button in the status
  bar (`app.rs:961`) next to the connection status.
- **Items:** `Exchange` and `UTC` first, then named zones, each shown with
  its offset, e.g. `(UTC-5) New York`, with a check mark on the selected one.
  TV's library list (`ui_elements/timezones`) has 98 zones: Etc/UTC, Africa
  (6), America (17), Asia (35), Atlantic (2), Australia (4), Europe (31),
  Pacific (5), US/Mountain. Use that exact list, plus `Local (OS)` as our own
  addition.
- **Effect:** It changes labels only (time axis, crosshair, countdown base,
  drawings' displayed times). Bars stay aligned as the exchange defines them.
  For Binance, days start at 00:00 UTC. TV does the same for crypto, because
  the exchange zone is UTC. Intraday resampling (`tt-core/src/resample.rs`)
  stays UTC-aligned.

### 2.7 Price scale menu (gear and right-click on the price axis)

TV's items, in order (TV library UI map, plus observation for "Reset" and
"Plus button"):

| Item | Type | Ours |
|---|---|---|
| Reset price scale (Alt+R) | action | have |
| Auto (fits data to screen) | check | have |
| Lock price to bar ratio | check | missing |
| Scale price chart only (indicators don't extend the range) | check | missing |
| Invert scale (Alt+I) | check | have |
| Regular / Percent / Indexed to 100 / Logarithmic (Alt+L) | radio | have |
| Move scale to left / right | action | missing |
| **Labels ›** Symbol name label, Symbol last value label, Indicator name labels, Indicator value labels, Bid/ask labels, High/low labels, Pre/post market labels, No overlapping labels | checks | partial (flat list) |
| **Lines ›** Symbol last price line, Previous close line, Bid/ask lines, High/low lines, Pre/post-market lines | checks | partial |
| Plus button | check | blocked |
| More settings… | action → Settings on the Scales and lines page | partial (opens the Symbol page) |

Every Labels/Lines item writes the same fields as the Scales and lines tab.
There is one source of truth.

### 2.8 Templates (chart settings)

TV's Settings dialog footer has a **Template ▾** button on the left. It
contains:

- **Save as…**: prompts for a name and saves the current dialog state as a
  named template. Saving with an existing name asks whether to overwrite.
- **Apply defaults**: resets everything to the factory defaults.
- A list of saved templates. Click one to apply it; the dialog stays open and
  the preview updates. Hover a row to reveal an × that deletes it (with a
  confirmation).

Contents: TV's Advanced Charts docs describe a chart template as the main
series' colors (candles, wicks, lines, background). In the TV web app, a
template covers the dialog's appearance settings. Decision for tradeterm: a
template stores the **whole `ChartStyle`** (all tabs, every chart type's
style and inputs), but **not** per-chart state (symbol, interval, chart type,
scale mode, indicators, drawings). Data modification fields (precision, time
zone) are included, and applying a template asks nothing. Templates are
global (available in every layout).

Our additions, also needed because tabs are independent charts:

- **Apply to all tabs** button in the footer (TV: "Apply to all" copies a
  chart's settings to every chart in the layout).
- **Save as default** in the Template menu. New tabs and new layouts start
  from the user default instead of the factory default. In TV, new charts
  take the first chart's indicators but not its settings; a user default is
  strictly more useful.
- **Apply defaults** applies the user default if one is set. A second item,
  "Factory defaults", is always present.

### 2.9 Layouts

What a layout holds (TV help: symbol, interval, chart type, indicators,
drawings, colors and settings; watchlists and alerts are *not* in layouts;
the visible time range is not saved):

- the tab list (each `TabSettings` plus that tab's `ChartStyle`)
- the active tab
- drawings for the symbols on those tabs (see the sync note below)

TV's layout menu (the layout name button on the top toolbar):

| Item | Behavior |
|---|---|
| Save layout (Ctrl+S) | Writes the current state to the current layout. If the layout is unnamed, it asks for a name. A dot or "Save" badge shows unsaved changes when autosave is off. |
| Autosave (toggle) | Default on. Changes save automatically (TV: drawings instantly, other changes after a delay). |
| Rename… | |
| Make a copy… | New layout from the current state, with "<name> copy" as the suggested name. It becomes current. |
| Create new layout… | Asks for a name, then starts from the default tab (user default style, BTCUSDT 1m + Volume) |
| Open layout… | Dialog: search box, list sorted by last modified (toggle name), star favorites, delete (with confirm, irreversible), shows symbols/intervals summary and modified time |
| Recently used | The last 5 (?) layouts, for one-click switching |
| Export chart data… | CSV of the active chart's bars. In scope here only as a menu entry; the implementation is small. |
| Copy link / Sharing | n/a (local app). Replace with **Export layout… / Import layout…** (JSON file). |

Drawing sync (TV: Off / In layout / Global): today ours behaves like "Global"
per symbol (`settings.rs:147-148`, `tab.rs:139-165`). Keep drawings global
per symbol in v1 of layouts (the layout file doesn't own drawings). Record
`drawings_sync: Global` in the layout format so "In layout" can come later
without a format break.

---

## 3. Infrastructure

### 3.1 Principles

- Every persisted struct is `#[serde(default)]`. Unknown fields are ignored.
  Removed fields are read into `#[serde(skip_serializing)]` legacy slots and
  migrated in a `normalized()` pass. This is the existing pattern
  (`settings.rs:102-104`, `:171-187`).
- Colors that should follow light/dark stay `Option` (`None` = theme). Only an
  explicit user pick stores a value.
- Rendering reads styles by reference. No per-frame cloning of `ChartStyle`.
  Today `Scene` holds `&ChartStyle`, so nested structs cost nothing.
- Settings-only toggles that are off must cost nothing at draw time:
  early-return, as `last_price_line` does (`draw.rs:1266`).

### 3.2 New value types (`crates/tt-chart/src/style/types.rs`)

```rust
/// sRGB + alpha, **unmultiplied**, serialized as "#rrggbb" or "#rrggbbaa".
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Rgba(pub [u8; 4]);           // Serialize/Deserialize via hex string; also
                                        // accepts ecolor's legacy [r,g,b,a] array.
impl Rgba { pub fn to_color32(self) -> Color32; pub fn from_color32(c: Color32) -> Self; }

/// A color slot: None = the theme token named by the field's doc comment.
pub type ColorSlot = Option<Rgba>;

pub enum LineKind { Solid, Dotted, Dashed }                    // serde: "solid" …
pub enum PriceSource { Open, High, Low, Close, Hl2, Hlc3, Ohlc4, Hlcc4 }
pub enum Visibility3 { OnHover, Always, Hidden }               // ScaleButtons becomes an alias
pub enum ButtonsVisibility { OnHover, Always, Never }
pub enum GridLines { Both, Vertical, Horizontal, None }
pub enum BackgroundKind { Solid, Gradient }
pub enum BoxMethod { Atr, Traditional, PercentLtp }
pub enum BrickSource { Close, Ohlc }
pub enum LabelMode { ValueLine, Value, Line, NameValueLine, NameValue, Name, Hidden }
pub enum LastValueMode { AccordingToScale, PriceAndPercent }
pub enum TitleFormat { Description, Ticker, TickerAndDescription }
pub enum DateFormat { DayMonYy, MonDdYyyy, DdMonYyyy, Iso, YyMmDd, YyyySlash,
                      DdMmYyyyDot, DdMmYyDot, DdMmYyyySlash, DdMmYySlash,
                      MmDdYyyySlash, MmDdYySlash, DdMmYyyyDash }
pub enum HoursFormat { H24, H12 }
pub enum Precision { Default, Decimals(u8 /*0..=10*/), Fraction(u16 /*2,4,…,320*/) }
pub enum Session { Regular, Extended, H24 }
pub enum TimeZoneSetting { Exchange, Utc, Local, Iana(String) }
pub enum ScalePlacement { Auto, Left, Right }
```

Keep `Color32` for the legacy `ChartColors` so old files round-trip during
migration. New fields use `Rgba` (fixes risk 4 in §1.2).

### 3.3 `ChartStyle` v2 (`crates/tt-chart/src/style/mod.rs`)

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ChartStyle {
    pub version: u32,                 // 2; absent in v1 files → 0
    pub symbol: SymbolStyle,
    pub status_line: StatusLineStyle,
    pub scales: ScalesStyle,
    pub canvas: CanvasStyle,
    pub trading: TradingStyle,        // model only (blocked)
    pub alerts: AlertsStyle,          // model only (blocked)
    pub events: EventsStyle,          // session breaks implemented
    // Runtime-only layout constants, as today (style.rs:56-64).
    #[serde(skip)] pub time_axis_height: f32,
    #[serde(skip)] pub price_label_spacing: f32,
    #[serde(skip)] pub time_label_spacing: f32,
    /// v1 flat fields; read once, migrated, never written.
    #[serde(flatten, skip_serializing)]
    legacy: LegacyV1,
}

pub struct SymbolStyle {
    pub candles: CandleStyle, pub hollow_candles: CandleStyle,
    pub volume_candles: CandleStyle, pub heikin_ashi: CandleStyle,
    pub bars: BarStyle,
    pub line: LineSeriesStyle, pub line_with_markers: LineSeriesStyle, pub step_line: LineSeriesStyle,
    pub area: AreaStyle, pub hlc_area: HlcAreaStyle, pub baseline: BaselineStyle,
    pub columns: ColumnStyle, pub high_low: HighLowStyle,
    pub renko: RenkoStyle, pub line_break: LineBreakStyle,
    pub kagi: KagiStyle, pub pnf: PnfStyle,
    pub data: DataModification,      // session, precision, time_zone
}
pub struct CandleStyle { pub color_by_prev_close: bool,
    pub body: bool, pub body_up: ColorSlot, pub body_down: ColorSlot,
    pub borders: bool, pub border_up: ColorSlot, pub border_down: ColorSlot,   // None = body color
    pub wick: bool, pub wick_up: ColorSlot, pub wick_down: ColorSlot }
pub struct BarStyle { pub color_by_prev_close: bool, pub hlc: bool, pub thin: bool,
    pub up: ColorSlot, pub down: ColorSlot }
pub struct LineSeriesStyle { pub source: PriceSource, pub color: ColorSlot, pub width: f32, pub kind: LineKind }
pub struct AreaStyle { pub source: PriceSource, pub line: ColorSlot, pub width: f32, pub kind: LineKind,
    pub fill_top: ColorSlot, pub fill_bottom: ColorSlot }
pub struct HlcAreaStyle { pub high: StrokeStyle, pub low: StrokeStyle, pub close: StrokeStyle,
    pub fill_high_close: ColorSlot, pub fill_close_low: ColorSlot }
pub struct BaselineStyle { pub source: PriceSource, pub level_pct: f32, pub base_color: ColorSlot,
    pub top_line: ColorSlot, pub top_width: f32, pub bottom_line: ColorSlot, pub bottom_width: f32,
    pub top_fill_1: ColorSlot, pub top_fill_2: ColorSlot, pub bottom_fill_1: ColorSlot, pub bottom_fill_2: ColorSlot }
pub struct ColumnStyle { pub source: PriceSource, pub color_by_prev_close: bool, pub up: ColorSlot, pub down: ColorSlot }
pub struct HighLowStyle { pub body: ColorSlot, pub borders: bool, pub border: ColorSlot,
    pub labels: bool, pub label: ColorSlot }
pub struct BrickColors { pub up: ColorSlot, pub down: ColorSlot, pub border_up: ColorSlot, pub border_down: ColorSlot,
    pub proj_up: ColorSlot, pub proj_down: ColorSlot, pub proj_border_up: ColorSlot, pub proj_border_down: ColorSlot }
pub struct RenkoStyle { pub colors: BrickColors, pub wick_up: ColorSlot, pub wick_down: ColorSlot,
    pub inputs: RenkoInputs }
pub struct RenkoInputs { pub source: BrickSource, pub method: BoxMethod, pub atr_length: u32,
    pub box_size: f64, pub percent: f64, pub wicks: bool }
pub struct LineBreakStyle { pub colors: BrickColors, pub lines: u32 }
pub struct KagiStyle { … }  pub struct PnfStyle { … }   // per §2.1
```

Similarly: `StatusLineStyle` (§2.2), `ScalesStyle` (§2.3: `currency_label:
Visibility3`, `scale_buttons: Visibility3`, `lock_ratio: Option<f64>`,
`placement`, `no_overlapping_labels`, `plus_button`, `countdown`,
`symbol_label: LabelMode`, `symbol_value_mode`, `price_line_width`,
`price_line_color`, `prev_close: LineLabel`, `indicators: LabelMode`,
`high_low: LineLabel`, `avg_close: LineLabel`, `bid_ask: BidAskStyle`,
`pre_post: LineLabel`, `day_of_week`, `date_format`, `hours_format`,
`keep_left_edge`), and `CanvasStyle` (§2.4). The `font_size` / margins
fields move into `scales.text_size` and `canvas.margins`.

**Chart-type lookup** (so draw code stays simple):

```rust
impl SymbolStyle {
    pub fn candle(&self, t: ChartType) -> &CandleStyle;      // Candles | Hollow | Volume | HA
    pub fn line(&self, t: ChartType) -> &LineSeriesStyle;    // Line | LineWithMarkers | StepLine
}
impl ChartStyle {
    /// Resolves a slot against the theme: slot.map(to_color32).unwrap_or(token).
    pub fn resolve(&self, slot: ColorSlot, token: Color32) -> Color32;
}
```

**Theme interaction.** `ChartStyle::theme()` (`style.rs:146-176`) keeps
mapping `symbol.candles.body_up/down` onto `theme.up/down` (and through them
onto `volume_up/down`). The tab strip, status bar and volume default colors
keep following the candle colors, as `style_overrides` does today
(`app.rs:1151-1156`).

**v1 → v2 migration** (`ChartStyle::migrate(&mut self)`, called from
`AppSettings::normalized`). It runs when `version == 0` and a legacy key is
present:

| v1 field (`style.rs`) | v2 destination |
|---|---|
| `font_size` | `scales.text_size` |
| `dotted_grid` | `canvas.grid_v_kind` and `canvas.grid_h_kind` = Dotted / Solid |
| `color_by_prev_close` | `symbol.candles`, `.heikin_ashi`, `.bars` `.color_by_prev_close` (v1 applied it to those three via `Scene::rising`) |
| `body` / `borders` / `wick` | `symbol.candles` and `.heikin_ashi` (v1 drew HA with `candles()`); `hollow_candles.wick` = `wick` |
| `status_title`, `status_values`, `status_change`, `status_volume`, `legend_*` | `status_line.*` |
| `scale_buttons`, `no_overlapping_labels`, `countdown` | `scales.*` |
| `last_price_label` + `last_price_line` | `scales.symbol_label` = (label, line) → ValueLine / Value / Line / Hidden |
| `indicator_value_labels` | `scales.indicators` = Value / Hidden |
| `day_of_week` | `scales.day_of_week` |
| `grid_vertical` + `grid_horizontal` | `canvas.grid_lines` |
| `margin_top/bottom/right` | `canvas.margins.*` |
| `colors.up_body/down_body` | the `up`/`down` slots of **every** type whose v1 color came from `theme.up/down` (candles, hollow, volume candles, HA, bars, columns, HLC area, baseline lines, bricks), to preserve v1 visuals |
| `colors.up_border/down_border`, `up_wick/down_wick` | candles + HA + hollow + volume candles |
| `colors.background`, `grid_vertical`, `grid_horizontal`, `crosshair`, `scale_text`, `scale_lines` | `canvas.*` |

Then set `version = 2`. Tests: a v1 JSON fixture (copied from a real
`settings.json`) loads, migrates, re-serializes without legacy keys, and
renders identical colors (`ChartStyle::theme` equality and per-type
`resolve` equality).

Downgrade compatibility: an older binary reading a v2 file sees none of the
flat keys and gets defaults for the chart style (everything else still loads).
This is acceptable. Mention it in release notes.

### 3.4 Per-chart-type rendering contract

- `draw.rs` `series()` (`:365-389`) gets the type's style struct via
  `s.style.symbol.candle(t)` and so on. No new allocation.
- **Price source**: `bars.rs` builds `DrawBar` with `close`. Add `value: f64`
  = the chosen source for line-like types. The LOD pyramid (`lod.rs`) is built
  on closes. When the source isn't Close, build it from a derived "source
  series" cached in `Derived` (`derived.rs`), keyed by `(source,
  src.version(), len, last)`. That is the same invalidation as bricks, so the
  live tick stays O(1). `ChartType::uses_close_only()` (`kind.rs:113-123`)
  becomes "uses source only".
- **Line styles**: `Canvas` has `dashed_line` and `dotted_line` for segments
  only (`render/mod.rs:27,33`). Add `fn dashed_polyline(&mut self, pts,
  width, color, dash, gap)` with a default implementation that splits into
  segments, and a fast path in `tt-chart-gpui/src/canvas.rs` that emits one
  merged triangle path (same as `polyline`).
- **Bricks inputs**: `Derived::sync(kind, inputs: &BrickInputs, src)`. The
  cache key includes the inputs, so a changed setting rebuilds once.
  `tt-core/src/bricks.rs` gets `renko_with(src, RenkoParams{method,
  atr_length, box_size, percent, source, wicks})`. It returns the bricks plus
  per-brick wick extents, and the projection brick when the last source bar
  hasn't completed a brick. Traditional box sizes are rounded to the tick
  (`SymbolInfo` tick size). Percentage (LTP) uses the last close, rounded to
  the tick and held constant until the source series version changes.
- **Gradient background**: `fill_area` already does vertical two-color
  gradients (`render/mod.rs:41`). Reuse it for the plot background.
- **Watermark**: draw in `draw_base` before the grid. It's two centered text
  runs (ticker + interval at about 6× font size, description at about 2×),
  costing nothing when disabled.

### 3.5 Per-tab style and "Apply to all tabs"

- `TabSettings` gains `style: Option<ChartStyle>` (`settings.rs:86-105`).
  `None` = use `AppSettings.chart_style` (the user default). Old files have
  no `style`, so every tab follows the global style, which is exactly today's
  behavior.
- The Settings dialog edits the **active tab's** effective style.
  `ChartSettingsPanel::new` keeps today's signature, plus the active
  `ChartType` so the Symbol page shows the right block.
  `TradeTerm::set_chart_style` (`app.rs:638-661`) becomes
  `set_tab_style(tab, style, persist)` and only touches that tab, which is
  cheaper than today.
- Footer **Apply to all tabs** sets every tab's `style = None` and the global
  style to the draft. **Save as default** only sets the global style.
- `Shared.chart_style` (`tab.rs:41-42`) stays the style for new tabs.

### 3.6 Templates storage (`crates/tt-session/src/templates.rs`)

- File: `<config dir>/chart-templates.json`, next to `settings.json` (same
  `ProjectDirs`). It's separate so that `settings.json` diffs and saves stay
  small.

```rust
#[serde(default)]
pub struct ChartTemplates {
    pub version: u32,                         // 1
    pub templates: Vec<ChartTemplate>,        // sorted by name, case-insensitive
}
pub struct ChartTemplate { pub name: String, pub style: ChartStyle, pub modified_ms: i64 }
```

- The user default is not in this file. It is `AppSettings.chart_style`
  (§3.5).
- Writes are immediate and atomic (same temp + rename helper, factored out of
  `settings.rs:307-315` into `tt-session/src/fsutil.rs`). Template edits are
  rare, so no debounce is needed.
- API: `list()`, `save(name, style)` (overwrites the same name after the UI
  confirms), `delete(name)`, `get(name)`. Name rules: trimmed, 1–64 chars,
  unique case-insensitively.
- Templates go through `ChartStyle::migrate()` on load, so they survive
  future format changes.

### 3.7 Layouts storage (`crates/tt-session/src/layouts.rs`)

Design: **the working state stays in `settings.json`**. That means `tabs`,
`active_tab` and drawings, as today, so crash recovery and the current
autosave stay intact. Named layouts are snapshot files.

```
<config dir>/layouts/<id>.json      id = 16 hex chars (random), never reused
<config dir>/layouts/index.json     [{id, name, modified_ms, favorite, summary}] (rebuildable from files)
```

```rust
#[serde(default)]
pub struct Layout {
    pub version: u32,                   // 1
    pub id: String,
    pub name: String,
    pub tabs: Vec<TabSettings>,         // each with its optional style (§3.5)
    pub active_tab: usize,
    pub drawings_sync: DrawingsSync,    // Global (only value implemented)
    pub modified_ms: i64,
}
pub struct LayoutSummary { pub symbols: Vec<String>, pub intervals: Vec<Timeframe> } // for the Open dialog
```

`AppSettings` gains:

| Field | Type | Default | Notes |
|---|---|---|---|
| `schema_version` | u32 | 2 | For future migrations |
| `current_layout` | Option<String> (id) | None | None = "Unnamed" |
| `layout_autosave` | bool | true | TV's default is on |
| `recent_layouts` | Vec<String> | [] | Ids, most recent first, max 10 |
| `layout_dirty` | bool | false | Unsaved changes in the current layout (autosave off). Restores the "Save" badge after a restart. |

Flows:

- **Autosave on**: each `persist()` of the working state also schedules a
  debounced write of the current layout file. It uses the same 750 ms
  debounce and goes through a second `DebouncedFile<Layout>`.
- **Switch layout** (Open / Recent): flush, then replace the tabs with the
  layout's tabs. Rebuild the charts with the same code path as startup
  (`app.rs:212-216`). Live streams restart; series caches per tab are lost,
  which is fine. Then set `current_layout`.
- **Make a copy**: new id, name "<name> copy", the current state, now
  current. **Rename**: index plus file. **Delete**: only from the Open dialog
  and never the current layout (TV: deleting can't be undone; confirm).
- **Create new layout**: one default tab with the user default style.
- **Export / Import**: write or read a `Layout` JSON file chosen with a file
  dialog. Import gets a new id and handles name collisions by adding " (2)".
- **Migration**: none needed. The first run with layouts has
  `current_layout = None`, and the working state is the "Unnamed" layout. The
  first Save asks for a name.

### 3.8 Time zone and time formatting

Today `DateTime::from_unix_ms` is UTC only (`scale.rs:197-234`), and every
label path goes through it: `time_axis.rs:36-83` (`boundary_weight`,
`tick_label`), `scale.rs:252` (`format_full_time`), drawings measurer
durations and dates.

Design:

- New `crates/tt-chart/src/tz.rs`:
  ```rust
  /// Offset table for one zone over a range: sorted (utc_ms_from, offset_ms).
  #[derive(Clone)] pub struct TzOffsets { transitions: Arc<[(i64, i32)]> }
  impl TzOffsets {
      pub fn utc() -> Self;
      pub fn offset_ms(&self, utc_ms: i64) -> i64;   // binary search; O(log n), n ~ 2/yr
      pub fn local_ms(&self, utc_ms: i64) -> i64 { utc_ms + self.offset_ms(utc_ms) }
  }
  ```
  `tt-chart` stays free of a tz database. The app (or `tt-session`) builds a
  `TzOffsets` for a zone and a year range with **`jiff`** (bundled tzdb on
  Windows, `jiff` with `tzdb-bundle-platform`), or with `chrono-tz` as the
  alternative. Rebuild only when the zone changes or the visible range
  crosses outside the table.
- `Scene` gets `tz: &TzOffsets` and a `TimeFormat { date: DateFormat, hours:
  HoursFormat, day_of_week: bool }`. `boundary_weight(prev, ms)` and
  `tick_label` take local ms (`tz.local_ms(..)`). The hour and minute
  divisibility checks (`time_axis.rs:58-71`) then also work on local time,
  so 30-minute offset zones (India, Kathmandu at +5:45) label correctly.
- Performance: one binary search per examined candidate (the tick pass
  already strides: `draw.rs:1118`). For the UTC fast path, `offset_ms`
  returns 0 without a search when the table has a single entry.
- Format functions: `scale::format_date(dt, DateFormat, weekday)`,
  `scale::format_time(dt, HoursFormat)` (12h → `3:35 PM`, ticks `3 PM` /
  `3:30 PM`). `format_full_time` composes them. Countdown stays a duration
  and is unaffected.
- "Exchange" resolves through `SymbolInfo` (add `timezone: Option<String>`,
  which is UTC for Binance). "Local" resolves via jiff's system zone.
- Status-bar clock: a one-second tick already exists (`app.rs:160-180`).
  Render the clock in the status bar only, never by repainting the chart.
- Session breaks (Events): in `draw_base`, use the time-axis candidates
  already computed with `TickWeight::Day` or higher, in local time. Draw them
  with `events.session_breaks` style, on intraday intervals only.

### 3.9 General app settings and persistence hardening (`tt-session/src/settings.rs`)

1. **Corrupt file backup**: when `read_settings` fails to parse, rename the
   file to `settings.json.corrupt-<unix-ms>` before falling back. Log it, and
   show a status-bar notice once.
2. **Schema version**: `schema_version` (above). `normalized()` runs the
   migrations in order: show_volume → old up/down colors (move the code from
   `app.rs:111-115` into `normalized`) → `ChartStyle::migrate()` for the
   global style and every tab style.
3. **Background writes**: `SettingsStore::save_now` serializes on the UI
   thread (cheap) and hands the bytes to the background executor for
   `write + rename`. A sequence number drops out-of-order completions.
4. **Cheaper change detection**: replace "snapshot everything + `PartialEq`"
   with a generation counter. `persist()` bumps it, and the store debounces
   on the counter. Only build the snapshot when the timer fires. Drawings are
   already tracked with `take_dirty` (`tab.rs:145`).
5. **Generic debounced file**: `DebouncedFile<T: Serialize>` reused for
   `settings.json` and the current layout file. Templates write immediately.
6. **Preferences** that are app-wide (not chart settings) stay in
   `AppSettings`: theme mode, favorites, recents, interval sections, layout
   bookkeeping, the user default chart style. A future "Preferences" dialog
   (language, hotkeys, reduce motion override) is out of scope here.

---

## 4. Implementation plan

Slices are ordered by priority. Each lists its files. A slice touching a file
another slice owns says which part. **S0 must land first**, because it fixes
the data model and does the mechanical file splits that keep later slices
from colliding.

| # | Slice | Priority | Depends on | Files (ownership) |
|---|---|---|---|---|
| **S0** | **Style model v2 and file splits.** New value types (§3.2), `ChartStyle` v2 with all fields from §2 (behavior unchanged: renderers read the new paths with the same effective values), v1 migration plus fixture tests, `resolve()`, `candle(t)` / `line(t)` lookups. Mechanical moves: `draw.rs` series functions (`:365-657`) → `draw/series.rs`, readout and legends (`:929-1092`, `:1371-1416`) → `draw/legend.rs`, the rest stays in `draw/mod.rs`. `chart_settings.rs` → `chart_settings/mod.rs` plus one file per page (same content). | P0 | — | `tt-chart/src/style.rs` → `style/{mod,types,series,legacy}.rs`; `tt-chart/src/draw.rs` → `draw/{mod,series,legend}.rs`; `tt-chart/src/lib.rs`; `tradeterm/src/chart_settings.rs` → `chart_settings/{mod,symbol,status_line,scales,canvas}.rs`; `tt-session/src/settings.rs` (call `migrate`) |
| **S1** | **Chart-type styles in the renderer.** Every type reads its own struct: candles, hollow, volume candles and HA (body/border/wick toggles and colors), bars (HLC, thin), line family (color, width, kind), area (fills), HLC area, baseline (level %, colors, fills), columns, high-low (borders, H/L labels). Adds `dashed_polyline` to `Canvas`. | P0 | S0 | `tt-chart/src/draw/series.rs`; `tt-chart/src/render/mod.rs`; `tt-chart-gpui/src/canvas.rs` (new method only) |
| **S2** | **Price source.** `DrawBar.value`, source-series cache in `Derived`, LOD built on the source, `uses_close_only` → source-aware fitting. | P1 | S0 | `tt-chart/src/bars.rs`, `derived.rs` (source part), `lod.rs`, `kind.rs`, `view.rs` (`price_extent` call `:750` only) |
| **S3** | **Brick inputs.** Renko method ATR / Traditional / Percentage, source Close/OHLC, wicks, projection brick; Line break number of lines; cache keyed on inputs; tests in `tt-core`. | P1 | S0 | `tt-core/src/bricks.rs`; `tt-chart/src/derived.rs` (bricks part: coordinate with S2, which touches `sync()`; land S3 after S2 or agree on the signature `sync(kind, &SeriesInputs, src)` in S0); `draw/series.rs` `bricks()` only |
| **S4** | **Settings dialog UI.** Symbol page shows the active chart type's block with all fields (color pickers with opacity, width and style dropdowns, numeric inputs), plus Data modification (Precision, Time zone; Session hidden for crypto). Status line, Scales and lines and Canvas pages show every field in §2.2–2.4. Only fields whose renderer exists are shown (no "coming soon" rows); each renderer slice adds its own rows to the page file it touches. "More settings…" opens the Scales page. Dialog receives the active chart type. | P0 | S0 | `tradeterm/src/chart_settings/*`; `tradeterm/src/app.rs` `open_chart_settings` (`:664-676`) only; `tt-chart-gpui/src/chart.rs` `OpenSettings` event payload (page) only |
| **S5** | **Scales and lines rendering.** Symbol label modes and value mode, price line width/color, previous day close, high/low (visible range) lines and labels, average close, indicator name labels, currency/unit label, keep-left-edge on interval change. Bid/ask needs a `bookTicker` stream: separate sub-task in `tt-brokers`. | P1 | S0 | `tt-chart/src/draw/mod.rs` (tags and lines: `last_price_*` `:1265-1307`, tag stacking); `tt-chart/src/view.rs` (keep-left-edge only); `tt-session/src/tab.rs` (prev-day-close data); `tt-brokers/src/binance/*` (bid/ask, optional) |
| **S6** | **Canvas rendering.** Gradient background, per-direction grid style, pane separator color, crosshair width and style, watermark, Right margin applied live, Navigation buttons overlay (zoom in/out, scroll, reset) with visibility, pane button visibility. | P1 | S0 | `tt-chart/src/draw/mod.rs` (grid `:142-170`, crosshair `:1310-1333`, background, watermark); `tt-chart/src/view.rs` (right margin); `tt-chart-gpui/src/chart.rs` (new `nav_buttons()`, `pane_controls` visibility `:1022+`) |
| **S7** | **Price scale menu completion and scale placement.** Labels › / Lines › submenus, Scale price chart only, Lock price to bar ratio, Move scale to left/right (layout in `view.rs`: price axis on the left, hit testing, corner controls position), Alt+R / Alt+I / Alt+L hotkeys. | P1 | S0 (and S5 for the label items it toggles: toggles work on fields; rendering comes with S5) | `tt-chart-gpui/src/chart.rs` `corner_controls` (`:776-1011`); `tt-chart/src/view.rs` (`layout`, `hit_test`, `drag_price_axis`, auto-fit when scale-series-only); `tradeterm/src/app.rs` `bind_keys` (`:43`) |
| **S8** | **Status line.** Title format, interval/exchange toggles, open-market status dot, last-day change (24h ticker), logo badge, indicator legend background and opacity, precision override in readout. | P1 | S0 | `tt-chart/src/draw/legend.rs`; `tt-chart/src/view.rs` `RenderInput` (pass description and 24h change) only; `tt-session/src/tab.rs` (expose 24h change, already used by the tab strip) |
| **S9** | **Time zone, date and hours formats, session breaks.** `tz.rs`, `TzOffsets` built with jiff in `tt-session`, time-axis and crosshair through local time, `DateFormat` / `HoursFormat` formatting, status-bar time zone button with the TV zone list plus clock, session-break lines. | P1 | S0 | `tt-chart/src/{tz.rs, scale.rs, time_axis.rs}`; `tt-chart/src/draw/mod.rs` (`time_ticks` `:1114-1133`, crosshair label `:1346-1365`, session breaks); `tt-chart/src/drawings.rs` (measurer date text only); `tt-session/src/tab.rs` (build `TzOffsets`); `tt-core/src/symbol_info.rs` (`timezone`); `tradeterm/src/app.rs` `status_bar` (`:961+`) |
| **S10** | **Persistence hardening.** Corrupt-file backup, `schema_version`, migrations moved into `normalized`, background writes, generation-counter debounce, `fsutil` atomic write, `DebouncedFile<T>`. | P0 | — (independent of S0; coordinate the `normalized()` edit) | `tt-session/src/settings.rs`; new `tt-session/src/fsutil.rs`; `tradeterm/src/app.rs` `persist` / `settings_snapshot` (`:311-371`) and the migration at `:111-115` |
| **S11** | **Per-tab style and chart-settings templates.** `TabSettings.style`, `set_tab_style`, Apply to all tabs, Save as default; `templates.rs` store; footer **Template ▾** menu (Save as… dialog with name and overwrite confirm, Apply defaults, Factory defaults, list with apply and delete). | P1 | S0, S4 (footer), S10 (fsutil) | `tt-session/src/{settings.rs (TabSettings.style only), tab.rs, templates.rs}`; `tradeterm/src/chart_settings/{mod.rs footer, template_menu.rs}`; `tradeterm/src/app.rs` (`set_chart_style` → `set_tab_style`, `:638-661`) |
| **S12** | **Layouts.** `layouts.rs` store and index, AppSettings layout fields, layout menu in the title bar (Save Ctrl+S, Autosave, Rename, Make a copy, Create new, Open layout… dialog, Recent, Export chart data, Export/Import layout), unsaved badge, switch-layout rebuild. | P1 | S10, S11 (tab style in the layout format) | `tt-session/src/layouts.rs`; `tt-session/src/settings.rs` (new AppSettings fields only); `tradeterm/src/app.rs` (title bar `:772-872`, switching); new `tradeterm/src/layouts_ui.rs` |
| **S13** | **Kagi and Point & figure.** Algorithms in `tt-core`, styles and inputs from S0, renderers, menu entries. Coordinate with the chart-types spec if a separate owner exists. | P2 | S0, S3 | `tt-core/src/bricks.rs` (or `kagi.rs` / `pnf.rs`); `tt-chart/src/{kind.rs, derived.rs, draw/series.rs}`; `tradeterm/src/{menus.rs, ui.rs}` (icons) |
| **S14** | **Trading / Alerts / Events tabs.** Show the Events tab with Session breaks only (from S9). Trading and Alerts stay hidden until their backends exist; their models are already in S0. | P3 | S9; backends | `tradeterm/src/chart_settings/events.rs` |

Parallelism after S0 + S10: S1, S2→S3, S4, S5, S6, S8 and S9 can run at the
same time, because their `draw.rs` regions were split into separate files or
separate functions in S0. S7 shares `view.rs` with S5 and S6 only in disjoint
functions. S11 then S12 run in sequence.

Acceptance for every slice:

- v1 `settings.json` fixtures still load and look the same.
- `cargo test` and `cargo clippy --all-targets -- -D warnings` pass.
- `bench_draw` with 1M bars shows no regression over 2 % with all new options
  at their defaults.
- Each new setting has a dialog row, persists, survives restart, is included
  in templates and resets via Apply defaults.

---

## 5. Sources

- TV help: [How to configure your Supercharts](https://www.tradingview.com/support/solutions/43000748166-how-to-configure-your-supercharts/) (tab contents, price scale menu, Template)
- TV Advanced Charts: [ChartPropertiesOverrides](https://www.tradingview.com/charting-library-docs/latest/api/interfaces/Charting_Library.ChartPropertiesOverrides) (defaults for every series style, pane, scales, session breaks)
- TV Advanced Charts: [Chart overrides](https://www.tradingview.com/charting-library-docs/latest/customization/overrides/chart-overrides/), [UI map](https://tradingview.com/charting-library-docs/ui-map/) (price scale menu item names), [Legend](https://www.tradingview.com/charting-library-docs/latest/ui_elements/Legend), [Time zones](https://www.tradingview.com/charting-library-docs/latest/ui_elements/timezones/), [Watermarks](https://www.tradingview.com/charting-library-docs/latest/ui_elements/watermarks/), [Saving and loading](https://www.tradingview.com/charting-library-docs/latest/saving_loading/) (chart templates, layouts)
- TV help: [Renko](https://www.tradingview.com/support/solutions/43000502284-understanding-renko-charts/), [Renko wicks](https://www.tradingview.com/support/solutions/43000481040-what-do-renko-wicks-mean/), [Kagi](https://www.tradingview.com/support/solutions/43000502272-learn-to-use-kagi-charts/), [Point & figure](https://www.tradingview.com/support/solutions/43000502276-point-and-figure-pnf-charts/), [One step back building](https://in.tradingview.com/support/solutions/43000564837-what-does-the-one-step-back-building-option-mean/), [Line break](https://www.tradingview.com/support/solutions/43000502273-line-break-charts/)
- TV help: [Layouts quick guide](https://www.tradingview.com/support/solutions/43000746975-tradingview-layouts-a-quick-guide/), [Layouts, charts, drawings, indicators and their interaction](https://www.tradingview.com/support/solutions/43000692404-layouts-charts-drawings-indicators-and-their-interaction/), [Blue vertical lines (session breaks)](https://www.tradingview.com/support/solutions/43000762935-i-cannot-remove-blue-vertical-lines-on-my-chart/), [Autosave blog post](https://www.tradingview.com/blog/en/chart-layout-autosave-can-be-disabled-now-7338/)
