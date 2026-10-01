#!/usr/bin/env python3
"""Seed demo data: two supplier shops, one creator, products, a sell-staff partnership,
orders, an active ad and published creator content.

Usage: python3 scripts/seed.py [http://localhost:8080]
Logins (password: password123): siam@demo.dev, lanna@demo.dev, bee@demo.dev, buyer@demo.dev, auto@demo.dev,
khao@demo.dev (restaurant), cover@demo.dev (insurance agent),
admin@demo.dev (platform admin — start the API with ADMIN_EMAILS=admin@demo.dev)
"""
import json, sys, urllib.request, urllib.error

BASE = (sys.argv[1] if len(sys.argv) > 1 else "http://localhost:8080") + "/api"


def req(method, path, body=None, token=None):
    r = urllib.request.Request(BASE + path, data=json.dumps(body).encode() if body is not None else None, method=method)
    r.add_header("Content-Type", "application/json")
    if token:
        r.add_header("Authorization", f"Bearer {token}")
    try:
        with urllib.request.urlopen(r) as resp:
            return json.loads(resp.read() or b"null")
    except urllib.error.HTTPError as e:
        raise SystemExit(f"{method} {path} -> {e.code}: {e.read().decode()}")


def account(email, name):
    try:
        return req("POST", "/auth/register", {"email": email, "password": "password123", "display_name": name})["token"]
    except SystemExit:
        return req("POST", "/auth/login", {"email": email, "password": "password123"})["token"]


def img(seed):
    return f"https://picsum.photos/seed/{seed}/800/1000"


def main():
    siam, lanna, bee, buyer, admin = (account(e, n) for e, n in [
        ("siam@demo.dev", "Siam Crafts"), ("lanna@demo.dev", "Lanna Botanics"),
        ("bee@demo.dev", "Bee"), ("buyer@demo.dev", "Demo Buyer"), ("admin@demo.dev", "Platform Admin")])
    if req("GET", "/auth/me", token=admin)["role"] != "admin":
        raise SystemExit("admin@demo.dev is not an admin — start the API with ADMIN_EMAILS=admin@demo.dev and re-run")

    if req("GET", "/me/shops", token=siam):
        print("already seeded")
        return

    s1 = req("POST", "/shops", {"slug": "siam-crafts", "name": "Siam Crafts", "description": "Handmade ceramics and homeware from Chiang Mai artisans."}, siam)
    s2 = req("POST", "/shops", {"slug": "lanna-botanics", "name": "Lanna Botanics", "description": "Small-batch natural skincare from northern Thailand."}, lanna)
    s3 = req("POST", "/shops", {"slug": "bee-picks", "name": "Bee Picks", "kind": "creator", "description": "Things I actually use. Honest reviews, every week."}, bee)

    # Shop categories (each seller organises their own storefront)
    def shop_cat(tok, shop, name, parent=None):
        return req("POST", f"/shops/{shop['id']}/categories", {"name": name, "parent_id": parent}, tok)["id"]
    ceramics = shop_cat(siam, s1, "Ceramics")
    sc = {"mugs": shop_cat(siam, s1, "Mugs & Cups", ceramics), "bowls": shop_cat(siam, s1, "Bowls", ceramics),
          "wood": shop_cat(siam, s1, "Woodwork"), "textile": shop_cat(siam, s1, "Textiles")}
    face = shop_cat(lanna, s2, "Face")
    sc.update({"serum": shop_cat(lanna, s2, "Serums", face), "lips": shop_cat(lanna, s2, "Lip care", face),
               "body": shop_cat(lanna, s2, "Body")})

    catalog = [
        (siam, s1, "CEL-MUG", "Celadon Mug", 45000, "handmade-ceramics", "mugs", 24, True, 1200, "Hand-thrown celadon glaze mug, 350ml. Each one is unique."),
        (siam, s1, "CEL-BOWL", "Celadon Rice Bowl Set", 89000, "home-kitchen", "bowls", 12, True, 1000, "Set of 4 rice bowls in jade celadon."),
        (siam, s1, "TEAK-TRAY", "Teak Serving Tray", 129000, "handmade-woodwork", "wood", 4, False, 0, "Reclaimed teak serving tray with brass handles."),
        (siam, s1, "INDIGO-THROW", "Indigo Cotton Throw", 159000, "home-textiles", "textile", 8, True, 1500, "Hand-dyed indigo throw, 130×170cm."),
        (lanna, s2, "LB-SERUM", "Rice Water Glow Serum", 59000, "beauty-skincare", "serum", 40, True, 2000, "Fermented rice water serum with niacinamide. 30ml."),
        (lanna, s2, "LB-BALM", "Lemongrass Lip Balm", 12000, "beauty-makeup", "lips", 3, True, 1500, "Beeswax lip balm with Chiang Rai lemongrass."),
        (lanna, s2, "LB-SOAP", "Tamarind Scrub Soap", 18000, "beauty-hair-body", "body", 60, False, 0, "Cold-process tamarind scrub bar, 120g."),
    ]
    products = {}
    for tok, shop, sku, name, price, cat, scat, stock, resell, bps, desc in catalog:
        p = req("POST", f"/shops/{shop['id']}/products", {
            "sku": sku, "name": name, "price_cents": price, "category": cat, "shop_category_id": sc[scat],
            "status": "active", "initial_stock": stock, "allow_resell": resell, "commission_bps": bps or 1000,
            "description": desc, "images": [img(sku.lower())]}, tok)
        products[sku] = p

    # Moderation: approve everything except one product left in the queue for the demo
    pending = [p["id"] for sku, p in products.items() if p["review_status"] == "pending" and sku != "LB-SOAP"]
    if pending:
        req("POST", "/admin/products/review", {"ids": pending, "action": "approve"}, admin)

    # Bee becomes sell-staff for both suppliers
    for sup_tok, sup in [(siam, s1), (lanna, s2)]:
        pa = req("POST", f"/shops/{s3['id']}/partnerships", {"supplier_shop_id": sup["id"], "message": "Lifestyle creator, 80k followers on TikTok."}, bee)
        req("POST", f"/partnerships/{pa['id']}/decision", {"approve": True, "commission_bps": 1800 if sup is s2 else None}, sup_tok)
    for sku in ["CEL-MUG", "INDIGO-THROW", "LB-SERUM", "LB-BALM"]:
        req("POST", f"/shops/{s3['id']}/listings", {"product_id": products[sku]["id"]}, bee)

    # Orders: via creator and direct
    addr = {"name": "Demo Buyer", "phone": "0812345678", "line1": "99 Nimman Rd", "city": "Chiang Mai", "postcode": "50200"}
    o = req("POST", "/orders/checkout", {"items": [
        {"product_id": products["LB-SERUM"]["id"], "qty": 2, "via_shop_id": s3["id"]},
        {"product_id": products["CEL-MUG"]["id"], "qty": 1, "via_shop_id": s3["id"]},
        {"product_id": products["CEL-BOWL"]["id"], "qty": 1}], "shipping_address": addr}, buyer)
    for order in o:
        req("POST", f"/orders/{order['id']}/pay", token=buyer)
    lanna_order = next(x for x in o if x["items"][0]["supplier_shop_id"] == s2["id"])
    req("POST", f"/shops/{s2['id']}/orders/{lanna_order['id']}/status", {"status": "shipped"}, lanna)
    req("POST", f"/shops/{s2['id']}/orders/{lanna_order['id']}/status", {"status": "completed"}, lanna)

    # Ads + content
    req("POST", f"/shops/{s2['id']}/ads", {"product_id": products["LB-SERUM"]["id"], "headline": "Glass skin, naturally", "body": "Fermented rice water serum — made in Chiang Mai.", "budget_cents": 50000, "cpc_cents": 300, "status": "active"}, lanna)
    req("POST", f"/shops/{s3['id']}/ads", {"product_id": products["INDIGO-THROW"]["id"], "headline": "My favourite cosy throw", "body": "Hand-dyed indigo. Bee-approved.", "budget_cents": 30000, "cpc_cents": 200, "status": "active"}, bee)
    req("POST", f"/shops/{s1['id']}/ads", {"product_id": products["CEL-BOWL"]["id"], "headline": "Jade celadon, set of 4", "budget_cents": 40000, "cpc_cents": 250, "status": "active"}, siam)

    posts = [
        ("LB-SERUM", "post", "3 weeks with rice water serum", "Okay I didn't expect much but my skin is SO much calmer. Lightweight, no sticky feeling, and it smells faintly like jasmine rice 🍚✨\n\nMade in small batches in Chiang Mai — love supporting local."),
        ("INDIGO-THROW", "video_script", "Cosy corner makeover (30s)", "HOOK: \"My sofa went from boring to boutique with ONE thing.\"\nSCENE 1: Plain grey sofa.\nSCENE 2: Throw the indigo blanket over — slow-mo.\nSCENE 3: Close-up of the hand-dyed pattern.\nCTA: \"Link in my shop 👇\""),
        ("CEL-MUG", "caption", "Morning ritual ☕", "Every morning starts in this celadon mug. No two are the same 💚\n#celadon #handmade #chiangmai #slowliving"),
    ]
    for sku, kind, title, body in posts:
        req("POST", f"/shops/{s3['id']}/contents", {"product_id": products[sku]["id"], "kind": kind, "title": title, "body": body, "status": "published"}, bee)

    # POS: business details, barcodes and a few counter sales
    req("PATCH", f"/shops/{s1['id']}", {"legal_name": "Siam Crafts Co., Ltd.", "tax_id": "0505566012345", "branch": "สำนักงานใหญ่ / Head office",
        "address": "88 Nimmanhaemin Rd, Suthep, Mueang Chiang Mai, Chiang Mai 50200", "phone": "053-000-888",
        "receipt_footer": "ขอบคุณที่อุดหนุน · Thank you!\nExchanges within 7 days with receipt."}, siam)
    for sku, code in [("CEL-MUG", "8851234000012"), ("CEL-BOWL", "8851234000029"), ("TEAK-TRAY", "8851234000036"), ("INDIGO-THROW", "8851234000043")]:
        req("PATCH", f"/products/{products[sku]['id']}", {"barcode": code}, siam)
    req("POST", f"/shops/{s1['id']}/pos/sales", {"items": [{"product_id": products["CEL-MUG"]["id"], "qty": 2}],
        "payments": [{"method": "cash", "amount_cents": 100000}]}, siam)
    req("POST", f"/shops/{s1['id']}/pos/sales", {"items": [{"product_id": products["CEL-BOWL"]["id"], "qty": 1}, {"name": "Gift wrap", "qty": 1, "unit_price_cents": 5000}],
        "payments": [{"method": "qr", "amount_cents": 94000}], "issue_invoice": True,
        "customer": {"name": "Lanna Hotel Co., Ltd.", "tax_id": "0505560000999", "branch": "Head office", "address": "1 Charoen Prathet Rd, Chiang Mai 50100"}}, siam)

    # Delivery couriers + cash on delivery (Anousith / HAL / Mixay), and one COD order in transit
    ship = lambda tok, shop, code, **kw: req("PUT", f"/shops/{shop['id']}/shipping/{code}", {"enabled": True, "fee_payer": "buyer", "fee_cents": 0, **kw}, tok)
    ship(siam, s1, "hal", fee_cents=3000, home_fee_cents=5000, cod_enabled=True, cod_fee_cents=2000, eta="1–2 days")
    ship(siam, s1, "anousith", fee_cents=2500, free_over_cents=100000, cod_enabled=True, eta="1–3 days")
    ship(siam, s1, "mixay", fee_payer="destination", fee_cents=3500, note="Pay the courier when you pick up")
    ship(lanna, s2, "hal", fee_cents=3000, cod_enabled=True, cod_fee_cents=2000)
    ship(lanna, s2, "mixay", fee_payer="destination", fee_cents=3500)
    cod = req("POST", "/orders/checkout", {"items": [{"product_id": products["INDIGO-THROW"]["id"], "qty": 1}],
        "shipping_address": {"name": "Demo Buyer", "phone": "+856 20 5555 1234", "address": "Ban Phonxay, Saysettha, Vientiane", "branch": "HAL Saysettha"},
        "delivery": [{"shop_id": s1["id"], "carrier_code": "hal", "delivery_type": "branch", "payment_method": "cod"}]}, buyer)[0]
    req("POST", f"/shops/{s1['id']}/orders/{cod['id']}/status", {"status": "shipped", "tracking_no": "HAL240927001"}, siam)

    # Social selling: product codes, a live session and some comment orders (simulator)
    req("POST", f"/shops/{s1['id']}/social/assign-codes", token=siam)
    req("PATCH", f"/shops/{s1['id']}/social/settings", {"shipping_cents": 5000,
        "payment_instructions": "PromptPay 053-000-888 (Siam Crafts Co., Ltd.)\nKasikorn Bank 123-4-56789-0"}, siam)
    req("POST", f"/shops/{s1['id']}/social/sessions", {"title": "Sunday ceramics live"}, siam)
    for who, prov, msg in [("Nok", "facebook", "CF A01 x2"), ("Ploy", "tiktok", "A02 ราคาเท่าไหร่คะ"), ("Ploy", "tiktok", "cf a02"),
                           ("Somchai", "whatsapp", "สั่ง A04 1 ชิ้น"), ("Mali", "facebook", "F A03"), ("Fah", "facebook", "สวยมากค่ะ 😍")]:
        req("POST", f"/shops/{s1['id']}/social/simulate", {"provider": prov, "user_name": who, "message": msg}, siam)

    seed_vehicles(admin, buyer)
    seed_kyb(admin, siam, s1, lanna, s2)
    seed_restaurant(buyer)
    seed_insurance(buyer)
    seed_finance(siam, s1)
    print("Seeded ✔  Logins (password123): siam@ · lanna@ · bee@ · buyer@ · auto@ · khao@ · cover@ · admin@demo.dev")


def seed_vehicles(admin, buyer):
    """Vehicle dealer: cars + motorbikes with spec sheets, and a few buyer requests."""
    auto = account("auto@demo.dev", "Vientiane Auto")
    shop = req("POST", "/shops", {"slug": "vientiane-auto", "name": "Vientiane Auto", "vertical": "vehicle", "currency": "LAK",
        "description": "Quality used & new cars and motorbikes in Vientiane. Every vehicle inspected, papers ready, finance available."}, auto)
    req("PATCH", f"/shops/{shop['id']}", {"phone": "+856 20 5999 8888", "address": "T4 Road, Ban Phonthan, Saysettha, Vientiane Capital",
        "legal_name": "Vientiane Auto Sole Co., Ltd."}, auto)
    req("PATCH", f"/shops/{shop['id']}/social/settings", {"payment_instructions": "BCEL One QR · Vientiane Auto Sole Co., Ltd.\nBCEL 010-12-00-12345678-001"}, auto)
    cats = {c["slug"]: c["id"] for c in flatten(req("GET", "/categories"))}
    M = 100  # LAK minor units
    cars = [
        ("VA-HILUX20", "Toyota Hilux Revo 2.4 Prerunner", 545_000_000, "vehicles-cars", dict(vehicle_type="car", make="Toyota", model="Hilux Revo", variant="2.4 E Prerunner", year=2020, mileage_km=68_000, fuel="diesel", transmission="automatic", body_type="pickup", drive="rwd", engine_cc=2393, power_hp=150, seats=5, doors=4, color="White", owners=1, plate_province="Vientiane Capital", plate_no="ກຂ 1234", vin="MR0HA3CD100123456", location="Vientiane Capital", features=["Reverse camera", "Android Auto", "Cruise control", "Tow bar"], deposit_cents=5_000_000 * M, finance_available=True), True),
        ("VA-FORTUNER21", "Toyota Fortuner 2.8 Legender 4x4", 890_000_000, "vehicles-cars", dict(vehicle_type="car", make="Toyota", model="Fortuner", variant="2.8 Legender 4WD", year=2021, mileage_km=41_500, fuel="diesel", transmission="automatic", body_type="suv", drive="4wd", engine_cc=2755, power_hp=204, seats=7, doors=5, color="Black", owners=1, plate_province="Vientiane Capital", features=["Leather seats", "360° camera", "Power tailgate", "Sunroof"], deposit_cents=10_000_000 * M, finance_available=True), False),
        ("VA-RANGER22", "Ford Ranger Wildtrak 2.0 Bi-Turbo", 780_000_000, "vehicles-cars", dict(vehicle_type="car", make="Ford", model="Ranger", variant="Wildtrak 2.0 Bi-Turbo 4x4", year=2022, mileage_km=29_000, fuel="diesel", transmission="automatic", body_type="pickup", drive="4wd", engine_cc=1996, power_hp=210, seats=5, doors=4, color="Orange", owners=1, features=["Lane assist", "Apple CarPlay", "Roller shutter"], deposit_cents=8_000_000 * M, finance_available=True), False),
        ("VA-CITY19", "Honda City 1.0 Turbo SV", 298_000_000, "vehicles-cars", dict(vehicle_type="car", make="Honda", model="City", variant="1.0 Turbo SV", year=2019, mileage_km=85_200, fuel="petrol", transmission="cvt", body_type="sedan", drive="fwd", engine_cc=988, seats=5, doors=4, color="Silver", owners=2, features=["Push start", "Reverse camera"], deposit_cents=3_000_000 * M, buy_online=True), False),
        ("VA-ATTO3", "BYD Atto 3 Extended Range", 620_000_000, "vehicles-cars", dict(vehicle_type="car", make="BYD", model="Atto 3", variant="Extended Range 60 kWh", year=2024, mileage_km=0, condition="new", fuel="electric", transmission="automatic", body_type="suv", drive="fwd", power_hp=204, seats=5, doors=5, color="Blue", features=["Panoramic roof", "V2L", "ADAS"], warranty_months=96, deposit_cents=10_000_000 * M, finance_available=True, negotiable=False), False),
        ("VA-CX5-18", "Mazda CX-5 2.0 SP", 415_000_000, "vehicles-cars", dict(vehicle_type="car", make="Mazda", model="CX-5", variant="2.0 SP", year=2018, mileage_km=102_000, fuel="petrol", transmission="automatic", body_type="suv", drive="fwd", engine_cc=1998, seats=5, doors=5, color="Soul Red", owners=2, sale_status="reserved", deposit_cents=4_000_000 * M), False),
        ("VA-CLICK160", "Honda Click 160", 32_500_000, "vehicles-motorbikes", dict(vehicle_type="motorbike", make="Honda", model="Click", variant="160 ABS", year=2023, mileage_km=4_200, fuel="petrol", transmission="automatic", body_type="scooter", engine_cc=157, color="Matte Grey", owners=1, deposit_cents=1_000_000 * M, buy_online=True), False),
        ("VA-WAVE125", "Honda Wave 125i", 21_900_000, "vehicles-motorbikes", dict(vehicle_type="motorbike", make="Honda", model="Wave", variant="125i", year=2024, mileage_km=0, condition="new", fuel="petrol", transmission="semi_auto", body_type="underbone", engine_cc=125, color="Red/Black", warranty_months=24, deposit_cents=500_000 * M, negotiable=False, buy_online=True), False),
        ("VA-NMAX", "Yamaha NMAX 155 Connected", 41_000_000, "vehicles-motorbikes", dict(vehicle_type="motorbike", make="Yamaha", model="NMAX", variant="155 Connected ABS", year=2022, mileage_km=12_800, fuel="petrol", transmission="automatic", body_type="scooter", engine_cc=155, color="Black", owners=1, deposit_cents=1_000_000 * M), False),
        ("VA-CB650R", "Honda CB650R", 138_000_000, "vehicles-motorbikes", dict(vehicle_type="motorbike", make="Honda", model="CB650R", year=2021, mileage_km=9_600, fuel="petrol", transmission="manual", body_type="naked", engine_cc=649, power_hp=94, color="Candy Red", owners=1, features=["Quick shifter", "Akrapovic exhaust"], deposit_cents=3_000_000 * M), False),
        ("VA-HIACE", "Toyota Hiace Commuter 3.0", 460_000_000, "vehicles-trucks", dict(vehicle_type="van", make="Toyota", model="Hiace", variant="Commuter 3.0 D4D", year=2017, mileage_km=190_000, fuel="diesel", transmission="manual", body_type="van", drive="rwd", engine_cc=2982, seats=15, doors=4, color="Silver", owners=2, deposit_cents=5_000_000 * M), False),
    ]
    ids = {}
    for i, (sku, name, price, cat, v, resell) in enumerate(cars):
        p = req("POST", f"/shops/{shop['id']}/products", {"sku": sku, "name": name, "price_cents": price * M, "category_id": cats[cat],
            "status": "active", "initial_stock": 1, "allow_resell": resell, "commission_bps": 150,
            "description": f"{name}. Inspected by our workshop, papers ready for transfer. Test drives every day at our T4 Road showroom.",
            "images": [img(sku.lower()), img(sku.lower() + "-2"), img(sku.lower() + "-3")], "vehicle": v}, auto)
        ids[sku] = p["id"]
    req("POST", "/admin/products/review", {"ids": list(ids.values()), "action": "approve"}, admin)
    lead = lambda sku, body: req("POST", f"/catalog/products/{ids[sku]}/leads", body, buyer)
    import datetime
    soon = (datetime.datetime.now(datetime.timezone.utc) + datetime.timedelta(days=2)).replace(hour=3, minute=0, second=0, microsecond=0)
    lead("VA-HILUX20", {"kind": "test_drive", "name": "Somphone", "phone": "020 5555 1234", "preferred_at": soon.isoformat(), "message": "Can I bring my mechanic?"})
    lead("VA-FORTUNER21", {"kind": "offer", "name": "Khamla", "phone": "020 7777 2222", "offer_cents": 850_000_000 * M, "message": "Cash buyer, can pay this week."})
    lead("VA-CLICK160", {"kind": "reserve", "name": "Noy", "phone": "020 9876 5432"})
    lead("VA-ATTO3", {"kind": "finance", "name": "Vanh", "phone": "+856 20 2345 6789", "message": "30% down, 48 months?"})
    lead("VA-RANGER22", {"kind": "enquiry", "name": "Tou", "phone": "030 512 3456", "message": "Any accident history?"})


def seed_restaurant(buyer):
    """Restaurant: menu with sections, two tables, a few kitchen orders."""
    cook = account("khao@demo.dev", "Khao Niew Kitchen")
    shop = req("POST", "/shops", {"slug": "khao-niew", "name": "Khao Niew Kitchen", "vertical": "restaurant", "currency": "LAK",
        "description": "Lao home cooking in Ban Anou — larb, tam mak hoong and sticky rice, made to order."}, cook)
    sid = shop["id"]
    req("PATCH", f"/shops/{sid}", {"phone": "+856 20 5888 1212", "address": "Ban Anou, Chanthabouly, Vientiane Capital"}, cook)
    req("PUT", f"/shops/{sid}/restaurant", {"accepting_orders": True, "dine_in": True, "takeaway": True, "delivery": True,
        "service_charge_bps": 0, "prep_minutes": 15}, cook)
    M = 100
    menu = {
        ("Lao classics", "ອາຫານລາວ"): [("Larb gai", "ລາບໄກ່", 45_000, 2, "Minced chicken, toasted rice, mint, lime."),
                                       ("Tam mak hoong", "ຕຳໝາກຫຸ່ງ", 25_000, 3, "Green papaya salad with padaek."),
                                       ("Ping sin", "ປີ້ງຊີ້ນ", 55_000, 0, "Grilled marinated beef, jeow bong.")],
        ("Noodles", "ເສັ້ນ"): [("Khao piak sen", "ເຂົ້າປຽກເສັ້ນ", 30_000, 0, "Rice noodle soup with chicken."),
                               ("Khao soi Luang Prabang", "ເຂົ້າຊອຍຫຼວງພະບາງ", 35_000, 1, "Pork tomato sauce, wide noodles.")],
        ("Rice & sides", "ເຂົ້າ ແລະ ອື່ນໆ"): [("Sticky rice", "ເຂົ້າໜຽວ", 5_000, 0, "Steamed in a bamboo basket.")],
        ("Drinks", "ເຄື່ອງດື່ມ"): [("Lao iced coffee", "ກາເຟລາວເຢັນ", 15_000, 0, "Dark roast, condensed milk."),
                                 ("Fresh coconut", "ໝາກພ້າວ", 20_000, 0, "")],
    }
    items = {}
    for (sec, sec_lo), dishes in menu.items():
        section = req("POST", f"/shops/{sid}/restaurant/sections", {"name": sec, "name_lo": sec_lo}, cook)
        for name, name_lo, price, spicy, desc in dishes:
            it = req("POST", f"/shops/{sid}/restaurant/items", {"name": name, "name_lo": name_lo, "price_cents": price * M,
                "spicy": spicy, "description": desc, "section_id": section["id"], "image_url": img("food-" + name.lower().replace(" ", "-"))}, cook)
            items[name] = it["id"]
    t1 = req("POST", f"/shops/{sid}/restaurant/tables", {"label": "T1", "seats": 4}, cook)
    req("POST", f"/shops/{sid}/restaurant/tables", {"label": "T2", "seats": 6}, cook)
    o1 = req("POST", "/restaurant/menu/khao-niew/orders", {"mode": "dine_in", "table_token": t1["token"], "items": [
        {"item_id": items["Larb gai"], "qty": 1}, {"item_id": items["Sticky rice"], "qty": 2}, {"item_id": items["Lao iced coffee"], "qty": 2}]})
    req("POST", "/restaurant/menu/khao-niew/orders", {"mode": "takeaway", "customer_name": "Noy", "customer_phone": "020 9876 5432",
        "items": [{"item_id": items["Tam mak hoong"], "qty": 2, "note": "medium spicy"}]}, buyer)
    req("PATCH", f"/restaurant/orders/{o1['id']}", {"status": "served", "paid": True, "payment_method": "cash"}, cook)


def seed_insurance(buyer):
    """Insurance agent with motor, travel and health plans, and one application."""
    agent = account("cover@demo.dev", "Mekong Cover")
    shop = req("POST", "/shops", {"slug": "mekong-cover", "name": "Mekong Cover", "vertical": "insurance", "currency": "LAK",
        "description": "Licensed insurance agent in Vientiane — car, motorbike, travel and health cover from Lao insurers."}, agent)
    req("PATCH", f"/shops/{shop['id']}", {"phone": "+856 20 5444 3333", "address": "Lane Xang Avenue, Vientiane Capital"}, agent)
    M = 100
    motor = req("POST", f"/shops/{shop['id']}/insurance/plans", {"kind": "motor", "insurer": "Demo Insurer A", "name": "Car — comprehensive",
        "name_lo": "ລົດ — ຄຸ້ມຄອງທຸກຢ່າງ", "coverage": ["Own damage & theft", "Third-party injury and property", "24h roadside help"],
        "premium_mode": "rate", "rate_bps": 250, "min_premium_cents": 1_500_000 * M,
        "sum_insured_min_cents": 50_000_000 * M, "sum_insured_max_cents": 3_000_000_000 * M, "term_months": 12}, agent)
    req("POST", f"/shops/{shop['id']}/insurance/plans", {"kind": "motor", "insurer": "Demo Insurer A", "name": "Motorbike — third party",
        "name_lo": "ລົດຈັກ — ບຸກຄົນທີສາມ", "coverage": ["Third-party injury", "Third-party property"], "premium_cents": 180_000 * M, "term_months": 12}, agent)
    req("POST", f"/shops/{shop['id']}/insurance/plans", {"kind": "travel", "insurer": "Demo Insurer B", "name": "ASEAN travel 30 days",
        "name_lo": "ທ່ອງທ່ຽວອາຊຽນ 30 ວັນ", "coverage": ["Medical up to 50,000 USD", "Trip delay", "Lost luggage"], "premium_cents": 250_000 * M, "term_months": 1}, agent)
    req("POST", f"/shops/{shop['id']}/insurance/plans", {"kind": "health", "insurer": "Demo Insurer B", "name": "Family health",
        "name_lo": "ສຸຂະພາບຄອບຄົວ", "coverage": ["In-patient", "Out-patient 20 visits", "Dental"], "premium_cents": 4_800_000 * M, "term_months": 12}, agent)
    import datetime
    start = (datetime.date.today() + datetime.timedelta(days=7)).isoformat()
    req("POST", f"/insurance/plans/{motor['id']}/apply", {"applicant_name": "Somphone K.", "phone": "020 5555 1234",
        "sum_insured_cents": 540_000_000 * M, "start_date": start, "details": {"plate": "ກຂ 1234", "make": "Toyota", "model": "Hilux Revo", "year": 2020}}, buyer)


def seed_finance(siam, s1):
    """Siam Crafts (verified business) appoints the platform as its tax agent."""
    pol = req("GET", "/finance/policy")
    req("POST", f"/shops/{s1['id']}/finance/mandate", {"accept": True, "policy_version": pol["policy_version"],
        "signer_name": "Anan S.", "signer_title": "Managing Director", "vat_registered": True}, siam)


def upload_doc(shop_id, kind, data, name, token):
    import uuid as _u
    b = _u.uuid4().hex
    body = (f'--{b}\r\nContent-Disposition: form-data; name="kind"\r\n\r\n{kind}\r\n'
            f'--{b}\r\nContent-Disposition: form-data; name="file"; filename="{name}"\r\nContent-Type: application/pdf\r\n\r\n').encode() + data + f"\r\n--{b}--\r\n".encode()
    r = urllib.request.Request(f"{BASE}/shops/{shop_id}/kyb/documents", data=body, method="POST")
    r.add_header("Content-Type", f"multipart/form-data; boundary={b}")
    r.add_header("Authorization", f"Bearer {token}")
    with urllib.request.urlopen(r) as resp:
        return json.loads(resp.read())


def sample_pdf(title):
    """Tiny valid one-page PDF with a line of text (demo documents)."""
    content = f"BT /F1 18 Tf 60 760 Td ({title}) Tj ET BT /F1 11 Tf 60 730 Td (Demo document - zaokaiy seed data) Tj ET".encode()
    objs = [b"<</Type/Catalog/Pages 2 0 R>>", b"<</Type/Pages/Kids[3 0 R]/Count 1>>",
            b"<</Type/Page/Parent 2 0 R/MediaBox[0 0 595 842]/Resources<</Font<</F1 4 0 R>>>>/Contents 5 0 R>>",
            b"<</Type/Font/Subtype/Type1/BaseFont/Helvetica>>", b"<</Length %d>>stream\n" % len(content) + content + b"\nendstream"]
    out, offs = b"%PDF-1.4\n", []
    for i, o in enumerate(objs, 1):
        offs.append(len(out))
        out += b"%d 0 obj\n" % i + o + b"\nendobj\n"
    xref = len(out)
    out += b"xref\n0 %d\n0000000000 65535 f \n" % (len(objs) + 1) + b"".join(b"%010d 00000 n \n" % o for o in offs)
    out += b"trailer<</Size %d/Root 1 0 R>>\nstartxref\n%d\n%%%%EOF\n" % (len(objs) + 1, xref)
    return out


def seed_kyb(admin, siam, s1, lanna, s2):
    """Business verification: Siam Crafts verified, Lanna Botanics waiting in the admin queue, Vientiane Auto verified."""
    auto = req("POST", "/auth/login", {"email": "auto@demo.dev", "password": "password123"})["token"]
    s3 = next(s for s in req("GET", "/me/shops", token=auto) if s["slug"] == "vientiane-auto")
    companies = [
        (siam, s1, {"company_type": "limited_company", "legal_name": "Siam Crafts Co., Ltd.", "registration_no": "0505566012345", "country": "TH",
                    "tax_id": "0505566012345", "province": "Chiang Mai", "registered_address": "88 Nimmanhaemin Rd, Suthep, Mueang Chiang Mai 50200",
                    "business_activity": "Handmade ceramics and homeware", "contact_phone": "053-000-888", "contact_email": "hello@siamcrafts.example",
                    "bank_name": "Kasikorn Bank", "bank_account_name": "Siam Crafts Co., Ltd.", "bank_account_number": "123-4-56789-0",
                    "persons": [{"roles": ["representative", "director", "ubo"], "full_name": "Somchai Jaidee", "title": "Managing Director", "nationality": "TH",
                                 "date_of_birth": "1980-04-12", "id_type": "national_id", "id_number": "3-5099-00123-45-6", "ownership_bps": 7000},
                                {"roles": ["ubo"], "full_name": "Malee Jaidee", "nationality": "TH", "id_type": "national_id", "id_number": "3-5099-00456-78-9", "ownership_bps": 3000}]},
         True, ["registration_certificate", "tax_certificate", "representative_id", "shareholder_register"]),
        (lanna, s2, {"company_type": "sole_enterprise", "legal_name": "Lanna Botanics", "registration_no": "0503560004567", "country": "TH", "tax_id": "1509900123456",
                     "province": "Chiang Mai", "registered_address": "12 Charoen Rat Rd, Chiang Mai 50000", "business_activity": "Natural skincare production and retail",
                     "contact_phone": "081-234-5678", "bank_name": "Bangkok Bank", "bank_account_name": "Lanna Botanics", "bank_account_number": "987-6-54321-0",
                     "persons": [{"roles": ["representative", "director"], "full_name": "Ploy Srisuk", "title": "Owner", "nationality": "TH",
                                  "id_type": "national_id", "id_number": "1-5099-00987-65-4"}]},
         False, ["registration_certificate", "tax_certificate", "representative_id"]),
        (auto, s3, {"company_type": "sole_company", "legal_name": "Vientiane Auto Sole Co., Ltd.", "legal_name_local": "ບໍລິສັດ ວຽງຈັນ ອໍໂຕ້ ຈຳກັດຜູ້ດຽວ",
                    "registration_no": "01-00045678", "registration_date": "2018-02-14", "country": "LA", "tax_id": "123456789-000",
                    "province": "Vientiane Capital", "registered_address": "T4 Road, Ban Phonthan, Saysettha, Vientiane Capital",
                    "business_activity": "Sale of new and used cars and motorbikes", "contact_phone": "+856 20 5999 8888",
                    "bank_name": "BCEL", "bank_account_name": "Vientiane Auto Sole Co., Ltd.", "bank_account_number": "010-12-00-12345678-001",
                    "persons": [{"roles": ["representative", "director", "ubo"], "full_name": "Souksavanh Phommachanh", "title": "Director", "nationality": "LA",
                                 "date_of_birth": "1983-09-01", "id_type": "national_id", "id_number": "01-0123456", "ownership_bps": 10000}]},
         True, ["registration_certificate", "tax_certificate", "representative_id", "shareholder_register", "business_license"]),
    ]
    for tok, shop, profile, approve, docs in companies:
        req("PATCH", f"/shops/{shop['id']}", {"entity_type": "business"}, tok)
        req("PUT", f"/shops/{shop['id']}/kyb", profile, tok)
        for k in docs:
            upload_doc(shop["id"], k, sample_pdf(f"{profile['legal_name']} - {k.replace('_', ' ')}"), f"{k}.pdf", tok)
        req("POST", f"/shops/{shop['id']}/kyb/submit", token=tok)
        if approve:
            req("POST", f"/admin/kyb/{shop['id']}/decision", {"action": "approve", "note": "Documents checked"}, admin)


def flatten(nodes):
    for n in nodes:
        yield n
        yield from flatten(n.get("children") or [])


if __name__ == "__main__":
    main()
