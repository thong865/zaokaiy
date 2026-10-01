//! The rules: which coupon/points a sale may use, what they take off, and the bookkeeping
//! (use, reverse, earn, grant). Every function here runs inside the caller's transaction.

use rand::Rng;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgConnection};
use uuid::Uuid;

use crate::error::{AppError, AppResult};

/// web (cart checkout) · social (comment/chat order link) · pos (counter).
pub const CHANNELS: [&str; 3] = ["web", "social", "pos"];

/// What the customer asks to use.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PromoReq {
    pub code: Option<String>,
    #[serde(default)]
    pub points: i64,
}

impl PromoReq {
    pub fn code(&self) -> Option<String> {
        self.code.as_deref().map(|c| c.trim().to_uppercase()).filter(|c| !c.is_empty())
    }
    pub fn is_empty(&self) -> bool {
        self.code().is_none() && self.points <= 0
    }
}

/// The sale being priced.
#[derive(Debug, Clone)]
pub struct Ctx {
    pub channel: &'static str,
    pub shop_id: Uuid,
    pub currency: String,
    /// Signed-in customer (web, or the chat-order page when signed in).
    pub user_id: Option<Uuid>,
    /// Customer phone in E.164 (shipping phone / member phone at the counter).
    pub phone: Option<String>,
    /// May spend this phone's points: the phone is the signed-in user's verified phone, or a cashier is serving the customer.
    pub points_trusted: bool,
    /// Goods after line discounts.
    pub subtotal_cents: i64,
    /// Shipping the customer pays at checkout (what a free-shipping coupon can take off).
    pub shipping_cents: i64,
}

/// What was applied. `discount_cents = coupon_cents + points_cents`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Applied {
    pub shop_id: Option<Uuid>,
    pub coupon_id: Option<Uuid>,
    pub coupon_code: Option<String>,
    pub coupon_type: Option<String>,
    pub coupon_cents: i64,
    /// Part of the discount the platform pays for (platform coupons).
    pub platform_cents: i64,
    #[serde(skip)]
    pub member_id: Option<Uuid>,
    pub points: i64,
    pub points_cents: i64,
    pub discount_cents: i64,
    /// Points balance before this sale (when the customer is a member).
    pub points_balance: Option<i64>,
    /// Campaign to create the customer's coupon from (public code campaigns), made on commit.
    #[serde(skip)]
    pub new_from_campaign: Option<Uuid>,
}

impl Applied {
    pub fn is_empty(&self) -> bool {
        self.coupon_id.is_none() && self.new_from_campaign.is_none() && self.points == 0
    }
}

#[derive(Debug, Clone, FromRow)]
struct Coupon {
    id: Uuid,
    shop_id: Option<Uuid>,
    user_id: Option<Uuid>,
    phone: Option<String>,
    code: String,
    reward_type: String,
    reward_value: i64,
    max_discount_cents: i64,
    min_spend_cents: i64,
    currency: String,
    channels: Vec<String>,
    status: String,
    expired: bool,
    owner_phone: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
pub struct Settings {
    pub enabled: bool,
    pub earn_per_cents: i64,
    pub point_value_cents: i64,
    pub min_redeem_points: i64,
    pub max_redeem_bps: i32,
    pub expiry_days: i32,
    pub channels: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self { enabled: false, earn_per_cents: 1_000_000, point_value_cents: 10_000, min_redeem_points: 10, max_redeem_bps: 5000, expiry_days: 0, channels: CHANNELS.map(String::from).to_vec() }
    }
}

pub async fn settings(conn: &mut PgConnection, shop_id: Uuid) -> AppResult<Settings> {
    Ok(sqlx::query_as(
        "SELECT enabled, earn_per_cents, point_value_cents, min_redeem_points, max_redeem_bps, expiry_days, channels
         FROM loyalty_settings WHERE shop_id = $1",
    )
    .bind(shop_id)
    .fetch_optional(conn)
    .await?
    .unwrap_or_default())
}

/// Discount a coupon-type reward gives on this sale.
pub fn reward_cents(kind: &str, value: i64, cap: i64, subtotal: i64, shipping: i64) -> i64 {
    match kind {
        "amount" => value.min(subtotal),
        "percent" => {
            let d = subtotal * value.clamp(0, 10_000) / 10_000;
            if cap > 0 { d.min(cap) } else { d }
        }
        "free_shipping" => shipping,
        _ => 0,
    }
    .max(0)
}

/// Points that can be spent: at most the balance and `max_redeem_bps` of what's left after the coupon.
pub fn usable_points(want: i64, balance: i64, s: &Settings, remaining_cents: i64) -> i64 {
    let cap_cents = remaining_cents.max(0) * s.max_redeem_bps as i64 / 10_000;
    want.min(balance).min(cap_cents / s.point_value_cents).max(0)
}

pub fn points_for(net_cents: i64, s: &Settings) -> i64 {
    if net_cents <= 0 { 0 } else { net_cents / s.earn_per_cents }
}

/// Unambiguous personal coupon code: ZK + 8 characters (no 0/O/1/I).
pub fn new_code() -> String {
    const A: &[u8] = b"23456789ABCDEFGHJKLMNPQRSTUVWXYZ";
    let mut r = rand::thread_rng();
    format!("ZK{}", (0..8).map(|_| A[r.gen_range(0..A.len())] as char).collect::<String>())
}

const COUPON_COLS: &str = "c.id, c.shop_id, c.user_id, c.phone, c.code, c.reward_type, c.reward_value, c.max_discount_cents,
    c.min_spend_cents, c.currency, c.channels, c.status, c.expires_at <= now() AS expired, u.phone AS owner_phone";

/// Prices the request for one sale. With `lock`, the coupon and member rows are locked for the commit that follows.
pub async fn apply(conn: &mut PgConnection, ctx: &Ctx, req: &PromoReq, lock: bool) -> AppResult<Applied> {
    let mut out = Applied { shop_id: Some(ctx.shop_id), ..Default::default() };
    let lock_sql = if lock { " FOR UPDATE OF c" } else { "" };
    if let Some(code) = req.code() {
        let coupon: Option<Coupon> = sqlx::query_as(&format!(
            "SELECT {COUPON_COLS} FROM promo_coupons c LEFT JOIN users u ON u.id = c.user_id WHERE c.code = $1{lock_sql}"
        ))
        .bind(&code)
        .fetch_optional(&mut *conn)
        .await?;
        let (kind, value, cap, min_spend, platform) = match coupon {
            Some(c) => {
                check_coupon(&c, ctx)?;
                out.coupon_id = Some(c.id);
                out.coupon_code = Some(c.code.clone());
                (c.reward_type, c.reward_value, c.max_discount_cents, c.min_spend_cents, c.shop_id.is_none())
            }
            None => {
                let camp = code_campaign(conn, ctx, &code).await?;
                out.new_from_campaign = Some(camp.id);
                out.coupon_code = Some(code.clone());
                (camp.reward_type, camp.reward_value, camp.max_discount_cents, camp.min_spend_cents, camp.shop_id.is_none())
            }
        };
        if ctx.subtotal_cents < min_spend {
            return Err(AppError::bad("the order is below this coupon's minimum spend"));
        }
        let cents = reward_cents(&kind, value, cap, ctx.subtotal_cents, ctx.shipping_cents);
        if kind == "free_shipping" && cents == 0 {
            return Err(AppError::bad("this coupon gives free shipping — choose a delivery option with shipping paid at checkout"));
        }
        out.coupon_type = Some(kind);
        out.coupon_cents = cents;
        out.platform_cents = if platform { cents } else { 0 };
    }
    // Points (per shop, keyed by phone).
    let s = settings(conn, ctx.shop_id).await?;
    let member: Option<(Uuid, i64)> = match (&ctx.phone, s.enabled) {
        (Some(p), true) => sqlx::query_as(&format!(
            "SELECT id, balance FROM loyalty_members WHERE shop_id = $1 AND phone = $2{}",
            if lock { " FOR UPDATE" } else { "" }
        ))
        .bind(ctx.shop_id)
        .bind(p)
        .fetch_optional(&mut *conn)
        .await?,
        _ => None,
    };
    out.points_balance = member.map(|m| m.1);
    if req.points > 0 {
        if !s.enabled || !s.channels.iter().any(|c| c == ctx.channel) {
            return Err(AppError::bad("this shop doesn't take loyalty points here"));
        }
        if !ctx.points_trusted {
            return Err(AppError::bad("sign in with your member phone number to use points"));
        }
        let Some((member_id, balance)) = member else {
            return Err(AppError::bad("no points yet for this phone number"));
        };
        if req.points < s.min_redeem_points {
            return Err(AppError::bad(format!("use at least {} points", s.min_redeem_points)));
        }
        let goods_left = ctx.subtotal_cents - if out.coupon_type.as_deref() == Some("free_shipping") { 0 } else { out.coupon_cents };
        let pts = usable_points(req.points, balance, &s, goods_left);
        if pts < s.min_redeem_points {
            return Err(AppError::bad("not enough points for this order"));
        }
        out.member_id = Some(member_id);
        out.points = pts;
        out.points_cents = pts * s.point_value_cents;
    }
    out.discount_cents = (out.coupon_cents + out.points_cents).min(ctx.subtotal_cents + ctx.shipping_cents);
    Ok(out)
}

fn check_coupon(c: &Coupon, ctx: &Ctx) -> AppResult<()> {
    if c.status == "used" {
        return Err(AppError::bad("this coupon has already been used"));
    }
    if c.status != "active" || c.expired {
        return Err(AppError::bad("this coupon has expired"));
    }
    if c.shop_id.is_some_and(|s| s != ctx.shop_id) {
        return Err(AppError::bad("this coupon is for another shop"));
    }
    if !c.channels.iter().any(|x| x == ctx.channel) {
        return Err(AppError::bad("this coupon can't be used here"));
    }
    if c.currency != ctx.currency {
        return Err(AppError::bad("this coupon is for another currency"));
    }
    // Personal coupon: the holder's account, or their phone number. At the counter the customer shows the code.
    let phone_ok = |p: &Option<String>| p.is_some() && *p == ctx.phone;
    let mine = ctx.channel == "pos" || (c.user_id.is_some() && c.user_id == ctx.user_id) || phone_ok(&c.phone) || phone_ok(&c.owner_phone);
    if !mine {
        return Err(AppError::bad("this coupon belongs to another customer"));
    }
    Ok(())
}

#[derive(Debug, FromRow)]
struct CodeCampaign {
    id: Uuid,
    shop_id: Option<Uuid>,
    reward_type: String,
    reward_value: i64,
    max_discount_cents: i64,
    min_spend_cents: i64,
}

/// A public code campaign (`LIVE10`): each customer may use it once.
async fn code_campaign(conn: &mut PgConnection, ctx: &Ctx, code: &str) -> AppResult<CodeCampaign> {
    let camp: Option<(CodeCampaign, bool, Vec<String>, String, bool)> = sqlx::query_as::<_, (Uuid, Option<Uuid>, String, i64, i64, i64, bool, Vec<String>, String, bool)>(
        "SELECT id, shop_id, reward_type, reward_value, max_discount_cents, min_spend_cents,
                active AND starts_at <= now() AND (ends_at IS NULL OR ends_at > now()) AND (max_grants = 0 OR grants < max_grants),
                channels, currency, shop_id IS NULL
         FROM promo_campaigns
         WHERE trigger = 'code' AND code = $1 AND (shop_id IS NULL OR shop_id = $2)
         ORDER BY shop_id NULLS LAST LIMIT 1",
    )
    .bind(code)
    .bind(ctx.shop_id)
    .fetch_optional(&mut *conn)
    .await?
    .map(|(id, shop_id, reward_type, reward_value, max_discount_cents, min_spend_cents, live, ch, cur, platform)| {
        (CodeCampaign { id, shop_id, reward_type, reward_value, max_discount_cents, min_spend_cents }, live, ch, cur, platform)
    });
    let Some((c, live, channels, currency, platform)) = camp else {
        return Err(AppError::bad("coupon code not found"));
    };
    if !live {
        return Err(AppError::bad("this promotion has ended"));
    }
    if !channels.iter().any(|x| x == ctx.channel) {
        return Err(AppError::bad("this coupon can't be used here"));
    }
    if platform && currency != ctx.currency {
        return Err(AppError::bad("this coupon is for another currency"));
    }
    if ctx.user_id.is_none() && ctx.phone.is_none() {
        return Err(AppError::bad("enter your phone number to use a code"));
    }
    let used: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM promo_coupons c LEFT JOIN users u ON u.id = c.user_id
                        WHERE c.campaign_id = $1 AND ((c.user_id IS NOT NULL AND c.user_id = $2) OR c.phone = $3 OR u.phone = $3))",
    )
    .bind(c.id)
    .bind(ctx.user_id)
    .bind(&ctx.phone)
    .fetch_one(&mut *conn)
    .await?;
    if used {
        return Err(AppError::bad("you have already used this code"));
    }
    Ok(c)
}

/// Consumes what `apply` priced, for sale `ref_type/ref_id`. Call in the sale's transaction.
pub async fn commit(conn: &mut PgConnection, ctx: &Ctx, a: &Applied, ref_type: &str, ref_id: Uuid, by: Option<Uuid>) -> AppResult<()> {
    if a.is_empty() {
        return Ok(());
    }
    let mut coupon_id = a.coupon_id;
    if let Some(camp) = a.new_from_campaign {
        // Count it against the campaign budget and give this customer their (immediately used) coupon.
        take_budget(conn, camp).await?.then_some(()).ok_or_else(|| AppError::bad("this promotion has ended"))?;
        let id: Uuid = sqlx::query_scalar(
            "INSERT INTO promo_coupons (campaign_id, shop_id, user_id, phone, code, reward_type, reward_value, max_discount_cents,
                                        min_spend_cents, currency, channels, status, expires_at, used_at)
             SELECT id, shop_id, $2, $3, $4, reward_type, reward_value, max_discount_cents, min_spend_cents,
                    CASE WHEN shop_id IS NULL THEN currency ELSE $5 END, channels, 'used', now(), now()
             FROM promo_campaigns WHERE id = $1 RETURNING id",
        )
        .bind(camp)
        .bind(ctx.user_id)
        .bind(if ctx.user_id.is_some() { None } else { ctx.phone.clone() })
        .bind(format!("{}-{}", a.coupon_code.clone().unwrap_or_default(), new_code()))
        .bind(&ctx.currency)
        .fetch_one(&mut *conn)
        .await
        .map_err(|e| match AppError::from(e) {
            AppError::Conflict(_) => AppError::bad("you have already used this code"),
            other => other,
        })?;
        coupon_id = Some(id);
    } else if let Some(id) = a.coupon_id {
        let ok: Option<Uuid> = sqlx::query_scalar("UPDATE promo_coupons SET status = 'used', used_at = now() WHERE id = $1 AND status = 'active' RETURNING id")
            .bind(id)
            .fetch_optional(&mut *conn)
            .await?;
        if ok.is_none() {
            return Err(AppError::bad("this coupon has already been used"));
        }
    }
    if let (Some(m), true) = (a.member_id, a.points > 0) {
        let bal: Option<i64> = sqlx::query_scalar(
            "UPDATE loyalty_members SET balance = balance - $2, last_activity_at = now() WHERE id = $1 AND balance >= $2 RETURNING balance",
        )
        .bind(m)
        .bind(a.points)
        .fetch_optional(&mut *conn)
        .await?;
        let bal = bal.ok_or_else(|| AppError::bad("not enough points for this order"))?;
        ledger(conn, m, -a.points, bal, "redeem", Some((ref_type, ref_id)), "", by).await?;
    }
    sqlx::query(
        "INSERT INTO promo_redemptions (ref_type, ref_id, shop_id, coupon_id, member_id, points, discount_cents, platform_cents, currency, coupon_cents)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)
         ON CONFLICT (ref_type, ref_id) DO UPDATE SET coupon_id = EXCLUDED.coupon_id, member_id = EXCLUDED.member_id,
             points = EXCLUDED.points, discount_cents = EXCLUDED.discount_cents, platform_cents = EXCLUDED.platform_cents,
             coupon_cents = EXCLUDED.coupon_cents, reversed_at = NULL, created_at = now()",
    )
    .bind(ref_type)
    .bind(ref_id)
    .bind(ctx.shop_id)
    .bind(coupon_id)
    .bind(a.member_id)
    .bind(a.points)
    .bind(a.discount_cents)
    .bind(a.platform_cents)
    .bind(&ctx.currency)
    .bind(a.coupon_cents)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

/// Sale cancelled / voided / refused: give the coupon and points back, and take back points it earned.
pub async fn reverse(conn: &mut PgConnection, ref_type: &str, ref_id: Uuid, by: Option<Uuid>) -> AppResult<()> {
    let r: Option<(Uuid, Option<Uuid>, Option<Uuid>, i64)> = sqlx::query_as(
        "SELECT id, coupon_id, member_id, points FROM promo_redemptions
         WHERE ref_type = $1 AND ref_id = $2 AND reversed_at IS NULL FOR UPDATE",
    )
    .bind(ref_type)
    .bind(ref_id)
    .fetch_optional(&mut *conn)
    .await?;
    if let Some((id, coupon, member, points)) = r {
        if let Some(c) = coupon {
            let trigger: Option<(Uuid, String)> = sqlx::query_as(
                "SELECT p.id, p.trigger FROM promo_coupons c JOIN promo_campaigns p ON p.id = c.campaign_id WHERE c.id = $1",
            )
            .bind(c)
            .fetch_optional(&mut *conn)
            .await?;
            match trigger {
                // Made from a public code at this sale: drop it, so the customer can use the code again.
                Some((camp, t)) if t == "code" => {
                    sqlx::query("UPDATE promo_campaigns SET grants = greatest(grants - 1, 0) WHERE id = $1").bind(camp).execute(&mut *conn).await?;
                    sqlx::query("DELETE FROM promo_coupons WHERE id = $1").bind(c).execute(&mut *conn).await?;
                }
                Some(_) => {
                    sqlx::query("UPDATE promo_coupons SET status = 'active', used_at = NULL WHERE id = $1").bind(c).execute(&mut *conn).await?;
                }
                None => {}
            }
        }
        if let (Some(m), true) = (member, points > 0) {
            let bal: i64 = sqlx::query_scalar("UPDATE loyalty_members SET balance = balance + $2 WHERE id = $1 RETURNING balance")
                .bind(m)
                .bind(points)
                .fetch_one(&mut *conn)
                .await?;
            ledger(conn, m, points, bal, "reverse", Some((ref_type, ref_id)), "points returned", by).await?;
        }
        sqlx::query("UPDATE promo_redemptions SET reversed_at = now() WHERE id = $1").bind(id).execute(&mut *conn).await?;
    }
    // Points this sale earned.
    let earned: Option<(Uuid, i64)> = sqlx::query_as(
        "SELECT l.member_id, l.delta FROM loyalty_ledger l
         WHERE l.ref_type = $1 AND l.ref_id = $2 AND l.reason = 'earn'
           AND NOT EXISTS (SELECT 1 FROM loyalty_ledger r WHERE r.ref_type = $1 AND r.ref_id = $2 AND r.reason = 'reverse' AND r.delta < 0)",
    )
    .bind(ref_type)
    .bind(ref_id)
    .fetch_optional(&mut *conn)
    .await?;
    if let Some((m, pts)) = earned {
        let (bal,): (i64,) = sqlx::query_as("SELECT balance FROM loyalty_members WHERE id = $1 FOR UPDATE").bind(m).fetch_one(&mut *conn).await?;
        let take = pts.min(bal);
        let bal: i64 = sqlx::query_scalar(
            "UPDATE loyalty_members SET balance = balance - $2, lifetime_points = lifetime_points - $2, orders = greatest(orders - 1, 0)
             WHERE id = $1 RETURNING balance",
        )
        .bind(m)
        .bind(take)
        .fetch_one(&mut *conn)
        .await?;
        ledger(conn, m, -take, bal, "reverse", Some((ref_type, ref_id)), "sale cancelled", by).await?;
    }
    Ok(())
}

/// A completed sale earns points (once) and, on a member's first order, runs the shop's first-order campaigns.
#[allow(clippy::too_many_arguments)]
pub async fn earn(
    conn: &mut PgConnection,
    shop_id: Uuid,
    channel: &str,
    phone_raw: &str,
    name: &str,
    net_cents: i64,
    ref_type: &str,
    ref_id: Uuid,
) -> AppResult<Option<i64>> {
    let s = settings(conn, shop_id).await?;
    let Some(phone) = super::normalize_phone(phone_raw) else { return Ok(None) };
    if !s.enabled || !s.channels.iter().any(|c| c == channel) {
        return Ok(None);
    }
    let already: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM loyalty_ledger WHERE ref_type = $1 AND ref_id = $2 AND reason = 'earn')")
        .bind(ref_type)
        .bind(ref_id)
        .fetch_one(&mut *conn)
        .await?;
    if already {
        return Ok(None);
    }
    let (member, orders): (Uuid, i32) = sqlx::query_as(
        "INSERT INTO loyalty_members (shop_id, phone, name) VALUES ($1, $2, $3)
         ON CONFLICT (shop_id, phone) DO UPDATE SET name = CASE WHEN loyalty_members.name = '' THEN EXCLUDED.name ELSE loyalty_members.name END
         RETURNING id, orders",
    )
    .bind(shop_id)
    .bind(&phone)
    .bind(name.trim())
    .fetch_one(&mut *conn)
    .await?;
    let pts = points_for(net_cents, &s);
    let bal: i64 = sqlx::query_scalar(
        "UPDATE loyalty_members SET balance = balance + $2, lifetime_points = lifetime_points + $2, orders = orders + 1, last_activity_at = now()
         WHERE id = $1 RETURNING balance",
    )
    .bind(member)
    .bind(pts)
    .fetch_one(&mut *conn)
    .await?;
    // Written even for 0 points, so the order is only counted once.
    ledger(conn, member, pts, bal, "earn", Some((ref_type, ref_id)), "", None).await?;
    if orders == 0 {
        first_order_rewards(conn, shop_id, channel, member, &phone).await?;
    }
    Ok(Some(pts))
}

async fn first_order_rewards(conn: &mut PgConnection, shop_id: Uuid, channel: &str, member: Uuid, phone: &str) -> AppResult<()> {
    let camps: Vec<(Uuid, String, i64, i32)> = sqlx::query_as(
        "SELECT id, reward_type, reward_value, coupon_days FROM promo_campaigns
         WHERE shop_id = $1 AND trigger = 'first_order' AND active AND starts_at <= now() AND (ends_at IS NULL OR ends_at > now())
           AND $2 = ANY(channels)",
    )
    .bind(shop_id)
    .bind(channel)
    .fetch_all(&mut *conn)
    .await?;
    for (id, kind, value, _) in camps {
        if kind == "points" {
            if !take_budget(conn, id).await? {
                continue;
            }
            let bal: i64 = sqlx::query_scalar("UPDATE loyalty_members SET balance = balance + $2, lifetime_points = lifetime_points + $2 WHERE id = $1 RETURNING balance")
                .bind(member)
                .bind(value)
                .fetch_one(&mut *conn)
                .await?;
            ledger(conn, member, value, bal, "bonus", None, &format!("campaign {id}"), None).await?;
        } else {
            grant_coupon(conn, id, None, Some(phone)).await?;
        }
    }
    Ok(())
}

/// One more grant from the campaign's budget (false when it's used up).
pub async fn take_budget(conn: &mut PgConnection, campaign: Uuid) -> AppResult<bool> {
    Ok(sqlx::query_scalar::<_, Uuid>(
        "UPDATE promo_campaigns SET grants = grants + 1 WHERE id = $1 AND (max_grants = 0 OR grants < max_grants) RETURNING id",
    )
    .bind(campaign)
    .fetch_optional(conn)
    .await?
    .is_some())
}

/// Gives a customer a coupon from a campaign (once per customer). Returns the code when granted.
pub async fn grant_coupon(conn: &mut PgConnection, campaign: Uuid, user_id: Option<Uuid>, phone: Option<&str>) -> AppResult<Option<String>> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM promo_coupons WHERE campaign_id = $1 AND ((user_id IS NOT NULL AND user_id = $2) OR (user_id IS NULL AND phone = $3)))",
    )
    .bind(campaign)
    .bind(user_id)
    .bind(phone)
    .fetch_one(&mut *conn)
    .await?;
    if exists || !take_budget(conn, campaign).await? {
        return Ok(None);
    }
    let code = new_code();
    sqlx::query(
        "INSERT INTO promo_coupons (campaign_id, shop_id, user_id, phone, code, reward_type, reward_value, max_discount_cents,
                                    min_spend_cents, currency, channels, expires_at)
         SELECT c.id, c.shop_id, $2, $3, $4, c.reward_type, c.reward_value, c.max_discount_cents, c.min_spend_cents,
                COALESCE(s.currency, c.currency), c.channels, now() + make_interval(days => c.coupon_days)
         FROM promo_campaigns c LEFT JOIN shops s ON s.id = c.shop_id WHERE c.id = $1",
    )
    .bind(campaign)
    .bind(user_id)
    .bind(phone)
    .bind(&code)
    .execute(&mut *conn)
    .await?;
    Ok(Some(code))
}

#[allow(clippy::too_many_arguments)]
pub async fn ledger(
    conn: &mut PgConnection,
    member: Uuid,
    delta: i64,
    balance_after: i64,
    reason: &str,
    r#ref: Option<(&str, Uuid)>,
    note: &str,
    by: Option<Uuid>,
) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO loyalty_ledger (member_id, delta, balance_after, reason, ref_type, ref_id, note, created_by)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8)",
    )
    .bind(member)
    .bind(delta)
    .bind(balance_after)
    .bind(reason)
    .bind(r#ref.map(|r| r.0))
    .bind(r#ref.map(|r| r.1))
    .bind(note)
    .bind(by)
    .execute(conn)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewards() {
        assert_eq!(reward_cents("amount", 5000, 0, 3000, 0), 3000);
        assert_eq!(reward_cents("percent", 1000, 0, 50_000, 0), 5000);
        assert_eq!(reward_cents("percent", 5000, 10_000, 50_000, 0), 10_000);
        assert_eq!(reward_cents("free_shipping", 0, 0, 50_000, 2500), 2500);
    }

    #[test]
    fn points() {
        let s = Settings { point_value_cents: 100, max_redeem_bps: 5000, earn_per_cents: 10_000, ..Default::default() };
        // 50% of 10 000 = 5 000 cents = 50 points max.
        assert_eq!(usable_points(80, 200, &s, 10_000), 50);
        assert_eq!(usable_points(30, 20, &s, 10_000), 20);
        assert_eq!(points_for(25_000, &s), 2);
        assert_eq!(points_for(-5, &s), 0);
        assert!(new_code().starts_with("ZK") && new_code().len() == 10);
    }
}
