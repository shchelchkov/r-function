use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use axum::routing::get;
use r_db::db::db::Database;
use r_router::route::db::{get_values, get_values_by_setting_code};
use sonic_rs::{JsonContainerTrait, JsonValueTrait, Value};
use std::sync::Arc;
use tower::ServiceExt;

fn app() -> (tempfile::TempDir, Router) {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = Database::new(dir.path(), 8).expect("open db");

    let v = |s: &str| sonic_rs::from_str::<Value>(s).unwrap();
    db.insert_value("a", Arc::from("x"), v("1")).unwrap();
    db.insert_value("a", Arc::from("x"), v("2")).unwrap();
    db.insert_value("ab", Arc::from("x"), v("3")).unwrap();
    db.insert_value("b", Arc::from("y"), v("4")).unwrap();

    let router = Router::new()
        .route("/db", get(get_values))
        .route("/db/{setting_code}", get(get_values_by_setting_code))
        .with_state(db);

    (dir, router)
}

async fn get_json(router: Router, uri: &str) -> (StatusCode, Value) {
    let response = router
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
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

fn keys(json: &Value) -> Vec<String> {
    let mut keys: Vec<String> = json["data"]
        .as_array()
        .expect("data is array")
        .iter()
        .map(|e| e["key"].as_str().expect("key").to_owned())
        .collect();
    keys.sort();
    keys
}

#[tokio::test]
async fn db_lists_every_persisted_key() {
    let (_dir, router) = app();
    let (status, json) = get_json(router, "/db").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(keys(&json), ["a.x", "ab.x", "b.y"]);
}

#[tokio::test]
async fn db_by_setting_code_is_prefix_scoped() {
    let (_dir, router) = app();
    let (status, json) = get_json(router, "/db/a").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(keys(&json), ["a.x"], "`ab.x` не должен попадать в `a`");

    let entry = &json["data"][0];
    assert_eq!(sonic_rs::to_string(&entry["values"]).unwrap(), "[2,1]");
}

#[tokio::test]
async fn db_unknown_setting_code_is_empty_not_error() {
    let (_dir, router) = app();
    let (status, json) = get_json(router, "/db/nope").await;

    assert_eq!(status, StatusCode::OK);
    assert!(keys(&json).is_empty());
}
