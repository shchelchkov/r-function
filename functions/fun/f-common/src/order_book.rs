use sonic_rs::{JsonContainerTrait, JsonValueTrait, Object, Value};

use crate::fun;

pub fn data_bas<'a>(obj: &'a Object) -> Option<(&'a Value, &'a Value, &'a str)> {
    let data = obj.get(&"data")?.as_object()?;
    let bids = data.get(&"b")?;
    let asks = data.get(&"a")?;
    let symbol = data.get(&"s")?.as_str()?;
    Some((bids, asks, symbol))
}

pub fn data_mexc<'a>(obj: &'a Object) -> Option<(&'a Value, &'a Value, &'a str)> {
    let data = obj.get(&"data")?.as_object()?;
    let bids = data.get(&"bids")?;
    let asks = data.get(&"asks")?;
    let symbol = obj.get(&"symbol")?.as_str()?;
    Some((bids, asks, symbol))
}

pub fn data_bas_2<'a>(
    obj: &'a Object,
) -> Option<(&'a Value, &'a Value, &'a str, Value, Value, Value)> {
    let (bids, asks, symbol) = data_bas(obj)?;

    let bid = bids
        .as_array()
        .and_then(|a| a.first())
        .and_then(|l| l.as_array())
        .and_then(|l| l.first())
        .cloned()
        .unwrap_or_default();
    let ask = asks
        .as_array()
        .and_then(|a| a.first())
        .and_then(|l| l.as_array())
        .and_then(|l| l.first())
        .cloned()
        .unwrap_or_default();

    let best_price = best_price(&bid, &ask);

    Some((bids, asks, symbol, bid, ask, best_price))
}

pub fn data_bas_mexc<'a>(
    obj: &'a Object,
) -> Option<(&'a Value, &'a Value, &'a str, Value, Value, Value)> {
    let (bids, asks, symbol) = data_mexc(obj)?;

    let bid = bids
        .as_array()
        .and_then(|a| a.first())
        .and_then(|l| l.as_array())
        .and_then(|l| l.first())
        .cloned()
        .unwrap_or_default();
    let ask = asks
        .as_array()
        .and_then(|a| a.first())
        .and_then(|l| l.as_array())
        .and_then(|l| l.first())
        .cloned()
        .unwrap_or_default();

    let best_price = best_price(&bid, &ask);

    Some((bids, asks, symbol, bid, ask, best_price))
}

pub fn best_price(bid: &Value, ask: &Value) -> Value {
    let bid_price = fun::to_f64(bid);

    if bid_price == 0.0 {
        return fun::json_f64(fun::to_f64(ask));
    }

    fun::json_f64(bid_price)
}

pub fn price_at(arr: &Value, i: usize) -> f64 {
    let Some(level) = arr.as_array().and_then(|a| a.get(i)) else {
        return 0.0;
    };
    let Some(level) = level.as_array() else {
        return 0.0;
    };
    level.get(0).map(fun::to_f64).unwrap_or(0.0)
}

pub fn z_score_update(x: f64, prev_mu: f64, prev_var: f64, alpha: f64) -> (f64, f64) {
    let mu = prev_mu + alpha * (x - prev_mu);
    let diff = x - mu;
    let var = (1.0 - alpha) * prev_var + alpha * diff * diff;
    (mu, var)
}

pub fn fragility(obi: f64, total_depth: f64) -> f64 {
    const EPS: f64 = 1e-9;
    obi.abs() / (total_depth + EPS)
}

pub fn total_depth_arr(bids: &Value, asks: &Value) -> f64 {
    let mut sum = 0.0;
    if let Some(b) = bids.as_array() {
        for i in 0..b.len() {
            sum += fun::vol_at(bids, i);
        }
    }
    if let Some(a) = asks.as_array() {
        for i in 0..a.len() {
            sum += fun::vol_at(asks, i);
        }
    }
    sum
}

pub fn ema_obi(x: f64, prev: f64, alpha: f64) -> f64 {
    alpha * x + (1.0 - alpha) * prev
}

pub fn imbalance_arr(bids: &Value, asks: &Value) -> f64 {
    let bids_arr = bids.as_array();
    let asks_arr = asks.as_array();
    let (Some(bids_arr), Some(asks_arr)) = (bids_arr, asks_arr) else {
        return 0.0;
    };
    match (bids_arr.is_empty(), asks_arr.is_empty()) {
        (false, true) => return 1.0,  
        (true, false) => return -1.0, 
        (true, true) => return 0.0,   
        (false, false) => {}
    }
    let n = bids_arr.len().min(asks_arr.len());

    let mut sum = 0.0;
    let mut diff = 0.0;
    for i in 0..n {
        let bv = fun::vol_at(bids, i);
        let av = fun::vol_at(asks, i);
        sum += bv + av;
        diff += bv - av;
    }
    if sum == 0.0 { 0.0 } else { diff / sum }
}

pub fn price_or_zero(v: Value) -> Value {
    if v.is_null() {
        Value::new_f64(0.0).unwrap()
    } else {
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sonic_rs::json;

    #[test]
    fn imbalance_symmetric_is_zero() {
        let bids = json!([["10.0", "1.0"], ["9.0", "2.0"]]);
        let asks = json!([["11.0", "1.0"], ["12.0", "2.0"]]);
        assert!((imbalance_arr(&bids, &asks)).abs() < 1e-12);
    }

    #[test]
    fn imbalance_bid_heavy_positive() {
        let bids = json!([["10.0", "3.0"]]);
        let asks = json!([["11.0", "1.0"]]);
        assert!((imbalance_arr(&bids, &asks) - 0.5).abs() < 1e-12);
    }

    #[test]
    fn imbalance_empty_is_zero() {
        let bids = json!([]);
        let asks = json!([]);
        assert_eq!(imbalance_arr(&bids, &asks), 0.0);
    }

    #[test]
    fn imbalance_empty_asks_is_plus_one() {
        let bids = json!([["79.370", "1144.8"], ["79.300", "996.6"]]);
        let asks = json!([]);
        assert_eq!(imbalance_arr(&bids, &asks), 1.0);
    }

    #[test]
    fn imbalance_empty_bids_is_minus_one() {
        let bids = json!([]);
        let asks = json!([["79.400", "500.0"]]);
        assert_eq!(imbalance_arr(&bids, &asks), -1.0);
    }

    #[test]
    fn ema_obi_first_step_from_zero() {
        assert!((ema_obi(1.0, 0.0, 0.2) - 0.2).abs() < 1e-12);
    }

    #[test]
    fn ema_obi_converges_to_x_when_alpha_one() {
        assert_eq!(ema_obi(0.7, 0.123, 1.0), 0.7);
    }

    #[test]
    fn ema_obi_keeps_prev_when_alpha_zero() {
        assert_eq!(ema_obi(0.7, 0.123, 0.0), 0.123);
    }

    #[test]
    fn z_score_update_first_step_from_zero() {
        let (mu, var) = z_score_update(1.0, 0.0, 0.0, 0.5);
        assert!((mu - 0.5).abs() < 1e-12);
        assert!((var - 0.125).abs() < 1e-12);
    }

    #[test]
    fn z_score_update_converges_on_constant_input() {
        let mut mu = 0.0;
        let mut var = 1.0;
        for _ in 0..200 {
            let (m, v) = z_score_update(1.0, mu, var, 0.1);
            mu = m;
            var = v;
        }
        assert!((mu - 1.0).abs() < 1e-6);
        assert!(var.abs() < 1e-6);
    }

    #[test]
    fn fragility_uses_abs_and_eps() {
        assert!((fragility(0.5, 1.0) - 0.5).abs() < 1e-6);
        assert!((fragility(-0.5, 1.0) - 0.5).abs() < 1e-6);
        assert!(fragility(0.5, 0.0).is_finite());
    }

    #[test]
    fn total_depth_sums_both_sides_all_levels() {
        let bids = json!([["10.0", "1.0"], ["9.0", "2.0"]]);
        let asks = json!([["11.0", "3.0"], ["12.0", "4.0"], ["13.0", "5.0"]]);
        assert!((total_depth_arr(&bids, &asks) - 15.0).abs() < 1e-12);
    }

    #[test]
    fn imbalance_accepts_numeric_levels() {
        let bids = json!([[10.0, 3.0]]);
        let asks = json!([[11.0, 1.0]]);
        assert!((imbalance_arr(&bids, &asks) - 0.5).abs() < 1e-12);
    }

    #[test]
    fn data_bas_mexc_parses_real_message() {
        let v = json!({
            "date_time": "2026-06-29T19:51:52.375992554Z",
            "data": {
                "asks": [[60520.4, 83807, 5], [60641.7, 0, 0]],
                "bids": [[60520.3, 203961, 1], [60517.2, 7188, 1], [58704.7, 36, 2]],
                "end": 39121987808_i64,
                "begin": 39121987800_i64,
                "version": 39121987808_i64,
                "cts": 1782762712191_i64
            },
            "topic": "push.depth.BTC_USDT",
            "symbol": "BTC_USDT",
            "channel": "push.depth",
            "ts": 1782762712201_i64,
            "instant": 1782762712375988224_i64
        });
        let obj = v.as_object().expect("obj");

        let (bids, asks, symbol, bid, ask, best_price) =
            data_bas_mexc(obj).expect("сообщение MEXC должно распарситься");

        assert_eq!(symbol, "BTC_USDT");
        assert!((fun::to_f64(&bid) - 60520.3).abs() < 1e-9);
        assert!((fun::to_f64(&ask) - 60520.4).abs() < 1e-9);
        assert!((fun::to_f64(&best_price) - 60520.3).abs() < 1e-9);
        assert!((price_at(bids, 0) - 60520.3).abs() < 1e-9);
        assert!((fun::vol_at(bids, 0) - 203961.0).abs() < 1e-9);
        assert!((price_at(asks, 0) - 60520.4).abs() < 1e-9);
        assert!((fun::vol_at(asks, 0) - 83807.0).abs() < 1e-9);
        assert!((imbalance_arr(bids, asks) - 0.4317).abs() < 1e-3);
    }

    #[test]
    fn best_price_prefers_bid() {
        let best = best_price(&json!(60520.3), &json!(60520.4));
        assert!((fun::to_f64(&best) - 60520.3).abs() < 1e-9);
    }

    #[test]
    fn best_price_falls_back_to_ask_when_bid_empty() {
        let best = best_price(&Value::default(), &json!(60520.4));
        assert!((fun::to_f64(&best) - 60520.4).abs() < 1e-9);
    }

    #[test]
    fn best_price_accepts_string_prices() {
        let best = best_price(&json!("60520.3"), &json!("60520.4"));
        assert!((fun::to_f64(&best) - 60520.3).abs() < 1e-9);
    }

    #[test]
    fn best_price_zero_when_both_sides_empty() {
        let best = best_price(&Value::default(), &Value::default());
        assert_eq!(fun::to_f64(&best), 0.0);
    }
}
