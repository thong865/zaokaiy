//! AI layer: content generation and a tool-using shop assistant agent, via OpenRouter
//! (OpenAI-compatible Chat Completions, so any compatible endpoint works through OPENROUTER_BASE_URL).
//! When OPENROUTER_API_KEY is not configured everything degrades to deterministic templates,
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
    st.cfg.ai_api_key.is_some()
}

/// Adds the model (plus OpenRouter fallback models, tried in order when the primary fails).
fn with_model(st: &AppState, mut body: Value) -> Value {
    body["model"] = json!(st.cfg.ai_model);
    if !st.cfg.ai_fallback_models.is_empty() {
        let mut models = vec![st.cfg.ai_model.clone()];
        models.extend(st.cfg.ai_fallback_models.iter().cloned());
        body["models"] = json!(models);
    }
    body
}

/// A failed AI call. `busy` = rate limited / overloaded even after retries; callers then fall back
/// to the offline templates instead of showing an error (when AI_OFFLINE_ON_BUSY is on).
struct CallErr {
    busy: bool,
    err: AppError,
}

impl From<CallErr> for AppError {
    fn from(e: CallErr) -> Self {
        e.err
    }
}

const NOTE_OFFLINE: &str = "offline template — set OPENROUTER_API_KEY for AI-written copy";
const NOTE_BUSY: &str = "AI is busy (rate limited by the model provider) — offline template shown, try again later";

fn short(s: &str, n: usize) -> String {
    let t: String = s.chars().take(n).collect();
    if t.len() < s.len() {
        format!("{t}…")
    } else {
        t
    }
}

/// POST /chat/completions and return the first choice's message.
/// Retries 408/429/5xx with backoff (honouring Retry-After), except OpenRouter's daily free quota.
async fn call(st: &AppState, body: Value) -> Result<Value, CallErr> {
    let key = st.cfg.ai_api_key.as_deref().ok_or_else(|| CallErr {
        busy: false,
        err: AppError::bad("AI is not configured"),
    })?;
    let body = with_model(st, body);
    let url = format!("{}/chat/completions", st.cfg.ai_base_url.trim_end_matches('/'));
    let max_attempts = st.cfg.ai_max_retries + 1;
    let mut attempt: u32 = 0;
    loop {
        attempt += 1;
        let mut req = st.http.post(&url).bearer_auth(key).json(&body);
        // OpenRouter app attribution (optional).
        if !st.cfg.ai_site_url.is_empty() {
            req = req.header("HTTP-Referer", &st.cfg.ai_site_url);
        }
        if !st.cfg.ai_app_name.is_empty() {
            req = req.header("X-Title", &st.cfg.ai_app_name);
        }
        let backoff = std::time::Duration::from_secs(1u64 << (attempt - 1).min(4));
        let res = match req.send().await {
            Ok(r) => r,
            Err(e) => {
                if attempt < max_attempts && (e.is_timeout() || e.is_connect()) {
                    tokio::time::sleep(backoff).await;
                    continue;
                }
                return Err(CallErr {
                    busy: e.is_timeout(),
                    err: AppError::Upstream(format!("AI request failed: {e}")),
                });
            }
        };
        let status = res.status();
        let retry_after = res
            .headers()
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.trim().parse::<u64>().ok());
        let v: Value = match res.json().await {
            Ok(v) => v,
            Err(e) => {
                let transient = status.as_u16() == 429 || status.is_server_error();
                if transient && attempt < max_attempts {
                    tokio::time::sleep(backoff).await;
                    continue;
                }
                return Err(CallErr {
                    busy: transient,
                    err: AppError::Upstream(format!("AI response ({status}): {e}")),
                });
            }
        };
        // OpenRouter can report errors with a 200 status too, at top level or per choice.
        let err = if v["error"].is_object() {
            Some(v["error"].clone())
        } else if v["choices"][0]["error"].is_object() {
            Some(v["choices"][0]["error"].clone())
        } else {
            None
        };
        if status.is_success() && err.is_none() {
            let msg = v["choices"][0]["message"].clone();
            if msg.is_null() {
                return Err(CallErr {
                    busy: false,
                    err: AppError::Upstream("AI returned no choices".into()),
                });
            }
            tracing::debug!(
                model = v["model"].as_str().unwrap_or_default(),
                provider = v["provider"].as_str().unwrap_or_default(),
                prompt_tokens = v["usage"]["prompt_tokens"].as_i64().unwrap_or(0),
                completion_tokens = v["usage"]["completion_tokens"].as_i64().unwrap_or(0),
                cost = v["usage"]["cost"].as_f64().unwrap_or(0.0),
                "ai call"
            );
            return Ok(msg);
        }

        let e = err.unwrap_or(Value::Null);
        let code = e["code"]
            .as_u64()
            .and_then(|c| u16::try_from(c).ok())
            .filter(|c| (400..600).contains(c))
            .unwrap_or(status.as_u16());
        let message = e["message"].as_str().unwrap_or("unknown error").to_string();
        let provider = e["metadata"]["provider_name"].as_str().unwrap_or_default();
        let raw = match &e["metadata"]["raw"] {
            Value::String(s) => s.clone(),
            Value::Null => String::new(),
            other => other.to_string(),
        };
        let transient = matches!(code, 408 | 429 | 500 | 502 | 503 | 504 | 529);
        // The daily free-model quota won't reset in seconds: don't retry it.
        let daily_quota = message.contains("per-day");
        if transient && !daily_quota && attempt < max_attempts {
            let wait = retry_after
                .map(std::time::Duration::from_secs)
                .unwrap_or(backoff)
                .min(std::time::Duration::from_secs(st.cfg.ai_retry_max_wait_secs));
            tracing::warn!(code, provider, attempt, wait_ms = wait.as_millis() as u64, "AI call failed, retrying: {message}");
            tokio::time::sleep(wait).await;
            continue;
        }
        let mut detail = message;
        if !provider.is_empty() {
            detail.push_str(&format!(" [{provider}]"));
        }
        if !raw.is_empty() {
            detail.push_str(&format!(" — {}", short(&raw, 200)));
        }
        let label = reqwest::StatusCode::from_u16(code)
            .map(|s| s.to_string())
            .unwrap_or_else(|_| code.to_string());
        tracing::warn!(code, attempts = attempt, "AI call failed: {detail}");
        return Err(CallErr {
            busy: transient,
            err: AppError::Upstream(format!("AI error ({label}): {detail}")),
        });
    }
}

/// Text of an assistant message: `content` is a string, or an array of parts on some providers.
fn text_of(msg: &Value) -> String {
    match &msg["content"] {
        Value::String(s) => s.clone(),
        Value::Array(parts) => parts
            .iter()
            .filter_map(|p| p["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
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
        return Ok(template(&inp, NOTE_OFFLINE));
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
    let msg = match call(
        st,
        json!({
            "max_tokens": 1500,
            "response_format": { "type": "json_object" },
            "messages": [
                { "role": "system", "content": "You are an expert e-commerce copywriter and content creator coach for Southeast Asian online sellers (Laos, Thailand). You write natural, fluent Lao, Thai and English." },
                { "role": "user", "content": prompt }
            ]
        }),
    )
    .await
    {
        Ok(m) => m,
        Err(e) if e.busy && st.cfg.ai_offline_on_busy => return Ok(template(&inp, NOTE_BUSY)),
        Err(e) => return Err(e.into()),
    };
    let text = text_of(&msg);
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

fn template(inp: &GenerateInput, note: &str) -> Generated {
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
        body: format!("{body}\n\n[{note}]"),
        ai: false,
    }
}

// ---------------------------------------------------------------------------
// Shop assistant agent
// ---------------------------------------------------------------------------

fn tools() -> Value {
    // Declared as {name, description, input_schema}; converted to OpenAI function tools below.
    let defs = json!([
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
    ]);
    Value::Array(
        defs.as_array()
            .into_iter()
            .flatten()
            .map(|t| {
                json!({ "type": "function", "function": {
                    "name": t["name"], "description": t["description"], "parameters": t["input_schema"] } })
            })
            .collect(),
    )
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
        return offline_agent(st, shop, "offline mode — set `OPENROUTER_API_KEY` for the full AI agent", vec![]).await;
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
    let mut messages: Vec<Value> = Vec::with_capacity(history.len() + 1);
    messages.push(json!({ "role": "system", "content": system }));
    messages.extend(history);
    let mut actions = Vec::new();
    let tools = tools();

    for _ in 0..MAX_AGENT_STEPS {
        let msg = match call(
            st,
            json!({ "max_tokens": 2000, "tools": tools, "tool_choice": "auto", "messages": messages }),
        )
        .await
        {
            Ok(m) => m,
            Err(e) if e.busy && st.cfg.ai_offline_on_busy => {
                let note = "the AI is busy right now (rate limited by the model provider) — here is a quick snapshot instead; try again later";
                return offline_agent(st, shop, note, actions).await;
            }
            Err(e) => return Err(e.into()),
        };
        let calls: Vec<Value> = msg["tool_calls"].as_array().cloned().unwrap_or_default();
        if calls.is_empty() {
            return Ok(AgentReply {
                reply: text_of(&msg),
                actions,
                ai: true,
            });
        }
        messages.push(json!({ "role": "assistant", "content": msg["content"], "tool_calls": calls }));
        for c in &calls {
            let name = c["function"]["name"].as_str().unwrap_or_default();
            // Arguments arrive as a JSON string (some providers send an object or an empty string).
            let input = match &c["function"]["arguments"] {
                Value::String(s) if !s.trim().is_empty() => {
                    serde_json::from_str(s).unwrap_or_else(|_| json!({}))
                }
                Value::Object(_) => c["function"]["arguments"].clone(),
                _ => json!({}),
            };
            let out = match run_tool(st, shop, name, &input).await {
                Ok(v) => v,
                Err(e) => json!({ "error": e.to_string() }),
            };
            actions.push(json!({ "tool": name, "input": input, "result": out }));
            messages.push(json!({ "role": "tool", "tool_call_id": c["id"], "content": out.to_string() }));
        }
    }
    Ok(AgentReply {
        reply: "I ran out of steps — try a narrower question.".into(),
        actions,
        ai: true,
    })
}

async fn offline_agent(
    st: &AppState,
    shop: &Shop,
    note: &str,
    actions: Vec<Value>,
) -> AppResult<AgentReply> {
    let s = stats_compute(st, shop.id).await?;
    let low = run_tool(st, shop, "get_low_stock", &json!({})).await?;
    let cur = &shop.currency;
    let mut reply = format!(
        "**{}** snapshot ({note}):\n\n\
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
        actions,
        ai: false,
    })
}
