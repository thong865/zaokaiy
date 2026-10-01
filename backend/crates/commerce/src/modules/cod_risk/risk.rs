//! Risk levels, per-customer statistics and the level the admin is advised to set.

use serde::Serialize;
use sqlx::{FromRow, PgConnection};

use crate::error::AppResult;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    None,
    Low,
    Medium,
    High,
    Blocked,
}

impl Level {
    pub const ALL: [Level; 5] = [Level::None, Level::Low, Level::Medium, Level::High, Level::Blocked];

    pub fn parse(s: &str) -> Option<Level> {
        Self::ALL.into_iter().find(|l| l.as_str() == s.trim())
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Level::None => "none",
            Level::Low => "low",
            Level::Medium => "medium",
            Level::High => "high",
            Level::Blocked => "blocked",
        }
    }
}

/// What the platform knows about one customer (from confirmed/pending reports).
#[derive(Debug, Clone, Default, Serialize, FromRow)]
pub struct Stats {
    pub confirmed: i64,
    pub pending: i64,
    pub dismissed: i64,
    /// Distinct shops with a confirmed report.
    pub shops: i64,
    /// Confirmed reports filed in the last 90 days.
    pub confirmed_90d: i64,
    /// COD value of confirmed refused parcels.
    pub amount_cents: i64,
}

/// Level suggested to the admin. Repeat refusals, and refusals across several shops, weigh most.
pub fn suggest(s: &Stats) -> Level {
    match s {
        s if s.confirmed >= 5 || (s.shops >= 3 && s.confirmed >= 3) => Level::Blocked,
        s if s.confirmed >= 3 || s.confirmed_90d >= 2 => Level::High,
        s if s.confirmed >= 2 || s.shops >= 2 => Level::Medium,
        s if s.confirmed == 1 => Level::Low,
        _ => Level::None,
    }
}

pub async fn stats(conn: &mut PgConnection, key: &str) -> AppResult<Stats> {
    Ok(sqlx::query_as(
        "SELECT count(*) FILTER (WHERE status = 'confirmed') AS confirmed,
                count(*) FILTER (WHERE status = 'pending') AS pending,
                count(*) FILTER (WHERE status = 'dismissed') AS dismissed,
                count(DISTINCT shop_id) FILTER (WHERE status = 'confirmed') AS shops,
                count(*) FILTER (WHERE status = 'confirmed' AND created_at > now() - interval '90 days') AS confirmed_90d,
                COALESCE(sum(amount_cents) FILTER (WHERE status = 'confirmed'), 0)::bigint AS amount_cents
         FROM cod_risk_reports WHERE customer_key = $1",
    )
    .bind(key)
    .fetch_one(conn)
    .await?)
}

/// SQL for the level in force (an expired level counts as none).
pub const EFFECTIVE_LEVEL_SQL: &str =
    "CASE WHEN level_expires_at IS NOT NULL AND level_expires_at <= now() THEN 'none' ELSE level END";

pub async fn effective_level(conn: &mut PgConnection, key: &str) -> AppResult<Level> {
    let l: Option<String> = sqlx::query_scalar(&format!("SELECT {EFFECTIVE_LEVEL_SQL} FROM cod_risk_customers WHERE customer_key = $1"))
        .bind(key)
        .fetch_optional(conn)
        .await?;
    Ok(l.as_deref().and_then(Level::parse).unwrap_or(Level::None))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(confirmed: i64, shops: i64, recent: i64) -> Stats {
        Stats { confirmed, shops, confirmed_90d: recent, ..Default::default() }
    }

    #[test]
    fn suggestions() {
        assert_eq!(suggest(&Stats::default()), Level::None);
        assert_eq!(suggest(&Stats { pending: 4, ..Default::default() }), Level::None, "unconfirmed reports never count");
        assert_eq!(suggest(&s(1, 1, 0)), Level::Low);
        assert_eq!(suggest(&s(2, 1, 0)), Level::Medium);
        assert_eq!(suggest(&s(2, 2, 2)), Level::High, "two recent refusals");
        assert_eq!(suggest(&s(3, 1, 0)), Level::High);
        assert_eq!(suggest(&s(3, 3, 0)), Level::Blocked, "refused at three different shops");
        assert_eq!(suggest(&s(5, 1, 0)), Level::Blocked);
    }

    #[test]
    fn levels_order_and_parse() {
        assert!(Level::Blocked > Level::High && Level::High > Level::Medium && Level::Low > Level::None);
        assert_eq!(Level::parse("high"), Some(Level::High));
        assert_eq!(Level::parse("HIGH"), None);
        assert_eq!(Level::parse("x"), None);
    }
}
