#!/usr/bin/env python3
"""Delivery (Anousith / HAL / Mixay / shop delivery) + cash-on-delivery tests.
Start the API with ADMIN_EMAILS=admin@demo.dev. Usage: python3 scripts/logistics_test.py [http://localhost:8080]
"""
import json, sys, urllib.error, urllib.request, uuid

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


ok = lambda m: print("  ✔", m)
print("logistics + COD tests")

admin = account("admin@demo.dev", "Admin")
seller = account(f"ship-{RUN}@t.dev", "Seller")
buyer = account(f"buyer-{RUN}@t.dev", "Buyer")
shop = call("POST", "/shops", {"slug": f"ship-{RUN}", "name": f"Vientiane Goods {RUN}", "currency": "LAK"}, seller)
sid = shop["id"]
call("PATCH", f"/admin/shops/{sid}", {"auto_approve": True}, admin)
mk = lambda sku, price, stock, code=None: call("POST", f"/shops/{sid}/products", {
    "sku": sku, "name": sku.title(), "price_cents": price, "status": "active", "initial_stock": stock, "category": "handmade", "social_code": code}, seller)
bag = mk(f"bag{RUN}", 15000000, 10, "L01")      # ₭150,000
cap = mk(f"cap{RUN}", 5000000, 10)               # ₭50,000

# ---- carriers --------------------------------------------------------------------------------
carriers = call("GET", "/carriers")
codes = [c["code"] for c in carriers]
assert {"anousith", "hal", "mixay", "self"} <= set(codes), codes
assert next(c for c in carriers if c["code"] == "hal")["name_lo"] == "ຮຸ່ງອາລຸນ ຂົນສົ່ງດ່ວນ"
code = f"lp{RUN}"
call("POST", "/admin/carriers", {"code": code, "name": "Lao Post", "tracking_url": "ftp://x"}, admin, expect=400)
call("POST", "/admin/carriers", {"code": "Bad Code", "name": "x"}, admin, expect=400)
call("POST", "/admin/carriers", {"code": code, "name": "Lao Post", "name_lo": "ໄປສະນີລາວ", "tracking_url": "https://track.example.la/?no={tracking}", "supports_cod": False}, admin)
call("POST", "/admin/carriers", {"code": code, "name": "dup"}, admin, expect=409)
call("POST", "/admin/carriers", {"code": "zz" + RUN, "name": "x"}, seller, expect=403)
call("PATCH", "/admin/carriers/hal", {"tracking_url": "https://hal.example/track/{tracking}"}, admin)
ok("courier list: Anousith, HAL, Mixay, shop delivery; admin can add/edit (with URL validation)")

# ---- seller shipping terms ------------------------------------------------------------------
opts = call("GET", f"/shops/{sid}/shipping", token=seller)
assert all(not o["enabled"] and not o["configured"] for o in opts)
term = lambda c, **kw: call("PUT", f"/shops/{sid}/shipping/{c}", {"enabled": True, "fee_payer": "buyer", "fee_cents": 0, **kw}, seller)
term("hal", fee_cents=2000000, home_fee_cents=3000000, cod_enabled=True, cod_fee_cents=500000, eta="1-2 days")   # ₭20k / ₭30k home / COD ₭5k
term("mixay", fee_payer="destination", fee_cents=2500000)                                                       # customer pays courier on pickup
term("anousith", fee_cents=1500000, free_over_cents=30000000, cod_enabled=True)                               # free over ₭300k
call("PUT", f"/shops/{sid}/shipping/{code}", {"enabled": True, "fee_payer": "buyer", "fee_cents": 1, "cod_enabled": True}, seller, expect=400)  # courier has no COD
call("PUT", f"/shops/{sid}/shipping/hal", {"enabled": True, "fee_payer": "nobody", "fee_cents": 1}, seller, expect=400)
call("PUT", f"/shops/{sid}/shipping/hal", {"enabled": True, "fee_payer": "buyer", "fee_cents": -1}, seller, expect=400)
call("GET", f"/shops/{sid}/shipping", token=buyer, expect=403)
pub = call("GET", f"/shipping/options?shops={sid},{uuid.uuid4()}")
mine = pub[sid]
assert [o["carrier_code"] for o in mine] == ["anousith", "hal", "mixay"], mine
assert next(o for o in mine if o["carrier_code"] == "hal")["cod_enabled"] and not next(o for o in mine if o["carrier_code"] == "mixay")["cod_enabled"]
ok("seller sets fee, who pays (checkout / destination / free over), home delivery, COD + fee; public options per shop")

# ---- checkout --------------------------------------------------------------------------------
addr = {"name": "Noy", "phone": "02055551234", "address": "Ban Phonxay, Saysettha, Vientiane", "branch": "HAL Saysettha"}
item = lambda p, q=1: [{"product_id": p["id"], "qty": q}]
call("POST", "/orders/checkout", {"items": item(bag), "shipping_address": addr}, buyer, expect=400)       # must choose delivery
e = call("POST", "/orders/checkout", {"items": item(bag), "shipping_address": addr}, buyer, expect=400, headers={"Accept-Language": "lo"})
assert "ກະລຸນາເລືອກວິທີຈັດສົ່ງສຳລັບ" in e["message"], e
dv = lambda c, t="branch", m="prepaid": [{"shop_id": sid, "carrier_code": c, "delivery_type": t, "payment_method": m}]
call("POST", "/orders/checkout", {"items": item(bag), "shipping_address": addr, "delivery": dv("mixay", m="cod")}, buyer, expect=400)   # no COD with Mixay
call("POST", "/orders/checkout", {"items": item(bag), "shipping_address": addr, "delivery": dv("mixay", "home")}, buyer, expect=400)    # no home delivery
call("POST", "/orders/checkout", {"items": item(bag), "shipping_address": addr, "delivery": dv("self")}, buyer, expect=400)             # not enabled by shop

o1 = call("POST", "/orders/checkout", {"items": item(bag), "shipping_address": addr, "delivery": dv("hal", "home", "cod")}, buyer)[0]
assert o1["payment_method"] == "cod" and o1["cod_status"] == "pending" and o1["carrier_code"] == "hal" and o1["delivery_type"] == "home"
assert o1["shipping_fee_cents"] == 3000000 and o1["cod_fee_cents"] == 500000
assert o1["grand_total_cents"] == 15000000 + 3000000 + 500000 == o1["cod_amount_cents"], o1
call("POST", f"/orders/{o1['id']}/pay", token=buyer, expect=400)
ok("COD order: home delivery fee + COD fee in the grand total = amount to collect; online pay refused")

o2 = call("POST", "/orders/checkout", {"items": item(cap), "shipping_address": addr, "delivery": dv("mixay")}, buyer)[0]
assert o2["fee_payer"] == "destination" and o2["shipping_fee_cents"] == 2500000 and o2["grand_total_cents"] == 5000000 and o2["cod_status"] == "none"
o3 = call("POST", "/orders/checkout", {"items": item(bag, 2), "shipping_address": addr, "delivery": dv("anousith", m="cod")}, buyer)[0]
assert o3["fee_payer"] == "seller" and o3["grand_total_cents"] == 30000000 == o3["cod_amount_cents"], o3
ok("fee paid at destination is not charged now; free shipping above the threshold")

# ---- shipping + COD ledger -------------------------------------------------------------------
st = lambda o, s, **kw: call("POST", f"/shops/{sid}/orders/{o['id']}/status", {"status": s, **kw}, seller)
call("POST", f"/shops/{sid}/orders/{o2['id']}/status", {"status": "shipped"}, seller, expect=400)        # prepaid must be paid first
s1 = st(o1, "shipped", tracking_no=" HAL-778899 ")
assert s1["status"] == "shipped" and s1["tracking_no"] == "HAL-778899" and s1["shipped_at"]
call("POST", f"/shops/{sid}/orders/{o3['id']}/status", {"status": "shipped", "carrier_code": "nope"}, seller, expect=400)
st(o3, "shipped", carrier_code="anousith", tracking_no="ANS1")
led = call("GET", f"/shops/{sid}/cod", token=seller)
row = next(r for r in led["rows"] if r["id"] == o1["id"])
assert row["cod_status"] == "pending" and row["customer"] == "Noy" and row["tracking_link"] == "https://hal.example/track/HAL-778899", row
pend = sum(t["amount_cents"] for t in led["totals"] if t["cod_status"] == "pending")
assert pend == o1["cod_amount_cents"] + o3["cod_amount_cents"], led["totals"]
ok("seller ships COD orders straight away with courier + tracking; ledger shows cash to collect and tracking links")

upd = lambda items, s, **kw: call("POST", f"/shops/{sid}/cod/update", {"items": [{"kind": "order", "id": o["id"]} for o in items], "status": s, **kw}, seller)
upd([o1], "collected")
assert call("GET", f"/orders/{o1['id']}", token=buyer)["status"] == "completed"
call("POST", f"/shops/{sid}/cod/update", {"items": [{"kind": "order", "id": o1["id"]}], "status": "collected"}, seller, expect=400)  # already
upd([o1], "remitted", reference="HAL transfer 27/09")
r1 = call("GET", f"/orders/{o1['id']}", token=buyer)
assert r1["cod_status"] == "remitted" and r1["cod_remit_ref"] == "HAL transfer 27/09" and r1["cod_remitted_at"]
ok("collected → order completed; remitted with the courier's transfer reference")

before = call("GET", f"/products/{bag['id']}", token=seller)["stock"]
upd([o3], "returned")
r3 = call("GET", f"/orders/{o3['id']}", token=buyer)
assert r3["status"] == "cancelled" and r3["cod_status"] == "returned"
assert call("GET", f"/products/{bag['id']}", token=seller)["stock"] == before + 2
moves = call("GET", f"/shops/{sid}/inventory/movements", token=seller)
assert any(m["reason"] == "return" for m in (moves if isinstance(moves, list) else moves.get("items", []))), "return movement"
other = account(f"other-{RUN}@t.dev", "Other")
oshop = call("POST", "/shops", {"slug": f"oth-{RUN}", "name": "Other"}, other)
call("POST", f"/shops/{oshop['id']}/cod/update", {"items": [{"kind": "order", "id": o1["id"]}], "status": "remitted"}, other, expect=403)
ok("refused parcel → returned: order cancelled, stock back (reason 'return'); other shops can't touch the ledger")

# cancelled COD order leaves the ledger
o4 = call("POST", "/orders/checkout", {"items": item(cap), "shipping_address": addr, "delivery": dv("hal", m="cod")}, buyer)[0]
call("POST", f"/orders/{o4['id']}/cancel", token=buyer)
assert call("GET", f"/orders/{o4['id']}", token=buyer)["cod_status"] == "none"
# marking completed also records the cash as collected
o5 = call("POST", "/orders/checkout", {"items": item(cap), "shipping_address": addr, "delivery": dv("hal", m="cod")}, buyer)[0]
call("POST", f"/shops/{sid}/cod/update", {"items": [{"kind": "order", "id": o5["id"]}], "status": "collected"}, seller, expect=400)  # not shipped yet
st(o5, "shipped", tracking_no="H5")
done = st(o5, "completed")
assert done["cod_status"] == "collected", done
ok("buyer cancellation clears COD; completing a COD order marks the cash collected")

# ---- social (comment) order with courier + COD ----------------------------------------------
call("POST", f"/shops/{sid}/social/sessions", {"title": "Live"}, seller)
c = call("POST", f"/shops/{sid}/social/simulate", {"user_name": "Mali", "message": "CF L01"}, seller)
tok = c["checkout_url"].rsplit("/", 1)[1]
pubo = call("GET", f"/public/social-orders/{tok}")
assert [o["carrier_code"] for o in pubo["shipping_options"]] == ["anousith", "hal", "mixay"]
info = {"name": "Mali", "phone": "02099998888", "address": "Ban Nongbone, Vientiane"}
call("POST", f"/public/social-orders/{tok}/confirm", info, expect=400)                                           # courier required
call("POST", f"/public/social-orders/{tok}/confirm", {**info, "carrier_code": "mixay", "cod": True}, expect=400)  # no COD
sc = call("POST", f"/public/social-orders/{tok}/confirm", {**info, "carrier_code": "hal", "delivery_type": "branch", "cod": True})
so = sc["order"]
assert so["carrier_code"] == "hal" and so["payment_method"] == "cod" and so["cod_status"] == "pending"
assert so["shipping_cents"] == 2000000 and so["cod_fee_cents"] == 500000 and so["total_cents"] == 15000000 + 2000000 + 500000 == so["cod_amount_cents"], so
call("POST", f"/public/social-orders/{tok}/payment", {"method": "cod", "reference": "x"}, expect=400)
shp = call("POST", f"/social/orders/{so['id']}/status", {"status": "shipped", "tracking_no": "HAL-S1"}, seller)
assert shp["order"]["status"] == "shipped" and shp["order"]["shipped_at"]
led = call("GET", f"/shops/{sid}/cod?status=pending", token=seller)
assert any(r["kind"] == "social" and r["id"] == so["id"] for r in led["rows"])
call("POST", f"/shops/{sid}/cod/update", {"items": [{"kind": "social", "id": so["id"]}], "status": "collected"}, seller)
assert call("GET", f"/social/orders/{so['id']}", token=seller)["order"]["status"] == "completed"
# switching to prepaid mixay (fee at destination) drops shipping from the total
c2 = call("POST", f"/shops/{sid}/social/simulate", {"user_name": "Toy", "message": "CF L01"}, seller)
tok2 = c2["checkout_url"].rsplit("/", 1)[1]
so2 = call("POST", f"/public/social-orders/{tok2}/confirm", {**info, "carrier_code": "mixay"})["order"]
assert so2["fee_payer"] == "destination" and so2["shipping_cents"] == 0 and so2["total_cents"] == 15000000 and so2["cod_status"] == "none", so2
ok("comment orders: customer picks courier + COD on the checkout page; COD ships without payment; ledger covers them")

print("ALL LOGISTICS TESTS PASSED ✔")
