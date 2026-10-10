use sonic_rs::{JsonContainerTrait, JsonValueTrait, Object, Value};

use crate::value::FnState;
use f_common::fun;

const STATE_CODE: &str = "order_book_depth";

const STATE_VERSION: i64 = 1;

pub const PRICE_SCALE: usize = 8;
const PRICE_UNIT: f64 = 1e8;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Level {
    pub px: i64,
    pub qty: f64,
}

pub struct Frame<'a> {
        pub snapshot: bool,
        pub first: u64,
    pub last: u64,
    pub seq: u64,
    pub cts: i64,
    pub instant: i64,
    pub bids: &'a Value,
    pub asks: &'a Value,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Applied {
        Snapshot,
        Delta,
        Duplicate,
        NotReady,
        Gap,
        Crossed,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct OrderBookDepth {
        pub u: u64,
    pub seq: u64,
    pub cts: i64,
    pub valid: bool,
                pub instant: i64,
        pub bids: Vec<Level>,
        pub asks: Vec<Level>,
}

#[derive(Clone, Copy)]
enum Side {
    Bid,
    Ask,
}

impl OrderBookDepth {
        pub fn apply(&mut self, frame: &Frame, depth: usize) -> Applied {
        if frame.snapshot {
            self.bids.clear();
            self.asks.clear();
            apply_side(&mut self.bids, frame.bids, Side::Bid);
            apply_side(&mut self.asks, frame.asks, Side::Ask);
            self.valid = true;
            return self.finish(frame, depth, Applied::Snapshot);
        }
        if !self.valid {
            return Applied::NotReady;
        }
        if frame.last <= self.u {
            return Applied::Duplicate;
        }
        if frame.first > self.u + 1 {
            self.invalidate(frame);
            return Applied::Gap;
        }
        apply_side(&mut self.bids, frame.bids, Side::Bid);
        apply_side(&mut self.asks, frame.asks, Side::Ask);
        self.finish(frame, depth, Applied::Delta)
    }

    fn finish(&mut self, frame: &Frame, depth: usize, applied: Applied) -> Applied {
        self.bids.truncate(depth);
        self.asks.truncate(depth);
        self.u = frame.last;
        self.seq = frame.seq;
        self.cts = frame.cts;
        match (self.bids.first(), self.asks.first()) {
            (Some(bid), Some(ask)) if bid.px >= ask.px => {
                self.invalidate(frame);
                Applied::Crossed
            }
            _ => {
                self.instant = frame.instant;
                applied
            }
        }
    }

        fn invalidate(&mut self, frame: &Frame) {
        self.valid = false;
        self.u = frame.last;
        self.seq = frame.seq;
        self.cts = frame.cts;
        self.bids.clear();
        self.asks.clear();
    }

        pub fn top_levels(&self, n: usize) -> (Value, Value) {
        (levels_value(&self.bids, n), levels_value(&self.asks, n))
    }

    pub fn best_bid(&self) -> f64 {
        self.bids.first().map_or(0.0, |l| px_f64(l.px))
    }

    pub fn best_ask(&self) -> f64 {
        self.asks.first().map_or(0.0, |l| px_f64(l.px))
    }
}

fn apply_side(side: &mut Vec<Level>, levels: &Value, s: Side) {
    let Some(levels) = levels.as_array() else {
        return;
    };
    for level in levels.iter() {
        let Some(fields) = level.as_array() else {
            continue;
        };
        let Some(px) = fields.get(0).and_then(parse_px) else {
            continue;
        };
        let qty = fields.get(1).map(fun::to_f64).unwrap_or(0.0);
        set_level(side, px, qty, s);
    }
}

fn set_level(side: &mut Vec<Level>, px: i64, qty: f64, s: Side) {
    let pos = side.binary_search_by(|l| match s {
        Side::Bid => px.cmp(&l.px),
        Side::Ask => l.px.cmp(&px),
    });
    match pos {
        Ok(i) if qty > 0.0 => side[i].qty = qty,
        Ok(i) => {
            side.remove(i);
        }
        Err(i) if qty > 0.0 => side.insert(i, Level { px, qty }),
        Err(_) => {}
    }
}

pub fn parse_px(v: &Value) -> Option<i64> {
    if let Some(s) = v.as_str() {
        return parse_decimal(s.trim());
    }
    let x = fun::to_f64(v);
    (x > 0.0).then(|| (x * PRICE_UNIT).round() as i64)
}

fn parse_decimal(s: &str) -> Option<i64> {
    let (int, frac) = s.split_once('.').unwrap_or((s, ""));
    let digits = |p: &str| p.bytes().all(|b| b.is_ascii_digit());
    if (int.is_empty() && frac.is_empty()) || !digits(int) || !digits(frac) {
        return None;
    }
    let mut units: i64 = if int.is_empty() { 0 } else { int.parse().ok()? };
    let frac = frac.as_bytes();
    for i in 0..PRICE_SCALE {
        let d = frac.get(i).map_or(0, |b| i64::from(b - b'0'));
        units = units.checked_mul(10)?.checked_add(d)?;
    }
    if frac.get(PRICE_SCALE).is_some_and(|b| *b >= b'5') {
        units = units.checked_add(1)?;
    }
    (units > 0).then_some(units)
}

fn px_f64(px: i64) -> f64 {
    px as f64 / PRICE_UNIT
}

fn levels_value(side: &[Level], n: usize) -> Value {
    let levels: Vec<Value> = side
        .iter()
        .take(n)
        .map(|l| Value::from(vec![fun::json_f64(px_f64(l.px)), fun::json_f64(l.qty)]))
        .collect();
    Value::from(levels)
}

fn stored_levels(side: &[Level]) -> Value {
    let levels: Vec<Value> = side
        .iter()
        .map(|l| Value::from(vec![Value::from(l.px), fun::json_f64(l.qty)]))
        .collect();
    Value::from(levels)
}

fn levels_of(obj: &Object, key: &str) -> Vec<Level> {
    let Some(levels) = obj.get(&key).and_then(|v| v.as_array()) else {
        return Vec::new();
    };
    levels
        .iter()
        .filter_map(|level| {
            let fields = level.as_array()?;
            Some(Level {
                px: fields.get(0)?.as_i64()?,
                qty: fields.get(1).map(fun::to_f64)?,
            })
        })
        .collect()
}

fn u64_of(obj: &Object, key: &str) -> u64 {
    obj.get(&key).and_then(|v| v.as_u64()).unwrap_or(0)
}

impl FnState for OrderBookDepth {
    const CODE: &'static str = STATE_CODE;

    fn from_value(values: &[Value]) -> Self {
        let Some(obj) = values.first().and_then(|v| v.as_object()) else {
            return Self::default();
        };
        if fun::f_i64(obj, "state_version") != STATE_VERSION {
            return Self::default();
        }
        Self {
            u: u64_of(obj, "u"),
            seq: u64_of(obj, "seq"),
            cts: fun::f_i64(obj, "cts"),
            valid: fun::f_bool(obj, "valid"),
            instant: fun::f_i64(obj, "instant"),
            bids: levels_of(obj, "b"),
            asks: levels_of(obj, "a"),
        }
    }

    fn into_value(self) -> Value {
        let mut o = Object::with_capacity(8);
        o.insert("u", self.u);
        o.insert("seq", self.seq);
        o.insert("cts", self.cts);
        o.insert("valid", self.valid);
        o.insert("instant", self.instant);
        o.insert("b", stored_levels(&self.bids));
        o.insert("a", stored_levels(&self.asks));
        o.insert("state_version", STATE_VERSION);
        o.into_value()
    }

            fn reloaded(&self) -> Self {
        let mut state = self.clone();
        state.cts = state.cts as f64 as i64;
        state.instant = state.instant as f64 as i64;
        for level in state.bids.iter_mut().chain(state.asks.iter_mut()) {
            if !level.qty.is_finite() {
                level.qty = 0.0;
            }
        }
        state
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use f_common::order_book;
    use sonic_rs::json;

    fn frame<'a>(snapshot: bool, first: u64, last: u64, bids: &'a Value, asks: &'a Value) -> Frame<'a> {
        Frame { snapshot, first, last, seq: last * 10, cts: last as i64, instant: last as i64 * 1_000, bids, asks }
    }

    fn delta<'a>(u: u64, bids: &'a Value, asks: &'a Value) -> Frame<'a> {
        frame(false, u, u, bids, asks)
    }

    fn snapshot(book: &mut OrderBookDepth, u: u64) -> Applied {
        let bids = json!([["100.0", "10"], ["99.9", "8"], ["99.8", "5"]]);
        let asks = json!([["100.1", "3"], ["100.2", "4"], ["100.3", "6"]]);
        book.apply(&frame(true, u, u, &bids, &asks), 50)
    }

    fn px(s: &str) -> i64 {
        parse_decimal(s).unwrap()
    }

    #[test]
    fn parse_decimal_is_exact() {
        assert_eq!(parse_decimal("60520.3"), Some(6_052_030_000_000));
        assert_eq!(parse_decimal("0.00001234"), Some(1_234));
        assert_eq!(parse_decimal("85"), Some(8_500_000_000));
        assert_eq!(parse_decimal("1.000000005"), Some(100_000_001), "9-й знак округляется");
        assert_eq!(parse_decimal("0"), None);
        assert_eq!(parse_decimal("abc"), None);
        assert_eq!(parse_px(&json!(60520.3)), Some(6_052_030_000_000));
    }

    #[test]
    fn snapshot_replaces_book_and_keeps_order() {
        let mut book = OrderBookDepth::default();
        let bids = json!([["99.8", "5"], ["100.0", "10"], ["99.9", "8"]]);
        let asks = json!([["100.3", "6"], ["100.1", "3"]]);
        assert_eq!(book.apply(&frame(true, 7, 7, &bids, &asks), 50), Applied::Snapshot);
        assert!(book.valid);
        assert_eq!(book.u, 7);
        let bid_px: Vec<i64> = book.bids.iter().map(|l| l.px).collect();
        let ask_px: Vec<i64> = book.asks.iter().map(|l| l.px).collect();
        assert_eq!(bid_px, vec![px("100.0"), px("99.9"), px("99.8")]);
        assert_eq!(ask_px, vec![px("100.1"), px("100.3")]);
    }

    #[test]
    fn delta_inserts_updates_and_deletes_levels() {
        let mut book = OrderBookDepth::default();
        snapshot(&mut book, 7);
        let bids = json!([["100.05", "2"], ["99.9", "0"]]);
        let asks = json!([["100.1", "2.9"]]);
        assert_eq!(book.apply(&delta(8, &bids, &asks), 50), Applied::Delta);
        assert_eq!(book.instant, 8_000, "delta по цепочке двигает instant");
        let bids: Vec<(i64, f64)> = book.bids.iter().map(|l| (l.px, l.qty)).collect();
        assert_eq!(bids, vec![(px("100.05"), 2.0), (px("100.0"), 10.0), (px("99.8"), 5.0)]);
        assert_eq!(book.asks[0], Level { px: px("100.1"), qty: 2.9 });
        assert_eq!(book.u, 8);
    }

    #[test]
    fn duplicate_is_skipped_and_gap_invalidates() {
        let mut book = OrderBookDepth::default();
        snapshot(&mut book, 7);
        let empty = json!([]);
        let asks = json!([["100.1", "1"]]);
        assert_eq!(book.apply(&delta(7, &empty, &asks), 50), Applied::Duplicate);
        assert_eq!(book.asks[0].qty, 3.0, "повтор не меняет стакан");

        assert_eq!(book.instant, 7_000, "повтор не двигает instant");

        assert_eq!(book.apply(&delta(9, &empty, &asks), 50), Applied::Gap);
        assert!(!book.valid);
        assert_eq!(book.instant, 7_000, "после разрыва instant остаётся от последнего валидного кадра");
        assert!(book.bids.is_empty() && book.asks.is_empty());

        assert_eq!(book.apply(&delta(10, &empty, &asks), 50), Applied::NotReady);
        assert_eq!(book.instant, 7_000, "кадры без стакана instant не двигают");
        assert_eq!(snapshot(&mut book, 20), Applied::Snapshot);
        assert!(book.valid);
        assert_eq!(book.instant, 20_000);
    }

    #[test]
    fn delta_before_first_snapshot_is_not_ready() {
        let mut book = OrderBookDepth::default();
        let bids = json!([["100.0", "1"]]);
        let empty = json!([]);
        assert_eq!(book.apply(&delta(5, &bids, &empty), 50), Applied::NotReady);
        assert!(book.bids.is_empty());
        assert_eq!(book.instant, 0, "валидного стакана ещё не было: watchdog видит «нет данных»");
    }

    #[test]
    fn merged_range_continues_the_chain() {
        let mut book = OrderBookDepth::default();
        snapshot(&mut book, 100);
        let asks = json!([["100.1", "1"]]);
        let empty = json!([]);
        assert_eq!(book.apply(&frame(false, 101, 180, &empty, &asks), 50), Applied::Delta);
        assert_eq!(book.u, 180);
        assert_eq!(book.apply(&frame(false, 150, 180, &empty, &asks), 50), Applied::Duplicate);
        assert_eq!(
            book.apply(&frame(false, 170, 200, &empty, &asks), 50),
            Applied::Delta,
            "часть версий уже в стакане: уровни абсолютные, кадр применяется"
        );
        assert_eq!(book.u, 200);
        assert_eq!(book.apply(&frame(false, 210, 230, &empty, &asks), 50), Applied::Gap, "пропуск 201..209");
    }

    #[test]
    fn sides_are_trimmed_to_depth() {
        let mut book = OrderBookDepth::default();
        let bids = json!([["100.0", "1"], ["99.9", "1"], ["99.8", "1"]]);
        let asks = json!([["100.1", "1"], ["100.2", "1"], ["100.3", "1"]]);
        book.apply(&frame(true, 3, 3, &bids, &asks), 2);
        assert_eq!(book.bids.len(), 2);
        assert_eq!(book.asks.last().unwrap().px, px("100.2"), "отрезан дальний аск");
    }

    #[test]
    fn crossed_book_invalidates() {
        let mut book = OrderBookDepth::default();
        snapshot(&mut book, 7);
        let bids = json!([["100.1", "1"]]);
        let empty = json!([]);
        assert_eq!(book.apply(&delta(8, &bids, &empty), 50), Applied::Crossed);
        assert!(!book.valid);
        assert_eq!(book.instant, 7_000, "instant остаётся от snapshot");
    }

    #[test]
    fn state_round_trips() {
        let mut book = OrderBookDepth::default();
        snapshot(&mut book, 7);
        let restored = OrderBookDepth::from_value(&[book.clone().into_value()]);
        assert_eq!(restored, book);
    }

    #[test]
    fn missing_or_old_state_is_an_empty_book() {
        assert_eq!(OrderBookDepth::from_value(&[]), OrderBookDepth::default());
        let old = json!({"u": 5, "valid": true, "b": [[1, 1.0]], "a": []});
        assert_eq!(OrderBookDepth::from_value(&[old]), OrderBookDepth::default());
    }

    #[test]
    fn top_levels_feed_book_imbalance() {
        let mut book = OrderBookDepth::default();
        snapshot(&mut book, 7);
        let (bids, asks) = book.top_levels(2);
        assert_eq!(order_book::price_at(&bids, 0), 100.0);
        assert_eq!(fun::vol_at(&asks, 1), 4.0);
        let obi = order_book::imbalance_arr(&bids, &asks);
        assert!((obi - (18.0 - 7.0) / 25.0).abs() < 1e-12, "OBI по двум лучшим уровням стакана");
        assert_eq!(book.best_bid(), 100.0);
        assert_eq!(book.best_ask(), 100.1);
    }
}
