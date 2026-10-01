#!/usr/bin/env python3
"""Super app: core registry, restaurant, insurance and finance (tax agent) cores.
Start the API with ADMIN_EMAILS=admin@demo.dev (all cores).
Backdating sales into last month (for the monthly filing) uses psql with DATABASE_URL
(default postgres://zaokaiy:zaokaiy@localhost:5432/zaokaiy); skipped when psql is missing.
Usage: python3 scripts/superapp_test.py [http://localhost:8080]
"""
import datetime, json, os, shutil, subprocess, sys, urllib.error, urllib.request, uuid

BASE = sys.argv[1] if len(sys.argv) > 1 else "http://localhost:8080"
API = BASE + "/api"
RUN = uuid.uuid4().hex[:6]
DB = os.environ.get("DATABASE_URL", "postgres://zaokaiy:zaokaiy@localhost:5432/zaokaiy")
LO = {"Accept-Language": "lo"}


def call(method, path, body=None, token=None, expect=200, headers=None, raw=False):
    r = urllib.request.Request(API + path, data=json.dumps(body).encode() if body is not None else None, method=method)
    r.add_header("Content-Type", "application/json")
    for k, v in (headers or {}).items():
        r.add_header(k, v)
    if token:
        r.add_header("Authorization", f"Bearer {token}")
    try:
        with urllib.request.urlopen(r) as resp:
            code, payload, ctype = resp.status, resp.read(), resp.headers.get("content-type", "")
    except urllib.error.HTTPError as e:
        code, payload, ctype = e.code, e.read(), e.headers.get("content-type", "")
    if raw:
        assert code == expect, f"{method} {path} -> {code}"
        return payload.decode(), ctype
    out = json.loads(payload or b"null")
    assert code == expect, f"{method} {path} -> {code} (expected {expect}): {out}"
    return out


def account(email, name):
    try:
        return call("POST", "/auth/login", {"email": email, "password": "password123"})["token"]
    except AssertionError:
        return call("POST", "/auth/register", {"email": email, "password": "password123", "display_name": name})["token"]


def sql(q):
    return subprocess.run(["psql", DB, "-qtAc", q], check=True, capture_output=True, text=True).stdout.strip()


ok = lambda m: print("  ✔", m)
today = (datetime.datetime.utcnow() + datetime.timedelta(hours=7)).date()
first = today.replace(day=1)
last_month = (first - datetime.timedelta(days=1)).replace(day=1)
period = last_month.strftime("%Y-%m")

print("super app tests")
admin = account("admin@demo.dev", "Admin")
cook = account(f"cook-{RUN}@t.dev", "Cook")
agent = account(f"agent-{RUN}@t.dev", "Agent")
guest = account(f"guest-{RUN}@t.dev", "Guest")

# --- registry --------------------------------------------------------------------------------
cores = call("GET", "/cores")["cores"]
names = [c["name"] for c in cores]
assert names == ["platform", "commerce", "vehicle", "restaurant", "insurance", "finance"], names
assert all(c["served_by"] == "local" for c in cores)
verticals = sum((c["verticals"] for c in cores), [])
assert verticals == ["general", "vehicle", "restaurant", "insurance"], verticals
call("POST", "/shops", {"slug": f"boat-{RUN}", "name": "x", "vertical": "boats"}, cook, expect=400)
ok("GET /cores lists the six cores with their shop types; unknown shop types are refused")

# --- restaurant ------------------------------------------------------------------------------
shop = call("POST", "/shops", {"slug": f"pho-{RUN}", "name": f"Pho {RUN}", "vertical": "restaurant", "currency": "LAK"}, cook)
sid, slug = shop["id"], shop["slug"]
call("GET", f"/restaurant/menu/pho-none-{RUN}", expect=404)
ov = call("GET", f"/shops/{sid}/restaurant", token=cook)
assert ov["settings"]["dine_in"] and not ov["settings"]["delivery"]
call("GET", f"/shops/{sid}/restaurant", token=guest, expect=403)
call("PUT", f"/shops/{sid}/restaurant", {"accepting_orders": True, "dine_in": True, "takeaway": True, "delivery": True,
                                         "service_charge_bps": 1000, "prep_minutes": 20}, cook)
e = call("PUT", f"/shops/{sid}/restaurant", {"accepting_orders": True, "dine_in": False, "takeaway": False, "delivery": False,
                                             "service_charge_bps": 0, "prep_minutes": 20}, cook, expect=400, headers=LO)
assert e["message"] == "ກະລຸນາເປີດວິທີສັ່ງຢ່າງໜ້ອຍ 1 ແບບ", e
noodles = call("POST", f"/shops/{sid}/restaurant/sections", {"name": "Noodles", "name_lo": "ເຝີ"}, cook)
drinks = call("POST", f"/shops/{sid}/restaurant/sections", {"name": "Drinks"}, cook)
pho = call("POST", f"/shops/{sid}/restaurant/items", {"name": "Beef pho", "name_lo": "ເຝີຊີ້ນ", "price_cents": 35_000_00,
                                                      "section_id": noodles["id"], "spicy": 1}, cook)
tea = call("POST", f"/shops/{sid}/restaurant/items", {"name": "Iced tea", "price_cents": 10_000_00, "section_id": drinks["id"]}, cook)
gone = call("POST", f"/shops/{sid}/restaurant/items", {"name": "Khao piak", "price_cents": 30_000_00, "available": False}, cook)
call("POST", f"/shops/{sid}/restaurant/items", {"name": "x", "price_cents": -1}, cook, expect=400)
call("PATCH", f"/restaurant/items/{pho['id']}", {"price_cents": 1}, guest, expect=403)
t1 = call("POST", f"/shops/{sid}/restaurant/tables", {"label": "T1", "seats": 4}, cook)
call("POST", f"/shops/{sid}/restaurant/tables", {"label": "T1"}, cook, expect=409)
assert call("GET", f"/restaurant/tables/{t1['token']}")["shop_slug"] == slug
menu = call("GET", f"/restaurant/menu/{slug}")
assert [s["name"] for s in menu["sections"]] == ["Noodles", "Drinks"] and len(menu["items"]) == 3
ok("restaurant: settings, sections, dishes, tables with QR tokens, public menu; owner-only edits")

# dine-in needs a valid table code; takeaway needs a phone
e = call("POST", f"/restaurant/menu/{slug}/orders", {"mode": "dine_in", "items": [{"item_id": pho["id"], "qty": 1}]}, expect=400, headers=LO)
assert e["message"] == "ກະລຸນາສະແກນ QR ໂຄດທີ່ໂຕະຂອງທ່ານເພື່ອສັ່ງອາຫານ", e
call("POST", f"/restaurant/menu/{slug}/orders", {"mode": "takeaway", "items": [{"item_id": pho["id"], "qty": 1}]}, expect=400)
e = call("POST", f"/restaurant/menu/{slug}/orders", {"mode": "dine_in", "table_token": t1["token"],
                                                     "items": [{"item_id": gone["id"], "qty": 1}]}, expect=400, headers=LO)
assert e["message"] == "'Khao piak' ໝົດແລ້ວ", e
o1 = call("POST", f"/restaurant/menu/{slug}/orders", {"mode": "dine_in", "table_token": t1["token"], "note": "no MSG",
                                                      "items": [{"item_id": pho["id"], "qty": 2, "note": "extra herbs"}, {"item_id": tea["id"], "qty": 1}]})
assert o1["number"] == 1 and o1["table_label"] == "T1"
assert o1["subtotal_cents"] == 80_000_00 and o1["service_cents"] == 8_000_00 and o1["total_cents"] == 88_000_00, o1
o2 = call("POST", f"/restaurant/menu/{slug}/orders", {"mode": "takeaway", "customer_name": "Noy", "customer_phone": "020 5555 0001",
                                                      "items": [{"item_id": tea["id"], "qty": 3}]}, token=guest)
assert o2["number"] == 2 and o2["user_id"]
tr = call("GET", f"/restaurant/track/{o1['track_token']}")
assert tr["status"] == "new" and tr["shop"]["prep_minutes"] == 20 and len(tr["items"]) == 2
board = call("GET", f"/shops/{sid}/restaurant/orders", token=cook)
assert [o["number"] for o in board] == [1, 2]
call("PATCH", f"/restaurant/orders/{o1['id']}", {"status": "preparing"}, cook)
e = call("PATCH", f"/restaurant/orders/{o1['id']}", {"status": "new"}, cook, expect=409, headers=LO)
assert e["message"] == "ບໍ່ສາມາດປ່ຽນອໍເດີຈາກ preparing ເປັນ new", e
call("PATCH", f"/restaurant/orders/{o1['id']}", {"status": "served", "paid": True, "payment_method": "cash"}, cook)
call("PATCH", f"/restaurant/orders/{o2['id']}", {"status": "cancelled"}, cook)
call("PATCH", f"/restaurant/orders/{o2['id']}", {"paid": True}, cook, expect=400)
done = call("GET", f"/shops/{sid}/restaurant/orders?view=done", token=cook)
assert [o["number"] for o in done] == [2]
assert call("GET", f"/shops/{sid}/restaurant", token=cook)["today"]["revenue_cents"] == 88_000_00
call("PUT", f"/shops/{sid}/restaurant", {"accepting_orders": False, "dine_in": True, "takeaway": True, "delivery": True,
                                         "service_charge_bps": 1000, "prep_minutes": 20}, cook)
call("POST", f"/restaurant/menu/{slug}/orders", {"mode": "dine_in", "table_token": t1["token"], "items": [{"item_id": pho["id"], "qty": 1}]}, expect=400)
ok("restaurant: QR dine-in + takeaway orders, service charge, daily ticket numbers, tracking link, kitchen flow, paid/cancel rules")

# --- insurance -------------------------------------------------------------------------------
ashop = call("POST", "/shops", {"slug": f"cover-{RUN}", "name": f"Cover {RUN}", "vertical": "insurance", "currency": "LAK"}, agent)
aid = ashop["id"]
call("POST", f"/shops/{sid}/insurance/plans", {"kind": "motor", "insurer": "AGL", "name": "x", "premium_cents": 1}, cook, expect=400)
e = call("POST", f"/shops/{aid}/insurance/plans", {"kind": "travel", "insurer": "AGL", "name": "Trip"}, agent, expect=400, headers=LO)
assert e["message"] == "ກະລຸນາກຳນົດຄ່າທຳນຽມປະກັນ", e
motor = call("POST", f"/shops/{aid}/insurance/plans", {
    "kind": "motor", "insurer": "AGL", "name": "Car class 1", "name_lo": "ລົດ ຊັ້ນ 1", "coverage": ["Own damage", "Theft", "Third party"],
    "premium_mode": "rate", "rate_bps": 250, "min_premium_cents": 1_500_000_00,
    "sum_insured_min_cents": 50_000_000_00, "sum_insured_max_cents": 3_000_000_000_00, "term_months": 12}, agent)
travel = call("POST", f"/shops/{aid}/insurance/plans", {"kind": "travel", "insurer": "Allianz", "name": "Asia trip",
                                                        "premium_cents": 250_000_00, "term_months": 1}, agent)
plans = call("GET", f"/insurance/plans?shop={ashop['slug']}")
assert {p["name"] for p in plans} == {"Car class 1", "Asia trip"} and plans[0]["currency"] == "LAK"
q = call("POST", f"/insurance/plans/{motor['id']}/quote", {"sum_insured_cents": 400_000_000_00})
assert q["premium_cents"] == 10_000_000_00, q
assert call("POST", f"/insurance/plans/{motor['id']}/quote", {"sum_insured_cents": 50_000_000_00})["premium_cents"] == 1_500_000_00
e = call("POST", f"/insurance/plans/{motor['id']}/quote", {"sum_insured_cents": 1_000_00}, expect=400, headers=LO)
assert e["message"] == "ທຶນປະກັນຕ້ອງຢູ່ລະຫວ່າງ 50,000,000 ຫາ 3,000,000,000", e
start = (today + datetime.timedelta(days=3)).isoformat()
app = call("POST", f"/insurance/plans/{motor['id']}/apply", {
    "applicant_name": "Somchai P.", "phone": "+856 20 5555 7777", "email": "s@t.dev", "sum_insured_cents": 400_000_000_00,
    "start_date": start, "details": {"plate": "ກທ 1234", "make": "Toyota", "model": "Hilux", "year": 2020}}, token=guest)
assert app["status"] == "submitted" and app["premium_cents"] == 10_000_000_00 and app["number"].startswith("INS-")
assert app["end_date"] == (datetime.date.fromisoformat(start).replace(year=datetime.date.fromisoformat(start).year + 1) - datetime.timedelta(days=1)).isoformat()
call("POST", f"/insurance/plans/{travel['id']}/apply", {"applicant_name": "A", "phone": "020 5555 1111",
                                                         "start_date": (today - datetime.timedelta(days=1)).isoformat()}, expect=400)
mine = call("GET", "/me/insurance", token=guest)
assert mine[0]["plan_name"] == "Car class 1" and mine[0]["shop_name"] == ashop["name"]
apps = call("GET", f"/shops/{aid}/insurance/applications", token=agent)
assert len(apps) == 1 and apps[0]["details"]["plate"] == "ກທ 1234"
call("GET", f"/shops/{aid}/insurance/applications", token=guest, expect=403)
call("POST", f"/insurance/applications/{app['id']}/decision", {"action": "issue", "policy_no": "P1"}, agent, expect=409)
call("POST", f"/insurance/applications/{app['id']}/decision", {"action": "approve"}, agent)
e = call("POST", f"/insurance/applications/{app['id']}/decision", {"action": "issue", "policy_no": "P1"}, agent, expect=400, headers=LO)
assert e["message"] == "ກະລຸນາໝາຍວ່າຈ່າຍຄ່າທຳນຽມແລ້ວ ກ່ອນອອກກົມມະທຳ", e
call("POST", f"/insurance/applications/{app['id']}/decision", {"action": "paid"}, agent)
issued = call("POST", f"/insurance/applications/{app['id']}/decision", {"action": "issue", "policy_no": "AGL-M-000123"}, agent)
assert issued["status"] == "issued" and issued["policy_no"] == "AGL-M-000123" and issued["paid"]
call("POST", f"/insurance/applications/{app['id']}/decision", {"action": "cancel"}, agent, expect=400)
call("PATCH", f"/insurance/plans/{travel['id']}", {"active": False}, agent)
call("GET", f"/insurance/plans/{travel['id']}", expect=404)
call("PATCH", f"/insurance/plans/{motor['id']}", {"name": "hijack"}, guest, expect=403)
ok("insurance: plans (fixed / rate of sum insured), quotes with limits, online applications, review → paid → policy issued")

# --- finance: tax-agent mandate --------------------------------------------------------------
pol = call("GET", "/finance/policy")
assert pol["vat_bps"] == 1000 and pol["policy_en"].startswith("1.") and pol["policy_lo"]
call("PUT", "/admin/finance/settings", {**{k: pol[k] for k in ["vat_bps", "due_day", "tax_office", "agent_name", "agent_tax_id", "policy_en", "policy_lo"]},
                                        "ecommerce_tax_bps": 100}, admin)
call("GET", "/admin/finance/settings", token=cook, expect=403)
call("PUT", "/admin/finance/settings", {**pol, "vat_bps": 9000}, admin, expect=400)
fin = call("GET", f"/shops/{sid}/finance", token=cook)
assert fin["mandate"] is None and fin["settings"]["ecommerce_tax_bps"] == 100
line = fin["preview"]["lines"][0]
assert line["currency"] == "LAK" and line["sources"][0]["key"] == "restaurant.orders" and line["taxes"]["gross_cents"] == 88_000_00, line
version = fin["settings"]["policy_version"]
body = {"accept": True, "policy_version": version, "signer_name": "Khamla S.", "signer_title": "Owner", "vat_registered": True}
e = call("POST", f"/shops/{sid}/finance/mandate", body, cook, expect=400, headers=LO)
assert e["message"] == "ກະລຸນາເພີ່ມເລກປະຈຳຕົວຜູ້ເສຍອາກອນໃນການຕັ້ງຄ່າຮ້ານກ່ອນ", e
call("PATCH", f"/shops/{sid}", {"tax_id": f"TIN{RUN}", "legal_name": f"Pho {RUN} Co."}, cook)
call("POST", f"/shops/{sid}/finance/mandate", {**body, "accept": False}, cook, expect=400)
call("POST", f"/shops/{sid}/finance/mandate", {**body, "policy_version": "old"}, cook, expect=409)
call("POST", f"/shops/{sid}/finance/mandate", body, guest, expect=403)
fin = call("POST", f"/shops/{sid}/finance/mandate", body, cook)
assert fin["mandate"]["status"] == "active" and fin["mandate"]["tax_id"] == f"TIN{RUN}" and not fin["policy_outdated"]
tx = fin["preview"]["lines"][0]["taxes"]
# 88,000 incl. 10% VAT → 8,000 VAT; 1% of 80,000 net → 800
assert (tx["vat_cents"], tx["net_cents"], tx["ecommerce_tax_cents"], tx["total_due_cents"]) == (8_000_00, 80_000_00, 800_00, 8_800_00), tx
ok("finance: tax-agent policy (EN/LO), mandate needs tax ID + current policy version, live VAT / e-commerce tax preview")

# --- finance: monthly filings ----------------------------------------------------------------
e = call("POST", "/admin/finance/filings/generate", {"period": first.strftime("%Y-%m")}, admin, expect=400, headers=LO)
assert e["message"] == "ຍື່ນໄດ້ສະເພາະເດືອນທີ່ສິ້ນສຸດແລ້ວ", e
call("POST", "/admin/finance/filings/generate", {"period": "Sept"}, admin, expect=400)
call("POST", "/admin/finance/filings/generate", {"period": period}, cook, expect=403)
if shutil.which("psql"):
    ts = f"'{last_month.isoformat()} 12:00+07'"
    sql(f"UPDATE restaurant.orders SET day = '{last_month.isoformat()}', created_at = {ts} WHERE shop_id = '{sid}'")
    gen = call("POST", "/admin/finance/filings/generate", {"period": period}, admin)
    mine = [f for f in gen["filings"] if f["shop_id"] == sid]
    assert len(mine) == 1, mine
    f = mine[0]
    assert f["status"] == "draft" and f["gross_cents"] == 88_000_00 and f["vat_cents"] == 8_000_00 and f["ecommerce_tax_cents"] == 800_00, f
    assert f["sources"][0]["key"] == "restaurant.orders" and f["sources"][0]["count"] == 1 and f["tax_id"] == f"TIN{RUN}"
    assert f["due_on"] == f"{first.strftime('%Y-%m')}-{pol['due_day']:02d}"
    e = call("PUT", f"/admin/finance/filings/{f['id']}", {"action": "submit"}, admin, expect=400, headers=LO)
    assert e["message"] == "ກະລຸນາໃສ່ເລກອ້າງອີງຈາກຫ້ອງການສ່ວຍສາອາກອນ", e
    call("PUT", f"/admin/finance/filings/{f['id']}", {"action": "paid"}, admin, expect=409)
    sub = call("PUT", f"/admin/finance/filings/{f['id']}", {"action": "submit", "reference_no": f"TAX-{RUN}"}, admin)
    assert sub["status"] == "submitted" and sub["submitted_at"]
    # regenerate leaves submitted filings alone
    sql(f"UPDATE restaurant.orders SET total_cents = total_cents + 100 WHERE shop_id = '{sid}'")
    gen = call("POST", "/admin/finance/filings/generate", {"period": period}, admin)
    assert [x for x in gen["filings"] if x["shop_id"] == sid][0]["gross_cents"] == 88_000_00
    assert gen["generated"]["locked"] >= 1
    paid = call("PUT", f"/admin/finance/filings/{f['id']}", {"action": "paid"}, admin)
    assert paid["status"] == "paid" and paid["paid_at"]
    csv, ctype = call("GET", f"/admin/finance/filings/export?period={period}", token=admin, raw=True)
    assert ctype.startswith("text/csv") and f"TAX-{RUN}" in csv and "880.00" not in csv and "88000.00" in csv, csv[:400]
    stm = call("GET", f"/shops/{sid}/finance", token=cook)["filings"]
    assert stm[0]["reference_no"] == f"TAX-{RUN}" and stm[0]["status"] == "paid"
    ok("finance: monthly filings from every core's sales, locked once submitted, submit → paid, CSV export, seller statements")
else:
    print("  – psql not found: skipped monthly filing checks")

fin = call("POST", f"/shops/{sid}/finance/mandate/revoke", {"reason": "closing"}, cook)
assert fin["mandate"]["status"] == "revoked"
call("POST", f"/shops/{sid}/finance/mandate/revoke", {"reason": "again"}, cook, expect=400)
ok("finance: shop can revoke the mandate")

print("ALL SUPER APP TESTS PASSED ✔")
