#!/usr/bin/env python3
"""Vehicle sellers: vehicle storefront, spec sheets, search + facets, buyer requests (enquiry,
test drive, offer, reservation deposit), sale status and online checkout rules.
Start the API with ADMIN_EMAILS=admin@demo.dev. Usage: python3 scripts/vehicle_test.py [http://localhost:8080]
"""
import datetime, json, sys, urllib.error, urllib.request, uuid

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
print("vehicle seller tests")
admin = account("admin@demo.dev", "Admin")
dealer = account(f"dealer-{RUN}@t.dev", "Dealer")
other = account(f"other-{RUN}@t.dev", "Other")
buyer = account(f"vbuyer-{RUN}@t.dev", "Buyer")
LO = {"Accept-Language": "lo"}

# --- shop type
call("POST", "/shops", {"slug": f"bad-{RUN}", "name": "x", "vertical": "boats"}, dealer, expect=400)
shop = call("POST", "/shops", {"slug": f"cars-{RUN}", "name": f"Cars {RUN}", "vertical": "vehicle", "currency": "LAK"}, dealer)
sid = shop["id"]
assert shop["vertical"] == "vehicle"
call("PATCH", f"/shops/{sid}", {"phone": "020 5999 1111", "address": "Dongdok, Vientiane"}, dealer)
e = call("PATCH", f"/shops/{sid}", {"vertical": "x"}, dealer, expect=400, headers=LO)
assert e["message"] == "ປະເພດຮ້ານຕ້ອງເປັນ ຮ້ານທົ່ວໄປ ຫຼື ຮ້ານຂາຍລົດ", e
call("PATCH", f"/admin/shops/{sid}", {"auto_approve": True}, admin)
cats = {c["slug"]: c["id"] for c in call("GET", "/categories")}
assert "vehicles-cars" in cats and "vehicles-motorbikes" in cats
ok("vehicle store type + marketplace vehicle categories")

# --- spec validation + create with spec
base = {"status": "active", "initial_stock": 1, "category_id": cats["vehicles-cars"]}
car = lambda sku, name, price, **v: call("POST", f"/shops/{sid}/products", {**base, "sku": sku, "name": name, "price_cents": price, "vehicle": v}, dealer)
for bad, msg in [({"make": "", "model": "X", "year": 2020}, "ກະລຸນາໃສ່ຍີ່ຫໍ້ ແລະ ລຸ້ນລົດ"),
                 ({"make": "Toyota", "model": "Vios", "year": 1800}, "ກະລຸນາໃສ່ປີລົດໃຫ້ຖືກຕ້ອງ"),
                 ({"make": "Toyota", "model": "Vios", "year": 2020, "fuel": "steam"}, "ບໍ່ຮູ້ຈັກປະເພດນ້ຳມັນເຊື້ອໄຟນີ້"),
                 ({"make": "Toyota", "model": "Vios", "year": 2020, "vin": "AB-1"}, "ເລກຖັງ (VIN) ຕ້ອງເປັນຕົວອັກສອນ ຫຼື ຕົວເລກ 6-20 ຕົວ"),
                 ({"make": "Toyota", "model": "Vios", "year": 2020, "seats": 0}, "ມີຕົວເລກໃນຂໍ້ມູນລົດທີ່ເກີນຂອບເຂດ")]:
    e = call("POST", f"/shops/{sid}/products", {**base, "sku": f"BAD-{RUN}", "name": "Bad", "price_cents": 1, "vehicle": bad}, dealer, expect=400, headers=LO)
    assert e["message"] == msg, e
assert not call("GET", f"/shops/{sid}/products?q=BAD-", token=dealer), "invalid spec must not create the product"
hilux = car("HILUX", "Toyota Hilux Revo", 54_500_000_000, make=" toyota ", model="Hilux Revo", year=2020, mileage_km=68000, fuel="diesel",
            body_type="Pickup", vin="mr0ha3cd100123456", plate_no="ກຂ 1234", deposit_cents=500_000_000, finance_available=True,
            features=["ABS", "abs", "Camera"], location="Vientiane Capital")
city = car("CITY", "Honda City", 29_800_000_000, make="Honda", model="City", year=2019, mileage_km=85000, transmission="cvt", body_type="sedan", buy_online=True)
vios = car("VIOS", "Toyota Vios", 25_000_000_000, make="Toyota", model="Vios", year=2016, mileage_km=150000, transmission="manual")
click = call("POST", f"/shops/{sid}/products", {**base, "category_id": cats["vehicles-motorbikes"], "sku": "CLICK", "name": "Honda Click",
             "price_cents": 3_250_000_000, "vehicle": {"vehicle_type": "motorbike", "make": "Honda", "model": "Click", "year": 2023, "condition": "new",
             "body_type": "scooter", "engine_cc": 157, "buy_online": True}}, dealer)
spec = call("GET", f"/products/{hilux['id']}/vehicle", token=dealer)
assert spec["make"] == "Toyota" and spec["body_type"] == "pickup" and spec["vin"] == "MR0HA3CD100123456" and spec["features"] == ["ABS", "Camera"], spec
assert spec["sale_status"] == "available" and spec["plate_no"] == "ກຂ 1234"
call("GET", f"/products/{hilux['id']}/vehicle", token=other, expect=403)
ok("spec sheet validated (Lao errors), normalised and stored with the product")

# --- update via PATCH + PUT
call("PATCH", f"/products/{vios['id']}", {"price_cents": 24_000_000_000, "vehicle": {"make": "Toyota", "model": "Vios", "variant": "1.5 J", "year": 2016, "mileage_km": 151000, "transmission": "manual"}}, dealer)
v = call("GET", f"/products/{vios['id']}/vehicle", token=dealer)
assert v["variant"] == "1.5 J" and v["mileage_km"] == 151000
v = call("PUT", f"/products/{vios['id']}/vehicle", {"make": "Toyota", "model": "Vios", "year": 2016, "mileage_km": 152000, "transmission": "manual", "sale_status": "reserved"}, dealer)
assert v["sale_status"] == "reserved" and v["reserved_until"] and v["mileage_km"] == 152000
v = call("POST", f"/products/{vios['id']}/vehicle/status", {"sale_status": "available"}, dealer)
assert v["sale_status"] == "available" and v["reserved_until"] is None
call("POST", f"/products/{vios['id']}/vehicle/status", {"sale_status": "gone"}, dealer, expect=400)
plain = call("POST", f"/shops/{sid}/products", {"sku": "MATS", "name": "Floor mats", "price_cents": 30_000_000, "status": "active", "initial_stock": 5, "category_id": cats["vehicles-parts"]}, dealer)
call("POST", f"/products/{plain['id']}/vehicle/status", {"sale_status": "sold"}, dealer, expect=400)
ok("spec updates via product PATCH, PUT /vehicle and sale status endpoint")

# --- public detail hides private fields
d = call("GET", f"/catalog/products/{hilux['id']}")
assert d["vehicle"]["vin_last4"] == "3456" and "vin" not in d["vehicle"] and "plate_no" not in d["vehicle"], d["vehicle"]
assert d["contact"]["phone"] == "020 5999 1111" and d["product"]["vehicle"]["year"] == 2020
assert call("GET", f"/catalog/products/{plain['id']}")["vehicle"] is None
ok("public listing: spec sheet without plate/VIN (last 4 only) + seller contact")

# --- search + facets
s = call("GET", f"/vehicles?shop=cars-{RUN}")
assert s["total"] == 4 and s["shop"]["vertical"] == "vehicle"
f = s["facets"]
assert dict(f["types"]) == {"car": 3, "motorbike": 1}, f["types"]
toyota = next(m for m in f["makes"] if m["make"] == "Toyota")
assert toyota["count"] == 2 and {m["model"] for m in toyota["models"]} == {"Hilux Revo", "Vios"}
assert f["year"] == {"min": 2016, "max": 2023}
names = lambda q: [i["name"] for i in call("GET", f"/vehicles?shop=cars-{RUN}&{q}")["items"]]
assert names("make=TOYOTA&sort=price_asc") == ["Toyota Vios", "Toyota Hilux Revo"]
assert names("type=motorbike") == ["Honda Click"]
assert names("year_min=2019&km_max=90000&type=car&sort=year_desc") == ["Toyota Hilux Revo", "Honda City"]
assert names("price_max=26000000000&type=car") == ["Toyota Vios"]
assert names("transmission=cvt") == ["Honda City"]
assert names("q=hilux") == ["Toyota Hilux Revo"]
assert call("GET", f"/vehicles?shop=cars-{RUN}&limit=2")["total"] == 4
car_facets = call("GET", f"/vehicles?shop=cars-{RUN}&type=motorbike")["facets"]
assert [m["make"] for m in car_facets["makes"]] == ["Honda"]
call("GET", "/vehicles?shop=no-such-shop-xyz", expect=404)
assert any(i["id"] == hilux["id"] for i in call("GET", "/vehicles?q=Hilux%20Revo&limit=100")["items"])
ok("search: make/model/year/price/mileage/transmission/text filters, sorting, facets per type")

# --- leads
lead = lambda pid, body, **kw: call("POST", f"/catalog/products/{pid}/leads", body, **kw)
e = lead(hilux["id"], {"kind": "enquiry", "name": "A", "phone": "123"}, expect=400, headers=LO)
assert e["message"] == "ກະລຸນາໃສ່ເບີໂທໃຫ້ຖືກຕ້ອງ"
lead(hilux["id"], {"kind": "spam", "name": "A", "phone": "02055551234"}, expect=400)
lead(hilux["id"], {"kind": "enquiry", "name": " ", "phone": "02055551234"}, expect=400)
lead(plain["id"], {"kind": "enquiry", "name": "A", "phone": "02055551234"}, expect=404)
lead(hilux["id"], {"kind": "test_drive", "name": "A", "phone": "02055551234"}, expect=400)
past = (datetime.datetime.now(datetime.timezone.utc) - datetime.timedelta(days=1)).isoformat()
lead(hilux["id"], {"kind": "test_drive", "name": "A", "phone": "02055551234", "preferred_at": past}, expect=400)
lead(hilux["id"], {"kind": "offer", "name": "A", "phone": "02055551234"}, expect=400)
lead(hilux["id"], {"kind": "offer", "name": "A", "phone": "02055551234", "offer_cents": 54_500_000_000 * 3}, expect=400)
soon = (datetime.datetime.now(datetime.timezone.utc) + datetime.timedelta(days=2)).isoformat()
td = lead(hilux["id"], {"kind": "test_drive", "name": "Somphone", "phone": "020 5555 1234", "preferred_at": soon, "message": "Morning"}, token=buyer)
assert td["status"] == "new" and not td["duplicate"] and td["contact"]["name"] == f"Cars {RUN}"
again = lead(hilux["id"], {"kind": "test_drive", "name": "Somphone", "phone": "+856 20 5555 1234", "preferred_at": soon})
assert again["duplicate"] and again["id"] == td["id"], "same phone+kind within 10 min returns the first request"
of = lead(hilux["id"], {"kind": "offer", "name": "Khamla", "phone": "02077772222", "offer_cents": 50_000_000_000})
rs = lead(hilux["id"], {"kind": "reserve", "name": "Noy", "phone": "02098765432"})
assert rs["deposit_cents"] == 500_000_000 and rs["currency"] == "LAK"
fin = lead(hilux["id"], {"kind": "finance", "name": "Vanh", "phone": "02023456789", "message": "30% down"})
rs2 = lead(hilux["id"], {"kind": "reserve", "name": "Late", "phone": "02011112222"})
ok("buyer requests: validation, Lao phone numbers, duplicate guard, deposit amount")

# spam guard: 5 per phone per hour
spam = {"name": "Spam", "phone": "02033334444"}
lead(vios["id"], {**spam, "kind": "enquiry"})
lead(city["id"], {**spam, "kind": "offer", "offer_cents": 1_000_000})
lead(vios["id"], {**spam, "kind": "finance"})
lead(city["id"], {**spam, "kind": "reserve"})
lead(click["id"], {**spam, "kind": "enquiry"})
e = lead(click["id"], {"kind": "offer", "name": "Spam", "phone": "02033334444", "offer_cents": 1_000_000}, expect=400, headers=LO)
assert e["message"] == "ມີຄຳຂໍຈາກເບີໂທນີ້ຫຼາຍເກີນໄປ, ກະລຸນາລອງໃໝ່ພາຍຫຼັງ", e
ok("spam guard: max 5 requests per phone per hour")

# --- seller works the leads
call("GET", f"/shops/{sid}/leads", token=other, expect=403)
L = call("GET", f"/shops/{sid}/leads", token=dealer)
assert L["counts"]["new"] == 10, L["counts"]
row = next(x for x in L["items"] if x["id"] == td["id"])
assert row["product_name"] == "Toyota Hilux Revo" and row["make"] == "Toyota" and row["phone"] == "+8562055551234" and row["preferred_at"]
assert [x["kind"] for x in call("GET", f"/shops/{sid}/leads?kind=offer&q=Khamla", token=dealer)["items"]] == ["offer"]
call("PATCH", f"/leads/{td['id']}", {"status": "maybe"}, dealer, expect=400)
call("PATCH", f"/leads/{td['id']}", {"status": "contacted"}, other, expect=403)
u = call("PATCH", f"/leads/{td['id']}", {"status": "scheduled", "scheduled_at": soon, "seller_note": "bring ID"}, dealer)
assert u["status"] == "scheduled" and u["scheduled_at"] and u["seller_note"] == "bring ID"
u = call("PATCH", f"/leads/{td['id']}", {"scheduled_at": None}, dealer)
assert u["scheduled_at"] is None and u["seller_note"] == "bring ID"
# deposit → reserved; second deposit blocked; reserve requests blocked; undo → available
u = call("PATCH", f"/leads/{rs['id']}", {"deposit_paid": True}, dealer)
assert u["deposit_paid_at"]
assert call("GET", f"/catalog/products/{hilux['id']}")["vehicle"]["sale_status"] == "reserved"
e = call("PATCH", f"/leads/{rs2['id']}", {"deposit_paid": True}, dealer, expect=400, headers=LO)
assert e["message"] == "ລົດຄັນນີ້ມີຄົນຈອງແລ້ວ"
lead(hilux["id"], {"kind": "reserve", "name": "Third", "phone": "02066667777"}, expect=400)
lead(hilux["id"], {"kind": "enquiry", "name": "Third", "phone": "02066667777"})
assert [i["sale_status"] for i in call("GET", f"/vehicles?shop=cars-{RUN}&q=hilux")["items"]] == ["reserved"]
call("PATCH", f"/leads/{rs['id']}", {"deposit_paid": False}, dealer)
assert call("GET", f"/catalog/products/{hilux['id']}")["vehicle"]["sale_status"] == "available"
ok("seller pipeline: status, appointment, notes; deposit reserves the vehicle (one at a time), undo frees it")

# --- sold: badge for 7 days, no new requests
call("POST", f"/products/{hilux['id']}/vehicle/status", {"sale_status": "sold"}, dealer)
e = lead(hilux["id"], {"kind": "enquiry", "name": "Late", "phone": "02088889999"}, expect=400, headers=LO)
assert e["message"] == "ລົດຄັນນີ້ຂາຍແລ້ວ"
items = call("GET", f"/vehicles?shop=cars-{RUN}")["items"]
assert items[-1]["id"] == hilux["id"] and items[-1]["sale_status"] == "sold", "sold vehicles sort last"
call("PATCH", f"/leads/{of['id']}", {"deposit_paid": True}, dealer, expect=400)
call("POST", f"/products/{hilux['id']}/vehicle/status", {"sale_status": "available"}, dealer)
ok("sold vehicles: listed last with a badge, closed to new requests")

# --- online checkout rules
addr = {"name": "Buyer", "phone": "+856 20 5555 0000", "address": "Vientiane"}
e = call("POST", "/orders/checkout", {"items": [{"product_id": hilux["id"], "qty": 1}], "shipping_address": addr}, buyer, expect=400, headers=LO)
assert e["message"] == "'Toyota Hilux Revo' ບໍ່ພ້ອມຈຳໜ່າຍ", e
call("POST", f"/products/{city['id']}/vehicle/status", {"sale_status": "reserved"}, dealer)
call("POST", "/orders/checkout", {"items": [{"product_id": city["id"], "qty": 1}], "shipping_address": addr}, buyer, expect=400)
call("POST", f"/products/{city['id']}/vehicle/status", {"sale_status": "available"}, dealer)
o = call("POST", "/orders/checkout", {"items": [{"product_id": city["id"], "qty": 1}], "shipping_address": addr}, buyer)[0]
assert call("GET", f"/products/{city['id']}/vehicle", token=dealer)["sale_status"] == "sold", "stock 1 → 0 marks the vehicle sold"
call("POST", f"/orders/{o['id']}/cancel", token=buyer)
assert call("GET", f"/products/{city['id']}/vehicle", token=dealer)["sale_status"] == "available", "cancel restocks → available again"
ok("checkout: only buy-online + available vehicles; selling the last unit marks it sold, cancelling frees it")

# --- brokers: a sell-staff storefront shows the vehicle, the buyer contacts the broker
call("PATCH", f"/products/{vios['id']}", {"allow_resell": True, "commission_bps": 150}, dealer)
broker = account(f"broker-{RUN}@t.dev", "Broker")
bshop = call("POST", "/shops", {"slug": f"broker-{RUN}", "name": f"Broker {RUN}", "vertical": "vehicle", "currency": "LAK"}, broker)
call("PATCH", f"/shops/{bshop['id']}", {"phone": "020 1234 0000"}, broker)
pa = call("POST", f"/shops/{bshop['id']}/partnerships", {"supplier_shop_id": sid, "message": "car broker"}, broker)
call("POST", f"/partnerships/{pa['id']}/decision", {"approve": True}, dealer)
call("POST", f"/shops/{bshop['id']}/listings", {"product_id": vios["id"]}, broker)
bs = call("GET", f"/vehicles?shop=broker-{RUN}")
assert [(i["name"], i["via_shop_id"]) for i in bs["items"]] == [("Toyota Vios", bshop["id"])], bs["items"]
d = call("GET", f"/catalog/products/{vios['id']}?via={bshop['id']}")
assert d["contact"]["phone"] == "020 1234 0000"
ok("broker storefronts list resold vehicles and show the broker's contact")

print("ALL VEHICLE TESTS PASSED ✔")
