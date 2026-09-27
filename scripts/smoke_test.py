#!/usr/bin/env python3
"""End-to-end smoke test for the zaokaiy API.

Flow: supplier opens shop + product -> creator requests sell-staff partnership ->
supplier approves with custom commission -> creator lists product -> buyer buys via
creator storefront -> supplier ships/completes -> commission approved -> paid.
Also exercises inventory, ads, content and AI endpoints.

Usage: python3 scripts/smoke_test.py [http://localhost:8080]
"""
import json, sys, uuid, urllib.request, urllib.error

BASE = (sys.argv[1] if len(sys.argv) > 1 else "http://localhost:8080") + "/api"
RUN = uuid.uuid4().hex[:6]


def req(method, path, body=None, token=None, expect=200):
    data = json.dumps(body).encode() if body is not None else None
    r = urllib.request.Request(BASE + path, data=data, method=method)
    r.add_header("Content-Type", "application/json")
    if token:
        r.add_header("Authorization", f"Bearer {token}")
    try:
        with urllib.request.urlopen(r) as resp:
            code, out = resp.status, json.loads(resp.read() or b"null")
    except urllib.error.HTTPError as e:
        code, out = e.code, json.loads(e.read() or b"null")
    assert code == expect, f"{method} {path} -> {code} (expected {expect}): {out}"
    return out


ADMIN_EMAIL = "admin@demo.dev"  # must be listed in the API's ADMIN_EMAILS


def admin_token():
    try:
        return req("POST", "/auth/login", {"email": ADMIN_EMAIL, "password": "password123"})["token"]
    except AssertionError:
        return req("POST", "/auth/register", {"email": ADMIN_EMAIL, "password": "password123", "display_name": "Admin"})["token"]


def user(name):
    r = req("POST", "/auth/register", {"email": f"{name}-{RUN}@test.dev", "password": "password123", "display_name": name})
    return r["token"]


def main():
    sup, cre, buyer = user("supplier"), user("creator"), user("buyer")

    shop = req("POST", "/shops", {"slug": f"sup-{RUN}", "name": "Siam Crafts"}, sup)
    cshop = req("POST", "/shops", {"slug": f"cre-{RUN}", "name": "Bee Picks", "kind": "creator"}, cre)
    req("POST", "/shops", {"slug": f"sup-{RUN}", "name": "dup"}, cre, expect=409)

    p = req("POST", f"/shops/{shop['id']}/products", {
        "sku": "MUG-1", "name": "Celadon Mug", "price_cents": 45000, "status": "active",
        "initial_stock": 10, "allow_resell": True, "commission_bps": 1000, "category": "home"}, sup)
    req("POST", f"/shops/{cshop['id']}/products", {"sku": "X", "name": "x", "price_cents": 1}, sup, expect=403)

    # moderation: product is queued until an admin approves it
    assert p["review_status"] in ("pending", "approved"), p
    if p["review_status"] == "pending":
        req("GET", f"/catalog/products/{p['id']}", expect=404)
        admin = admin_token()
        req("POST", f"/admin/products/{p['id']}/review", {"action": "approve"}, admin)

    # inventory
    req("POST", f"/products/{p['id']}/stock", {"delta": 5, "reason": "restock"}, sup)
    req("POST", f"/products/{p['id']}/stock", {"delta": -100, "reason": "adjust"}, sup, expect=400)
    movs = req("GET", f"/shops/{shop['id']}/inventory/movements", token=sup)
    assert len(movs) == 2, movs

    # reseller program
    market = req("GET", f"/marketplace/products?shop_id={cshop['id']}", token=cre)
    assert any(m["id"] == p["id"] for m in market)
    req("POST", f"/shops/{cshop['id']}/listings", {"product_id": p["id"]}, cre, expect=400)  # not approved yet
    pa = req("POST", f"/shops/{cshop['id']}/partnerships", {"supplier_shop_id": shop["id"], "message": "I have 50k followers"}, cre)
    req("POST", f"/partnerships/{pa['id']}/decision", {"approve": True}, cre, expect=403)  # reseller can't approve
    pa = req("POST", f"/partnerships/{pa['id']}/decision", {"approve": True, "commission_bps": 1500}, sup)
    assert pa["status"] == "approved" and pa["commission_bps"] == 1500
    req("POST", f"/shops/{cshop['id']}/listings", {"product_id": p["id"]}, cre)
    front = req("GET", f"/storefront/cre-{RUN}")
    assert front["resold"][0]["via_shop_id"] == cshop["id"]
    detail = req("GET", f"/catalog/products/{p['id']}?via={cshop['id']}")
    assert detail["via_shop"]["id"] == cshop["id"]

    # buyer checks out through creator storefront
    orders = req("POST", "/orders/checkout", {
        "items": [{"product_id": p["id"], "qty": 2, "via_shop_id": cshop["id"]}],
        "shipping_address": {"name": "Buyer", "line1": "1 Sukhumvit", "city": "Bangkok"}}, buyer)
    o = orders[0]
    assert o["total_cents"] == 90000 and o["items"][0]["commission_cents"] == 13500, o
    req("POST", "/orders/checkout", {"items": [{"product_id": p["id"], "qty": 99}], "shipping_address": {}}, buyer, expect=400)
    req("POST", f"/orders/{o['id']}/pay", token=buyer)
    req("POST", f"/shops/{shop['id']}/orders/{o['id']}/status", {"status": "completed"}, sup, expect=400)  # must ship first
    req("POST", f"/shops/{shop['id']}/orders/{o['id']}/status", {"status": "shipped"}, sup)
    req("POST", f"/shops/{shop['id']}/orders/{o['id']}/status", {"status": "completed"}, sup)

    earned = req("GET", f"/shops/{cshop['id']}/commissions", token=cre)
    assert earned["summary"]["approved"] == 13500, earned
    payable = req("GET", f"/shops/{shop['id']}/commissions/payable", token=sup)
    req("POST", f"/commissions/{payable['items'][0]['id']}/pay", token=sup)
    assert req("GET", f"/shops/{cshop['id']}/commissions", token=cre)["summary"]["paid"] == 13500

    # cancellation restocks + voids commission
    o2 = req("POST", "/orders/checkout", {"items": [{"product_id": p["id"], "qty": 1, "via_shop_id": cshop["id"]}], "shipping_address": {}}, buyer)[0]
    req("POST", f"/orders/{o2['id']}/cancel", token=buyer)
    prod = req("GET", f"/products/{p['id']}", token=sup)
    assert prod["stock"] == 13, prod["stock"]  # 15 - 2 (sold) ; cancelled one returned
    assert req("GET", f"/shops/{cshop['id']}/commissions", token=cre)["summary"]["void"] == 6750

    # ads
    ad = req("POST", f"/shops/{cshop['id']}/ads", {"product_id": p["id"], "headline": "Handmade celadon", "budget_cents": 300, "cpc_cents": 100, "status": "active"}, cre)
    served = req("GET", "/ads/serve?limit=10")
    assert any(a["id"] == ad["id"] and a["via_shop_id"] == cshop["id"] for a in served)
    for _ in range(3):
        req("POST", f"/ads/{ad['id']}/click")
    req("POST", f"/ads/{ad['id']}/click", expect=404)  # budget exhausted
    ads = req("GET", f"/shops/{cshop['id']}/ads", token=cre)
    assert ads[0]["spent_cents"] == 300 and ads[0]["status"] == "ended", ads[0]

    # content + AI
    gen = req("POST", f"/shops/{cshop['id']}/ai/generate", {"product_id": p["id"], "kind": "video_script", "save": True}, cre)
    assert gen["saved"]["status"] == "draft"
    req("PATCH", f"/contents/{gen['saved']['id']}", {"status": "published"}, cre)
    feed = req("GET", "/feed/contents")
    assert any(f["id"] == gen["saved"]["id"] and f["product"]["via_shop_id"] == cshop["id"] for f in feed)
    agent = req("POST", f"/shops/{shop['id']}/ai/agent", {"messages": [{"role": "user", "content": "How is my shop doing?"}]}, sup)
    assert agent["reply"]

    stats = req("GET", f"/shops/{shop['id']}/stats", token=sup)
    assert stats["revenue_cents"] == 90000 and stats["resellers"] == 1, stats
    print("ALL SMOKE TESTS PASSED ✔")


if __name__ == "__main__":
    main()
