#!/usr/bin/env python3
"""POS terminals (desktop app): pairing, catalogue pull with cursors, offline sales push, voids.
Start the API with ADMIN_EMAILS=admin@demo.dev. Usage: python3 scripts/pos_sync_test.py [http://localhost:8080]
"""
import json, sys, urllib.error, urllib.request, uuid
from datetime import datetime, timezone, timedelta

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


def vat_incl(net, bps):
    return (net * bps * 2 + (10000 + bps)) // ((10000 + bps) * 2)


def sale(seq, code, lines, pay_method="cash", pay=None, discount=0, vat_bps=700, prefix="R", when=None):
    subtotal = sum(l["unit_price_cents"] * l["qty"] - l.get("discount_cents", 0) for l in lines)
    net = subtotal - discount
    vat = vat_incl(net, vat_bps)
    pay = pay if pay is not None else net
    return {
        "id": str(uuid.uuid4()), "seq": seq, "number": f"{prefix}{code}-{seq:06d}",
        "created_at": (when or datetime.now(timezone.utc)).isoformat(),
        "items": lines, "discount_cents": discount, "vat_bps": vat_bps, "prices_include_vat": True,
        "currency": "LAK", "subtotal_cents": subtotal, "vat_cents": vat, "total_cents": net,
        "paid_cents": pay, "change_cents": pay - net, "payments": [{"method": pay_method, "amount_cents": pay}],
    }


def pull_all(tok, cursor=None, limit=None):
    """Follow has_more pages like the terminal does; returns (cursor, products by id, deleted, pages)."""
    prods, deleted, pages = {}, [], 0
    while True:
        q = f"/pos/device/pull?cursor={cursor or ''}" + (f"&limit={limit}" if limit else "")
        r = call("GET", q, token=tok)
        pages += 1
        for p in r["products"]:
            prods[p["id"]] = p
        deleted += r["deleted"]
        cursor = r["cursor"]
        if not r["has_more"]:
            return cursor, prods, deleted, pages, r


ok = lambda m: print("  ✔", m)
print("POS sync (desktop terminals) tests")

owner = account(f"possync-{RUN}@t.dev", "Owner")
admin = account("admin@demo.dev", "Admin")
shop = call("POST", "/shops", {"slug": f"possync-{RUN}", "name": f"Dao Market {RUN}", "currency": "LAK"}, owner)
sid = shop["id"]
call("PATCH", f"/admin/shops/{sid}", {"auto_approve": True}, admin)
prods = [call("POST", f"/shops/{sid}/products", {"sku": f"ps{RUN}{i}", "name": f"Item {i}", "price_cents": 1000000 + i * 100000,
                                                  "status": "active", "initial_stock": 10, "category": "handmade"}, owner) for i in range(7)]
rice = prods[0]

# ---- pairing ------------------------------------------------------------------------------------
install = f"install-{RUN}-abcdef"
st = call("POST", "/pos/pair/start", {"name": "Front counter", "platform": "windows", "app_version": "0.1.0", "install_id": install})
assert len(st["code"]) == 9 and st["code"][4] == "-" and st["poll_secret"]
assert call("POST", "/pos/pair/poll", {"pair_id": st["pair_id"], "poll_secret": st["poll_secret"]})["status"] == "pending"
call("POST", "/pos/pair/poll", {"pair_id": st["pair_id"], "poll_secret": "wrong"}, expect=400)
info = call("GET", f"/pos/pair/{st['code'].lower()}", token=owner)
assert info["status"] == "pending" and info["name"] == "Front counter"
stranger = account(f"stranger-{RUN}@t.dev", "Stranger")
call("POST", "/pos/pair/approve", {"code": st["code"], "shop_id": sid}, stranger, expect=403)
call("POST", "/pos/pair/approve", {"code": st["code"], "shop_id": sid}, owner)
call("POST", "/pos/pair/approve", {"code": st["code"], "shop_id": sid}, owner, expect=400)   # once
p = call("POST", "/pos/pair/poll", {"pair_id": st["pair_id"], "poll_secret": st["poll_secret"]})
assert p["status"] == "approved" and p["token"].startswith("zkd_") and p["shop"]["id"] == sid
tok = p["token"]
call("POST", "/pos/pair/poll", {"pair_id": st["pair_id"], "poll_secret": st["poll_secret"]}, expect=400)  # token handed out once
me = call("GET", "/pos/device/me", token=tok)
assert me["device"]["code"] == "T01" and me["device"]["last_seq"] == 0 and me["device"]["cashier"]["can_void"]
assert me["shop"]["currency"] == "LAK" and me["shop"]["receipt_prefix"] == "R"
call("GET", "/pos/device/me", token=owner, expect=401)          # a user JWT is not a device token
call("GET", "/pos/device/me", token="zkd_nope", expect=401)
ok("pairing: terminal gets a code, a signed-in owner approves it, the terminal collects a device token once (T01)")

# ---- pull ---------------------------------------------------------------------------------------
cur, got, deleted, pages, last = pull_all(tok, limit=3)
assert pages == 3 and len(got) == 7 and not deleted and last["cursor"].startswith("c:"), (pages, len(got))
assert got[rice["id"]]["stock"] == 10 and got[rice["id"]]["price_cents"] == 1000000
cur2, got2, _, _, _ = pull_all(tok, cur)
assert not got2, "nothing changed → nothing pulled"
call("PATCH", f"/products/{prods[1]['id']}", {"price_cents": 1234500}, owner)
call("DELETE", f"/products/{prods[6]['id']}", token=owner)
cur3, got3, deleted3, _, _ = pull_all(tok, cur2)
assert set(got3) == {prods[1]["id"], prods[6]["id"]} and got3[prods[1]["id"]]["price_cents"] == 1234500, got3
assert got3[prods[6]["id"]]["status"] == "archived" and not deleted3        # deleting a product archives it
ok("pull: paged full snapshot, then only products changed since the cursor (archived ones included)")

# ---- push offline sales --------------------------------------------------------------------------
L = lambda p, q, **kw: {"product_id": p["id"], "name": p["name"], "sku": p["sku"], "qty": q, "unit_price_cents": p["price_cents"], **kw}
yesterday = datetime.now(timezone.utc) - timedelta(hours=20)
s1 = sale(1, "T01", [L(rice, 3)], when=yesterday)
s2 = sale(2, "T01", [L(rice, 9), {"product_id": None, "name": "Plastic bag", "qty": 1, "unit_price_cents": 50000}], pay_method="cash", pay=40000000)
s3 = sale(3, "T01", [{"product_id": str(uuid.uuid4()), "name": "Item gone from the server", "qty": 1, "unit_price_cents": 1600000}], pay_method="qr")
bad = sale(4, "T01", [L(rice, 1)])
bad["total_cents"] += 1
r = call("POST", "/pos/device/push", {"sales": [s1, s2, s3, bad]}, tok)
res = {x["id"]: x for x in r["sales"]}
assert res[s1["id"]]["status"] == "ok" and not res[s1["id"]]["shortfall"]
assert res[s2["id"]]["status"] == "ok" and res[s2["id"]]["shortfall"] == [{"product_id": rice["id"], "name": rice["name"], "qty": 2}], res[s2["id"]]
assert res[s3["id"]]["status"] == "ok"
assert res[bad["id"]]["status"] == "rejected" and "totals do not match" in res[bad["id"]]["error"]
assert call("GET", f"/products/{rice['id']}", token=owner)["stock"] == 0          # 10 − 3 − 7 (2 short)
doc = call("GET", f"/pos/sales/{s1['id']}", token=owner)
assert doc["sale"]["number"] == "RT01-000001" and doc["sale"]["total_cents"] == s1["total_cents"]
assert doc["sale"]["created_at"][:13] == yesterday.isoformat()[:13]                 # keeps the time it was sold
assert call("GET", f"/pos/sales/{s3['id']}", token=owner)["items"][0]["product_id"] is None
ok("push: offline sales keep their receipt numbers and time, stock can't go below zero (shortfall recorded), bad totals rejected")

r = call("POST", "/pos/device/push", {"sales": [s1, s2]}, tok)
assert [x["status"] for x in r["sales"]] == ["duplicate", "duplicate"]
assert call("GET", f"/products/{rice['id']}", token=owner)["stock"] == 0
clash = sale(1, "T01", [L(rice, 1)])
r = call("POST", "/pos/device/push", {"sales": [clash]}, tok, headers={"Accept-Language": "lo"})
assert r["sales"][0]["status"] == "rejected" and r["sales"][0]["error"] == "ເລກໃບບິນນີ້ຖືກໃຊ້ແລ້ວ", r
assert call("GET", "/pos/device/me", token=tok)["device"]["last_seq"] == 3
ok("push is idempotent (re-sent sales are 'duplicate'), receipt numbers can't be reused, errors in Lao")

# ---- voids --------------------------------------------------------------------------------------
call("POST", f"/products/{rice['id']}/stock", {"delta": 5, "reason": "restock"}, owner)
r = call("POST", "/pos/device/push", {"voids": [{"sale_id": s2["id"], "reason": "customer changed mind"}]}, tok)
assert r["voids"][0]["status"] == "ok", r
assert call("GET", f"/products/{rice['id']}", token=owner)["stock"] == 5 + 7     # only the 7 taken come back
r = call("POST", "/pos/device/push", {"voids": [{"sale_id": s2["id"], "reason": "again"}, {"sale_id": str(uuid.uuid4()), "reason": "x"}]}, tok)
assert [v["status"] for v in r["voids"]] == ["duplicate", "rejected"]
ok("voids from the terminal restock only what was taken, are idempotent")

# ---- staff terminal without void permission, revocation, re-pair keeps the code -------------------
staff = call("GET", f"/shops/{sid}/staff", token=owner)
cashier_role = next(x["id"] for x in staff["roles"] if x["name"] == "Cashier")
inv = call("POST", f"/shops/{sid}/staff/invites", {"role_id": cashier_role, "note": "till 2"}, owner)
noy = account(f"noy-{RUN}@t.dev", "Noy")
call("POST", f"/join/{inv['link'].rsplit('/', 1)[1]}", token=noy)
st2 = call("POST", "/pos/pair/start", {"name": "Till 2", "platform": "macos", "install_id": f"install2-{RUN}-xyz"})
call("POST", "/pos/pair/approve", {"code": st2["code"], "shop_id": sid}, noy)
tok2 = call("POST", "/pos/pair/poll", {"pair_id": st2["pair_id"], "poll_secret": st2["poll_secret"]})["token"]
me2 = call("GET", "/pos/device/me", token=tok2)
assert me2["device"]["code"] == "T02" and not me2["device"]["cashier"]["can_void"]
s5 = sale(1, "T02", [L(prods[2], 1)], pay_method="card")
r = call("POST", "/pos/device/push", {"sales": [s5], "voids": [{"sale_id": s5["id"], "reason": "oops"}]}, tok2)
assert r["sales"][0]["status"] == "ok" and r["voids"][0]["status"] == "rejected"
assert call("GET", f"/pos/sales/{s5['id']}", token=owner)["cashier"] == "Noy"
call("GET", f"/shops/{sid}/pos-devices", token=noy, expect=403)                   # owner only
devs = call("GET", f"/shops/{sid}/pos-devices", token=owner)
assert [d["code"] for d in devs] == ["T01", "T02"] and devs[0]["sales"] == 3 and devs[0]["last_receipt"] == "RT01-000003"
t01 = devs[0]["id"]
call("POST", f"/shops/{sid}/pos-devices/{t01}/revoke", token=owner)
call("GET", "/pos/device/me", token=tok, expect=401)
st3 = call("POST", "/pos/pair/start", {"name": "Front counter", "platform": "windows", "install_id": install})
call("POST", "/pos/pair/approve", {"code": st3["code"], "shop_id": sid}, owner)
tok3 = call("POST", "/pos/pair/poll", {"pair_id": st3["pair_id"], "poll_secret": st3["poll_secret"]})["token"]
assert call("GET", "/pos/device/me", token=tok3)["device"]["code"] == "T03"          # revoked T01 is gone for good
# the staff member leaving the shop cuts their terminal off
call("DELETE", f"/me/memberships/{sid}", token=noy)
call("GET", "/pos/device/me", token=tok2, expect=403)
ok("terminals act as the approving cashier (no void right → void rejected), owners list/revoke them, leaving staff lose access")

print("all POS sync tests passed")
