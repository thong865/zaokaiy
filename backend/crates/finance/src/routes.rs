use std::collections::BTreeMap;

use axum::{
    extract::{Path, Query, State},
    http::header,
    response::IntoResponse,
    Json,
};
use chrono::{Datelike, Months, NaiveDate, Utc};
use kernel::guards::owned_shop;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::FromRow;
use uuid::Uuid;

use crate::{
    auth::{AdminUser, AuthUser},
    calc::compute,
    error::{AppError, AppResult},
    AppState,
};

// ---------------------------------------------------------------------------------------------
// Settings + policy
// ---------------------------------------------------------------------------------------------

#[derive(Serialize, FromRow, Clone)]
pub struct Settings {
    pub vat_bps: i32,
    pub ecommerce_tax_bps: i32,
    pub due_day: i32,
    pub tax_office: String,
    pub agent_name: String,
    pub agent_tax_id: String,
    pub policy_version: String,
    pub policy_en: String,
    pub policy_lo: String,
    pub updated_at: chrono::DateTime<Utc>,
}

async fn settings(st: &AppState) -> AppResult<Settings> {
    Ok(sqlx::query_as("SELECT * FROM finance.settings WHERE id = 1").fetch_one(&st.db).await?)
}

/// Public: the tax-agent policy and current rates.
pub async fn policy(State(st): State<AppState>) -> AppResult<Json<Settings>> {
    Ok(Json(settings(&st).await?))
}

pub async fn admin_settings(State(st): State<AppState>, _a: AdminUser) -> AppResult<Json<Settings>> {
    Ok(Json(settings(&st).await?))
}

#[derive(Deserialize)]
pub struct SettingsReq {
    pub vat_bps: i32,
    pub ecommerce_tax_bps: i32,
    pub due_day: i32,
    #[serde(default)]
    pub tax_office: String,
    #[serde(default)]
    pub agent_name: String,
    #[serde(default)]
    pub agent_tax_id: String,
    pub policy_en: String,
    pub policy_lo: String,
}

pub async fn admin_save_settings(State(st): State<AppState>, a: AdminUser, Json(r): Json<SettingsReq>) -> AppResult<Json<Settings>> {
    if !(0..=5000).contains(&r.vat_bps) || !(0..=5000).contains(&r.ecommerce_tax_bps) {
        return Err(AppError::bad("tax rates must be between 0% and 50%"));
    }
    if !(1..=28).contains(&r.due_day) {
        return Err(AppError::bad("due day must be between 1 and 28"));
    }
    if r.policy_en.trim().is_empty() {
        return Err(AppError::bad("the policy text can't be empty"));
    }
    let before = settings(&st).await?;
    // A new policy text gets a new version; shops accepted the old one and are asked to re-accept.
    let changed = before.policy_en.trim() != r.policy_en.trim() || before.policy_lo.trim() != r.policy_lo.trim();
    let version = if changed { Utc::now().format("%Y-%m-%d.%H%M").to_string() } else { before.policy_version };
    sqlx::query(
        "UPDATE finance.settings SET vat_bps = $1, ecommerce_tax_bps = $2, due_day = $3, tax_office = $4, agent_name = $5,
            agent_tax_id = $6, policy_en = $7, policy_lo = $8, policy_version = $9, updated_at = now() WHERE id = 1",
    )
    .bind(r.vat_bps)
    .bind(r.ecommerce_tax_bps)
    .bind(r.due_day)
    .bind(r.tax_office.trim())
    .bind(r.agent_name.trim())
    .bind(r.agent_tax_id.trim())
    .bind(r.policy_en.trim())
    .bind(r.policy_lo.trim())
    .bind(&version)
    .execute(&st.db)
    .await?;
    event(&st, None, None, a.0.id, "settings_updated", &format!("policy {version}")).await?;
    Ok(Json(settings(&st).await?))
}

async fn event(st: &AppState, shop: Option<Uuid>, filing: Option<Uuid>, actor: Uuid, action: &str, note: &str) -> AppResult<()> {
    sqlx::query("INSERT INTO finance.events (shop_id, filing_id, actor_id, action, note) VALUES ($1,$2,$3,$4,$5)")
        .bind(shop)
        .bind(filing)
        .bind(actor)
        .bind(action)
        .bind(note)
        .execute(&st.db)
        .await?;
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// Taxable sales from every core
// ---------------------------------------------------------------------------------------------

/// Local "today" (Laos, UTC+7).
fn today() -> NaiveDate {
    (Utc::now() + chrono::Duration::hours(7)).date_naive()
}

fn month_start(d: NaiveDate) -> NaiveDate {
    d.with_day(1).expect("day 1")
}

fn next_month(d: NaiveDate) -> NaiveDate {
    d + Months::new(1)
}

fn parse_period(s: &str) -> AppResult<NaiveDate> {
    NaiveDate::parse_from_str(&format!("{}-01", s.trim()), "%Y-%m-%d").map_err(|_| AppError::bad("period must look like 2026-09"))
}

#[derive(Serialize, Clone)]
pub struct SourceLine {
    pub key: &'static str,
    pub label: &'static str,
    pub gross_cents: i64,
    pub count: i64,
}

/// (shop, currency) → per-source totals, for sales in [from, to).
async fn gather(st: &AppState, from: NaiveDate, to: NaiveDate, shop: Option<Uuid>) -> AppResult<BTreeMap<(Uuid, String), Vec<SourceLine>>> {
    let mut out: BTreeMap<(Uuid, String), Vec<SourceLine>> = BTreeMap::new();
    for src in &st.reg.tax_sources {
        for l in src.taxable(st, from, to, shop).await? {
            if l.gross_cents == 0 && l.count == 0 {
                continue;
            }
            out.entry((l.shop_id, l.currency.clone())).or_default().push(SourceLine {
                key: src.key(),
                label: src.label(),
                gross_cents: l.gross_cents,
                count: l.count,
            });
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------------------------
// Seller: mandate + statements
// ---------------------------------------------------------------------------------------------

#[derive(Serialize, FromRow)]
pub struct Mandate {
    pub status: String,
    pub policy_version: String,
    pub signer_name: String,
    pub signer_title: String,
    pub tax_id: String,
    pub vat_registered: bool,
    pub accepted_at: chrono::DateTime<Utc>,
    pub revoked_at: Option<chrono::DateTime<Utc>>,
    pub revoke_reason: String,
}

#[derive(Serialize, FromRow)]
pub struct Filing {
    pub id: Uuid,
    pub period: NaiveDate,
    pub shop_id: Uuid,
    pub currency: String,
    pub sources: Value,
    pub gross_cents: i64,
    pub vat_bps: i32,
    pub ecommerce_tax_bps: i32,
    pub vat_registered: bool,
    pub net_cents: i64,
    pub vat_cents: i64,
    pub ecommerce_tax_cents: i64,
    pub total_due_cents: i64,
    pub due_on: NaiveDate,
    pub status: String,
    pub reference_no: String,
    pub note: String,
    pub computed_at: chrono::DateTime<Utc>,
    pub submitted_at: Option<chrono::DateTime<Utc>>,
    pub paid_at: Option<chrono::DateTime<Utc>>,
}

async fn mandate(st: &AppState, shop_id: Uuid) -> AppResult<Option<Mandate>> {
    Ok(sqlx::query_as("SELECT * FROM finance.mandates WHERE shop_id = $1").bind(shop_id).fetch_optional(&st.db).await?)
}

pub async fn shop_overview(State(st): State<AppState>, user: AuthUser, Path(id): Path<Uuid>) -> AppResult<Json<Value>> {
    let shop = owned_shop(&st, id, &user).await?;
    let s = settings(&st).await?;
    let m = mandate(&st, id).await?;
    let filings: Vec<Filing> = sqlx::query_as("SELECT * FROM finance.filings WHERE shop_id = $1 AND status <> 'void' ORDER BY period DESC, currency LIMIT 36")
        .bind(id)
        .fetch_all(&st.db)
        .await?;
    // Live estimate for the month so far.
    let from = month_start(today());
    let vat_registered = m.as_ref().map(|m| m.vat_registered).unwrap_or(shop.vat_bps > 0);
    let preview: Vec<Value> = gather(&st, from, next_month(from), Some(id))
        .await?
        .into_iter()
        .map(|((_, currency), sources)| {
            let gross: i64 = sources.iter().map(|x| x.gross_cents).sum();
            json!({ "currency": currency, "sources": sources, "taxes": compute(gross, vat_registered, s.vat_bps, s.ecommerce_tax_bps) })
        })
        .collect();
    let outdated = m.as_ref().is_some_and(|m| m.status == "active" && m.policy_version != s.policy_version);
    Ok(Json(json!({
        "shop": { "id": shop.id, "name": shop.name, "legal_name": shop.legal_name, "tax_id": shop.tax_id,
                  "currency": shop.currency, "entity_type": shop.entity_type, "kyb_verified": shop.kyb_verified_at.is_some() },
        "settings": s,
        "mandate": m,
        "policy_outdated": outdated,
        "preview": { "period": from, "vat_registered": vat_registered, "lines": preview },
        "filings": filings,
    })))
}

#[derive(Deserialize)]
pub struct AcceptReq {
    pub accept: bool,
    pub policy_version: String,
    pub signer_name: String,
    #[serde(default)]
    pub signer_title: String,
    pub vat_registered: bool,
}

pub async fn accept_mandate(State(st): State<AppState>, user: AuthUser, Path(id): Path<Uuid>, Json(r): Json<AcceptReq>) -> AppResult<Json<Value>> {
    let shop = owned_shop(&st, id, &user).await?;
    let s = settings(&st).await?;
    if !r.accept {
        return Err(AppError::bad("you must accept the tax-agent policy"));
    }
    if r.policy_version != s.policy_version {
        return Err(AppError::Conflict("the policy has changed — read it again before accepting".into()));
    }
    let signer = r.signer_name.trim();
    if signer.is_empty() || signer.chars().count() > 120 {
        return Err(AppError::bad("enter the full name of the person signing"));
    }
    if shop.tax_id.trim().is_empty() {
        return Err(AppError::bad("add your tax ID in shop settings first"));
    }
    if shop.entity_type == "business" && shop.kyb_verified_at.is_none() {
        return Err(AppError::bad("verify your business before appointing a tax agent"));
    }
    sqlx::query(
        "INSERT INTO finance.mandates (shop_id, status, policy_version, signer_name, signer_title, tax_id, vat_registered, accepted_by, accepted_at)
         VALUES ($1, 'active', $2, $3, $4, $5, $6, $7, now())
         ON CONFLICT (shop_id) DO UPDATE SET status = 'active', policy_version = $2, signer_name = $3, signer_title = $4,
            tax_id = $5, vat_registered = $6, accepted_by = $7, accepted_at = now(), revoked_at = NULL, revoke_reason = ''",
    )
    .bind(id)
    .bind(&s.policy_version)
    .bind(signer)
    .bind(r.signer_title.trim())
    .bind(shop.tax_id.trim())
    .bind(r.vat_registered)
    .bind(user.id)
    .execute(&st.db)
    .await?;
    event(&st, Some(id), None, user.id, "mandate_accepted", &format!("policy {} · {signer}", s.policy_version)).await?;
    shop_overview(State(st), user, Path(id)).await
}

#[derive(Deserialize)]
pub struct RevokeReq {
    #[serde(default)]
    pub reason: String,
}

pub async fn revoke_mandate(State(st): State<AppState>, user: AuthUser, Path(id): Path<Uuid>, Json(r): Json<RevokeReq>) -> AppResult<Json<Value>> {
    owned_shop(&st, id, &user).await?;
    let n = sqlx::query("UPDATE finance.mandates SET status = 'revoked', revoked_at = now(), revoke_reason = $2 WHERE shop_id = $1 AND status = 'active'")
        .bind(id)
        .bind(r.reason.trim())
        .execute(&st.db)
        .await?
        .rows_affected();
    if n == 0 {
        return Err(AppError::bad("there is no active tax-agent mandate"));
    }
    event(&st, Some(id), None, user.id, "mandate_revoked", r.reason.trim()).await?;
    shop_overview(State(st), user, Path(id)).await
}

// ---------------------------------------------------------------------------------------------
// Admin: monthly filings
// ---------------------------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct PeriodQ {
    pub period: Option<String>,
}

fn period_or_last(q: &PeriodQ) -> AppResult<NaiveDate> {
    match q.period.as_deref().filter(|p| !p.trim().is_empty()) {
        Some(p) => parse_period(p),
        None => Ok(month_start(today()) - Months::new(1)),
    }
}

#[derive(Serialize, FromRow)]
pub struct AdminFiling {
    #[sqlx(flatten)]
    #[serde(flatten)]
    pub filing: Filing,
    pub shop_name: String,
    pub shop_slug: String,
    pub legal_name: String,
    pub tax_id: String,
}

pub async fn admin_filings(State(st): State<AppState>, _a: AdminUser, Query(q): Query<PeriodQ>) -> AppResult<Json<Value>> {
    let period = period_or_last(&q)?;
    let rows: Vec<AdminFiling> = sqlx::query_as(
        "SELECT f.*, s.name AS shop_name, s.slug AS shop_slug, s.legal_name, s.tax_id
         FROM finance.filings f JOIN shops s ON s.id = f.shop_id
         WHERE f.period = $1 ORDER BY s.name, f.currency",
    )
    .bind(period)
    .fetch_all(&st.db)
    .await?;
    let mut totals: BTreeMap<String, (i64, i64, i64, i64)> = BTreeMap::new();
    for r in rows.iter().filter(|r| r.filing.status != "void") {
        let t = totals.entry(r.filing.currency.clone()).or_default();
        t.0 += r.filing.gross_cents;
        t.1 += r.filing.vat_cents;
        t.2 += r.filing.ecommerce_tax_cents;
        t.3 += r.filing.total_due_cents;
    }
    let (mandated,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM finance.mandates WHERE status = 'active'").fetch_one(&st.db).await?;
    let can_generate = next_month(period) <= month_start(today());
    Ok(Json(json!({
        "period": period,
        "can_generate": can_generate,
        "mandated_shops": mandated,
        "totals": totals.into_iter().map(|(c, (g, v, e, t))| json!({ "currency": c, "gross_cents": g, "vat_cents": v, "ecommerce_tax_cents": e, "total_due_cents": t })).collect::<Vec<_>>(),
        "filings": rows,
    })))
}

#[derive(Deserialize)]
pub struct GenerateReq {
    pub period: String,
}

/// Compute (or recompute) draft filings for every shop with an active mandate.
/// Submitted, paid and void filings are never touched.
pub async fn admin_generate(State(st): State<AppState>, a: AdminUser, Json(r): Json<GenerateReq>) -> AppResult<Json<Value>> {
    let period = parse_period(&r.period)?;
    let to = next_month(period);
    if to > month_start(today()) {
        return Err(AppError::bad("you can only file a month that has ended"));
    }
    let s = settings(&st).await?;
    let due_on = to.with_day(s.due_day as u32).unwrap_or(to);
    let mandates: Vec<(Uuid, bool, String)> = sqlx::query_as(
        "SELECT m.shop_id, m.vat_registered, s.currency FROM finance.mandates m JOIN shops s ON s.id = m.shop_id WHERE m.status = 'active'",
    )
    .fetch_all(&st.db)
    .await?;
    let sales = gather(&st, period, to, None).await?;
    let (mut written, mut locked) = (0, 0);
    for (shop_id, vat_registered, shop_currency) in mandates {
        // One filing per currency sold in; a nil return in the shop's currency when nothing sold.
        let mut lines: Vec<(String, Vec<SourceLine>)> =
            sales.iter().filter(|((sid, _), _)| *sid == shop_id).map(|((_, c), v)| (c.clone(), v.clone())).collect();
        if lines.is_empty() {
            lines.push((shop_currency, vec![]));
        }
        for (currency, sources) in lines {
            let gross: i64 = sources.iter().map(|x| x.gross_cents).sum();
            let t = compute(gross, vat_registered, s.vat_bps, s.ecommerce_tax_bps);
            let n = sqlx::query(
                "INSERT INTO finance.filings (period, shop_id, currency, sources, gross_cents, vat_bps, ecommerce_tax_bps, vat_registered,
                    net_cents, vat_cents, ecommerce_tax_cents, total_due_cents, due_on, computed_at)
                 VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13, now())
                 ON CONFLICT (period, shop_id, currency) DO UPDATE SET
                    sources = EXCLUDED.sources, gross_cents = EXCLUDED.gross_cents, vat_bps = EXCLUDED.vat_bps,
                    ecommerce_tax_bps = EXCLUDED.ecommerce_tax_bps, vat_registered = EXCLUDED.vat_registered,
                    net_cents = EXCLUDED.net_cents, vat_cents = EXCLUDED.vat_cents, ecommerce_tax_cents = EXCLUDED.ecommerce_tax_cents,
                    total_due_cents = EXCLUDED.total_due_cents, due_on = EXCLUDED.due_on, computed_at = now()
                 WHERE finance.filings.status = 'draft'",
            )
            .bind(period)
            .bind(shop_id)
            .bind(&currency)
            .bind(json!(sources))
            .bind(t.gross_cents)
            .bind(s.vat_bps)
            .bind(s.ecommerce_tax_bps)
            .bind(vat_registered)
            .bind(t.net_cents)
            .bind(t.vat_cents)
            .bind(t.ecommerce_tax_cents)
            .bind(t.total_due_cents)
            .bind(due_on)
            .execute(&st.db)
            .await?
            .rows_affected();
            if n == 1 {
                written += 1
            } else {
                locked += 1
            }
        }
    }
    event(&st, None, None, a.0.id, "filings_generated", &format!("{} · {written} drafts", period.format("%Y-%m"))).await?;
    let mut out = admin_filings(State(st), a, Query(PeriodQ { period: Some(period.format("%Y-%m").to_string()) })).await?.0;
    out["generated"] = json!({ "drafts": written, "locked": locked });
    Ok(Json(out))
}

#[derive(Deserialize)]
pub struct FilingAction {
    pub action: String,
    #[serde(default)]
    pub reference_no: String,
    #[serde(default)]
    pub note: String,
}

pub async fn admin_update_filing(State(st): State<AppState>, a: AdminUser, Path(id): Path<Uuid>, Json(r): Json<FilingAction>) -> AppResult<Json<Filing>> {
    let f: Filing = sqlx::query_as("SELECT * FROM finance.filings WHERE id = $1").bind(id).fetch_optional(&st.db).await?.ok_or(AppError::NotFound)?;
    let note = r.note.trim();
    let q = match (r.action.as_str(), f.status.as_str()) {
        ("submit", "draft") => {
            if r.reference_no.trim().is_empty() {
                return Err(AppError::bad("enter the tax office reference number"));
            }
            sqlx::query(
                "UPDATE finance.filings SET status = 'submitted', reference_no = $2, note = CASE WHEN $3 = '' THEN note ELSE $3 END,
                    submitted_at = now(), submitted_by = $4 WHERE id = $1",
            )
            .bind(id)
            .bind(r.reference_no.trim())
            .bind(note)
            .bind(a.0.id)
        }
        ("paid", "submitted") => sqlx::query("UPDATE finance.filings SET status = 'paid', paid_at = now(), note = CASE WHEN $2 = '' THEN note ELSE $2 END WHERE id = $1")
            .bind(id)
            .bind(note),
        ("void", "draft" | "submitted") => {
            if note.is_empty() {
                return Err(AppError::bad("add a note explaining why"));
            }
            sqlx::query("UPDATE finance.filings SET status = 'void', note = $2 WHERE id = $1").bind(id).bind(note)
        }
        ("reopen", "void") => sqlx::query("UPDATE finance.filings SET status = 'draft', reference_no = '', submitted_at = NULL, submitted_by = NULL WHERE id = $1").bind(id),
        _ => return Err(AppError::Conflict("this filing can't be changed that way".into())),
    };
    q.execute(&st.db).await?;
    event(&st, Some(f.shop_id), Some(id), a.0.id, &r.action, if note.is_empty() { r.reference_no.trim() } else { note }).await?;
    Ok(Json(sqlx::query_as("SELECT * FROM finance.filings WHERE id = $1").bind(id).fetch_one(&st.db).await?))
}

fn money(c: i64) -> String {
    format!("{}{}.{:02}", if c < 0 { "-" } else { "" }, c.abs() / 100, c.abs() % 100)
}

fn csv_field(s: &str) -> String {
    // Quote always; neutralise spreadsheet formulas.
    let s = if s.starts_with(['=', '+', '-', '@']) { format!("'{s}") } else { s.to_string() };
    format!("\"{}\"", s.replace('"', "\"\""))
}

/// CSV of a period's filings for the tax office.
pub async fn admin_export(State(st): State<AppState>, a: AdminUser, Query(q): Query<PeriodQ>) -> AppResult<impl IntoResponse> {
    let period = period_or_last(&q)?;
    let data = admin_filings(State(st), a, Query(PeriodQ { period: Some(period.format("%Y-%m").to_string()) })).await?.0;
    let mut out = String::from("\u{feff}period,shop,legal_name,tax_id,currency,vat_registered,gross,vat,net,ecommerce_tax,total_due,due_on,status,reference_no\n");
    for f in data["filings"].as_array().into_iter().flatten() {
        let g = |k: &str| f[k].as_i64().unwrap_or(0);
        let t = |k: &str| f[k].as_str().unwrap_or("").to_string();
        out.push_str(&[
            period.format("%Y-%m").to_string(),
            csv_field(&t("shop_name")),
            csv_field(&t("legal_name")),
            csv_field(&t("tax_id")),
            t("currency"),
            f["vat_registered"].as_bool().unwrap_or(false).to_string(),
            money(g("gross_cents")),
            money(g("vat_cents")),
            money(g("net_cents")),
            money(g("ecommerce_tax_cents")),
            money(g("total_due_cents")),
            t("due_on"),
            t("status"),
            csv_field(&t("reference_no")),
        ]
        .join(","));
        out.push('\n');
    }
    let name = format!("attachment; filename=\"zaokaiy-tax-{}.csv\"", period.format("%Y-%m"));
    Ok(([(header::CONTENT_TYPE, "text/csv; charset=utf-8".to_string()), (header::CONTENT_DISPOSITION, name)], out))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn periods() {
        assert_eq!(parse_period("2026-09").unwrap(), NaiveDate::from_ymd_opt(2026, 9, 1).unwrap());
        assert!(parse_period("2026-13").is_err());
        assert!(parse_period("sept").is_err());
        assert_eq!(next_month(NaiveDate::from_ymd_opt(2026, 12, 1).unwrap()), NaiveDate::from_ymd_opt(2027, 1, 1).unwrap());
    }

    #[test]
    fn csv_is_safe() {
        assert_eq!(csv_field("=cmd"), "\"'=cmd\"");
        assert_eq!(csv_field("a\"b"), "\"a\"\"b\"");
        assert_eq!(money(123456), "1234.56");
        assert_eq!(money(-5), "-0.05");
    }
}
