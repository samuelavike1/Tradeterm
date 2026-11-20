# tradeterm

A native desktop trading/charting app written in Rust, built to be the fastest,
smoothest charting and execution app around. The UI is
[gpui](https://www.gpui.rs) (Zed's GPU-accelerated UI framework) with
[gpui-component](https://github.com/longbridge/gpui-kit) via `gpui-kit`. It uses no
web technology (no Tauri, Electron or WebView).

It streams live Binance spot candles from public, key-less endpoints and draws them
with a custom chart widget built to handle a million bars.

## Features

- **Custom window chrome:** the title bar (gpui-component's `TitleBar`) holds a tab
  strip (symbol badge, symbol, 24h change %, close on hover or middle-click, "+" for a
  new tab), a theme toggle, a settings menu and native-style window buttons. Dragging,
  double-click to maximize and OS snapping work as usual.
- **Tabs:** each tab is an independent chart with its own live stream, so background
  tabs stay current. Open tabs, their order and the active tab are restored on the
  next launch.
- **Chart types (TradingView's menu):** Bars, Candles, Hollow candles, Volume candles ·
  Line, Line with markers, Step line · Area, HLC area, Baseline · Columns, High-low ·
  Heikin Ashi, Renko (ATR 14 box), Line break (3 lines). The grouped menu shows icons and
  hover descriptions; star a type to pin it to the toolbar as a quick button. Heikin Ashi,
  Renko and Line break are computed in `tt-core`, with tests. Footprint, TPO, session
  volume profile and range bars need tick data and aren't included; Kagi and
  Point & figure aren't implemented yet.
- **Price scales:** regular, percent and indexed-to-100 (both relative to the first
  visible close), and logarithmic, optionally inverted. The price axis auto-fits the
  visible bars with the top/bottom margins from the chart settings (8% by default).
  Drag the axis to scale it manually; double-click it to return to auto-fit.
- **Price-scale corner (like TradingView):** hovering the price axis shows **A**
  (auto scale), **L** (log) and **%** (percent) buttons above the **⚙** in the chart's
  bottom-right corner. The ⚙ opens the price-scale menu: reset, auto, invert, scale
  mode, last-price label and line, countdown, indicator value labels, no overlapping
  labels, and **More settings…**.
- **Chart settings:** a dialog with Symbol (previous-close coloring, body / borders /
  wick toggles and colors), Status line (title, OHLC values, change, volume, indicator
  titles / inputs / values), Scales and lines (scale buttons, labels, lines, day of
  week, font size) and Canvas (background, grid, crosshair and scale colors, margins)
  pages. Changes preview live; Cancel reverts.
- **Navigation:** drag the plot to pan (it also pans vertically when the scale is
  manual). Mouse wheel or pinch zooms around the cursor. Drag the time axis to zoom
  horizontally. Double-click the plot to reset the view.
- **Time axis by bar index:** weekend and closed-session gaps take no space.
  Labels step through year → quarter → month → 5 days → day → 12h → … → 1m depending
  on zoom (~80 px apart) and never overlap; boundaries heavier than the regular unit
  (e.g. full hours among half-hours, a new day) are drawn semibold. The crosshair
  shows TradingView's date format, e.g. `Fri 20 Nov '26  03:35`.
- **Price axis (TradingView-style):** labels every ~38 px with "nice" 1-2-5 steps and
  thousands separators; precision from the symbol's tick size. The last-price tag spans
  the axis in the candle color and shows a countdown to the bar close; tick labels under
  the last-price or crosshair tags are hidden.
- **Grid:** dashed lines at every price and time label, one GPU quad each.
- **Indicators (TradingView-style):** the **Indicators** button (or **/**) opens
  "Indicators, metrics, and strategies": search (keeps the last query; matches in
  blue), a Favorites / Technicals sidebar, and an alphabetical list with stars.
  Clicking a row adds it; the dialog stays open. Built in:
  - Volume (+ optional MA)
  - Price-pane overlays: Moving Average Simple/Exponential, Bollinger Bands, session
    VWAP
  - Own panes: RSI, MACD, Stochastic, ATR, OBV
- **Volume is an indicator**, as on TradingView. It is drawn in the bottom quarter
  of the price pane, behind the bars, with its own scale. New charts start with it;
  remove it like any other indicator. Inputs: MA length, color by previous close,
  Volume MA.
- **Indicator panes** each have their own auto-fit scale, grid and labels.
  Oscillators get guide levels (RSI 30/50/70, Stochastic 20/50/80). Drag a pane
  divider to resize. Hovering a pane shows TradingView's pane buttons at its top
  right: **move up / down**, **delete**, **collapse** (to a strip with just the
  legend; also Ctrl + double-click) and **maximize** (the other panes collapse).
  Collapsed panes are saved with the tab.
- **Drawing tools** (left toolbar, like TradingView's): cross cursor; **Lines**
  flyout with trend line (Alt + T), ray, extended line, horizontal line (Alt + H),
  horizontal ray (Alt + J), vertical line (Alt + V) and cross line (Alt + C);
  **Fib retracement** (Alt + F) with TradingView's levels and colors;
  **Rectangle** (Alt + Shift + R); **Measurers**: price range, date range, date and
  price range (change, %, bars, duration). Click to place each point, or drag out a
  two-point drawing in one gesture. Click a drawing to select it: drag its body or
  handles, and a floating toolbar sets color, width, line style, lock and delete.
  Delete / Backspace removes it, Escape cancels placing or releases the tool. Below:
  **magnet** (snap to bar open / high / low / close), **keep drawing**, **lock all**,
  **hide all** and **remove drawings**. Drawings are anchored in time and price,
  saved per symbol and shown on every timeframe.
- **Legend:** one row per indicator with its inputs and values at the crosshair.
  Price-pane rows sit under the OHLC readout and collapse to a "⌄ N" button; each
  pane's row is at its top left. Hovering a row shows **eye** (hide), **gear**
  (settings) and **trash** (remove); double-click opens the settings. Every visible
  output's last value is tagged on the price axis, and colliding tags stack.
- **Indicator settings:** Inputs / Style tabs, applied live, with **Defaults ▾**
  (reset), **Cancel** (restore) and **Ok**.
- Indicators compute incrementally: a live tick recomputes only the forming bar
  (RSI, MACD and Stochastic do one full O(n) pass).
- **Crosshair:** snaps to bar centers, spans every pane, shows labels on both axes
  (an indicator's value inside its pane), and an OHLC + change % readout at the top
  left.
- **Timeframes (TradingView-style):** any number of seconds, minutes, hours, days, weeks
  or months. Toolbar buttons are your favorites (plus the current interval). The chevron
  menu has collapsible Seconds/Minutes/Hours/Days sections: star an interval to add it
  to the toolbar, or "Add custom interval…" for things like 7m or 2D (custom ones can
  be removed). Binance-native intervals stream directly. Others are built from the
  largest native interval that divides them (45m ← 15m, 3h ← 1h, 3M ← 1M) and
  resampled live. Weeks start Monday and months follow the calendar; the bar-close
  countdown uses exact bar ends.
- **Symbol search (TradingView-style):** click the ticker pill or just start typing on
  the chart to open a search dialog. The field starts with the current symbol selected
  (or with the letter you typed). Quote-currency chips (All, USDT, USDC, FDUSD, BTC, …)
  filter results and are remembered. Matches are highlighted in blue. ↑/↓ highlight a
  row and preview it in the field, Enter opens it (if results are still loading, the top
  result opens when they arrive), click opens, and Esc or clicking outside closes. An
  empty field lists recent symbols, which are saved.
- **Backfill:** scrolling near the oldest loaded bar fetches the previous 1000 bars in
  the background and prepends them without moving the view. It stops at 250k bars.
- **Live data:** the WebSocket connects before history is fetched, so no bars are
  missed. Reconnects use exponential backoff (500 ms → 30 s) and reload history while
  keeping backfilled bars.
- **Large data:** below about 2 px per bar, bars are drawn decimated as one min/max
  column per pixel, built from a precomputed pyramid. 1,000,000 visible bars keep
  60 fps while panning (see [Performance](#performance)).
- **Resampling:** `tt-core` builds higher timeframes from lower ones (1m → 5m, 15m,
  1h, 4h, 1D) with buckets aligned to the UTC clock. It works in batch and
  incrementally.
- **Themes:** semantic color tokens with neutral Dark and Light palettes (pure grays in
  the style of shadcn's "neutral" scale, with TradingView's current up/down colors
  #089981 / #f23645),
  and a System mode that follows the OS. Chart colors can be overridden in the chart settings. The whole
  UI (panels, widgets, rounding, spacing) uses the tokens.
- **Icons:** [Lucide](https://lucide.dev) (ISC): gpui-component's icon set plus the
  `lucide-icons` font for chart-type and toolbar glyphs, in theme colors.
- **Native text:** gpui shapes text with the OS font stack (Segoe UI on Windows, the
  system font elsewhere).
- **Motion, kept snappy:** menus, popovers and dialogs use gpui-kit motion at about
  two-thirds of gpui-component's default durations (90 / 130 / 200 ms); dialogs fade in
  and settle; the active tab's background eases in over 120 ms; the "connecting" dot
  breathes. Hover feedback is instant, and prices, charts and the crosshair are never
  animated. The OS "reduce motion" setting (Windows: *Animation effects* off) turns it
  all off.
- **Performance overlay (F12):** per-second paint counts and CPU time of the chart's
  base and overlay layers, primitives drawn, and data events per second.
- **Settings persistence:** theme, colors, and every tab (symbol, timeframe, chart type,
  scale mode, indicators with their inputs, colors and pane sizes) plus the active
  tab and favorite indicators are saved as JSON, 750 ms after the last
  change and again on exit.

### Hotkeys

| Key | Action |
|---|---|
| Ctrl+T / Ctrl+W (Cmd on macOS) | New tab / close tab |
| Ctrl+Tab / Ctrl+Shift+Tab | Next / previous tab |
| Ctrl+Shift+T (Cmd+Shift+T on macOS) | Toggle dark/light theme |
| F12 | Performance overlay |
| Ctrl+Shift+D (Cmd+Shift+D on macOS) | Load 1,000,000 synthetic bars (stress test) |
| Any letter/digit on the chart | Open symbol search with that text |
| / | Indicators dialog |
| ↑ / ↓, Enter, Esc | Navigate, choose or cancel symbol search results |

| Mouse | Action |
|---|---|
| Drag the plot | Pan (also vertically when the price scale is manual) |
| Wheel / pinch / Ctrl+wheel | Zoom around the cursor |
| Horizontal wheel / trackpad swipe | Pan |
| Drag the price axis | Scale prices manually |
| Double-click the price axis | Return to auto-fit |
| Drag the time axis | Zoom horizontally |
| Double-click the plot | Reset the view |

The **⚙** menu in the title bar has the theme selector (Dark / Light / System),
**Chart settings…** (the same dialog as the price-scale menu's More settings…) and the synthetic-data
command. You can also start with synthetic data
from the command line: `tradeterm --synthetic [N]`.

## Architecture

```
crates/
  tt-core/       Domain types, no UI or network. Symbol, Timeframe, Candle,
                 PriceTicks, SymbolInfo, CandleSeries (structure of arrays, with a
                 structural version counter), MarketEvent, EventSink, Heikin Ashi,
                 resampling, and indicator math (studies.rs: Study inputs,
                 outputs, incremental compute).
  tt-theme/      Semantic color tokens (Theme), Dark/Light palettes, ThemeMode.
  tt-brokers/    MarketDataSource trait + Binance adapter (tokio, tokio-tungstenite
                 with rustls, reqwest).
  tt-chart/      The chart, independent of the UI toolkit.
     viewport.rs   Pure coordinate math: index_to_x, x_to_index, price_to_y,
                   y_to_price, visible_range, in linear/log/percent modes.
     scale.rs      Nice price ticks, precision, formatting, UTC dates.
     time_axis.rs  Index-based time ticks with weighted, non-overlapping labels.
     bars.rs       The per-frame DrawBar list (one per bar, or one per pixel column).
     lod.rs        Min/max pyramid used for decimation.
     draw.rs       Builds the picture from Canvas primitives only, in two layers:
                   draw_base (everything static) and draw_overlay (crosshair,
                   readout, legends).
     render/       Canvas trait (the backend boundary). Geometry and colors are
                   emath / ecolor types; the crate has no UI-toolkit dependency.
     indicators.rs Indicator: a Study + colors/visibility, with cached outputs.
     view.rs       ChartView: render_base / render_overlay, hit testing and input
                   (pan, zoom, axis and divider drags, legend buttons), caches.
  tt-chart-gpui/ Canvas on gpui (instanced quads, merged triangle paths, shaped
                 text) and the Chart view: a cached base layer plus a pointer
                 overlay, so moving the mouse never repaints bars or indicators.
  tt-session/    UI-independent app state: chart tabs with their live streams and
                 backfill, settings persistence, the background runtime, synthetic
                 data.
  tradeterm/     The gpui app: title bar and tabs, toolbar, menus (intervals, chart
                 types), symbol search, indicator dialogs, chart settings, status bar,
                 performance overlay, hotkeys.
```

**Why a separate `tt-theme` crate:** the tokens style both the chart and the rest of
the app's widgets. If they lived in `tt-chart`, the app's styling would depend on the
chart crate. As their own crate, other panes or a future wgpu renderer can use them
without pulling in chart code.

**Data flow:**

```
tokio thread (background)                 UI thread (gpui)
BinanceSource::run ── EventSink ──► crossbeam channel ──► drained in one update
  1. connect WebSocket                    │                  per burst of events
  2. GET klines (1000) → History          └─ wake() ──► futures channel ──► gpui task
  3. live klines → CandleUpdate
symbol_info / fetch_history(before) / search_symbols ── side requests on the same runtime
```

The UI thread never blocks on network I/O. It repaints only on input or when an
adapter sends an event; a burst of ticks is coalesced into one update, and only the
charts whose data changed repaint their base layer. Each stream
gets its own channel, so after a symbol or timeframe switch, stale events from the old
stream are dropped.

**Rendering boundary:** chart code draws only through `render::Canvas`: rects,
lines, dashed lines, polylines, gradient area fills and text, all in screen points.
`tt-chart-gpui`'s `GpuiCanvas` implements it on gpui: rects become instanced quads,
lines and fills become triangle paths (consecutive same-color triangles merged), and
quads and text are painted inside gpui layers so gpui skips its per-primitive
draw-order bookkeeping. A different backend (for example a custom GPU renderer) would
implement the same trait; viewport math, ticks, decimation and interaction stay as
they are.

**Caches stay incremental:** `CandleSeries::version()` changes only on structural
edits (history load, prepend, clear). The Heikin Ashi cache and the decimation pyramid
recompute just the last bar or block on live updates, and rebuild only when the
version changes.

## Performance

`cargo run -p tt-chart-gpui --release --example bench -- <hover|pan|static|empty>`
drives a 1920×1040 chart with 1,000,000 synthetic 1m bars and Volume, EMA 21, BB 20,
RSI 14 and MACD every frame (Windows 11, 60 Hz):

| Scenario | fps | Base layer | Overlay | Process CPU / frame |
|---|---|---|---|---|
| Empty window (floor) | 60 | – | – | 2.2 ms |
| Hover (crosshair moves) | 60 | never repainted | 0.83 ms | 6.3 ms |
| Static chart, redrawn every frame | 60 | never repainted | 0.56 ms | 5.9 ms |
| Pan (half the time over all 1M bars) | 58.9 | 5.2 ms per frame | 0.81 ms | 11.5 ms |

Profiling notes are in [crates/tt-chart-gpui/README.md](crates/tt-chart-gpui/README.md).
`cargo run --release -p tt-chart --example bench_draw` measures the chart's own CPU cost
headlessly (both layers into a counting canvas): 0.31 ms per frame with 1,000,000 bars
visible, under 0.1 ms zoomed in.

## Setup

You need a recent stable Rust toolchain from <https://rustup.rs>. This project was
built with Rust 1.99 on edition 2024.

### Windows

1. Install the **Visual Studio Build Tools** with the *Desktop development with C++*
   workload (this provides the MSVC linker and Windows SDK).
2. Install Rust: download and run `rustup-init.exe` from <https://rustup.rs>.
3. Use an up-to-date GPU driver. gpui renders with DirectX 11.

### macOS

```bash
xcode-select --install
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

gpui renders through Metal, so you don't need anything else.

### Linux

Install a C toolchain, the X11/Wayland/keyboard/font development headers gpui needs
(the same as for building Zed), and a Vulkan driver.

**Debian / Ubuntu**

```bash
sudo apt install build-essential pkg-config cmake \
  libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev \
  libxkbcommon-dev libwayland-dev libx11-dev libxcursor-dev libxrandr-dev libxi-dev \
  mesa-vulkan-drivers libvulkan1
```

**Fedora**

```bash
sudo dnf install gcc gcc-c++ pkgconf-pkg-config cmake \
  libxcb-devel libxkbcommon-devel wayland-devel libX11-devel libXcursor-devel \
  libXrandr-devel libXi-devel mesa-vulkan-drivers vulkan-loader
```

**Arch**

```bash
sudo pacman -S --needed base-devel cmake libxcb libxkbcommon wayland libx11 \
  libxcursor libxrandr libxi vulkan-icd-loader
```

Then install Rust:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

TLS uses rustls with the aws-lc-rs backend, so OpenSSL isn't required. If the
aws-lc-sys build complains, make sure `cmake` and a C compiler are installed.

## Build, test, run

```bash
cargo run -p tradeterm --release                  # the app
cargo run -p tradeterm --release -- --synthetic   # start with 1,000,000 synthetic bars
cargo build                                       # debug build (dependencies at opt-level 2)
cargo test                                        # unit tests
cargo clippy --all-targets -- -D warnings         # lints
cargo run --release -p tt-chart-gpui --example bench -- hover   # gpui chart benchmark
cargo run --release -p tt-chart --example bench_draw     # headless chart benchmark
cargo run -p tt-brokers --example stream -- ETHUSDT 5m 20  # live API smoke test
```

The release profile uses thin LTO with `codegen-units = 1`. Logging is controlled by
`RUST_LOG`, e.g. `RUST_LOG=debug`. The first build compiles gpui and takes a while
(about 15 minutes for a cold debug build); incremental builds take seconds.

Settings are stored at:

- Windows: `%APPDATA%\tradeterm\config\settings.json`
- macOS: `~/Library/Application Support/tradeterm/settings.json`
- Linux: `~/.config/tradeterm/settings.json`

Delete the file to reset to defaults. Set `TRADETERM_SETTINGS=/path/to/settings.json` to
use a different file (portable installs, test runs).

### Window chrome notes

The window has no OS decorations; gpui-component's `TitleBar` draws the title bar and
handles moving, double-click maximize and snapping. On macOS the native traffic
lights are kept; Windows and Linux get Windows-style buttons on the right.
