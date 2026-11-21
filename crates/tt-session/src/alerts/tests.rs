use super::*;

const T0: i64 = 1_700_000_040_000; // a minute boundary
const M: i64 = 60_000;

fn alert(id: u64, op: Operator, v: f64, trigger: Trigger) -> Alert {
    Alert {
        id,
        symbol: "BTCUSDT".into(),
        interval: Timeframe::M1,
        condition: Condition::value(op, v),
        trigger,
        message: "{{ticker}} {{close}}".into(),
        ..Alert::default()
    }
}

fn eval(alerts: Vec<Alert>) -> Evaluator {
    let mut e = Evaluator::default();
    e.set_alerts(alerts);
    e
}

/// Feeds `(offset ms, price)` ticks; returns the fired events.
fn feed(e: &mut Evaluator, ticks: &[(i64, f64)]) -> Vec<Fired> {
    let mut out = Vec::new();
    for &(dt, p) in ticks {
        e.on_tick("BTCUSDT", p, T0 + dt, &mut out);
    }
    out
}

#[test]
fn crossing_needs_a_previous_price_and_catches_gaps() {
    let mut e = eval(vec![alert(
        1,
        Operator::Crossing,
        100.0,
        Trigger::OncePerBar,
    )]);
    // First tick above: no previous price, no crossing.
    assert!(feed(&mut e, &[(0, 101.0)]).is_empty());
    // A gap from 101 down to 90 crosses 100.
    let out = feed(&mut e, &[(1000, 90.0)]);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].price, 90.0);
    assert_eq!(out[0].message, "BTCUSDT 90.00");
}

#[test]
fn crossing_up_and_down_are_directional() {
    let mut up = eval(vec![alert(
        1,
        Operator::CrossingUp,
        100.0,
        Trigger::OncePerMinute,
    )]);
    let mut down = eval(vec![alert(
        1,
        Operator::CrossingDown,
        100.0,
        Trigger::OncePerMinute,
    )]);
    let ticks = [(0, 99.0), (1000, 101.0)];
    assert_eq!(feed(&mut up, &ticks).len(), 1);
    assert!(feed(&mut down, &ticks).is_empty());
    // Touching the level then leaving it upward counts once (Pine's
    // crossover: prev <= v < cur).
    let mut up = eval(vec![alert(
        1,
        Operator::CrossingUp,
        100.0,
        Trigger::OncePerBar,
    )]);
    let out = feed(&mut up, &[(0, 99.0), (100, 100.0), (200, 100.5)]);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].price, 100.5);
}

#[test]
fn only_once_fires_once_and_reports_last() {
    let mut e = eval(vec![alert(
        1,
        Operator::GreaterThan,
        100.0,
        Trigger::OnlyOnce,
    )]);
    let out = feed(&mut e, &[(0, 101.0), (10, 102.0), (2 * M, 103.0)]);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].kind, FiredKind::Triggered { last: true });
    assert!(e.symbols().is_empty(), "a done alert needs no feed");
}

#[test]
fn once_per_bar_fires_once_in_each_bar() {
    let mut e = eval(vec![alert(
        1,
        Operator::GreaterThan,
        100.0,
        Trigger::OncePerBar,
    )]);
    let out = feed(
        &mut e,
        &[
            (0, 101.0),
            (1000, 102.0),
            (M - 1, 103.0),
            (M, 104.0),
            (M + 5, 105.0),
        ],
    );
    assert_eq!(out.len(), 2);
    assert_eq!(out[0].time_ms, T0);
    assert_eq!(out[1].time_ms, T0 + M);
    assert_eq!(out[1].kind, FiredKind::Triggered { last: false });
}

#[test]
fn once_per_minute_throttles_on_a_longer_interval() {
    let mut a = alert(1, Operator::LessThan, 100.0, Trigger::OncePerMinute);
    a.interval = Timeframe::H1;
    let mut e = eval(vec![a]);
    let out = feed(
        &mut e,
        &[(0, 99.0), (30_000, 98.0), (M, 97.0), (M + 1, 96.0)],
    );
    assert_eq!(out.len(), 2);
    assert_eq!(out[1].time_ms, T0 + M);
}

#[test]
fn once_per_bar_close_uses_closes_only() {
    let mut e = eval(vec![alert(
        1,
        Operator::CrossingUp,
        100.0,
        Trigger::OncePerBarClose,
    )]);
    // Bar 1 closes at 99, bar 2 spikes to 105 but closes at 99.5: no close
    // crossed. Bar 3 closes at 101: fires when bar 4 opens.
    let out = feed(
        &mut e,
        &[
            (0, 99.0),
            (M, 105.0),
            (M + 10, 99.5),
            (2 * M, 101.0),
            (3 * M, 101.0),
        ],
    );
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].price, 101.0);
    assert_eq!(out[0].time_ms, T0 + 3 * M);
    assert_eq!(out[0].message, "BTCUSDT 101.00");
}

#[test]
fn the_clock_closes_bars_without_trades() {
    let mut e = eval(vec![alert(
        1,
        Operator::GreaterThan,
        100.0,
        Trigger::OncePerBarClose,
    )]);
    let mut out = feed(&mut e, &[(0, 101.0)]);
    assert!(out.is_empty());
    e.on_timer(T0 + M + 500, &mut out);
    assert_eq!(out.len(), 1, "the bar closed at 101 > 100");
    e.on_timer(T0 + 2 * M + 500, &mut out);
    assert_eq!(out.len(), 2, "the next bar (no trades) closed at 101 too");
}

#[test]
fn channels() {
    let mut a = alert(1, Operator::EnteringChannel, 110.0, Trigger::OncePerBar);
    a.condition.lower = 100.0;
    let mut b = a.clone();
    b.id = 2;
    b.condition.op = Operator::ExitingChannel;
    let mut c = a.clone();
    c.id = 3;
    c.condition.op = Operator::OutsideChannel;
    let mut e = eval(vec![a, b, c]);
    let out = feed(&mut e, &[(0, 95.0), (100, 105.0)]);
    let ids: Vec<u64> = out.iter().map(|f| f.alert_id).collect();
    assert_eq!(ids, vec![3, 1], "outside at 95, then entering");
    let out = feed(&mut e, &[(M, 115.0)]);
    let ids: Vec<u64> = out.iter().map(|f| f.alert_id).collect();
    assert_eq!(ids, vec![2, 3]);
}

#[test]
fn moving_up_compares_with_the_close_n_bars_ago() {
    let mut a = alert(1, Operator::MovingUp, 5.0, Trigger::OncePerBar);
    a.condition.bars = 2;
    let mut p = alert(2, Operator::MovingDownPercent, 10.0, Trigger::OncePerBar);
    p.condition.bars = 1;
    let mut e = eval(vec![a, p]);
    // Closes: 100 (bar 0), 102 (bar 1); bar 2 trades at 104 (+4 vs 100: no),
    // then 105.5 (+5.5: fires).
    let out = feed(
        &mut e,
        &[(0, 100.0), (M, 102.0), (2 * M, 104.0), (2 * M + 1, 105.5)],
    );
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].alert_id, 1);
    // Bar 3: 94 is -10.9 % vs bar 2's close (105.5).
    let out = feed(&mut e, &[(3 * M, 94.0)]);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].alert_id, 2);
}

#[test]
fn expiration_stops_the_alert() {
    let mut a = alert(1, Operator::GreaterThan, 100.0, Trigger::OncePerBar);
    a.expiration = Some(T0 + 10_000);
    let mut e = eval(vec![a]);
    let mut out = feed(&mut e, &[(0, 99.0)]);
    e.on_timer(T0 + 10_000, &mut out);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].kind, FiredKind::Expired);
    // Done: nothing fires after the expiration.
    let out = feed(&mut e, &[(20_000, 150.0)]);
    assert!(out.is_empty());
}

#[test]
fn trend_lines_follow_time() {
    let line = LineRef::through(
        7,
        LineKind::TrendLine,
        (T0 + 10 * M, 110.0),
        (T0, 100.0),
        false,
        true,
    );
    assert_eq!((line.t1, line.p1), (T0, 100.0), "points sorted by time");
    assert!(!line.extend_left && line.extend_right);
    assert_eq!(line.value_at(T0 + 5 * M), Some(105.0));
    assert_eq!(line.value_at(T0 + 20 * M), Some(120.0), "extended right");
    assert_eq!(line.value_at(T0 - 1), None, "not extended left");
    let mut a = alert(1, Operator::Crossing, 0.0, Trigger::OncePerBar);
    a.condition.target = Target::Line(line);
    let mut e = eval(vec![a]);
    // 104 at t0+5m is below the line (105); 106 a second later is above.
    let out = feed(&mut e, &[(5 * M, 104.0), (5 * M + 1000, 106.0)]);
    assert_eq!(out.len(), 1);
    // Horizontal line: constant.
    assert_eq!(LineRef::horizontal(1, 50.0).value_at(i64::MIN), Some(50.0));
}

#[test]
fn editing_text_keeps_the_run_state() {
    let a = alert(1, Operator::Crossing, 100.0, Trigger::OnlyOnce);
    let mut e = eval(vec![a.clone()]);
    assert_eq!(feed(&mut e, &[(0, 99.0), (10, 101.0)]).len(), 1);
    // The UI resends it (renamed) before it learned it fired: no re-fire.
    let mut renamed = a.clone();
    renamed.name = "renamed".into();
    e.set_alerts(vec![renamed]);
    assert!(feed(&mut e, &[(20, 99.0), (30, 101.0)]).is_empty());
    // A changed rule starts over.
    let mut moved = a;
    moved.condition = Condition::value(Operator::Crossing, 98.0);
    e.set_alerts(vec![moved]);
    assert_eq!(feed(&mut e, &[(40, 97.0), (50, 99.0)]).len(), 1);
    // Stopped alerts are not evaluated.
    let mut stopped = alert(2, Operator::GreaterThan, 0.0, Trigger::OncePerBar);
    stopped.status = AlertStatus::Stopped;
    let e = eval(vec![stopped]);
    assert!(e.is_empty());
}

#[test]
fn placeholders() {
    let v = Placeholders {
        ticker: "BTCUSDT",
        exchange: "BINANCE",
        interval: Timeframe::H4,
        time_ms: 1_566_899_760_000,
        now_ms: 1_566_899_761_000,
        open: 1.0,
        high: 2.5,
        low: 0.5,
        close: 2.0,
        volume: None,
        precision: 2,
    };
    assert_eq!(
        format_message("{{ticker}} closed at {{close}} ({{ INTERVAL }})", &v),
        "BTCUSDT closed at 2.00 (240)"
    );
    assert_eq!(
        format_message("{{exchange}}:{{time}} {{timenow}}", &v),
        "BINANCE:2019-08-27T09:56:00Z 2019-08-27T09:56:01Z"
    );
    assert_eq!(
        format_message("o{{open}} h{{high}} l{{low}} v{{volume}}", &v),
        "o1.00 h2.50 l0.50 vNaN"
    );
    assert_eq!(
        format_message("{{plot_0}} {{close", &v),
        "{{plot_0}} {{close",
        "unknown and unterminated placeholders stay"
    );
    assert_eq!(interval_placeholder(Timeframe::M15), "15");
    assert_eq!(interval_placeholder(Timeframe::D1), "1D");
    assert_eq!(
        interval_placeholder(Timeframe::new(TimeUnit::Second, 30)),
        "30S"
    );
}

#[test]
fn default_messages_match_tradingview() {
    let mut a = alert(1, Operator::Crossing, 57667.32, Trigger::OnlyOnce);
    assert_eq!(a.default_message(), "BTCUSDT Crossing 57,667.32");
    a.condition = Condition {
        op: Operator::EnteringChannel,
        target: Target::Value(57667.32),
        lower: 57667.33,
        bars: 1,
    };
    assert_eq!(
        a.default_message(),
        "BTCUSDT Entering Channel (Upper Bound: 57,667.33, Lower Bound: 57,667.32)"
    );
    a.interval = Timeframe::M30;
    a.condition = Condition::value(Operator::MovingUp, 1.0);
    assert_eq!(a.default_message(), "BTCUSDT, 30 Moving Up 1.00 in 1 bar");
    assert_eq!(a.title(), "Moving Up 1.00 in 1 bar");
}

#[test]
fn local_expiration_helpers() {
    let ms = parse_local_input("2026-12-21 15:41").unwrap();
    assert_eq!(format_local_input(ms), "2026-12-21 15:41");
    assert_eq!(format_local_long(ms), "December 21, 2026 at 15:41");
    assert_eq!(
        format_local_input(parse_local_input("2026-12-21").unwrap()),
        "2026-12-21 23:59"
    );
    assert!(parse_local_input("tomorrow").is_none());
    let now = parse_local_input("2026-11-21 10:00").unwrap();
    assert_eq!(ExpiryPreset::OpenEnded.resolve(now), None);
    let eod = ExpiryPreset::EndOfDay.resolve(now).unwrap();
    assert_eq!(format_local_input(eod), "2026-11-21 23:59");
    let week = ExpiryPreset::OneWeek.resolve(now).unwrap();
    assert_eq!(format_local_input(week), "2026-11-28 10:00");
    let month = ExpiryPreset::OneMonth.resolve(now).unwrap();
    assert_eq!(format_local_input(month), "2026-12-21 10:00");
}

#[test]
fn storage_round_trip_and_tolerant_loading() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(ALERTS_FILE);
    // Missing: empty.
    let f = AlertsFile::load(Some(&path));
    assert!(f.alerts.is_empty());
    assert_eq!(f.next_id, 0);

    let mut f = AlertsFile::default();
    let id = f.take_id();
    f.alerts
        .push(alert(id, Operator::Crossing, 1.5, Trigger::OncePerBar));
    for i in 0..MAX_LOG + 3 {
        f.push_log(LogEntry {
            alert_id: id,
            time_ms: i as i64,
            ..LogEntry::default()
        });
    }
    assert_eq!(f.log.len(), MAX_LOG);
    assert_eq!(f.log[0].time_ms, 3, "oldest dropped");
    std::fs::write(&path, serde_json::to_vec(&f).unwrap()).unwrap();
    let back = AlertsFile::load(Some(&path));
    assert_eq!(back.alerts, f.alerts);
    assert_eq!(back.log, f.log);
    assert_eq!(back.next_id, 2);

    // An alert from a newer build (unknown operator), a duplicate id and
    // missing fields: the good ones load, the rest is kept raw.
    let text = r#"{
        "version": 9,
        "alerts": [
            {"id": 4, "symbol": "ETHUSDT", "condition": {"op": "GreaterThan", "target": {"Value": 3000.0}}},
            {"id": 5, "condition": {"op": "Teleporting"}},
            {"id": 4, "symbol": "DUP"}
        ],
        "log": [{"alert_id": 4, "time_ms": 1}, "garbage"]
    }"#;
    std::fs::write(&path, text).unwrap();
    let f = AlertsFile::load(Some(&path));
    assert_eq!(f.alerts.len(), 1);
    assert_eq!(f.alerts[0].symbol, "ETHUSDT");
    assert_eq!(f.alerts[0].trigger, Trigger::OnlyOnce, "defaults fill gaps");
    assert_eq!(f.unknown.len(), 2);
    assert_eq!(f.log.len(), 1);
    assert_eq!(f.next_id, 5);
    let written = serde_json::to_value(&f).unwrap();
    assert_eq!(
        written["alerts"].as_array().unwrap().len(),
        3,
        "unknown kept"
    );

    // Corrupt: moved aside, empty.
    std::fs::write(&path, "{ not json").unwrap();
    let f = AlertsFile::load(Some(&path));
    assert!(f.alerts.is_empty());
    assert!(!path.exists());
    let backups = std::fs::read_dir(dir.path()).unwrap().count();
    assert_eq!(backups, 1);
}
