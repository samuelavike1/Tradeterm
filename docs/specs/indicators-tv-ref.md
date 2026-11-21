# TradingView indicator dialogs: reference

Captured from tradingview.com (dark theme, Nov 2026) by adding a temporary
instance of each indicator from the picker, opening its settings, reading
every tab with `getBoundingClientRect` / `getComputedStyle`, then removing
it. Screenshots: `img/tv-ref/tv-ind-<indicator>-<tab>.png`,
`tv-ind-rsi-plottype.png` (the plot type menu), `tv-ind-picker.png` and
`tv-ind-picker-search.png`. Sizes are CSS px; the density tokens are the
comfortable ones of [ui-guidelines.md §2.10](ui-guidelines.md) (same as the
drawing dialogs, [drawings-tv-ref.md](drawings-tv-ref.md)).

TradingView's built-ins are Pine scripts newer than some of ours (e.g. RSI
has smoothing / divergence inputs, MACD MA-type selects, a CALCULATION
group with Timeframe). tradeterm renders *its own* metadata
(`tt_core::studies::meta`) with TradingView's layout; the inputs it does not
have are listed per indicator as "TV only".

## Settings dialog: common to every indicator

| Part | TradingView | tradeterm |
|---|---|---|
| Surface | radius 20, translucent, width 380 (Inputs / Style); grows with the widest label (RSI Style 429) and to 453 on Visibility | `DialogOpts::comfortable().live_width(..)`: 380 min, widest label + controls on Inputs / Style, 454 on Visibility |
| Header | title = short name ("RSI", "BB", "Vol", "Stoch"), 20 / 600; "Dock to right" pin, ×; **no rename**; draggable | short name, ×, draggable; no dock button |
| Tabs | Inputs · Style · Visibility (16 / 600, 4 px underline) | same |
| Body | padding 16 / 20 | same |
| Footer | `Defaults ▾` (100 × 34 outline) · Cancel · Ok | same (`DialogFooter::comfortable`) |
| Defaults menu | Reset settings, Save as default | Reset settings, Save as default, (Apply default, Remove saved default), Save preset as…, presets |

### Inputs tab

- A two-column table: the label cell takes the free width, the controls sit
  in a right-hand column as wide as its widest control (number / select
  100 × 34, radius 8). Inputs with a tooltip get an ⓘ (18 px, muted) after
  the control; the column then includes that slot (127 = 100 + 27).
- Rows 50 tall; a lone checkbox ("Color based on previous close", "Wait for
  timeframe closes") 34 tall, spanning the row.
- A checkbox can lead a row with a field ("☑ Bands Multiplier #1 [1]").
- Groups: an uppercase 11 px muted caption 32 tall, 16 above it ("VWAP
  SETTINGS", "SMOOTHING", "CALCULATION").
- Disabled inputs: filled field, muted text.
- Selects truncate with "…" ("Standar…", "(H + L +…").

### Style tab

| Row | Controls |
|---|---|
| plot | `[x] Title` · color + line button (75 × 34: 24 swatch + 30 px line preview of its width / dash) · plot type button (34 × 34, the type's icon) |
| multi-color plot (Volume, MACD histogram, OBV) | `[x] Title` alone (34 tall), then one row per color, 42 tall, label indented to the checkbox label ("Color 0", "Growing"): color + line button, the first also the plot type button; 8 px gap after the group |
| level (hline) | `[x] Title` · color + line button · value field (100) |
| fill / background | `[x] Title` · swatch (34); gradient fills two swatches |
| OUTPUT VALUES | caption; `Precision [Default ▾]`; ☑ Labels on price scale; ☑ Values in status line (34 tall) |
| INPUT VALUES | caption; ☑ Inputs in status line |

- Label column = widest title + checkbox (26) + 20.
- Unchecked rows dim their controls.
- **Plot type menu** (`tv-ind-rsi-plottype.png`): 203 wide, radius 6; first
  row "Price line" with a switch (38 × 20); separator; Line, Line with
  breaks, Step line, Step line with breaks, Step line with diamonds,
  Histogram, Cross, Area, Area with breaks, Columns, Circles (icon +
  label, 32 tall; current one inverted).

### Visibility tab

Identical to the drawing dialogs (`tv-ind-rsi-visibility.png`): ☑ Ticks;
per unit `[x] Seconds [1] ——o==o—— [59]` (label column 96, fields 100,
slider 100); ☑ Ranges. Ranges: Seconds / Minutes 1–59, Hours 1–24, Days
1–366, Weeks 1–52, Months 1–12. tradeterm shares the rows
(`crate::visibility_tv`); Ticks and Ranges persist only (no tick / range
charts yet), like the drawings'.

## Per indicator

### Volume (`tv-ind-volume-inputs.png`, `tv-ind-volume-style.png`)

| Tab | TradingView | tradeterm |
|---|---|---|
| Inputs | MA Length [20]; ☐ Color based on previous close | same |
| Style | ☑ Volume; Growing [color+line][type]; Falling [color+line]; ☐ Volume MA [color+line][type] | ☑ Volume; Falling, Growing (palette order); ☐ Volume MA (the `show_ma` input is this checkbox) |

### Moving Average Simple / Exponential (`tv-ind-sma-*.png`, `tv-ind-ema-*.png`)

| Tab | TradingView | tradeterm |
|---|---|---|
| Inputs | Length [9], Source [Close], Offset [0]; SMOOTHING: Type [None], Length, BB StdDev (disabled while None); CALCULATION: Timeframe, ☑ Wait for timeframe closes | Length, Source |
| Style | ☑ MA / ☑ EMA [color+line][type]; output / input values | ☑ Value [color+line][type] |

TV only: Offset, smoothing, Timeframe.

### Bollinger Bands (`tv-ind-bb-*.png`)

| Tab | TradingView | tradeterm |
|---|---|---|
| Inputs | Length [20] ⓘ, Basis MA Type [SMA] ⓘ, Source [Close] ⓘ, StdDev [2] ⓘ, Offset [0] ⓘ; CALCULATION | Length, StdDev, Source |
| Style | ☑ Basis, ☑ Upper, ☑ Lower [color+line][type]; ☑ Background [swatch] | same |

### VWAP (`tv-ind-vwap-*.png`)

| Tab | TradingView | tradeterm |
|---|---|---|
| Inputs | VWAP SETTINGS: ☐ Hide VWAP on 1D or Above, Anchor Period [Session], Source [(H + L + C)/3], Offset; BANDS SETTINGS: Bands Calculation Mode, ☑ Bands Multiplier #1 [1], ☐ #2, ☐ #3; CALCULATION | Anchor Period [Session] |
| Style | ☑ VWAP, ☑ Upper Band #1, ☑ Lower Band #1 [color+line][type]; ☑ Bands Fill #1 [swatch] | ☑ Value [color+line][type] |

### RSI (`tv-ind-rsi-*.png`)

| Tab | TradingView | tradeterm |
|---|---|---|
| Inputs | RSI SETTINGS: RSI Length [14], Source [Close], ☐ Calculate Divergence ⓘ; SMOOTHING: Type [SMA], Length [14], BB StdDev; CALCULATION | RSI Length, Source |
| Style | ☑ RSI, ☑ RSI-based MA [color+line][type]; ☑ RSI Upper / Middle / Lower Band [line][70 / 50 / 30]; ☑ RSI Background Fill [swatch]; ☑ Overbought / Oversold Gradient Fill [swatch][swatch] | ☑ RSI; the three bands; ☑ RSI Background Fill |

### MACD (`tv-ind-macd-*.png`)

| Tab | TradingView | tradeterm |
|---|---|---|
| Inputs | Source [Close], Fast length [12], Slow length [26], Signal length [9], Oscillator MA type [EMA], Signal MA type [EMA]; CALCULATION | Fast Length, Slow Length, Signal Smoothing, Source |
| Style | ☑ Histogram; Color 0–3 [color+line] (Color 0 also [type]); ☑ MACD, ☑ Signal line [color+line][type]; ☑ Zero [line][0] | ☑ Histogram; Grow above / Fall above / Rise below / Fall below; ☑ MACD; ☑ Signal (no Zero line) |

### Stochastic (`tv-ind-stoch-*.png`)

| Tab | TradingView | tradeterm |
|---|---|---|
| Inputs | %K Length [14], %K Smoothing [1], %D Smoothing [3]; CALCULATION | same, no CALCULATION |
| Style | ☑ %K, ☑ %D [color+line][type]; ☑ Upper / Middle / Lower Band [line][80 / 50 / 20]; ☑ Background [swatch] | same |

### ATR (`tv-ind-atr-*.png`)

| Tab | TradingView | tradeterm |
|---|---|---|
| Inputs | Length [14], Smoothing [RMA]; CALCULATION | Length |
| Style | ☑ ATR [color+line][type] | same |

### OBV (`tv-ind-obv-*.png`)

| Tab | TradingView | tradeterm |
|---|---|---|
| Inputs | ☑ Show Signal Line, Signal Smoothing Type [SMA], Length [21], ☐ Highlight Crossovers, ☑ Apply OBV/Signal Ribbon Filling; CALCULATION | none ("No inputs") |
| Style | ☑ OBV + Color 0–2; ☑ Signal + Color 0–2; ☑ Crossover / Crossunder Mark [color][shape][location]; ☑ Plots Background + Color 0–2 | ☑ OBV [color+line][type] |

## Indicators picker ("Indicators, metrics, and strategies")

`tv-ind-picker.png`, `tv-ind-picker-search.png`

| Part | TradingView | tradeterm |
|---|---|---|
| Surface | 840 × 638, radius 12 (translucent `#131313d9`); header 68 (title 20 / 600, × 34) | `DialogOpts::comfortable().width(840)`, body 570 |
| Search | full width minus 20 each side, 40 tall, radius 8, 1 px border; search icon in a 28 box; clear × when filled | same |
| Left nav | 230 wide (items 200 at x 20); captions "PERSONAL", "BUILT-IN", "COMMUNITY" (11 px muted, 30 tall); items 40 tall, radius 8, icon in a 28 box at +8, label 14 at +44; current item filled `#303030` and semibold | Personal: Favorites; Built-in: Technicals (no scripts / community) |
| List header | "SCRIPT NAME" 11 px muted, row 40 (search: group captions "TECHNICALS", "COMMUNITY") | "SCRIPT NAME"; "TECHNICALS" while searching |
| Technicals pills | Indicators · Strategies · Profiles · Patterns · Options (28 tall pills) | none (indicators only) |
| Rows | 32 tall, radius 8, x 240 (10 into the list), star in a 22 box at +4 (hidden until hover unless a favorite), name 14 at +30; matched text in blue; keyboard row `#132042` | same; keyboard row `active_bg`, match `active` |
| Columns | Author / Boosts (community scripts only) | none |
