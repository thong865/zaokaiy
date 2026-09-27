//! AI layer: content generation and a tool-using shop assistant agent (Anthropic Messages API).
//! When ANTHROPIC_API_KEY is not configured everything degrades to deterministic templates,
//! so the platform stays fully usable offline.

use serde_json::{json, Value};
use uuid::Uuid;

use crate::{
    error::{AppError, AppResult},
    models::{Product, Shop},
    routes::{content_insert, stats_compute},
    AppState,
};

const MAX_AGENT_STEPS: usize = 8;

pub fn enabled(st: &AppState) -> bool {
    st.cfg.anthropic_api_key.is_some()
}

async fn call(st: &AppState, body: Value) -> AppResult<Value> {
    let key = st
        .cfg
        .anthropic_api_key
        .as_deref()
        .ok_or_else(|| AppError::bad("AI is not configured"))?;
    let res = st
        .http
        .post(format!(
            "{}/v1/messages",
            st.cfg.anthropic_base_url.trim_end_matches('/')
        ))
        .header("x-api-key", key)
        .header("anthropic-version", "2023-06-01")
        .json(&body)
        .send()
        .await
        .map_err(|e| AppError::Upstream(format!("AI request failed: {e}")))?;
    let status = res.status();
    let v: Value = res
        .json()
        .await
        .map_err(|e| AppError::Upstream(format!("AI response: {e}")))?;
    if !status.is_success() {
        let msg = v["error"]["message"].as_str().unwrap_or("unknown error");
        return Err(AppError::Upstream(format!("AI error ({status}): {msg}")));
    }
    Ok(v)
}

fn text_of(resp: &Value) -> String {
    resp["content"]
        .as_array()
        .map(|blocks| {
            blocks
                .iter()
                .filter(|b| b["type"] == "text")
                .filter_map(|b| b["text"].as_str())
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
}

fn money(cents: i64, cur: &str) -> String {
    format!("{:.2} {cur}", cents as f64 / 100.0)
}

// ---------------------------------------------------------------------------
// Content generation
// ---------------------------------------------------------------------------

pub struct GenerateInput<'a> {
    pub product: &'a Product,
    pub shop: &'a Shop,
    pub kind: &'a str,
    pub tone: &'a str,
    pub language: &'a str,
    pub extra: &'a str,
}

pub struct Generated {
    pub title: String,
    pub body: String,
    pub ai: bool,
}

fn kind_brief(kind: &str) -> &'static str {
    match kind {
        "description" => "a product description for the product page: a hook sentence, 3-5 benefit bullets, and a short spec/usage paragraph",
        "ad_copy" => "a short ad: a headline under 40 characters and body copy under 120 characters with a clear call to action",
        "caption" => "a social media caption (Instagram/TikTok/Facebook) with emojis and 5-8 relevant hashtags",
        "video_script" => "a 30-45 second short-form video script (TikTok/Reels) with HOOK, 3 SCENES (visual + voiceover) and CTA",
        _ => "a creator social post reviewing/recommending the product, authentic first-person voice, ending with a call to action",
    }
}

pub async fn generate(st: &AppState, inp: GenerateInput<'_>) -> AppResult<Generated> {
    if !enabled(st) {
        return Ok(template(&inp));
    }
    let p = inp.product;
    let prompt = format!(
        "Write {brief}.\n\nProduct: {name}\nCategory: {cat}\nPrice: {price}\nShop: {shop}\nDetails: {desc}\n\
         Tone: {tone}\nLanguage: {lang}\n{extra}\n\n\
         Respond with ONLY a JSON object: {{\"title\": string, \"body\": string}}. \
         Do not invent certifications, medical claims or specs that are not in the details.",
        brief = kind_brief(inp.kind),
        name = p.name,
        cat = p.category,
        price = money(p.price_cents, &inp.shop.currency),
        shop = inp.shop.name,
        desc = if p.description.is_empty() { "(none provided)" } else { &p.description },
        tone = inp.tone,
        lang = inp.language,
        extra = if inp.extra.is_empty() { String::new() } else { format!("Extra instructions: {}", inp.extra) },
    );
    let resp = call(
        st,
        json!({
            "model": st.cfg.ai_model,
            "max_tokens": 1200,
            "system": "You are an expert e-commerce copywriter and content creator coach for Southeast Asian online sellers.",
            "messages": [{ "role": "user", "content": prompt }]
        }),
    )
    .await?;
    let text = text_of(&resp);
    let json_part = text
        .find('{')
        .and_then(|s| text.rfind('}').map(|e| &text[s..=e]));
    match json_part.and_then(|j| serde_json::from_str::<Value>(j).ok()) {
        Some(v) => Ok(Generated {
            title: v["title"].as_str().unwrap_or(&p.name).to_string(),
            body: v["body"].as_str().unwrap_or(&text).to_string(),
            ai: true,
        }),
        None => Ok(Generated {
            title: p.name.clone(),
            body: text,
            ai: true,
        }),
    }
}

fn template(inp: &GenerateInput) -> Generated {
    let p = inp.product;
    let price = money(p.price_cents, &inp.shop.currency);
    let desc = if p.description.is_empty() {
        format!("A great pick from {}.", inp.shop.name)
    } else {
        p.description.clone()
    };
    let tag = p.category.replace(' ', "");
    let (title, body) = match inp.kind {
        "description" => (
            p.name.clone(),
            format!("{desc}\n\n• Quality you can feel\n• Ships from {}\n• Only {price}\n\nOrder today while stock lasts.", inp.shop.name),
        ),
        "ad_copy" => (format!("{} — only {price}", p.name), format!("{desc} Tap to shop now before it sells out!")),
        "caption" => (
            p.name.clone(),
            format!("✨ New favourite: {} ✨\n{desc}\n💸 {price}\n🛒 Link in bio!\n#{tag} #zaokaiy #shopnow #musthave #review", p.name),
        ),
        "video_script" => (
            format!("{} — 30s script", p.name),
            format!(
                "HOOK (0-3s): \"Stop scrolling — you need to see this!\"\n\
                 SCENE 1 (3-12s): Unbox {name}. VO: \"{desc}\"\n\
                 SCENE 2 (12-25s): Show it in use. VO: \"Here's why I love it…\"\n\
                 SCENE 3 (25-35s): Close-up + price on screen ({price}).\n\
                 CTA (35-40s): \"Tap the link to get yours!\"",
                name = p.name
            ),
        ),
        _ => (
            format!("Why I'm obsessed with {}", p.name),
            format!("I've been using {} and honestly… {desc}\n\nFor {price} it's a steal. Grab it from my shop 👇", p.name),
        ),
    };
    Generated {
        title,
        body: format!("{body}\n\n[offline template — set ANTHROPIC_API_KEY for AI-written copy]"),
        ai: false,
    }
}

// ---------------------------------------------------------------------------
// Shop assistant agent
// ---------------------------------------------------------------------------

fn tools() -> Value {
    json!([
        { "name": "get_shop_stats", "description": "Revenue, orders, low-stock count, commissions, ad performance and 14-day revenue series for the shop.",
          "input_schema": { "type": "object", "properties": {} } },
        { "name": "list_products", "description": "List the shop's products with price, stock, status and reseller settings.",
          "input_schema": { "type": "object", "properties": {
              "status": { "type": "string", "enum": ["draft","active","archived"] },
              "query": { "type": "string", "description": "name/SKU filter" } } } },
        { "name": "get_low_stock", "description": "Products at or below their low-stock threshold.",
          "input_schema": { "type": "object", "properties": {} } },
        { "name": "list_recent_orders", "description": "Recent orders for this shop's products, with items and status.",
          "input_schema": { "type": "object", "properties": { "limit": { "type": "integer", "minimum": 1, "maximum": 50 } } } },
        { "name": "list_reseller_requests", "description": "Sell-staff (reseller) partnership requests and their status.",
          "input_schema": { "type": "object", "properties": {} } },
        { "name": "draft_content", "description": "Save a DRAFT piece of content (post, caption, video_script, description, ad_copy) for the seller to review. Never publishes.",
          "input_schema": { "type": "object", "required": ["kind","title","body"], "properties": {
              "product_id": { "type": "string" },
              "kind": { "type": "string", "enum": ["post","caption","video_script","description","ad_copy"] },
              "title": { "type": "string" }, "body": { "type": "string" } } } },
        { "name": "draft_ad_campaign", "description": "Create a DRAFT ad campaign for a product (not activated; the seller must activate it).",
          "input_schema": { "type": "object", "required": ["product_id","headline","budget_cents"], "properties": {
              "product_id": { "type": "string" }, "headline": { "type": "string" }, "body": { "type": "string" },
              "budget_cents": { "type": "integer", "minimum": 100 }, "cpc_cents": { "type": "integer", "minimum": 1 } } } }
    ])
}

async fn run_tool(st: &AppState, shop: &Shop, name: &str, input: &Value) -> AppResult<Value> {
    let sid = shop.id;
    match name {
        "get_shop_stats" => stats_compute(st, sid).await,
        "list_products" => {
            let rows: Vec<Product> = sqlx::query_as(
                "SELECT * FROM products WHERE shop_id=$1 AND ($2::text IS NULL OR status=$2)
                   AND ($3::text IS NULL OR name ILIKE '%'||$3||'%' OR sku ILIKE '%'||$3||'%')
                 ORDER BY updated_at DESC LIMIT 50",
            )
            .bind(sid)
            .bind(input["status"].as_str())
            .bind(input["query"].as_str())
            .fetch_all(&st.db)
            .await?;
            Ok(json!(rows
                .iter()
                .map(|p| json!({ "id": p.id, "sku": p.sku, "name": p.name, "price_cents": p.price_cents, "stock": p.stock,
                                 "status": p.status, "review_status": p.review_status, "review_note": p.review_note, "category": p.category, "allow_resell": p.allow_resell,
                                 "commission_bps": p.commission_bps, "has_description": !p.description.is_empty() }))
                .collect::<Vec<_>>()))
        }
        "get_low_stock" => {
            let rows: Vec<(Uuid, String, i32, i32)> = sqlx::query_as(
                "SELECT id, name, stock, low_stock_threshold FROM products
                 WHERE shop_id=$1 AND status<>'archived' AND stock<=low_stock_threshold ORDER BY stock",
            )
            .bind(sid)
            .fetch_all(&st.db)
            .await?;
            Ok(json!(rows
                .iter()
                .map(|(id, n, s, t)| json!({ "id": id, "name": n, "stock": s, "threshold": t }))
                .collect::<Vec<_>>()))
        }
        "list_recent_orders" => {
            let limit = input["limit"].as_i64().unwrap_or(10).clamp(1, 50);
            let rows: Vec<(Uuid, String, i64, chrono::DateTime<chrono::Utc>, String)> =
                sqlx::query_as(
                    "SELECT o.id, o.status, o.total_cents, o.created_at,
                        string_agg(oi.product_name || ' x' || oi.qty, ', ')
                 FROM orders o JOIN order_items oi ON oi.order_id=o.id
                 WHERE oi.supplier_shop_id=$1 GROUP BY o.id ORDER BY o.created_at DESC LIMIT $2",
                )
                .bind(sid)
                .bind(limit)
                .fetch_all(&st.db)
                .await?;
            Ok(json!(rows
                .iter()
                .map(|(id, s, t, at, items)| json!({ "id": id, "status": s, "total_cents": t, "created_at": at, "items": items }))
                .collect::<Vec<_>>()))
        }
        "list_reseller_requests" => {
            let rows: Vec<(Uuid, String, String, Option<i32>, String)> = sqlx::query_as(
                "SELECT pa.id, re.name, pa.status, pa.commission_bps, pa.message FROM partnerships pa
                 JOIN shops re ON re.id=pa.reseller_shop_id WHERE pa.supplier_shop_id=$1 ORDER BY pa.created_at DESC LIMIT 50",
            )
            .bind(sid)
            .fetch_all(&st.db)
            .await?;
            Ok(json!(rows
                .iter()
                .map(|(id, n, s, b, m)| json!({ "id": id, "reseller": n, "status": s, "commission_bps": b, "message": m }))
                .collect::<Vec<_>>()))
        }
        "draft_content" => {
            let product_id = parse_owned_or_listed(st, sid, input["product_id"].as_str()).await?;
            let req = crate::routes::CreateContentReq {
                product_id,
                kind: input["kind"].as_str().unwrap_or("post").to_string(),
                title: input["title"].as_str().unwrap_or("Untitled").to_string(),
                body: input["body"].as_str().unwrap_or_default().to_string(),
                media_url: None,
                ai_generated: Some(true),
                status: Some("draft".into()),
            };
            if !crate::routes::CONTENT_KINDS.contains(&req.kind.as_str()) {
                return Ok(json!({ "error": "invalid kind" }));
            }
            let c = content_insert(st, sid, &req).await?;
            Ok(json!({ "saved": true, "content_id": c.id, "status": "draft" }))
        }
        "draft_ad_campaign" => {
            let Some(product_id) =
                parse_owned_or_listed(st, sid, input["product_id"].as_str()).await?
            else {
                return Ok(
                    json!({ "error": "product_id must be one of this shop's own or resold products" }),
                );
            };
            let budget = input["budget_cents"].as_i64().unwrap_or(10_000).max(100);
            let id: Uuid = sqlx::query_scalar(
                "INSERT INTO ad_campaigns (shop_id, product_id, headline, body, budget_cents, cpc_cents, status)
                 VALUES ($1,$2,$3,$4,$5,$6,'draft') RETURNING id",
            )
            .bind(sid)
            .bind(product_id)
            .bind(input["headline"].as_str().unwrap_or("New campaign"))
            .bind(input["body"].as_str().unwrap_or_default())
            .bind(budget)
            .bind(input["cpc_cents"].as_i64().unwrap_or(100).max(1))
            .fetch_one(&st.db)
            .await?;
            Ok(json!({ "saved": true, "campaign_id": id, "status": "draft" }))
        }
        other => Ok(json!({ "error": format!("unknown tool {other}") })),
    }
}

/// Accept a product id only if the shop owns it or resells it.
async fn parse_owned_or_listed(
    st: &AppState,
    shop_id: Uuid,
    raw: Option<&str>,
) -> AppResult<Option<Uuid>> {
    let Some(id) = raw.and_then(|s| Uuid::parse_str(s).ok()) else {
        return Ok(None);
    };
    let ok: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM products WHERE id=$2 AND shop_id=$1)
             OR EXISTS (SELECT 1 FROM listings WHERE product_id=$2 AND reseller_shop_id=$1)",
    )
    .bind(shop_id)
    .bind(id)
    .fetch_one(&st.db)
    .await?;
    Ok(ok.then_some(id))
}

pub struct AgentReply {
    pub reply: String,
    pub actions: Vec<Value>,
    pub ai: bool,
}

pub async fn agent(st: &AppState, shop: &Shop, history: Vec<Value>) -> AppResult<AgentReply> {
    if !enabled(st) {
        return offline_agent(st, shop).await;
    }
    let system = format!(
        "You are the AI business assistant for the online shop \"{name}\" (kind: {kind}, currency: {cur}) on zaokaiy, \
         a social commerce platform where sellers run shops, track inventory, run ads, and let other shops/creators \
         resell their products as sell-staff for a commission (commission_bps: 1000 = 10%). Money values are in minor \
         units (cents). Use tools to look at real data before answering. You may create DRAFTS (content, ad campaigns) \
         but never claim something is published or live. Be concise and practical; format with short markdown lists. \
         Reply in the same language the seller writes in.",
        name = shop.name,
        kind = shop.kind,
        cur = shop.currency
    );
    let mut messages: Vec<Value> = history;
    let mut actions = Vec::new();

    for _ in 0..MAX_AGENT_STEPS {
        let resp = call(
            st,
            json!({ "model": st.cfg.ai_model, "max_tokens": 2000, "system": system, "tools": tools(), "messages": messages }),
        )
        .await?;
        let content = resp["content"].clone();
        if resp["stop_reason"] != "tool_use" {
            return Ok(AgentReply {
                reply: text_of(&resp),
                actions,
                ai: true,
            });
        }
        messages.push(json!({ "role": "assistant", "content": content }));
        let mut results = Vec::new();
        for block in content
            .as_array()
            .into_iter()
            .flatten()
            .filter(|b| b["type"] == "tool_use")
        {
            let name = block["name"].as_str().unwrap_or_default();
            let out = match run_tool(st, shop, name, &block["input"]).await {
                Ok(v) => v,
                Err(e) => json!({ "error": e.to_string() }),
            };
            actions.push(json!({ "tool": name, "input": block["input"], "result": out }));
            results.push(json!({ "type": "tool_result", "tool_use_id": block["id"], "content": out.to_string() }));
        }
        messages.push(json!({ "role": "user", "content": results }));
    }
    Ok(AgentReply {
        reply: "I ran out of steps — try a narrower question.".into(),
        actions,
        ai: true,
    })
}

async fn offline_agent(st: &AppState, shop: &Shop) -> AppResult<AgentReply> {
    let s = stats_compute(st, shop.id).await?;
    let low = run_tool(st, shop, "get_low_stock", &json!({})).await?;
    let cur = &shop.currency;
    let mut reply = format!(
        "**{}** snapshot (offline mode — set `ANTHROPIC_API_KEY` for the full AI agent):\n\n\
         - Revenue: {}  ·  Orders: {}  ·  To ship: {}\n\
         - Commission earned: {}  ·  owed to sell-staff: {}\n\
         - Ads: {} spent, {} clicks / {} impressions\n\
         - Resellers: {} approved, {} pending requests\n",
        shop.name,
        money(s["revenue_cents"].as_i64().unwrap_or(0), cur),
        s["orders"],
        s["orders_to_ship"],
        money(s["commission_earned_cents"].as_i64().unwrap_or(0), cur),
        money(s["commission_owed_cents"].as_i64().unwrap_or(0), cur),
        money(s["ad_spend_cents"].as_i64().unwrap_or(0), cur),
        s["ad_clicks"],
        s["ad_impressions"],
        s["resellers"],
        s["pending_reseller_requests"],
    );
    if let Some(items) = low.as_array().filter(|a| !a.is_empty()) {
        reply.push_str("\n**Low stock — restock soon:**\n");
        for it in items {
            reply.push_str(&format!(
                "- {} ({} left)\n",
                it["name"].as_str().unwrap_or("?"),
                it["stock"]
            ));
        }
    }
    Ok(AgentReply {
        reply,
        actions: vec![],
        ai: false,
    })
}
