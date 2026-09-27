#!/usr/bin/env python3
"""Seed demo data: two supplier shops, one creator, products, a sell-staff partnership,
orders, an active ad and published creator content.

Usage: python3 scripts/seed.py [http://localhost:8080]
Logins (password: password123): siam@demo.dev, lanna@demo.dev, bee@demo.dev, buyer@demo.dev,
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
    for sku, code in [("CEL-MUG", "8851234000011"), ("CEL-BOWL", "8851234000028"), ("TEAK-TRAY", "8851234000035"), ("INDIGO-THROW", "8851234000042")]:
        req("PATCH", f"/products/{products[sku]['id']}", {"barcode": code}, siam)
    req("POST", f"/shops/{s1['id']}/pos/sales", {"items": [{"product_id": products["CEL-MUG"]["id"], "qty": 2}],
        "payments": [{"method": "cash", "amount_cents": 100000}]}, siam)
    req("POST", f"/shops/{s1['id']}/pos/sales", {"items": [{"product_id": products["CEL-BOWL"]["id"], "qty": 1}, {"name": "Gift wrap", "qty": 1, "unit_price_cents": 5000}],
        "payments": [{"method": "qr", "amount_cents": 94000}], "issue_invoice": True,
        "customer": {"name": "Lanna Hotel Co., Ltd.", "tax_id": "0505560000999", "branch": "Head office", "address": "1 Charoen Prathet Rd, Chiang Mai 50100"}}, siam)

    # Social selling: product codes, a live session and some comment orders (simulator)
    req("POST", f"/shops/{s1['id']}/social/assign-codes", token=siam)
    req("PATCH", f"/shops/{s1['id']}/social/settings", {"shipping_cents": 5000,
        "payment_instructions": "PromptPay 053-000-888 (Siam Crafts Co., Ltd.)\nKasikorn Bank 123-4-56789-0"}, siam)
    req("POST", f"/shops/{s1['id']}/social/sessions", {"title": "Sunday ceramics live"}, siam)
    for who, prov, msg in [("Nok", "facebook", "CF A01 x2"), ("Ploy", "tiktok", "A02 ราคาเท่าไหร่คะ"), ("Ploy", "tiktok", "cf a02"),
                           ("Somchai", "whatsapp", "สั่ง A04 1 ชิ้น"), ("Mali", "facebook", "F A03"), ("Fah", "facebook", "สวยมากค่ะ 😍")]:
        req("POST", f"/shops/{s1['id']}/social/simulate", {"provider": prov, "user_name": who, "message": msg}, siam)

    print("Seeded ✔  Logins (password123): siam@ · lanna@ · bee@ · buyer@ · admin@demo.dev")


if __name__ == "__main__":
    main()
