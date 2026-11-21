//! The Symbol page: the style block of the chart's current type (each type
//! keeps its own values), then Data modification.

use super::*;
use tt_chart::style::{
    BoxMethod, BrickColors, BrickSource, Precision, PriceSource, TimeZoneSetting, resolve,
};

fn palette0(_: &ChartStyle, t: &Theme) -> Color32 {
    t.series_palette[0]
}

/// Which brick style (Renko or Line break) a colors block edits.
#[derive(Clone, Copy)]
enum Bricks {
    Renko,
    LineBreak,
}

impl Bricks {
    fn get(self, s: &ChartStyle) -> &BrickColors {
        match self {
            Bricks::Renko => &s.symbol.renko.colors,
            Bricks::LineBreak => &s.symbol.line_break.colors,
        }
    }

    fn get_mut(self, s: &mut ChartStyle) -> &mut BrickColors {
        match self {
            Bricks::Renko => &mut s.symbol.renko.colors,
            Bricks::LineBreak => &mut s.symbol.line_break.colors,
        }
    }

    fn key(self) -> &'static str {
        match self {
            Bricks::Renko => "renko",
            Bricks::LineBreak => "lb",
        }
    }
}

/// Up / down effective brick colors.
fn brick_up(b: Bricks, s: &ChartStyle, t: &Theme) -> Color32 {
    resolve(b.get(s).up, t.up)
}
fn brick_down(b: Bricks, s: &ChartStyle, t: &Theme) -> Color32 {
    resolve(b.get(s).down, t.down)
}
fn brick_proj_up(b: Bricks, s: &ChartStyle, t: &Theme) -> Color32 {
    resolve(
        b.get(s).proj_up,
        tt_theme::mix(brick_up(b, s, t), t.bg, 0.6),
    )
}
fn brick_proj_down(b: Bricks, s: &ChartStyle, t: &Theme) -> Color32 {
    resolve(
        b.get(s).proj_down,
        tt_theme::mix(brick_down(b, s, t), t.bg, 0.6),
    )
}

impl ChartSettingsPanel {
    pub(super) fn symbol_page(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Rows {
        let t = self.chart_type;
        let mut rows = vec![section(t.label())];
        match t {
            ChartType::Candles
            | ChartType::HollowCandles
            | ChartType::VolumeCandles
            | ChartType::HeikinAshi => self.candle_block(&mut rows, t, window, cx),
            ChartType::Bars => self.bars_block(&mut rows, window, cx),
            ChartType::Line | ChartType::LineWithMarkers | ChartType::StepLine => {
                self.line_block(&mut rows, t, window, cx)
            }
            ChartType::Area => self.area_block(&mut rows, window, cx),
            ChartType::HlcArea => self.hlc_area_block(&mut rows, window, cx),
            ChartType::Baseline => self.baseline_block(&mut rows, window, cx),
            ChartType::Columns => self.columns_block(&mut rows, window, cx),
            ChartType::HighLow => self.high_low_block(&mut rows, window, cx),
            ChartType::Renko => {
                self.brick_colors(&mut rows, Bricks::Renko, window, cx);
                self.renko_inputs(&mut rows, window, cx);
            }
            ChartType::LineBreak => {
                self.brick_colors(&mut rows, Bricks::LineBreak, window, cx);
                let lines = self.number(
                    "lb.lines",
                    |s| s.symbol.line_break.lines as f64,
                    |s, v| s.symbol.line_break.lines = v.round() as u32,
                    1.0,
                    100.0,
                    1.0,
                    window,
                    cx,
                );
                rows.push(
                    Self::row("Number of lines")
                        .control(lines)
                        .into_any_element(),
                );
            }
        }
        self.data_modification(&mut rows, cx);
        rows
    }

    fn source(
        &self,
        id: &str,
        current: PriceSource,
        set: impl Fn(&mut ChartStyle, PriceSource) + 'static,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let sel = self.select(
            format!("{id}.source"),
            PriceSource::ALL.iter().map(|p| (*p, p.label())),
            current,
            set,
            cx,
        );
        Self::row("Price source").control(sel).into_any_element()
    }

    fn candle_block(
        &mut self,
        rows: &mut Rows,
        t: ChartType,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let k = format!("{t:?}");
        let c = *self.draft.symbol.candle(t);
        if t != ChartType::HollowCandles {
            rows.push(
                self.check(
                    format!("{k}.prev"),
                    "Color bars based on previous close",
                    c.color_by_prev_close,
                    move |s, v| s.symbol.candle_mut(t).color_by_prev_close = v,
                    cx,
                )
                .into_any_element(),
            );
        }
        let body_up = move |s: &ChartStyle, th: &Theme| resolve(s.symbol.candle(t).body_up, th.up);
        let body_down =
            move |s: &ChartStyle, th: &Theme| resolve(s.symbol.candle(t).body_down, th.down);
        let up = self.color(
            format!("{k}.body_up"),
            move |s| s.symbol.candle(t).body_up,
            move |s, v| s.symbol.candle_mut(t).body_up = v,
            |_, th| th.up,
            window,
            cx,
        );
        let down = self.color(
            format!("{k}.body_down"),
            move |s| s.symbol.candle(t).body_down,
            move |s, v| s.symbol.candle_mut(t).body_down = v,
            |_, th| th.down,
            window,
            cx,
        );
        rows.push(
            self.check(
                format!("{k}.body"),
                "Body",
                c.body,
                move |s, v| s.symbol.candle_mut(t).body = v,
                cx,
            )
            .control(up)
            .control(down)
            .into_any_element(),
        );
        let up = self.color(
            format!("{k}.border_up"),
            move |s| s.symbol.candle(t).border_up,
            move |s, v| s.symbol.candle_mut(t).border_up = v,
            body_up,
            window,
            cx,
        );
        let down = self.color(
            format!("{k}.border_down"),
            move |s| s.symbol.candle(t).border_down,
            move |s, v| s.symbol.candle_mut(t).border_down = v,
            body_down,
            window,
            cx,
        );
        rows.push(
            self.check(
                format!("{k}.borders"),
                "Borders",
                c.borders,
                move |s, v| s.symbol.candle_mut(t).borders = v,
                cx,
            )
            .control(up)
            .control(down)
            .into_any_element(),
        );
        let up = self.color(
            format!("{k}.wick_up"),
            move |s| s.symbol.candle(t).wick_up,
            move |s, v| s.symbol.candle_mut(t).wick_up = v,
            |_, th| th.up_wick,
            window,
            cx,
        );
        let down = self.color(
            format!("{k}.wick_down"),
            move |s| s.symbol.candle(t).wick_down,
            move |s, v| s.symbol.candle_mut(t).wick_down = v,
            |_, th| th.down_wick,
            window,
            cx,
        );
        rows.push(
            self.check(
                format!("{k}.wick"),
                "Wick",
                c.wick,
                move |s, v| s.symbol.candle_mut(t).wick = v,
                cx,
            )
            .control(up)
            .control(down)
            .into_any_element(),
        );
    }

    fn bars_block(&mut self, rows: &mut Rows, window: &mut Window, cx: &mut Context<Self>) {
        rows.push(
            check!(
                self,
                cx,
                "bars.prev",
                "Color bars based on previous close",
                symbol.bars.color_by_prev_close
            )
            .into_any_element(),
        );
        rows.push(check!(self, cx, "bars.hlc", "HLC bars", symbol.bars.hlc).into_any_element());
        rows.push(check!(self, cx, "bars.thin", "Thin bars", symbol.bars.thin).into_any_element());
        let up = color!(self, window, cx, "bars.up", symbol.bars.up, |_, t| t.up);
        rows.push(Self::row("Up color").control(up).into_any_element());
        let down = color!(self, window, cx, "bars.down", symbol.bars.down, |_, t| t
            .down);
        rows.push(Self::row("Down color").control(down).into_any_element());
    }

    fn line_block(
        &mut self,
        rows: &mut Rows,
        t: ChartType,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let k = format!("{t:?}");
        let l = *self.draft.symbol.line(t);
        rows.push(self.source(
            &k,
            l.source,
            move |s, v| s.symbol.line_mut(t).source = v,
            cx,
        ));
        let color = self.color(
            format!("{k}.color"),
            move |s| s.symbol.line(t).color,
            move |s, v| s.symbol.line_mut(t).color = v,
            palette0,
            window,
            cx,
        );
        let width = self.width(
            format!("{k}.width"),
            l.width,
            move |s, v| s.symbol.line_mut(t).width = v,
            cx,
        );
        let kind = self.line_kind(
            format!("{k}.kind"),
            l.kind,
            move |s, v| s.symbol.line_mut(t).kind = v,
            cx,
        );
        rows.push(
            Self::row("Line")
                .control(color)
                .control(width)
                .control(kind)
                .into_any_element(),
        );
    }

    fn area_block(&mut self, rows: &mut Rows, window: &mut Window, cx: &mut Context<Self>) {
        let a = self.draft.symbol.area;
        rows.push(self.source("area", a.source, |s, v| s.symbol.area.source = v, cx));
        let line = |s: &ChartStyle, t: &Theme| resolve(s.symbol.area.line, t.series_palette[0]);
        let color = color!(self, window, cx, "area.line", symbol.area.line, palette0);
        let width = self.width("area.width", a.width, |s, v| s.symbol.area.width = v, cx);
        let kind = self.line_kind("area.kind", a.kind, |s, v| s.symbol.area.kind = v, cx);
        rows.push(
            Self::row("Line")
                .control(color)
                .control(width)
                .control(kind)
                .into_any_element(),
        );
        let top = color!(
            self,
            window,
            cx,
            "area.top",
            symbol.area.fill_top,
            move |s, t| { alpha(line(s, t), 0.28) }
        );
        let bottom = color!(
            self,
            window,
            cx,
            "area.bottom",
            symbol.area.fill_bottom,
            move |s, t| alpha(line(s, t), 0.0)
        );
        rows.push(
            Self::row("Fill")
                .control(top)
                .control(bottom)
                .into_any_element(),
        );
    }

    fn hlc_area_block(&mut self, rows: &mut Rows, window: &mut Window, cx: &mut Context<Self>) {
        let h = self.draft.symbol.hlc_area;
        macro_rules! stroke {
            ($label:literal, $key:literal, $field:ident, $token:expr) => {{
                let color = color!(
                    self,
                    window,
                    cx,
                    concat!("hlc.", $key, ".color"),
                    symbol.hlc_area.$field.color,
                    $token
                );
                let width = self.width(
                    concat!("hlc.", $key, ".width"),
                    h.$field.width,
                    |s, v| s.symbol.hlc_area.$field.width = v,
                    cx,
                );
                let kind = self.line_kind(
                    concat!("hlc.", $key, ".kind"),
                    h.$field.kind,
                    |s, v| s.symbol.hlc_area.$field.kind = v,
                    cx,
                );
                rows.push(
                    Self::row($label)
                        .control(color)
                        .control(width)
                        .control(kind)
                        .into_any_element(),
                );
            }};
        }
        stroke!("High line", "high", high, |_, t| t.up);
        stroke!("Low line", "low", low, |_, t| t.down);
        stroke!("Close line", "close", close, |_, t| t.text_muted);
        let hc = color!(
            self,
            window,
            cx,
            "hlc.fill_hc",
            symbol.hlc_area.fill_high_close,
            |_, t| alpha(t.up, 0.2)
        );
        rows.push(Self::row("Fill high–close").control(hc).into_any_element());
        let cl = color!(
            self,
            window,
            cx,
            "hlc.fill_cl",
            symbol.hlc_area.fill_close_low,
            |_, t| alpha(t.down, 0.2)
        );
        rows.push(Self::row("Fill close–low").control(cl).into_any_element());
    }

    fn baseline_block(&mut self, rows: &mut Rows, window: &mut Window, cx: &mut Context<Self>) {
        let b = self.draft.symbol.baseline;
        rows.push(self.source(
            "baseline",
            b.source,
            |s, v| s.symbol.baseline.source = v,
            cx,
        ));
        let level = self.number(
            "baseline.level",
            |s| s.symbol.baseline.level_pct as f64,
            |s, v| s.symbol.baseline.level_pct = v as f32,
            0.0,
            100.0,
            1.0,
            window,
            cx,
        );
        rows.push(
            Self::row("Base level")
                .control(level)
                .unit("%")
                .into_any_element(),
        );
        let base = color!(
            self,
            window,
            cx,
            "baseline.base",
            symbol.baseline.base_color,
            |_, t| t.text_muted
        );
        rows.push(Self::row("Baseline").control(base).into_any_element());
        let top = color!(
            self,
            window,
            cx,
            "baseline.top",
            symbol.baseline.top_line,
            |_, t| t.up
        );
        let top_w = self.width(
            "baseline.top_w",
            b.top_width,
            |s, v| s.symbol.baseline.top_width = v,
            cx,
        );
        rows.push(
            Self::row("Top line")
                .control(top)
                .control(top_w)
                .into_any_element(),
        );
        let bottom = color!(
            self,
            window,
            cx,
            "baseline.bottom",
            symbol.baseline.bottom_line,
            |_, t| t.down
        );
        let bottom_w = self.width(
            "baseline.bottom_w",
            b.bottom_width,
            |s, v| s.symbol.baseline.bottom_width = v,
            cx,
        );
        rows.push(
            Self::row("Bottom line")
                .control(bottom)
                .control(bottom_w)
                .into_any_element(),
        );
        let f1 = color!(
            self,
            window,
            cx,
            "baseline.tf1",
            symbol.baseline.top_fill_1,
            |_, t| alpha(t.up, 0.28)
        );
        let f2 = color!(
            self,
            window,
            cx,
            "baseline.tf2",
            symbol.baseline.top_fill_2,
            |_, t| alpha(t.up, 0.05)
        );
        rows.push(
            Self::row("Top fill area")
                .control(f1)
                .control(f2)
                .into_any_element(),
        );
        let f1 = color!(
            self,
            window,
            cx,
            "baseline.bf1",
            symbol.baseline.bottom_fill_1,
            |_, t| alpha(t.down, 0.05)
        );
        let f2 = color!(
            self,
            window,
            cx,
            "baseline.bf2",
            symbol.baseline.bottom_fill_2,
            |_, t| alpha(t.down, 0.28)
        );
        rows.push(
            Self::row("Bottom fill area")
                .control(f1)
                .control(f2)
                .into_any_element(),
        );
    }

    fn columns_block(&mut self, rows: &mut Rows, window: &mut Window, cx: &mut Context<Self>) {
        let c = self.draft.symbol.columns;
        rows.push(self.source("columns", c.source, |s, v| s.symbol.columns.source = v, cx));
        rows.push(
            check!(
                self,
                cx,
                "columns.prev",
                "Color bars based on previous close",
                symbol.columns.color_by_prev_close
            )
            .into_any_element(),
        );
        let up = color!(self, window, cx, "columns.up", symbol.columns.up, |_, t| {
            alpha(t.up, 0.5)
        });
        rows.push(Self::row("Up color").control(up).into_any_element());
        let down = color!(
            self,
            window,
            cx,
            "columns.down",
            symbol.columns.down,
            |_, t| { alpha(t.down, 0.5) }
        );
        rows.push(Self::row("Down color").control(down).into_any_element());
    }

    fn high_low_block(&mut self, rows: &mut Rows, window: &mut Window, cx: &mut Context<Self>) {
        let body = color!(self, window, cx, "hl.body", symbol.high_low.body, palette0);
        rows.push(Self::row("Body").control(body).into_any_element());
        let border = color!(
            self,
            window,
            cx,
            "hl.border",
            symbol.high_low.border,
            palette0
        );
        rows.push(
            check!(self, cx, "hl.borders", "Borders", symbol.high_low.borders)
                .control(border)
                .into_any_element(),
        );
        let label = color!(
            self,
            window,
            cx,
            "hl.label",
            symbol.high_low.label,
            palette0
        );
        rows.push(
            check!(self, cx, "hl.labels", "Labels", symbol.high_low.labels)
                .control(label)
                .into_any_element(),
        );
    }

    /// Up / down / projection colors with their borders (Renko, Line break).
    fn brick_colors(
        &mut self,
        rows: &mut Rows,
        b: Bricks,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let k = b.key();
        type Get = fn(&BrickColors) -> ColorSlot;
        type Set = fn(&mut BrickColors, ColorSlot);
        type Default = fn(Bricks, &ChartStyle, &Theme) -> Color32;
        /// (label, fill id, get, set, default, border id, get, set, default)
        type Spec = (
            &'static str,
            &'static str,
            Get,
            Set,
            Default,
            &'static str,
            Get,
            Set,
            Default,
        );
        let rows_spec: [Spec; 4] = [
            (
                "Up bars",
                "up",
                |c| c.up,
                |c, v| c.up = v,
                |_, _, t| t.up,
                "border_up",
                |c| c.border_up,
                |c, v| c.border_up = v,
                brick_up,
            ),
            (
                "Down bars",
                "down",
                |c| c.down,
                |c, v| c.down = v,
                |_, _, t| t.down,
                "border_down",
                |c| c.border_down,
                |c, v| c.border_down = v,
                brick_down,
            ),
            (
                "Projection up bars",
                "proj_up",
                |c| c.proj_up,
                |c, v| c.proj_up = v,
                |b, s, t| tt_theme::mix(brick_up(b, s, t), t.bg, 0.6),
                "proj_border_up",
                |c| c.proj_border_up,
                |c, v| c.proj_border_up = v,
                brick_proj_up,
            ),
            (
                "Projection down bars",
                "proj_down",
                |c| c.proj_down,
                |c, v| c.proj_down = v,
                |b, s, t| tt_theme::mix(brick_down(b, s, t), t.bg, 0.6),
                "proj_border_down",
                |c| c.proj_border_down,
                |c, v| c.proj_border_down = v,
                brick_proj_down,
            ),
        ];
        for (label, fill_id, get, set, def, border_id, bget, bset, bdef) in rows_spec {
            let fill = self.color(
                format!("{k}.{fill_id}"),
                move |s| get(b.get(s)),
                move |s, v| set(b.get_mut(s), v),
                move |s, t| def(b, s, t),
                window,
                cx,
            );
            let border = self.color(
                format!("{k}.{border_id}"),
                move |s| bget(b.get(s)),
                move |s, v| bset(b.get_mut(s), v),
                move |s, t| bdef(b, s, t),
                window,
                cx,
            );
            rows.push(
                Self::row(label)
                    .control(fill)
                    .control(border)
                    .into_any_element(),
            );
        }
    }

    fn renko_inputs(&mut self, rows: &mut Rows, window: &mut Window, cx: &mut Context<Self>) {
        let up = color!(
            self,
            window,
            cx,
            "renko.wick_up",
            symbol.renko.wick_up,
            |_, t| t.up_wick
        );
        let down = color!(
            self,
            window,
            cx,
            "renko.wick_down",
            symbol.renko.wick_down,
            |_, t| { t.down_wick }
        );
        rows.push(
            check!(self, cx, "renko.wicks", "Wicks", symbol.renko.inputs.wicks)
                .control(up)
                .control(down)
                .into_any_element(),
        );
        let i = self.draft.symbol.renko.inputs;
        let source = self.select(
            "renko.source",
            BrickSource::ALL.iter().map(|v| (*v, v.label())),
            i.source,
            |s, v| s.symbol.renko.inputs.source = v,
            cx,
        );
        rows.push(Self::row("Source").control(source).into_any_element());
        let method = self.select(
            "renko.method",
            BoxMethod::ALL.iter().map(|v| (*v, v.label())),
            i.method,
            |s, v| s.symbol.renko.inputs.method = v,
            cx,
        );
        rows.push(
            Self::row("Box size assignment method")
                .control(method)
                .into_any_element(),
        );
        let row = match i.method {
            BoxMethod::Atr => {
                let n = self.number(
                    "renko.atr",
                    |s| s.symbol.renko.inputs.atr_length as f64,
                    |s, v| s.symbol.renko.inputs.atr_length = v.round().max(1.0) as u32,
                    1.0,
                    500.0,
                    1.0,
                    window,
                    cx,
                );
                Self::row("ATR length").control(n)
            }
            BoxMethod::Traditional => {
                let n = self.number(
                    "renko.box",
                    |s| s.symbol.renko.inputs.box_size,
                    |s, v| s.symbol.renko.inputs.box_size = v,
                    0.000_000_01,
                    1e12,
                    1.0,
                    window,
                    cx,
                );
                Self::row("Box size").control(n)
            }
            BoxMethod::PercentLtp => {
                let n = self.number(
                    "renko.pct",
                    |s| s.symbol.renko.inputs.percent,
                    |s, v| s.symbol.renko.inputs.percent = v,
                    0.001,
                    100.0,
                    0.1,
                    window,
                    cx,
                );
                Self::row("Percentage").control(n).unit("%")
            }
        };
        rows.push(row.indent().into_any_element());
    }

    fn data_modification(&self, rows: &mut Rows, cx: &mut Context<Self>) {
        rows.push(section("Data modification"));
        let d = &self.draft.symbol.data;
        let mut precision: Vec<(Precision, String)> = vec![(Precision::Default, "Default".into())];
        precision.extend((0..=10u8).map(|n| {
            let label = if n == 0 {
                "1".to_owned()
            } else {
                format!("0.{}1", "0".repeat(n as usize - 1))
            };
            (Precision::Decimals(n), label)
        }));
        if let Precision::Fraction(f) = d.precision {
            precision.push((d.precision, format!("1/{f}")));
        }
        let sel = self.select(
            "data.precision",
            precision,
            d.precision,
            |s, v| s.symbol.data.precision = v,
            cx,
        );
        rows.push(Self::row("Precision").control(sel).into_any_element());
        // Exchange, UTC, Local and TradingView's zones, with current offsets.
        // (Binance is the only venue: its exchange zone is UTC.)
        let mut zones = tt_session::tz::choices(Some("UTC"), crate::source::now_ms());
        if let TimeZoneSetting::Iana(z) = &d.time_zone
            && !zones.iter().any(|(v, _)| v == &d.time_zone)
        {
            zones.push((d.time_zone.clone(), z.clone()));
        }
        let sel = self
            .select(
                "data.tz",
                zones,
                d.time_zone.clone(),
                |s, v| s.symbol.data.time_zone = v,
                cx,
            )
            .width(W_SELECT_WIDE);
        rows.push(Self::row("Time zone").control(sel).into_any_element());
    }
}
