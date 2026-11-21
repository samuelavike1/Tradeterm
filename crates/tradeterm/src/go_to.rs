//! The bottom toolbar's "Go to" dialog (Alt+G), laid out like
//! TradingView's (docs/specs/bottom-panel.md): comfortable density, tabs
//! Date | Custom range, `YYYY-MM-DD` + `HH:MM` fields over a month calendar
//! (Monday first, no future days), footer Cancel / Go to.
//!
//! Times are wall-clock times in the chart's time zone. Date scrolls the
//! chart so that bar is in the middle (zoom kept); Custom range fits the
//! span into the plot. Both load older history first when needed.

use crate::app::TradeTerm;
use crate::source::now_ms;
use chrono::{Datelike as _, NaiveDate, Weekday};
use gpui_kit::component::calendar::{Calendar, CalendarEvent, CalendarState, Date, Matcher};
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::*;
use tt_chart::TzOffsets;
use tt_chart::range::{ViewTarget, format_date_time, parse_date_time, to_utc};
use tt_ui::prelude::*;
use tt_ui::tokens::*;
use tt_ui::{DialogOpts, hsla};

/// Dialog width (TradingView's is 302; ours fits the wider kit fields).
const W_DIALOG: f32 = 340.0;
/// Date and time field widths.
const W_DATE: f32 = 172.0;
const W_TIME: f32 = 100.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Date,
    Range,
}

/// Which date field the calendar edits.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Slot {
    Date,
    From,
    To,
}

pub struct GoTo {
    app: WeakEntity<TradeTerm>,
    tz: TzOffsets,
    tab: Tab,
    slot: Slot,
    date: Entity<InputState>,
    time: Entity<InputState>,
    from_date: Entity<InputState>,
    from_time: Entity<InputState>,
    to_date: Entity<InputState>,
    to_time: Entity<InputState>,
    calendar: Entity<CalendarState>,
    error: Option<SharedString>,
    _subs: Vec<Subscription>,
}

fn naive(date: &str) -> Option<NaiveDate> {
    let ms = parse_date_time(date, "")?;
    let (y, m, d) = tt_core::civil_from_days(ms.div_euclid(86_400_000));
    NaiveDate::from_ymd_opt(y as i32, m, d)
}

impl GoTo {
    /// `visible`: the chart's visible span (UTC), the Custom range default.
    pub fn new(
        app: WeakEntity<TradeTerm>,
        tz: TzOffsets,
        visible: Option<(i64, i64)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let now = now_ms();
        let (today, _) = format_date_time(tz.local_ms(now));
        let (from, to) = visible.unwrap_or((now - 30 * 86_400_000, now));
        let (fd, ft) = format_date_time(tz.local_ms(from));
        let (td, tt) = format_date_time(tz.local_ms(to.min(now)));
        let field = |v: &str, ph: &str, window: &mut Window, cx: &mut Context<Self>| {
            let (v, ph) = (v.to_owned(), ph.to_owned());
            cx.new(|cx| InputState::new(window, cx).placeholder(ph).default_value(v))
        };
        let date = field(&today, "YYYY-MM-DD", window, cx);
        let time = field("00:00", "HH:MM", window, cx);
        let from_date = field(&fd, "YYYY-MM-DD", window, cx);
        let from_time = field(&ft, "HH:MM", window, cx);
        let to_date = field(&td, "YYYY-MM-DD", window, cx);
        let to_time = field(&tt, "HH:MM", window, cx);
        let today_naive = naive(&today);
        let calendar = cx.new(|cx| {
            let mut c = CalendarState::new(window, cx)
                .disabled_matcher(Matcher::interval(None, today_naive));
            if let Some(d) = today_naive {
                c.set_date(d, window, cx);
            }
            c
        });
        let mut subs = vec![cx.subscribe_in(
            &calendar,
            window,
            |this: &mut Self, _, ev: &CalendarEvent, window, cx| {
                let CalendarEvent::Selected(Date::Single(Some(d))) = ev else {
                    return;
                };
                let text = format!("{:04}-{:02}-{:02}", d.year(), d.month(), d.day());
                let input = this.slot_input(this.slot).clone();
                input.update(cx, |s, cx| s.set_value(text, window, cx));
                this.error = None;
                cx.notify();
            },
        )];
        for (input, slot) in [
            (&date, Slot::Date),
            (&from_date, Slot::From),
            (&to_date, Slot::To),
        ] {
            subs.push(cx.subscribe_in(
                input,
                window,
                move |this: &mut Self, input, ev: &InputEvent, window, cx| match ev {
                    // The calendar follows the date field being edited.
                    InputEvent::Focus => {
                        this.slot = slot;
                        let text = input.read(cx).value();
                        this.sync_calendar(&text, window, cx);
                        cx.notify();
                    }
                    InputEvent::Change => {
                        this.error = None;
                        let text = input.read(cx).value();
                        this.sync_calendar(&text, window, cx);
                        cx.notify();
                    }
                    InputEvent::PressEnter { .. } => this.submit_and_close(window, cx),
                    _ => {}
                },
            ));
        }
        for input in [&time, &from_time, &to_time] {
            subs.push(cx.subscribe_in(
                input,
                window,
                |this: &mut Self, _, ev: &InputEvent, window, cx| match ev {
                    InputEvent::Change => {
                        if this.error.take().is_some() {
                            cx.notify();
                        }
                    }
                    InputEvent::PressEnter { .. } => this.submit_and_close(window, cx),
                    _ => {}
                },
            ));
        }
        Self {
            app,
            tz,
            tab: Tab::Date,
            slot: Slot::Date,
            date,
            time,
            from_date,
            from_time,
            to_date,
            to_time,
            calendar,
            error: None,
            _subs: subs,
        }
    }

    /// Opens the dialog with the date field focused.
    pub fn open(panel: Entity<Self>, window: &mut Window, cx: &mut App) {
        let focus = panel.read(cx).date.read(cx).focus_handle(cx);
        let p = panel.downgrade();
        let footer = DialogFooter::new()
            .comfortable()
            .cancel()
            .ok("Go to")
            .on_ok(move |_, cx| p.update(cx, |p, cx| p.submit(cx)).unwrap_or(true));
        tt_ui::open_dialog(
            window,
            cx,
            DialogOpts::new("Go to", DialogSize::Sm)
                .comfortable()
                .width(W_DIALOG)
                .footer(footer)
                .on_open(move |w, cx| focus.focus(w, cx)),
            panel,
        );
    }

    fn slot_input(&self, slot: Slot) -> &Entity<InputState> {
        match slot {
            Slot::Date => &self.date,
            Slot::From => &self.from_date,
            Slot::To => &self.to_date,
        }
    }

    fn sync_calendar(&self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(d) = naive(text) {
            self.calendar.update(cx, |c, cx| c.set_date(d, window, cx));
        }
    }

    fn set_tab(&mut self, tab: Tab, window: &mut Window, cx: &mut Context<Self>) {
        self.tab = tab;
        self.slot = if tab == Tab::Date {
            Slot::Date
        } else {
            Slot::From
        };
        self.error = None;
        let input = self.slot_input(self.slot).clone();
        let text = input.read(cx).value();
        self.sync_calendar(&text, window, cx);
        let focus = input.read(cx).focus_handle(cx);
        focus.focus(window, cx);
        cx.notify();
    }

    /// UTC time of a date + time field pair.
    fn read(&self, date: &Entity<InputState>, time: &Entity<InputState>, cx: &App) -> Option<i64> {
        let local = parse_date_time(&date.read(cx).value(), &time.read(cx).value())?;
        Some(to_utc(local, &self.tz))
    }

    /// The target of the current tab, or an error to show.
    fn target(&self, cx: &App) -> Result<ViewTarget, &'static str> {
        match self.tab {
            Tab::Date => self
                .read(&self.date, &self.time, cx)
                .map(ViewTarget::Date)
                .ok_or("Enter a date as YYYY-MM-DD and a time as HH:MM."),
            Tab::Range => {
                let from = self.read(&self.from_date, &self.from_time, cx);
                let to = self.read(&self.to_date, &self.to_time, cx);
                match (from, to) {
                    (Some(from), Some(to)) if from < to => Ok(ViewTarget::Span { from, to }),
                    (Some(_), Some(_)) => Err("The end must be after the start."),
                    _ => Err("Enter dates as YYYY-MM-DD and times as HH:MM."),
                }
            }
        }
    }

    /// Goes to the target; returns true to close the dialog.
    fn submit(&mut self, cx: &mut Context<Self>) -> bool {
        match self.target(cx) {
            Ok(target) => {
                let app = self.app.clone();
                cx.defer(move |cx| {
                    app.update(cx, |a, cx| a.go_to(target, cx)).ok();
                });
                true
            }
            Err(e) => {
                self.error = Some(e.into());
                cx.notify();
                false
            }
        }
    }

    fn submit_and_close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        use gpui_kit::component::WindowExt as _;
        if self.submit(cx) {
            window.close_dialog(cx);
        }
    }
}

impl Render for GoTo {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.ui().clone();
        let this = cx.entity().downgrade();
        let nav = NavTabs::horizontal()
            .item("date", "Date", None)
            .item("range", "Custom range", None)
            .selected(if self.tab == Tab::Date {
                "date"
            } else {
                "range"
            })
            .on_change(move |id, window, cx| {
                let tab = if id == "date" { Tab::Date } else { Tab::Range };
                this.update(cx, |p, cx| p.set_tab(tab, window, cx)).ok();
            });
        let row = |d: &Entity<InputState>, tm: &Entity<InputState>| {
            h_flex()
                .gap(px(C_GAP))
                .child(Field::new(d).width(W_DATE))
                .child(Field::new(tm).width(W_TIME))
        };
        let fields = match self.tab {
            Tab::Date => v_flex().child(row(&self.date, &self.time)),
            Tab::Range => v_flex()
                .gap(px(C_GAP))
                .child(row(&self.from_date, &self.from_time))
                .child(row(&self.to_date, &self.to_time)),
        };
        let body = v_flex()
            .gap(px(C_GAP * 2.0))
            .child(fields)
            .child(
                div()
                    .flex()
                    .justify_center()
                    .child(Calendar::new(&self.calendar).first_day_of_week(Weekday::Mon)),
            )
            .children(self.error.clone().map(|e| {
                div()
                    .text_size(px(TEXT_SMALL))
                    .text_color(hsla(t.danger))
                    .child(e)
            }));
        DialogFrame::new(DialogSize::Sm)
            .id("go-to")
            .comfortable()
            .nav(nav)
            .body(body)
    }
}
