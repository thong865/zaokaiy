#!/usr/bin/env python3
"""Promotions & loyalty points (module `promo`): sign-up coupons, code campaigns, first-order bonus,
points earned/spent on the web, in chat orders and at the POS, reversal on cancel/void.

Run scripts/mock_graph.py, then start the API with:
  ADMIN_EMAILS=admin@demo.dev PROMO_JOB_SECS=1 META_GRAPH_URL=http://127.0.0.1:9998 \\
  WHATSAPP_TOKEN=watok WHATSAPP_PHONE_NUMBER_ID=12345 OTP_DEV_ECHO=true OTP_RESEND_SECONDS=2
Usage: python3 scripts/promo_test.py [http://localhost:8080]
"""
import json, random, sys, time, urllib.error, urllib.request, uuid

BASE = sys.argv[1] if len(sys.argv) > 1 else "http://localhost:8080"
API = BASE + "/api"
RUN = uuid.uuid4().hex[:6]


def call(method, path, body=None, token=None, expect=200, headers=None):
    r = urllib.request.Request(API + path, data=json.dumps(body).encode() if body is not None else None, method=method)
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
    out = json.loads(payload or b"null")
    assert code == expect, f"{method} {path} -> {code} (expected {expect}): {out}"
    return out


def account(email, name):
    try:
        return call("POST", "/auth/login", {"email": email, "password": "password123"})["token"]
    except AssertionError:
        return call("POST", "/auth/register", {"email": email, "password": "password123", "display_name": name})["token"]


def wait_for(fn, what, secs=15):
    end = time.time() + secs
    while time.time() < end:
        v = fn()
        if v:
            return v
        time.sleep(0.5)
    raise AssertionError(f"timed out waiting for {what}")


ok = lambda m: print("  ✔", m)
print("promotions + loyalty tests")
assert call("GET", "/modules")["promo"]["enabled"]

admin = account("admin@demo.dev", "Admin")
seller = account(f"promo-{RUN}@t.dev", "Seller")
shop = call("POST", "/shops", {"slug": f"promo-{RUN}", "name": f"Promo Shop {RUN}", "currency": "LAK"}, seller)
sid = shop["id"]
call("PATCH", f"/admin/shops/{sid}", {"auto_approve": True}, admin)
bag = call("POST", f"/shops/{sid}/products", {"sku": f"bag{RUN}", "name": "Bag", "price_cents": 5000000, "status": "active",
                                               "initial_stock": 50, "category": "handmade", "social_code": "P01"}, seller)

# ---- platform campaigns (admin) ------------------------------------------------------------------
camp = lambda body, **kw: call("POST", "/admin/promo/campaigns", body, admin, **kw)
call("POST", "/admin/promo/campaigns", {"name": "x", "trigger": "signup", "reward_type": "amount", "reward_value": 1}, seller, expect=403)
camp({"name": "x", "trigger": "first_order", "reward_type": "amount", "reward_value": 1}, expect=400)       # shops only
camp({"name": "x", "trigger": "signup", "reward_type": "points", "reward_value": 1}, expect=400)            # points are per shop
e = camp({"name": "x", "trigger": "signup", "reward_type": "percent", "reward_value": 0}, expect=400, headers={"Accept-Language": "lo"})
assert "ເປີເຊັນ" in e["message"], e
wa = camp({"name": f"WhatsApp welcome {RUN}", "trigger": "signup", "providers": ["whatsapp"], "reward_type": "amount",
           "reward_value": 1000000, "currency": "LAK", "coupon_days": 30, "channels": ["web", "social", "pos"]})
assert wa["providers"] == ["whatsapp"] and wa["grants"] == 0
em = camp({"name": f"Email welcome {RUN}", "trigger": "signup", "providers": ["email"], "reward_type": "free_shipping", "currency": "LAK"})
ok("admin creates sign-up campaigns per sign-up method; rules checked (Lao errors)")

# ---- shop programme ------------------------------------------------------------------------------
call("PUT", f"/shops/{sid}/loyalty", {"enabled": True, "earn_per_cents": 0, "point_value_cents": 1, "min_redeem_points": 1, "max_redeem_bps": 5000}, seller, expect=400)
lo = call("PUT", f"/shops/{sid}/loyalty", {"enabled": True, "earn_per_cents": 100000, "point_value_cents": 10000,
                                            "min_redeem_points": 5, "max_redeem_bps": 5000, "expiry_days": 365}, seller)
assert lo["enabled"] and lo["stats"]["members"] == 0
# 1 point per ₭1,000 spent; 1 point = ₭100 off; at most 50% of a bill.
sc = lambda body, **kw: call("POST", f"/shops/{sid}/promo/campaigns", body, seller, **kw)
sc({"name": "x", "trigger": "signup", "reward_type": "amount", "reward_value": 1}, expect=400)
first = sc({"name": "First order bonus", "trigger": "first_order", "reward_type": "points", "reward_value": 20})
live = sc({"name": "Live 10%", "trigger": "code", "code": "live" + RUN[:4], "reward_type": "percent", "reward_value": 1000, "max_discount_cents": 1000000})
LIVE = live["code"]
assert LIVE == ("LIVE" + RUN[:4]).upper()
sc({"name": "dup", "trigger": "code", "code": LIVE, "reward_type": "amount", "reward_value": 1}, expect=409)
assert len(call("GET", f"/shops/{sid}/promo/campaigns", token=seller)) == 2
ok("shop sets up points (earn / value / cap / expiry) + first-order bonus + a live code")

# ---- sign-up coupons -----------------------------------------------------------------------------
ph = "+85620" + str(random.randint(10_000_000, 99_999_999))
s = call("POST", "/auth/whatsapp/send", {"phone": ph})
w = call("POST", "/auth/whatsapp/verify", {"phone": ph, "code": s["dev_code"], "display_name": "Nok"})
buyer = w["token"]
mail = account(f"mail-{RUN}@t.dev", "Mail")
rw = wait_for(lambda: [c for c in call("GET", "/me/rewards", token=buyer)["coupons"] if c["campaign"] == wa["name"]], "WhatsApp sign-up coupon")
wcode = rw[0]["code"]
assert rw[0]["reward_type"] == "amount" and rw[0]["status"] == "active" and rw[0]["shop_id"] is None
mine = wait_for(lambda: call("GET", "/me/rewards", token=mail)["coupons"], "email sign-up coupon")
assert [c["campaign"] for c in mine if c["campaign"].endswith(RUN)] == [em["name"]], mine   # email user gets only the email campaign
assert not [c for c in call("GET", "/me/rewards", token=buyer)["coupons"] if c["campaign"] == em["name"]]
ok("WhatsApp sign-up → WhatsApp coupon; email sign-up → email coupon (job hands them out)")

# ---- web checkout with the sign-up coupon --------------------------------------------------------
addr = {"name": "Nok", "phone": ph, "address": "Ban Sisaket, Vientiane"}
item = lambda n=1: [{"product_id": bag["id"], "qty": n}]
q = call("POST", "/promo/quote", {"code": wcode, "shops": [{"shop_id": sid, "subtotal_cents": 5000000}]}, buyer)
assert q["applied"][0]["discount_cents"] == 1000000 and q["applied"][0]["platform_cents"] == 1000000, q
call("POST", "/promo/quote", {"code": wcode, "shops": [{"shop_id": sid, "subtotal_cents": 5000000}]}, mail, expect=400)   # not theirs
call("POST", "/promo/quote", {"code": "NOPE", "shops": [{"shop_id": sid, "subtotal_cents": 5000000}]}, buyer, expect=400)
o1 = call("POST", "/orders/checkout", {"items": item(), "shipping_address": addr, "promo": {"code": wcode}}, buyer)[0]
assert o1["discount_cents"] == 1000000 and o1["platform_discount_cents"] == 1000000 and o1["grand_total_cents"] == 4000000, o1
call("POST", "/orders/checkout", {"items": item(), "shipping_address": addr, "promo": {"code": wcode}}, buyer, expect=400)   # used
ok("coupon previewed and used at checkout (platform-funded), then refused a second time")

# Paid → shipped → completed: points earned + first-order bonus.
call("POST", f"/orders/{o1['id']}/pay", token=buyer)
call("POST", f"/shops/{sid}/orders/{o1['id']}/status", {"status": "shipped"}, seller)
call("POST", f"/shops/{sid}/orders/{o1['id']}/status", {"status": "completed"}, seller)
pts = call("GET", "/me/rewards", token=buyer)["points"]
assert pts[0]["shop_id"] == sid and pts[0]["balance"] == 40 + 20, pts          # ₭40,000 → 40 + 20 bonus
ok("completed order earns points on what was paid (40) + first-order bonus (20)")

# ---- points + live code on the web; cancel gives both back ---------------------------------------
q = call("POST", "/promo/quote", {"code": LIVE, "points": {sid: 30}, "shops": [{"shop_id": sid, "subtotal_cents": 5000000}]}, buyer)
a = q["applied"][0]
assert (a["coupon_cents"], a["points"], a["points_cents"], a["discount_cents"]) == (500000, 30, 300000, 800000), a
assert q["points"][0]["balance"] == 60 and not q["points_need_verified_phone"]
q = call("POST", "/promo/quote", {"points": {sid: 60}, "shops": [{"shop_id": sid, "subtotal_cents": 1000000}]}, buyer)
assert q["applied"][0]["points"] == 50, q        # capped at 50% of ₭10,000 = 50 points
call("POST", "/promo/quote", {"points": {sid: 3}, "shops": [{"shop_id": sid, "subtotal_cents": 5000000}]}, buyer, expect=400)   # min 5
call("POST", "/promo/quote", {"points": {sid: 10}, "shops": [{"shop_id": sid, "subtotal_cents": 5000000}]}, mail, expect=400)  # no member / unverified
o2 = call("POST", "/orders/checkout", {"items": item(), "shipping_address": addr, "promo": {"code": LIVE, "points": {sid: 30}}}, buyer)[0]
assert o2["discount_cents"] == 800000 and o2["platform_discount_cents"] == 0 and o2["grand_total_cents"] == 4200000, o2
assert call("GET", "/me/rewards", token=buyer)["points"][0]["balance"] == 30
call("POST", "/orders/checkout", {"items": item(), "shipping_address": addr, "promo": {"code": LIVE}}, buyer, expect=400)   # once per customer
call("POST", f"/orders/{o2['id']}/cancel", token=buyer)
assert call("GET", "/me/rewards", token=buyer)["points"][0]["balance"] == 60
q = call("POST", "/promo/quote", {"code": LIVE, "shops": [{"shop_id": sid, "subtotal_cents": 5000000}]}, buyer)   # code usable again
assert q["applied"][0]["coupon_cents"] == 500000
ok("live code + points on the web (50% cap, minimum); cancelling returns points and frees the code")

# ---- chat order (CF) -----------------------------------------------------------------------------
c = call("POST", f"/shops/{sid}/social/simulate", {"user_name": "Mali", "message": "CF P01"}, seller)
tok = c["checkout_url"].rsplit("/", 1)[1]
mali = "020" + str(random.randint(10_000_000, 99_999_999))
pq = call("POST", f"/public/social-orders/{tok}/promo", {"code": LIVE, "phone": mali})
assert pq["applied"]["discount_cents"] == 500000 and pq["points_enabled"] and not pq["points_trusted"], pq
call("POST", f"/public/social-orders/{tok}/promo", {"code": wcode, "phone": mali}, expect=400)          # used / not hers
info = {"name": "Mali", "phone": mali, "address": "Ban Nongbone, Vientiane"}
so = call("POST", f"/public/social-orders/{tok}/confirm", {**info, "promo": {"code": LIVE}})["order"]
assert so["discount_cents"] == 500000 and so["total_cents"] == 4500000, so
pq = call("POST", f"/public/social-orders/{tok}/promo", {"code": LIVE, "phone": mali})   # its own code still previews
assert pq["applied"]["discount_cents"] == 500000 and pq["current"] == {"code": LIVE, "points": 0}, pq
so = call("POST", f"/public/social-orders/{tok}/confirm", {**info, "promo": {"code": LIVE}})["order"]   # confirming again re-applies once
assert so["total_cents"] == 4500000
call("POST", f"/social/orders/{so['id']}/status", {"status": "paid"}, seller)
call("POST", f"/social/orders/{so['id']}/status", {"status": "shipped", "tracking_no": "HAL-1"}, seller)
done = call("POST", f"/social/orders/{so['id']}/status", {"status": "completed"}, seller)
assert done["notify"]["status"] in ("none", "unsupported"), done["notify"]      # simulator: no chat to message
members = call("GET", f"/shops/{sid}/loyalty/members", token=seller)
m = next(x for x in members if x["name"] == "Mali")
assert m["balance"] == 45 + 20 and m["orders"] == 1, m      # ₭45,000 → 45 points + first-order bonus
# Points in a chat order need the member's verified phone.
c2 = call("POST", f"/shops/{sid}/social/simulate", {"user_name": "Nok", "message": "CF P01"}, seller)
tok2 = c2["checkout_url"].rsplit("/", 1)[1]
call("POST", f"/public/social-orders/{tok2}/promo", {"points": 10, "phone": ph}, expect=400)
pq = call("POST", f"/public/social-orders/{tok2}/promo", {"points": 10, "phone": ph}, headers={"Authorization": f"Bearer {buyer}"})
assert pq["applied"]["points_cents"] == 100000 and pq["points_trusted"]
ok("chat order: code on the order link, points earned on completion; spending points needs the member's sign-in")

# ---- POS -----------------------------------------------------------------------------------------
lk = call("GET", f"/shops/{sid}/loyalty/lookup?phone={ph}", token=seller)
assert lk["member"]["balance"] == 60 and lk["settings"]["point_value_cents"] == 10000
call("GET", f"/shops/{sid}/loyalty/lookup?phone=abc", token=seller, expect=400)
pq = call("POST", f"/shops/{sid}/loyalty/quote", {"member_phone": ph, "points": 20, "subtotal_cents": 5000000}, seller)
assert pq["points_cents"] == 200000 and pq["points_balance"] == 60, pq
sale = {"items": [{"product_id": bag["id"], "qty": 1}], "payments": [{"method": "cash", "amount_cents": 5000000}],
        "promo": {"member_phone": ph, "points": 20}}
doc = call("POST", f"/shops/{sid}/pos/sales", sale, seller)
s1 = doc["sale"]
assert s1["discount_cents"] == 200000 and s1["total_cents"] == 4800000 and s1["member_phone"] == ph, s1
m = call("GET", f"/shops/{sid}/loyalty/lookup?phone={ph}", token=seller)["member"]
assert m["balance"] == 60 - 20 + 48, m            # spent 20, earned 48 on ₭48,000
call("POST", f"/pos/sales/{s1['id']}/void", {"reason": "test"}, seller)
m = call("GET", f"/shops/{sid}/loyalty/lookup?phone={ph}", token=seller)["member"]
assert m["balance"] == 60, m
det = call("GET", f"/shops/{sid}/loyalty/members/{m['id']}", token=seller)
assert [l["reason"] for l in det["ledger"]][:4] == ["reverse", "reverse", "earn", "redeem"], det["ledger"][:4]
ok("POS: member lookup, points off the bill, points earned; void puts everything back")

# ---- adjust, liability, staff --------------------------------------------------------------------
call("POST", f"/shops/{sid}/loyalty/members/{m['id']}/adjust", {"delta": -1000, "note": "x"}, seller, expect=400)
adj = call("POST", f"/shops/{sid}/loyalty/members/{m['id']}/adjust", {"delta": 5, "note": "goodwill"}, seller)
assert adj["member"]["balance"] == 65
liab = call("GET", "/admin/promo/liability", token=admin)
row = next(r for r in liab if r["shop_id"] == sid)
assert row["amount_cents"] == 1000000 and row["sales"] == 1, row
lst = call("GET", "/admin/promo/campaigns", token=admin)
assert next(x for x in lst if x["id"] == wa["id"])["used"] == 1
call("GET", f"/shops/{sid}/loyalty", token=buyer, expect=403)
ok("manual adjustment (never below zero); platform owes the shop its coupons; outsiders blocked")

print("ALL PROMO TESTS PASSED ✔")
