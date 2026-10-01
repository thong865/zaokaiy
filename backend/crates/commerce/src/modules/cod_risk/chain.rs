//! Tamper-evident ledger for COD risk events.
//!
//! * Every event is a **block**: `hash = sha256("{height}|{prev_hash}|{ts_micros}|{event}|{payload}")`,
//!   linked to the previous block's hash. Editing or deleting any block breaks every later hash
//!   ([`verify`]), and the table itself refuses updates/deletes (trigger in the migration).
//! * Blocks are appended inside the same database transaction as the change they record, under
//!   an advisory lock, so the ledger and the tables never disagree.
//! * **Anchoring** (`COD_RISK_ANCHOR=webhook`): unanchored blocks are batched, their Merkle root is
//!   POSTed to a chain gateway (MBlock, an EVM relay, …) signed with HMAC-SHA256, and the returned
//!   transaction id is stored. [`merkle_proof`] lets anyone check that one block is in an anchored
//!   batch, using only the block hash, the proof and the root that is on-chain.
//! * Payloads never contain personal data: customer keys (keyed hashes), ids, amounts and hashes
//!   of free-text notes.

use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::{FromRow, PgConnection};

use crate::{
    error::{AppError, AppResult},
    AppState,
};

pub const GENESIS: &str = "0000000000000000000000000000000000000000000000000000000000000000";
/// pg_advisory_xact_lock key serialising appends ("cod-risk" in ASCII-ish).
const APPEND_LOCK: i64 = 0x636f_645f_7269_736b;
const ANCHOR_LOCK: i64 = 0x636f_645f_616e_6368;
/// Largest batch published in one anchor.
const MAX_BATCH: i64 = 5000;

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct Block {
    pub height: i64,
    pub prev_hash: String,
    pub hash: String,
    pub event: String,
    pub payload: String,
    pub ts_micros: i64,
    pub anchor_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct Anchor {
    pub id: i64,
    pub from_height: i64,
    pub to_height: i64,
    pub merkle_root: String,
    pub driver: String,
    pub tx_ref: String,
    pub status: String,
    pub error: String,
    pub created_at: DateTime<Utc>,
}

pub fn sha256_hex(data: impl AsRef<[u8]>) -> String {
    hex::encode(Sha256::digest(data))
}

pub fn block_hash(height: i64, prev_hash: &str, ts_micros: i64, event: &str, payload: &str) -> String {
    sha256_hex(format!("{height}|{prev_hash}|{ts_micros}|{event}|{payload}"))
}

/// Canonical JSON: serde_json objects are BTreeMaps here, so keys come out sorted.
pub fn canonical(v: &Value) -> String {
    serde_json::to_string(v).unwrap_or_default()
}

/// Append one event. Call inside the transaction that makes the change being recorded.
pub async fn append(conn: &mut PgConnection, event: &str, payload: Value) -> AppResult<Block> {
    sqlx::query("SELECT pg_advisory_xact_lock($1)").bind(APPEND_LOCK).execute(&mut *conn).await?;
    let tip: Option<(i64, String, i64)> = sqlx::query_as("SELECT height, hash, ts_micros FROM cod_risk_blocks ORDER BY height DESC LIMIT 1")
        .fetch_optional(&mut *conn)
        .await?;
    let (height, prev, last_ts) = tip.map(|(h, x, t)| (h + 1, x, t)).unwrap_or((1, GENESIS.to_string(), 0));
    // Strictly increasing timestamps (clock skew between API instances can't reorder blocks).
    let ts = Utc::now().timestamp_micros().max(last_ts + 1);
    let payload = canonical(&payload);
    let hash = block_hash(height, &prev, ts, event, &payload);
    let b: Block = sqlx::query_as(
        "INSERT INTO cod_risk_blocks (height, prev_hash, hash, event, payload, ts_micros)
         VALUES ($1,$2,$3,$4,$5,$6) RETURNING height, prev_hash, hash, event, payload, ts_micros, anchor_id",
    )
    .bind(height)
    .bind(&prev)
    .bind(&hash)
    .bind(event)
    .bind(&payload)
    .bind(ts)
    .fetch_one(&mut *conn)
    .await?;
    Ok(b)
}

/// Where the chain is broken, if anywhere.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Broken {
    pub height: i64,
    pub reason: &'static str,
}

/// Check a run of consecutive blocks. `prev` is the hash before the first block (GENESIS for 1).
pub fn verify(prev: &str, first_height: i64, blocks: &[Block]) -> Result<(), Broken> {
    let mut prev = prev.to_string();
    let mut expect = first_height;
    for b in blocks {
        if b.height != expect {
            return Err(Broken { height: expect, reason: "missing block" });
        }
        if b.prev_hash != prev {
            return Err(Broken { height: b.height, reason: "previous hash does not match" });
        }
        if block_hash(b.height, &b.prev_hash, b.ts_micros, &b.event, &b.payload) != b.hash {
            return Err(Broken { height: b.height, reason: "block content was changed" });
        }
        prev = b.hash.clone();
        expect += 1;
    }
    Ok(())
}

/// Verify the whole ledger in pages. Returns (height, tip hash, result).
pub async fn verify_all(db: &sqlx::PgPool) -> AppResult<(i64, String, Result<(), Broken>)> {
    let mut prev = GENESIS.to_string();
    let mut next = 1i64;
    loop {
        let page: Vec<Block> = sqlx::query_as(
            "SELECT height, prev_hash, hash, event, payload, ts_micros, anchor_id FROM cod_risk_blocks
             WHERE height >= $1 ORDER BY height LIMIT 5000",
        )
        .bind(next)
        .fetch_all(db)
        .await?;
        let Some(last) = page.last().cloned() else {
            return Ok((next - 1, prev, Ok(())));
        };
        if let Err(e) = verify(&prev, next, &page) {
            return Ok((last.height, last.hash, Err(e)));
        }
        prev = last.hash;
        next = last.height + 1;
    }
}

// ---------------------------------------------------------------------------------------------
// Merkle tree (leaf = sha256(0x00 || block hash), node = sha256(0x01 || left || right);
// an odd node is carried up unchanged)
// ---------------------------------------------------------------------------------------------

fn leaf(block_hash_hex: &str) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update([0u8]);
    h.update(hex::decode(block_hash_hex).unwrap_or_default());
    h.finalize().into()
}

fn node(l: &[u8; 32], r: &[u8; 32]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update([1u8]);
    h.update(l);
    h.update(r);
    h.finalize().into()
}

pub fn merkle_root(block_hashes: &[String]) -> String {
    let mut level: Vec<[u8; 32]> = block_hashes.iter().map(|h| leaf(h)).collect();
    if level.is_empty() {
        return GENESIS.to_string();
    }
    while level.len() > 1 {
        level = level.chunks(2).map(|p| if p.len() == 2 { node(&p[0], &p[1]) } else { p[0] }).collect();
    }
    hex::encode(level[0])
}

/// One proof step: the sibling hash and which side it sits on.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Step {
    pub sibling: String,
    /// "left" = sibling || current, "right" = current || sibling.
    pub side: &'static str,
}

pub fn merkle_proof(block_hashes: &[String], index: usize) -> Vec<Step> {
    let mut level: Vec<[u8; 32]> = block_hashes.iter().map(|h| leaf(h)).collect();
    let mut idx = index;
    let mut out = Vec::new();
    while level.len() > 1 {
        let sib = idx ^ 1;
        if sib < level.len() {
            out.push(Step { sibling: hex::encode(level[sib]), side: if sib < idx { "left" } else { "right" } });
        }
        level = level.chunks(2).map(|p| if p.len() == 2 { node(&p[0], &p[1]) } else { p[0] }).collect();
        idx /= 2;
    }
    out
}

pub fn verify_proof(block_hash_hex: &str, proof: &[Step], root: &str) -> bool {
    let mut cur = leaf(block_hash_hex);
    for s in proof {
        let Ok(sib) = <[u8; 32]>::try_from(hex::decode(&s.sibling).unwrap_or_default()) else {
            return false;
        };
        cur = if s.side == "left" { node(&sib, &cur) } else { node(&cur, &sib) };
    }
    hex::encode(cur) == root
}

// ---------------------------------------------------------------------------------------------
// Anchoring
// ---------------------------------------------------------------------------------------------

/// Publish the Merkle root of all unanchored blocks. Ok(None) when there is nothing to do or
/// another instance is already anchoring.
pub async fn anchor_pending(st: &AppState) -> AppResult<Option<Anchor>> {
    let c = super::cfg();
    if !c.anchoring() {
        return Err(AppError::bad("anchoring is not configured (set COD_RISK_ANCHOR=webhook and COD_RISK_ANCHOR_URL)"));
    }
    let mut tx = st.db.begin().await?;
    let locked: bool = sqlx::query_scalar("SELECT pg_try_advisory_xact_lock($1)").bind(ANCHOR_LOCK).fetch_one(&mut *tx).await?;
    if !locked {
        return Ok(None);
    }
    let blocks: Vec<(i64, String)> = sqlx::query_as("SELECT height, hash FROM cod_risk_blocks WHERE anchor_id IS NULL ORDER BY height LIMIT $1")
        .bind(MAX_BATCH)
        .fetch_all(&mut *tx)
        .await?;
    let (Some(first), Some(last)) = (blocks.first(), blocks.last()) else {
        return Ok(None);
    };
    let (from, to, tip) = (first.0, last.0, last.1.clone());
    let hashes: Vec<String> = blocks.iter().map(|b| b.1.clone()).collect();
    let root = merkle_root(&hashes);
    let body = json!({
        "ledger": c.ledger_name,
        "from_height": from,
        "to_height": to,
        "count": hashes.len(),
        "merkle_root": root,
        "tip_hash": tip,
        "timestamp": Utc::now().to_rfc3339(),
    });
    let (status, tx_ref, error) = match post_webhook(st, c, &body).await {
        Ok(r) => ("anchored", r, String::new()),
        Err(e) => ("failed", String::new(), e.chars().take(500).collect()),
    };
    let a: Anchor = sqlx::query_as(
        "INSERT INTO cod_risk_anchors (from_height, to_height, merkle_root, driver, tx_ref, status, error)
         VALUES ($1,$2,$3,$4,$5,$6,$7) RETURNING *",
    )
    .bind(from)
    .bind(to)
    .bind(&root)
    .bind(&c.anchor)
    .bind(&tx_ref)
    .bind(status)
    .bind(&error)
    .fetch_one(&mut *tx)
    .await?;
    if status == "anchored" {
        sqlx::query("UPDATE cod_risk_blocks SET anchor_id = $1 WHERE height BETWEEN $2 AND $3 AND anchor_id IS NULL")
            .bind(a.id)
            .bind(from)
            .bind(to)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(Some(a))
}

/// POST the batch to the chain gateway. The body is signed: `X-Signature: sha256=<hex hmac>`.
/// The gateway answers `{"tx": "<transaction hash or id>"}` (also accepted: `tx_hash`, `id`).
async fn post_webhook(st: &AppState, c: &super::Config, body: &Value) -> Result<String, String> {
    let raw = canonical(body);
    let mut mac = Hmac::<Sha256>::new_from_slice(c.anchor_secret.as_bytes()).map_err(|e| e.to_string())?;
    mac.update(raw.as_bytes());
    let sig = hex::encode(mac.finalize().into_bytes());
    let res = st
        .http
        .post(&c.anchor_url)
        .header("Content-Type", "application/json")
        .header("X-Signature", format!("sha256={sig}"))
        .timeout(std::time::Duration::from_secs(20))
        .body(raw)
        .send()
        .await
        .map_err(|e| format!("gateway unreachable: {e}"))?;
    let code = res.status();
    let v: Value = res.json().await.unwrap_or(Value::Null);
    if !code.is_success() {
        return Err(format!("gateway returned {code}: {v}"));
    }
    ["tx", "tx_hash", "id"]
        .iter()
        .find_map(|k| v.get(*k).and_then(|x| x.as_str().map(str::to_string).or_else(|| x.as_i64().map(|n| n.to_string()))))
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("gateway response has no transaction id: {v}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chain(n: i64) -> Vec<Block> {
        let mut prev = GENESIS.to_string();
        (1..=n)
            .map(|h| {
                let payload = canonical(&json!({ "n": h, "customer_key": "ab" }));
                let hash = block_hash(h, &prev, 1_000 + h, "report.filed", &payload);
                let b = Block { height: h, prev_hash: prev.clone(), hash: hash.clone(), event: "report.filed".into(), payload, ts_micros: 1_000 + h, anchor_id: None };
                prev = hash;
                b
            })
            .collect()
    }

    #[test]
    fn canonical_json_sorts_keys() {
        assert_eq!(canonical(&json!({ "b": 1, "a": { "d": 2, "c": 3 } })), r#"{"a":{"c":3,"d":2},"b":1}"#);
    }

    #[test]
    fn detects_tampering() {
        let c = chain(5);
        assert_eq!(verify(GENESIS, 1, &c), Ok(()));
        let mut t = c.clone();
        t[2].payload = t[2].payload.replace("\"n\":3", "\"n\":4");
        assert_eq!(verify(GENESIS, 1, &t), Err(Broken { height: 3, reason: "block content was changed" }));
        let mut t = c.clone();
        t.remove(1);
        assert_eq!(verify(GENESIS, 1, &t).unwrap_err().height, 2);
        // Re-hashing a changed block doesn't help: the next block still points at the old hash.
        let mut t = c.clone();
        t[2].payload = "{}".into();
        t[2].hash = block_hash(3, &t[2].prev_hash, t[2].ts_micros, &t[2].event, &t[2].payload);
        assert_eq!(verify(GENESIS, 1, &t), Err(Broken { height: 4, reason: "previous hash does not match" }));
    }

    #[test]
    fn merkle_proofs_verify_for_every_leaf() {
        for n in 1..=9 {
            let hashes: Vec<String> = chain(n).into_iter().map(|b| b.hash).collect();
            let root = merkle_root(&hashes);
            for (i, h) in hashes.iter().enumerate() {
                let p = merkle_proof(&hashes, i);
                assert!(verify_proof(h, &p, &root), "n={n} i={i}");
                assert!(!verify_proof(&hashes[(i + 1) % hashes.len()], &p, &root) || n == 1, "wrong leaf must fail n={n} i={i}");
            }
        }
        assert_eq!(merkle_root(&[]), GENESIS);
    }
}
