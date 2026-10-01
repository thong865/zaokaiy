//! End-to-end: a terminal pairs, goes offline, keeps selling, comes back and syncs.
//! Needs a running API with demo data: `ZK_API=http://localhost:8080/api cargo test --test offline_sync`
//! (skipped when ZK_API is not set).

use std::sync::Arc;

use serde_json::{json, Value};
use zaokaiy_pos_lib::{
    api::Api,
    db::{NewSale, Store},
    money::{Line, Payment},
    sync::Engine,
};

async fn http(method: &str, url: &str, token: Option<&str>, body: Option<Value>) -> Value {
    let c = reqwest::Client::new();
    let mut r = match method {
        "GET" => c.get(url),
        "PATCH" => c.patch(url),
        _ => c.post(url),
    };
    if let Some(t) = token {
        r = r.bearer_auth(t);
    }
    if let Some(b) = body {
        r = r.json(&b);
    }
    let res = r.send().await.unwrap();
    let status = res.status();
    let v: Value = res.json().await.unwrap_or(Value::Null);
    assert!(status.is_success(), "{method} {url} -> {status}: {v}");
    v
}

fn sale(pid: &str, name: &str, qty: i32, price: i64) -> NewSale {
    NewSale {
        lines: vec![Line { product_id: Some(pid.into()), name: name.into(), sku: String::new(), qty, unit_price_cents: price, discount_cents: 0 }],
        discount_cents: 0,
        payments: vec![Payment { method: "cash".into(), amount_cents: price * qty as i64 + 500_000, reference: String::new() }],
        note: String::new(),
        customer: None,
        bill_id: None,
    }
}

#[tokio::test]
async fn offline_then_online() {
    let Ok(api_base) = std::env::var("ZK_API") else {
        eprintln!("ZK_API not set — skipping");
        return;
    };
    let run = uuid::Uuid::new_v4().simple().to_string()[..6].to_string();

    // A shop with two products.
    let owner = http("POST", &format!("{api_base}/auth/register"), None, Some(json!({ "email": format!("e2e-{run}@t.dev"), "password": "password123", "display_name": "E2E" }))).await["token"]
        .as_str()
        .unwrap()
        .to_string();
    let admin = http("POST", &format!("{api_base}/auth/login"), None, Some(json!({ "email": "admin@demo.dev", "password": "password123" }))).await["token"].as_str().unwrap().to_string();
    let shop = http("POST", &format!("{api_base}/shops"), Some(&owner), Some(json!({ "slug": format!("e2e-{run}"), "name": "ຮ້ານດາວ E2E", "currency": "LAK" }))).await;
    let sid = shop["id"].as_str().unwrap().to_string();
    http("PATCH", &format!("{api_base}/admin/shops/{sid}"), Some(&admin), Some(json!({ "auto_approve": true }))).await;
    let mk = |sku: &str, name: &str, price: i64, stock: i64| {
        json!({ "sku": format!("{sku}{run}"), "name": name, "price_cents": price, "status": "active", "initial_stock": stock, "category": "handmade", "barcode": null })
    };
    let beer = http("POST", &format!("{api_base}/shops/{sid}/products"), Some(&owner), Some(mk("beer", "ເບຍລາວ", 1_500_000, 24))).await;
    let rice = http("POST", &format!("{api_base}/shops/{sid}/products"), Some(&owner), Some(mk("rice", "Sticky rice 1kg", 2_500_000, 5))).await;
    let (beer_id, rice_id) = (beer["id"].as_str().unwrap().to_string(), rice["id"].as_str().unwrap().to_string());

    // Pair the terminal.
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(Store::open(&dir.path().join("pos.sqlite")).unwrap());
    let engine = Engine::new(store.clone());
    let a = Api::new(&api_base, None, "en");
    let p = a.pair_start("E2E till", &format!("install-{run}-e2e")).await.unwrap();
    http("POST", &format!("{api_base}/pos/pair/approve"), Some(&owner), Some(json!({ "code": p["code"], "shop_id": sid }))).await;
    let r = a.pair_poll(p["pair_id"].as_str().unwrap(), p["poll_secret"].as_str().unwrap()).await.unwrap();
    store.set("api_base", &api_base).unwrap();
    store.set("token", r["token"].as_str().unwrap()).unwrap();

    let s = engine.cycle().await;
    assert!(s.online && s.paired && s.products == 2 && s.pending == 0, "{s:?}");
    assert_eq!(store.get_json("device")["code"], "T01");
    assert_eq!(store.scan(&format!("beer{run}")).unwrap().unwrap().stock, 24);

    // --- the internet goes down -----------------------------------------------------------
    store.set("api_base", "http://127.0.0.1:9/api").unwrap();
    let shop_v = store.get_json("shop");
    let dev_v = store.get_json("device");
    let s1 = store.create_sale(&sale(&beer_id, "ເບຍລາວ", 6, 1_500_000), &shop_v, &dev_v, Value::Null).unwrap();
    let s2 = store.create_sale(&sale(&rice_id, "Sticky rice 1kg", 7, 2_500_000), &shop_v, &dev_v, Value::Null).unwrap(); // more than the 5 in stock
    let s3 = store.create_sale(&sale(&beer_id, "ເບຍລາວ", 1, 1_500_000), &shop_v, &dev_v, Value::Null).unwrap();
    assert_eq!((s1.number.as_str(), s3.number.as_str()), ("RT01-000001", "RT01-000003"));
    store.void_sale(&s3.id, "wrong item").unwrap();
    let s = engine.cycle().await;
    assert!(!s.online && s.pending == 4, "offline, 3 sales + 1 void queued: {s:?}");
    assert_eq!(store.scan(&format!("beer{run}")).unwrap().unwrap().stock, 18); // 24 − 6 (voided one not counted)

    // Meanwhile the owner changes the price and the web till sells 2 beers.
    http("PATCH", &format!("{api_base}/products/{beer_id}"), Some(&owner), Some(json!({ "price_cents": 1_600_000 }))).await;
    http("POST", &format!("{api_base}/shops/{sid}/pos/sales"), Some(&owner),
         Some(json!({ "items": [{ "product_id": beer_id, "qty": 2 }], "payments": [{ "method": "cash", "amount_cents": 3_200_000 }] }))).await;

    // --- back online ------------------------------------------------------------------------
    store.set("api_base", &api_base).unwrap();
    let s = engine.cycle().await;
    assert!(s.online && s.pending == 0 && s.rejected == 0, "{s:?}");
    let beer_now = store.scan(&format!("beer{run}")).unwrap().unwrap();
    assert_eq!((beer_now.stock, beer_now.price_cents), (24 - 2 - 6, 1_600_000));
    assert_eq!(store.scan(&format!("rice{run}")).unwrap().unwrap().stock, 0);
    let r2 = store.get_sale(&s2.id).unwrap().unwrap();
    assert_eq!(r2.sync_state, "synced");
    assert_eq!(r2.shortfall[0]["qty"], 2, "7 sold offline, 5 were in stock");
    assert_eq!(store.get_sale(&s3.id).unwrap().unwrap().void_sync, "synced");

    let server = http("GET", &format!("{api_base}/pos/sales/{}", s3.id), Some(&owner), None).await;
    assert_eq!(server["sale"]["status"], "voided");
    assert_eq!(server["sale"]["number"], "RT01-000003");
    let list = http("GET", &format!("{api_base}/shops/{sid}/pos/sales"), Some(&owner), None).await;
    assert_eq!(list.as_array().unwrap().len(), 4); // 3 from the terminal + 1 from the web till

    // Syncing again changes nothing.
    let s = engine.cycle().await;
    assert!(s.online && s.pending == 0);
    // The next receipt continues the terminal's series.
    let s4 = store.create_sale(&sale(&beer_id, "ເບຍລາວ", 1, 1_600_000), &store.get_json("shop"), &store.get_json("device"), Value::Null).unwrap();
    assert_eq!(s4.number, "RT01-000004");
}
