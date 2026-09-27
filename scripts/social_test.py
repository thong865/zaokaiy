#!/usr/bin/env python3
"""Social commerce (comment → order) tests.

Start the API with:
  META_VERIFY_TOKEN=vt META_APP_SECRET=appsecret TIKTOK_CLIENT_SECRET=ttsecret \\
  META_GRAPH_URL=http://127.0.0.1:9998 PUBLIC_WEB_URL=http://localhost:3000
and run scripts/mock_graph.py alongside. Usage: python3 scripts/social_test.py [http://localhost:8080]
"""
import hashlib, hmac, json, sys, time, urllib.request, urllib.error, uuid

BASE = sys.argv[1] if len(sys.argv) > 1 else "http://localhost:8080"
API = BASE + "/api"
GRAPH = "http://127.0.0.1:9998"
RUN = uuid.uuid4().hex[:6]


def call(method, path, body=None, token=None, expect=200, headers=None, raw=None, text=False):
    data = raw if raw is not None else (json.dumps(body).encode() if body is not None else None)
    r = urllib.request.Request(API + path, data=data, method=method)
    r.add_header("Content-Type", "application/json")
    for k, v in (headers or {}).items():
        r.add_header(k, v)
    if token:
        r.add_header("Authorization", f"Bearer {token}")
    try:
        with urllib.request.urlopen(r) as resp:
            code, payload = resp.status, resp.read()
    except urllib.error.HTTPError as e:
        code, payload = e.code, e.read()
    out = payload.decode() if text else json.loads(payload or b"null")
    assert code == expect, f"{method} {path} -> {code} (expected {expect}): {out}"
    return out


def graph_log():
    with urllib.request.urlopen(GRAPH + "/_log") as r:
        return json.loads(r.read())


def graph_clear():
    urllib.request.urlopen(urllib.request.Request(GRAPH + "/_log", method="DELETE")).read()


def hsig(secret, data):
    return hmac.new(secret.encode(), data, hashlib.sha256).hexdigest()


def main():
    tok = call("POST", "/auth/register", {"email": f"live-{RUN}@t.dev", "password": "password123", "display_name": "Live"})["token"]
    shop = call("POST", "/shops", {"slug": f"live-{RUN}", "name": "Live Shop"}, tok)
    sid = shop["id"]
    mk = lambda sku, price, stock, code=None: call("POST", f"/shops/{sid}/products", {"sku": sku, "name": sku.title(), "price_cents": price, "initial_stock": stock, "social_code": code}, tok)
    a01 = mk("dress", 29900, 3, "a01")
    b02 = mk("scarf", 15000, 10, "B02")
    mk("hat", 9900, 5)
    assert a01["social_code"] == "A01"
    call("POST", f"/shops/{sid}/products", {"sku": "dup", "name": "d", "price_cents": 1, "social_code": "A01"}, tok, expect=409)
    call("POST", f"/shops/{sid}/products", {"sku": "bad", "name": "d", "price_cents": 1, "social_code": "12"}, tok, expect=400)
    assert call("POST", f"/shops/{sid}/social/assign-codes", token=tok)["assigned"] == 1

    s = call("PATCH", f"/shops/{sid}/social/settings", {"shipping_cents": 5000, "hold_hours": 2, "payment_instructions": "PromptPay 081-234-5678"}, tok)
    assert s["shipping_cents"] == 5000 and "cf" in s["triggers"]
    live = call("POST", f"/shops/{sid}/social/sessions", {"title": "Friday live"}, tok)

    # --- Simulator: claims, partial, sold out, questions ---
    o1 = call("POST", f"/shops/{sid}/social/simulate", {"user_name": "Nok", "message": "CF A01 x2"}, tok)
    assert o1["result"] == "claimed" and o1["order_number"] == "SO000001", o1
    assert "฿648" in o1["reply_text"] and "/c/" in o1["reply_text"] and o1["reply_status"] == "simulated", o1["reply_text"]
    o2 = call("POST", f"/shops/{sid}/social/simulate", {"user_name": "Nok", "message": "a01 2"}, tok)
    assert o2["result"] == "partial" and o2["order_id"] == o1["order_id"] and "หมด" in o2["reply_text"], o2
    o3 = call("POST", f"/shops/{sid}/social/simulate", {"user_name": "Ploy", "message": "cf a01"}, tok)
    assert o3["result"] == "sold_out" and o3["order_id"] is None, o3
    assert call("POST", f"/shops/{sid}/social/simulate", {"user_name": "Ploy", "message": "B02 ราคาเท่าไหร่คะ"}, tok)["result"] == "ignored"
    assert call("GET", f"/products/{a01['id']}", token=tok)["stock"] == 0
    board = call("GET", f"/shops/{sid}/social/board", token=tok)
    assert next(p for p in board["products"] if p["code"] == "A01")["claimed"] == 3 and board["session"]["id"] == live["id"]
    feed = call("GET", f"/shops/{sid}/social/feed?only_orders=true", token=tok)
    assert [f["result"] for f in feed] == ["sold_out", "partial", "claimed"], feed

    # --- Meta webhook: handshake, signature, dedupe, reply via Graph API ---
    assert call("GET", "/webhooks/meta?hub.mode=subscribe&hub.verify_token=vt&hub.challenge=12345", text=True) == "12345"
    call("GET", "/webhooks/meta?hub.mode=subscribe&hub.verify_token=wrong&hub.challenge=1", expect=403, text=True)
    fb = call("POST", f"/shops/{sid}/social/channels", {"provider": "facebook", "name": "My Page", "external_id": f"PAGE{RUN}", "access_token": "PAGE_TOKEN"}, tok)
    assert fb["has_token"] and "access_token" not in fb
    graph_clear()
    payload = json.dumps({"object": "page", "entry": [{"id": f"PAGE{RUN}", "changes": [
        {"field": "feed", "value": {"item": "comment", "verb": "add", "comment_id": f"c1-{RUN}", "post_id": "p1", "message": "CF B02 x2", "from": {"id": "fbuser1", "name": "Mali"}}},
        {"field": "feed", "value": {"item": "comment", "verb": "add", "comment_id": f"c2-{RUN}", "post_id": "p1", "message": "CF B02", "from": {"id": f"PAGE{RUN}", "name": "Page"}}},
    ]}]}).encode()
    call("POST", "/webhooks/meta", raw=payload, expect=401)
    good = {"X-Hub-Signature-256": "sha256=" + hsig("appsecret", payload)}
    assert call("POST", "/webhooks/meta", raw=payload, headers=good)["handled"] == 1          # own comment skipped
    call("POST", "/webhooks/meta", raw=payload, headers=good)                                    # Meta retry
    assert call("GET", f"/products/{b02['id']}", token=tok)["stock"] == 8                         # no double reservation
    log = graph_log()
    dm = next(l for l in log if l["path"].endswith(f"PAGE{RUN}/messages"))
    assert dm["auth"] == "Bearer PAGE_TOKEN" and dm["body"]["recipient"]["comment_id"] == f"c1-{RUN}" and "/c/" in dm["body"]["message"]["text"]
    assert any(l["path"].endswith(f"c1-{RUN}/comments") for l in log)                            # public ack

    # --- WhatsApp ---
    call("POST", f"/shops/{sid}/social/channels", {"provider": "whatsapp", "name": "WA", "external_id": f"WA{RUN}", "access_token": "WA_TOKEN"}, tok)
    graph_clear()
    wa = json.dumps({"object": "whatsapp_business_account", "entry": [{"changes": [{"field": "messages", "value": {
        "metadata": {"phone_number_id": f"WA{RUN}"}, "contacts": [{"wa_id": "66812345678", "profile": {"name": "Somchai"}}],
        "messages": [{"from": "66812345678", "id": f"wamid-{RUN}", "type": "text", "text": {"body": "สั่ง B02 ๓ ชิ้น"}}]}}]}]}).encode()
    call("POST", "/webhooks/meta", raw=wa, headers={"X-Hub-Signature-256": "sha256=" + hsig("appsecret", wa)})
    sent = graph_log()[0]
    assert sent["path"].endswith(f"WA{RUN}/messages") and sent["body"]["to"] == "66812345678" and sent["auth"] == "Bearer WA_TOKEN", sent
    assert call("GET", f"/products/{b02['id']}", token=tok)["stock"] == 5

    # --- TikTok (signed, replay-protected) ---
    call("POST", f"/shops/{sid}/social/channels", {"provider": "tiktok", "name": "TT", "external_id": f"TT{RUN}"}, tok)
    tt = json.dumps({"event": "live.comment", "user_openid": f"TT{RUN}", "content": json.dumps(
        {"comment_id": f"tt1-{RUN}", "text": "F b02", "user": {"open_id": "ttu1", "nickname": "Fah"}})}).encode()
    ts = str(int(time.time()))
    call("POST", "/webhooks/tiktok", raw=tt, headers={"TikTok-Signature": f"t={ts},s=bad"}, expect=401)
    old = str(int(time.time()) - 3600)
    call("POST", "/webhooks/tiktok", raw=tt, headers={"TikTok-Signature": f"t={old},s={hsig('ttsecret', (old + '.').encode() + tt)}"}, expect=401)
    r = call("POST", "/webhooks/tiktok", raw=tt, headers={"TikTok-Signature": f"t={ts},s={hsig('ttsecret', (ts + '.').encode() + tt)}"})
    assert r["result"] == "claimed" and r["reply_text"], r

    # --- Generic signed ingest (n8n / Zapier) ---
    gen = call("POST", f"/shops/{sid}/social/channels", {"provider": "webhook", "name": "n8n"}, tok)
    body = json.dumps({"id": f"g1-{RUN}", "user_id": "line:U123", "user_name": "Jib", "message": "cf B02"}).encode()
    call("POST", f"/webhooks/ingest/{gen['id']}", raw=body, headers={"X-Zaokaiy-Signature": "sha256=nope"}, expect=401)
    g = call("POST", f"/webhooks/ingest/{gen['id']}", raw=body, headers={"X-Zaokaiy-Signature": "sha256=" + hsig(gen["secret"], body)})
    assert g["result"] == "claimed" and g["checkout_url"].startswith("http://localhost:3000/c/") and g["reply_status"] == "unsupported"

    # --- Customer checkout ---
    token = o1["checkout_url"].rsplit("/", 1)[1]
    pub = call("GET", f"/public/social-orders/{token}")
    assert pub["order"]["total_cents"] == 3 * 29900 + 5000 and "token" not in pub["order"] and pub["shop"]["payment_instructions"]
    call("POST", f"/public/social-orders/{token}/payment", {"method": "transfer", "reference": "x"}, expect=400)   # confirm first
    call("POST", f"/public/social-orders/{token}/confirm", {"name": "", "phone": "", "address": ""}, expect=400)
    c = call("POST", f"/public/social-orders/{token}/confirm", {"name": "Nok S.", "phone": "0812345678", "address": "9 Sukhumvit 11, Bangkok 10110"})
    assert c["order"]["status"] == "confirmed"
    call("POST", f"/public/social-orders/{token}/payment", {"method": "transfer", "reference": "SLIP-889"})
    call("GET", "/public/social-orders/short", expect=404)

    # --- Seller workflow ---
    oid = o1["order_id"]
    d = call("GET", f"/social/orders/{oid}", token=tok)
    assert d["order"]["payment_ref"] == "SLIP-889" and len(d["comments"]) == 2 and d["checkout_url"].endswith(token)
    call("POST", f"/social/orders/{oid}/status", {"status": "shipped"}, tok, expect=400)
    call("POST", f"/social/orders/{oid}/status", {"status": "paid"}, tok)
    call("POST", f"/social/orders/{oid}/status", {"status": "shipped", "tracking_no": "TH123456789"}, tok)
    done = call("POST", f"/social/orders/{oid}/status", {"status": "completed"}, tok)
    assert done["order"]["tracking_no"] == "TH123456789" and done["order"]["status"] == "completed"

    # edit + cancel releases stock
    wa_order = next(o for o in call("GET", f"/shops/{sid}/social/orders", token=tok) if o["customer_name"] == "Somchai")
    call("POST", f"/social/orders/{wa_order['id']}/items", {"product_id": b02["id"], "qty": 1}, tok)
    before = call("GET", f"/products/{b02['id']}", token=tok)["stock"]
    call("POST", f"/social/orders/{wa_order['id']}/status", {"status": "cancelled"}, tok)
    assert call("GET", f"/products/{b02['id']}", token=tok)["stock"] == before + 1

    lst = call("GET", f"/shops/{sid}/social/orders?q=SO000001", token=tok)
    assert len(lst) == 1 and lst[0]["status"] == "completed"
    call("POST", f"/social/sessions/{live['id']}/end", token=tok)
    other = call("POST", "/auth/register", {"email": f"o-{RUN}@t.dev", "password": "password123", "display_name": "O"})["token"]
    call("GET", f"/social/orders/{oid}", token=other, expect=403)
    call("GET", f"/shops/{sid}/social/channels", token=other, expect=403)
    print("ALL SOCIAL TESTS PASSED ✔")


if __name__ == "__main__":
    main()
