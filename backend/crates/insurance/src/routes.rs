use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    Json,
};
use chrono::{DateTime, Months, NaiveDate, Utc};
use kernel::guards::owned_shop;
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::FromRow;
use uuid::Uuid;

use crate::{
    auth::{optional_user, AuthUser},
    error::{AppError, AppResult},
    AppState,
};

pub const KINDS: &[&str] = &["motor", "health", "travel", "life", "property", "accident", "other"];

fn local_today() -> NaiveDate {
    (Utc::now() + chrono::Duration::hours(7)).date_naive()
}

fn major(c: i64) -> String {
    // 1234567 → "12,345.67" ; whole amounts without decimals
    let whole = c / 100;
    let s = whole.to_string();
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    if c % 100 != 0 {
        out.push_str(&format!(".{:02}", c % 100));
    }
    out
}

#[derive(Serialize, FromRow, Clone)]
pub struct Plan {
    pub id: Uuid,
    pub shop_id: Uuid,
    pub kind: String,
    pub insurer: String,
    pub name: String,
    pub name_lo: String,
    pub description: String,
    pub coverage: Vec<String>,
    pub premium_mode: String,
    pub premium_cents: i64,
    pub rate_bps: i32,
    pub min_premium_cents: i64,
    pub sum_insured_min_cents: i64,
    pub sum_insured_max_cents: i64,
    pub term_months: i32,
    pub active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Premium for a sum insured. Fixed plans ignore the sum (unless it is out of range).
pub fn premium(p: &Plan, sum_insured_cents: i64) -> AppResult<i64> {
    if p.premium_mode == "rate" && sum_insured_cents <= 0 {
        return Err(AppError::bad("enter the sum insured"));
    }
    if p.sum_insured_max_cents > 0 && !(p.sum_insured_min_cents..=p.sum_insured_max_cents).contains(&sum_insured_cents) {
        return Err(AppError::bad(format!(
            "sum insured must be between {} and {}",
            major(p.sum_insured_min_cents),
            major(p.sum_insured_max_cents)
        )));
    }
    Ok(match p.premium_mode.as_str() {
        "rate" => ((sum_insured_cents as i128 * p.rate_bps as i128 + 5_000) / 10_000) as i64,
        _ => p.premium_cents,
    }
    .max(p.min_premium_cents))
}

async fn load_plan(st: &AppState, id: Uuid) -> AppResult<Plan> {
    sqlx::query_as("SELECT * FROM insurance.plans WHERE id = $1").bind(id).fetch_optional(&st.db).await?.ok_or(AppError::NotFound)
}

// ---------------------------------------------------------------------------------------------
// Public
// ---------------------------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct PlansQ {
    pub kind: Option<String>,
    /// Shop slug.
    pub shop: Option<String>,
}

const PUBLIC_COLS: &str = "p.*, s.name AS shop_name, s.slug AS shop_slug, s.currency, s.phone AS shop_phone,
                           (s.kyb_verified_at IS NOT NULL) AS shop_verified";

#[derive(Serialize, FromRow)]
pub struct PublicPlan {
    #[sqlx(flatten)]
    #[serde(flatten)]
    pub plan: Plan,
    pub shop_name: String,
    pub shop_slug: String,
    pub currency: String,
    pub shop_phone: String,
    pub shop_verified: bool,
}

pub async fn public_plans(State(st): State<AppState>, Query(q): Query<PlansQ>) -> AppResult<Json<Vec<PublicPlan>>> {
    Ok(Json(
        sqlx::query_as(&format!(
            "SELECT {PUBLIC_COLS} FROM insurance.plans p JOIN shops s ON s.id = p.shop_id
             WHERE p.active AND s.vertical = 'insurance' AND ($1::text IS NULL OR p.kind = $1) AND ($2::text IS NULL OR s.slug = $2)
             ORDER BY p.kind, p.created_at DESC LIMIT 200"
        ))
        .bind(q.kind.filter(|k| !k.is_empty()))
        .bind(q.shop.filter(|k| !k.is_empty()))
        .fetch_all(&st.db)
        .await?,
    ))
}

pub async fn public_plan(State(st): State<AppState>, Path(id): Path<Uuid>) -> AppResult<Json<PublicPlan>> {
    Ok(Json(
        sqlx::query_as(&format!("SELECT {PUBLIC_COLS} FROM insurance.plans p JOIN shops s ON s.id = p.shop_id WHERE p.id = $1 AND p.active"))
            .bind(id)
            .fetch_optional(&st.db)
            .await?
            .ok_or(AppError::NotFound)?,
    ))
}

#[derive(Deserialize)]
pub struct QuoteReq {
    #[serde(default)]
    pub sum_insured_cents: i64,
}

pub async fn quote(State(st): State<AppState>, Path(id): Path<Uuid>, Json(r): Json<QuoteReq>) -> AppResult<Json<Value>> {
    let p = load_plan(&st, id).await?;
    if !p.active {
        return Err(AppError::NotFound);
    }
    let currency: String = sqlx::query_scalar("SELECT currency FROM shops WHERE id = $1").bind(p.shop_id).fetch_one(&st.db).await?;
    Ok(Json(json!({ "premium_cents": premium(&p, r.sum_insured_cents)?, "term_months": p.term_months, "currency": currency })))
}

#[derive(Deserialize)]
pub struct ApplyReq {
    pub applicant_name: String,
    pub phone: String,
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub sum_insured_cents: i64,
    pub start_date: NaiveDate,
    #[serde(default)]
    pub details: serde_json::Map<String, Value>,
}

#[derive(Serialize, FromRow)]
pub struct Application {
    pub id: Uuid,
    pub number: String,
    pub plan_id: Uuid,
    pub shop_id: Uuid,
    pub user_id: Option<Uuid>,
    pub applicant_name: String,
    pub phone: String,
    pub email: String,
    pub details: Value,
    pub sum_insured_cents: i64,
    pub premium_cents: i64,
    pub currency: String,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub status: String,
    pub policy_no: String,
    pub paid: bool,
    pub decision_note: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

fn phone_ok(p: &str) -> bool {
    let d = p.chars().filter(|c| c.is_ascii_digit()).count();
    (8..=15).contains(&d) && p.chars().all(|c| c.is_ascii_digit() || " +-()".contains(c))
}

pub async fn apply(State(st): State<AppState>, headers: HeaderMap, Path(id): Path<Uuid>, Json(r): Json<ApplyReq>) -> AppResult<Json<Application>> {
    let p = load_plan(&st, id).await?;
    if !p.active {
        return Err(AppError::NotFound);
    }
    let name = r.applicant_name.trim();
    if name.is_empty() || name.chars().count() > 120 {
        return Err(AppError::bad("enter the applicant's full name"));
    }
    if !phone_ok(r.phone.trim()) {
        return Err(AppError::bad("enter a valid phone number"));
    }
    let email = r.email.trim();
    if !email.is_empty() && (!email.contains('@') || email.len() > 200) {
        return Err(AppError::bad("enter a valid email address"));
    }
    let today = local_today();
    if r.start_date < today || r.start_date > today + chrono::Duration::days(180) {
        return Err(AppError::bad("start date must be within the next 6 months"));
    }
    // Free-form details: small, flat, text / number values only.
    if r.details.len() > 20 || r.details.iter().any(|(k, v)| k.len() > 40 || !(v.is_string() || v.is_number() || v.is_boolean()) || v.to_string().len() > 300) {
        return Err(AppError::bad("too many or too long details"));
    }
    let prem = premium(&p, r.sum_insured_cents)?;
    let end = r.start_date + Months::new(p.term_months as u32) - chrono::Duration::days(1);
    let currency: String = sqlx::query_scalar("SELECT currency FROM shops WHERE id = $1").bind(p.shop_id).fetch_one(&st.db).await?;
    let number = format!("INS-{}-{:04}", today.format("%y%m%d"), rand::thread_rng().gen_range(0..10_000));
    let user = optional_user(&st, &headers);
    Ok(Json(
        sqlx::query_as(
            "INSERT INTO insurance.applications (number, plan_id, shop_id, user_id, applicant_name, phone, email, details,
                sum_insured_cents, premium_cents, currency, start_date, end_date)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13) RETURNING *",
        )
        .bind(number)
        .bind(p.id)
        .bind(p.shop_id)
        .bind(user.map(|u| u.id))
        .bind(name)
        .bind(r.phone.trim())
        .bind(email)
        .bind(Value::Object(r.details))
        .bind(r.sum_insured_cents.max(0))
        .bind(prem)
        .bind(currency)
        .bind(r.start_date)
        .bind(end)
        .fetch_one(&st.db)
        .await?,
    ))
}

pub async fn my_applications(State(st): State<AppState>, user: AuthUser) -> AppResult<Json<Vec<Value>>> {
    let rows: Vec<(sqlx::types::Json<Value>,)> = sqlx::query_as(
        "SELECT to_jsonb(a) || jsonb_build_object('plan_name', p.name, 'insurer', p.insurer, 'kind', p.kind, 'shop_name', s.name, 'shop_phone', s.phone)
         FROM insurance.applications a JOIN insurance.plans p ON p.id = a.plan_id JOIN shops s ON s.id = a.shop_id
         WHERE a.user_id = $1 ORDER BY a.created_at DESC LIMIT 100",
    )
    .bind(user.id)
    .fetch_all(&st.db)
    .await?;
    Ok(Json(rows.into_iter().map(|r| r.0 .0).collect()))
}

// ---------------------------------------------------------------------------------------------
// Agent: plans
// ---------------------------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct PlanReq {
    pub kind: Option<String>,
    pub insurer: Option<String>,
    pub name: Option<String>,
    pub name_lo: Option<String>,
    pub description: Option<String>,
    pub coverage: Option<Vec<String>>,
    pub premium_mode: Option<String>,
    pub premium_cents: Option<i64>,
    pub rate_bps: Option<i32>,
    pub min_premium_cents: Option<i64>,
    pub sum_insured_min_cents: Option<i64>,
    pub sum_insured_max_cents: Option<i64>,
    pub term_months: Option<i32>,
    pub active: Option<bool>,
}

/// Merge a request onto a plan (or defaults) and validate the result.
fn merged(base: Option<&Plan>, r: &PlanReq) -> AppResult<Plan> {
    let now = Utc::now();
    let mut p = base.cloned().unwrap_or(Plan {
        id: Uuid::nil(),
        shop_id: Uuid::nil(),
        kind: String::new(),
        insurer: String::new(),
        name: String::new(),
        name_lo: String::new(),
        description: String::new(),
        coverage: vec![],
        premium_mode: "fixed".into(),
        premium_cents: 0,
        rate_bps: 0,
        min_premium_cents: 0,
        sum_insured_min_cents: 0,
        sum_insured_max_cents: 0,
        term_months: 12,
        active: true,
        created_at: now,
        updated_at: now,
    });
    macro_rules! set {
        ($f:ident) => {
            if let Some(v) = r.$f.clone() {
                p.$f = v;
            }
        };
    }
    set!(kind);
    set!(insurer);
    set!(name);
    set!(name_lo);
    set!(description);
    set!(coverage);
    set!(premium_mode);
    set!(premium_cents);
    set!(rate_bps);
    set!(min_premium_cents);
    set!(sum_insured_min_cents);
    set!(sum_insured_max_cents);
    set!(term_months);
    set!(active);
    p.insurer = p.insurer.trim().to_string();
    p.name = p.name.trim().to_string();
    p.name_lo = p.name_lo.trim().chars().take(120).collect();
    p.description = p.description.trim().chars().take(2000).collect();
    p.coverage = p.coverage.iter().map(|c| c.trim().to_string()).filter(|c| !c.is_empty()).collect();
    if !KINDS.contains(&p.kind.as_str()) {
        return Err(AppError::bad("choose an insurance type"));
    }
    if p.insurer.is_empty() || p.insurer.chars().count() > 120 {
        return Err(AppError::bad("enter the insurer's name"));
    }
    if p.name.is_empty() || p.name.chars().count() > 120 {
        return Err(AppError::bad("enter a plan name (up to 120 characters)"));
    }
    if p.coverage.len() > 20 || p.coverage.iter().any(|c| c.chars().count() > 160) {
        return Err(AppError::bad("up to 20 coverage points, 160 characters each"));
    }
    match p.premium_mode.as_str() {
        "fixed" if p.premium_cents > 0 => {}
        "fixed" => return Err(AppError::bad("set the premium")),
        "rate" if (1..=10_000).contains(&p.rate_bps) => {}
        "rate" => return Err(AppError::bad("set the premium rate")),
        _ => return Err(AppError::bad("premium must be fixed or a rate of the sum insured")),
    }
    if p.premium_cents < 0 || p.min_premium_cents < 0 || p.sum_insured_min_cents < 0 || p.sum_insured_max_cents < 0 {
        return Err(AppError::bad("amounts can't be negative"));
    }
    if p.sum_insured_max_cents > 0 && p.sum_insured_max_cents < p.sum_insured_min_cents {
        return Err(AppError::bad("maximum sum insured must be at least the minimum"));
    }
    if p.premium_mode == "rate" && p.sum_insured_max_cents == 0 {
        return Err(AppError::bad("set the maximum sum insured"));
    }
    if !(1..=120).contains(&p.term_months) {
        return Err(AppError::bad("term must be between 1 and 120 months"));
    }
    Ok(p)
}

pub async fn shop_plans(State(st): State<AppState>, user: AuthUser, Path(id): Path<Uuid>) -> AppResult<Json<Vec<Plan>>> {
    owned_shop(&st, id, &user).await?;
    Ok(Json(sqlx::query_as("SELECT * FROM insurance.plans WHERE shop_id = $1 ORDER BY created_at DESC").bind(id).fetch_all(&st.db).await?))
}

async fn save(st: &AppState, p: &Plan, insert: bool) -> AppResult<Plan> {
    let sql = if insert {
        "INSERT INTO insurance.plans (shop_id, kind, insurer, name, name_lo, description, coverage, premium_mode, premium_cents, rate_bps,
            min_premium_cents, sum_insured_min_cents, sum_insured_max_cents, term_months, active)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15) RETURNING *"
    } else {
        "UPDATE insurance.plans SET kind = $2, insurer = $3, name = $4, name_lo = $5, description = $6, coverage = $7, premium_mode = $8,
            premium_cents = $9, rate_bps = $10, min_premium_cents = $11, sum_insured_min_cents = $12, sum_insured_max_cents = $13,
            term_months = $14, active = $15, updated_at = now()
         WHERE id = $16 RETURNING *"
    };
    let q = sqlx::query_as(sql)
        .bind(p.shop_id)
        .bind(&p.kind)
        .bind(&p.insurer)
        .bind(&p.name)
        .bind(&p.name_lo)
        .bind(&p.description)
        .bind(&p.coverage)
        .bind(&p.premium_mode)
        .bind(p.premium_cents)
        .bind(p.rate_bps)
        .bind(p.min_premium_cents)
        .bind(p.sum_insured_min_cents)
        .bind(p.sum_insured_max_cents)
        .bind(p.term_months)
        .bind(p.active);
    let q = if insert { q } else { q.bind(p.id) };
    Ok(q.fetch_one(&st.db).await?)
}

pub async fn create_plan(State(st): State<AppState>, user: AuthUser, Path(id): Path<Uuid>, Json(r): Json<PlanReq>) -> AppResult<Json<Plan>> {
    let shop = owned_shop(&st, id, &user).await?;
    if shop.vertical != "insurance" {
        return Err(AppError::bad("only insurance agent shops can publish plans"));
    }
    let mut p = merged(None, &r)?;
    p.shop_id = id;
    Ok(Json(save(&st, &p, true).await?))
}

pub async fn update_plan(State(st): State<AppState>, user: AuthUser, Path(id): Path<Uuid>, Json(r): Json<PlanReq>) -> AppResult<Json<Plan>> {
    let before = load_plan(&st, id).await?;
    owned_shop(&st, before.shop_id, &user).await?;
    let p = merged(Some(&before), &r)?;
    Ok(Json(save(&st, &p, false).await?))
}

// ---------------------------------------------------------------------------------------------
// Agent: applications → policies
// ---------------------------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct AppsQ {
    pub status: Option<String>,
}

pub async fn shop_applications(State(st): State<AppState>, user: AuthUser, Path(id): Path<Uuid>, Query(q): Query<AppsQ>) -> AppResult<Json<Vec<Value>>> {
    owned_shop(&st, id, &user).await?;
    let rows: Vec<(sqlx::types::Json<Value>,)> = sqlx::query_as(
        "SELECT to_jsonb(a) || jsonb_build_object('plan_name', p.name, 'insurer', p.insurer, 'kind', p.kind)
         FROM insurance.applications a JOIN insurance.plans p ON p.id = a.plan_id
         WHERE a.shop_id = $1 AND ($2::text IS NULL OR a.status = $2) ORDER BY a.created_at DESC LIMIT 300",
    )
    .bind(id)
    .bind(q.status.filter(|s| !s.is_empty()))
    .fetch_all(&st.db)
    .await?;
    Ok(Json(rows.into_iter().map(|r| r.0 .0).collect()))
}

#[derive(Deserialize)]
pub struct Decision {
    /// review · approve · issue · reject · cancel · paid · unpaid
    pub action: String,
    #[serde(default)]
    pub policy_no: String,
    #[serde(default)]
    pub note: String,
}

/// Next status for an action, if allowed from `from`.
pub fn next_status(action: &str, from: &str) -> Option<&'static str> {
    match (action, from) {
        ("review", "submitted") => Some("reviewing"),
        ("approve", "submitted" | "reviewing") => Some("approved"),
        ("issue", "approved") => Some("issued"),
        ("reject", "submitted" | "reviewing" | "approved") => Some("rejected"),
        ("cancel", "submitted" | "reviewing" | "approved" | "issued") => Some("cancelled"),
        _ => None,
    }
}

pub async fn decide(State(st): State<AppState>, user: AuthUser, Path(id): Path<Uuid>, Json(d): Json<Decision>) -> AppResult<Json<Application>> {
    let a: Application =
        sqlx::query_as("SELECT * FROM insurance.applications WHERE id = $1").bind(id).fetch_optional(&st.db).await?.ok_or(AppError::NotFound)?;
    owned_shop(&st, a.shop_id, &user).await?;
    let note = d.note.trim();
    if matches!(d.action.as_str(), "paid" | "unpaid") {
        if matches!(a.status.as_str(), "rejected" | "cancelled") {
            return Err(AppError::Conflict("this application is closed".into()));
        }
        return Ok(Json(
            sqlx::query_as("UPDATE insurance.applications SET paid = $2, updated_at = now() WHERE id = $1 RETURNING *")
                .bind(id)
                .bind(d.action == "paid")
                .fetch_one(&st.db)
                .await?,
        ));
    }
    let next = next_status(&d.action, &a.status).ok_or_else(|| AppError::Conflict(format!("can't {} an application that is {}", d.action, a.status)))?;
    let policy_no = d.policy_no.trim();
    if next == "issued" {
        if !a.paid {
            return Err(AppError::bad("mark the premium as paid before issuing the policy"));
        }
        if policy_no.is_empty() || policy_no.len() > 60 {
            return Err(AppError::bad("enter the policy number from the insurer"));
        }
    }
    if matches!(next, "rejected" | "cancelled") && note.is_empty() {
        return Err(AppError::bad("add a note for the customer"));
    }
    Ok(Json(
        sqlx::query_as(
            "UPDATE insurance.applications SET status = $2, policy_no = CASE WHEN $3 = '' THEN policy_no ELSE $3 END,
                decision_note = CASE WHEN $4 = '' THEN decision_note ELSE $4 END, updated_at = now() WHERE id = $1 RETURNING *",
        )
        .bind(id)
        .bind(next)
        .bind(policy_no)
        .bind(note)
        .fetch_one(&st.db)
        .await?,
    ))
}

#[cfg(test)]
#[allow(clippy::inconsistent_digit_grouping)]
mod tests {
    use super::*;

    fn plan(mode: &str) -> Plan {
        let r = PlanReq {
            kind: Some("motor".into()),
            insurer: Some("AGL".into()),
            name: Some("Class 1".into()),
            name_lo: None,
            description: None,
            coverage: None,
            premium_mode: Some(mode.into()),
            premium_cents: Some(150_000_00),
            rate_bps: Some(250),
            min_premium_cents: Some(100_000_00),
            sum_insured_min_cents: Some(10_000_000_00),
            sum_insured_max_cents: Some(2_000_000_000_00),
            term_months: Some(12),
            active: None,
        };
        merged(None, &r).unwrap()
    }

    #[test]
    fn premiums() {
        let fixed = plan("fixed");
        assert_eq!(premium(&fixed, 50_000_000_00).unwrap(), 150_000_00);
        let rate = plan("rate");
        // 2.5% of 300,000,000 = 7,500,000
        assert_eq!(premium(&rate, 300_000_000_00).unwrap(), 7_500_000_00);
        // floor at the minimum premium
        assert_eq!(premium(&rate, 10_000_000_00).unwrap(), 250_000_00);
        assert!(premium(&rate, 0).is_err());
        let e = premium(&rate, 5_000_000_00).unwrap_err().to_string();
        assert_eq!(e, "sum insured must be between 10,000,000 and 2,000,000,000");
    }

    #[test]
    fn validation() {
        let mut r = PlanReq { kind: Some("pets".into()), insurer: None, name: None, name_lo: None, description: None, coverage: None, premium_mode: None, premium_cents: None, rate_bps: None, min_premium_cents: None, sum_insured_min_cents: None, sum_insured_max_cents: None, term_months: None, active: None };
        assert_eq!(merged(None, &r).err().unwrap().to_string(), "choose an insurance type");
        r.kind = Some("travel".into());
        r.insurer = Some("X".into());
        r.name = Some("Trip".into());
        assert_eq!(merged(None, &r).err().unwrap().to_string(), "set the premium");
    }

    #[test]
    fn flow() {
        assert_eq!(next_status("approve", "reviewing"), Some("approved"));
        assert_eq!(next_status("issue", "submitted"), None);
        assert_eq!(next_status("cancel", "issued"), Some("cancelled"));
        assert_eq!(next_status("review", "rejected"), None);
        assert_eq!(major(123_456_789), "1,234,567.89");
        assert_eq!(major(100_000_00), "100,000");
    }
}
