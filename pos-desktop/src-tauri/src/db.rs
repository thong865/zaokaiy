//! Local SQLite store: the terminal's copy of the catalogue, its sales (the outbox), parked bills
//! and settings. Everything the till does reads and writes here first, so it works offline.

use std::{path::Path, sync::Mutex};

use chrono::{SecondsFormat, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::money::{self, code_key, Line, Payment};

pub type DbResult<T> = Result<T, String>;

fn e(err: rusqlite::Error) -> String {
    format!("local database: {err}")
}

const SCHEMA: &[&str] = &[
    // v1
    "CREATE TABLE kv (key TEXT PRIMARY KEY, value TEXT NOT NULL);
     CREATE TABLE products (
        id TEXT PRIMARY KEY, name TEXT NOT NULL, sku TEXT NOT NULL DEFAULT '', barcode TEXT, code_key TEXT,
        sku_key TEXT, price_cents INTEGER NOT NULL, stock INTEGER NOT NULL, status TEXT NOT NULL,
        category_id TEXT, image TEXT, gen INTEGER NOT NULL DEFAULT 0);
     CREATE INDEX products_code ON products(code_key);
     CREATE INDEX products_sku ON products(sku_key);
     CREATE TABLE categories (id TEXT PRIMARY KEY, parent_id TEXT, name TEXT NOT NULL, position INTEGER NOT NULL DEFAULT 0);
     CREATE TABLE sales (
        id TEXT PRIMARY KEY, shop_id TEXT NOT NULL, device_id TEXT NOT NULL, seq INTEGER NOT NULL,
        number TEXT NOT NULL, created_at TEXT NOT NULL, total_cents INTEGER NOT NULL, payload TEXT NOT NULL,
        status TEXT NOT NULL DEFAULT 'completed',          -- completed | voided
        sync_state TEXT NOT NULL DEFAULT 'pending',        -- pending | synced | rejected
        sync_error TEXT, synced_at TEXT, shortfall TEXT,
        void_reason TEXT, voided_at TEXT,
        void_sync TEXT NOT NULL DEFAULT 'none',            -- none | pending | synced | rejected
        void_error TEXT,
        UNIQUE (device_id, seq));
     CREATE INDEX sales_created ON sales(created_at);
     CREATE INDEX sales_sync ON sales(sync_state);
     CREATE TABLE sale_lines (sale_id TEXT NOT NULL REFERENCES sales(id) ON DELETE CASCADE, product_id TEXT, qty INTEGER NOT NULL);
     CREATE INDEX sale_lines_product ON sale_lines(product_id);
     CREATE TABLE bills (id TEXT PRIMARY KEY, label TEXT NOT NULL DEFAULT '', data TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL);",
];

pub fn now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

pub struct Store {
    conn: Mutex<Connection>,
}

// ----------------------------------------------------------------------------------------------
// Shapes exchanged with the server (pull) and the UI
// ----------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct RemoteProduct {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub sku: String,
    pub barcode: Option<String>,
    pub price_cents: i64,
    pub stock: i64,
    pub status: String,
    pub shop_category_id: Option<String>,
    pub image: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Category {
    pub id: String,
    pub parent_id: Option<String>,
    pub name: String,
    #[serde(default)]
    pub position: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PullPage {
    pub cursor: String,
    pub full: bool,
    pub has_more: bool,
    pub products: Vec<RemoteProduct>,
    #[serde(default)]
    pub deleted: Vec<String>,
    #[serde(default)]
    pub categories: Vec<Category>,
    pub shop: Value,
    pub device: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProductView {
    pub id: String,
    pub name: String,
    pub sku: String,
    pub barcode: Option<String>,
    pub price_cents: i64,
    /// Server stock minus sales not yet synced.
    pub stock: i64,
    pub category_id: Option<String>,
    pub image: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NewSale {
    pub lines: Vec<Line>,
    #[serde(default)]
    pub discount_cents: i64,
    pub payments: Vec<Payment>,
    #[serde(default)]
    pub note: String,
    #[serde(default)]
    pub customer: Option<Value>,
    /// The parked bill this sale settles (deleted with the sale).
    #[serde(default)]
    pub bill_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SaleRecord {
    pub id: String,
    pub number: String,
    pub seq: i64,
    pub created_at: String,
    pub total_cents: i64,
    pub status: String,
    pub sync_state: String,
    pub sync_error: Option<String>,
    pub synced_at: Option<String>,
    pub shortfall: Value,
    pub void_reason: Option<String>,
    pub voided_at: Option<String>,
    pub void_sync: String,
    pub void_error: Option<String>,
    pub sale: Value,
}

/// Result of one pushed item, as returned by the server.
#[derive(Debug, Clone, Deserialize)]
pub struct PushResult {
    #[serde(alias = "sale_id")]
    pub id: String,
    pub status: String,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub shortfall: Option<Value>,
}

const PRODUCT_VIEW: &str = "SELECT p.id, p.name, p.sku, p.barcode, p.price_cents,
        p.stock - COALESCE((SELECT SUM(CASE
            WHEN s.sync_state = 'pending' AND s.status = 'completed' THEN l.qty
            WHEN s.sync_state = 'synced' AND s.status = 'voided' AND s.void_sync = 'pending' THEN -l.qty
            ELSE 0 END)
          FROM sale_lines l JOIN sales s ON s.id = l.sale_id WHERE l.product_id = p.id), 0),
        p.category_id, p.image
     FROM products p";

fn product_row(r: &rusqlite::Row) -> rusqlite::Result<ProductView> {
    Ok(ProductView {
        id: r.get(0)?,
        name: r.get(1)?,
        sku: r.get(2)?,
        barcode: r.get(3)?,
        price_cents: r.get(4)?,
        stock: r.get(5)?,
        category_id: r.get(6)?,
        image: r.get(7)?,
    })
}

const SALE_COLS: &str = "id, number, seq, created_at, total_cents, status, sync_state, sync_error, synced_at, shortfall,
                         void_reason, voided_at, void_sync, void_error, payload";

fn sale_row(r: &rusqlite::Row) -> rusqlite::Result<SaleRecord> {
    let shortfall: Option<String> = r.get(9)?;
    let payload: String = r.get(14)?;
    Ok(SaleRecord {
        id: r.get(0)?,
        number: r.get(1)?,
        seq: r.get(2)?,
        created_at: r.get(3)?,
        total_cents: r.get(4)?,
        status: r.get(5)?,
        sync_state: r.get(6)?,
        sync_error: r.get(7)?,
        synced_at: r.get(8)?,
        shortfall: shortfall.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or(json!([])),
        void_reason: r.get(10)?,
        voided_at: r.get(11)?,
        void_sync: r.get(12)?,
        void_error: r.get(13)?,
        sale: serde_json::from_str(&payload).unwrap_or(Value::Null),
    })
}

impl Store {
    pub fn open(path: &Path) -> DbResult<Self> {
        let conn = Connection::open(path).map_err(e)?;
        Self::init(conn)
    }

    pub fn open_in_memory() -> DbResult<Self> {
        Self::init(Connection::open_in_memory().map_err(e)?)
    }

    fn init(conn: Connection) -> DbResult<Self> {
        conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA synchronous = FULL; PRAGMA foreign_keys = ON;").map_err(e)?;
        let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0)).map_err(e)?;
        for (i, sql) in SCHEMA.iter().enumerate().skip(version as usize) {
            conn.execute_batch(&format!("BEGIN; {sql} PRAGMA user_version = {}; COMMIT;", i + 1)).map_err(e)?;
        }
        Ok(Self { conn: Mutex::new(conn) })
    }

    fn c(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|p| p.into_inner())
    }

    // ------------------------------------------------------------------ kv

    pub fn get(&self, key: &str) -> Option<String> {
        self.c().query_row("SELECT value FROM kv WHERE key = ?1", [key], |r| r.get(0)).optional().ok().flatten()
    }

    pub fn set(&self, key: &str, value: &str) -> DbResult<()> {
        self.c()
            .execute("INSERT INTO kv (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value", params![key, value])
            .map_err(e)?;
        Ok(())
    }

    pub fn del(&self, key: &str) -> DbResult<()> {
        self.c().execute("DELETE FROM kv WHERE key = ?1", [key]).map_err(e)?;
        Ok(())
    }

    pub fn get_json(&self, key: &str) -> Value {
        self.get(key).and_then(|s| serde_json::from_str(&s).ok()).unwrap_or(Value::Null)
    }

    pub fn set_json(&self, key: &str, v: &Value) -> DbResult<()> {
        self.set(key, &v.to_string())
    }

    // ------------------------------------------------------------------ catalogue

    /// Apply one pull page in a single transaction. A full snapshot (first pull) stamps rows with a
    /// new generation and removes older rows only once its last page has arrived, so the till keeps
    /// a usable catalogue even if a full sync is interrupted.
    pub fn apply_pull(&self, page: &PullPage) -> DbResult<()> {
        let mut c = self.c();
        let tx = c.transaction().map_err(e)?;
        let get = |k: &str| -> Option<String> { tx.query_row("SELECT value FROM kv WHERE key = ?1", [k], |r| r.get(0)).optional().ok().flatten() };
        let gen: i64 = if page.full {
            match get("full_gen") {
                Some(g) => g.parse().unwrap_or(1),
                None => get("gen").and_then(|g| g.parse().ok()).unwrap_or(0) + 1,
            }
        } else {
            get("gen").and_then(|g| g.parse().ok()).unwrap_or(0)
        };
        {
            let mut up = tx
                .prepare(
                    "INSERT INTO products (id, name, sku, barcode, code_key, sku_key, price_cents, stock, status, category_id, image, gen)
                     VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)
                     ON CONFLICT(id) DO UPDATE SET name=excluded.name, sku=excluded.sku, barcode=excluded.barcode,
                        code_key=excluded.code_key, sku_key=excluded.sku_key, price_cents=excluded.price_cents,
                        stock=excluded.stock, status=excluded.status, category_id=excluded.category_id,
                        image=excluded.image, gen=excluded.gen",
                )
                .map_err(e)?;
            for p in &page.products {
                let code = p.barcode.as_deref().filter(|b| !b.trim().is_empty()).map(code_key);
                let sku = Some(p.sku.trim()).filter(|s| !s.is_empty()).map(code_key);
                up.execute(params![p.id, p.name, p.sku, p.barcode, code, sku, p.price_cents, p.stock, p.status, p.shop_category_id, p.image, gen])
                    .map_err(e)?;
            }
            let mut del = tx.prepare("DELETE FROM products WHERE id = ?1").map_err(e)?;
            for id in &page.deleted {
                del.execute([id]).map_err(e)?;
            }
        }
        tx.execute("DELETE FROM categories", []).map_err(e)?;
        for cat in &page.categories {
            tx.execute("INSERT INTO categories (id, parent_id, name, position) VALUES (?1,?2,?3,?4)", params![cat.id, cat.parent_id, cat.name, cat.position])
                .map_err(e)?;
        }
        let kv = |k: &str, v: &str| tx.execute("INSERT INTO kv (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value", params![k, v]);
        kv("shop", &page.shop.to_string()).map_err(e)?;
        kv("device", &page.device.to_string()).map_err(e)?;
        kv("cursor", &page.cursor).map_err(e)?;
        if page.full {
            if page.has_more {
                kv("full_gen", &gen.to_string()).map_err(e)?;
            } else {
                tx.execute("DELETE FROM products WHERE gen < ?1", [gen]).map_err(e)?;
                tx.execute("DELETE FROM kv WHERE key = 'full_gen'", []).map_err(e)?;
                kv("gen", &gen.to_string()).map_err(e)?;
            }
        }
        if !page.has_more {
            kv("last_pull_at", &now()).map_err(e)?;
        }
        tx.commit().map_err(e)
    }

    /// Forget the catalogue (switching shops).
    pub fn reset_catalogue(&self) -> DbResult<()> {
        self.c()
            .execute_batch("DELETE FROM products; DELETE FROM categories; DELETE FROM kv WHERE key IN ('cursor','full_gen','gen','shop','device','last_pull_at');")
            .map_err(e)
    }

    pub fn search(&self, q: &str, category_id: Option<&str>, limit: i64) -> DbResult<Vec<ProductView>> {
        let q = q.trim();
        let key = code_key(q);
        let like = format!("%{}%", q.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_"));
        let c = self.c();
        // Category filter includes sub-categories (up to 3 levels).
        let sql = format!(
            "WITH RECURSIVE sub(id) AS (SELECT ?4 UNION ALL SELECT c.id FROM categories c JOIN sub ON c.parent_id = sub.id)
             {PRODUCT_VIEW}
             WHERE p.status <> 'archived'
               AND (?1 = '' OR p.name LIKE ?2 ESCAPE '\\' OR p.sku LIKE ?2 ESCAPE '\\' OR p.code_key = ?3)
               AND (?4 IS NULL OR p.category_id IN (SELECT id FROM sub))
             ORDER BY (p.code_key = ?3 OR p.sku_key = ?3) DESC, p.name COLLATE NOCASE
             LIMIT ?5"
        );
        let mut st = c.prepare(&sql).map_err(e)?;
        let rows = st.query_map(params![q, like, key, category_id, limit.clamp(1, 500)], product_row).map_err(e)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(e)
    }

    /// Exact barcode / SKU match for a scanner.
    pub fn scan(&self, code: &str) -> DbResult<Option<ProductView>> {
        let key = code_key(code);
        if key.is_empty() {
            return Ok(None);
        }
        let c = self.c();
        c.query_row(
            &format!("{PRODUCT_VIEW} WHERE p.status <> 'archived' AND (p.code_key = ?1 OR p.sku_key = ?1) ORDER BY p.code_key = ?1 DESC LIMIT 1"),
            [key],
            product_row,
        )
        .optional()
        .map_err(e)
    }

    pub fn categories(&self) -> DbResult<Vec<Category>> {
        let c = self.c();
        let mut st = c.prepare("SELECT id, parent_id, name, position FROM categories ORDER BY position, name").map_err(e)?;
        let rows = st
            .query_map([], |r| Ok(Category { id: r.get(0)?, parent_id: r.get(1)?, name: r.get(2)?, position: r.get(3)? }))
            .map_err(e)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(e)
    }

    pub fn product_count(&self) -> i64 {
        self.c().query_row("SELECT COUNT(*) FROM products WHERE status <> 'archived'", [], |r| r.get(0)).unwrap_or(0)
    }

    // ------------------------------------------------------------------ sales

    /// Record a sale locally (final receipt number included) and queue it for upload.
    pub fn create_sale(&self, input: &NewSale, shop: &Value, device: &Value, print: Value) -> DbResult<SaleRecord> {
        let shop_id = shop["id"].as_str().ok_or("this terminal is not paired with a shop")?.to_string();
        let device_id = device["id"].as_str().ok_or("this terminal is not paired with a shop")?.to_string();
        let code = device["code"].as_str().unwrap_or("T00").to_string();
        let vat_bps = shop["vat_bps"].as_i64().unwrap_or(0) as i32;
        let inclusive = shop["prices_include_vat"].as_bool().unwrap_or(true);
        let prefix = shop["receipt_prefix"].as_str().unwrap_or("R").to_string();
        let currency = shop["currency"].as_str().unwrap_or("LAK").to_string();

        let lines: Vec<Line> = input
            .lines
            .iter()
            .map(|l| Line { name: l.name.trim().chars().take(120).collect(), ..l.clone() })
            .collect();
        let totals = money::compute(&lines, input.discount_cents, &input.payments, vat_bps, inclusive)?;

        let mut c = self.c();
        let tx = c.transaction().map_err(e)?;
        // Next number of this terminal: after anything sold here or known to the server.
        let local_max: i64 = tx.query_row("SELECT COALESCE(MAX(seq), 0) FROM sales WHERE device_id = ?1", [&device_id], |r| r.get(0)).map_err(e)?;
        let server_max = device["last_seq"].as_i64().unwrap_or(0);
        let seq = local_max.max(server_max) + 1;
        let number = format!("{prefix}{code}-{seq:06}");
        let id = uuid::Uuid::new_v4().to_string();
        let created_at = now();
        let sale = json!({
            "id": id, "seq": seq, "number": number, "created_at": created_at,
            "items": lines, "discount_cents": totals.discount_cents,
            "vat_bps": vat_bps, "prices_include_vat": inclusive, "currency": currency,
            "subtotal_cents": totals.subtotal_cents, "vat_cents": totals.vat_cents, "total_cents": totals.total_cents,
            "paid_cents": totals.paid_cents, "change_cents": totals.change_cents,
            "payments": input.payments, "note": input.note.trim().chars().take(500).collect::<String>(),
            "customer": input.customer,
            // Not used by the server: a snapshot for reprinting this receipt exactly as issued.
            "print": print,
        });
        tx.execute(
            "INSERT INTO sales (id, shop_id, device_id, seq, number, created_at, total_cents, payload) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
            params![id, shop_id, device_id, seq, number, created_at, totals.total_cents, sale.to_string()],
        )
        .map_err(e)?;
        for l in &lines {
            tx.execute("INSERT INTO sale_lines (sale_id, product_id, qty) VALUES (?1,?2,?3)", params![id, l.product_id, l.qty]).map_err(e)?;
        }
        if let Some(bill) = &input.bill_id {
            tx.execute("DELETE FROM bills WHERE id = ?1", [bill]).map_err(e)?;
        }
        tx.commit().map_err(e)?;
        drop(c);
        self.get_sale(&id)?.ok_or_else(|| "sale not saved".into())
    }

    pub fn get_sale(&self, id: &str) -> DbResult<Option<SaleRecord>> {
        self.c().query_row(&format!("SELECT {SALE_COLS} FROM sales WHERE id = ?1"), [id], sale_row).optional().map_err(e)
    }

    /// Sales between two instants (ISO strings), newest first; `q` matches the receipt number.
    pub fn list_sales(&self, from: Option<&str>, to: Option<&str>, q: Option<&str>, state: Option<&str>, limit: i64) -> DbResult<Vec<SaleRecord>> {
        let c = self.c();
        let mut st = c
            .prepare(&format!(
                "SELECT {SALE_COLS} FROM sales
                 WHERE (?1 IS NULL OR created_at >= ?1) AND (?2 IS NULL OR created_at < ?2)
                   AND (?3 IS NULL OR number LIKE '%' || ?3 || '%')
                   AND (?4 IS NULL OR sync_state = ?4 OR (?4 = 'rejected' AND void_sync = 'rejected')
                        OR (?4 = 'pending' AND void_sync = 'pending'))
                 ORDER BY created_at DESC LIMIT ?5"
            ))
            .map_err(e)?;
        let q = q.map(str::trim).filter(|s| !s.is_empty());
        let rows = st.query_map(params![from, to, q, state, limit.clamp(1, 2000)], sale_row).map_err(e)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(e)
    }

    pub fn void_sale(&self, id: &str, reason: &str) -> DbResult<SaleRecord> {
        let reason: String = reason.trim().chars().take(300).collect();
        if reason.is_empty() {
            return Err("give a reason for voiding".into());
        }
        let n = self
            .c()
            .execute(
                "UPDATE sales SET status = 'voided', void_reason = ?2, voided_at = ?3, void_sync = 'pending', void_error = NULL
                 WHERE id = ?1 AND status = 'completed'",
                params![id, reason, now()],
            )
            .map_err(e)?;
        if n == 0 {
            return Err("sale is already voided".into());
        }
        self.get_sale(id)?.ok_or_else(|| "sale not found".into())
    }

    /// Put a rejected sale / void back in the queue (e.g. after the server was fixed).
    pub fn retry(&self, id: &str) -> DbResult<()> {
        self.c()
            .execute(
                "UPDATE sales SET sync_state = CASE WHEN sync_state = 'rejected' THEN 'pending' ELSE sync_state END,
                                  void_sync  = CASE WHEN void_sync  = 'rejected' THEN 'pending' ELSE void_sync END
                 WHERE id = ?1",
                [id],
            )
            .map_err(e)?;
        Ok(())
    }

    /// The next upload batch for a shop: pending sales (oldest first) and voids whose sale is on the
    /// server already or in this batch (the server applies sales before voids).
    pub fn outbox(&self, shop_id: &str, limit: i64) -> DbResult<(Vec<Value>, Vec<Value>)> {
        let c = self.c();
        let mut st = c
            .prepare("SELECT id, payload FROM sales WHERE shop_id = ?1 AND sync_state = 'pending' ORDER BY created_at, seq LIMIT ?2")
            .map_err(e)?;
        let sales: Vec<(String, String)> = st.query_map(params![shop_id, limit], |r| Ok((r.get(0)?, r.get(1)?))).map_err(e)?.collect::<Result<_, _>>().map_err(e)?;
        let ids: Vec<&str> = sales.iter().map(|s| s.0.as_str()).collect();
        let mut st = c
            .prepare("SELECT id, sync_state, void_reason, voided_at FROM sales WHERE shop_id = ?1 AND void_sync = 'pending' ORDER BY voided_at LIMIT ?2")
            .map_err(e)?;
        let voids: Vec<Value> = st
            .query_map(params![shop_id, limit], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, Option<String>>(2)?, r.get::<_, Option<String>>(3)?)))
            .map_err(e)?
            .filter_map(Result::ok)
            .filter(|(id, state, _, _)| state == "synced" || ids.contains(&id.as_str()))
            .map(|(id, _, reason, at)| json!({ "sale_id": id, "reason": reason, "voided_at": at }))
            .collect();
        let sales = sales.into_iter().filter_map(|(_, p)| serde_json::from_str(&p).ok()).collect();
        Ok((sales, voids))
    }

    pub fn apply_push_results(&self, sales: &[PushResult], voids: &[PushResult]) -> DbResult<()> {
        let mut c = self.c();
        let tx = c.transaction().map_err(e)?;
        let at = now();
        for r in sales {
            match r.status.as_str() {
                "ok" | "duplicate" => {
                    let sf = r.shortfall.as_ref().filter(|v| v.as_array().is_some_and(|a| !a.is_empty())).map(|v| v.to_string());
                    tx.execute(
                        "UPDATE sales SET sync_state = 'synced', sync_error = NULL, synced_at = ?2, shortfall = COALESCE(?3, shortfall) WHERE id = ?1",
                        params![r.id, at, sf],
                    )
                    .map_err(e)?;
                }
                _ => {
                    tx.execute("UPDATE sales SET sync_state = 'rejected', sync_error = ?2 WHERE id = ?1", params![r.id, r.error]).map_err(e)?;
                }
            }
        }
        for r in voids {
            let (state, err) = if r.status == "ok" || r.status == "duplicate" { ("synced", None) } else { ("rejected", r.error.clone()) };
            tx.execute("UPDATE sales SET void_sync = ?2, void_error = ?3 WHERE id = ?1", params![r.id, state, err]).map_err(e)?;
        }
        tx.commit().map_err(e)
    }

    /// (pending uploads, rejected items) for the current shop.
    pub fn queue_counts(&self, shop_id: &str) -> (i64, i64) {
        self.c()
            .query_row(
                "SELECT COALESCE(SUM(sync_state = 'pending') + SUM(void_sync = 'pending'), 0),
                        COALESCE(SUM(sync_state = 'rejected') + SUM(void_sync = 'rejected'), 0)
                 FROM sales WHERE shop_id = ?1",
                [shop_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap_or((0, 0))
    }

    /// Sales of any shop still waiting for upload (blocks switching shops).
    pub fn unsynced_total(&self) -> i64 {
        self.c()
            .query_row("SELECT COUNT(*) FROM sales WHERE sync_state = 'pending' OR void_sync = 'pending'", [], |r| r.get(0))
            .unwrap_or(0)
    }

    /// Takings between two instants: per payment method (cash net of change), voids, VAT, top items.
    pub fn summary(&self, from: &str, to: &str) -> DbResult<Value> {
        let sales = self.list_sales(Some(from), Some(to), None, None, 2000)?;
        let mut methods: std::collections::BTreeMap<String, i64> = Default::default();
        let mut items: std::collections::HashMap<String, (i64, i64)> = Default::default();
        let (mut count, mut total, mut vat, mut voided, mut voided_total) = (0, 0i64, 0i64, 0, 0i64);
        for s in &sales {
            if s.status == "voided" {
                voided += 1;
                voided_total += s.total_cents;
                continue;
            }
            count += 1;
            total += s.total_cents;
            vat += s.sale["vat_cents"].as_i64().unwrap_or(0);
            let change = s.sale["change_cents"].as_i64().unwrap_or(0);
            for p in s.sale["payments"].as_array().into_iter().flatten() {
                let m = p["method"].as_str().unwrap_or("other").to_string();
                *methods.entry(m).or_default() += p["amount_cents"].as_i64().unwrap_or(0);
            }
            *methods.entry("cash".into()).or_default() -= change;
            for l in s.sale["items"].as_array().into_iter().flatten() {
                let name = l["name"].as_str().unwrap_or("").to_string();
                let e = items.entry(name).or_default();
                e.0 += l["qty"].as_i64().unwrap_or(0);
                e.1 += l["unit_price_cents"].as_i64().unwrap_or(0) * l["qty"].as_i64().unwrap_or(0) - l["discount_cents"].as_i64().unwrap_or(0);
            }
        }
        methods.retain(|_, v| *v != 0);
        let mut top: Vec<_> = items.into_iter().collect();
        top.sort_by(|a, b| b.1 .1.cmp(&a.1 .1));
        top.truncate(10);
        Ok(json!({
            "count": count, "total_cents": total, "vat_cents": vat, "voided": voided, "voided_cents": voided_total,
            "methods": methods,
            "top": top.into_iter().map(|(name, (qty, amount))| json!({ "name": name, "qty": qty, "amount_cents": amount })).collect::<Vec<_>>(),
        }))
    }

    // ------------------------------------------------------------------ parked bills

    pub fn bills(&self) -> DbResult<Vec<Value>> {
        let c = self.c();
        let mut st = c.prepare("SELECT id, label, data, created_at, updated_at FROM bills ORDER BY created_at").map_err(e)?;
        let rows = st
            .query_map([], |r| {
                let data: String = r.get(2)?;
                Ok(json!({ "id": r.get::<_, String>(0)?, "label": r.get::<_, String>(1)?,
                           "data": serde_json::from_str::<Value>(&data).unwrap_or(Value::Null),
                           "created_at": r.get::<_, String>(3)?, "updated_at": r.get::<_, String>(4)? }))
            })
            .map_err(e)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(e)
    }

    pub fn save_bill(&self, id: &str, label: &str, data: &Value) -> DbResult<()> {
        let at = now();
        self.c()
            .execute(
                "INSERT INTO bills (id, label, data, created_at, updated_at) VALUES (?1,?2,?3,?4,?4)
                 ON CONFLICT(id) DO UPDATE SET label = excluded.label, data = excluded.data, updated_at = excluded.updated_at",
                params![id, label.trim().chars().take(60).collect::<String>(), data.to_string(), at],
            )
            .map_err(e)?;
        Ok(())
    }

    pub fn delete_bill(&self, id: &str) -> DbResult<()> {
        self.c().execute("DELETE FROM bills WHERE id = ?1", [id]).map_err(e)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(full: bool, has_more: bool, products: Vec<(&str, i64)>, deleted: Vec<&str>, last_seq: i64) -> PullPage {
        PullPage {
            cursor: "c:1".into(),
            full,
            has_more,
            products: products
                .into_iter()
                .map(|(id, stock)| RemoteProduct {
                    id: id.into(),
                    name: format!("Product {id}"),
                    sku: format!("SKU-{id}"),
                    barcode: Some(format!("08859099508{:02}", id.len())),
                    price_cents: 1000,
                    stock,
                    status: "active".into(),
                    shop_category_id: None,
                    image: None,
                })
                .collect(),
            deleted: deleted.into_iter().map(String::from).collect(),
            categories: vec![],
            shop: json!({ "id": "shop1", "vat_bps": 700, "prices_include_vat": true, "receipt_prefix": "R", "currency": "LAK" }),
            device: json!({ "id": "dev1", "code": "T01", "last_seq": last_seq }),
        }
    }

    fn sell(db: &Store, pid: &str, qty: i32) -> SaleRecord {
        let input = NewSale {
            lines: vec![Line { product_id: Some(pid.into()), name: "P".into(), sku: String::new(), qty, unit_price_cents: 1000, discount_cents: 0 }],
            discount_cents: 0,
            payments: vec![Payment { method: "cash".into(), amount_cents: 1000 * qty as i64, reference: String::new() }],
            note: String::new(),
            customer: None,
            bill_id: None,
        };
        db.create_sale(&input, &db.get_json("shop"), &db.get_json("device"), Value::Null).unwrap()
    }

    #[test]
    fn full_snapshot_replaces_catalogue_only_when_complete() {
        let db = Store::open_in_memory().unwrap();
        db.apply_pull(&page(true, false, vec![("a", 5), ("old", 1)], vec![], 0)).unwrap();
        assert_eq!(db.product_count(), 2);
        // a new full sync in two pages: "old" survives until the last page
        db.apply_pull(&page(true, true, vec![("a", 5)], vec![], 0)).unwrap();
        assert_eq!(db.product_count(), 2);
        db.apply_pull(&page(true, false, vec![("b", 3)], vec![], 0)).unwrap();
        let ids: Vec<String> = db.search("", None, 50).unwrap().into_iter().map(|p| p.id).collect();
        assert_eq!(ids, vec!["a", "b"]);
        // incremental: deletions
        db.apply_pull(&page(false, false, vec![], vec!["b"], 0)).unwrap();
        assert_eq!(db.product_count(), 1);
    }

    #[test]
    fn offline_sales_reduce_visible_stock_until_synced() {
        let db = Store::open_in_memory().unwrap();
        db.apply_pull(&page(true, false, vec![("a", 5)], vec![], 7)).unwrap();
        let s = sell(&db, "a", 2);
        assert_eq!(s.number, "RT01-000008"); // continues after the server's last receipt
        assert_eq!(db.scan("00885909950801").unwrap().unwrap().stock, 3); // EAN-13 / GTIN-14 forms match
        let (sales, voids) = db.outbox("shop1", 100).unwrap();
        assert_eq!((sales.len(), voids.len()), (1, 0));
        assert_eq!(sales[0]["total_cents"], 2000);
        // the server accepted it and now reports stock 3 → no double counting
        db.apply_push_results(&[PushResult { id: s.id.clone(), status: "ok".into(), error: None, shortfall: Some(json!([])) }], &[]).unwrap();
        db.apply_pull(&page(false, false, vec![("a", 3)], vec![], 8)).unwrap();
        assert_eq!(db.scan("SKU-a").unwrap().unwrap().stock, 3);
        // void after sync: stock back locally until the void is uploaded
        db.void_sale(&s.id, "wrong item").unwrap();
        assert_eq!(db.scan("SKU-a").unwrap().unwrap().stock, 5);
        let (sales, voids) = db.outbox("shop1", 100).unwrap();
        assert_eq!((sales.len(), voids.len()), (0, 1));
        assert_eq!(db.queue_counts("shop1"), (1, 0));
        let s2 = sell(&db, "a", 1);
        assert_eq!(s2.seq, 9);
    }

    #[test]
    fn void_before_sync_is_sent_with_the_sale() {
        let db = Store::open_in_memory().unwrap();
        db.apply_pull(&page(true, false, vec![("a", 5)], vec![], 0)).unwrap();
        let s = sell(&db, "a", 1);
        db.void_sale(&s.id, "test").unwrap();
        assert_eq!(db.scan("SKU-a").unwrap().unwrap().stock, 5);
        let (sales, voids) = db.outbox("shop1", 100).unwrap();
        assert_eq!((sales.len(), voids.len()), (1, 1));
        db.apply_push_results(
            &[PushResult { id: s.id.clone(), status: "rejected".into(), error: Some("boom".into()), shortfall: None }],
            &[],
        )
        .unwrap();
        assert_eq!(db.queue_counts("shop1"), (1, 1));
        db.retry(&s.id).unwrap();
        assert_eq!(db.queue_counts("shop1"), (2, 0));
    }
}
