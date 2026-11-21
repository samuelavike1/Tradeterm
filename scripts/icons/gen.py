"""Writes tradeterm's own 1-stroke icon set (crates/tt-ui/assets/icons/*.svg).

Every icon: 28 x 28 viewBox, stroke width 1 at 1x, fill none, round caps
and joins, stroke = currentColor (the kit tints it). Axis-aligned strokes
sit on half pixels so they are crisp at 28 px. Endpoint handles are open
circles (r 2, or r 1.5 for line markers); lines stop at the circle edge.

Run: python -I scripts/icons/gen.py   (from the repo root)
"""

import math
import os

OUT = os.path.join(os.path.dirname(__file__), "..", "..", "crates", "tt-ui", "assets", "icons")

HEAD = (
    '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 28 28" width="28" height="28" '
    'fill="none" stroke="currentColor" stroke-width="1" stroke-linecap="round" '
    'stroke-linejoin="round">'
)


def f(v):
    s = f"{v:.2f}".rstrip("0").rstrip(".")
    return s


def path(d, extra=""):
    return f'<path d="{d}"{extra}/>'


def rect(x, y, w, h, extra=""):
    return f'<rect x="{f(x)}" y="{f(y)}" width="{f(w)}" height="{f(h)}"{extra}/>'


def circle(cx, cy, r=2.0):
    return f'<circle cx="{f(cx)}" cy="{f(cy)}" r="{f(r)}"/>'


def seg(a, b, ra=0.0, rb=0.0):
    """Line a -> b, trimmed by ra at a and rb at b (handle radii)."""
    dx, dy = b[0] - a[0], b[1] - a[1]
    n = math.hypot(dx, dy)
    ux, uy = dx / n, dy / n
    s = (a[0] + ux * ra, a[1] + uy * ra)
    e = (b[0] - ux * rb, b[1] - uy * rb)
    return f"M{f(s[0])} {f(s[1])}L{f(e[0])} {f(e[1])}"


def handles_line(points, r=2.0, open_start=False, open_end=False):
    """Polyline through handle circles (trimmed around each handle)."""
    parts = []
    for i in range(len(points) - 1):
        ra = 0.0 if (i == 0 and open_start) else r
        rb = 0.0 if (i == len(points) - 2 and open_end) else r
        parts.append(seg(points[i], points[i + 1], ra, rb))
    return "".join(parts)


FILLED = ' fill="currentColor"'
DASH = ' stroke-linecap="butt" stroke-dasharray="2 2"'

ICONS = {}

# ---- chart types -------------------------------------------------------------

ICONS["chart-bars"] = [
    path("M9.5 5.5v16M6.5 17.5h3M9.5 9.5h3M18.5 8.5v15M15.5 12.5h3M18.5 19.5h3"),
]


def candle(x, wick_top, body_top, body_bot, wick_bot, w=4, extra=""):
    cx = x + w / 2
    out = [rect(x, body_top, w, body_bot - body_top, extra)]
    d = ""
    if wick_top < body_top:
        d += f"M{f(cx)} {f(wick_top)}V{f(body_top)}"
    if wick_bot > body_bot:
        d += f"M{f(cx)} {f(body_bot)}V{f(wick_bot)}"
    if d:
        out.append(path(d))
    return out


ICONS["chart-candles"] = candle(7.5, 4.5, 8.5, 19.5, 23.5) + candle(16.5, 7.5, 11.5, 18.5, 21.5)
ICONS["chart-hollow-candles"] = candle(7.5, 4.5, 8.5, 19.5, 23.5) + candle(
    16.5, 7.5, 11.5, 18.5, 21.5, extra=FILLED
)
ICONS["chart-volume-candles"] = candle(8.5, 6.5, 9.5, 18.5, 21.5, w=2) + candle(
    14.5, 4.5, 8.5, 20.5, 23.5, w=8
)
# Heikin Ashi: rising candles with no lower wicks, each opening mid-body of
# the previous one.
ICONS["chart-heikin-ashi"] = (
    candle(4.5, 14.5, 17.5, 23.5, 23.5)
    + candle(12.5, 8.5, 11.5, 19.5, 19.5)
    + candle(20.5, 3.5, 6.5, 14.5, 14.5)
)

LINE = [(4.5, 19.5), (10.5, 12.5), (15.5, 17.5), (23.5, 7.5)]
ICONS["chart-line"] = [path("M" + "L".join(f"{f(x)} {f(y)}" for x, y in LINE))]
ICONS["chart-line-markers"] = [path(handles_line(LINE, r=1.5))] + [
    circle(x, y, 1.5) for x, y in LINE
]
ICONS["chart-step-line"] = [path("M4.5 20.5H9.5V14.5H14.5V17.5H19.5V8.5H23.5")]
ICONS["chart-area"] = [
    path("M4.5 18.5L10.5 12.5L15.5 16.5L23.5 7.5V23.5H4.5Z"),
    path("M8.5 16.5V21.5M13.5 16V21.5M18.5 15V21.5"),
]
ICONS["chart-hlc-area"] = [
    path("M4.5 13.5L10.5 7.5L15.5 10.5L23.5 4.5"),
    path("M4.5 18.5L10.5 13.5L15.5 16.5L23.5 10.5"),
    path("M4.5 23.5L10.5 19.5L15.5 22.5L23.5 16.5"),
]
ICONS["chart-baseline"] = [
    path("M4 14.5H24", DASH),
    path("M4.5 19.5L9.5 9.5L14.5 18.5L19.5 10.5L23.5 15.5"),
]
ICONS["chart-columns"] = [
    path("M4 23.5H24"),
    path("M5.5 23.5V15.5H9.5V23.5M12.5 23.5V7.5H16.5V23.5M19.5 23.5V12.5H23.5V23.5"),
]
ICONS["chart-high-low"] = [
    rect(4.5, 10.5, 4, 9),
    rect(12.5, 5.5, 4, 11),
    rect(20.5, 12.5, 4, 10),
]
ICONS["chart-renko"] = [
    rect(4.5, 17.5, 5, 5),
    rect(9.5, 12.5, 5, 5),
    rect(14.5, 7.5, 5, 5),
    rect(19.5, 12.5, 5, 5),
]
ICONS["chart-line-break"] = [
    rect(4.5, 15.5, 4, 8),
    rect(9.5, 9.5, 4, 6),
    rect(14.5, 4.5, 4, 5),
    rect(19.5, 4.5, 4, 10),
]

# ---- drawing tools -----------------------------------------------------------

ICONS["draw-cursor-cross"] = [path("M14.5 4.5V11.5M14.5 17.5V24.5M4.5 14.5H11.5M17.5 14.5H24.5")]
ICONS["draw-trend-line"] = [path(seg((6, 22), (22, 6), 2, 2)), circle(6, 22), circle(22, 6)]
ICONS["draw-ray"] = [
    path(seg((6, 22), (13, 15), 2, 2) + seg((13, 15), (23.5, 4.5), 2, 0)),
    circle(6, 22),
    circle(13, 15),
]
ICONS["draw-info-line"] = [
    path(seg((6, 22), (14, 14), 2, 2)),
    circle(6, 22),
    circle(14, 14),
    rect(16.5, 4.5, 7, 6, ' rx="1"'),
    path("M18.5 6.5h3M18.5 8.5h3"),
]
ICONS["draw-extended-line"] = [
    path(
        seg((4.5, 23.5), (11, 17), 0, 2)
        + seg((11, 17), (17, 11), 2, 2)
        + seg((17, 11), (23.5, 4.5), 2, 0)
    ),
    circle(11, 17),
    circle(17, 11),
]
_a = math.atan2(21.5 - 8, 21 - 6)
ICONS["draw-trend-angle"] = [
    path(seg((6, 21.5), (21, 8), 2, 2)),
    path("M8 21.5H23.5"),
    path(f"M14 21.5A8 8 0 0 0 {f(6 + 8 * math.cos(_a))} {f(21.5 - 8 * math.sin(_a))}"),
    circle(6, 21.5),
    circle(21, 8),
]
ICONS["draw-horizontal-line"] = [path("M4 14.5H12M16 14.5H24"), circle(14, 14.5)]
ICONS["draw-horizontal-ray"] = [path("M9 14.5H24"), circle(7, 14.5)]
ICONS["draw-vertical-line"] = [path("M14.5 4V12M14.5 16V24"), circle(14.5, 14)]
ICONS["draw-cross-line"] = [
    path("M4 14.5H12.5M16.5 14.5H24M14.5 4V12.5M14.5 16.5V24"),
    circle(14.5, 14.5),
]
ICONS["draw-parallel-channel"] = [
    path(seg((6, 14), (22, 6), 2, 2)),
    path(seg((4, 23), (14, 18), 0, 2) + seg((14, 18), (24, 13), 2, 0)),
    circle(6, 14),
    circle(22, 6),
    circle(14, 18),
]
ICONS["draw-arrow"] = [path("M6.5 21.5L21.5 6.5M14.5 6.5H21.5V13.5")]
ICONS["draw-fib-retracement"] = [
    path("M4 5.5H20M4 10.5H24M4 14.5H24M4 18.5H24M8 22.5H24"),
    circle(22, 5.5),
    circle(6, 22.5),
]
_fe = [(5, 23), (10, 16), (15, 20)]
ICONS["draw-fib-extension"] = [
    path("M4 5.5H24M4 9.5H24M4 13.5H24"),
    path(handles_line(_fe)),
] + [circle(x, y) for x, y in _fe]
ICONS["draw-rectangle"] = [
    path("M8.5 8.5H21.5V17.5M19.5 19.5H6.5V10.5"),
    circle(6.5, 8.5),
    circle(21.5, 19.5),
]
ICONS["draw-price-range"] = [path("M6.5 4.5H22.5M6.5 23.5H22.5M14.5 6.5V21.5M11.5 9.5L14.5 6.5L17.5 9.5")]
ICONS["draw-date-range"] = [path("M4.5 6.5V22.5M23.5 6.5V22.5M6.5 14.5H21.5M18.5 11.5L21.5 14.5L18.5 17.5")]
ICONS["draw-date-price-range"] = [
    path("M4.5 4.5V23.5H23.5"),
    path("M14.5 20.5V6.5M11.5 9.5L14.5 6.5L17.5 9.5M8.5 14.5H21.5M18.5 11.5L21.5 14.5L18.5 17.5"),
]

# ---- drawing toolbar toggles -------------------------------------------------

ICONS["draw-magnet"] = [
    path("M7.5 5.5V15a7 7 0 0 0 14 0V5.5H17.5V15a3 3 0 0 1-6 0V5.5Z"),
    path("M7.5 9.5H11.5M17.5 9.5H21.5"),
]
ICONS["draw-keep-drawing"] = [
    path("M7.5 20.5L18.5 9.5L20.5 11.5L9.5 22.5L6.5 23.5Z"),
    path("M16.5 11.5L18.5 13.5M15.5 23.5H23.5"),
]
ICONS["draw-lock"] = [
    rect(7.5, 13.5, 14, 10, ' rx="1"'),
    path("M10.5 13.5V10a4 4 0 0 1 8 0v3.5M14.5 17.5v2"),
]
ICONS["draw-unlock"] = [
    rect(7.5, 13.5, 14, 10, ' rx="1"'),
    path("M18.5 13.5V9.5a4 4 0 0 0-8 0v1M14.5 17.5v2"),
]
_eye = "M4.5 14.5C7 10 10.5 7.5 14.5 7.5S22 10 24.5 14.5C22 19 18.5 21.5 14.5 21.5S7 19 4.5 14.5Z"
ICONS["draw-eye"] = [path(_eye), circle(14.5, 14.5, 3)]
ICONS["draw-eye-off"] = [path(_eye), circle(14.5, 14.5, 3), path("M6.5 22.5L22.5 6.5")]
ICONS["draw-trash"] = [
    path("M6.5 8.5H22.5M11.5 8.5V6.5H17.5V8.5"),
    path("M8.5 8.5L9.5 23.5H19.5L20.5 8.5M12.5 12V20M16.5 12V20"),
]

# ---- bottom toolbar ----------------------------------------------------------

# Go to: a calendar open at the lower left, an arrow entering it
# (TradingView's go-to-date icon, drawn in our stroke).
ICONS["go-to-date"] = [
    path("M5.5 13.5V8.5a2 2 0 0 1 2-2h13a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2H14.5"),
    path("M5.5 10.5H22.5M10.5 4.5V8.5M17.5 4.5V8.5"),
    path("M4.5 18.5H11.5M8.5 15.5L11.5 18.5L8.5 21.5"),
]


# ---- right sidebar (docs/specs/right-panel.md) -------------------------------

# Watchlist: a bookmark with two list lines (TradingView's shape).
ICONS["panel-watchlist"] = [
    path("M7.5 4.5H20.5A1 1 0 0 1 21.5 5.5V23.5L14 19.5L6.5 23.5V5.5A1 1 0 0 1 7.5 4.5Z"),
    path("M10.5 9.5H17.5M10.5 13.5H17.5"),
]
# Alerts: an alarm clock (two bells, hands at 12 and 9).
ICONS["panel-alerts"] = [
    circle(14, 15, 8),
    path("M5.5 8.5L9 5M22.5 8.5L19 5M14 10.5V15.5H10.5"),
]
# Object tree: two stacked layers.
ICONS["panel-object-tree"] = [
    path("M14 5.5L23.5 11L14 16.5L4.5 11Z"),
    path("M4.5 16L14 21.5L23.5 16"),
]
# Object tree / data window rows: an indicator (a wave).
ICONS["tree-indicator"] = [path("M4.5 18.5C8 7.5 11 7.5 14 14S20 20.5 23.5 9.5")]


def main():
    os.makedirs(OUT, exist_ok=True)
    for name, parts in ICONS.items():
        with open(os.path.join(OUT, name + ".svg"), "w", newline="\n") as fh:
            fh.write(HEAD + "".join(parts) + "</svg>\n")
    print(len(ICONS), "icons")


if __name__ == "__main__":
    main()
