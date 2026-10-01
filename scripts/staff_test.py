#!/usr/bin/env python3
"""Shop staff (roles, invite links, per-route permissions) and POS open bills.
Start the API with ADMIN_EMAILS=admin@demo.dev. Usage: python3 scripts/staff_test.py [http://localhost:8080]
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
print("staff + POS bills tests")

admin = account("admin@demo.dev", "Admin")
owner = account(f"own-{RUN}@t.dev", "Owner")
shop = call("POST", "/shops", {"slug": f"staff-{RUN}", "name": f"Khamphone Mart {RUN}", "currency": "LAK"}, owner)
sid = shop["id"]
call("PATCH", f"/admin/shops/{sid}", {"auto_approve": True}, admin)
prod = call("POST", f"/shops/{sid}/products", {"sku": f"st{RUN}", "name": "Sticky rice 1kg", "price_cents": 2500000, "status": "active", "initial_stock": 50, "category": "handmade"}, owner)

# ---- roles ------------------------------------------------------------------------------------
st = call("GET", f"/shops/{sid}/staff", token=owner)
names = {r["name"]: r for r in st["roles"]}
assert {"Manager", "Cashier", "Stock keeper", "Orders & delivery", "Marketing"} <= set(names), names
assert names["Cashier"]["permissions"] == ["pos"] and "settings" in names["Manager"]["permissions"]
assert "pos_void" in st["permissions"]
call("POST", f"/shops/{sid}/staff/roles", {"name": "Bad", "permissions": ["pos", "staff"]}, owner, expect=400)
call("POST", f"/shops/{sid}/staff/roles", {"name": "Cashier", "permissions": ["pos"]}, owner, expect=409)
st = call("POST", f"/shops/{sid}/staff/roles", {"name": "Senior cashier", "permissions": ["pos_void", "pos", "pos"]}, owner)
senior = next(r for r in st["roles"] if r["name"] == "Senior cashier")
assert senior["permissions"] == ["pos", "pos_void"], senior
lao_shop = call("POST", "/shops", {"slug": f"staff-lo-{RUN}", "name": "ຮ້ານລາວ", "currency": "LAK"}, owner)
lo_roles = [r["name"] for r in call("GET", f"/shops/{lao_shop['id']}/staff", token=owner, headers={"Accept-Language": "lo"})["roles"]]
assert "ພະນັກງານເກັບເງິນ" in lo_roles and "ຜູ້ຈັດການ" in lo_roles, lo_roles
ok("every shop gets starter roles (manager, cashier, stock, orders, marketing); owners add custom roles from a fixed permission list")

# ---- invite -----------------------------------------------------------------------------------
cashier_role = names["Cashier"]["id"]
inv = call("POST", f"/shops/{sid}/staff/invites", {"role_id": cashier_role, "note": "Noy — morning shift"}, owner)
token = inv["token"]
assert inv["link"].endswith(f"/join/{token}") and len(token) == 48
info = call("GET", f"/join/{token}")
assert info["state"] == "open" and info["role"] == "Cashier" and info["shop"]["id"] == sid and info["permissions"] == ["pos"]
call("GET", "/join/nonsense", expect=404)
noy = account(f"noy-{RUN}@t.dev", "Noy")
call("POST", f"/join/{token}", token=owner, expect=400)                  # the owner can't join as staff
j = call("POST", f"/join/{token}", token=noy)
assert j["joined"] and j["shop_id"] == sid
call("POST", f"/join/{token}", token=noy)                                   # same person again: fine
other = account(f"other-{RUN}@t.dev", "Other")
e = call("POST", f"/join/{token}", token=other, expect=400, headers={"Accept-Language": "lo"})
assert e["message"].startswith("ລິ້ງເຊີນນີ້ໃຊ້ບໍ່ໄດ້ແລ້ວ"), e
assert call("GET", f"/join/{token}")["state"] == "used"
mine = call("GET", "/me/shops", token=noy)
ms = next(s for s in mine if s["id"] == sid)
assert ms["access"] == {"owner": False, "role_id": cashier_role, "role": "Cashier", "permissions": ["pos"]}, ms["access"]
assert next(s for s in call("GET", "/me/shops", token=owner) if s["id"] == sid)["access"]["owner"] is True
ok("owner shares a one-time invite link; the staff member joins while signed in; /me/shops shows their role and permissions")

# ---- permissions per route ----------------------------------------------------------------------
call("GET", f"/shops/{sid}/pos/products", token=noy)
sale = call("POST", f"/shops/{sid}/pos/sales", {"items": [{"product_id": prod["id"], "qty": 2}], "payments": [{"method": "cash", "amount_cents": 5000000}]}, noy)
assert sale["cashier"] == "Noy", sale.get("cashier")
sale_id = sale["sale"]["id"]
call("GET", f"/pos/sales/{sale_id}", token=noy)
for m, path, body in [("GET", f"/shops/{sid}/products", None), ("PATCH", f"/shops/{sid}", {"name": "x"}), ("GET", f"/shops/{sid}/stats", None),
                      ("POST", f"/pos/sales/{sale_id}/void", {"reason": "x"}), ("GET", f"/shops/{sid}/staff", None), ("GET", f"/shops/{sid}/kyb", None),
                      ("POST", f"/products/{prod['id']}/stock", {"delta": 5, "reason": "restock"}), ("GET", f"/shops/{sid}/cod", None),
                      ("POST", f"/shops/{sid}/staff/invites", {"role_id": cashier_role}), ("GET", f"/shops/{sid}/image-suggest?q=rice", None)]:
    call(m, path, body, noy, expect=403)
call("GET", f"/shops/{sid}/pos/products", token=other, expect=403)
ok("a cashier can sell at the POS (recorded as the cashier) but can't see products, stats, settings, voids, stock, COD or staff")

# promote to a manager
mgr = names["Manager"]["id"]
noy_id = call("GET", "/auth/me", token=noy)["id"]
call("PATCH", f"/shops/{sid}/staff/{noy_id}", {"role_id": mgr}, owner)
call("GET", f"/shops/{sid}/products", token=noy)
call("PATCH", f"/shops/{sid}", {"receipt_footer": "ຂອບໃຈ"}, noy)
call("PATCH", f"/shops/{sid}", {"entity_type": "business"}, noy, expect=400)
call("POST", f"/products/{prod['id']}/stock", {"delta": 5, "reason": "restock"}, noy)
call("POST", f"/pos/sales/{sale_id}/void", {"reason": "customer returned"}, noy)
call("GET", f"/shops/{sid}/staff", token=noy, expect=403)
call("GET", f"/shops/{sid}/kyb", token=noy, expect=403)
call("DELETE", f"/shop-roles/{mgr}", token=owner, expect=409)
ok("owner changes the role → a manager edits products, stock, storefront settings and voids sales; staff and verification stay owner-only")

# suspend / remove / leave
call("PATCH", f"/shops/{sid}/staff/{noy_id}", {"active": False}, owner)
call("GET", f"/shops/{sid}/products", token=noy, expect=403)
assert all(s["id"] != sid for s in call("GET", "/me/shops", token=noy))
call("PATCH", f"/shops/{sid}/staff/{noy_id}", {"active": True}, owner)
call("GET", f"/shops/{sid}/products", token=noy)
inv2 = call("POST", f"/shops/{sid}/staff/invites", {"role_id": senior["id"], "days": 2}, owner)
call("DELETE", f"/staff-invites/{inv2['id']}", token=owner)
assert call("GET", f"/join/{inv2['token']}")["state"] == "revoked"
call("POST", f"/join/{inv2['token']}", token=other, expect=400)
call("DELETE", f"/me/memberships/{sid}", token=noy)
call("GET", f"/shops/{sid}/products", token=noy, expect=403)
doc = call("GET", f"/shops/{sid}/staff", token=owner)
assert doc["members"] == [] and any(i["accepted_name"] == "Noy" for i in doc["invites"])
ok("suspend and restore a staff member, revoke unused links, staff can leave a shop")

# ---- POS open bills ---------------------------------------------------------------------------
inv3 = call("POST", f"/shops/{sid}/staff/invites", {"role_id": cashier_role}, owner)
call("POST", f"/join/{inv3['token']}", token=other)                        # "Other" becomes a cashier
b1 = call("POST", f"/shops/{sid}/pos/bills", {"data": {"lines": []}}, other)
b2 = call("POST", f"/shops/{sid}/pos/bills", {"label": "Table 5"}, owner)
assert (b1["number"], b2["number"], b2["label"], b1["version"]) == (1, 2, "Table 5", 1), (b1, b2)
assert b1["created_by_name"] == "Other"
assert [b["number"] for b in call("GET", f"/shops/{sid}/pos/bills", token=owner)] == [1, 2]
cart = {"lines": [{"product_id": prod["id"], "name": "Sticky rice 1kg", "qty": 3, "unit": 2500000}], "billDiscount": 0}
s1 = call("PUT", f"/pos/bills/{b1['id']}", {"data": cart, "version": 1}, other)
assert s1["version"] == 2 and s1["data"]["lines"][0]["qty"] == 3
e = call("PUT", f"/pos/bills/{b1['id']}", {"data": {"lines": []}, "version": 1}, owner, expect=409, headers={"Accept-Language": "lo"})
assert e["message"].startswith("ບິນນີ້ຖືກແກ້ໄຂຢູ່ເຄື່ອງຂາຍອື່ນ"), e
call("PUT", f"/pos/bills/{b1['id']}", {"label": "x" * 41, "version": 2}, other, expect=400)
call("PUT", f"/pos/bills/{b1['id']}", {"data": {"blob": "x" * 210_000}, "version": 2}, other, expect=400)
call("PUT", f"/pos/bills/{b1['id']}", {"label": "Mr. Somsak", "version": 2}, owner)
assert call("GET", f"/pos/bills/{b1['id']}", token=other)["label"] == "Mr. Somsak"
paid = call("POST", f"/shops/{sid}/pos/sales", {"bill_id": b1["id"], "items": [{"product_id": prod["id"], "qty": 3}],
                                               "payments": [{"method": "cash", "amount_cents": 7500000}]}, other)
assert paid["sale"]["total_cents"] == 7500000
call("GET", f"/pos/bills/{b1['id']}", token=other, expect=404)
assert [b["label"] for b in call("GET", f"/shops/{sid}/pos/bills", token=other)] == ["Table 5"]
b3 = call("POST", f"/shops/{sid}/pos/bills", {}, other)
assert b3["number"] == 3
call("DELETE", f"/pos/bills/{b2['id']}", token=other)
call("DELETE", f"/pos/bills/{b3['id']}", token=other)
assert call("POST", f"/shops/{sid}/pos/bills", {}, other)["number"] == 1, "numbering restarts when no bill is open"
outsider = account(f"out-{RUN}@t.dev", "Outsider")
call("GET", f"/shops/{sid}/pos/bills", token=outsider, expect=403)
call("PUT", f"/pos/bills/{b2['id']}", {"version": 1}, outsider, expect=404)
ok("several open bills per shop, shared by all tills: numbered, renamed, stale saves rejected (409), paying a bill closes it")

print("ALL STAFF + POS BILL TESTS PASSED ✔")
