use std::io::{self, Read};

use f_common::host;
use sonic_rs::{JsonContainerTrait, JsonValueTrait, Object, Value};

fn num(v: &Value, k: &str) -> Option<i64> {
    let f = v.get(k)?;
    f.as_i64().or_else(|| f.as_f64().map(|x| x as i64))
}

fn symbols_from(v: Option<&Value>) -> Vec<String> {
    let Some(v) = v else {
        return Vec::new();
    };
    if let Some(arr) = v.as_array() {
        return arr
            .iter()
            .filter_map(|x| x.as_str().map(|s| s.to_string()))
            .collect();
    }
    if let Some(s) = v.as_str() {
        return s
            .trim()
            .trim_start_matches('[')
            .trim_end_matches(']')
            .split(',')
            .map(|x| x.trim().trim_matches('"').to_string())
            .filter(|x| !x.is_empty())
            .collect();
    }
    Vec::new()
}

fn get_value(setting_code: &str, key: &str) -> Vec<Value> {
    eprintln!("f-watchdog::get_value::setting_code = {setting_code} key = {key}");

    let mut req = Object::with_capacity(2);
    req.insert("setting_code", setting_code);
    req.insert("key", key);
    let bytes = match sonic_rs::to_vec(&req) {
        Ok(b) => b,
        Err(_) => return Vec::new(),
    };
    match host::get_value(&bytes) {
        Ok(Some(b)) => sonic_rs::from_slice::<Vec<Value>>(&b).unwrap_or_default(),
        _ => Vec::new(),
    }
}

fn feed_is_live(setting_code: &str, symbols: &[String], lag_ns: i64, now_ns: i64) -> bool {
    symbols.iter().any(|sym| {
        eprintln!("f-watchdog::feed_is_live::setting_code = {setting_code} sym = {sym}");
        let instant = get_value(setting_code, sym)
            .first()
            .and_then(|v| num(v, "instant"))
            .unwrap_or(0);
        instant != 0 && now_ns - instant <= lag_ns
    })
}

fn get_function_value(setting_code: &str, key: &str) -> Vec<Value> {
    let mut req = Object::with_capacity(2);
    req.insert("setting_code", setting_code);
    req.insert("key", key);
    let bytes = match sonic_rs::to_vec(&req) {
        Ok(b) => b,
        Err(_) => return Vec::new(),
    };
    match host::get_function_value(&bytes) {
        Ok(Some(b)) => sonic_rs::from_slice::<Vec<Value>>(&b).unwrap_or_default(),
        _ => Vec::new(),
    }
}

fn put_ts(setting_code: &str, key: &str, instant: i64) {
    eprintln!("f-watchdog: setting_code = {setting_code} put_ts");
    let mut val = Object::with_capacity(1);
    val.insert("instant", instant);
    let mut req = Object::with_capacity(3);
    req.insert("setting_code", setting_code);
    req.insert("key", key);
    req.insert("value", val.into_value());
    let bytes = match sonic_rs::to_vec(&req) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("f-watchdog: setting_code = {setting_code} put_ts serialize: {e}");
            return;
        }
    };
    if let Err(rc) = host::put_value(&bytes) {
        eprintln!("f-watchdog: setting_code = {setting_code} put_value rc = {rc}");
    }
}

fn http(method: &str, url: &str, body: Option<&str>) -> Result<(), i32> {
    let mut req = Object::with_capacity(4);
    req.insert("method", method);
    req.insert("url", url);
    if let Some(b) = body {
        req.insert("body", b);
        req.insert("content_type", "application/json");
    }
    let bytes = sonic_rs::to_vec(&req).map_err(|_| -100i32)?;
    host::http_request(&bytes)
}

fn restart_feed(url: &str, config_code: &str, cfg: &Value) {
    let base = url.trim_end_matches('/');
    let stop = format!("{base}/stop/{config_code}");
    let config = format!("{base}/config/{config_code}");
    let start = format!("{base}/start/{config_code}");

    let _ = http("POST", &stop, None);

    let body = match sonic_rs::to_string(cfg) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("f-watchdog: config_code = {config_code} restart_feed serialize cfg: {e}");
            return;
        }
    };
    if let Err(rc) = http("PUT", &config, Some(&body)) {
        eprintln!("f-watchdog: config_code = {config_code} restart_feed config rc = {rc}");
        return;
    }
    if let Err(rc) = http("POST", &start, None) {
        eprintln!("f-watchdog: config_code = {config_code} restart_feed start rc = {rc}");
    }
}

fn stop_feed(url: &str, config_code: &str) {
    let base = url.trim_end_matches('/');
    let stop = format!("{base}/stop/{config_code}");
    if let Err(rc) = http("POST", &stop, None) {
        eprintln!("f-watchdog: config_code = {config_code} stop_feed rc = {rc}");
    }
}

fn main() {
    eprintln!("f-watchdog:::::::::::::::::: 0005 f-watchdog");
    let mut buf = Vec::new();
    if let Err(e) = io::stdin().read_to_end(&mut buf) {
        eprintln!("f-watchdog: read stdin: {e}");
        return;
    }
    let input: Value = match sonic_rs::from_slice(&buf) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("f-watchdog:::::::::::::::::: bad input json: {e}");
            return;
        }
    };

    let setting_code = input.get("setting_code").as_str().unwrap_or("").to_string();
    eprintln!("f-watchdog:::::::::::::::::: setting_code: {setting_code}");
    let key = input.get("key").as_str().unwrap_or("").to_string();
    let now_ns = num(&input, "now_ns").unwrap_or(0);

    if setting_code.is_empty() || key.is_empty() || now_ns == 0 {
        eprintln!("f-watchdog: setting_code = {setting_code} missing setting_code/key/now_ns");
        return;
    }

    eprintln!("f-watchdog: setting_code = {setting_code}.{key}");

    let cfg_arr = get_function_value(&setting_code, &key);
    if cfg_arr.is_empty() {
        eprintln!("f-watchdog: setting_code = {setting_code} no config at {setting_code}.{key}");
        return;
    }

    let l = cfg_arr.len();
    eprintln!(
        "f-watchdog: setting_code = {setting_code} start key = {key} now_ns = {now_ns} cfg_arr = {l}"
    );

    for cfg in &cfg_arr {
        check_feed(&setting_code, cfg, now_ns);
    }
}

fn check_feed(setting_code: &str, cfg: &Value, now_ns: i64) {
    let url = cfg
        .get("exchenge_ws_url")
        .as_str()
        .unwrap_or("")
        .to_string();
    let config_code = cfg.get("config_code").as_str().unwrap_or("").to_string();

    if url.is_empty() || config_code.is_empty() {
        eprintln!(
            "f-watchdog: config_code = {config_code} incomplete config (url = {:?}, config_code = {:?})",
            url, config_code
        );
        return;
    }

    let symbols = symbols_from(cfg.get("symbol"));
    let lag_ns = num(cfg, "lag_ns").unwrap_or(0);

    if symbols.is_empty() {
        eprintln!(
            "f-watchdog: config_code = {config_code} incomplete config (symbols = 0, config_code = {config_code:?})"
        );
        return;
    }

    if !cfg.get("is_active").as_bool().unwrap_or(true) {
        if feed_is_live(setting_code, &symbols, lag_ns, now_ns) {
            eprintln!("f-watchdog: {config_code} is_active = false > stop");
            stop_feed(&url, &config_code);
        } else {
            eprintln!("f-watchdog: config_code = {config_code} is_active = false > skip");
        }
        return;
    }
    eprintln!("f-watchdog: config_code = {config_code} is_active = true");

    let cooldown_ns = num(cfg, "cooldown_ns").unwrap_or(0);

    let mut stale: Vec<String> = Vec::new();
    for sym in &symbols {
        let vals = get_value(setting_code, sym);
        let instant = vals.first().and_then(|v| num(v, "instant")).unwrap_or(0);
        let age = now_ns - instant;

        if instant == 0 || age > lag_ns {
            eprintln!(
                "f-watchdog: config_code = {config_code} sym = {sym} now_ns = {now_ns} - instant = {instant} = age = {age} ns > lag_ns = {lag_ns} , cooldown_ns = {cooldown_ns} = LAG"
            );
            stale.push(format!("{sym}(age = {age}ms)"));
        } else {
            eprintln!(
                "f-watchdog: config_code = {config_code} sym = {sym} now_ns = {now_ns} instant = {instant} age = {age} ns > lag_ns = {lag_ns} , cooldown_ns = {cooldown_ns} = OK"
            );
        }
    }

    if stale.is_empty() {
        return; 
    }

    let cd_key = format!("__wd_cooldown_{config_code}");
    let last = get_value(setting_code, &cd_key)
        .first()
        .and_then(|v| num(v, "instant"))
        .unwrap_or(0);

    let rr = now_ns - last;
    if last != 0 && rr < cooldown_ns {
        eprintln!(
            "f-watchdog: config_code = {config_code} stale {:?}, {} ms ",
            stale,
            cooldown_ns - (now_ns - last)
        );
        return;
    }

    eprintln!(
        "f-watchdog: config_code = {config_code} {:?} cd_key = {cd_key} rr = {rr} :: now_ns = {now_ns} - last = {last} (lag_ns = {lag_ns}, cooldown_ns = {cooldown_ns}) > restart {config_code} @ {url}",
        stale
    );
    restart_feed(&url, &config_code, cfg);
    put_ts(setting_code, &cd_key, now_ns);
}
