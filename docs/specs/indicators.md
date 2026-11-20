# Indicators (studies): specification

Status: draft spec. Owner area: indicators, their settings dialog, presets/defaults, indicator
templates, legend (status line) interactions, panes/scales for indicators.

Goal: indicators that are as complete as TradingView's. That means every input and every style
item, presets and templates, not just a plotted line. Performance stays the top priority (see the
project memory): style changes must never trigger a recompute, and input changes recompute only
that indicator.

## Confidence markers

TradingView publishes no single machine-readable list of its built-in indicator settings, so each
fact below carries a source marker:

- **[HC]**: confirmed in the TradingView Help Center (tradingview.com/support/solutions/...) or
  a TradingView blog post.
- **[PS]**: from the built-in indicator's Pine Script source as shown in TradingView's Pine
  editor. This comes from memory of that source and was cross-checked where possible against
  the Help Center and mirrors such as indie-script.github.io. Confidence is high, but verify
  against the live app before you freeze a default.
- **[?]**: uncertain or inferred. Verify against the live TradingView app, and do not treat it
  as fact.

Colors are given as Pine names with hex: `color.blue` = `#2196F3`, `color.green` = `#4CAF50`,
`color.red` = `#F44336`, `color.yellow` = `#FFEB3B`, `color.olive` = `#808000`,
`color.teal` = `#00897B` (Pine palette, [PS]). "transp 90" means the Pine transparency value,
so alpha = 10 %.

---

## 1. Current state

### 1.1 What exists

| Area | Where | What it does |
|---|---|---|
| Catalog | `crates/tt-core/src/studies.rs:87-112` | 10 kinds: Volume, SMA, EMA, BB, VWAP, RSI, MACD, Stochastic, ATR, OBV. |
| Inputs | `studies.rs:175-216` | Typed enum `Study` (serde `tag = "type"`). |
| Defaults | `studies.rs:130-169` | Matches TradingView for the inputs that exist (SMA/EMA 9, BB 20/2, RSI 14, MACD 12/26/9, Stoch 14/1/3, ATR 14, Vol MA 20). |
| Compute | `studies.rs:319-487` | Incremental from `from`. RSI, MACD and Stoch do a full pass (`recompute_fully`, `:490-495`). EMA is SMA-seeded, as in Pine `ta.ema`. RSI and ATR use Wilder RMA. BB uses population stdev (matches `ta.stdev`). |
| Placement | `studies.rs:66-73, 236-244` | Fixed per kind: Overlay / VolumeOverlay / Pane. |
| Plot kinds | `studies.rs:77-83` | `Line`, `Histogram` (MACD sign colors), `VolumeColumns`. |
| Guide levels | `studies.rs:296-302` | Hard-coded: RSI 30/50/70, Stoch 20/50/80, fixed 0..100 bounds. |
| Indicator | `crates/tt-chart/src/indicators.rs:52-65` | `study`, `visible`, `colors: Vec<Color32>` (one per output index), `pane_weight`, `pane_collapsed`, output cache. |
| Default colors | `tt-chart/src/indicators.rs:25-35` | One color per output. |
| Value format | `tt-chart/src/indicators.rs:163-178` | Volume/OBV abbreviated, RSI/Stoch 2 decimals, the rest use price precision. |
| Drawing | `crates/tt-chart/src/draw.rs:757-788` | Lines are 1 px (`:783`). The MACD histogram fades a shrinking bar to alpha 0.45 (`:792-817`). The BB fill is hard-coded (`:760-776`). |
| Guides | `draw.rs:820-843` | Dashed outer lines and a dotted middle line in `theme.text_muted`. The band fill is `color(0)` at alpha 0.08. |
| Axis tags | `draw.rs:861-922` | A last-value tag for every visible output. One global switch `style.indicator_value_labels` (`draw.rs:215`, `style.rs:93`). |
| Legend | `draw.rs:929-1092` | Title + inputs + values. Hover shows eye/gear/trash (`:1032-1037`). Global `legend_titles/inputs/values` (`style.rs:83-85`, `draw.rs:1013, 1058`). Double-click opens settings. |
| Panes | `crates/tt-chart/src/view.rs:769-794, 336-430` | One pane per Pane-placement indicator: move up/down, collapse, maximize, resize. Linear auto-fit only (`view.rs:990-1004`). |
| Overlay autoscale | `view.rs:750-756` | Price-pane overlays take part in price auto-fit. |
| Indicators dialog | `crates/tradeterm/src/indicators.rs:65-325` | Search, aliases, Favorites/Technicals, stars, keyboard navigation. |
| Settings dialog | `tradeterm/src/indicators.rs:329-846` | Inputs/Style tabs, live preview, Cancel/× restores (`:556-562`), Ok commits, Defaults ▾ → "Reset settings" only (`:815-827`). |
| Persistence | `crates/tt-session/src/settings.rs:35-83` | `IndicatorConfig { study, visible, colors: Vec<[u8;4]>, pane_weight, pane_collapsed }`, per tab in `TabSettings.indicators` (`:95`). Favorites in `AppSettings.favorite_indicators` (`:144`). |

### 1.2 Missing or half-done (concrete)

**Inputs**
1. The Inputs tab is hand-coded per variant (`tradeterm/src/indicators.rs:364-452`). There is no
   declarative input schema, so every new indicator needs dialog code.
2. These inputs are missing from every indicator that should have them: **Offset**; the
   **Smoothing** group (Type / Length / BB StdDev) on SMA, EMA, RSI and OBV; **Calculate
   Divergence** (RSI); **Basis MA Type** (BB); **Oscillator MA type / Signal MA type** (MACD);
   **Smoothing** (ATR, which is RMA-only, `studies.rs:458-470`); VWAP **Anchor Period**,
   **Source**, **Hide on 1D or above**, **Bands calculation mode**, **3 band multipliers**. VWAP is
   a unit variant pinned to the UTC day with an hlc3 source (`studies.rs:195-196, 374-408`). The
   dialog only prints "Session (UTC day)" (`tradeterm/src/indicators.rs:760-762`).
3. There is no **Calculation** section (Timeframe / Wait for timeframe closes).
4. There is no "Inputs in status line" checkbox. Only the global chart setting exists.
5. `Source` can only be OHLC-derived (`studies.rs:16-25`). There is no "indicator on
   indicator" source (another study's output).

**Style**
6. The Style tab offers only one color picker per output, plus the Volume MA checkbox and an
   indicator-level "Visible" checkbox (`tradeterm/src/indicators.rs:767-795`). It has **no**
   per-plot visibility checkbox, **no line width, no line style, no plot type**
   (line/step/histogram/columns/area/circles/cross), **no price line**, **no opacity control**
   (only whatever alpha the color picker returns), **no hlines** (levels are not editable: not
   their value, color, style or width, `studies.rs:296-302`), **no fill rows** (the BB and RSI
   background fills are hard-coded, `draw.rs:760-776, 826-829`), **no Precision**, and no
   **Labels on price scale** / **Values in status line** per indicator.
7. Volume column colors cannot be edited. They follow the theme (`tradeterm/src/indicators.rs:535-537`,
   `draw.rs:702-707`). TradingView exposes Falling/Growing colors on the Volume plot [PS].
8. The MACD histogram uses a 2-color scheme with alpha fading (`draw.rs:808-813`).
   TradingView uses 4 editable colors: `#26A69A` grow above, `#B2DFDB` fall above, `#FFCDD2`
   rise below, `#FF5252` fall below [PS].
9. Default colors differ from TradingView ([PS]):
   - SMA/EMA lines are `color.blue` `#2196F3`. We use `#2962FF` (`tt-chart/src/indicators.rs:28`).
   - BB Basis is `#2962FF`. We use `#2196F3` (`:29`).
   - The BB fill is `rgb(33,150,243)` at alpha 5 %. We use `color(0)` at 8 %.
   - The OBV plot is `#2962FF`, which matches.
10. Colors are stored **by output index** (`settings.rs:40-43`). Adding outputs (for example
    "RSI-based MA" or the BB bands of the smoothing group) would shift indices and corrupt saved
    colors. Styles must move to stable plot ids (see §3.4).

**Visibility, defaults, templates**
11. There is no Visibility tab (interval ranges).
12. The Defaults menu has only "Reset settings" (factory). There is no "Save as default" (per
    kind) and no named presets. `reset()` (`tradeterm/src/indicators.rs:632-645`) updates the
    number inputs but **not the color pickers**, which then show stale colors (a bug). It also
    does not reset `visible`.
13. There are no indicator templates: no storage in `AppSettings` (`settings.rs:123-149`) and
    no toolbar menu.

**Legend, panes, scales**
14. The legend has no **More (…)** menu and no source-code button (BACKLOG.md:30). There is no
    right-click context menu on a legend row or a plot.
15. Placement is fixed per kind (`studies.rs:236-244`). An oscillator cannot be overlaid on
    price, an MA cannot be moved to its own pane, and two indicators cannot share a pane
    (BACKLOG.md:29). There is no "Pin to scale" (right / left / no scale). Indicator panes are
    always linear auto-fit.
16. There is no plot offset (shift by N bars). Ichimoku needs plots drawn **past the last bar**.

**Missing catalog** (vs. TradingView's most-used built-ins): Ichimoku Cloud, Supertrend,
Parabolic SAR, DMI/ADX, CCI, Williams %R, MFI, Stochastic RSI, Keltner Channels, Donchian
Channels, WMA, HMA, VWMA, Pivot Points Standard, Volume Profile Visible Range.

---

## 2. The settings dialog (TradingView reference)

Opened from the legend gear, by double-clicking the legend or the plot, or with "Settings…" in
the More / context menu [HC]. The title is the indicator's short title (for example "RSI").

### 2.1 Layout

- Tabs: **Inputs**, **Style**, **Visibility** [HC: Inputs/Style named in the Help Center articles;
  Visibility in the blog post "Customizable indicator visibility"]. Indicators without inputs
  still show the Inputs tab, with only "Inputs in status line" [?].
- Footer: **Defaults ▾** on the left, **Cancel** and **Ok** on the right. Changes apply live.
  Cancel, ×, Esc and clicking outside revert. Ok keeps. (We already do this.)
- **Defaults ▾** menu:
  - **Reset settings**: restores the *user default* if one is saved, else factory [?]. Some
    TradingView builds show "Reset settings" (factory) separately from "Apply defaults" [?].
    Our menu: *Reset settings* (to the user default, or to factory if there is none), plus
    *Reset to factory settings* when a user default exists.
  - **Save as default**: stores the current inputs and style as the default for every new
    instance of this indicator kind [HC: third-party guides describe "save as default" at the
    bottom of the indicator settings].
  - **Named presets**: TradingView's drawing tools have named templates ("Save as…", list of
    names). For indicators, named presets in this menu are **not confirmed** [?]. We implement
    them as a tradeterm extension in the same menu: **Save preset as…**, then the list of saved
    presets (click to apply, × to delete), shown only when presets exist.

### 2.2 Inputs tab (common rows)

- Inputs are rendered in declaration order, in **groups** with small caps headings (for example
  "RSI SETTINGS", "SMOOTHING", "CALCULATION") [PS: `group=` in Pine]. Inline inputs share one
  row (VWAP "☑ Bands Multiplier #1 [1]") [PS: `inline=`].
- Widget per type: int/float → number field with stepper, min/max clamped and a `step`. bool →
  checkbox. string with options → dropdown. source → dropdown (open, high, low, close, hl2,
  hlc3, ohlc4, hlcc4, plus the outputs of other indicators on the chart: "indicator on
  indicator"). color → swatch. timeframe → interval dropdown.
- Inputs may have a **tooltip** (an ⓘ icon) [PS].
- **Calculation** group, for indicators that support MTF [HC 43000591555]:
  - **Timeframe**: dropdown. The default is "Chart" (same as the chart) [PS: `timeframe=""`].
    The options are the chart's interval list, including custom intervals.
  - **Wait for timeframe closes**: checkbox, default **on** [PS: `timeframe_gaps=true`]. When on,
    higher-timeframe values appear only when the HTF bar closes, and gaps are left. When off, the
    last value fills forward and the real-time bar updates on every tick.
- **Inputs in status line**: a checkbox at the bottom of the Inputs tab, default on [?
  placement; the option itself exists]. It shows the input values after the title in the legend.
  Inputs marked `display = data_window` in Pine are never shown in the status line [PS]. Those
  are marked "SL: no" in the tables below.

### 2.3 Style tab (common rows)

1. **One row per plot**: `[☑] <Plot title>  [color/width swatch]  [Plot type ▾]`. The checkbox
   hides only that plot. The swatch popup has a color grid, custom color, an **opacity** slider
   (0-100 %) and **thickness** 1-4 px (for line-like types) [HC: ATR "color, thickness, and
   visual type"]. Plots with several colors (MACD histogram, Volume) show one swatch per color
   [PS]. A **Price line** toggle shows a horizontal line at the plot's last value across the pane
   [HC: ATR/VWAP "toggle ... and its current-value price line"]. Its exact UI (a checkbox under
   the row or inside the popup) is uncertain [?].
   - Plot types [HC: Charting Library `LineStudyPlotStyle`]: **Line, Line with breaks, Step
     line, Step line with breaks, Step line with diamonds, Histogram, Cross, Area, Area with
     breaks, Columns, Circles**.
2. **One row per hline**: `[☑] <Band title>  [color/width/style swatch]  [value]`. The style is
   solid / dashed / dotted. The default for `hline` is **dashed**, width 1 [PS]. The value is
   editable [HC: Stochastic "Sets its value (80 by default)…thickness, and style (default:
   dashes)"].
3. **One row per fill**: `[☑] <Fill title>  [color + opacity swatch]` [HC: Stochastic
   "Background … sets the color and opacity"]. Gradient fills (RSI overbought/oversold) show a
   two-color swatch [?].
4. Common rows at the bottom:
   - **Precision**: dropdown **Default, 0, 1, 2, 3, 4, 5, 6, 7, 8** decimals [HC: ATR/VWAP
     "Precision"; the list is ?]. Default = the indicator's `format`/`precision` (price precision,
     RSI 2, volume abbreviated).
   - **Labels on price scale**: checkbox, default on [HC 43000659666].
   - **Values in status line**: checkbox, default on [HC 43000659666]. Both are ANDed with the
     global chart settings: if the global setting is off, the per-indicator option cannot bring
     it back [HC].

### 2.4 Visibility tab

[HC: blog "Customizable indicator visibility", "Faster setup of visibility on intervals"]: the
same control as for drawings. One row per interval unit, each with a checkbox, a **from** field,
a dual-handle slider and a **to** field:

| Row | Range |
|---|---|
| Ticks | checkbox only [?] (n/a for us) |
| Seconds | 1-59 |
| Minutes | 1-59 |
| Hours | 1-24 |
| Days | 1-366 |
| Weeks | 1-52 |
| Months | 1-12 |
| Ranges | checkbox only [?] (n/a) |

The defaults are all checked, over the full range. The context menu has **Intervals visibility ▸**
with presets [HC: the label "Intervals visibility" is confirmed]. The presets are believed to be
*Current interval and above*, *Current interval and below*, *Current interval only* and *All
intervals* [?]. An indicator that is hidden by visibility is not computed (saves CPU) and shows
no legend row [?, our choice: show the legend row greyed, like a hidden indicator, so it can be
found].

---

## 3. Shared infrastructure spec

### 3.1 Declarative study metadata (tt-core)

Keep `Study` typed: it is fast, exhaustive matches catch mistakes, and existing serde keeps
working. Add a **metadata layer** so the dialog, legend, defaults and drawing are generic:

```rust
// tt-core/src/studies/meta.rs
pub struct StudyMeta {
    pub kind: StudyKind,
    pub name: &'static str,          // "Relative Strength Index"
    pub short: &'static str,         // "RSI" (legend + dialog title)
    pub placement: Placement,        // default placement (overlay / pane / volume overlay)
    pub format: ValueFormat,         // Price | Volume | Percent | Fixed(u8)
    pub mtf: bool,                   // has Calculation group
    pub inputs: &'static [InputDef],
    pub plots: &'static [PlotDef],
    pub hlines: &'static [HLineDef],
    pub fills: &'static [FillDef],
    pub bounds: Option<(f64, f64)>,  // fixed scale hint (RSI 0..100)
}

pub struct InputDef {
    pub id: &'static str,            // stable: "length", "src", "ma_type"
    pub title: &'static str,         // "RSI Length"
    pub group: Option<&'static str>, // "RSI Settings", "Smoothing", "Calculation"
    pub inline: Option<&'static str>,
    pub tooltip: Option<&'static str>,
    pub kind: InputKind,             // Int{min,max,step} | Float{min,max,step} | Bool
                                     // | Choice(&[&str]) | Source | Timeframe | Color
    pub in_status_line: bool,        // Pine display flag
    pub enabled_if: Option<fn(&Study) -> bool>, // e.g. BB StdDev only for "SMA + Bollinger Bands"
}

pub enum InputValue { Int(i64), Float(f64), Bool(bool), Choice(u16), Source(SourceRef), Timeframe(Option<Timeframe>) }

impl Study {
    pub fn meta(&self) -> &'static StudyMeta;
    pub fn get(&self, input_id: &str) -> Option<InputValue>;
    pub fn set(&mut self, input_id: &str, v: InputValue) -> bool; // clamps to InputDef range
    pub fn legend_title(&self) -> String; // short + in_status_line inputs, in order (generic)
}
```

`PlotDef { id, title, default: PlotStyle, kind: PlotKind, palette: &[(&str /*color name*/, Color32)] }`.
`palette` covers multi-color plots: MACD "Histogram" has 4 named colors, and Volume has
"Falling"/"Growing". `kind` says what the plot *means* to the drawer: `Value` (normal),
`SignedColumns` (MACD palette), `VolumeColumns`, `Shape` (divergence labels).
`enabled_if` lets plots that depend on inputs (for example the RSI-based MA when Type = None)
render greyed out instead of disappearing. This matches Pine `editable = enableMA` [PS].

Output order in `compute` = the `plots` order. `Study::outputs()` becomes `meta().plots`.

**File split** (to allow parallel work): turn `studies.rs` into `studies/mod.rs` (the enum, the
`Source`/`SourceRef` types, dispatch), `studies/ma.rs` (sma/ema/rma/wma/vwma/hma/smoothing-MA
helpers, `MaType`), `studies/meta.rs`, and one file per indicator family (`volume.rs`, `ma_simple.rs`,
`bb.rs`, `vwap.rs`, `rsi.rs`, `macd.rs`, `stoch.rs`, `atr.rs`, `obv.rs`, then new ones). Each file
owns its compute fn, its `StudyMeta` const and its tests.

Shared enums:

```rust
pub enum MaType { Sma, Ema, Smma /* RMA */, Wma, Vwma }               // label "SMMA (RMA)"
pub enum SmoothingType { None, Sma, SmaBb, Ema, Smma, Wma, Vwma }     // "SMA + Bollinger Bands"
pub struct Smoothing { pub kind: SmoothingType, pub length: u32 /*14*/, pub bb_mult: f64 /*2.0*/ }
pub enum SourceRef { Price(Source), Study { indicator: u64 /*runtime id*/, plot: &'static str } }
```

`Source` gains `Hlcc4` ((h+l+2c)/4) [PS]. `SourceRef::Study` persists as `{ "study_index": n,
"plot": "rsi" }` (index into the tab's indicator list, resolved at load). The indicator-on-indicator
compute order is a topological sort, and cycles are rejected in the dropdown. This is slice I-11.

### 3.2 Per-plot style data model (tt-chart)

```rust
// tt-chart/src/indicator_style.rs
#[derive(Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct IndicatorStyle {
    pub plots: BTreeMap<String, PlotStyle>,   // key = PlotDef.id; missing = default
    pub hlines: BTreeMap<String, HLineStyle>, // key = HLineDef.id
    pub fills: BTreeMap<String, FillStyle>,   // key = FillDef.id
    pub precision: Option<u8>,                // None = "Default", else 0..=8
    pub labels_on_price_scale: bool,          // default true
    pub values_in_status_line: bool,          // default true
    pub inputs_in_status_line: bool,          // default true (lives on the Inputs tab in UI)
    pub visibility: IntervalVisibility,       // Visibility tab
}

pub struct PlotStyle {
    pub visible: bool,
    pub colors: Vec<Rgba>,      // 1 entry, or palette size (MACD 4, Volume 2)
    pub width: u8,              // 1..=4
    pub plot_type: PlotType,    // Line | LineBr | Step | StepBr | StepDiamond | Histogram
                                // | Cross | Area | AreaBr | Columns | Circles
    pub line_style: LineStyle,  // Solid | Dashed | Dotted (TradingView exposes it for some line plots [?])
    pub price_line: bool,       // default false
}
pub struct HLineStyle { pub visible: bool, pub color: Rgba, pub width: u8, pub style: LineStyle /* default Dashed */, pub value: f64 }
pub struct FillStyle  { pub visible: bool, pub color: Rgba /* opacity in alpha */, pub color2: Option<Rgba> /* gradient */ }
pub struct IntervalVisibility { pub seconds: Option<(u16,u16)>, pub minutes: …, pub hours: …, pub days: …, pub weeks: …, pub months: … } // None = unit hidden; default all full range
```

Opacity is stored in the color's alpha (TradingView does the same in its color picker).
`Rgba` = `[u8; 4]` for serde, and converts to `Color32` at draw time.

`Indicator` (`tt-chart/src/indicators.rs:52`) gets `style: IndicatorStyle`, a `pane: PaneId` and a
`scale: ScaleBinding` (see §3.6). `colors` is removed. `Indicator::color(k)` becomes
`plot_style(id)`, which resolves an explicit style or falls back to the meta default.

Drawing (`draw.rs`) is driven only by `meta().plots` + `style`:
- `study_plots` dispatches on `PlotType` and uses `width`. Needed new primitives: step lines,
  circles/crosses/diamonds (batched quads), area-to-zero fills (`fill_area` exists), columns
  (exists). A `Canvas` trait addition, `fn markers(&mut self, pts: &[Pos2], shape, size, color)`,
  makes the drawing one batched call (performance).
- Fills come from `meta().fills` (between two plots, between two hlines, or plot↔hline). The
  hard-coded BB and RSI fills are removed.
- hlines come from `meta().hlines` + `style.hlines` (value, style, width, color). `bounds()`
  remains only as a scale hint.
- Price line: a horizontal line at the last value, in the plot color, solid 1 px [?].
- Precision: `format_value` takes `style.precision`.
- The tags in `study_tags` are gated by `style.labels_on_price_scale && global`. Legend values are
  gated by `style.values_in_status_line && global`. Legend inputs are gated by
  `style.inputs_in_status_line && global`.
- Plot offset: draw value `i` at bar `i + offset`. Ichimoku needs a positive offset past the
  last bar, which requires the x-mapping to accept indices beyond `bar_count` (it already takes
  floats, `viewport.rs:123`). Legend/crosshair values for bar `b` read `values[b - offset]`.

**Performance rule:** `IndicatorStyle` is **not** part of the cache key (`Cache.study`,
`indicators.rs:43-50`). Style edits only repaint, and input edits recompute that indicator only.

### 3.3 Visibility / interval gating

`IntervalVisibility::shows(tf: Timeframe) -> bool`. `ChartView` skips the sync, extent and draw
of hidden indicators (`view.rs:688, 751`). A hidden pane indicator collapses its pane away [?].
TradingView removes the pane space; we do the same.

### 3.4 Persistence and backward compatibility (tt-session)

Today:

```json
{ "study": {"type":"Rsi","length":14,"source":"Close"}, "visible": true,
  "colors": [[126,87,194,255]], "pane_weight": 1.0, "pane_collapsed": false }
```

New `IndicatorConfig` (all new fields `#[serde(default)]`, unknown fields ignored):

```rust
pub struct IndicatorConfig {
    pub study: Study,             // new variant fields each #[serde(default = "…")]
    pub visible: bool,
    #[serde(default, skip_serializing)] pub colors: Vec<[u8; 4]>, // legacy, read only
    pub style: IndicatorStyle,    // new
    pub pane_weight: f32,
    pub pane_collapsed: bool,
    pub pane: Option<u32>,        // new: pane slot index in TabSettings.panes; None = legacy layout
    pub scale: ScaleBinding,      // new, default = kind's default
}
```

Rules:
1. **Study variants.** New inputs are added as fields with `#[serde(default = "fn")]`, where the
   function returns TradingView's default (for example `Rsi { length, source, #[serde(default)]
   divergence: bool, #[serde(default = "smoothing_sma14")] smoothing: Smoothing, … }`). The
   variant **name** never changes. The unit variants `Vwap` and `Obv` become struct variants
   whose fields are all defaulted. `{"type":"Vwap"}` still deserializes, but add a test to prove
   it. Note the old file `{"type":"Rsi",...}` gets `smoothing = SMA 14`, which makes a yellow MA
   appear on old charts. Decision: a missing `smoothing` key on legacy RSI deserializes as
   **None**, and only `default_study()` uses SMA, so old charts look unchanged. Use a separate
   `#[serde(default = "smoothing_none")]` for that.
2. **Legacy colors.** When `colors` is non-empty and `style.plots` is empty, map them by
   **frozen legacy output order** (a table in settings.rs, *not* `meta().plots`):
   Volume [vol_up?, "vol_ma"], SMA/EMA/VWAP/OBV ["plot"], BB ["basis","upper","lower"], RSI ["rsi"],
   MACD ["histogram"(palette[0] = grow above),"macd","signal"], Stoch ["k","d"], ATR ["atr"].
   Volume index 0 was never user-editable, so skip it. Never write `colors` again.
3. `visible` stays the indicator-level eye. The per-plot `visible` lives in `style`.
4. A file written by the new version and read by the old one: the old version ignores `style`,
   `pane` and `scale`, and loses only the new features. This works because `#[serde(default)]`
   is on the struct and serde ignores unknown fields (already tested in
   `settings.rs:357-361`). Old binaries **will fail** on new Study fields only if
   `deny_unknown_fields` is set. It is not set, so that is fine. Old binaries fail on **new Study
   variants** (for example `{"type":"Supertrend"}`): the whole settings file is rejected and
   falls back to defaults (`settings.rs:297-303`). Mitigation for downgrades: deserialize
   `TabSettings.indicators` element-wise and skip unknown entries with a warning (a custom
   `deserialize_with` that collects `Vec<serde_json::Value>` and tries each one). Land this
   **before** any new variant ships.
5. Store sparse styles: only write the plot/hline/fill entries that differ from the meta default,
   so defaults can improve later and settings stay small.

### 3.5 Defaults, presets and templates storage (AppSettings)

```rust
pub struct IndicatorPreset {            // inputs + style, no runtime/pane fields
    pub study: Study,
    pub style: IndicatorStyle,
}
pub struct NamedPreset { pub name: String, pub preset: IndicatorPreset }

pub struct IndicatorTemplate {
    pub name: String,
    pub indicators: Vec<IndicatorConfig>, // full configs incl. pane/scale assignment and weights
    pub panes: Vec<PaneConfig>,           // pane order/weights (see 3.6)
    pub symbol: Option<String>,           // "Remember Symbol" [HC]
    pub timeframe: Option<Timeframe>,     // "Remember Interval" [HC (pineify) / blog says interval saved]
    pub favorite: bool,
}

// AppSettings additions (#[serde(default)]):
pub indicator_defaults: BTreeMap<StudyKind, IndicatorPreset>,   // "Save as default"
pub indicator_presets:  BTreeMap<StudyKind, Vec<NamedPreset>>,  // tradeterm extension
pub indicator_templates: Vec<IndicatorTemplate>,
```

- `add_indicator(kind)` (`tradeterm/src/app.rs:512-519`) uses `indicator_defaults[kind]` when it
  exists, else `kind.default_study()` + the meta style.
- "Reset settings" → user default if one exists, else factory. "Reset to factory settings" →
  factory.
- Presets and defaults that fail to deserialize (an unknown kind from a newer version) are skipped
  element-wise, as in 3.4 rule 4.

### 3.6 Panes and scales

Replace "placement decides the pane" with an explicit assignment:

```rust
pub struct PaneConfig { pub weight: f32, pub collapsed: bool }  // index 0 = price pane
pub enum ScaleBinding { Right /* Scale A, shared with price on pane 0 */, Left, NoScale /* overlay, own invisible auto-fit, e.g. Volume */ , Own /* pane's own right scale */ }
```

- `TabSettings.panes: Vec<PaneConfig>`: when it is empty (a legacy file), derive the panes as
  today, one per Pane-placement indicator, with weight/collapsed taken from the indicator config.
- Each `Indicator` has a `pane: usize` (a slot) and a `scale`. Defaults: Overlay → (0, Right),
  VolumeOverlay → (0, NoScale, with the bottom-25 % margin as today), Pane → (new pane, Own).
- Several indicators in one pane: the pane's auto-fit is the union of the extents of its
  indicators that use the same scale. Hlines and fills of every indicator in the pane are drawn.
  The legend has one row per indicator, stacked.
- Moving an oscillator onto the price pane with "Right" scale squashes the price, as in
  TradingView. The default for "Move to price pane" is to keep `Own`, which becomes a left-hand or
  hidden scale [?]. Our rule: an indicator moved to the price pane gets `NoScale` unless its
  values overlap the price range (an MA moved back), in which case it gets `Right`.
- Pane removal: removing the last indicator in a pane removes the pane.

---

## 4. Legend (status line) and context menu

Hover buttons on each legend row (TradingView order) [HC partial / ?]: **Hide/Show (eye)**,
**Settings (gear)**, **Source code `{}`** (we can skip it or show "built-in" info), **Remove (×/trash)**,
**More (…)**. The More menu and the right-click on the legend row or the plot open the same menu:

| Item | Notes |
|---|---|
| Add alert on <short>… | Depends on the alerts system. Disabled until it exists. [?] |
| Add indicator on <short>… | Opens the Indicators dialog with the source preset to this indicator's first plot (I-11). [?] |
| Visual order ▸ Bring to front / Bring forward / Send backward / Send to back | [HC 43000474299] Changes the draw order within the pane. |
| Move to ▸ New pane above / New pane below / Existing pane above / Existing pane below | [HC: "Move to" exists; labels ?]. "Existing pane above" from the first indicator pane = the price pane. |
| Pin to scale ▸ Right (Scale A) / Left / No scale (full screen) | [HC 43000775579: "Pin to Scale", "No Scale", "Left", "Right", "Scale A"]. TradingView also offers "Pin to new left/right scale" [?]. |
| Intervals visibility ▸ presets | [HC blog] |
| Copy (Ctrl+C) / Paste | Copies the indicator config onto another chart tab [?]. |
| Hide | |
| Settings… | |
| Remove (Del) | |

Also: dragging a legend row or plot onto another pane moves it there [HC 43000762829 "Just drag
it manually"]. Double-clicking a plot opens its settings.

---

## 5. Per-indicator specifications

Table columns for inputs: **Title** (exact UI label) · **id** (our stable id) · **Type** · **Default**
· **Range / options** · **SL** (shown in status line) · **Src**.

For style plots: `Title (id) — default color · width · type`. All plots default to visible, width
1, type Line, and price line off, unless stated otherwise. hlines default to dashed, width 1.

### 5.1 Volume ("Volume", short "Vol")

Placement: price pane, bottom, `NoScale` (overlay), format volume. TradingView's Volume is a
built-in non-Pine study. Its labels are from the app [?].

| Title | id | Type | Default | Range | SL | Src |
|---|---|---|---|---|---|---|
| MA Length | ma_length | int | 20 | 1-? (we use 1-500) | yes | ? |
| Color based on previous close | prev_close | bool | false | | no | ? |
| (Symbol override?) | | | | | | not offered by TradingView's Volume [?] |

Style:
- **Volume (vol)**: Columns, palette **Falling** `#F7525F` @50 %, **Growing** `#22AB94` @50 % [?
  exact values; ours follow the theme `volume_up/down`]. Plot types allowed: Columns, Histogram,
  Line, Area…
- **Volume MA (vol_ma)**: `#2196F3`, width 1, Line, **hidden by default** (replaces our
  `show_ma` input, which moves to the style plot's visible flag. Migration: `show_ma=true` →
  `plots.vol_ma.visible=true`).
- Precision, Labels on price scale, Values in status line.
- Placement extras [?]: TradingView's Volume can "Move to" its own pane, where it gets a normal
  scale.

### 5.2 Moving Average Simple (short "SMA") [PS]

The legend title in TradingView is "SMA 9 close" [?; older builds "MA"]. We currently show "MA 9 close".

| Title | id | Type | Default | Range | SL |
|---|---|---|---|---|---|
| Length | length | int | 9 | ≥1 | yes |
| Source | src | source | close | | yes |
| Offset | offset | int | 0 | -500..500 | no |
| **Smoothing** › Type | sm_type | choice | **None** | None, SMA, SMA + Bollinger Bands, EMA, SMMA (RMA), WMA, VWMA [HC 43000742042] | no |
| Smoothing › Length | sm_length | int | 14 | ≥1 | no |
| Smoothing › BB StdDev | sm_bb | float | 2.0 | 0.001-50, step 0.5; enabled only for "SMA + Bollinger Bands" | no |
| Calculation › Timeframe / Wait for timeframe closes | tf / tf_gaps | | Chart / on | | no |

Style:
- MA (plot): `color.blue #2196F3`
- SMA-based MA (sm_ma): `color.yellow #FFEB3B`. Hidden and not editable while Type = None.
- Upper Bollinger Band (sm_upper) / Lower Bollinger Band (sm_lower): `color.green #4CAF50`. Shown
  only for SMA + BB.
- Fill "Bollinger Bands Background Fill" (sm_bb_fill): green, transp 90.
- Precision / Labels / Values.

### 5.3 Moving Average Exponential (short "EMA") [PS]

Same as 5.2 but EMA: Length 9, Source close, Offset 0, Smoothing (Type None, Length 14, BB
StdDev 2), Calculation. Plot **EMA** (plot) `#2196F3`. "EMA-based MA" yellow, BB plots and fill
as in 5.2.

### 5.4 Bollinger Bands (short "BB") [PS, HC 43000501840 confirms Length=20, Basis MA Type]

| Title | id | Type | Default | Range | SL |
|---|---|---|---|---|---|
| Length | length | int | 20 | ≥1 | yes |
| Basis MA Type | ma_type | choice | SMA | SMA, EMA, SMMA (RMA), WMA, VWMA | yes |
| Source | src | source | close | | yes |
| StdDev | mult | float | 2.0 | 0.001-50 | yes |
| Offset | offset | int | 0 | -500..500 | no |
| Calculation | | | Chart / on | | no |

Legend (TradingView): "BB 20 SMA close 2" (input order) [PS/?]. Ours is "BB 20 2 close".
Stdev is always `ta.stdev(src, length)` (population) around the chosen basis MA.

Style: **Basis** (basis) `#2962FF`. **Upper** (upper) `#F23645`. **Lower** (lower) `#089981`. Fill
**Background** (bg) between upper and lower, `rgb(33,150,243)` transp 95 (alpha 5 %). Precision,
Labels, Values.

### 5.5 Volume Weighted Average Price (short "VWAP") [PS; HC 43000502018 confirms the names]

| Title | id | Type | Default | Range / options | SL |
|---|---|---|---|---|---|
| *VWAP Settings* › Hide VWAP on 1D or Above | hide_dwm | bool | false | | no |
| Anchor Period | anchor | choice | Session | Session, Week, Month, Quarter, Year, Decade, Century, Earnings, Dividends, Splits (we omit the last 3 for crypto) | yes |
| Source | src | source | hlc3 | | no |
| Offset | offset | int | 0 | ≥0 | no |
| *Bands Settings* › Bands Calculation Mode | band_mode | choice | Standard Deviation | Standard Deviation, Percentage (multiplier 1 = 1 %) | no |
| ☑ Bands Multiplier #1 | band1_on / band1 | bool + float (inline) | **on**, 1.0 | ≥0, step 0.5 | no |
| ☑ Bands Multiplier #2 | band2_on / band2 | bool + float | off, 2.0 | ≥0, step 0.5 | no |
| ☑ Bands Multiplier #3 | band3_on / band3 | bool + float | off, 3.0 | ≥0, step 0.5 | no |
| Calculation › Timeframe / Wait | | | Chart / on | | no |

Compute: the anchor resets the cumulative sums at the start of each period. Session = the
exchange session. For 24/7 crypto that is 00:00 UTC, as today. Week = ISO week (Monday 00:00
UTC). The rest are calendar periods in UTC; switch to the chart timezone when timezone support
lands (BACKLOG). Stdev band:
`sqrt(Σv·src²/Σv − vwap²)`. Percentage band: `vwap·mult/100`. When `hide_dwm` is on and the chart
is ≥1D, nothing is drawn.

Style: **VWAP** (vwap) `#2962FF`. **Upper Band #1 / Lower Band #1** `color.green`. **#2**
`color.olive`. **#3** `color.teal`. Fills **Bands Fill #1/#2/#3** in the same colors at transp 95.
Band plots are hidden and not editable when their checkbox is off. Precision, Labels, Values.

### 5.6 Relative Strength Index (short "RSI") [HC 43000502338 + 43000742042; PS]

| Title | id | Type | Default | Range | SL |
|---|---|---|---|---|---|
| *RSI Settings* › RSI Length | length | int | 14 | ≥1 | yes |
| Source | src | source | close | | yes |
| Calculate Divergence | divergence | bool | false | tooltip: "needed for divergence alerts" | no |
| *Smoothing* › Type | sm_type | choice | **SMA** | None, SMA, SMA + Bollinger Bands, EMA, SMMA (RMA), WMA, VWMA | no |
| Smoothing › Length | sm_length | int | 14 | ≥1 | no |
| Smoothing › BB StdDev | sm_bb | float | 2.0 | 0.001-50 | no |
| *Calculation* › Timeframe / Wait for timeframe closes | | | Chart / on | | no |

Divergence (fixed constants, not inputs [PS]): pivot lookback left 5 / right 5, range between
pivots 5..60 bars. Regular bullish = price makes a lower low while RSI makes a higher low (at RSI
pivot lows). Regular bearish = the mirror. Drawn as lines between the two RSI pivots, plus labels
"Bull"/"Bear", offset −5 bars (found 5 bars late, plotted at the pivot).

Style:
- **RSI** (rsi) `#7E57C2`.
- **RSI-based MA** (sm_ma) `color.yellow`. Hidden when Type = None.
- **Upper Bollinger Band / Lower Bollinger Band** (sm_upper/sm_lower) `color.green`, only for
  SMA + BB. Fill **Bollinger Bands Background Fill**: green, transp 90.
- hlines: **RSI Upper Band** (upper) 70 `#787B86`. **RSI Middle Band** (middle) 50 `#787B86` @50 %.
  **RSI Lower Band** (lower) 30 `#787B86`. All dashed, width 1 [PS: default `hline` style]. Ours
  draws the middle one dotted (`draw.rs:839`); change it to dashed.
- Fill **RSI Background Fill** (bg) between upper and lower: `rgb(126,87,194)` transp 90.
- Fills **Overbought Gradient Fill** (ob_grad) between RSI and 50, clipped to 70..100: green
  gradient (opaque at 100 → transparent at 70). **Oversold Gradient Fill** (os_grad) between 0 and
  30: red gradient [PS]. The renderer needs a vertical-gradient band fill clipped to a value range
  (extend `fill_between`).
- **Regular Bullish** (bull) green, width 2. **Regular Bullish Label** (bull_label) green label
  with white text " Bull ". **Regular Bearish** (bear) red, width 2. **Regular Bearish Label**
  (bear_label) [PS]. Shown only when Calculate Divergence is on.
- Precision default 2 (format price, precision 2). Labels, Values.
- Scale: TradingView's RSI auto-fits. It is not locked to 0..100 [?]. Ours forces 0..100 to be
  included (`view.rs:992-996`). Keep including the hline range, as TradingView's built-ins do via
  hlines.

### 5.7 MACD (title "Moving Average Convergence Divergence", short "MACD") [PS; HC 43000502344]

| Title | id | Type | Default | Range | SL |
|---|---|---|---|---|---|
| Fast Length | fast | int | 12 | ≥1 | yes |
| Slow Length | slow | int | 26 | ≥1 | yes |
| Source | src | source | close | | yes |
| Signal Smoothing | signal | int | 9 | 1-50 | no |
| Oscillator MA Type | osc_ma | choice | EMA | SMA, EMA | no |
| Signal Line MA Type | sig_ma | choice | EMA | SMA, EMA | no |
| Calculation › Timeframe / Wait | | | Chart / on | | no |

Labels: the current Help Center calls them "Fast length / Slow length / Signal length /
Oscillator MA type / Signal MA type" [HC]. The Pine source labels are as in the table [PS]. Use
the HC wording if the live app agrees [?]. The legend (TradingView) is "MACD 12 26 close"
(signal/types are `data_window`) [PS/?]. Ours is "MACD 12 26 9 close".

Style:
- **Histogram** (hist): **Columns**, 4 colors: *grow above* `#26A69A`, *fall above* `#B2DFDB`,
  *rise below* `#FFCDD2`, *fall below* `#FF5252` (rule: `hist ≥ 0 ? (hist[1] < hist ? c0 : c1)
  : (hist[1] < hist ? c2 : c3)`).
- **MACD** (macd) `#2962FF`. **Signal** (signal) `#FF6D00`.
- hline **Zero Line** (zero) 0, `#787B86` @50 %.
- Precision, Labels, Values.
- Alerts (later): "Rising to falling", "Falling to rising" [PS].

### 5.8 Stochastic (short "Stoch") [PS; HC 43000502332]

| Title | id | Type | Default | Range | SL |
|---|---|---|---|---|---|
| %K Length | k | int | 14 | ≥1 | yes |
| %K Smoothing | k_smooth | int | 1 | ≥1 | yes |
| %D Smoothing | d | int | 3 | ≥1 | yes |
| Calculation | | | Chart / on | | no |

The HC article names these "K 14 / D 3 / Smooth 3" and says Smooth defaults to 3. That
conflicts with the source (smoothK = 1) and with the legend "Stoch 14 1 3" seen on charts. Keep
**1** [? verify].

Style: **%K** (k) `#2962FF`. **%D** (d) `#FF6D00`. hlines **Upper Band** 80 `#787B86`, **Middle
Band** 50 `#787B86` @50 %, **Lower Band** 20 `#787B86`. Fill **Background** (bg) between the
upper and lower bands, `rgb(33,150,243)` transp 90. (Ours uses `color(0)` = `#2962FF` @8 %,
`draw.rs:828`.) Precision default 2.

### 5.9 Average True Range (short "ATR") [PS; HC 43000501823]

| Title | id | Type | Default | Range | SL |
|---|---|---|---|---|---|
| Length | length | int | 14 | ≥1 | yes |
| Smoothing | smoothing | choice | RMA | RMA, SMA, EMA, WMA | yes |
| Calculation | | | Chart / on | | no |

Style: **ATR** (atr) `#B71C1C`. Precision, Labels, Values. TR on the first bar = high − low
(`ta.tr(true)`), which matches today.

### 5.10 On Balance Volume (short "OBV") [PS]

Inputs: none for the calculation (source fixed to close) [PS]. A **Smoothing** group (Type
**None**, Length 14, BB StdDev 2) was added in the 2023-24 refresh [?]. Calculation group.

Style: **OnBalanceVolume** (obv) `#2962FF`. "OBV-based MA" yellow, plus the BB plots and fill as
in 5.2 [?]. Format volume. TradingView raises the error "No volume is provided by the data
vendor." when total volume is 0 [PS]. Show that as the legend text.

---

### 5.11 New indicators (prioritized)

Priority = how often traders expect it on a crypto charting app.

#### P1. Supertrend [PS]
Inputs: **ATR Length** 10 (≥1), **Factor** 3.0 (≥0.01, step 0.01). Calculation group.
Compute: `ta.supertrend(factor, atrPeriod)`, with ATR = RMA of TR. Na on the first bar.
Plots: **Up Trend** `color.green` (*Line with breaks*), **Down Trend** `color.red` (*Line with
breaks*), **Body Middle** ((o+c)/2, display none, used for the fills). Fills: **Uptrend
background** (body middle↔up trend, green transp 90), **Downtrend background** (red transp 90).
Overlay. Alerts: "Downtrend to Uptrend", "Uptrend to Downtrend", "Trend Change".

#### P1. Ichimoku Cloud (short "Ichimoku") [PS]
Inputs: **Conversion Line Length** 9, **Base Line Length** 26, **Leading Span B Length** 52,
**Lagging Span** 26 (the displacement) (all ≥1).
Compute: donchian(n) = avg(highest high, lowest low). Conversion = donchian(9). Base =
donchian(26). Lead A = avg(conv, base). Lead B = donchian(52).
Plots: **Conversion Line** `#2962FF`, **Base Line** `#B71C1C`, **Lagging Span** = close, offset
−(disp−1), `#43A047`. **Leading Span A** offset +(disp−1), `#A5D6A7`. **Leading Span B** offset
+(disp−1), `#EF9A9A`. **Kumo Cloud Upper/Lower Line** (display none). Fill between A and B,
colored `rgb(67,160,71) transp 90` when A > B, else `rgb(244,67,54) transp 90` (a conditional
fill color, which needs a two-color fill style "Plots Background" [?]).
Requires offset drawing past the last bar (§3.2).

#### P1. Stochastic RSI (short "Stoch RSI") [PS]
Inputs: **K** 3, **D** 3, **RSI Length** 14, **Stochastic Length** 14, **RSI Source** close (all
int ≥1). Calculation group.
Compute: `k = sma(stoch(rsi, rsi, rsi, lenStoch), smoothK)`, `d = sma(k, smoothD)`.
Plots **K** `#2962FF`, **D** `#FF6D00`. hlines Upper Band 80, Middle Band 50 (@50 %), Lower Band
20 (`#787B86`). Fill **Background** `rgb(33,150,243)` transp 90.

#### P1. Directional Movement Index (short "DMI") and Average Directional Index ("ADX") [PS]
DMI inputs: **ADX Smoothing** 14 (1-50), **DI Length** 14 (≥1). Plots **ADX** `#F50057`, **+DI**
`#2962FF`, **−DI** `#FF6D00`.
ADX (separate catalog entry): the same two inputs. Plot **ADX** `color.red`.
Compute: Wilder RMA of +DM/−DM/TR. `ADX = 100 · RMA(|+DI − −DI| / (+DI + −DI), adxLen)`.

#### P1. Parabolic SAR (short "SAR") [PS]
Inputs: **start** 0.02, **increment** 0.02, **Max Value** 0.2 (floats). Calculation group.
Plot **ParabolicSAR** `#2962FF`, type **Cross**. Overlay. Needs the Cross/Circles plot type.

#### P2. Commodity Channel Index (short "CCI") [PS]
Inputs: **Length** 20 (≥1), **Source** hlc3. *Smoothing* group: **Method** SMA (SMA, EMA, SMMA (RMA),
WMA, VWMA), **Length** 5 [PS/?].
`CCI = (src − sma(src, n)) / (0.015 · meanDev(src, n))`.
Plots **CCI** `#2962FF`, **Smoothing Line** `#F37F20` (hidden by default [?]). hlines **Upper
Band** 100 dashed, **Middle Band** 0 @50 %, **Lower Band** −100 dashed (`#787B86`). Fill
**Background** `rgb(33,150,243)` transp 90.

#### P2. Williams Percent Range (short "%R") [PS]
Inputs: **Length** 14, **Source** close.
`%R = 100 · (src − highest(n)) / (highest(n) − lowest(n))`.
Plot **%R** `#7E57C2`. hlines **Upper Band** −20, **Middle Level** −50 (dotted), **Lower Band**
−80 (`#787B86`). Fill **Background** `rgb(126,87,194)` transp 90. Overbought/oversold gradient
fills as in RSI [PS/?].

#### P2. Money Flow Index (short "MFI") [PS]
Inputs: **Length** 14 (1-2000). Source fixed hlc3.
Plot **MF** `#7E57C2`. hlines **Overbought** 80, **Middle Band** 50 (@50 %), **Oversold** 20.
Fill **Background** `rgb(126,87,194)` transp 90.

#### P2. Keltner Channels (short "KC") [PS]
Inputs: **Length** 20, **Multiplier** 2.0, **Source** close, **Use Exponential MA** true, **Bands
Style** "Average True Range" (Average True Range, True Range, Range), **ATR Length** 10.
Plots **Upper**, **Basis**, **Lower**, all `#2962FF`. Fill **Background** `rgb(33,150,243)`
transp 95. Overlay.

#### P2. Donchian Channels (short "DC") [PS]
Inputs: **Length** 20, **Offset** 0 (−500..500).
Plots **Basis** `#FF6D00`, **Upper** `#2962FF`, **Lower** `#2962FF`. Fill **Background**
`rgb(33,150,243)` transp 95. Overlay.

#### P2. Moving Average Weighted ("WMA"), Hull Moving Average ("HMA"), VWMA [PS/?]
- WMA: **Length** 9, **Source** close, **Offset** 0. Plot `#2962FF` [? color].
- HMA: **Length** 9, **Source** close. `wma(2·wma(n/2) − wma(n), floor(sqrt(n)))`. Plot `#2962FF` [?].
- VWMA: **Length** 20, **Source** close, **Offset** 0. `sma(src·vol, n)/sma(vol, n)`. Plot `#2962FF` [?].
- All three get the Smoothing group (Type None) [?] and the Calculation group.

#### P3. Pivot Points Standard (short "Pivots") [PS]
Inputs: **Type** Traditional (Traditional, Fibonacci, Woodie, Classic, DM, Camarilla).
**Pivots Timeframe** Auto (Auto, Daily, Weekly, Monthly, Quarterly, Yearly, Biyearly, Triyearly,
Quinquennially, Decennially). **Number of Pivots Back** 15 (1-200). **Use Daily-based Values**
true. *Labels* group: **Show Labels** true, **Show Prices** true, **Labels Position** Left
(Left/Right). *Levels* group: **Line Width** 1 (1-100), then per level P, S1/R1 … S5/R5 a color
(`#FB8C00` each) + a show checkbox (true).
Auto = 1D for intraday ≤15 min, 1W for other intraday, 1M for daily, 12M for weekly and above
[PS/?]. Drawn as horizontal segments per period, not as plots. The renderer needs a
"segments + labels" output kind (`PlotKind::Segments`). Note that TradingView puts these colors
and widths on the *Inputs* tab, not Style, because Pine draws them with `line.new` [PS].

#### P3. Volume Profile Visible Range (short "VPVR"/"VRVP") [?, non-Pine built-in]
Inputs: **Rows Layout** Number Of Rows (Number Of Rows / Ticks Per Row), **Row Size** 24,
**Volume** Up/Down (Up/Down, Total, Delta), **Value Area Volume** 70 %.
Style: **Volume Profile**: Width (% of the box) 30, Placement Right (Left/Right), **Up Volume**
and **Down Volume** colors (blue-ish / yellow-ish, alpha ~25 %), **Value Area Up/Down** colors
(stronger alpha). **POC** line (color, width), **Value Area High / Low** lines. Optionally
**Developing POC / VA** lines. Recomputes on every pan or zoom from the visible bars only, cached
by `range`. Needs a horizontal-histogram renderer in the price pane. Intrabar precision is limited
to bar OHLCV (distribute each bar's volume over the rows its range covers).

---

## 6. Indicator templates (TradingView reference)

- Entry point: the **Indicator templates** button on the top toolbar, next to "Indicators" (the
  4-squares icon) [HC 43000543048].
- The menu [HC/?]:
  - **Save indicator template…**: a dialog with **Template name**, ☐ **Remember Symbol**, ☐
    **Remember Interval** [HC blog / pineify], and Save / Cancel. Saving under an existing name
    asks to overwrite [?].
  - Sections **My templates** (user, each row with ☆ favorite and × delete on hover) and
    **Technicals** (built-in, star only) [?]. Built-ins listed by HC: *Bill Williams' 3 Lines,
    Displaced EMA, MA Exp Ribbon, Oscillators, Swing Trading, Volume Based* [HC]. Oscillators =
    Volume + RSI + Stoch RSI + CCI. 3 Lines = Volume + 3 MAs [HC].
  - Favorited templates appear in the favorites toolbar / top of the menu [?].
- **Apply** replaces all indicators on the chart with the template's indicators and settings,
  including pane layout [pineify: "replaces the indicators already on the target chart"]. If the
  template remembers a symbol or interval, those are switched too [HC blog].
- Templates do **not** store drawings, chart type or colors (those belong to the layout and chart
  settings) [pineify].

tradeterm spec:
- A toolbar button "Indicator templates" right of "Indicators" (`tradeterm/src/app.rs:946-956`),
  with a dropdown menu as above. Built-in templates we can ship with today's catalog:
  *Oscillators* (Volume, RSI, Stoch, MACD) and *Moving averages* (Volume, EMA 9/21/50/200). Add
  the TradingView ones once their indicators exist.
- Apply = undoable replace (keep the previous set to restore with one "Undo" toast [?
  extension]). Indicators get new runtime ids. Pane weights and collapsed state are restored.
- Stored in `AppSettings.indicator_templates` (§3.5). Names are unique, compared
  case-insensitively.

---

## 7. Implementation plan (parallel slices)

Order: **I-0 → I-1 → (I-2 … I-10 in parallel)**. Each slice lists the files it owns. Overlaps are
noted, and they are kept to append-only additions (new match arms, new meta consts) to minimise
merge conflicts.

| Slice | Content | Files (primary) | Depends on |
|---|---|---|---|
| **I-0 Foundations** (1 engineer, first) | Split `studies.rs` into a `studies/` module dir (pure move, tests unchanged). Add `StudyMeta`/`InputDef`/`PlotDef`/`HLineDef`/`FillDef`, `InputValue`, `Study::meta/get/set`, generic `legend_title`. Add `MaType`, `Smoothing`, the ma helpers (wma, vwma, hma, smma), `Source::Hlcc4`. Element-wise tolerant deserialization of indicator lists (§3.4 rule 4). Serde tests for legacy JSON (unit `Vwap`, old RSI without smoothing). | `tt-core/src/studies/*` (new), `tt-core/src/lib.rs`, `tt-session/src/settings.rs` (tolerant `indicators` deserializer only) | — |
| **I-1 Style model + generic drawing** | `IndicatorStyle`, `PlotStyle`, `HLineStyle`, `FillStyle`, precision, status-line flags. Replace `Indicator.colors`. The drawer is driven by meta + style: widths, plot types (line/br, step/br/diamond, histogram, columns, area/br, circles, cross), palettes (MACD 4 colors, Volume falling/growing), hlines, fills (plot↔plot, hline↔hline, gradient clipped), price lines, offset (incl. past the last bar), per-indicator tags/values gating, precision in `format_value`. `Canvas::markers`. Legacy color migration by frozen table. | `tt-chart/src/indicators.rs`, `tt-chart/src/indicator_style.rs` (new), `tt-chart/src/draw.rs` (study_plots, histogram, pane_guides, study_tags, legend_row values), `tt-chart/src/render/*` + `tt-chart-gpui/src/canvas.rs` (markers primitive), `tt-session/src/settings.rs` (IndicatorConfig fields + migration) | I-0 |
| **I-2 Generic settings dialog** | Rewrite the Inputs tab from `InputDef` (groups, inline, tooltips, enabled_if, choice/bool/int/float/source/timeframe widgets). Style tab rows for plots (checkbox, swatch with opacity + width, plot-type dropdown, price line), hlines (value, style, width), fills, Precision, Labels on price scale, Values in status line. "Inputs in status line" on the Inputs tab. Fix the reset/picker desync. Cancel restores study + style. | `tradeterm/src/indicators.rs` (settings panel part; consider moving it to `tradeterm/src/indicator_settings.rs`), maybe a new `tradeterm/src/widgets/line_style_picker.rs` | I-0, I-1 (types only; can start against the type definitions) |
| **I-3 Defaults & presets** | Defaults ▾ menu: Reset settings / Reset to factory / Save as default / Save preset as… / preset list with delete. `AppSettings.indicator_defaults`, `indicator_presets`. `add_indicator` uses defaults. | `tt-session/src/settings.rs` (AppSettings fields), `tradeterm/src/app.rs` (`add_indicator`, persist), the footer section of the settings dialog (coordinate with I-2: I-2 exposes a `footer_menu` hook) | I-1 |
| **I-4 Visibility tab** | `IntervalVisibility` + the Visibility tab UI (per-unit checkbox + range inputs + dual slider). Gating in sync/extent/draw/legend. "Intervals visibility ▸" presets in the context menu (I-6). | `tt-chart/src/indicator_style.rs` (visibility type), `tt-chart/src/view.rs` (gating), `tradeterm/src/indicator_visibility.rs` (new tab component) | I-1 |
| **I-5 Upgrade existing 10 indicators to TradingView parity** | All §5.1-5.10 inputs and styles: Smoothing group (SMA/EMA/RSI/OBV), RSI divergence + gradient fills, BB Basis MA Type + offset, VWAP anchors / source / bands / hide-on-D, MACD MA types + 4-color histogram + zero line, ATR smoothing, Volume palette and MA as a plot, TradingView default colors, offset input. Can split per indicator file among 2-3 engineers. | `tt-core/src/studies/{volume,ma_simple,bb,vwap,rsi,macd,stoch,atr,obv}.rs` | I-0 (compute), I-1 (for fills/palettes to render) |
| **I-6 Legend More menu + context menu** | Legend buttons + More (…) and right-click menu: Visual order, Move to, Pin to scale, Intervals visibility, Copy/Paste, Hide, Settings, Remove. Add alert / Add indicator on … are disabled stubs. Drag a legend row to another pane. | `tt-chart/src/draw.rs` (legend_row buttons only), `tt-chart/src/view.rs` (hit test), `tt-chart-gpui/src/chart.rs` (menu + drag), `tradeterm/src/menus.rs` | I-7 for Move to / Pin (menu items can land disabled first) |
| **I-7 Panes & scales model** | `PaneConfig`, `ScaleBinding`, several indicators per pane, move to price pane / new pane above/below / existing pane, pin to right/left/no scale, union auto-fit per scale, left scale rendering, visual order within a pane. Legacy layout derivation. | `tt-chart/src/view.rs` (layout, panes, ranges), `tt-chart/src/draw.rs` (pane loop, left scale), `tt-session/src/settings.rs` + `tt-session/src/tab.rs` (TabSettings.panes) | I-0. Touches `draw.rs`/`view.rs` like I-1, so sequence after I-1 merges or coordinate on functions |
| **I-8 Indicator templates** | Toolbar button + menu, save dialog (name, Remember Symbol, Remember Interval), apply (replace, switch symbol/interval), favorite, delete, built-in templates. | `tradeterm/src/indicator_templates.rs` (new), `tradeterm/src/app.rs` (toolbar button, apply), `tt-session/src/settings.rs` (`indicator_templates`) | I-1 (IndicatorConfig shape), ideally I-7 (pane layout) |
| **I-9 New indicators wave 1** | Supertrend, Stoch RSI, DMI, ADX, Parabolic SAR, CCI, Williams %R, MFI, Keltner, Donchian, WMA, HMA, VWMA. Compute + meta + tests (incremental-vs-full like `studies.rs:763-808`), catalog aliases in the Indicators dialog. One engineer per 3-4 indicators. | `tt-core/src/studies/<name>.rs` (new each), `studies/mod.rs` (enum arm + dispatch, append-only), `tradeterm/src/indicators.rs` (`aliases`, append-only) | I-0, I-1 |
| **I-10 New indicators wave 2** | Ichimoku (future offset), Pivot Points Standard (segments + labels output kind), Volume Profile Visible Range (horizontal histogram, range-dependent compute). | `tt-core/src/studies/{ichimoku,pivots,vpvr}.rs`, `tt-chart/src/draw.rs` (new segment/profile renderers; isolated new fns) | I-1, I-9 pattern |
| **I-11 MTF + indicator-on-indicator** | Calculation group: compute on a resampled series (`tt-core/src/resample.rs`) with "wait for timeframe closes" gaps. `SourceRef::Study` with dependency ordering and cycle check. "Add indicator on …" enabled. | `tt-chart/src/indicators.rs` (sync with resampled source), `tt-chart/src/view.rs` (dependency order), `tt-core/src/studies/mod.rs` (SourceRef) | I-0, I-1, I-6 |
| **I-12 Alerts hooks** (later) | "Add alert on …", alertconditions from the meta (MACD rising/falling, Supertrend trend change, RSI divergence). | alerts subsystem (not yet present) | alerts system |

Notes for coordinators:
- `tt-session/src/settings.rs` is touched by I-0, I-1, I-3, I-7 and I-8. Each adds separate
  fields and tests. Merge in that order, and keep each field's serde attributes self-contained.
- `draw.rs` is touched by I-1 (plots), I-6 (legend buttons), I-7 (pane loop) and I-10 (new
  renderers). Land I-1 first. I-6, I-7 and I-10 then touch disjoint functions.
- Update `README.md` (Indicators section) and remove BACKLOG.md:29-30 when I-6 and I-7 land.
- Every slice keeps the performance rule: style edits never invalidate `Indicator` caches, and
  new indicators must pass the incremental-tail-equals-full test.

## Sources

- TradingView Help Center: RSI 43000502338, Smoothing 43000742042, Calculation/MTF 43000591555,
  MACD 43000502344, Stochastic 43000502332, ATR 43000501823, VWAP 43000502018, BB 43000501840,
  hide individual indicator values 43000659666, indicator templates 43000543048, pin to scale
  43000775579, move to main pane 43000762829, visual order 43000474299.
- TradingView blog: "Indicator Templates With a Saved Symbol", "Customizable indicator
  visibility", "Faster setup of visibility on intervals".
- TradingView Charting Library docs: `LineStudyPlotStyle` enum (plot types).
- Mirrors and guides: indie-script.github.io built-in RSI (cross-check), pineify.app indicator
  templates workflow.
