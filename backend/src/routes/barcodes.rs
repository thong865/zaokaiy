//! Barcodes for the till: scanner lookup and in-store barcode generation.
//!
//! * `GET /shops/{id}/pos/scan?code=` — exact lookup for a scanned code. Scanners and suppliers
//!   write the same GTIN in different lengths (UPC-A 12 digits vs EAN-13 with a leading 0, GTIN-14
//!   padded with zeros), so all equivalent forms are matched. SKU matches too (case-insensitive).
//! * `POST /shops/{id}/barcodes/generate` — gives products without a barcode an in-store EAN-13
//!   from the GS1 "restricted circulation" range (prefix 20), unique per shop, so they can be
//!   labelled and scanned.

use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{
    auth::AuthUser,
    error::{AppError, AppResult},
    routes::owned_shop,
    AppState,
};

/// GS1 mod-10 check digit for the digits *without* the check digit.
pub fn gtin_check_digit(body: &str) -> Option<u32> {
    if body.is_empty() || !body.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let sum: u32 = body
        .bytes()
        .rev()
        .enumerate()
        .map(|(i, b)| (b - b'0') as u32 * if i % 2 == 0 { 3 } else { 1 })
        .sum();
    Some((10 - sum % 10) % 10)
}

/// True when `code` is an all-digit GTIN (8/12/13/14) with a correct check digit.
pub fn gtin_valid(code: &str) -> bool {
    matches!(code.len(), 8 | 12 | 13 | 14)
        && code.bytes().all(|b| b.is_ascii_digit())
        && gtin_check_digit(&code[..code.len() - 1]) == code[code.len() - 1..].parse().ok()
}

/// Clean what a scanner sent (spaces, control characters, AIM prefix like "]E0") and list the
/// equivalent forms to look up.
pub fn scan_candidates(raw: &str) -> Vec<String> {
    let mut code: String = raw.chars().filter(|c| !c.is_control() && !c.is_whitespace()).collect();
    if code.len() > 3 && code.starts_with(']') {
        code = code[3..].to_string(); // AIM symbology identifier (e.g. "]E0", "]C1")
    }
    if code.is_empty() {
        return Vec::new();
    }
    let mut out = vec![code.clone()];
    let trimmed = code.trim_start_matches('0');
    if code.bytes().all(|b| b.is_ascii_digit()) && !trimmed.is_empty() {
        // Same GTIN at every standard length: GTIN-8/12/13/14 are zero-padded forms of each other.
        for len in [8usize, 12, 13, 14] {
            if trimmed.len() <= len {
                let padded = format!("{:0>len$}", trimmed, len = len);
                if !out.contains(&padded) {
                    out.push(padded);
                }
            }
        }
    }
    out.retain(|c| !c.is_empty() && c.len() <= 64);
    out
}

#[derive(Deserialize)]
pub struct ScanQ {
    pub code: String,
}

/// Exact product for a scanned code (barcode in any equivalent GTIN length, or SKU).
pub async fn scan(State(st): State<AppState>, user: AuthUser, Path(shop_id): Path<Uuid>, Query(q): Query<ScanQ>) -> AppResult<Json<Value>> {
    owned_shop(&st, shop_id, &user).await?;
    let cands = scan_candidates(&q.code);
    let code = cands.first().cloned().unwrap_or_default();
    if code.is_empty() {
        return Err(AppError::bad("empty barcode"));
    }
    type Row = (Uuid, String, String, Option<String>, i64, i32, Value, String, Option<Uuid>, Option<Value>);
    let row: Option<Row> = sqlx::query_as(
        "SELECT id, name, sku, barcode, price_cents, stock, images, status, shop_category_id, cover
         FROM products
         WHERE shop_id = $1 AND status <> 'archived' AND (barcode = ANY($2) OR lower(sku) = lower($3))
         ORDER BY (barcode = $3) DESC, (barcode = ANY($2)) DESC
         LIMIT 1",
    )
    .bind(shop_id)
    .bind(&cands)
    .bind(&code)
    .fetch_optional(&st.db)
    .await?;
    let product = row.map(|(id, name, sku, barcode, price, stock, images, status, cat, cover)| {
        json!({ "id": id, "name": name, "sku": sku, "barcode": barcode, "price_cents": price, "stock": stock,
                "image": images.get(0), "cover": cover, "status": status, "shop_category_id": cat, "exact": true })
    });
    Ok(Json(json!({ "code": code, "valid_gtin": gtin_valid(&code), "product": product })))
}

#[derive(Deserialize)]
pub struct GenerateReq {
    /// Products to give a barcode (those that already have one are skipped). Empty = every
    /// product of the shop without a barcode.
    #[serde(default)]
    pub product_ids: Vec<Uuid>,
}

/// Assign in-store EAN-13 barcodes (20 + 10-digit shop counter + check digit).
pub async fn generate(State(st): State<AppState>, user: AuthUser, Path(shop_id): Path<Uuid>, Json(r): Json<GenerateReq>) -> AppResult<Json<Value>> {
    owned_shop(&st, shop_id, &user).await?;
    if r.product_ids.len() > 1000 {
        return Err(AppError::bad("select at most 1000 products"));
    }
    let mut tx = st.db.begin().await?;
    let todo: Vec<Uuid> = sqlx::query_scalar(
        "SELECT id FROM products WHERE shop_id = $1 AND (barcode IS NULL OR barcode = '')
           AND (cardinality($2::uuid[]) = 0 OR id = ANY($2)) AND status <> 'archived'
         ORDER BY created_at FOR UPDATE",
    )
    .bind(shop_id)
    .bind(&r.product_ids)
    .fetch_all(&mut *tx)
    .await?;
    let mut assigned = Vec::new();
    for pid in todo {
        // Skip numbers already taken (e.g. typed in by hand).
        let code = loop {
            let n: i64 = sqlx::query_scalar(
                "INSERT INTO shop_counters (shop_id, kind, next) VALUES ($1, 'barcode', 2)
                 ON CONFLICT (shop_id, kind) DO UPDATE SET next = shop_counters.next + 1 RETURNING next - 1",
            )
            .bind(shop_id)
            .fetch_one(&mut *tx)
            .await?;
            let body = format!("20{n:010}");
            let code = format!("{body}{}", gtin_check_digit(&body).unwrap_or(0));
            let taken: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM products WHERE shop_id = $1 AND barcode = $2)")
                .bind(shop_id)
                .bind(&code)
                .fetch_one(&mut *tx)
                .await?;
            if !taken {
                break code;
            }
        };
        sqlx::query("UPDATE products SET barcode = $2, updated_at = now() WHERE id = $1")
            .bind(pid)
            .bind(&code)
            .execute(&mut *tx)
            .await?;
        assigned.push(json!({ "id": pid, "barcode": code }));
    }
    tx.commit().await?;
    Ok(Json(json!({ "assigned": assigned })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_digits() {
        assert_eq!(gtin_check_digit("885123400001"), Some(2)); // 8851234000012
        assert!(gtin_valid("8851234000012"));
        assert!(gtin_valid("036000291452")); // UPC-A
        assert!(gtin_valid("96385074")); // EAN-8
        assert!(!gtin_valid("8851234000011"));
        assert!(!gtin_valid("ABC123"));
    }

    #[test]
    fn candidates() {
        let c = scan_candidates(" 036000291452\r\n");
        assert_eq!(c[0], "036000291452");
        assert!(c.contains(&"0036000291452".to_string()) && c.contains(&"00036000291452".to_string()));
        let c = scan_candidates("]E08851234000011");
        assert_eq!(c[0], "8851234000011");
        assert!(c.contains(&"08851234000011".to_string()));
        assert_eq!(scan_candidates("sku-12"), vec!["sku-12".to_string()]);
        assert!(scan_candidates(" \r\n").is_empty());
        assert_eq!(scan_candidates("0000"), vec!["0000".to_string()]);
    }

    #[test]
    fn in_store_codes_are_valid_ean13() {
        let body = format!("20{:010}", 42);
        let code = format!("{body}{}", gtin_check_digit(&body).unwrap());
        assert_eq!(code.len(), 13);
        assert!(gtin_valid(&code));
    }
}
