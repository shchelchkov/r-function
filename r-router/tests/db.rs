use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Method, Request, StatusCode};
use axum::routing::get;
use r_db::db::db::{Database, DatabaseOptions};
use r_router::route::db::{
    delete_value, get_history, get_registry, get_registry_by_setting_code, get_value, get_values,
    get_values_by_setting_code,
};
use sonic_rs::{JsonContainerTrait, JsonValueTrait, Value};
use tower::ServiceExt;

fn app() -> (tempfile::TempDir, Router) {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = Database::open(
        dir.path(),
        DatabaseOptions {
            limit: 8,
            history: 100,
        },
    )
    .expect("open db");

    let v = |s: &str| sonic_rs::from_str::<Value>(s).unwrap();
    db.insert_value("a", "x", v("1")).unwrap();
    db.insert_value("a", "x", v("2")).unwrap();
    db.insert_value("ab", "x", v("3")).unwrap();
    db.insert_value("b", "y", v("4")).unwrap();
    db.insert_value("a.b", "c", v("5")).unwrap();

    for ts in 1..=5u64 {
        db.append_value("h", "k", ts, v(&format!("{{\"t\":{ts}}}")))
            .unwrap();
    }

    let router = Router::new()
        .route("/db/registry", get(get_registry))
        .route(
            "/db/registry/{setting_code}",
            get(get_registry_by_setting_code),
        )
        .route("/db/values", get(get_values))
        .route("/db/values/{setting_code}", get(get_values_by_setting_code))
        .route(
            "/db/values/{setting_code}/{key}",
            get(get_value).delete(delete_value),
        )
        .route("/db/history/{setting_code}/{key}", get(get_history))
        .with_state(db);

    (dir, router)
}

async fn call(router: Router, method: Method, uri: &str) -> (StatusCode, Value) {
    let response = router
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = to_bytes(response.into_body(), 1 << 20).await.unwrap();
    let json = if body.is_empty() {
        Value::default()
    } else {
        sonic_rs::from_slice(&body).unwrap_or_else(|e| panic!("{uri}: not json ({e}): {body:?}"))
    };
    (status, json)
}

async fn get_json(router: Router, uri: &str) -> (StatusCode, Value) {
    call(router, Method::GET, uri).await
}

fn pairs(json: &Value) -> Vec<String> {
    let mut keys: Vec<String> = json["data"]
        .as_array()
        .expect("data is array")
        .iter()
        .map(|e| {
            format!(
                "{}/{}",
                e["setting_code"].as_str().expect("setting_code"),
                e["key"].as_str().expect("key")
            )
        })
        .collect();
    keys.sort();
    keys
}

#[tokio::test]
async fn db_lists_every_persisted_key() {
    let (_dir, router) = app();
    let (status, json) = get_json(router, "/db/values").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(pairs(&json), ["a.b/c", "a/x", "ab/x", "b/y"]);
}

#[tokio::test]
async fn db_by_setting_code_is_prefix_scoped() {
    let (_dir, router) = app();
    let (status, json) = get_json(router, "/db/values/a").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        pairs(&json),
        ["a/x"],
        "`ab` и `a.b` не должны попадать в `a`"
    );

    let entry = &json["data"][0];
    assert_eq!(sonic_rs::to_string(&entry["values"]).unwrap(), "[2,1]");
}

#[tokio::test]
async fn db_unknown_setting_code_is_empty_not_error() {
    let (_dir, router) = app();
    let (status, json) = get_json(router, "/db/values/nope").await;

    assert_eq!(status, StatusCode::OK);
    assert!(pairs(&json).is_empty());
}

#[tokio::test]
async fn db_single_key_and_not_found() {
    let (_dir, router) = app();
    let (status, json) = get_json(router.clone(), "/db/values/a.b/c").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(sonic_rs::to_string(&json["data"]).unwrap(), "[5]");

    let (status, _) = get_json(router, "/db/values/a/nope").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn db_delete_reports_presence() {
    let (_dir, router) = app();

    let (status, _) = call(router.clone(), Method::DELETE, "/db/values/b/y").await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, _) = call(router.clone(), Method::DELETE, "/db/values/b/y").await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, json) = get_json(router, "/db/values").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(pairs(&json), ["a.b/c", "a/x", "ab/x"]);
}

#[tokio::test]
async fn db_history_honours_bounds_limit_and_order() {
    let (_dir, router) = app();

    let ts = |json: &Value| -> Vec<u64> {
        json["data"]
            .as_array()
            .expect("data is array")
            .iter()
            .map(|e| e["timestamp"].as_u64().expect("timestamp"))
            .collect()
    };

    let (status, json) = get_json(router.clone(), "/db/history/h/k").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(ts(&json), [1, 2, 3, 4, 5]);
    assert_eq!(
        sonic_rs::to_string(&json["data"][0]["value"]).unwrap(),
        r#"{"t":1}"#
    );

    let (_, json) = get_json(router.clone(), "/db/history/h/k?from=2&to=4").await;
    assert_eq!(ts(&json), [2, 3, 4]);

    let (_, json) = get_json(router.clone(), "/db/history/h/k?limit=2&order=desc").await;
    assert_eq!(ts(&json), [5, 4]);

    let (status, json) = get_json(router, "/db/history/h/nope").await;
    assert_eq!(status, StatusCode::OK);
    assert!(ts(&json).is_empty());
}

#[tokio::test]
async fn registry_lists_keys_of_both_tiers() {
    let (_dir, router) = app();
    let (status, json) = get_json(router.clone(), "/db/registry").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(pairs(&json), ["a.b/c", "a/x", "ab/x", "b/y", "h/k"]);

    let flags = |json: &Value, pair: &str| -> (bool, bool) {
        let e = json["data"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| {
                format!(
                    "{}/{}",
                    e["setting_code"].as_str().unwrap(),
                    e["key"].as_str().unwrap()
                ) == pair
            })
            .unwrap_or_else(|| panic!("{pair} not in registry"));
        (
            e["has_values"].as_bool().unwrap(),
            e["has_history"].as_bool().unwrap(),
        )
    };
    assert_eq!(flags(&json, "a/x"), (true, false));
    assert_eq!(flags(&json, "h/k"), (false, true));
    assert!(json["data"][0]["updated_at"].as_u64().unwrap() > 0);

    let (status, json) = get_json(router.clone(), "/db/registry/a").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        pairs(&json),
        ["a/x"],
        "`ab` и `a.b` не должны попадать в `a`"
    );

    let (status, _) = call(router.clone(), Method::DELETE, "/db/values/b/y").await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, json) = get_json(router, "/db/registry/b").await;
    assert!(pairs(&json).is_empty());
}
