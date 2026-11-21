# TradingView drawing settings: reference per tool

Captured from tradingview.com (dark theme, Nov 2026) by drawing a temporary
instance of each tool, opening its settings, reading every tab (labels,
controls, select / menu options) and its floating toolbar, then deleting it.
Screenshots: `img/tv-ref/tv-<tool>-<tab>.png` (the horizontal ray / line
reference is `tv-hray-*.png`). Density and sizes are in
[ui-guidelines.md §2.10](ui-guidelines.md); this file is about *what* each
dialog shows and in which order.

Common to every tool:

- Header: tool name (renamable, pencil), ×; tabs Style · Text (when the tool
  has text) · Coordinates · Visibility; footer Template ▾ · Cancel · Ok.
- **Visibility** is the same for every tool: Ticks, Seconds … Months (check
  + `[min] ——o==o—— [max]`), Ranges (`tv-hray-visibility.png`).
- **Coordinates**: one row per point, `#k (price, bar)` `[price] [bar]`;
  vertical line `#1 (bar)` (bar only); horizontal line `#1 (price)`;
  horizontal ray `#1 (price, bar)`.
- **Text tab** (where present): `[color][size ▾][B][I]`, text area ("Add
  text"), then `Text alignment [vertical ▾] [horizontal ▾]`. Font sizes:
  8 10 11 12 14 16 18 20 22 24 28 32 40.
- Selects with checkable items: **Extend** is a menu of two checkable items
  ("Extend left line" / "Extend right line", the button shows "Don't
  extend", one item, or both joined with ", "); **Stats** is a menu of
  checkable stats (the button shows "Hidden" or the checked names joined).
- Floating toolbar order: grip · template · line color (pencil) ·
  background (bucket) · text color (T) · width (— Npx) · style · settings ·
  [alert] · lock · remove · more. Only the buttons listed per tool appear.
  (Alert is left out in tradeterm until alerts exist.)

## Trend line, Ray, Extended line, Info line, Arrow

`tv-trend-style.png`, `tv-trend-style-stats.png`, `tv-ray-style.png`,
`tv-extended-style.png`, `tv-info-style.png`, `tv-arrow-style.png`,
`tv-trend-text.png`, `tv-info-text.png`, `tv-trend-coordinates.png`,
`tv-trend-toolbar.png`

| Tab | Row | Control |
|---|---|---|
| Style | Line | LinePicker · left end `[—o]` (Normal / Arrow) · right end `[o—]` (Normal / Arrow), one row |
| | Extend | menu: Extend left line ☐ / Extend right line ☐ ("Don't extend") |
| | Middle point | checkbox |
| | Price labels | checkbox |
| | INFO | section caption |
| | Stats | menu: Price range, Percent change, Change in pips, Bars range, Date/time range, Distance, Angle ("Hidden") |
| | Stats position | select Left / Center / Right / Auto |
| | Always show stats | checkbox |
| Text | | `[color][14][B][I]`, text area, Text alignment `[Top ▾][Center ▾]` (vertical Top/Middle/Bottom; horizontal Left/Center/Right) |
| Coordinates | | `#1 (price, bar)`, `#2 (price, bar)` |
| Toolbar | | template, line color, text color, width, style, settings, (alert), lock, remove, more |

Per-tool defaults: Ray extends right; Extended line both; Info line: all
stats, position Center, Always show stats on; Arrow: right end Arrow.

## Trend angle

`tv-angle-style.png`, `tv-angle-coordinates.png`, `tv-angle-toolbar.png`

| Tab | Row | Control |
|---|---|---|
| Style | Line | LinePicker only (no ends) |
| | Extend | menu (as trend line) |
| | Middle point, Price labels | checkboxes |
| | INFO / Stats | menu: Price range, Percent change, Change in pips, Bars range |
| | Stats position | Left / Center / Right / Auto |
| | Always show stats | checkbox |
| Text | — | no Text tab |
| Coordinates | | `#1 (price, bar)` and `Angle [deg]` |
| Toolbar | | template, line color, width, style, settings, (alert), lock, remove, more (no text color) |

## Vertical line

`tv-vline-style.png`, `tv-vline-text.png`

| Tab | Row | Control |
|---|---|---|
| Style | Line | LinePicker |
| | Extend | checkbox (through all panes), on |
| | Time label | checkbox, on |
| Text | | bar + text area; Text alignment `[Middle ▾][Center ▾]`; Text orientation `[Vertical ▾]` (Horizontal / Vertical) |
| Coordinates | | `#1 (bar)` |
| Toolbar | | template, line color, text color, width, style, settings, (alert), lock, remove, more |

## Cross line

`tv-cross-style.png`, `tv-cross-toolbar.png`

| Tab | Row | Control |
|---|---|---|
| Style | Line | LinePicker |
| | Price label, Time label | checkboxes, on |
| Text | — | none |
| Coordinates | | `#1 (price, bar)` |
| Toolbar | | template, line color, width, style, settings, lock, remove, more (no alert) |

## Parallel channel

`tv-channel-style.png`, `tv-channel-text.png`, `tv-channel-toolbar.png`

| Tab | Row | Control |
|---|---|---|
| Style | levels | one column, one row per level in coefficient order: `[☐] [coeff] [LinePicker]`; the 0 and 1 rows (the channel's own lines) have no checkbox |
| | | defaults: −0.25 off, 0 on, 0.25 off, 0.5 on (dashed), 0.75 off, 1 on, 1.25 off |
| | Extend | menu: Extend left line / Extend right line |
| | Background | checkbox + color swatch |
| Text | | bar + text area; Text alignment `[Top ▾][Left ▾]` (vertical Top / Inside / Bottom) |
| Coordinates | | `#1 (price, bar)`, `#2 (price, bar)`, `Price offset` |
| Toolbar | | template, line color, background, text color, width, settings, (alert), lock, remove, more (no style) |

## Fib retracement, Trend-based Fib extension

`tv-fib-style.png`, `tv-fib-style-2.png`, `tv-fib-style-3.png`,
`tv-fibext-style.png`, `tv-fibext-style-2.png`, `tv-fib-toolbar.png`.
Wider dialog (334 vs 278 px at the capture scale); the Style tab scrolls.

| Tab | Row | Control |
|---|---|---|
| Style | ☑ Trend line | checkbox + LinePicker (gray, dashed) |
| | Levels line | width button · style button |
| | Extend | menu: Extend lines left / Extend lines right |
| | levels | grid, two columns, filled row by row: `[☐] [coeff] [swatch]` × 24 |
| | Use one color | swatch (split two-color icon) |
| | ☑ Background | checkbox + opacity slider |
| | Reverse | checkbox |
| | Prices | checkbox |
| | ☑ Levels | checkbox + select Values / Percents |
| | Labels | `[Left ▾][Middle ▾]` (horizontal Left/Center/Right, vertical Top/Middle/Bottom) |
| | ☑ Text | checkbox + `[Center ▾][Middle ▾]` (level text; tradeterm has no level text yet) |
| | Font size | select 8 … 24 |
| | Fib levels based on log scale | checkbox (disabled unless the scale is log) |
| Text | — | none |
| Coordinates | | `#1`, `#2` (retracement), `#1 … #3` (extension) `(price, bar)` |
| Toolbar | | template, line color (rainbow bar), width, settings, (alert), lock, remove, more (no style) |

Level order (row by row): 0, 0.236 / 0.382, 0.5 / 0.618, 0.786 / 1, 1.618 /
2.618, 3.618 / 4.236, 1.272 / 1.414, 2.272 / 2.414, 2 / 3, 3.272 / 3.414, 4
/ 4.272, 4.414 / 4.618, 4.764.

## Rectangle

`tv-rect-style.png`, `tv-rect-text.png`, `tv-rect-toolbar.png`

| Tab | Row | Control |
|---|---|---|
| Style | Extend | menu: Extend left / Extend right (first row) |
| | Border | LinePicker |
| | ☑ Middle line | checkbox + LinePicker (dashed) |
| | ☑ Background | checkbox + swatch |
| Text | | bar + text area; Text alignment `[Inside ▾][Right ▾]` (vertical Top / Inside / Bottom) |
| Coordinates | | `#1 (price, bar)`, `#2 (price, bar)` |
| Toolbar | | template, line color, background, text color, width, style, settings, (alert), lock, remove, more |

## Price range, Date range, Date and price range

`tv-prange-style.png`, `tv-prange-text.png`, `tv-drange-style.png`,
`tv-dprange-style.png`, `tv-range-toolbar.png`

| Tab | Row | Control |
|---|---|---|
| Style | Line | LinePicker |
| | ☐ Border | (date and price range only) checkbox + LinePicker |
| | ☑ Background | checkbox + swatch |
| | Extend | menu: Extend left / Extend right (price range), Extend top / Extend bottom (date range); none for date and price range |
| | INFO / Stats | menu: price: Price range, Percent change, Change in pips; date: Bars range, Date/time range, Volume; both: all six |
| | Label | color swatch + size select |
| | ☑ Label background | checkbox + swatch |
| Text | | `[color][12][B][I]` + text area (no alignment row) |
| Coordinates | | `#1 (price, bar)`, `#2 (price, bar)` |
| Toolbar | | template, line color, background, text color, width, settings, lock, remove, more (no style, no alert) |
