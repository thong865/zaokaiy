//! zaokaiy POS desktop app (Tauri 2). The Rust side owns the local database, the sync engine and
//! printing; the Nuxt UI talks to it only through the commands below.

pub mod api;
pub mod db;
pub mod money;
pub mod printer;
pub mod sync;

use std::sync::{Arc, Mutex};

use base64::Engine as _;
use serde::Deserialize;
use serde_json::{json, Value};
use tauri::{Emitter, Manager, State};

use crate::{
    api::Api,
    db::{NewSale, ProductView, SaleRecord, Store},
    sync::{Engine, SyncStatus},
};

type CmdResult<T> = Result<T, String>;

struct Pairing {
    api_base: String,
    pair_id: String,
    secret: String,
}

pub struct App {
    engine: Arc<Engine>,
    pairing: Mutex<Option<Pairing>>,
}

impl App {
    fn store(&self) -> &Store {
        &self.engine.store
    }
}

fn install_id(store: &Store) -> String {
    store.get("install_id").unwrap_or_else(|| {
        let id = uuid::Uuid::new_v4().to_string();
        let _ = store.set("install_id", &id);
        id
    })
}

/// "https://shop.example/api" → "https://shop.example" (where the web app lives).
fn web_from_api(api: &str) -> String {
    api.trim_end_matches('/').trim_end_matches("/api").to_string()
}

fn normalize_base(s: &str) -> CmdResult<String> {
    let s = s.trim().trim_end_matches('/');
    if !(s.starts_with("https://") || s.starts_with("http://")) || s.len() < 10 {
        return Err("enter the server address, e.g. https://zaokaiy.com/api".into());
    }
    Ok(if s.ends_with("/api") { s.to_string() } else { format!("{s}/api") })
}

// ---------------------------------------------------------------------------------------------
// App state & settings
// ---------------------------------------------------------------------------------------------

#[tauri::command]
fn app_info(app: State<'_, App>) -> Value {
    let s = app.store();
    json!({
        "version": env!("CARGO_PKG_VERSION"),
        "os": std::env::consts::OS,
        "install_id": install_id(s),
        "api_base": s.get("api_base"),
        "web_base": s.get("web_base"),
        "lang": s.get("lang").unwrap_or_else(|| "en".into()),
        "settings": s.get_json("settings"),
        "shop": s.get_json("shop"),
        "device": s.get_json("device"),
        "status": app.engine.status(),
    })
}

#[tauri::command]
fn set_lang(app: State<'_, App>, lang: String) -> CmdResult<()> {
    let lang = if lang == "lo" { "lo" } else { "en" };
    app.store().set("lang", lang)
}

/// Printer, scanner and display preferences (free-form JSON owned by the UI).
#[tauri::command]
fn save_settings(app: State<'_, App>, settings: Value) -> CmdResult<()> {
    if !settings.is_object() {
        return Err("settings must be an object".into());
    }
    app.store().set_json("settings", &settings)
}

// ---------------------------------------------------------------------------------------------
// Pairing
// ---------------------------------------------------------------------------------------------

#[tauri::command]
async fn pair_start(app: State<'_, App>, api_base: String, web_base: Option<String>, name: String) -> CmdResult<Value> {
    let api_base = normalize_base(&api_base)?;
    let web = web_base.map(|w| w.trim().trim_end_matches('/').to_string()).filter(|w| !w.is_empty()).unwrap_or_else(|| web_from_api(&api_base));
    let lang = app.store().get("lang").unwrap_or_else(|| "en".into());
    let api = Api::new(&api_base, None, &lang);
    let r = api.pair_start(name.trim(), &install_id(app.store())).await.map_err(|e| e.to_string())?;
    let (Some(pair_id), Some(secret), Some(code)) = (r["pair_id"].as_str(), r["poll_secret"].as_str(), r["code"].as_str()) else {
        return Err("unexpected answer from the server".into());
    };
    *app.pairing.lock().unwrap() = Some(Pairing { api_base: api_base.clone(), pair_id: pair_id.into(), secret: secret.into() });
    app.store().set("web_base", &web)?;
    Ok(json!({
        "code": code,
        "expires_at": r["expires_at"],
        "interval": r["interval"],
        "approve_url": format!("{web}/pos-pair?code={code}"),
    }))
}

/// pending | expired | approved. On approval the device token is stored and a full sync starts.
#[tauri::command]
async fn pair_poll(app: State<'_, App>) -> CmdResult<Value> {
    let (base, pair_id, secret) = {
        let p = app.pairing.lock().unwrap();
        let p = p.as_ref().ok_or("start pairing first")?;
        (p.api_base.clone(), p.pair_id.clone(), p.secret.clone())
    };
    let api = Api::new(&base, None, &app.store().get("lang").unwrap_or_else(|| "en".into()));
    let r = api.pair_poll(&pair_id, &secret).await.map_err(|e| e.to_string())?;
    if r["status"] != "approved" {
        return Ok(json!({ "status": r["status"] }));
    }
    let token = r["token"].as_str().ok_or("unexpected answer from the server")?;
    let new_shop = r["shop"]["id"].as_str().unwrap_or_default();
    let s = app.store();
    if s.get_json("shop")["id"].as_str() != Some(new_shop) || s.get("api_base").as_deref() != Some(base.as_str()) {
        s.reset_catalogue()?;
    }
    s.set("api_base", &base)?;
    s.set("token", token)?;
    // Terminal + shop details come with the first sync round.
    s.del("device")?;
    s.del("shop")?;
    *app.pairing.lock().unwrap() = None;
    let status = app.engine.cycle().await;
    Ok(json!({ "status": "approved", "shop": r["shop"], "sync": status }))
}

/// Forget the pairing. Refused while sales are still waiting for upload unless `force`.
#[tauri::command]
fn unpair(app: State<'_, App>, force: bool) -> CmdResult<()> {
    let s = app.store();
    let waiting = s.unsynced_total();
    if waiting > 0 && !force {
        return Err(format!("{waiting} sales are not uploaded yet — connect to the internet and sync first"));
    }
    s.del("token")?;
    s.reset_catalogue()?;
    app.engine.kick();
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// Catalogue
// ---------------------------------------------------------------------------------------------

#[tauri::command]
fn search_products(app: State<'_, App>, q: String, category_id: Option<String>, limit: Option<i64>) -> CmdResult<Vec<ProductView>> {
    app.store().search(&q, category_id.as_deref().filter(|c| !c.is_empty()), limit.unwrap_or(120))
}

#[tauri::command]
fn scan_code(app: State<'_, App>, code: String) -> CmdResult<Option<ProductView>> {
    app.store().scan(&code)
}

#[tauri::command]
fn list_categories(app: State<'_, App>) -> CmdResult<Vec<db::Category>> {
    app.store().categories()
}

// ---------------------------------------------------------------------------------------------
// Sales
// ---------------------------------------------------------------------------------------------

#[tauri::command]
fn create_sale(app: State<'_, App>, input: NewSale) -> CmdResult<SaleRecord> {
    let s = app.store();
    if s.get("token").is_none() {
        return Err("this terminal is not paired with a shop".into());
    }
    let shop = s.get_json("shop");
    let device = s.get_json("device");
    let print = json!({
        "shop": shop, "device_code": device["code"], "device_name": device["name"],
        "cashier": device["cashier"]["name"],
    });
    let sale = s.create_sale(&input, &shop, &device, print)?;
    app.engine.kick();
    Ok(sale)
}

#[tauri::command]
fn list_sales(app: State<'_, App>, from: Option<String>, to: Option<String>, q: Option<String>, state: Option<String>, limit: Option<i64>) -> CmdResult<Vec<SaleRecord>> {
    app.store().list_sales(from.as_deref(), to.as_deref(), q.as_deref(), state.as_deref().filter(|s| !s.is_empty()), limit.unwrap_or(300))
}

#[tauri::command]
fn get_sale(app: State<'_, App>, id: String) -> CmdResult<Option<SaleRecord>> {
    app.store().get_sale(&id)
}

#[tauri::command]
fn void_sale(app: State<'_, App>, id: String, reason: String) -> CmdResult<SaleRecord> {
    let s = app.store();
    if s.get_json("device")["cashier"]["can_void"] != Value::Bool(true) {
        return Err("your role is not allowed to void sales".into());
    }
    let r = s.void_sale(&id, &reason)?;
    app.engine.kick();
    Ok(r)
}

#[tauri::command]
fn retry_sale(app: State<'_, App>, id: String) -> CmdResult<()> {
    app.store().retry(&id)?;
    app.engine.kick();
    Ok(())
}

#[tauri::command]
fn sales_summary(app: State<'_, App>, from: String, to: String) -> CmdResult<Value> {
    app.store().summary(&from, &to)
}

// ---------------------------------------------------------------------------------------------
// Sync
// ---------------------------------------------------------------------------------------------

#[tauri::command]
fn sync_status(app: State<'_, App>) -> SyncStatus {
    app.engine.status()
}

#[tauri::command]
async fn sync_now(app: State<'_, App>) -> CmdResult<SyncStatus> {
    Ok(app.engine.cycle().await)
}

// ---------------------------------------------------------------------------------------------
// Parked bills (local to this terminal)
// ---------------------------------------------------------------------------------------------

#[tauri::command]
fn list_bills(app: State<'_, App>) -> CmdResult<Vec<Value>> {
    app.store().bills()
}

#[tauri::command]
fn save_bill(app: State<'_, App>, id: String, label: String, data: Value) -> CmdResult<()> {
    app.store().save_bill(&id, &label, &data)
}

#[tauri::command]
fn delete_bill(app: State<'_, App>, id: String) -> CmdResult<()> {
    app.store().delete_bill(&id)
}

// ---------------------------------------------------------------------------------------------
// Printing
// ---------------------------------------------------------------------------------------------

#[derive(Deserialize)]
struct PrinterTarget {
    /// "network" (host:port raw TCP) or "system" (installed printer by name)
    kind: String,
    #[serde(default)]
    host: String,
    #[serde(default)]
    port: Option<u16>,
    #[serde(default)]
    name: String,
}

async fn send_job(target: &PrinterTarget, job: Vec<u8>) -> CmdResult<()> {
    match target.kind.as_str() {
        "network" => printer::send_tcp(&target.host, target.port.unwrap_or(9100), &job).await,
        "system" => {
            let name = target.name.clone();
            tauri::async_runtime::spawn_blocking(move || printer::send_to_printer(&name, &job)).await.map_err(|e| e.to_string())?
        }
        _ => Err("choose a receipt printer in Settings".into()),
    }
}

#[tauri::command]
fn list_printers() -> Vec<String> {
    printer::list_printers()
}

#[tauri::command]
async fn print_raster(target: PrinterTarget, width_bytes: usize, height: usize, data: String, cut: bool, drawer: bool) -> CmdResult<()> {
    let bits = base64::engine::general_purpose::STANDARD.decode(data).map_err(|_| "invalid receipt image".to_string())?;
    let job = printer::raster_job(width_bytes, height, &bits, cut, drawer)?;
    send_job(&target, job).await
}

#[tauri::command]
async fn open_drawer(target: PrinterTarget) -> CmdResult<()> {
    send_job(&target, [printer::INIT, printer::DRAWER_KICK].concat()).await
}

// ---------------------------------------------------------------------------------------------

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let store = Arc::new(Store::open(&dir.join("pos.sqlite")).map_err(std::io::Error::other)?);
            let engine = Engine::new(store);
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(engine.clone().run(move |s| {
                let _ = handle.emit("sync-status", s);
            }));
            app.manage(App { engine, pairing: Mutex::new(None) });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_info,
            set_lang,
            save_settings,
            pair_start,
            pair_poll,
            unpair,
            search_products,
            scan_code,
            list_categories,
            create_sale,
            list_sales,
            get_sale,
            void_sale,
            retry_sale,
            sales_summary,
            sync_status,
            sync_now,
            list_bills,
            save_bill,
            delete_bill,
            list_printers,
            print_raster,
            open_drawer,
        ])
        .run(tauri::generate_context!())
        .expect("error while running zaokaiy POS");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_addresses() {
        assert_eq!(normalize_base("https://zaokaiy.com").unwrap(), "https://zaokaiy.com/api");
        assert_eq!(normalize_base("https://zaokaiy.com/api/").unwrap(), "https://zaokaiy.com/api");
        assert!(normalize_base("zaokaiy.com").is_err());
        assert_eq!(web_from_api("https://zaokaiy.com/api"), "https://zaokaiy.com");
    }
}
