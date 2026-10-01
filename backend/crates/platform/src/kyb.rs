//! Corporate KYC ("know your business") for business shops.
//!
//! Seller:  GET|PUT /shops/{id}/kyb · POST /shops/{id}/kyb/submit
//!          POST /shops/{id}/kyb/documents (multipart: kind, file, expires_on) · DELETE /kyb/documents/{id}
//!          GET /kyb/documents/{id}/file   (owner or admin; decrypted on the fly, never cached)
//! Admin:   GET /admin/kyb · GET /admin/kyb/{shop_id} · POST /admin/kyb/{shop_id}/decision
//!
//! Status: none → draft → submitted → approved | changes_requested | rejected; approved → revoked.
//! A verified shop that edits its details stays verified while the update is reviewed; only
//! `revoke` removes verification (and takes its products off sale).
//! ID and bank account numbers are encrypted (only the last 4 characters are kept in clear);
//! documents are encrypted files in private storage. Admin views are written to the audit log.

use axum::{
    body::Body,
    extract::{Multipart, Path, Query, State},
    http::{header, HeaderValue},
    response::Response,
    Json,
};
use bytes::Bytes;
use chrono::{DateTime, Months, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::FromRow;
use uuid::Uuid;

use crate::{
    auth::{AdminUser, AuthUser},
    crypto::last4,
    error::{AppError, AppResult},
    models::Shop,
    guards::owned_shop,
    AppState,
};

pub const COMPANY_TYPES: &[&str] = &[
    "sole_enterprise", "sole_company", "limited_company", "public_company", "partnership", "state_enterprise",
    "cooperative", "foreign_branch", "other",
];
/// Company types whose owners are shareholders: beneficial owners and a shareholder register are required.
const SHAREHOLDER_TYPES: &[&str] = &["sole_company", "limited_company", "public_company", "partnership"];
pub const DOC_KINDS: &[&str] = &[
    "registration_certificate", "tax_certificate", "business_license", "representative_id", "authorization_letter",
    "shareholder_register", "articles", "proof_of_address", "bank_proof", "other",
];
pub const ID_TYPES: &[&str] = &["national_id", "passport", "family_book", "other"];
pub const ROLES: &[&str] = &["representative", "director", "ubo"];
const MAX_DOC_BYTES: usize = 10 * 1_048_576;
const MAX_DOCS: i64 = 30;
const MAX_PERSONS: usize = 20;

// ---------------------------------------------------------------------------------------------
// Data
// ---------------------------------------------------------------------------------------------

#[derive(FromRow, Clone)]
struct ProfileRow {
    company_type: String,
    legal_name: String,
    legal_name_local: String,
    registration_no: String,
    registration_date: Option<NaiveDate>,
    tax_id: String,
    country: String,
    province: String,
    registered_address: String,
    business_activity: String,
    website: String,
    contact_email: String,
    contact_phone: String,
    bank_name: String,
    bank_account_name: String,
    bank_account_enc: String,
    bank_account_last4: String,
    risk_flags: Vec<String>,
    submitted_at: Option<DateTime<Utc>>,
    reviewed_at: Option<DateTime<Utc>>,
    review_note: String,
    review_due_at: Option<DateTime<Utc>>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow, Clone)]
struct PersonRow {
    id: Uuid,
    roles: Vec<String>,
    full_name: String,
    title: String,
    nationality: String,
    date_of_birth: Option<NaiveDate>,
    id_type: String,
    id_number_enc: String,
    id_last4: String,
    ownership_bps: i32,
    is_pep: bool,
}

#[derive(FromRow, Serialize, Clone)]
pub struct DocRow {
    pub id: Uuid,
    pub kind: String,
    pub content_type: String,
    pub size_bytes: i64,
    pub sha256: String,
    pub original_name: String,
    pub expires_on: Option<NaiveDate>,
    pub status: String,
    pub review_note: String,
    pub created_at: DateTime<Utc>,
}

fn profile_json(p: &ProfileRow, st: &AppState, reveal: bool) -> AppResult<Value> {
    Ok(json!({
        "company_type": p.company_type, "legal_name": p.legal_name, "legal_name_local": p.legal_name_local,
        "registration_no": p.registration_no, "registration_date": p.registration_date, "tax_id": p.tax_id,
        "country": p.country, "province": p.province, "registered_address": p.registered_address,
        "business_activity": p.business_activity, "website": p.website, "contact_email": p.contact_email,
        "contact_phone": p.contact_phone, "bank_name": p.bank_name, "bank_account_name": p.bank_account_name,
        "bank_account_last4": p.bank_account_last4,
        "bank_account_number": if reveal { Some(st.sealer.open_str(&p.bank_account_enc)?) } else { None },
        "risk_flags": p.risk_flags, "submitted_at": p.submitted_at, "reviewed_at": p.reviewed_at,
        "review_note": p.review_note, "review_due_at": p.review_due_at, "updated_at": p.updated_at,
    }))
}

fn person_json(p: &PersonRow, st: &AppState, reveal: bool) -> AppResult<Value> {
    Ok(json!({
        "id": p.id, "roles": p.roles, "full_name": p.full_name, "title": p.title, "nationality": p.nationality,
        "date_of_birth": p.date_of_birth, "id_type": p.id_type, "id_last4": p.id_last4,
        "has_id_number": !p.id_number_enc.is_empty(),
        "id_number": if reveal { Some(st.sealer.open_str(&p.id_number_enc)?) } else { None },
        "ownership_bps": p.ownership_bps, "is_pep": p.is_pep,
    }))
}

async fn load(st: &AppState, shop_id: Uuid) -> AppResult<(Option<ProfileRow>, Vec<PersonRow>, Vec<DocRow>)> {
    let profile: Option<ProfileRow> = sqlx::query_as("SELECT * FROM kyb_profiles WHERE shop_id = $1")
        .bind(shop_id)
        .fetch_optional(&st.db)
        .await?;
    let persons: Vec<PersonRow> = sqlx::query_as("SELECT * FROM kyb_persons WHERE shop_id = $1 ORDER BY position_no, created_at")
        .bind(shop_id)
        .fetch_all(&st.db)
        .await?;
    let docs: Vec<DocRow> = sqlx::query_as("SELECT * FROM kyb_documents WHERE shop_id = $1 ORDER BY created_at")
        .bind(shop_id)
        .fetch_all(&st.db)
        .await?;
    Ok((profile, persons, docs))
}

async fn event(st: &AppState, shop_id: Uuid, actor: Option<Uuid>, action: &str, note: &str) -> AppResult<()> {
    sqlx::query("INSERT INTO kyb_events (shop_id, actor_id, action, note) VALUES ($1,$2,$3,$4)")
        .bind(shop_id)
        .bind(actor)
        .bind(action)
        .bind(note)
        .execute(&st.db)
        .await?;
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// Completeness
// ---------------------------------------------------------------------------------------------

/// What is still needed before the verification can be submitted (codes the UI translates).
fn missing(p: Option<&ProfileRow>, persons: &[PersonRow], docs: &[DocRow], today: NaiveDate) -> Vec<String> {
    let mut m: Vec<String> = Vec::new();
    let Some(p) = p else {
        return vec!["company".into()];
    };
    for (k, v) in [
        ("company_type", &p.company_type),
        ("legal_name", &p.legal_name),
        ("registration_no", &p.registration_no),
        ("tax_id", &p.tax_id),
        ("registered_address", &p.registered_address),
        ("business_activity", &p.business_activity),
        ("contact_phone", &p.contact_phone),
        ("bank_name", &p.bank_name),
        ("bank_account_name", &p.bank_account_name),
        ("bank_account_number", &p.bank_account_enc),
    ] {
        if v.trim().is_empty() {
            m.push(k.into());
        }
    }
    let reps: Vec<&PersonRow> = persons.iter().filter(|x| x.roles.iter().any(|r| r == "representative")).collect();
    match reps.len() {
        0 => m.push("representative".into()),
        1 => {}
        _ => m.push("one_representative".into()),
    }
    if persons.iter().any(|x| x.id_number_enc.is_empty()) {
        m.push("person_id_number".into());
    }
    let shareholders = SHAREHOLDER_TYPES.contains(&p.company_type.as_str());
    let ubos: Vec<&PersonRow> = persons.iter().filter(|x| x.roles.iter().any(|r| r == "ubo")).collect();
    if shareholders && ubos.is_empty() {
        m.push("ubo".into());
    }
    if ubos.iter().map(|x| x.ownership_bps as i64).sum::<i64>() > 10_000 {
        m.push("ownership_total".into());
    }
    // Documents: the live ones (not rejected), and not expired.
    let live = |kind: &str| docs.iter().any(|d| d.kind == kind && d.status != "rejected" && d.expires_on.is_none_or(|e| e >= today));
    let mut need = vec!["registration_certificate", "tax_certificate", "representative_id"];
    if shareholders {
        need.push("shareholder_register");
    }
    let rep_is_director = reps.first().is_some_and(|r| r.roles.iter().any(|x| x == "director"));
    if !reps.is_empty() && !rep_is_director {
        need.push("authorization_letter");
    }
    for k in need {
        if !live(k) {
            m.push(format!("doc:{k}"));
        }
    }
    m
}

// ---------------------------------------------------------------------------------------------
// Seller
// ---------------------------------------------------------------------------------------------

fn options() -> Value {
    json!({ "company_types": COMPANY_TYPES, "shareholder_types": SHAREHOLDER_TYPES, "doc_kinds": DOC_KINDS,
            "id_types": ID_TYPES, "roles": ROLES, "max_doc_mb": MAX_DOC_BYTES / 1_048_576 })
}

async fn view(st: &AppState, shop: &Shop, reveal: bool, admin: bool) -> AppResult<Value> {
    let (profile, persons, docs) = load(st, shop.id).await?;
    let today = Utc::now().date_naive();
    let events: Vec<Value> = sqlx::query_scalar(
        "SELECT jsonb_build_object('action', e.action, 'note', e.note, 'created_at', e.created_at,
                                   'actor', COALESCE(u.display_name, ''), 'by_admin', COALESCE(u.role = 'admin', false))
         FROM kyb_events e LEFT JOIN users u ON u.id = e.actor_id
         WHERE e.shop_id = $1 AND ($2 OR e.action NOT IN ('viewed','document_viewed'))
         ORDER BY e.created_at DESC LIMIT 100",
    )
    .bind(shop.id)
    .bind(admin)
    .fetch_all(&st.db)
    .await?;
    Ok(json!({
        "shop": { "id": shop.id, "name": shop.name, "slug": shop.slug, "entity_type": shop.entity_type,
                  "kyb_status": shop.kyb_status, "kyb_verified_at": shop.kyb_verified_at, "owner_id": shop.owner_id },
        "profile": profile.as_ref().map(|p| profile_json(p, st, reveal)).transpose()?,
        "persons": persons.iter().map(|p| person_json(p, st, reveal)).collect::<AppResult<Vec<_>>>()?,
        "documents": docs,
        "missing": missing(profile.as_ref(), &persons, &docs, today),
        "editable": editable(&shop.kyb_status),
        "events": events,
        "required_to_publish": st.cfg.kyb_required_to_publish,
        "options": options(),
    }))
}

fn editable(status: &str) -> bool {
    status != "submitted"
}

pub async fn get_kyb(State(st): State<AppState>, user: AuthUser, Path(shop_id): Path<Uuid>) -> AppResult<Json<Value>> {
    let shop = owned_shop(&st, shop_id, &user).await?;
    Ok(Json(view(&st, &shop, false, false).await?))
}

#[derive(Deserialize)]
pub struct PersonInput {
    /// Existing person: keeps the stored ID number when `id_number` is omitted.
    pub id: Option<Uuid>,
    #[serde(default)]
    pub roles: Vec<String>,
    #[serde(default)]
    pub full_name: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub nationality: String,
    pub date_of_birth: Option<NaiveDate>,
    #[serde(default)]
    pub id_type: String,
    pub id_number: Option<String>,
    #[serde(default)]
    pub ownership_bps: i32,
    #[serde(default)]
    pub is_pep: bool,
}

#[derive(Deserialize)]
pub struct ProfileInput {
    #[serde(default)]
    pub company_type: String,
    #[serde(default)]
    pub legal_name: String,
    #[serde(default)]
    pub legal_name_local: String,
    #[serde(default)]
    pub registration_no: String,
    pub registration_date: Option<NaiveDate>,
    #[serde(default)]
    pub tax_id: String,
    #[serde(default)]
    pub country: String,
    #[serde(default)]
    pub province: String,
    #[serde(default)]
    pub registered_address: String,
    #[serde(default)]
    pub business_activity: String,
    #[serde(default)]
    pub website: String,
    #[serde(default)]
    pub contact_email: String,
    #[serde(default)]
    pub contact_phone: String,
    #[serde(default)]
    pub bank_name: String,
    #[serde(default)]
    pub bank_account_name: String,
    /// Omitted = keep the stored number; "" = clear.
    pub bank_account_number: Option<String>,
    #[serde(default)]
    pub persons: Vec<PersonInput>,
}

fn clean(s: &str, max: usize) -> AppResult<String> {
    let t = s.trim();
    if t.chars().count() > max {
        return Err(AppError::bad("a field in the verification form is too long"));
    }
    Ok(t.to_string())
}

fn country(c: &str) -> AppResult<String> {
    let c = c.trim().to_uppercase();
    let c = if c.is_empty() { "LA".to_string() } else { c };
    if c.len() == 2 && c.chars().all(|x| x.is_ascii_uppercase()) {
        Ok(c)
    } else {
        Err(AppError::bad("country must be a 2-letter code, e.g. LA or TH"))
    }
}

/// Identity / account numbers: letters, digits, spaces, '-' and '/', 4-40 characters.
fn id_number(s: &str) -> AppResult<String> {
    let t = s.split_whitespace().collect::<Vec<_>>().join(" ").to_uppercase();
    let core: String = t.chars().filter(|c| c.is_alphanumeric()).collect();
    if core.len() < 4 || t.chars().count() > 40 || !t.chars().all(|c| c.is_alphanumeric() || " -/.".contains(c)) {
        return Err(AppError::bad("ID and account numbers must be 4-40 letters or digits"));
    }
    Ok(t)
}

pub async fn save_kyb(
    State(st): State<AppState>,
    user: AuthUser,
    Path(shop_id): Path<Uuid>,
    Json(r): Json<ProfileInput>,
) -> AppResult<Json<Value>> {
    let shop = owned_shop(&st, shop_id, &user).await?;
    if shop.entity_type != "business" {
        return Err(AppError::bad("set the shop type to business first"));
    }
    if !editable(&shop.kyb_status) {
        return Err(AppError::Conflict("your verification is under review — wait for the decision before editing".into()));
    }
    if !r.company_type.is_empty() && !COMPANY_TYPES.contains(&r.company_type.as_str()) {
        return Err(AppError::bad("unknown company type"));
    }
    if r.persons.len() > MAX_PERSONS {
        return Err(AppError::bad("at most 20 people"));
    }
    let email = clean(&r.contact_email, 120)?.to_lowercase();
    if !email.is_empty() && !(email.contains('@') && email.contains('.') && !email.contains(' ')) {
        return Err(AppError::bad("invalid email"));
    }
    let website = clean(&r.website, 200)?;
    if !website.is_empty() && !(website.starts_with("https://") || website.starts_with("http://")) {
        return Err(AppError::bad("website must start with https://"));
    }
    if r.registration_date.is_some_and(|d| d > Utc::now().date_naive()) {
        return Err(AppError::bad("registration date cannot be in the future"));
    }
    let (existing, old_persons, _) = load(&st, shop_id).await?;
    let bank_enc = match r.bank_account_number.as_deref().map(str::trim) {
        None => existing.as_ref().map(|p| (p.bank_account_enc.clone(), p.bank_account_last4.clone())).unwrap_or_default(),
        Some("") => (String::new(), String::new()),
        Some(n) => {
            let n = id_number(n)?;
            (st.sealer.seal_str(&n), last4(&n))
        }
    };

    // Validate people before writing anything.
    #[allow(clippy::type_complexity)]
    let mut people: Vec<(Uuid, Vec<String>, String, String, String, Option<NaiveDate>, String, String, String, i32, bool)> = Vec::new();
    for p in &r.persons {
        let name = clean(&p.full_name, 120)?;
        if name.is_empty() {
            return Err(AppError::bad("each person needs a full name"));
        }
        let mut roles: Vec<String> = p.roles.iter().map(|x| x.trim().to_lowercase()).filter(|x| !x.is_empty()).collect();
        roles.sort();
        roles.dedup();
        if roles.is_empty() || roles.iter().any(|x| !ROLES.contains(&x.as_str())) {
            return Err(AppError::bad("each person needs a role: representative, director or beneficial owner"));
        }
        let id_type = if p.id_type.is_empty() { "national_id".to_string() } else { p.id_type.clone() };
        if !ID_TYPES.contains(&id_type.as_str()) {
            return Err(AppError::bad("unknown ID document type"));
        }
        if p.date_of_birth.is_some_and(|d| d > Utc::now().date_naive() || d.format("%Y").to_string().parse::<i32>().unwrap_or(0) < 1900) {
            return Err(AppError::bad("enter a valid date of birth"));
        }
        if !(0..=10_000).contains(&p.ownership_bps) {
            return Err(AppError::bad("ownership must be between 0 and 100%"));
        }
        let (enc, l4) = match p.id_number.as_deref().map(str::trim) {
            Some(n) if !n.is_empty() => {
                let n = id_number(n)?;
                (st.sealer.seal_str(&n), last4(&n))
            }
            Some(_) => (String::new(), String::new()),
            None => p
                .id
                .and_then(|id| old_persons.iter().find(|o| o.id == id))
                .map(|o| (o.id_number_enc.clone(), o.id_last4.clone()))
                .unwrap_or_default(),
        };
        let bps = if roles.iter().any(|x| x == "ubo") { p.ownership_bps } else { 0 };
        // Keep the id of an existing person (stable ids across saves), otherwise a new one.
        let pid = p.id.filter(|id| old_persons.iter().any(|o| o.id == *id) && !people.iter().any(|x| x.0 == *id)).unwrap_or_else(Uuid::new_v4);
        people.push((pid, roles, name, clean(&p.title, 80)?, country(&p.nationality)?, p.date_of_birth, id_type, enc, l4, bps, p.is_pep));
    }

    let mut tx = st.db.begin().await?;
    sqlx::query(
        "INSERT INTO kyb_profiles (shop_id, company_type, legal_name, legal_name_local, registration_no, registration_date, tax_id,
            country, province, registered_address, business_activity, website, contact_email, contact_phone, bank_name,
            bank_account_name, bank_account_enc, bank_account_last4)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18)
         ON CONFLICT (shop_id) DO UPDATE SET company_type = EXCLUDED.company_type, legal_name = EXCLUDED.legal_name,
            legal_name_local = EXCLUDED.legal_name_local, registration_no = EXCLUDED.registration_no,
            registration_date = EXCLUDED.registration_date, tax_id = EXCLUDED.tax_id, country = EXCLUDED.country,
            province = EXCLUDED.province, registered_address = EXCLUDED.registered_address,
            business_activity = EXCLUDED.business_activity, website = EXCLUDED.website, contact_email = EXCLUDED.contact_email,
            contact_phone = EXCLUDED.contact_phone, bank_name = EXCLUDED.bank_name, bank_account_name = EXCLUDED.bank_account_name,
            bank_account_enc = EXCLUDED.bank_account_enc, bank_account_last4 = EXCLUDED.bank_account_last4, updated_at = now()",
    )
    .bind(shop_id)
    .bind(&r.company_type)
    .bind(clean(&r.legal_name, 200)?)
    .bind(clean(&r.legal_name_local, 200)?)
    .bind(clean(&r.registration_no, 60)?.to_uppercase())
    .bind(r.registration_date)
    .bind(clean(&r.tax_id, 40)?.to_uppercase())
    .bind(country(&r.country)?)
    .bind(clean(&r.province, 80)?)
    .bind(clean(&r.registered_address, 500)?)
    .bind(clean(&r.business_activity, 300)?)
    .bind(website)
    .bind(email)
    .bind(clean(&r.contact_phone, 40)?)
    .bind(clean(&r.bank_name, 80)?)
    .bind(clean(&r.bank_account_name, 120)?)
    .bind(&bank_enc.0)
    .bind(&bank_enc.1)
    .execute(&mut *tx)
    .await?;
    sqlx::query("DELETE FROM kyb_persons WHERE shop_id = $1").bind(shop_id).execute(&mut *tx).await?;
    for (i, (pid, roles, name, title, nat, dob, id_type, enc, l4, bps, pep)) in people.into_iter().enumerate() {
        sqlx::query(
            "INSERT INTO kyb_persons (id, shop_id, position_no, roles, full_name, title, nationality, date_of_birth, id_type,
                id_number_enc, id_last4, ownership_bps, is_pep) VALUES ($13,$1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)",
        )
        .bind(shop_id)
        .bind(i as i32)
        .bind(&roles)
        .bind(name)
        .bind(title)
        .bind(nat)
        .bind(dob)
        .bind(id_type)
        .bind(enc)
        .bind(l4)
        .bind(bps)
        .bind(pep)
        .bind(pid)
        .execute(&mut *tx)
        .await?;
    }
    // Keep "changes requested" (the seller still sees the reviewer's note) — otherwise a draft.
    sqlx::query("UPDATE shops SET kyb_status = CASE WHEN kyb_status = 'changes_requested' THEN kyb_status ELSE 'draft' END WHERE id = $1")
        .bind(shop_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    event(&st, shop_id, Some(user.id), "saved", "").await?;
    let shop = owned_shop(&st, shop_id, &user).await?;
    Ok(Json(view(&st, &shop, false, false).await?))
}

pub async fn submit(State(st): State<AppState>, user: AuthUser, Path(shop_id): Path<Uuid>) -> AppResult<Json<Value>> {
    let shop = owned_shop(&st, shop_id, &user).await?;
    if shop.entity_type != "business" {
        return Err(AppError::bad("set the shop type to business first"));
    }
    if shop.kyb_status == "submitted" {
        return Err(AppError::Conflict("your verification is already under review".into()));
    }
    let (profile, persons, docs) = load(&st, shop_id).await?;
    let today = Utc::now().date_naive();
    if !missing(profile.as_ref(), &persons, &docs, today).is_empty() {
        return Err(AppError::bad("some required information or documents are missing"));
    }
    let p = profile.expect("checked by missing()");
    // Risk flags for the reviewer (they never block submission).
    let mut flags: Vec<String> = Vec::new();
    if persons.iter().any(|x| x.is_pep) {
        flags.push("pep".into());
    }
    let dup: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM kyb_profiles k JOIN shops s ON s.id = k.shop_id
                        WHERE k.shop_id <> $1 AND k.country = $2 AND lower(k.registration_no) = lower($3) AND s.owner_id <> $4)",
    )
    .bind(shop_id)
    .bind(&p.country)
    .bind(&p.registration_no)
    .bind(shop.owner_id)
    .fetch_one(&st.db)
    .await?;
    if dup {
        flags.push("duplicate_registration".into());
    }
    if persons.iter().any(|x| x.nationality != p.country) {
        flags.push("foreign_person".into());
    }
    if docs.iter().any(|d| d.status != "rejected" && d.expires_on.is_some_and(|e| e <= today + chrono::Duration::days(30))) {
        flags.push("document_expiring".into());
    }
    let ubo_total: i64 = persons.iter().filter(|x| x.roles.iter().any(|r| r == "ubo")).map(|x| x.ownership_bps as i64).sum();
    if SHAREHOLDER_TYPES.contains(&p.company_type.as_str()) && ubo_total < 2_500 {
        flags.push("low_ownership_declared".into());
    }
    let mut tx = st.db.begin().await?;
    sqlx::query("UPDATE kyb_profiles SET submitted_at = now(), risk_flags = $2, updated_at = now() WHERE shop_id = $1")
        .bind(shop_id)
        .bind(&flags)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE shops SET kyb_status = 'submitted' WHERE id = $1").bind(shop_id).execute(&mut *tx).await?;
    tx.commit().await?;
    event(&st, shop_id, Some(user.id), "submitted", "").await?;
    let shop = owned_shop(&st, shop_id, &user).await?;
    Ok(Json(view(&st, &shop, false, false).await?))
}

// ---------------------------------------------------------------------------------------------
// Documents
// ---------------------------------------------------------------------------------------------

/// Allowed document formats, detected from the file content (not the name).
pub fn doc_type(data: &[u8]) -> Option<&'static str> {
    match infer::get(data)?.mime_type() {
        "image/jpeg" => Some("image/jpeg"),
        "image/png" => Some("image/png"),
        "image/webp" => Some("image/webp"),
        "application/pdf" => Some("application/pdf"),
        _ => None,
    }
}

pub async fn upload_document(
    State(st): State<AppState>,
    user: AuthUser,
    Path(shop_id): Path<Uuid>,
    mut form: Multipart,
) -> AppResult<Json<DocRow>> {
    let shop = owned_shop(&st, shop_id, &user).await?;
    if shop.entity_type != "business" {
        return Err(AppError::bad("set the shop type to business first"));
    }
    if !editable(&shop.kyb_status) {
        return Err(AppError::Conflict("your verification is under review — wait for the decision before editing".into()));
    }
    let (mut kind, mut expires, mut file) = (String::new(), None::<NaiveDate>, None::<(String, Bytes)>);
    while let Some(field) = form.next_field().await.map_err(|_| AppError::bad("upload interrupted or too large"))? {
        match field.name() {
            Some("kind") => kind = field.text().await.unwrap_or_default().trim().to_string(),
            Some("expires_on") => {
                let t = field.text().await.unwrap_or_default();
                if !t.trim().is_empty() {
                    expires = Some(t.trim().parse().map_err(|_| AppError::bad("expiry date must be YYYY-MM-DD"))?);
                }
            }
            Some("file") => {
                let name: String = field.file_name().unwrap_or("document").chars().filter(|c| !c.is_control()).take(150).collect();
                let data = field.bytes().await.map_err(|_| AppError::bad("upload interrupted or too large"))?;
                file = Some((name, data));
            }
            _ => {}
        }
    }
    if !DOC_KINDS.contains(&kind.as_str()) {
        return Err(AppError::bad("unknown document type"));
    }
    let (name, data) = file.filter(|(_, d)| !d.is_empty()).ok_or_else(|| AppError::bad("no files received (use the 'file' field)"))?;
    if data.len() > MAX_DOC_BYTES {
        return Err(AppError::bad("documents can be at most 10 MB"));
    }
    let ctype = doc_type(&data).ok_or_else(|| AppError::bad("documents must be JPG, PNG, WebP or PDF"))?;
    if expires.is_some_and(|e| e < Utc::now().date_naive()) {
        return Err(AppError::bad("this document has already expired"));
    }
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM kyb_documents WHERE shop_id = $1")
        .bind(shop_id)
        .fetch_one(&st.db)
        .await?;
    if count >= MAX_DOCS {
        return Err(AppError::bad("at most 30 documents — delete old ones first"));
    }
    let sha = hex::encode(Sha256::digest(&data));
    let key = format!("kyb/{shop_id}/{}", Uuid::new_v4());
    let sealed = st.sealer.seal(&data);
    st.private.put(&key, Bytes::from(sealed), "application/octet-stream").await?;
    let doc: DocRow = sqlx::query_as(
        "INSERT INTO kyb_documents (shop_id, kind, storage_key, content_type, size_bytes, sha256, original_name, expires_on, uploaded_by)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)
         RETURNING id, kind, content_type, size_bytes, sha256, original_name, expires_on, status, review_note, created_at",
    )
    .bind(shop_id)
    .bind(&kind)
    .bind(&key)
    .bind(ctype)
    .bind(data.len() as i64)
    .bind(&sha)
    .bind(&name)
    .bind(expires)
    .bind(user.id)
    .fetch_one(&st.db)
    .await?;
    event(&st, shop_id, Some(user.id), "document_uploaded", &kind).await?;
    Ok(Json(doc))
}

async fn doc_with_shop(st: &AppState, id: Uuid) -> AppResult<(Uuid, String, String, String, String, String)> {
    let row: Option<(Uuid, String, String, String, String, String)> = sqlx::query_as(
        "SELECT shop_id, storage_key, content_type, original_name, status, kind FROM kyb_documents WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&st.db)
    .await?;
    row.ok_or(AppError::NotFound)
}

pub async fn delete_document(State(st): State<AppState>, user: AuthUser, Path(id): Path<Uuid>) -> AppResult<Json<Value>> {
    let (shop_id, key, _, _, status, kind) = doc_with_shop(&st, id).await?;
    let shop = owned_shop(&st, shop_id, &user).await?;
    if !editable(&shop.kyb_status) {
        return Err(AppError::Conflict("your verification is under review — wait for the decision before editing".into()));
    }
    if status == "accepted" && !user.is_admin() {
        return Err(AppError::bad("accepted documents can't be deleted — upload a newer one instead"));
    }
    sqlx::query("DELETE FROM kyb_documents WHERE id = $1").bind(id).execute(&st.db).await?;
    st.private.delete(&key).await;
    event(&st, shop_id, Some(user.id), "document_deleted", &kind).await?;
    Ok(Json(json!({ "deleted": true })))
}

/// Decrypted document for the shop owner or an admin. Never cached; admin views are audited.
pub async fn document_file(State(st): State<AppState>, user: AuthUser, Path(id): Path<Uuid>) -> AppResult<Response> {
    let (shop_id, key, ctype, name, _, kind) = doc_with_shop(&st, id).await?;
    owned_shop(&st, shop_id, &user).await?;
    let data = st.sealer.open(&st.private.get(&key).await?)?;
    if user.is_admin() {
        let owner: bool = sqlx::query_scalar("SELECT owner_id = $2 FROM shops WHERE id = $1")
            .bind(shop_id)
            .bind(user.id)
            .fetch_one(&st.db)
            .await?;
        if !owner {
            event(&st, shop_id, Some(user.id), "document_viewed", &kind).await?;
        }
    }
    let safe: String = name.chars().filter(|c| c.is_ascii_alphanumeric() || "._- ".contains(*c)).take(100).collect();
    let mut res = Response::new(Body::from(data));
    let h = res.headers_mut();
    h.insert(header::CONTENT_TYPE, HeaderValue::from_str(&ctype).unwrap_or(HeaderValue::from_static("application/octet-stream")));
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("private, no-store"));
    h.insert(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    h.insert(header::CONTENT_SECURITY_POLICY, HeaderValue::from_static("default-src 'none'; img-src 'self' data:; style-src 'unsafe-inline'; sandbox"));
    if let Ok(v) = HeaderValue::from_str(&format!("inline; filename=\"{}\"", if safe.is_empty() { "document".into() } else { safe })) {
        h.insert(header::CONTENT_DISPOSITION, v);
    }
    Ok(res)
}

// ---------------------------------------------------------------------------------------------
// Admin
// ---------------------------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct QueueQ {
    pub status: Option<String>,
    pub q: Option<String>,
}

pub async fn admin_queue(State(st): State<AppState>, _a: AdminUser, Query(q): Query<QueueQ>) -> AppResult<Json<Value>> {
    let status = q.status.as_deref().map(str::trim).filter(|s| !s.is_empty() && *s != "all").map(str::to_string);
    let rows: Vec<Value> = sqlx::query_scalar(
        "SELECT jsonb_build_object('shop_id', s.id, 'shop_name', s.name, 'slug', s.slug, 'kyb_status', s.kyb_status,
                'kyb_verified_at', s.kyb_verified_at, 'owner', COALESCE(u.email, u.phone, u.display_name),
                'legal_name', k.legal_name, 'company_type', k.company_type, 'registration_no', k.registration_no,
                'country', k.country, 'submitted_at', k.submitted_at, 'reviewed_at', k.reviewed_at,
                'review_due_at', k.review_due_at, 'risk_flags', COALESCE(k.risk_flags, '{}'),
                'documents', (SELECT COUNT(*) FROM kyb_documents d WHERE d.shop_id = s.id))
         FROM shops s JOIN users u ON u.id = s.owner_id LEFT JOIN kyb_profiles k ON k.shop_id = s.id
         WHERE s.kyb_status <> 'none'
           AND ($1::text IS NULL OR s.kyb_status = $1)
           AND ($2::text IS NULL OR (s.name || ' ' || COALESCE(k.legal_name, '') || ' ' || COALESCE(k.registration_no, '')) ILIKE '%' || $2 || '%')
         ORDER BY (s.kyb_status = 'submitted') DESC, k.submitted_at ASC NULLS LAST, s.name
         LIMIT 500",
    )
    .bind(status)
    .bind(q.q.as_deref().map(str::trim).filter(|s| !s.is_empty()))
    .fetch_all(&st.db)
    .await?;
    let counts: Vec<(String, i64)> = sqlx::query_as("SELECT kyb_status, COUNT(*) FROM shops WHERE kyb_status <> 'none' GROUP BY 1")
        .fetch_all(&st.db)
        .await?;
    let counts: serde_json::Map<String, Value> = counts.into_iter().map(|(k, n)| (k, json!(n))).collect();
    Ok(Json(json!({ "items": rows, "counts": counts })))
}

pub async fn admin_detail(State(st): State<AppState>, AdminUser(a): AdminUser, Path(shop_id): Path<Uuid>) -> AppResult<Json<Value>> {
    let shop: Shop = sqlx::query_as("SELECT * FROM shops WHERE id = $1")
        .bind(shop_id)
        .fetch_optional(&st.db)
        .await?
        .ok_or(AppError::NotFound)?;
    // Audit who looked at identity data (at most once per admin per 10 minutes).
    let recent: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM kyb_events WHERE shop_id = $1 AND actor_id = $2 AND action = 'viewed' AND created_at > now() - interval '10 minutes')",
    )
    .bind(shop_id)
    .bind(a.id)
    .fetch_one(&st.db)
    .await?;
    if !recent {
        event(&st, shop_id, Some(a.id), "viewed", "").await?;
    }
    let mut v = view(&st, &shop, true, true).await?;
    let owner: Option<(String, Option<String>, Option<String>)> =
        sqlx::query_as("SELECT display_name, email, phone FROM users WHERE id = $1").bind(shop.owner_id).fetch_optional(&st.db).await?;
    v["owner"] = json!(owner.map(|(n, e, p)| json!({ "display_name": n, "email": e, "phone": p })));
    Ok(Json(v))
}

#[derive(Deserialize)]
pub struct DocDecision {
    pub id: Uuid,
    pub status: String,
    #[serde(default)]
    pub note: String,
}

#[derive(Deserialize)]
pub struct Decision {
    pub action: String,
    #[serde(default)]
    pub note: String,
    #[serde(default)]
    pub documents: Vec<DocDecision>,
}

pub async fn admin_decide(
    State(st): State<AppState>,
    AdminUser(a): AdminUser,
    Path(shop_id): Path<Uuid>,
    Json(d): Json<Decision>,
) -> AppResult<Json<Value>> {
    let shop: Shop = sqlx::query_as("SELECT * FROM shops WHERE id = $1")
        .bind(shop_id)
        .fetch_optional(&st.db)
        .await?
        .ok_or(AppError::NotFound)?;
    let note = d.note.trim().to_string();
    if note.chars().count() > 2000 {
        return Err(AppError::bad("note is too long"));
    }
    match d.action.as_str() {
        "approve" | "request_changes" | "reject" if shop.kyb_status != "submitted" => {
            return Err(AppError::bad("only submitted verifications can be decided"))
        }
        "revoke" if shop.kyb_verified_at.is_none() => return Err(AppError::bad("this shop is not verified")),
        "request_changes" | "reject" | "revoke" if note.is_empty() => {
            return Err(AppError::bad("add a note explaining the decision to the seller"))
        }
        "approve" | "request_changes" | "reject" | "revoke" => {}
        _ => return Err(AppError::bad("action must be approve, request_changes, reject or revoke")),
    }
    let mut tx = st.db.begin().await?;
    for dd in &d.documents {
        if !matches!(dd.status.as_str(), "accepted" | "rejected" | "pending") {
            return Err(AppError::bad("document status must be accepted, rejected or pending"));
        }
        if dd.status == "rejected" && dd.note.trim().is_empty() {
            return Err(AppError::bad("say why the document is rejected"));
        }
        sqlx::query("UPDATE kyb_documents SET status = $3, review_note = $4 WHERE id = $1 AND shop_id = $2")
            .bind(dd.id)
            .bind(shop_id)
            .bind(&dd.status)
            .bind(dd.note.trim())
            .execute(&mut *tx)
            .await?;
    }
    match d.action.as_str() {
        "approve" => {
            let docs: Vec<DocRow> = sqlx::query_as("SELECT * FROM kyb_documents WHERE shop_id = $1").bind(shop_id).fetch_all(&mut *tx).await?;
            let profile: Option<ProfileRow> = sqlx::query_as("SELECT * FROM kyb_profiles WHERE shop_id = $1").bind(shop_id).fetch_optional(&mut *tx).await?;
            let persons: Vec<PersonRow> = sqlx::query_as("SELECT * FROM kyb_persons WHERE shop_id = $1").bind(shop_id).fetch_all(&mut *tx).await?;
            if !missing(profile.as_ref(), &persons, &docs, Utc::now().date_naive()).is_empty() {
                return Err(AppError::bad("required documents are rejected or missing — request changes instead"));
            }
            sqlx::query("UPDATE kyb_documents SET status = 'accepted' WHERE shop_id = $1 AND status = 'pending'")
                .bind(shop_id)
                .execute(&mut *tx)
                .await?;
            let due = Utc::now().checked_add_months(Months::new(st.cfg.kyb_review_months)).unwrap_or_else(Utc::now);
            sqlx::query("UPDATE kyb_profiles SET reviewed_at = now(), reviewed_by = $2, review_note = $3, review_due_at = $4 WHERE shop_id = $1")
                .bind(shop_id)
                .bind(a.id)
                .bind(&note)
                .bind(due)
                .execute(&mut *tx)
                .await?;
            sqlx::query("UPDATE shops SET kyb_status = 'approved', kyb_verified_at = now() WHERE id = $1").bind(shop_id).execute(&mut *tx).await?;
        }
        "request_changes" | "reject" => {
            let next = if d.action == "reject" { "rejected" } else { "changes_requested" };
            sqlx::query("UPDATE kyb_profiles SET reviewed_at = now(), reviewed_by = $2, review_note = $3 WHERE shop_id = $1")
                .bind(shop_id)
                .bind(a.id)
                .bind(&note)
                .execute(&mut *tx)
                .await?;
            // A verified shop whose update is sent back stays verified.
            sqlx::query("UPDATE shops SET kyb_status = CASE WHEN kyb_verified_at IS NOT NULL AND $2 = 'rejected' THEN 'approved' ELSE $2 END WHERE id = $1")
                .bind(shop_id)
                .bind(next)
                .execute(&mut *tx)
                .await?;
        }
        _ => {
            // revoke: remove verification and take the shop's products off sale until it is verified again.
            sqlx::query("UPDATE kyb_profiles SET reviewed_at = now(), reviewed_by = $2, review_note = $3 WHERE shop_id = $1")
                .bind(shop_id)
                .bind(a.id)
                .bind(&note)
                .execute(&mut *tx)
                .await?;
            sqlx::query("UPDATE shops SET kyb_status = 'revoked', kyb_verified_at = NULL WHERE id = $1").bind(shop_id).execute(&mut *tx).await?;
            // Each core pauses what the shop sells (commerce: products back to not-submitted).
            if st.cfg.kyb_required_to_publish {
                for hook in &st.reg.shop_hooks {
                    hook.verification_revoked(&mut *tx, &st, shop_id).await?;
                }
            }
        }
    }
    sqlx::query("INSERT INTO kyb_events (shop_id, actor_id, action, note) VALUES ($1,$2,$3,$4)")
        .bind(shop_id)
        .bind(a.id)
        .bind(match d.action.as_str() {
            "approve" => "approved",
            "request_changes" => "changes_requested",
            "reject" => "rejected",
            _ => "revoked",
        })
        .bind(&note)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    let shop: Shop = sqlx::query_as("SELECT * FROM shops WHERE id = $1").bind(shop_id).fetch_one(&st.db).await?;
    Ok(Json(view(&st, &shop, true, true).await?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(company_type: &str) -> ProfileRow {
        ProfileRow {
            company_type: company_type.into(), legal_name: "Siam Crafts Co., Ltd.".into(), legal_name_local: String::new(),
            registration_no: "01-00012345".into(), registration_date: None, tax_id: "123456789".into(), country: "LA".into(),
            province: String::new(), registered_address: "Vientiane".into(), business_activity: "Retail".into(),
            website: String::new(), contact_email: String::new(), contact_phone: "+85620".into(), bank_name: "BCEL".into(),
            bank_account_name: "Siam Crafts".into(), bank_account_enc: "k1:x".into(), bank_account_last4: "0001".into(),
            risk_flags: vec![], submitted_at: None, reviewed_at: None, review_note: String::new(), review_due_at: None,
            updated_at: Utc::now(),
        }
    }
    fn person(roles: &[&str], bps: i32) -> PersonRow {
        PersonRow {
            id: Uuid::new_v4(), roles: roles.iter().map(|s| s.to_string()).collect(), full_name: "A".into(), title: String::new(),
            nationality: "LA".into(), date_of_birth: None, id_type: "national_id".into(), id_number_enc: "k1:x".into(),
            id_last4: "1234".into(), ownership_bps: bps, is_pep: false,
        }
    }
    fn doc(kind: &str, status: &str, expires_on: Option<NaiveDate>) -> DocRow {
        DocRow {
            id: Uuid::new_v4(), kind: kind.into(), content_type: "application/pdf".into(), size_bytes: 1, sha256: String::new(),
            original_name: String::new(), expires_on, status: status.into(), review_note: String::new(), created_at: Utc::now(),
        }
    }

    #[test]
    fn completeness() {
        let today = NaiveDate::from_ymd_opt(2026, 9, 28).unwrap();
        assert_eq!(missing(None, &[], &[], today), vec!["company"]);
        let base = ["registration_certificate", "tax_certificate", "representative_id"];
        let docs: Vec<DocRow> = base.iter().map(|k| doc(k, "pending", None)).collect();
        // Sole enterprise: representative who is also director, no UBO/register needed.
        assert!(missing(Some(&profile("sole_enterprise")), &[person(&["representative", "director"], 0)], &docs, today).is_empty());
        // Limited company: needs a beneficial owner and the shareholder register.
        let m = missing(Some(&profile("limited_company")), &[person(&["representative", "director"], 0)], &docs, today);
        assert_eq!(m, vec!["ubo", "doc:shareholder_register"]);
        // Representative who is not a director needs an authorization letter; >100% ownership flagged.
        let ppl = [person(&["representative"], 0), person(&["director", "ubo"], 6000), person(&["ubo"], 5000)];
        let mut d2 = docs.clone();
        d2.push(doc("shareholder_register", "accepted", None));
        assert_eq!(missing(Some(&profile("limited_company")), &ppl, &d2, today), vec!["ownership_total", "doc:authorization_letter"]);
        // Rejected or expired documents don't count; missing ID numbers are reported.
        let mut d3 = docs.clone();
        d3[0].status = "rejected".into();
        d3[1].expires_on = NaiveDate::from_ymd_opt(2026, 1, 1);
        let mut p = person(&["representative", "director"], 0);
        p.id_number_enc.clear();
        assert_eq!(
            missing(Some(&profile("sole_enterprise")), &[p], &d3, today),
            vec!["person_id_number", "doc:registration_certificate", "doc:tax_certificate"]
        );
        let mut pr = profile("sole_enterprise");
        pr.bank_account_enc.clear();
        pr.tax_id.clear();
        let two_reps = [person(&["representative"], 0), person(&["representative", "director"], 0)];
        let m = missing(Some(&pr), &two_reps, &docs, today);
        assert!(m.contains(&"tax_id".into()) && m.contains(&"bank_account_number".into()) && m.contains(&"one_representative".into()));
    }

    #[test]
    fn formats() {
        assert_eq!(id_number(" p 1234 567 ").unwrap(), "P 1234 567");
        assert!(id_number("12").is_err());
        assert!(id_number("12345<script>").is_err());
        assert_eq!(country("th").unwrap(), "TH");
        assert_eq!(country("").unwrap(), "LA");
        assert!(country("LAO").is_err());
        assert_eq!(doc_type(b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n1 0 obj"), Some("application/pdf"));
        assert_eq!(doc_type(b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR"), Some("image/png"));
        assert_eq!(doc_type(b"MZ\x90\0\x03\0\0\0"), None);
        assert_eq!(doc_type(b"<html><script>"), None);
    }
}
