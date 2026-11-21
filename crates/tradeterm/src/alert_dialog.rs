//! "Create alert" / "Edit alert", laid out like TradingView's dialog
//! (docs/specs/alerts.md §2; comfortable density, `AL_*` tokens): a
//! 131 px label column and a 305 px control column, Condition (source,
//! operator, value / channel bounds / "amount in N bars", interval),
//! Trigger and Expiration, then summary rows that open the Message and
//! Notifications tabs (TradingView opens them as sub-pages; here they are
//! the dialog's top tabs). Footer Cancel / Create (Save when editing).
//!
//! Also the alert popup ("Show popup" notification).

use crate::alerts::{center, round_to};
use gpui_kit::component::input::{InputEvent, InputState, Textarea, TextareaState};
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use tt_core::Timeframe;
use tt_session::alert_engine::now_ms;
use tt_session::alerts::{
    Alert, ExpiryPreset, LogEntry, Operator, PLACEHOLDERS, Target, Trigger, format_local_input,
    format_local_long, format_local_short, parse_local_input,
};
use tt_ui::prelude::*;
use tt_ui::tokens::*;
use tt_ui::{DialogOpts, hsla};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Settings,
    Message,
    Notifications,
}

impl Tab {
    fn id(self) -> &'static str {
        match self {
            Tab::Settings => "settings",
            Tab::Message => "message",
            Tab::Notifications => "notifications",
        }
    }
}

/// The Expiration select.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Expiry {
    Preset(ExpiryPreset),
    Custom,
}

/// Intervals offered for the per-bar triggers and Moving operators.
const INTERVALS: [Timeframe; 10] = [
    Timeframe::M1,
    Timeframe::M3,
    Timeframe::M5,
    Timeframe::M15,
    Timeframe::M30,
    Timeframe::H1,
    Timeframe::H4,
    Timeframe::D1,
    Timeframe::W1,
    Timeframe::MN1,
];

pub struct AlertDialog {
    draft: Alert,
    is_new: bool,
    tab: Tab,
    /// Value (Crossing…), upper bound (channels) or amount (Moving).
    value: Entity<InputState>,
    lower: Entity<InputState>,
    bars: Entity<InputState>,
    name: Entity<InputState>,
    message: Entity<TextareaState>,
    custom_date: Entity<InputState>,
    expiry: Expiry,
    /// The message was typed by the user (else it follows the condition,
    /// like TradingView's).
    message_edited: bool,
    /// Writing the automatic message into the field (not a user edit).
    syncing: bool,
    error: Option<SharedString>,
    /// The price the dialog opened with (operator switches start from it).
    base_price: f64,
    _subs: Vec<Subscription>,
}

fn num(v: f64, precision: usize) -> String {
    format!("{v:.precision$}")
}

impl AlertDialog {
    pub fn new(draft: Alert, is_new: bool, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let p = draft.precision;
        let c = &draft.condition;
        let (value, lower) = match c.target {
            Target::Value(v) if c.op.is_moving() => (num(v, 2), num(c.lower, p)),
            Target::Value(v) => (num(v, p), num(c.lower, p)),
            Target::Line(l) => (num(l.p1, p), num(c.lower, p)),
        };
        let base_price = match c.target {
            Target::Value(v) if !c.op.is_moving() => v,
            Target::Line(l) => l.p1,
            _ => center(cx)
                .and_then(|c| c.read(cx).chart.as_ref().and_then(|x| x.price))
                .unwrap_or(0.0),
        };
        let input = |text: String, window: &mut Window, cx: &mut Context<Self>| {
            cx.new(|cx| InputState::new(window, cx).default_value(text))
        };
        let value = input(value, window, cx);
        let lower = input(lower, window, cx);
        let bars = input(c.bars.max(1).to_string(), window, cx);
        let name = input(draft.name.clone(), window, cx);
        let custom_date = input(
            format_local_input(
                draft
                    .expiration
                    .unwrap_or_else(|| ExpiryPreset::OneMonth.resolve(now_ms()).unwrap_or(0)),
            ),
            window,
            cx,
        );
        let message_edited = !draft.message.is_empty() && draft.message != draft.default_message();
        let text = if draft.message.is_empty() {
            draft.default_message()
        } else {
            draft.message.clone()
        };
        let message = cx.new(|cx| TextareaState::new(window, cx).default_value(text));
        let expiry = match draft.expiration {
            None => Expiry::Preset(ExpiryPreset::OpenEnded),
            Some(_) if is_new => Expiry::Preset(ExpiryPreset::OneMonth),
            Some(_) => Expiry::Custom,
        };
        let mut subs = Vec::new();
        for field in [&value, &lower, &bars, &name, &custom_date] {
            subs.push(cx.subscribe(field, |this, _, ev: &InputEvent, cx| {
                if matches!(ev, InputEvent::Change) {
                    this.read_fields(cx);
                    this.error = None;
                    cx.notify();
                }
            }));
        }
        subs.push(cx.subscribe(&message, |this, _, ev: &InputEvent, cx| {
            if matches!(ev, InputEvent::Change) && !this.syncing {
                this.message_edited = true;
                cx.notify();
            }
        }));
        let mut me = Self {
            draft,
            is_new,
            tab: Tab::Settings,
            value,
            lower,
            bars,
            name,
            message,
            custom_date,
            expiry,
            message_edited,
            syncing: false,
            error: None,
            base_price,
            _subs: subs,
        };
        me.read_fields(cx);
        me
    }

    /// Opens the dialog for `draft` (`is_new`: "Create", else "Save").
    pub fn open(draft: Alert, is_new: bool, window: &mut Window, cx: &mut App) {
        let title = format!(
            "{} alert on {}",
            if is_new { "Create" } else { "Edit" },
            draft.symbol
        );
        let panel = cx.new(|cx| Self::new(draft, is_new, window, cx));
        let p = panel.downgrade();
        let footer = DialogFooter::new()
            .comfortable()
            .cancel()
            .ok(if is_new { "Create" } else { "Save" })
            .on_ok(move |_, cx| p.update(cx, |p, cx| p.save(cx)).unwrap_or(true));
        tt_ui::open_dialog(
            window,
            cx,
            DialogOpts::new(title, DialogSize::Sm)
                .comfortable()
                .width(AL_W_DIALOG)
                .footer(footer),
            panel,
        );
    }

    /// Parses the fields into the draft (invalid text keeps the last good
    /// value; [`Self::save`] reports it).
    fn read_fields(&mut self, cx: &App) {
        let parse = |e: &Entity<InputState>| -> Option<f64> {
            let s = e.read(cx).value().replace([',', ' '], "");
            s.parse::<f64>().ok().filter(|v| v.is_finite())
        };
        let c = &mut self.draft.condition;
        if let Target::Value(_) = c.target
            && let Some(v) = parse(&self.value)
        {
            c.target = Target::Value(v);
        }
        if let Some(v) = parse(&self.lower) {
            c.lower = v;
        }
        if let Ok(n) = self.bars.read(cx).value().trim().parse::<u32>() {
            c.bars = n.clamp(1, tt_session::alerts::MAX_BARS);
        }
        self.draft.name = self.name.read(cx).value().trim().to_owned();
        self.draft.expiration = match self.expiry {
            Expiry::Preset(p) => p.resolve(now_ms()),
            Expiry::Custom => parse_local_input(&self.custom_date.read(cx).value()),
        };
    }

    /// Another operator: values that fit it (TradingView's defaults: 1.00 in
    /// 1 bar for Moving, bounds around the price for channels).
    fn set_op(&mut self, op: Operator, window: &mut Window, cx: &mut Context<Self>) {
        let old = self.draft.condition.op;
        let p = self.draft.precision;
        let set = |e: &Entity<InputState>, text: String, window: &mut Window, cx: &mut App| {
            e.update(cx, |s, cx| s.set_value(text, window, cx));
        };
        if op.is_moving() && !old.is_moving() {
            set(&self.value, num(1.0, 2), window, cx);
            set(&self.bars, "1".to_owned(), window, cx);
        } else if !op.is_moving() && old.is_moving() {
            set(&self.value, num(self.base_price, p), window, cx);
        }
        if op.is_channel() && !old.is_channel() {
            let v = match self.draft.condition.target {
                Target::Value(v) if !old.is_moving() => v,
                _ => self.base_price,
            };
            set(&self.value, num(round_to(v * 1.005, p), p), window, cx);
            set(&self.lower, num(round_to(v * 0.995, p), p), window, cx);
        }
        self.draft.condition.op = op;
        self.read_fields(cx);
        cx.notify();
    }

    /// The message as it will be saved.
    fn effective_message(&self, cx: &App) -> String {
        if self.message_edited {
            self.message.read(cx).value().to_string()
        } else {
            self.draft.default_message()
        }
    }

    /// Writes the automatic message into the field (Message tab).
    fn sync_message(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.message_edited {
            return;
        }
        let text = self.draft.default_message();
        if self.message.read(cx).value() != text {
            self.syncing = true;
            self.message
                .update(cx, |m, cx| m.set_value(text, window, cx));
            self.syncing = false;
        }
    }

    fn set_tab(&mut self, tab: Tab, window: &mut Window, cx: &mut Context<Self>) {
        self.tab = tab;
        if tab == Tab::Message {
            self.sync_message(window, cx);
        }
        cx.notify();
    }

    /// Validates and saves; `true` closes the dialog.
    fn save(&mut self, cx: &mut Context<Self>) -> bool {
        self.read_fields(cx);
        let c = &self.draft.condition;
        let bad = |e: &Entity<InputState>| {
            !e.read(cx)
                .value()
                .replace([',', ' '], "")
                .parse::<f64>()
                .is_ok_and(|v| v.is_finite())
        };
        let error = if matches!(c.target, Target::Value(_)) && bad(&self.value) {
            Some("Enter a number.")
        } else if c.op.is_channel() && bad(&self.lower) {
            Some("Enter a number for the lower bound.")
        } else if c.op.is_moving() && self.bars.read(cx).value().trim().parse::<u32>().is_err() {
            Some("Enter a number of bars.")
        } else if self.expiry == Expiry::Custom
            && self.draft.expiration.is_none_or(|e| e <= now_ms())
        {
            Some("Enter a future date as YYYY-MM-DD HH:MM.")
        } else {
            None
        };
        if let Some(e) = error {
            self.error = Some(e.into());
            self.tab = Tab::Settings;
            cx.notify();
            return false;
        }
        let mut a = self.draft.clone();
        if let Target::Value(v) = a.condition.target
            && !a.condition.op.is_moving()
        {
            a.condition.target = Target::Value(round_to(v, a.precision));
        }
        a.message = self.effective_message(cx);
        if let Some(center) = center(cx) {
            cx.defer(move |cx| {
                center.update(cx, |c, cx| {
                    c.upsert(a, cx);
                });
            });
        }
        true
    }

    // ---- rendering ------------------------------------------------------

    /// A form row: muted label column, control column.
    fn row(label: impl Into<SharedString>, controls: impl IntoElement, t: &tt_theme::Theme) -> Div {
        h_flex()
            .w_full()
            .items_start()
            .child(
                div()
                    .flex_none()
                    .w(px(AL_W_LABEL))
                    .h(px(C_CONTROL))
                    .flex()
                    .items_center()
                    .text_color(hsla(t.text_muted))
                    .child(label.into()),
            )
            .child(v_flex().w(px(AL_W_CONTROL)).gap(px(C_GAP)).child(controls))
    }

    /// A summary row (Trigger…, Message, Notifications): label, value and a
    /// chevron; a click runs `on`.
    fn summary(
        id: &'static str,
        label: &'static str,
        value: impl Into<SharedString>,
        t: &tt_theme::Theme,
        on: impl Fn(&mut Window, &mut App) + 'static,
    ) -> impl IntoElement {
        h_flex()
            .w_full()
            .h(px(AL_H_ROW))
            .items_center()
            .child(
                div()
                    .flex_none()
                    .w(px(AL_W_LABEL))
                    .text_color(hsla(t.text_muted))
                    .child(label),
            )
            .child(
                h_flex()
                    .id(id)
                    .max_w(px(AL_W_CONTROL))
                    .min_w_0()
                    .gap(px(S1))
                    .items_center()
                    .px(px(S1))
                    .ml(px(-S1))
                    .rounded(px(R_SM))
                    .cursor_pointer()
                    .hover(|s| s.bg(hsla(t.hover)))
                    .on_click(move |_, w, cx| on(w, cx))
                    .child(
                        div()
                            .min_w_0()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .child(value.into()),
                    )
                    .child(glyph(Glyph::CHEVRON_RIGHT, ICON_SM)),
            )
    }

    fn settings_tab(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let t = cx.ui().clone();
        let this = cx.entity().downgrade();
        let c = self.draft.condition;
        let is_line = matches!(c.target, Target::Line(_));

        // Source: "Price" (or the drawing the alert follows).
        let source_label: SharedString = match c.target {
            Target::Line(l) => l.kind.label().into(),
            Target::Value(_) => "Price".into(),
        };
        let source = Select::new("al-source", [(0u8, source_label)], 0)
            .comfortable()
            .width(AL_W_CONTROL);
        let ops: Vec<(Operator, &str)> = if is_line {
            Operator::LINE.iter().map(|o| (*o, o.label())).collect()
        } else {
            Operator::ALL.iter().map(|o| (*o, o.label())).collect()
        };
        let w = this.clone();
        let op = Select::new("al-op", ops, c.op)
            .comfortable()
            .width(AL_W_CONTROL)
            .on_change(move |op, window, cx| {
                w.update(cx, |p, cx| p.set_op(op, window, cx)).ok();
            });
        let half_select = |id: &'static str| {
            Select::new(id, [(0u8, "Value")], 0)
                .comfortable()
                .width(AL_W_HALF)
        };
        let mut condition = v_flex().gap(px(C_GAP)).child(source).child(op);
        let mut extra_rows: Vec<AnyElement> = Vec::new();
        if c.op.is_channel() {
            extra_rows.push(
                Self::row(
                    "Upper bound",
                    h_flex()
                        .gap(px(C_GAP))
                        .child(half_select("al-upper-src"))
                        .child(Field::new(&self.value).width(AL_W_HALF)),
                    &t,
                )
                .into_any_element(),
            );
            extra_rows.push(
                Self::row(
                    "Lower bound",
                    h_flex()
                        .gap(px(C_GAP))
                        .child(half_select("al-lower-src"))
                        .child(Field::new(&self.lower).width(AL_W_HALF)),
                    &t,
                )
                .into_any_element(),
            );
        } else if c.op.is_moving() {
            let unit = if c.bars == 1 { "bar" } else { "bars" };
            condition = condition.child(
                h_flex()
                    .gap(px(C_GAP))
                    .items_center()
                    .child(Field::new(&self.value).width(120.0))
                    .child(div().text_color(hsla(t.text_muted)).child("in"))
                    .child(Field::new(&self.bars).width(100.0))
                    .child(div().text_color(hsla(t.text_muted)).child(unit)),
            );
        } else if !is_line {
            condition = condition.child(
                h_flex()
                    .gap(px(C_GAP))
                    .child(half_select("al-value-src"))
                    .child(Field::new(&self.value).width(AL_W_HALF)),
            );
        }
        // Interval: the bar of "per bar" triggers and Moving operators.
        let per_bar = c.op.is_moving()
            || matches!(
                self.draft.trigger,
                Trigger::OncePerBar | Trigger::OncePerBarClose
            );
        let mut intervals: Vec<Timeframe> = INTERVALS.to_vec();
        if !intervals.contains(&self.draft.interval) {
            intervals.push(self.draft.interval);
            intervals.sort_by_key(|t| t.duration_ms());
        }
        let w = this.clone();
        let interval = Select::new(
            "al-interval",
            intervals.into_iter().map(|tf| (tf, tf.long_name())),
            self.draft.interval,
        )
        .comfortable()
        .width(AL_W_CONTROL)
        .on_change(move |tf, _, cx| {
            w.update(cx, |p, cx| {
                p.draft.interval = tf;
                cx.notify();
            })
            .ok();
        });

        let w = this.clone();
        let trigger = Select::new(
            "al-trigger",
            Trigger::ALL.iter().map(|t| (*t, t.label())),
            self.draft.trigger,
        )
        .comfortable()
        .width(AL_W_CONTROL)
        .on_change(move |tr, _, cx| {
            w.update(cx, |p, cx| {
                p.draft.trigger = tr;
                cx.notify();
            })
            .ok();
        });
        let now = now_ms();
        let mut expiries: Vec<(Expiry, String)> = ExpiryPreset::ALL
            .iter()
            .map(|p| {
                let hint = p
                    .resolve(now)
                    .map_or_else(|| "Won't expire".to_owned(), format_local_short);
                (Expiry::Preset(*p), format!("{} · {hint}", p.label()))
            })
            .collect();
        expiries.push((Expiry::Custom, "Custom date".to_owned()));
        let w = this.clone();
        let expiry = Select::new("al-expiry", expiries, self.expiry)
            .comfortable()
            .width(AL_W_CONTROL)
            .on_change(move |e, _, cx| {
                w.update(cx, |p, cx| {
                    p.expiry = e;
                    p.read_fields(cx);
                    cx.notify();
                })
                .ok();
            });
        let expiry_text: SharedString = match self.draft.expiration {
            None if self.expiry == Expiry::Custom => "Enter a date as YYYY-MM-DD HH:MM".into(),
            None => "The alert won't expire".into(),
            Some(e) => format!("Expires {}", format_local_long(e)).into(),
        };
        let mut expiry_col = v_flex().gap(px(C_GAP)).child(expiry);
        if self.expiry == Expiry::Custom {
            expiry_col = expiry_col.child(Field::new(&self.custom_date).width(AL_W_CONTROL));
        }
        expiry_col = expiry_col.child(
            div()
                .text_size(px(TEXT_SMALL))
                .text_color(hsla(t.text_muted))
                .child(expiry_text),
        );

        let message = self.effective_message(cx);
        let notify = self.draft.notify.summary();
        let (wm, wn) = (this.clone(), this.clone());
        v_flex()
            .gap(px(C_GAP))
            .child(Self::row("Condition", condition, &t))
            .children(extra_rows)
            .when(per_bar, |d| d.child(Self::row("Interval", interval, &t)))
            .child(div().my(px(S2)).h(px(1.0)).w_full().bg(hsla(t.border)))
            .child(Self::row("Trigger", trigger, &t))
            .child(Self::row("Expiration", expiry_col, &t))
            .child(Self::summary(
                "al-msg",
                "Message",
                message,
                &t,
                move |w, cx| {
                    wm.update(cx, |p, cx| p.set_tab(Tab::Message, w, cx)).ok();
                },
            ))
            .child(Self::summary(
                "al-notify",
                "Notifications",
                notify,
                &t,
                move |w, cx| {
                    wn.update(cx, |p, cx| p.set_tab(Tab::Notifications, w, cx))
                        .ok();
                },
            ))
            .children(self.error.clone().map(|e| {
                div()
                    .pl(px(AL_W_LABEL))
                    .text_size(px(TEXT_SMALL))
                    .text_color(hsla(t.danger))
                    .child(e)
            }))
            .into_any_element()
    }

    fn message_tab(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let t = cx.ui().clone();
        let label = |s: &'static str| div().text_color(hsla(t.text_muted)).pb(px(S1)).child(s);
        let area = self.message.clone();
        let w = cx.entity().downgrade();
        let placeholders = ToolbarButton::text("al-placeholder", "Add placeholder")
            .outlined(true)
            .caret()
            .menu(move |mut menu, _, _| {
                menu = menu.id("al-placeholders").section("General");
                for (i, (name, text)) in PLACEHOLDERS.iter().enumerate() {
                    if i == 5 {
                        menu = menu.section("Symbol data");
                    }
                    let (area, w) = (area.clone(), w.clone());
                    menu = menu.row(MenuRow::new(("al-ph", i), *name).shortcut(*text).on_select(
                        move |window, cx| {
                            area.update(cx, |a, cx| a.insert(*text, window, cx));
                            w.update(cx, |p, cx| {
                                p.message_edited = true;
                                cx.notify();
                            })
                            .ok();
                        },
                    ));
                }
                menu
            });
        let w = cx.entity().downgrade();
        let reset = ToolbarButton::text("al-msg-reset", "Reset to default")
            .disabled(!self.message_edited)
            .on_click(move |_, window, cx| {
                w.update(cx, |p, cx| {
                    p.message_edited = false;
                    p.sync_message(window, cx);
                    cx.notify();
                })
                .ok();
            });
        v_flex()
            .gap(px(S3))
            .child(
                v_flex()
                    .child(label("Alert name"))
                    .child(Field::new(&self.name).width(AL_W_LABEL + AL_W_CONTROL + 4.0)),
            )
            .child(
                v_flex().child(label("Message")).child(
                    Textarea::new(&self.message)
                        .h(px(C_H_TEXTAREA))
                        .rounded(px(C_R_CONTROL)),
                ),
            )
            .child(
                h_flex()
                    .gap(px(S2))
                    .items_center()
                    .child(placeholders)
                    .child(reset),
            )
            .into_any_element()
    }

    fn notifications_tab(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let t = cx.ui().clone();
        let n = self.draft.notify;
        let w = cx.entity().downgrade();
        let item = |id: &'static str,
                    label: &'static str,
                    desc: &'static str,
                    on: bool,
                    set: fn(&mut tt_session::alerts::Notify, bool)| {
            let w = w.clone();
            v_flex()
                .gap(px(S1))
                .child(CheckBox::new(id, label, on).on_change(move |v, _, cx| {
                    w.update(cx, |p, cx| {
                        set(&mut p.draft.notify, v);
                        cx.notify();
                    })
                    .ok();
                }))
                .child(
                    div()
                        .pl(px(C_CHECKBOX + S2))
                        .text_size(px(TEXT_SMALL))
                        .text_color(hsla(t.text_muted))
                        .child(desc),
                )
        };
        v_flex()
            .gap(px(S4))
            .child(item(
                "al-n-popup",
                "Show popup",
                "Opens a dialog with the message in the app.",
                n.popup,
                |n, v| n.popup = v,
            ))
            .child(item(
                "al-n-toast",
                "Show toast notification",
                "Displays a notification in the window corner.",
                n.toast,
                |n, v| n.toast = v,
            ))
            .child(item(
                "al-n-sound",
                "Play sound",
                "Plays an audio cue when your alert triggers.",
                n.sound,
                |n, v| n.sound = v,
            ))
            .into_any_element()
    }
}

impl Render for AlertDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let w = cx.entity().downgrade();
        let nav = NavTabs::horizontal()
            .item(Tab::Settings.id(), "Settings", None)
            .item(Tab::Message.id(), "Message", None)
            .item(Tab::Notifications.id(), "Notifications", None)
            .selected(self.tab.id())
            .on_change(move |id, window, cx| {
                let tab = match id {
                    "message" => Tab::Message,
                    "notifications" => Tab::Notifications,
                    _ => Tab::Settings,
                };
                w.update(cx, |p, cx| p.set_tab(tab, window, cx)).ok();
            });
        let body = match self.tab {
            Tab::Settings => self.settings_tab(cx),
            Tab::Message => self.message_tab(cx),
            Tab::Notifications => self.notifications_tab(cx),
        };
        let _ = self.is_new;
        DialogFrame::new(DialogSize::Sm)
            .id("alert-dialog")
            .comfortable()
            .min_body(300.0)
            .nav(nav)
            .body(body)
    }
}

// ---- popup ------------------------------------------------------------------

/// The "Show popup" notification: the alert's message until dismissed.
pub struct AlertPopup {
    entry: LogEntry,
}

impl Render for AlertPopup {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.ui().clone();
        let e = &self.entry;
        v_flex()
            .px(px(C_PAD_X))
            .py(px(C_PAD_Y))
            .gap(px(S2))
            .child(
                h_flex()
                    .gap(px(S2))
                    .items_center()
                    .child(glyph(Glyph::ALARM, ICON_MD))
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(e.title.clone()),
                    ),
            )
            .child(e.message.clone())
            .child(
                div()
                    .text_size(px(TEXT_SMALL))
                    .text_color(hsla(t.text_muted))
                    .child(format!(
                        "{} · {} at {}",
                        e.symbol,
                        tt_chart::scale::format_price(e.price, e.precision),
                        format_local_long(e.time_ms)
                    )),
            )
    }
}

/// Opens the popup of a triggered alert.
pub fn open_popup(entry: LogEntry, window: &mut Window, cx: &mut App) {
    let title = format!("Alert on {}", entry.symbol);
    let panel = cx.new(|_| AlertPopup { entry });
    tt_ui::open_dialog(
        window,
        cx,
        DialogOpts::new(title, DialogSize::Sm)
            .comfortable()
            .width(AL_W_DIALOG - 80.0)
            .footer(DialogFooter::new().comfortable().ok("OK")),
        panel,
    );
}
