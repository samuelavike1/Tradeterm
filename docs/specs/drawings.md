# Drawing tools: specification

Status: draft, 2026-11-20. Scope: TradingView's left drawing toolbar, the drawings it creates,
their settings dialog, floating toolbar, context menu, templates, keyboard and mouse
behaviour. Goal: every tool fully featured (settings, customization, templates), at
TradingView quality, without giving up the performance north star (drawing must never cost
frame time when nothing changes).

## How to read this

- **TV** = TradingView Supercharts (tradingview.com). **TV-lib** = TradingView's Charting
  Library / Advanced Charts, whose "Drawings Overrides" page lists the exact property keys and
  default values of each drawing type. TV-lib keys are the best public description of what a
  drawing stores. TV.com's own defaults are sometimes different. For example, TV-lib's line
  color is teal `rgba(21,119,96)` at 1 px, while TV.com draws trend lines in `#2962FF` at 2 px.
  Where they differ, this spec uses the TV.com value and notes the TV-lib value.
- Confidence markers:
  - **[doc]**: stated in a TradingView help center article or the TV-lib docs.
  - **[ui]**: TV.com behaviour from the author's knowledge of the product, not found in public
    docs. It is very likely right but should be checked against a live chart before it ships.
  - **[?]**: uncertain. The spec gives a proposal instead of claiming it is TradingView's
    behaviour.
- Sources:
  - TV help center, folder "How to use various drawing tools":
    https://www.tradingview.com/support/folders/43000547459
  - The per-tool articles in that folder, e.g. trend line 43000518095, info line 43000791601,
    trend angle 43000791602, horizontal line 43000518124, horizontal ray 43000518121, vertical
    line 43000518093, cross line 43000477747, parallel channel 43000518117, Fib retracement
    43000518158, trend-based Fib extension 43000518137, rectangle 43000516984, ellipse
    43000516988, circle 43000662172, brush 43000516987, path 43000791739, polyline
    43000516986, arrow 43000518134, text 43000516983, note 43000737571, callout 43000516978,
    price label 43000518083, long position 43000517002, long/short how-to 43000475660, date
    and price ranges 43000516996.
  - Tips: clone 43000537251, multi-select 43000537219, 45° lines 43000537268,
    horizontal/vertical drag 43000538248, magnet 43000722509.
  - TV-lib v26 "Drawings Overrides":
    https://www.tradingview.com/charting-library-docs/v26/customization/overrides/Drawings-Overrides
  - TV blog posts on interval visibility (34613), same-type templates (33205) and magnet
    hotkeys (14315).

---

## 1. Current state

### 1.1 What exists

| Area | Where | Notes |
|---|---|---|
| Tool enum, 12 tools | `crates/tt-chart/src/drawings.rs:73-87` | Trend line, ray, extended line, horizontal line, horizontal ray, vertical line, cross line, Fib retracement, rectangle, price range, date range, date and price range |
| Drawing model | `drawings.rs:204-235` | `tool, points, color, width, style, locked`. `id` is runtime only (`:207`) |
| Line styles | `drawings.rs:177-195`, `styled_line :731-746` | Solid, dashed, dotted |
| Placement | `press :521-569`, `motion :572-614`, `release :617-631`, `finish :634-646` | 1- or 2-point tools: click-click or drag-to-place |
| Select / drag body / drag handle | `hit :490-518`, `press`, `motion` | Body drag snaps to whole bars (`:593`) |
| Magnet (single mode) | `Mapper::snapped :260-285`, `MAGNET_DISTANCE :34` | Snaps price to the nearest OHLC within 30 pt. This is a "weak" magnet |
| Hover | `motion :608-612`, `draw_overlay :808-815` | Hover shows handles; pointer cursor (`tt-chart-gpui/src/chart.rs:424`) |
| Axis tags | `axis_tags :764-790`, `draw_overlay :816-837` | Horizontal line, horizontal ray and cross line always tag the price axis. Trend line, ray, extended line and rectangle tag their point prices only while selected |
| Fib retracement | `fib :917-948`, `FIB_LEVELS :43-55` | 11 TV levels, bands, "level (price)" labels on the left, dashed trend line |
| Measurers | `measurer :962-1033` | Box, arrows, label with Δprice (%) and bars + duration |
| Floating toolbar | `tt-chart-gpui/src/chart.rs:595-751` | Tool name, 12-swatch color, width 1-4, style, lock, delete |
| Left toolbar | `crates/tradeterm/src/drawing_toolbar.rs:158-349` | Cursor, Lines flyout, Fib, Rectangle, Measurers flyout, magnet, keep drawing, lock all, hide all, remove all |
| Keys | `crates/tradeterm/src/app.rs:1051-1072` | Esc, Delete/Backspace, Alt shortcuts (`drawing_toolbar.rs:56-67`) |
| Persistence | `crates/tt-session/src/settings.rs:148`, `tt-session/src/tab.rs:139-160` | Drawings per symbol (display symbol), shown on every timeframe |

### 1.2 Missing or half-done (concrete)

**Model and persistence**
1. No property model beyond `color/width/style/locked` (`drawings.rs:204-217`). None of the
   per-tool settings in section 3 can be stored.
2. `color` and `width` have no `#[serde(default)]` (`drawings.rs:211-212`), and `DrawingTool` is
   a plain enum. A drawing that fails to parse (for example a tool name written by a newer
   build) fails the whole `AppSettings`. `read_settings` then **ignores the entire settings
   file** (`tt-session/src/settings.rs:297-302`), so tabs, styles and all drawings are lost.
   This must be fixed before new tools ship (section 6.3).
3. `Color32` is stored premultiplied (ecolor). That is fine for the opaque colors we use today,
   but it loses precision at low opacity. Opacity is about to become a first-class field, so a
   straight-alpha color type is needed (section 6.2).
4. No per-timeframe visibility, per-drawing hide, z-order, or name.
5. No templates and no "remember last used style" per tool: `Drawing::new` hard-codes defaults
   (`drawings.rs:220-234`, `default_color :168-174`).

**Interaction** (`drawings.rs`, `chart.rs`, `app.rs`)

6. Only one selection (`Drawings::selected: Option<u64>`, `drawings.rs:361`). There is no
   Ctrl+click multi-select and no area select.
7. `press`/`motion` take no modifiers (`drawings.rs:521,572`). That rules out Shift to
   constrain angles while drawing, Shift to constrain axis while dragging, Ctrl to toggle the
   magnet, and Ctrl+drag to clone.
8. Placement supports only 1 or 2 points (`points() :108-116`). It cannot place 3-point tools
   (parallel channel, Fib extension), N-point tools that finish on double-click or Esc (path,
   polyline), or freehand tools (brush).
9. There is one magnet mode (weak) and no strong mode (`:260-285`).
10. Double-clicking a drawing does nothing special. `mouse_down` hands it to `drawing_press`
    (`chart.rs:350`), which just reselects. There is no settings dialog.
11. There is no right-click menu. Only left-button handlers are registered (`chart.rs:553-564`).
12. Missing keys (`app.rs:1053-1066`): Ctrl+Z/Y, Ctrl+C/V, arrow-key nudge, Alt+A alert,
    Shift+click quick measure.
13. There is no undo history. `dirty` only drives saving (`drawings.rs:371,412`).
14. Drawings live only on the price pane (`tt-chart/src/view.rs:451-459`). That is fine for now
    (BACKLOG item).

**Rendering gaps for existing tools**

15. Trend, ray and extended lines have no line ends, middle point, price-label toggle, stats,
    text, or extend-left/right options. Ray and extended line are separate tools that
    hard-code their extension (`:853-860`).
16. Horizontal line, horizontal ray and cross line always show the axis tag (`:764-790`). There
    is no text label. The vertical and cross lines have no time-axis tag. The vertical line
    always spans all panes (`:872-882`) with no "Extend" toggle.
17. Rectangle: fill is fixed at 20 % (`:896`). There is no separate border and background
    color, no middle line, no extend, no text, and only 2 corner handles (BACKLOG). Shift does
    not draw a square.
18. Fib retracement:
    - Levels are a `const` (`:43-55`) and cannot be edited, turned on or off, or reversed.
    - The band alpha is a hard-coded 0.12 (`:933`). TV draws 20 %, i.e. transparency 80.
    - Labels are always "level (price)" on the left (`:937-944`).
    - The trend line style is hard-coded (`:947`).
    - There are no extend-lines, percent-levels or log-scale options.
19. Measurers:
    - Red-for-down is hard-coded (`:965-969`).
    - There is no stats checklist (ticks/pips, volume), no border or label-background
      settings, and no extend.
    - `measurer_rect` ignores the tool (`:727-729`).
    - `format_span` has no seconds and no weeks (`:1049-1061`).
20. The palette has 12 swatches (`:58-71`). There is no opacity and no custom color.

**Floating toolbar** (`chart.rs:595-751`)

21. It is pinned to the pane's top right (`:718-726`), not draggable, and does not remember its
    position.
22. It has no template, text color, fill/background color, settings gear, alert, or "…" menu.
23. Its width/style buttons show text ("2px", "Solid") instead of the line glyphs TV uses.

**Left toolbar** (`drawing_toolbar.rs`)

24. Only 4 groups. Missing: info line, trend angle, parallel channel, Fib extension, the shapes
    and text tools, and long/short position.
25. The magnet button has no weak/strong dropdown, and the toggles are not persisted (BACKLOG).
26. The flyout list is hard-coded per group (`:216-245`).

**Canvas** (`crates/tt-chart/src/render/mod.rs:11-90`)

27. No polygon fill (polyline, triangle, brush background, channel band), no ellipse, no
    italic text (`text_styled` only has `bold`, `:58-66`), no rotated text (vertical-line text
    orientation), and no arrow-head primitive (the arrow is hand-drawn in `drawings.rs:1035`).

---

## 2. Shared field vocabulary

All tables in section 3 use these types.

| Type | Values | Notes |
|---|---|---|
| `Color` | RGBA, straight alpha | The picker shows TV's swatch grid, "+" for a custom color, and an **Opacity** slider (0-100 %) **[ui]**. Stored as `#RRGGBBAA` |
| `Width` | 1, 2, 3, 4 px | TV offers exactly 4 widths **[ui]**. The picker draws lines of each width |
| `LineStyle` | Solid, Dashed, Dotted | TV-lib also has `LINESTYLE_LARGE_DASHED = 3` **[doc]**, but the TV.com UI shows 3 styles **[ui]** |
| `LineEnd` | Normal, Arrow | TV-lib also defines `LINEEND_CIRCLE` **[doc]**. The TV.com "Left end / Right end" dropdowns show Normal and Arrow **[doc: trend line article]** |
| `Line` | `Color` + `Width` + `LineStyle` | TV renders this as one row: a color swatch button and a width/style button **[ui]** |
| `Extend` | Don't extend, Extend left, Extend right, Extend both | Dropdown in TV's trend angle dialog **[doc]**. Other tools use two checkboxes ("Extend left", "Extend right"). Both map to `{left: bool, right: bool}` |
| `FontSize` | 10, 11, 12, 14, 16, 20, 24, 28, 32, 40 | **[ui]** |
| `TextStyle` | `Color`, `FontSize`, Bold, Italic | TV shows these as one row: color swatch, size dropdown, **B** and *I* toggles **[doc]** |
| `HAlign` / `VAlign` | Left/Center/Right; Top/Middle/Bottom | "Text alignment" is two dropdowns **[doc]** |
| `StatsPosition` | Left, Center, Right (trend angle adds Auto) | TV-lib: `STATS_POSITION_LEFT=0, CENTER=1, RIGHT=2` **[doc]**. Auto is from the trend angle article **[doc]** |
| `Stats` | Checklist: Price range, Percent change, Change in pips, Bars range, Date/time range, Distance, Angle (+ Volume for range tools) | TV shows these as a "Stats" dropdown checklist **[doc]** |
| `Coordinate` | `Bar #` (integer, relative bar index) + `Price` (number at symbol precision) | Coordinates tab, one row per point ("Price 1", "Price 2", …) **[doc]** |
| `Visibility` | Section 2.2 | |

### 2.1 Text tab (common form)

Used by line tools, horizontal/vertical lines, rectangle, channels and ranges.

| Field | Type | Default |
|---|---|---|
| Text (enable) | checkbox | off. TV-lib `showLabel:false` **[doc]**. The checkbox turns on as soon as text is typed **[ui]** |
| Text | multi-line text area | empty |
| Text style | `TextStyle` | color = the tool's line color, 14 (12 for horizontal lines and trend angle, TV-lib **[doc]**), not bold, not italic |
| Text alignment | `VAlign` + `HAlign` | per tool (TV-lib `vertLabelsAlign` / `horzLabelsAlign`) |

Text can also be edited on the chart: click the text, type **[doc]**.

### 2.2 Visibility tab (common to every tool)

TV shows one row per interval unit: **checkbox**, **min** number input, **range slider**, **max**
number input **[ui]**. Defaults are every row on with its full range.

| Row | Range | Default |
|---|---|---|
| Ticks | 1-1000 | on. We have no tick charts: **omit** |
| Seconds | 1-59 | on, 1-59 |
| Minutes | 1-59 | on, 1-59 |
| Hours | 1-24 | on, 1-24 |
| Days | 1-366 | on, 1-366 |
| Weeks | 1-52 | on, 1-52 |
| Months | 1-12 | on, 1-12 |
| Ranges | checkbox only | on. Applies to range-bar charts, which we don't have: **omit** |

Visibility test: a drawing is visible when the chart interval's unit row is on and the
interval's multiplier is inside `[min, max]`. Renko and line-break charts use their underlying
interval.

Quick presets, in the context menu and the "…" menu under **Visibility on intervals ▸**
**[doc: blog 34613; names [ui]]**:
- Current interval and above
- Current interval and below
- Current interval only
- All intervals

Each preset just rewrites the rows.

### 2.3 Coordinates tab (common form)

One row per anchor: `#N: [Bar # input] [Price input]`. TV shows "Price 1", "Price 2"
**[doc]**. Some tools differ:
- Horizontal line: Price only.
- Vertical line: Bar # only.
- Trend angle: Price 1 + Angle.
- Parallel channel: #1, #2 + "Price offset".

`Bar #` is the bar index in the current series. Converting it to `time_ms` uses
`SlotTimes::time_at_frac`, so a bar number past the last bar extrapolates by the interval, the
same way `Mapper::anchor` does today.

---

## 3. Per-tool specification

Each tool lists:
- **Place**: how it is placed. "click-click" also allows drag-to-place, which we have today.
- **Handles**: the anchors shown when it is selected.
- **Tabs**: the tabs of its settings dialog.
- The fields of each tab, with type and default.
- **TB**: its floating-toolbar buttons (section 4.1).

Every tool also has a Visibility tab (2.2). Its rows are not repeated here.

### 3.1 Trend line (TV-lib `linetooltrendline`)

Place: click-click. Shift snaps the angle to multiples of 45° **[doc]**.
Handles: 2 endpoints. With "Middle point" on, the midpoint is drawn but is not a handle **[?]**.
Tabs: Style, Text, Coordinates, Visibility.

**Style**

| Field | Type | Default (TV.com) | TV-lib key / default |
|---|---|---|---|
| Line | `Line` | `#2962FF`, 2 px, solid **[ui]** | `linecolor` teal, `linewidth 1`, `linestyle 0` |
| Left end | `LineEnd` | Normal | `leftEnd 0` |
| Right end | `LineEnd` | Normal | `rightEnd 0` |
| Extend | `Extend` (two checkboxes: Extend left / Extend right) | none | `extendLeft/Right false` |
| Middle point | checkbox | off | `showMiddlePoint false` |
| Price labels | checkbox (both points' prices tagged on the price axis) | off | `showPriceLabels false` |
| Stats | `Stats` checklist: Price range, Percent change, Change in pips, Bars range, Date/time range, Distance, Angle | all off | `showPriceRange`, `showBarsRange`, `showDateTimeRange`, `showDistance`, `showAngle` all false. TV-lib has no separate percent/pips keys, but the TV.com UI lists them **[doc]** |
| Stats position | `StatsPosition` | Right | `statsPosition 2` |
| Always show stats | checkbox (off = stats only while selected) | off | `alwaysShowStats false` |

**Text**: section 2.1. Alignment defaults are horizontal Center, vertical Bottom (TV-lib
`horzLabelsAlign center`, `vertLabelsAlign bottom`). The text runs along the line, rotated to
the line's angle **[ui]**.

**Coordinates**: Price 1, Price 2.

**TB**: Template, Line color, Text color (when text is on **[?]**), Width, Style, Settings,
Alert, Lock, Remove, More.

Stats label contents. One block, each line present when its stat is on **[ui]**; the exact
punctuation is **[?]**:
1. `{Δprice} ({Δ%}) {Δpips}`. Price range, percent change and pips are each optional.
   "Pips" means Δprice / tick size.
2. `{N} bars, {span}`. Bars range and date/time range.
3. `{distance}`. **[?]** TV does not document the unit. Proposal: hide this option until it is
   verified.
4. `{angle}°`. Screen-space angle, so it changes with zoom **[ui]**.

Position: next to the second point (Right), the first point (Left), or the midpoint (Center).

### 3.2 Ray (`linetoolray`)

Same as the trend line, with **Extend right = on** by default **[doc: TV-lib]**. The Ray is a
preset of the trend line model: changing Extend in its dialog is allowed.

### 3.3 Extended line (`linetoolextended`)

Same as the trend line, with **Extend left = on** and **Extend right = on** **[doc: TV-lib]**.

### 3.4 Info line (`linetoolinfoline`)

Same fields as the trend line, with these defaults **[doc: TV-lib]**:
- Stats: Price range, Percent change, Pips, Bars range, Date/time range, Distance and Angle
  all **on**.
- Stats position **Center**.
- Always show stats **on**.

The help article describes the info box as showing absolute price change, percent change,
distance in bars, distance in time, and angle **[doc]**. Its Text tab has text color, opacity,
font size, bold and italic **[doc]**.
Place: click-click. Handles: 2.

### 3.5 Trend angle (`linetooltrendangle`)

Place: click-click. Handles: 2.

Rendering:
- A horizontal reference line from point 1.
- An arc between the reference line and the trend line.
- A label `{angle}°` at the arc **[ui]**. The angle is in screen space.

**Style** **[doc]**:

| Field | Type | Default |
|---|---|---|
| Line | `Line` | `#2962FF`, 2 px, solid |
| Extend | `Extend` dropdown (none / left / right / both) | none |
| Middle point | checkbox | off |
| Price labels | checkbox | off |
| Stats | checklist: Price range, Percent change, Change in pips, Bars range | all off |
| Stats position | Left / Center / Right / Auto | Right (TV-lib) |
| Always show stats | checkbox | off |

**Text** **[?]**: the article describes none. TV-lib has `textcolor` and `fontsize 12`. That
font size applies to the angle label. Proposal: a TextStyle row for the angle label only.

**Coordinates** **[doc]**: Price 1 (bar + price), **Angle** (degrees). Editing the angle keeps
the screen length and rotates point 2.

### 3.6 Horizontal line (`linetoolhorzline`)

Place: one click. Handles: 1, at the clicked time.

**Style**:

| Field | Type | Default |
|---|---|---|
| Line | `Line` | `#2962FF`, 2 px, solid **[ui]** (TV-lib: 1 px teal) |
| Price label | checkbox (tag on the price axis) | **on** (TV-lib `showPrice true`) |

**Text**: section 2.1. Defaults: font size 12, horizontal Center, vertical **Top**. TV-lib uses
`vertLabelsAlign top`; the text sits above the line.

**Coordinates**: Price.

**TB**: Template, Line color, Text color, Width, Style, Settings, Alert, Lock, Remove, More.

### 3.7 Horizontal ray (`linetoolhorzray`)

Same as the horizontal line. Coordinates: **Price + Bar #** (the ray's start) **[doc]**.

### 3.8 Vertical line (`linetoolvertline`)

Place: one click. Handles: 1.

**Style** **[doc]**:

| Field | Type | Default |
|---|---|---|
| Line | `Line` | `#2962FF`, 2 px, solid **[ui]** |
| Extend | checkbox. On: the line runs through the indicator panes. Off: price pane only | on (TV-lib `extendLine true`) |
| Time label | checkbox (tag on the time axis) | on (TV-lib `showTime true`) |

**Text**: section 2.1, plus **Text orientation**: Horizontal / Vertical, default **Vertical**
(TV-lib `textOrientation vertical`). Alignment defaults: horizontal **Right**, vertical **Top**
(TV-lib). Vertical orientation means the text is rotated 90° along the line. The Canvas needs
rotated text for this (gap 27).

**Coordinates**: Bar #.

### 3.9 Cross line (`linetoolcrossline`)

Place: one click. Handles: 1.

**Style** **[doc]**:
- Line (`Line`).
- Price label: on (TV-lib `showPrice true`).
- Time label: on (TV-lib `showTime true`).

**Text**: none **[doc: not mentioned; TV-lib has no text keys]**.

**Coordinates**: Price + Bar #.

### 3.10 Parallel channel (`linetoolparallelchannel`)

Place: 3 clicks.
1. The first click and the second click (or a drag) set line 0.
2. Moving the pointer then offsets the parallel line.
3. The third click fixes the offset **[ui]**.

Shift snaps line 0 to 45° **[doc]**.

Handles: 6 **[ui]**:
- the 2 endpoints of line 0;
- the 2 endpoints of line 1, which move line 1's offset and the matching endpoint;
- 2 midpoints, one on each line, which move that line parallel to itself.

**Style**:

| Field | Type | Default |
|---|---|---|
| Channel's levels | list of rows `[on] [ratio] [Line]` | Rows **[?]**: 0 (on), 1 (on), 0.5 middle (on, dashed). Additional levels (off): −0.25, 0.25, 0.75, 1.25 **[?]** |
| Extend left line | checkbox | off |
| Extend right line | checkbox | off |
| Background | checkbox + `Color` | on, `#2962FF` at 20 % (TV-lib `backgroundColor rgba(41,98,255,0.2)`, `transparency 20`) |

TV-lib keys: `linecolor`, `linewidth`, `linestyle`, `extendLeft`, `extendRight`,
`fillBackground true`, `backgroundColor`, `showMidline true`, `midlinecolor`,
`midlinewidth 1`, `midlinestyle 2` (dashed) **[doc]**. The newer TV.com UI turned the midline
into a "levels" list with per-level ratios **[doc: "custom ratio can be entered"]**. Model the
levels list, and treat the midline as level 0.5.

**Text**: section 2.1 **[doc: "How to add text to channel drawing tools"]**.

**Coordinates**: #1, #2 (bar + price), **Price offset** (price distance of line 1 from line 0)
**[doc]**.

**TB**: Template, Line color, Background color, Width, Style, Settings, Lock, Remove, More.

### 3.11 Fib retracement (`linetoolfibretracement`)

Place: click-click. Handles: 2.

Level placement, matching TV:
- Level 0 is at point 2, level 1 at point 1. Our code already does this (`drawings.rs:915`).
- With **Reverse** on, they swap.

**Style** **[doc: TV help 43000518158 + TV-lib]**:

| Field | Type | Default |
|---|---|---|
| Trend line | checkbox + `Line` | on, `#787B86`, 1 px, **dashed** (TV-lib `trendline {visible true, color rgba(120,123,134,1), linewidth 1, linestyle 2}`) |
| Levels line | `Width` + `LineStyle` (no color: each level has its own) | 1 px, solid (TV-lib `levelsStyle`). **[?]** TV.com may use 2 px |
| Extend lines | Extend left checkbox, Extend right checkbox | off, off (TV-lib `extendLinesLeft false`, `extendLines false` = right) |
| Levels list | 24 rows `[on] [coeff] [Color]`, shown in two columns **[ui]** | see the level table below |
| Use one color | color dropdown; applying it sets every level's color | n/a (action, not stored) |
| Background | checkbox + opacity | on, 20 % (TV-lib `fillBackground true`, `transparency 80`). Each band uses the color of its upper level **[ui]** |
| Reverse | checkbox | off |
| Prices | checkbox (price after the level label) | on (`showPrices true`) |
| Levels | checkbox + Values / Percents dropdown | on, Values (`showCoeffs true`, `coeffsAsPercents false`) |
| Labels | `HAlign` (Left/Center/Right) + `VAlign` (Top/Middle/Bottom) | Left, Bottom (`horzLabelsAlign left`, `vertLabelsAlign bottom`). Left means outside, left of the leftmost point (our current rendering) |
| Text | checkbox + text + position along the level | off **[doc]** |
| Font size | `FontSize` | 12 (`labelFontSize 12`) |
| Fib levels based on log scale | checkbox, enabled only when the chart is in log scale | off (`fibLevelsBasedOnLogScale false`) |

Default levels (24 slots, TV-lib `level1..level24 {coeff, color, visible}`):

| Coeff | On | Color |
|---|---|---|
| 0 | yes | `#787B86` |
| 0.236 | yes | `#F23645` |
| 0.382 | yes | `#FF9800` |
| 0.5 | yes | `#4CAF50` |
| 0.618 | yes | `#089981` |
| 0.786 | yes | `#00BCD4` |
| 1 | yes | `#787B86` |
| 1.618 | yes | `#2962FF` |
| 2.618 | yes | `#F23645` |
| 3.618 | yes | `#9C27B0` |
| 4.236 | yes | `#E91E63` |

The 11 visible levels match our `FIB_LEVELS`. The other 13 slots are off by default **[ui]**:
1.272, 1.414, 2, 2.272, 2.414, 3, 3.272, 3.414, 4, 4.272, 4.414, 4.618, 4.764. Their colors are
**[?]**; check them in TV, or reuse the palette cycle.

Log-scale semantics: with the option on, levels are interpolated in log space. That is what
`level_price` does today through `sc.to_scaled` (`drawings.rs:921-922`) whenever the chart is
in log mode. With the option off, levels must be interpolated **linearly in price**, even on a
log chart. Today's code effectively has the option permanently on.

**Coordinates**: Price 1, Price 2.

**TB**: Template, Line color (= "use one color" **[?]**), Width, Style, Settings, Lock, Remove,
More.

### 3.12 Trend-based Fib extension (`linetooltrendbasedfibextension`)

Place: 3 clicks: start of the move, end of the move, end of the correction **[doc]**.
Handles: 3. The trend line is drawn p1 → p2 → p3 (dashed).
Level price: `p3 + (p2 − p1) × coeff`, interpolated in scale space when log-based is on.
Levels span from p3's x to the rightmost point **[?]**.

**Style**: the same fields as the Fib retracement **[doc]**. Same TV-lib keys and defaults
(`showCoeffs`, `showPrices`, `fillBackground`, `transparency 80`, `extendLines`,
`extendLinesLeft`, `reverse`, `coeffsAsPercents`, `fibLevelsBasedOnLogScale`,
`labelFontSize 12`, `trendline`, `levelsStyle`, 24 levels).

Default visible levels **[?]**: use the retracement's list (0, 0.236, 0.382, 0.5, 0.618, 0.786,
1, 1.618, 2.618, 3.618, 4.236) until checked against TV.

**Coordinates**: Price 1, Price 2, Price 3.

### 3.13 Rectangle (`linetoolrectangle`)

Place: click-click. Shift draws a square (in screen points) **[doc]**.
Handles: 8 **[ui]**: 4 corners and 4 side midpoints. A side handle moves only that edge.
Hit test: the border, plus the interior when the background is on.

**Style** **[doc]**:

| Field | Type | Default |
|---|---|---|
| Border | `Line` | `#9C27B0`, 2 px **[ui]** (TV-lib `color #9c27b0`, `linewidth 1`), solid |
| Background | checkbox + `Color` | on, `#9C27B0` at 20 % (TV-lib `backgroundColor rgba(156,39,176,0.2)`) |
| Middle line | checkbox + `Line` (horizontal line at the mid price) | off, border color, 1 px, dashed **[?]** |
| Extend left | checkbox | off |
| Extend right | checkbox | off |

**Text** **[doc]**: section 2.1, with text inside the box. Alignment defaults: horizontal Left,
vertical Bottom (TV-lib); text color `#9C27B0`, size 14. **[?]** Whether "Bottom" means inside
or outside the box: TV-lib documents no "inside/outside" option, so Top/Middle/Bottom should be
read as inside the box.

**Coordinates**: Price 1, Price 2.

**TB**: Template, Border color, Background color, Width, Style, Settings, Alert **[?]**, Lock,
Remove, More.

### 3.14 Ellipse (`linetoolellipse`)

Place: click-click, giving the two corners of the bounding box **[?]**. Shift draws a circle
**[doc]**.
Handles **[?]**: 4 at the ellipse's left, right, top and bottom extremes; each resizes that
axis.

**Style** **[doc]**:
- Border: `Line`, default `#E91E63`, 2 px **[ui]** (TV-lib 1 px), solid.
- Background: checkbox + `Color`, default on, `#E91E63` at 20 % (TV-lib
  `rgba(233,30,99,0.2)`).

**Text** **[doc: "How do you add text to a circle or ellipse?"]**: text, TextStyle, text
centered by default **[?]**.

**Coordinates**: Price 1, Price 2.

### 3.15 Circle (TV-lib has no entry in v26; TV.com tool added later)

Place: the first click sets the center, the second click fixes the radius **[doc]**.
The radius is a screen distance stored as the anchor of an edge point. The circle stays round
only with "lock price to bar ratio"; otherwise it becomes an ellipse in screen space when the
scales change **[doc]**. Proposal: keep it round in screen space at any zoom by storing the
center plus an edge anchor, and taking the screen distance each frame.

Handles: center (moves the circle) and edge point (radius).

**Style**: Line (color, width, opacity) and Fill (color, opacity) **[doc]**. Defaults **[?]**:
`#FF9800`, 2 px, fill at 20 %. **Text**: as for the ellipse.

### 3.16 Brush (`linetoolbrush`)

Place: press, drag, release, which records every pointer sample. Smoothed with
`smooth 5` (TV-lib; a moving-average window of 5 points).
Handles **[?]**: none per point. The body can be dragged. Proposal: show the start and end
points as non-draggable markers when selected.

**Style** **[doc + TV-lib]**:

| Field | Type | Default |
|---|---|---|
| Line | `Line` | TV-lib `linecolor #00bcd4`, 1 px, solid. TV.com: 2 px **[?]** |
| Line ends **[?]** | Left end, Right end (`LineEnd`) | Normal, Normal (TV-lib `leftEnd`/`rightEnd`) |
| Background | checkbox + `Color` (fills the closed shape) | off, `#00BCD4` at 50 % (TV-lib `fillBackground false`, `backgroundColor rgba(0,188,212,0.5)`) |

No Coordinates tab. Point storage: simplify on release with Ramer-Douglas-Peucker at 0.5 pt
and cap the samples at 2,000, so huge strokes do not bloat settings.

### 3.17 Path (`linetoolpath`)

Place: click to add points (unlimited). Double-click the last point or press **Esc** to finish
**[doc]**. Handles: every vertex.

**Style** (TV-lib):
- Line: color, `lineWidth 2`, solid.
- Left end: Normal.
- Right end: **Arrow** (TV-lib `rightEnd 1`).

TV.com default color **[?]**: `#2962FF`.

**Coordinates**: one row per vertex **[?]**. Proposal: show them, because the existing form
scales to N rows.

### 3.18 Polyline (`linetoolpolyline`)

Place: click to add points. Clicking the first point closes the shape and finishes it.
Double-click, right-click or Esc finishes it as an open shape **[doc]**.
Handles: every vertex.

**Style** **[doc]**:
- Border: `Line`, TV-lib `#7E57C2`, 1 px. TV.com: 2 px **[?]**.
- Background: checkbox + `Color`, on, `#7E57C2` at 20 % (TV-lib `fillBackground true`). The
  fill applies only when the shape is closed (TV-lib `filled false` is the closed flag).

### 3.19 Arrow (`linetoolarrow`)

A trend line with **Right end = Arrow**. It has every trend line field, the same key set in
TV-lib, and the same defaults otherwise **[doc]**. Place: click-click. Handles: 2.

### 3.20 Text (`linetooltext`)

Place: one click at the anchor. An inline editor opens with a placeholder **[ui]**. Committing
empty text deletes the drawing **[?]**.
Handles: none. The body drags; the anchor is the text's top-left **[?]**.
The text has a fixed screen size (TV-lib `fixedSize true`).

**Text** tab (TV calls it the "Text property dialog") **[doc]**:

| Field | Type | Default |
|---|---|---|
| Text style | `TextStyle` | `#2962FF` **[ui]** (TV-lib teal), 14, regular |
| Text | text area | "" |
| Background | checkbox + `Color` | off, `rgba(91,133,191,0.3)` (TV-lib, transparency 70) |
| Border | checkbox + `Color` | off, `rgba(102,123,139,1)` |
| Text wrap | checkbox (+ width) | off, width 200 pt (TV-lib `wordWrap false`, `wordWrapWidth 200`) |

Coordinates: Price + Bar # **[?]** (TV-lib anchors to a point; the article does not mention the
tab). Proposal: include it.

Out of scope: TV also has "Anchored text", which is pinned to a screen position.

### 3.21 Note (`linetoolnote`)

Place: one click, "linked to a selected price level" **[doc]**. It shows a pin marker at the
anchor. Hovering or selecting shows the note bubble with its text **[ui]**. Edit: click the
text field **[doc]**.

**Text** tab (TV-lib keys):

| Field | Type | Default |
|---|---|---|
| Label (marker) color | `Color` | `#2962FF` (TV-lib `markerColor` teal) |
| Text style | `TextStyle` | `#FFFFFF`, 14, regular |
| Text | text area | "" |
| Background | `Color` | `rgba(41,98,255,0.7)` |
| Border | `Color` | marker color |

Coordinates: Price + Bar #.

### 3.22 Callout (`linetoolcallout`)

Place: two points **[doc]**:
- point 1 is the pointer tip on the chart;
- point 2 is the text container.

Handles: 2.

**Text** tab **[doc]**:

| Field | Type | Default (TV-lib) |
|---|---|---|
| Text style | `TextStyle` | `#FFFFFF`, 14, regular |
| Text | text area | "" |
| Background | `Color` | `rgba(0,151,167,0.7)` |
| Border | `Line` (color, opacity, width, style) | `rgba(0,151,167,1)`, 1 px, solid |
| Text wrap | checkbox (+ width) | off, 200 |

**Coordinates**: Price 1 (the point), Price 2 (the container) **[doc]**.

### 3.23 Price label (`linetoolpricelabel`)

Place: one click. The label shows the price at its anchor, which updates while it is dragged,
in a bubble pointing to the anchor **[doc]**.

**Style** **[doc]**:
- Text: color + font size + opacity. Default `#FFFFFF`, 14, **bold** (TV-lib
  `fontWeight bold`).
- Background: `Color`, TV-lib `rgba(21,119,96,1)`; TV.com uses `#2962FF` **[?]**.
- Border: `Color`, same as the background.

**Coordinates**: Price + Bar #.

### 3.24 Long position / 3.25 Short position (`linetoolriskrewardlong` / `…short`)

Place: one click at the entry (time and price) **[ui]**. The tool appears with default stop and
target distances and a default width **[?]**. Proposal:
- stop and target each 2 % of the visible price range from the entry (R:R 1.00);
- width 20 bars.

Handles (4) **[ui]**:
1. Entry (left edge, entry price): moves the entry price and start time.
2. Target (left edge, target price).
3. Stop (left edge, stop price).
4. Right edge (at the entry price): sets the end time.

Dragging the body moves everything.

Rendering **[ui]**:
- Target zone between the entry and the target, filled with the target color.
- Stop zone between the entry and the stop, filled with the stop color.
- Entry line.
- Once price has crossed the entry inside the tool's time span, a darker fill shows the
  realized move up to the exit or the last bar, plus a dashed line from the entry crossing to
  the exit.
- Three labels: target label beyond the target, stop label beyond the stop, center label at the
  entry. The center label turns from "Open" to "Closed" when the target or stop is hit inside
  the span.

**Inputs** tab **[doc: 43000517002, 43000475660]**:

| Field | Type | Default |
|---|---|---|
| Account size | number + currency (the symbol's quote currency) | 1000 (TV-lib `accountSize 1000`) |
| Lot size | number | 1 (`lotSize 1`) |
| Risk | number + unit dropdown **%** / **currency** | 25, % (`risk 25`, `riskDisplayMode percentage`) |
| Leverage | number | 1 **[?]**. The newer TV.com article lists the field; TV-lib v26 has no key |
| Entry price | price | the clicked price |
| Profit level | Ticks ⇄ Price (two linked inputs) | from placement |
| Stop level | Ticks ⇄ Price | from placement |
| Qty precision | dropdown: Default, 0 … 8 decimals **[?]** | Default (symbol quantity step) |

**Style** tab **[doc]**:

| Field | Type | Default |
|---|---|---|
| Lines | `Color` + `Width` (entry, target and stop lines) | `#787B86`, 1 px |
| Stop color | `Color` | `rgba(242,54,69,0.2)` |
| Target color | `Color` | `rgba(8,153,129,0.2)` |
| Text | color + `FontSize` | `#FFFFFF`, 12 |
| Compact stats mode | checkbox | off (`compact false`) |
| Price labels | checkbox (entry, target and stop on the price axis) | on (`showPriceLabels true`) |
| Always show stats | checkbox (off = labels only while selected) | off |

TV-lib also has `fillLabelBackground true`, `labelBackgroundColor rgba(88,88,88,1)`,
`drawBorder false`, `borderColor`. Use them as the center label's neutral background. On
TV.com the target and stop labels are green and red **[ui]**.

Coordinates: none; the prices are in Inputs **[?]**.

Formulas, **Long** **[doc]**:

```
RiskSize = Risk (currency)            | Risk/100 × AccountSize (%)
QtyRisk  = RiskSize / ((Entry − Stop) × PointValue) / LotSize
QtyLvg   = AccountSize × Leverage / Entry × PointValue / LotSize
Qty      = min(QtyRisk, QtyLvg), rounded down to Qty precision
TargetΔ  = Profit − Entry;  Target%  = TargetΔ / Entry × 100;  TargetTicks = TargetΔ / tick
StopΔ    = Entry − Stop;    Stop%    = StopΔ / Entry × 100;    StopTicks   = StopΔ / tick
R:R      = (Profit − Entry) / (Entry − Stop)
Balance at TP = AccountSize + (Profit − Entry) × Qty × PointValue × LotSize
Balance at SL = AccountSize − (Entry − Stop) × Qty × PointValue × LotSize
Open P&L:  no bars in span (history) → RightEdgeClose − Entry
           span in the future         → LastPrice − Entry
Closed P&L: TP hit → TargetΔ; SL hit → −StopΔ   (first hit inside the span wins)
```

**Short** is the mirror image (swap the signs) **[doc]**. Point value is 1 for Binance spot.
The tick size comes from `tt-core/src/symbol_info.rs`.

Label text. Wording is **[ui]**, exact punctuation **[?]**:
- Target: `Target: {TargetΔ} ({Target%}%) {TargetTicks}, Amount: {profit}`
- Center (open): `Open P&L: {pnl}, Qty: {qty}` / `Risk/Reward Ratio: {rr}`
- Center (closed): `Closed P&L: {pnl}, Qty: {qty}` / `Risk/Reward Ratio: {rr}`
- Stop: `Stop: {StopΔ} ({Stop%}%) {StopTicks}, Amount: {loss}`
- Compact mode keeps only the numbers. Proposal: `{Target%}% · {profit}`.

Context menu extra: **Reverse position** (long ⇄ short, mirror the target and stop around the
entry) **[doc: 43000744866]**. Alerts are out of scope (BACKLOG).

### 3.26 Price range (`linetoolpricerange`) / 3.27 Date range (`linetooldaterange`) / 3.28 Date and price range (`linetooldateandpricerange`)

Place: click-click. Handles: 2 (the corners at the anchor points) **[?]**.

Rendering:
- A shaded box with arrows along the measured axes, as today.
- A label below the box when measuring down and above it when measuring up, as today.
- TV.com tints the box and label red when measuring down for the price-measuring tools
  **[ui]**. Our code does this already.

**Style** **[doc: 43000516996 + TV-lib]**:

| Field | Type | Default |
|---|---|---|
| Line | `Color` + `Width` (the arrows and the edge lines) | `#2962FF`, 1 px |
| Border | checkbox + `Line` | off, `#2962FF` at 90 %, 1 px (TV-lib `drawBorder false`, `borderColor rgba(41,98,255,0.9)`, `borderWidth 1`) |
| Background | checkbox + `Color` | on, `#2962FF` at 20 % (TV-lib `fillBackground true`, `backgroundColor rgba(41,98,255,0.2)`) |
| Extend | Price range: Extend left/right. Date range: Extend top/bottom. Date and price range: none | off |
| Stats | checklist. Price range: Price range, Percent change, Change in pips. Date range: Bars range, Date/time range, Volume. Date and price range: all six **[doc + ?: the article does not say which options each tool gets]** | all on except Volume |
| Label | `TextStyle` (color, opacity, size) | `#FFFFFF`, 12 |
| Label background | checkbox + `Color` | on, `#2962FF` at 90 % (TV-lib `labelBackgroundColor rgba(41,98,255,0.9)`) |

**Text** **[doc: "How to add text to range tools?"]**: TextStyle + text, shown with the box.
**Coordinates**: Price 1, Price 2.
**Volume** = the sum of `volume` over the bars inside the range (needs `CandleSeries` volumes;
O(n) at render, cached per drawing).

---

## 4. Shared interaction

### 4.1 Floating toolbar (selected drawing)

Behaviour **[ui]**:
- One toolbar per chart, shown while one or more drawings are selected.
- It is **draggable** by its grip. Its position is remembered across selections and restarts
  (stored in app settings, as a fraction of the chart size so it survives resizes).
- It is hidden while placing or dragging a drawing (we already do this, `chart.rs:607`).

Buttons in order **[ui]**; each button appears only when the selected tool has that property:

| # | Button | Content |
|---|---|---|
| 1 | Grip | Drag handle |
| 2 | Template ▾ | Section 5 menu |
| 3 | Line / border color | Pencil icon with a color bar; palette + opacity |
| 4 | Background color | Bucket icon (shapes, channels, ranges, position) |
| 5 | Text color | "T" icon (tools with text) |
| 6 | Font size | Text tools only **[?]** |
| 7 | Line width | Glyph of the current width; menu of 1-4 px lines |
| 8 | Line style | Glyph; menu of solid, dashed, dotted |
| 9 | Settings ⚙ | Opens the dialog (same as double-click) |
| 10 | Alert | Out of scope; leave a slot (BACKLOG) |
| 11 | Lock / Unlock | Toggles `locked` |
| 12 | Remove 🗑 | Deletes the selection |
| 13 | More … | Menu below |

With a multi-selection, the toolbar shows the buttons common to all selected drawings, and an
edit applies to all of them (TV "change the color of multiple tools at once" **[doc]**).

The **More "…" menu** **[doc for visibility (blog 34613), rest [ui]/[?]]**:
- Visual order ▸ Bring to front / Bring forward / Send backward / Send to back
- Visibility on intervals ▸ (presets, 2.2)
- Clone
- Copy (Ctrl + C)
- Hide
- Object tree… (later slice)

### 4.2 Right-click context menu on a drawing **[ui]**

Right-click selects the drawing if it is not selected, then shows:

```
Settings…
───────────────
Template ▸           (section 5)
Visual order ▸       Bring to front / Bring forward / Send backward / Send to back
Visibility on intervals ▸
───────────────
Clone                Ctrl + drag
Copy                 Ctrl + C
───────────────
Lock / Unlock
Hide
Remove               Delete
───────────────
Reverse position     (long / short only)
```

TV also has "Add alert" (Alt + A) and, in newer builds, "Move to ▸" / "Pin to scale ▸". These
are out of scope; note them in BACKLOG.

Right-click on empty chart space gets two new entries: **Paste** (Ctrl + V) when the clipboard
holds drawings, and **Remove drawings**.

### 4.3 Mouse

| Gesture | Behaviour | Source |
|---|---|---|
| Hover | The drawing's handles show and the cursor becomes a pointer. We have this; keep it | [ui] |
| Click | Selects it and clears other selections | [ui] |
| Ctrl + click | Adds or removes it from the selection | [doc 43000537219] |
| Ctrl + drag on empty space | Rubber-band select of the drawings it intersects | [doc 43000537219: "select a required area"] |
| Drag body | Moves the drawing. Multi-selection moves all | [doc] |
| Shift + drag body | Moves strictly horizontally or vertically (axis chosen by the first 4 pt of motion) | [doc 43000538248] |
| Ctrl + drag body | **Clones**: the copy follows the pointer and the original stays | [doc 43000537251] |
| Drag handle | Moves that anchor. Tool rules apply: Shift = 45° for lines and channels, square for rectangles, circle for ellipses | [doc] |
| Double-click | Opens the settings dialog. For text, note and callout, double-click on the text starts inline editing **[?]** | [doc] |
| Right-click | Context menu (4.2). While placing a polyline it finishes the shape | [doc] |
| Shift while placing | 45° snap for trend-like lines and the channel base; square or circle for rectangle, ellipse and circle | [doc] |
| Ctrl while placing or editing | Inverts the magnet for that gesture (off → strong, on → off) | [doc 43000722509, blog 14315] |

### 4.4 Keyboard

| Key | Action | Source |
|---|---|---|
| Delete / Backspace | Remove the selection | have |
| Esc | Finish path/polyline; else cancel placing; else release the tool; else deselect | [doc path], have |
| Ctrl + Z / Ctrl + Y (also Ctrl + Shift + Z) | Undo / redo of drawing changes | [ui] |
| Ctrl + C / Ctrl + V | Copy the selection / paste onto the current chart at the same anchors, then select the pasted copies **[?: offset]** | [doc 43000537219 "copy"] |
| ← → | Nudge the selection one bar | [doc 43000538248] |
| ↑ ↓ | Nudge the selection one tick (symbol min tick) | [doc 43000538248] |
| Alt + T / H / J / V / C / F, Alt + Shift + R | Tool shortcuts | have |
| Shift + click on the chart | Quick measure (temporary date and price range) | BACKLOG |

### 4.5 Magnet

TV has two modes plus a "Snap to indicators" option **[doc: support page via search]**.
- **Weak magnet** snaps to the nearest O/H/L/C of the bar under the pointer only within a
  distance threshold, so free placement is still possible. Today's behaviour, 30 pt.
- **Strong magnet** always snaps to the nearest O/H/L/C of the bar under the pointer, so
  drawings "attach" to prices.
- **Snap to indicators**: overlay indicator values become snap targets too. Later slice.

The left-toolbar magnet button gets a flyout: Weak / Strong / Snap to indicators. Ctrl inverts
it per gesture (4.3). The magnet applies to placing points and to dragging handles (and to
neither body drags nor nudges) **[ui]**.

### 4.6 Selection handles per tool (summary)

| Tool | Handles |
|---|---|
| Trend line, ray, extended line, info line, arrow, trend angle | 2 endpoints |
| Horizontal line, horizontal ray, vertical line, cross line, price label, note, text | 1 anchor (text: none, body only **[?]**) |
| Parallel channel | 4 endpoints + 2 line midpoints |
| Fib retracement | 2 |
| Fib extension | 3 |
| Rectangle | 4 corners + 4 side midpoints |
| Ellipse | 4 extremes **[?]** |
| Circle | center + edge |
| Brush | none (body only) |
| Path, polyline | every vertex |
| Callout | 2 (tip, box) |
| Long / short | entry, target, stop, right edge |
| Ranges | 2 corners |

Handle look: keep ours, a hollow circle in the line color with radius 5 (`drawings.rs:840-845`).
TV uses the same idea **[ui]**.

---

## 5. Templates

### 5.1 Behaviour

- The **Template ▾** menu is in two places: the settings dialog footer (left) and the floating
  toolbar **[doc: third-party guide + blog 33205]**. Contents:

  ```
  Save Drawing Template As…
  Apply Default Drawing Template
  ───────────────
  <named template 1>      ×
  <named template 2>      ×
  ```

- **Save As…** asks for a name. An existing name asks to overwrite **[doc: third-party
  guide]**.
- Clicking a named template applies it. **×** deletes it after confirmation.
- **Apply Default Drawing Template** resets the drawing's style to the tool's factory
  defaults (section 3).
- **Last-used style.** TV remembers the last style used per tool: after you recolor a trend
  line, the next trend line is drawn in that color **[ui]**. Spec: every committed style edit
  (dialog Ok, or a floating-toolbar change) writes the drawing's style to the tool's
  `default`. New drawings start from `default` when it is set, and from factory defaults
  otherwise. "Apply Default Drawing Template" applies the factory defaults **and** clears
  `default`. **[?]** Whether TV clears its remembered style here. Our choice: yes, so the user
  has a way back.
- **Multi-selection** of drawings of the same type: Template applies to all of them **[doc:
  blog 33205]**. With mixed types, the Template button is hidden.
- Templates are **per tool**. A trend line template does not appear for rays. **[?]** TV may
  share templates between trend line, ray and extended line; check.

### 5.2 What a template contains

A template contains the drawing's **style properties** (`Props`, section 6): lines, fills,
text *formatting*, stats toggles, Fib levels, extend options, and position Inputs (account
size, risk, lot size, leverage, qty precision).

It **excludes** points/coordinates, text *content*, locked, hidden, visibility and z-order.
**[?]** TV may include text content and visibility; excluding them is the safer choice. Only
the prices of the position tool's entry, target and stop are coordinates.

### 5.3 Storage (app settings)

```jsonc
// AppSettings (tt-session/src/settings.rs), new field, #[serde(default)]
"drawing_templates": {
  "TrendLine": {
    "default": { /* Props as JSON, overlay */ },
    "named":   [ { "name": "Support", "props": { /* … */ } } ]
  },
  "FibRetracement": { "default": null, "named": [] }
}
```

- Keys are `DrawingTool` variant names. Unit variants serialize as strings, which serde_json
  accepts as map keys.
- `props` is kept as `serde_json::Value` and applied as an **overlay** onto the tool's factory
  defaults (6.3). Templates saved by an older build therefore still apply after fields are
  added, and unknown keys are ignored.
- Templates are app-wide, not per symbol or per tab, like TV's account-wide templates.

---

## 6. Data model

### 6.1 Requirements

- Typed and allocation-free at render time (performance north star). This rules out a stringly
  property bag on the hot path.
- Adding a field later must not break saved files in either direction:
  - forward: an old file read by a new build;
  - backward: a new file read by an old build, which matters while users downgrade.
- One unknown or broken drawing must never discard the settings file (gap 2).
- Templates must be the same data as the props.

### 6.2 Proposal: typed `Props` per tool family + an overlay deserializer

```rust
// tt-chart/src/drawings/model.rs
pub struct Drawing {
    #[serde(skip, default = "next_id")] pub id: u64,
    pub tool: DrawingTool,
    pub points: Vec<Anchor>,
    pub props: Props,                      // style (= template payload)
    #[serde(default)] pub text: String,    // text content (not in templates)
    #[serde(default)] pub locked: bool,
    #[serde(default)] pub hidden: bool,
    #[serde(default)] pub visibility: Visibility,
    // z-order = position in Drawings::items (no field)
}

/// Straight-alpha color, serialized as "#RRGGBBAA".
pub struct Rgba(pub [u8; 4]);

pub struct Stroke { pub color: Rgba, pub width: u8 /*1..=4*/, pub style: LineStyle }
pub struct Fill   { pub on: bool, pub color: Rgba }
pub struct TextStyle { pub color: Rgba, pub size: u8, pub bold: bool, pub italic: bool }
pub struct TextLayout { pub on: bool, pub h: HAlign, pub v: VAlign, pub vertical: bool }
pub struct Ends   { pub left: LineEnd, pub right: LineEnd }
pub struct Extend { pub left: bool, pub right: bool }
pub struct Stats  { pub price_range: bool, pub percent: bool, pub pips: bool, pub bars: bool,
                    pub date_time: bool, pub distance: bool, pub angle: bool, pub volume: bool,
                    pub position: StatsPosition, pub always: bool }
pub struct Level  { pub coeff: f64, pub color: Rgba, pub on: bool, pub stroke: Option<Stroke> }

#[serde(tag = "kind")]
pub enum Props {
    Line(LineProps),          // trend, ray, extended, info, trend angle, arrow
    HLine(HLineProps),        // horizontal line / ray
    VLine(VLineProps),
    Cross(CrossProps),
    Channel(ChannelProps),    // parallel channel (+ future flat / disjoint)
    Fib(FibProps),            // retracement, trend-based extension (+ future fib channel)
    Shape(ShapeProps),        // rectangle, ellipse, circle, polyline
    Brush(BrushProps),
    Path(PathProps),
    TextBox(TextBoxProps),    // text, note, callout, price label (variant flags)
    Position(PositionProps),  // long / short (inputs + style)
    Range(RangeProps),        // price / date / date-and-price range
}
pub struct LineProps { pub line: Stroke, pub ends: Ends, pub extend: Extend,
                       pub middle_point: bool, pub price_labels: bool, pub stats: Stats,
                       pub text_style: TextStyle, pub text: TextLayout }
// …one struct per family, each #[serde(default)] with Default = the family default.
```

- `Props::defaults(tool) -> Props` gives the per-tool factory values (section 3), e.g. the Ray
  is `LineProps { extend: right, .. }`.
- Accessors used by the floating toolbar and multi-edit:
  - `primary_stroke_mut()`
  - `fill_mut()`
  - `text_style_mut()`
  - `capabilities() -> Caps` (bitflags: LINE, FILL, TEXT, …), which decides which toolbar
    buttons show.
- Each family module owns `draw`, `hit` and `handles` for its props, so engineers on different
  tools touch different files.
- `Visibility { seconds: Span, minutes: Span, hours: Span, days: Span, weeks: Span,
  months: Span }`, with `Span { on: bool, min: u16, max: u16 }`.

### 6.3 Serde compatibility and robustness

1. **Migration of old drawings.** Old drawings are `{tool, points, color, width, style?,
   locked?}`. Deserialize through `#[serde(from = "DrawingRepr")]`:
   - `DrawingRepr` has `props: Option<serde_json::Value>` and optional legacy `color`
     (`Color32` array), `width`, `style`.
   - If `props` is missing, start from `Props::defaults(tool)` and set the primary stroke from
     the legacy fields.
   - For the Fib retracement, the legacy `color` was only the trend line color
     (`drawings.rs:947`), so map it to `trendline.color`.
2. **Overlay.** When `props` is present, merge the saved JSON over
   `serde_json::to_value(Props::defaults(tool))` (recursive object merge), then deserialize.
   Fields missing from old files get the *tool's* defaults, not the family's, and unknown keys
   are ignored. This runs only at load or template-apply time, never per frame.
3. **Downgrade safety.** Serialize via `#[serde(into = "DrawingRepr")]` and keep writing the
   legacy `color`, `width` and `style`, mirrored from the primary stroke, for at least 2
   releases. Today's builds require `color` and `width`.
4. **Lenient lists.** Replace `BTreeMap<String, Vec<Drawing>>` (`settings.rs:148`) with
   `BTreeMap<String, DrawingList>`. `DrawingList` deserializes element by element, skips any
   drawing that fails (unknown `tool`, bad JSON) with a `warn!`, and keeps the rest. Keep the
   skipped raw `Value`s in `DrawingList::unknown` and write them back on save, so a downgrade
   followed by an upgrade loses nothing.
5. **New enum variants** (`DrawingTool`, `LineStyle`, `LineEnd`, …) are additive. Never rename
   a variant. Add a test that deserializes a frozen JSON fixture of today's format
   (`tests/fixtures/drawings_v1.json`).

### 6.4 Settings dialog layout

New file `crates/tradeterm/src/drawing_settings.rs`. It follows `IndicatorSettingsPanel`
(`crates/tradeterm/src/indicators.rs:488-645`):
- edits apply live;
- **Cancel**, Esc or × restores the snapshot taken at open;
- **Ok** commits one undo step and updates the tool's last-used default (5.1).

```
┌ Trend Line ───────────────────────────────── × ┐
│  Style   Text   Coordinates   Visibility       │  ← tabs; Inputs first for long/short
│ ─────────────────────────────────────────────  │
│  Line            [■ color] [━━ width/style ▾]  │  label column 140 px, controls right
│  Left end        [Normal ▾]  Right end [Arrow ▾]│
│  Extend          ☐ Left   ☐ Right              │
│  ☐ Middle point                                │
│  ☐ Price labels                                │
│  STATS                                         │  section header (muted caps, like
│  Stats           [Price range, Bars ▾]         │  chart_settings::section)
│  Stats position  [Right ▾]                     │
│  ☐ Always show stats                           │
│ ─────────────────────────────────────────────  │
│ [Template ▾]                   [Cancel] [ Ok ] │
└────────────────────────────────────────────────┘
```

- Width 460 px; the content scrolls when tall (the Fib levels list).
- The dialog remembers the last tab per tool, in memory **[ui]**.
- With a multi-selection of the same tool, the dialog edits all of them. It shows the first
  drawing's values and hides the Coordinates tab.
- Fib levels: a two-column grid of `[☐] [coeff input] [■]` rows, then "Use one color [■]".
- Opening:
  - `ChartEvent::OpenDrawingSettings(id)`, emitted on double-click, the gear, or the context
    menu.
  - `TradeTerm::open_drawing_settings`, modelled on `open_indicator_settings`
    (`app.rs:531-560`).
- The color control is one shared component (`ui.rs` or a new `color_menu.rs`) used by the
  dialog and the floating toolbar:
  - TV swatch grid;
  - "+" opens the gpui-kit `ColorPicker`;
  - recent custom colors stored in `AppSettings::custom_colors`;
  - opacity slider.

Motion: the dialog and the flyouts use the existing short, interruptible gpui-kit motion.
Drawings themselves never animate. This follows the project's "snappy animations" rule: motion
on UI only, never on data.

### 6.5 Undo / redo model

- `History { undo: Vec<Edit>, redo: Vec<Edit> }` per chart, capped at 100 entries.
- `Edit` is a snapshot diff of the drawings it touched: `{before: Vec<Drawing>, after:
  Vec<Drawing>}`, keyed by id, with `None` for created or deleted.
- Steps are recorded at gesture end: place finished, drag released, dialog Ok, toolbar edit,
  delete, paste, clone, template apply.
- Clearing the redo stack happens on any new edit.
- Switching symbol keeps a separate history per symbol key **[?]**. Simplest choice: clear the
  history on symbol change.

### 6.6 Rendering and performance notes

- Precompute visibility per drawing when the timeframe changes. `draw_base` skips invisible
  and hidden drawings with no per-frame interval math.
- Cull by screen bounding box in `layout`, except extended lines and full-height tools.
- Cache stats and position label strings per drawing. Rebuild them only when the drawing's
  anchors change or a new bar arrives inside its span (position, range volume).
- The brush stores simplified points (3.16); polygon fills are tessellated once per layout.
- Canvas additions (`render/mod.rs`, implemented in `tt-chart-gpui/src/canvas.rs`):
  - `fill_polygon(&[Pos2], Color32)`
  - `stroke_polyline_styled(points, width, color, style)`
  - `ellipse(rect, stroke, fill)`
  - `text_styled` gains `italic` and `rotation`
  - `arrow_head(tip, dir, size, color)`

---

## 7. Implementation plan

Slices are ordered by priority. **S0 must land first**: it splits `drawings.rs` into a module so
that the later slices touch disjoint files. After S0, slices A, B, C, G and H can run in
parallel; D, E and F can start as soon as A's dialog skeleton exists (they only add tab
content).

Target module layout, created by S0:

```
crates/tt-chart/src/drawings/
  mod.rs        Drawings (interaction state machine), re-exports
  model.rs      Drawing, Anchor, DrawingTool (+ groups, labels, icons, shortcuts), Rgba, Visibility
  props.rs      Props enum, shared sub-structs, defaults(tool), overlay merge, serde repr
  place.rs      placement state machine (fixed N points, open-ended, freehand), Shift constraints
  hit.rs        hit testing dispatch, handle lists
  render.rs     draw_base / draw_overlay / axis_tags dispatch, styled_line, arrows, labels
  stats.rs      stats text (trend line, info line, ranges) + format_span
  lines.rs      trend / ray / extended / info / angle / arrow / h-line / h-ray / v-line / cross
  channel.rs    parallel channel
  fib.rs        retracement + extension
  shapes.rs     rectangle, ellipse, circle, polyline
  freehand.rs   brush, path
  text.rs       text, note, callout, price label
  position.rs   long / short (math + render)
  measure.rs    price / date / date-and-price range
  history.rs    undo / redo
  tests/…       per-family tests + drawings_v1.json fixture
```

### S0: Foundation (blocking; 1 engineer; small)
- Split `drawings.rs` into the layout above with behaviour unchanged. Keep the public API
  re-exported from `tt-chart/src/lib.rs:20-35`.
- Add `Props` + `Rgba` + `Visibility` + serde repr, migration, overlay, `DrawingList`, the
  `drawings_v1.json` fixture test, and the `AppSettings.drawings` type change.
- Add **every** planned `DrawingTool` variant now, with `DrawingTool::available()` returning
  only the implemented ones. Later slices then only flip availability in their own match arm,
  and the toolbar stays data-driven.
- Make `press`/`motion`/`release` take a `Mods { shift, ctrl }` argument and thread it through
  `ChartView::drawing_press/motion` (`view.rs:463-486`) and `chart.rs:342-464`. Ignore the
  modifiers for now.
- Generalize placement to `Placement::{Points(n), OpenEnded, Freehand}`.
- Add the Canvas primitives (6.6).
- Move the floating toolbar out of `chart.rs:595-751` into
  `crates/tt-chart-gpui/src/drawing_toolbar.rs`, behaviour unchanged.

Files:
- `tt-chart/src/drawings/*`, `tt-chart/src/lib.rs`, `tt-chart/src/view.rs`,
  `tt-chart/src/render/mod.rs`
- `tt-chart-gpui/src/canvas.rs`, `tt-chart-gpui/src/chart.rs`,
  `tt-chart-gpui/src/drawing_toolbar.rs` (new)
- `tt-session/src/settings.rs`, `tt-session/src/tab.rs`

### A: Settings dialog, templates, floating toolbar, visibility (P0)
- `drawing_settings.rs` dialog with the Style / Text / Coordinates / Visibility tab framework.
  The fields are generated from per-family form descriptors exposed by each family module
  (`fn form(props) -> Vec<Field>`), so B-G add fields without editing the dialog.
- Shared color menu with opacity and custom colors.
- Double-click and the gear open the dialog (`ChartEvent::OpenDrawingSettings`).
- Templates (section 5): `AppSettings.drawing_templates`, last-used default, factory reset,
  named save/apply/delete, multi-apply.
- Full floating toolbar (4.1): draggable, remembered position, capability-driven buttons, glyph
  width/style menus, "…" menu.
- Visibility evaluation + presets.

Files:
- `crates/tradeterm/src/drawing_settings.rs` (new), `crates/tradeterm/src/color_menu.rs` (new),
  `crates/tradeterm/src/app.rs` (open dialog, event)
- `tt-chart-gpui/src/drawing_toolbar.rs`
- `tt-session/src/settings.rs` (templates, toolbar position, custom colors)
- `tt-chart/src/drawings/props.rs` (form descriptor types), `tt-chart/src/drawings/model.rs`
  (visibility logic)

### H: Interaction: selection, clipboard, undo, constraints, magnet, context menu (P0)
- Multi-select (Ctrl+click, Ctrl+area): `selected: Vec<u64>`.
- Shift constraints for placing, handle drags and body drags.
- Ctrl+drag clone; arrow-key nudge.
- Weak and strong magnet with Ctrl inversion (4.5).
- `history.rs` undo/redo (6.5) with Ctrl+Z/Y.
- Ctrl+C/V: an in-app clipboard holding `Vec<Drawing>` JSON; optionally mirrored to the OS
  clipboard as JSON text.
- Right-click context menu (4.2) and empty-space Paste.
- Visual order operations; per-drawing Hide.

Files:
- `tt-chart/src/drawings/{mod.rs, place.rs, hit.rs, history.rs}`
- `tt-chart-gpui/src/drawing_menu.rs` (new), `tt-chart-gpui/src/chart.rs` (right-button
  handlers only)
- `crates/tradeterm/src/app.rs` (key handling `:1051-1072`)

Coordinate with A on `app.rs`: A only adds the dialog opener; H only touches the key handler.

### B: Line tools to full fidelity + new line tools (P1)
- Trend line, ray and extended line: line ends, extend, middle point, price labels, stats +
  stats position + always show, text along the line.
- Horizontal line, horizontal ray, vertical line and cross line: label toggles, time-axis tags,
  text with alignment and orientation, vertical "extend" across panes.
- New tools: info line, trend angle, arrow, parallel channel.

Files: `tt-chart/src/drawings/{lines.rs, channel.rs, stats.rs}`, and those families' arms in
`render.rs`/`hit.rs`.

### C: Fibonacci (P1)
- Editable 24-level list, reverse, extend left/right, labels/prices/percent, label alignment,
  trend line toggle, background opacity, log-based option (fixing the always-log behaviour,
  3.11).
- Trend-based Fib extension (3 points).

Files: `tt-chart/src/drawings/fib.rs`, plus the Fib levels widget in
`crates/tradeterm/src/drawing_settings/fib_levels.rs` (a submodule file owned by C).

### G: Measurers (P1)
- Stats checklist (+ pips, volume), border, label background, extend, text.
- Tool-correct `measurer_rect`.
- `format_span` with seconds and weeks.

Files: `tt-chart/src/drawings/measure.rs`, `stats.rs` (shared with B: agree on the API in S0).

### D: Shapes and freehand (P2)
- Rectangle with 8 handles, middle line, extend, text, Shift square.
- Ellipse, circle, polyline (close or open), path, brush (smoothing + simplification).

Files: `tt-chart/src/drawings/{shapes.rs, freehand.rs}`.

### E: Text and annotations (P2)
- Text, note, callout, price label.
- Inline on-chart text editor: an absolutely positioned gpui-kit `Input` over the anchor,
  committed on blur or Ctrl+Enter.

Files: `tt-chart/src/drawings/text.rs`, `tt-chart-gpui/src/text_edit.rs` (new).

### F: Long / short position (P2)
- Math (3.24) with unit tests against the documented formulas.
- Rendering, including the open/closed state from the series.
- Labels + compact mode, 4 handles, Reverse position.
- Inputs tab.

Files: `tt-chart/src/drawings/position.rs`, `crates/tradeterm/src/drawing_settings/inputs.rs`
(new submodule), `tt-core/src/symbol_info.rs` (read-only use of tick/qty step).

### I: Left toolbar and polish (P3)
- Data-driven groups and flyouts from `DrawingTool` groups:
  - Lines: trend line, ray, info line, extended line, trend angle, horizontal line,
    horizontal ray, vertical line, cross line, parallel channel
  - Fib: retracement, extension
  - Shapes: rectangle, circle, ellipse, path, polyline, arrow, brush
  - Annotation: text, note, callout, price label
  - Forecasting: long, short
  - Measurers
- Magnet flyout (weak / strong / snap to indicators).
- Persist the magnet, keep drawing, lock and hide toggles (BACKLOG).
- Favorites toolbar.
- Object tree (later).

Files: `crates/tradeterm/src/drawing_toolbar.rs`, `tt-session/src/settings.rs` (toggles).

Finally, update `README.md` and `BACKLOG.md` in each slice as its features land.

### Overlap matrix (after S0)

| File | A | H | B | C | G | D | E | F | I |
|---|---|---|---|---|---|---|---|---|---|
| drawings/props.rs | ● | | ○ | ○ | ○ | ○ | ○ | ○ | |
| drawings/mod.rs, place.rs, hit.rs | | ● | ○ | ○ | | ○ | | ○ | |
| drawings/render.rs (dispatch arms) | | | ○ | ○ | ○ | ○ | ○ | ○ | |
| family files | | | ● | ● | ● | ● | ● | ● | |
| tradeterm/drawing_settings.rs | ● | | | ○ | | | | ○ | |
| tt-chart-gpui/drawing_toolbar.rs | ● | ○ | | | | | | | |
| tradeterm/app.rs | ○ | ○ | | | | | | | |
| tt-session/settings.rs | ● | | | | | | | | ○ |
| tradeterm/drawing_toolbar.rs | | | | | | | | | ● |

● = owner, ○ = small, well-delimited additions such as one match arm or one props struct.

---

## 8. Open questions to verify in a live TradingView chart

1. Exact floating-toolbar button order per tool, and the "…" menu items.
2. Exact right-click menu items and order.
3. Whether templates are shared across trend line, ray and extended line.
4. Whether templates include text content and visibility.
5. Whether "Apply Default Drawing Template" also resets the remembered last-used style.
6. Colors of the 13 hidden Fib retracement levels; the default visible levels of the Fib
   extension.
7. Parallel channel default level rows (beyond 0, 0.5 and 1).
8. The ellipse placement model and handles.
9. The meaning and unit of the trend line "Distance" stat.
10. Long/short:
    - default stop/target distance and width when placed;
    - the exact label wording and compact-mode format;
    - qty precision options;
    - whether there is a Coordinates tab.
11. The Ctrl+V paste position on the same chart (same anchors or offset).
12. TV.com default widths: 1 px vs 2 px for Fib levels, rectangle, ellipse, brush and polyline.
