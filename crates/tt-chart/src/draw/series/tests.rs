//! Per-type style resolution and price-source drawing.

use super::*;
use crate::ChartStyle;
use crate::bars::{collect_bars, price_source};
use crate::drawings::Drawings;
use crate::lod::Lod;
use crate::style::{PriceSource, Rgba};
use crate::viewport::{PlotRect, PriceScale, Viewport};
use emath::{Vec2, vec2};
use tt_core::{Candle, CandleSeries};

#[derive(Debug, Clone, PartialEq)]
enum Prim {
    Rect(Rect, Color32),
    Stroke(Color32),
    Poly(Vec<Pos2>, f32, Color32),
    Dashed(f32, Color32, f32),
    DashLine(Pos2, Color32),
    Area(Color32, Color32),
    Between(Color32),
    Circle(Color32),
    Text(String),
}

#[derive(Default)]
struct Rec(Vec<Prim>);

impl Canvas for Rec {
    fn pixels_per_point(&self) -> f32 {
        1.0
    }
    fn set_clip(&mut self, _: Rect) {}
    fn fill_rect(&mut self, r: Rect, c: Color32) {
        self.0.push(Prim::Rect(r, c));
    }
    fn fill_rounded_rect(&mut self, _: Rect, _: f32, _: Color32) {}
    fn stroke_rect(&mut self, _: Rect, _: f32, c: Color32) {
        self.0.push(Prim::Stroke(c));
    }
    fn line(&mut self, _: Pos2, _: Pos2, _: f32, _: Color32) {}
    fn dashed_line(&mut self, a: Pos2, _: Pos2, _: f32, c: Color32, _: f32, _: f32) {
        self.0.push(Prim::DashLine(a, c));
    }
    fn polyline(&mut self, p: &[Pos2], w: f32, c: Color32) {
        self.0.push(Prim::Poly(p.to_vec(), w, c));
    }
    fn dashed_polyline(&mut self, _: &[Pos2], w: f32, c: Color32, dash: f32, _: f32) {
        self.0.push(Prim::Dashed(w, c, dash));
    }
    fn fill_area(&mut self, _: &[Pos2], _: f32, _: f32, t: Color32, b: Color32) {
        self.0.push(Prim::Area(t, b));
    }
    fn fill_between(&mut self, _: &[Pos2], _: &[Pos2], c: Color32) {
        self.0.push(Prim::Between(c));
    }
    fn fill_circle(&mut self, _: Pos2, _: f32, c: Color32) {
        self.0.push(Prim::Circle(c));
    }
    fn text_styled(&mut self, p: Pos2, a: Align2, t: &str, s: f32, _: Color32, _: bool) -> Rect {
        self.0.push(Prim::Text(t.to_owned()));
        a.anchor_size(p, vec2(t.len() as f32 * s * 0.6, s))
    }
    fn text_size_styled(&mut self, t: &str, s: f32, _: bool) -> Vec2 {
        vec2(t.len() as f32 * s * 0.6, s)
    }
}

impl Rec {
    fn rect_colors(&self) -> Vec<Color32> {
        let rects = self.0.iter().filter_map(|p| match p {
            Prim::Rect(_, c) => Some(*c),
            _ => None,
        });
        rects.collect()
    }

    fn count(&self, f: impl Fn(&Prim) -> bool) -> usize {
        self.0.iter().filter(|p| f(p)).count()
    }
}

fn data() -> CandleSeries {
    let mut s = CandleSeries::new();
    // Rising, falling, rising; high = close + 2, low = close - 4.
    for (i, (o, cl)) in [(100.0, 102.0), (102.0, 98.0), (98.0, 104.0)]
        .into_iter()
        .enumerate()
    {
        s.upsert(Candle {
            open_time_ms: i as i64 * 60_000,
            open: o,
            high: cl + 2.0,
            low: cl - 4.0,
            close: cl,
            volume: 1.0 + i as f64,
        });
    }
    s
}

fn vp(spacing: f32, n: usize) -> Viewport {
    Viewport {
        bar_spacing: spacing,
        right_offset_bars: 0.0,
        price_min: 90.0,
        price_max: 110.0,
        plot: PlotRect {
            left: 0.0,
            top: 0.0,
            right: 400.0,
            bottom: 200.0,
        },
        bar_count: n,
        scale: PriceScale::linear(),
    }
}

/// Draws `bars` of `series` as `ty` and returns the primitives.
fn draw(
    ty: ChartType,
    style: &ChartStyle,
    series: &CandleSeries,
    bars: &[DrawBar],
    vp: Viewport,
    decimated: bool,
    projection_from: Option<usize>,
) -> Rec {
    let theme = Theme::dark();
    let drawings = Drawings::default();
    let rect = Rect::from_min_max(pos2(0.0, 0.0), pos2(400.0, 200.0));
    let s = Scene {
        vp,
        theme: &theme,
        style,
        chart_type: ty,
        full: rect,
        plot: rect,
        volume_rect: None,
        indicators: &[],
        panes: Vec::new(),
        range: 0..series.len(),
        legend_collapsed: false,
        main_collapsed: false,
        legend_hover: None,
        price_axis: rect,
        time_axis: rect,
        display: series,
        projection_from,
        last_raw: series.last(),
        bars,
        decimated,
        crosshair: None,
        precision: 2,
        tick_size: Some(0.01),
        bar_interval_ms: 60_000,
        now_ms: None,
        bar_close_ms: None,
        title: "",
        drawings: &drawings,
    };
    let mut c = Rec::default();
    super::series(&mut c, &s);
    c
}

fn simple(ty: ChartType, style: &ChartStyle) -> Rec {
    let s = data();
    let v = vp(20.0, s.len());
    let bars = collect_bars(&s, 0..s.len(), &v, price_source(&style.symbol, ty));
    draw(ty, style, &s, &bars, v, false, None)
}

const RED: Rgba = Rgba([255, 0, 0, 255]);
const BLUE: Rgba = Rgba([0, 0, 255, 255]);

#[test]
fn candle_slots_follow_theme_and_body() {
    let t = Theme::dark();
    let mut st = CandleStyle::default();
    let c = candle_colors(&st, &t);
    assert_eq!(c.body, [t.down, t.up]);
    assert_eq!(c.border, c.body, "borders follow the body");
    assert_eq!(c.wick, [t.down_wick, t.up_wick]);
    st.body_up = Some(RED);
    st.wick_down = Some(BLUE);
    let c = candle_colors(&st, &t);
    assert_eq!(
        (c.body[1], c.border[1]),
        (RED.to_color32(), RED.to_color32())
    );
    assert_eq!(c.wick[0], BLUE.to_color32());
    let p = brick_palette(&BrickColors::default(), &t);
    assert_eq!(p.fill[0], [t.down, t.up]);
    assert_eq!(p.fill[1][1], mix(t.up, t.bg, 0.6));
    assert_eq!(p.border[1][1], p.fill[1][1]);
}

#[test]
fn each_candle_type_reads_its_own_style() {
    let mut style = ChartStyle::default();
    style.symbol.heikin_ashi.body_up = Some(RED);
    style.symbol.volume_candles.wick = false;
    style.symbol.hollow_candles.body_down = Some(BLUE);
    let ha = simple(ChartType::HeikinAshi, &style);
    assert!(ha.rect_colors().contains(&RED.to_color32()));
    let candles = simple(ChartType::Candles, &style);
    assert!(!candles.rect_colors().contains(&RED.to_color32()));
    // Volume candles without wicks: one body rect per bar.
    assert_eq!(simple(ChartType::VolumeCandles, &style).0.len(), 3);
    // Hollow: the falling bar is filled in its own down color; rising bars
    // are outlines.
    let hollow = simple(ChartType::HollowCandles, &style);
    assert!(hollow.rect_colors().contains(&BLUE.to_color32()));
    assert_eq!(hollow.count(|p| matches!(p, Prim::Stroke(_))), 2);
    // Body and wick off: borders only.
    style.symbol.candles.body = false;
    style.symbol.candles.wick = false;
    let c = simple(ChartType::Candles, &style);
    assert!(
        c.0.iter().all(|p| matches!(p, Prim::Stroke(_))),
        "{:?}",
        c.0
    );
}

#[test]
fn bars_hlc_and_thin() {
    let mut style = ChartStyle::default();
    assert_eq!(simple(ChartType::Bars, &style).0.len(), 9);
    style.symbol.bars.hlc = true;
    style.symbol.bars.up = Some(RED);
    let b = simple(ChartType::Bars, &style);
    assert_eq!(b.0.len(), 6, "no open ticks");
    assert_eq!(b.rect_colors()[0], RED.to_color32());
    let width = |r: &Rec| match r.0[0] {
        Prim::Rect(r, _) => r.width(),
        _ => 0.0,
    };
    assert_eq!(width(&b), 1.0, "thin: 1 px");
    style.symbol.bars.thin = false;
    assert!(width(&simple(ChartType::Bars, &style)) > 1.0);
}

#[test]
fn line_family_color_width_style_and_source() {
    let mut style = ChartStyle::default();
    let t = Theme::dark();
    let v = vp(20.0, 3);
    // Defaults: palette[0], width 2, solid, through closes.
    match &simple(ChartType::Line, &style).0[0] {
        Prim::Poly(p, w, c) => {
            assert_eq!((*w, *c), (2.0, t.series_palette[0]));
            assert_eq!(p[1].y, v.y_of(98.0));
        }
        p => panic!("{p:?}"),
    }
    style.symbol.step_line.color = Some(RED);
    style.symbol.step_line.width = 3.0;
    style.symbol.step_line.kind = LineKind::Dashed;
    assert_eq!(
        simple(ChartType::StepLine, &style).0,
        [Prim::Dashed(3.0, RED.to_color32(), 18.0)]
    );
    // Line keeps its own instance.
    assert!(matches!(
        simple(ChartType::Line, &style).0[0],
        Prim::Poly(..)
    ));
    // Source: (H + L) / 2 = close - 1.
    style.symbol.line.source = PriceSource::Hl2;
    match &simple(ChartType::Line, &style).0[0] {
        Prim::Poly(p, ..) => assert_eq!(p[1].y, v.y_of(97.0)),
        p => panic!("{p:?}"),
    }
    style.symbol.line_with_markers.kind = LineKind::Dotted;
    let m = simple(ChartType::LineWithMarkers, &style);
    assert!(matches!(m.0[0], Prim::Dashed(w, _, d) if w == 2.0 && d == 2.0));
    assert_eq!(m.0.len(), 4, "line + 3 markers");
}

#[test]
fn area_hlc_area_baseline_columns_fills() {
    let t = Theme::dark();
    let mut style = ChartStyle::default();
    let p0 = t.series_palette[0];
    let v = vp(20.0, 3);
    assert_eq!(
        simple(ChartType::Area, &style).0[0],
        Prim::Area(with_alpha(p0, 0.28), with_alpha(p0, 0.0))
    );
    style.symbol.area.fill_top = Some(RED);
    assert!(
        matches!(simple(ChartType::Area, &style).0[0], Prim::Area(c, _) if c == RED.to_color32())
    );

    let h = simple(ChartType::HlcArea, &style);
    assert_eq!(h.0[0], Prim::Between(with_alpha(t.up, 0.2)));
    assert!(matches!(&h.0[4], Prim::Poly(_, w, c) if *w == 2.0 && *c == t.text_muted));

    // Base level at 25 % of 90..110 = 95.
    style.symbol.baseline.level_pct = 25.0;
    style.symbol.baseline.base_color = Some(BLUE);
    let b = simple(ChartType::Baseline, &style);
    assert!(b.0.contains(&Prim::DashLine(pos2(0.0, v.y_of(95.0)), BLUE.to_color32())));
    assert!(b.0.contains(&Prim::Area(with_alpha(t.up, 0.28), with_alpha(t.up, 0.05))));
    assert!(b.0.contains(&Prim::Area(
        with_alpha(t.down, 0.28),
        with_alpha(t.down, 0.05)
    )));

    // Columns: 50 % by default, colored by the previous value (the first
    // bar compares with its open).
    let c = simple(ChartType::Columns, &style);
    let (up, down) = (with_alpha(t.up, 0.5), with_alpha(t.down, 0.5));
    assert_eq!(c.rect_colors(), [up, down, up]);
    style.symbol.columns.color_by_prev_close = false;
    style.symbol.columns.up = Some(RED);
    let c = simple(ChartType::Columns, &style);
    assert_eq!(c.rect_colors(), [RED.to_color32(), down, RED.to_color32()]);
    style.symbol.columns.source = PriceSource::High;
    match simple(ChartType::Columns, &style).0[2] {
        Prim::Rect(r, _) => assert_eq!(r.top(), v.y_of(106.0)),
        ref p => panic!("{p:?}"),
    }
}

#[test]
fn high_low_borders_and_labels() {
    let mut style = ChartStyle::default();
    // Wide bars: labels fit.
    let s = data();
    let v = vp(60.0, 3);
    let bars = collect_bars(&s, 0..3, &v, PriceSource::Close);
    let r = draw(ChartType::HighLow, &style, &s, &bars, v, false, None);
    assert!(r.0.contains(&Prim::Text("106.00".into())));
    assert!(r.0.contains(&Prim::Text("94.00".into())));
    style.symbol.high_low.labels = false;
    style.symbol.high_low.border = Some(RED);
    let r = draw(ChartType::HighLow, &style, &s, &bars, v, false, None);
    assert_eq!(r.count(|p| matches!(p, Prim::Text(_))), 0);
    assert_eq!(r.count(|p| matches!(p, Prim::Stroke(_))), 3);
}

#[test]
fn bricks_use_projection_colors_and_wicks() {
    let t = Theme::dark();
    let mut style = ChartStyle::default();
    let s = data();
    let v = vp(20.0, 3);
    let bars = collect_bars(&s, 0..3, &v, PriceSource::Close);
    let r = draw(ChartType::LineBreak, &style, &s, &bars, v, false, Some(2));
    assert_eq!(r.rect_colors(), [t.up, t.down, mix(t.up, t.bg, 0.6)]);
    // Renko wicks: these bars' high/low reach beyond the bodies.
    let r = draw(ChartType::Renko, &style, &s, &bars, v, false, None);
    assert!(r.rect_colors().contains(&t.up_wick));
    style.symbol.renko.inputs.wicks = false;
    let r = draw(ChartType::Renko, &style, &s, &bars, v, false, None);
    assert_eq!(r.rect_colors().len(), 3);
}

#[test]
fn decimated_lines_use_the_selected_source() {
    let n = 4000;
    let mut s = CandleSeries::new();
    for i in 0..n {
        let p = 100.0 + (i % 10) as f64 * 0.5;
        s.upsert(Candle {
            open_time_ms: i as i64 * 60_000,
            open: p,
            high: p + 5.0,
            low: p - 5.0,
            close: p,
            volume: 1.0,
        });
    }
    let mut style = ChartStyle::default();
    style.symbol.line.source = PriceSource::High;
    let v = Viewport {
        price_min: 80.0,
        price_max: 120.0,
        ..vp(0.1, n)
    };
    let mut lod = Lod::new();
    lod.sync(&s, price_source(&style.symbol, ChartType::Line));
    let bars = lod.columns(&s, 0..n, &v, 1.0);
    let r = draw(ChartType::Line, &style, &s, &bars, v, true, None);
    // Envelope tops reach the highest high (109.5), not the highest close.
    let top =
        r.0.iter()
            .filter_map(|p| match p {
                Prim::Rect(r, _) => Some(r.top()),
                _ => None,
            })
            .fold(f32::MAX, f32::min);
    assert_eq!(top, v.y_of(109.5));
    let Some(Prim::Poly(pts, ..)) = r.0.last() else {
        panic!("no line");
    };
    let last = s.last().unwrap();
    assert_eq!(pts.last().unwrap().y, v.y_of(last.high));
}
