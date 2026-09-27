#!/usr/bin/env python3
"""POS / receipt / invoice tests. Usage: python3 scripts/pos_test.py [http://localhost:8080]"""
import json, sys, urllib.request, urllib.error, uuid

API = (sys.argv[1] if len(sys.argv) > 1 else "http://localhost:8080") + "/api"
RUN = uuid.uuid4().hex[:6]


def call(method, path, body=None, token=None, expect=200):
    r = urllib.request.Request(API + path, data=json.dumps(body).encode() if body is not None else None, method=method)
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


def user(name):
    return call("POST", "/auth/register", {"email": f"{name}-{RUN}@t.dev", "password": "password123", "display_name": name})["token"]


def main():
    owner, stranger = user("cashier"), user("stranger")
    shop = call("POST", "/shops", {"slug": f"pos-{RUN}", "name": "Counter Shop"}, owner)
    sid = shop["id"]
    assert shop["vat_bps"] == 700 and shop["prices_include_vat"] and shop["receipt_prefix"] == "R"

    # Business / tax settings
    call("PATCH", f"/shops/{sid}", {"vat_bps": 99999}, owner, expect=400)
    call("PATCH", f"/shops/{sid}", {"receipt_prefix": "bad prefix!"}, owner, expect=400)
    shop = call("PATCH", f"/shops/{sid}", {"legal_name": "Counter Co., Ltd.", "tax_id": "0105555555555", "branch": "Head office",
                                           "address": "1 Rama IV, Bangkok 10500", "phone": "02-000-0000", "receipt_prefix": "cs"}, owner)
    assert shop["receipt_prefix"] == "CS"

    # Products with barcodes
    mug = call("POST", f"/shops/{sid}/products", {"sku": "MUG", "name": "Mug", "price_cents": 45000, "initial_stock": 10, "barcode": "8850000000011"}, owner)
    tea = call("POST", f"/shops/{sid}/products", {"sku": "TEA", "name": "Jasmine Tea", "price_cents": 12000, "initial_stock": 3}, owner)
    call("POST", f"/shops/{sid}/products", {"sku": "DUP", "name": "dup", "price_cents": 1, "barcode": "8850000000011"}, owner, expect=409)
    call("PATCH", f"/products/{tea['id']}", {"barcode": "8850000000028"}, owner)
    hits = call("GET", f"/shops/{sid}/pos/products?q=8850000000028", token=owner)
    assert hits[0]["id"] == tea["id"] and hits[0]["exact"], hits
    assert len(call("GET", f"/shops/{sid}/pos/products", token=owner)) == 2        # browse without a query
    call("GET", f"/shops/{sid}/pos/products", token=stranger, expect=403)

    # Sale: 2 mugs (line discount 50.00) + 1 tea + custom item 30.00, bill discount 10.00, VAT incl. 7%
    body = {
        "items": [
            {"product_id": mug["id"], "qty": 2, "discount_cents": 5000},
            {"product_id": tea["id"], "qty": 1},
            {"name": "Gift wrap", "qty": 1, "unit_price_cents": 3000},
        ],
        "discount_cents": 1000,
        "payments": [{"method": "cash", "amount_cents": 100000}],
    }
    doc = call("POST", f"/shops/{sid}/pos/sales", body, owner)
    s = doc["sale"]
    # subtotal = 90000-5000 + 12000 + 3000 = 100000 ; net = 99000 ; VAT = 99000*7/107 = 6476.6 -> 6477
    assert (s["subtotal_cents"], s["discount_cents"], s["total_cents"], s["vat_cents"]) == (100000, 1000, 99000, 6477), s
    assert s["change_cents"] == 1000 and s["number"] == "CS000001", s
    assert doc["amount_before_vat_cents"] == 99000 - 6477
    assert doc["shop"]["tax_id"] == "0105555555555" and doc["cashier"] == "cashier"
    assert [i["line_total_cents"] for i in doc["items"]] == [85000, 12000, 3000]
    assert call("GET", f"/products/{mug['id']}", token=owner)["stock"] == 8
    moves = call("GET", f"/shops/{sid}/inventory/movements?product_id={mug['id']}", token=owner)
    assert moves[0]["delta"] == -2 and moves[0]["note"] == "POS CS000001", moves[0]

    # Validation
    call("POST", f"/shops/{sid}/pos/sales", {**body, "payments": [{"method": "cash", "amount_cents": 500}]}, owner, expect=400)       # short
    call("POST", f"/shops/{sid}/pos/sales", {**body, "payments": [{"method": "card", "amount_cents": 100000}]}, owner, expect=400)    # card change
    call("POST", f"/shops/{sid}/pos/sales", {"items": [{"product_id": tea["id"], "qty": 5}], "payments": [{"method": "cash", "amount_cents": 99999}]}, owner, expect=400)  # stock
    call("POST", f"/shops/{sid}/pos/sales", {"items": [{"name": "x", "qty": 1}], "payments": []}, owner, expect=400)                  # custom w/o price
    call("POST", f"/shops/{sid}/pos/sales", body, stranger, expect=403)

    # Split payment + tax invoice at checkout
    s2 = call("POST", f"/shops/{sid}/pos/sales", {
        "items": [{"product_id": tea["id"], "qty": 1}],
        "payments": [{"method": "card", "amount_cents": 10000, "reference": "1234"}, {"method": "cash", "amount_cents": 5000}],
        "issue_invoice": True, "customer": {"name": "ACME Co., Ltd.", "address": "9 Silom, Bangkok", "tax_id": "0107777777777"}}, owner)["sale"]
    assert s2["number"] == "CS000002" and s2["invoice_number"] == "INV000001" and s2["change_cents"] == 3000
    call("POST", f"/shops/{sid}/pos/sales", {"items": [{"product_id": tea["id"], "qty": 1}], "payments": [{"method": "cash", "amount_cents": 12000}],
                                            "issue_invoice": True, "customer": {"name": "No address"}}, owner, expect=400)

    # Invoice later for sale 1
    call("POST", f"/pos/sales/{s['id']}/invoice", {"customer_name": "", "customer_address": ""}, owner, expect=400)
    inv = call("POST", f"/pos/sales/{s['id']}/invoice", {"customer_name": "Buyer Ltd", "customer_address": "Chiang Mai", "customer_tax_id": "0101"}, owner)["sale"]
    assert inv["invoice_number"] == "INV000002" and inv["invoice_issued_at"]
    call("POST", f"/pos/sales/{s['id']}/invoice", {"customer_name": "x", "customer_address": "y"}, owner, expect=409)

    # Summary before void
    summ = call("GET", f"/shops/{sid}/pos/summary", token=owner)
    assert summ["sales"] == 2 and summ["total_cents"] == 99000 + 12000, summ
    assert summ["by_method"]["cash"] == (100000 - 1000) + (5000 - 3000) and summ["by_method"]["card"] == 10000, summ["by_method"]
    assert summ["first_number"] == "CS000001" and summ["invoices"] == 2

    # Void restocks; can't void twice or invoice a voided sale
    call("POST", f"/pos/sales/{s['id']}/void", {"reason": ""}, owner, expect=400)
    v = call("POST", f"/pos/sales/{s['id']}/void", {"reason": "Customer returned"}, owner)["sale"]
    assert v["status"] == "voided"
    assert call("GET", f"/products/{mug['id']}", token=owner)["stock"] == 10
    call("POST", f"/pos/sales/{s['id']}/void", {"reason": "again"}, owner, expect=400)
    summ = call("GET", f"/shops/{sid}/pos/summary", token=owner)
    assert summ["sales"] == 1 and summ["voids"] == 1 and summ["void_total_cents"] == 99000

    # List + search + VAT-exclusive shop
    lst = call("GET", f"/shops/{sid}/pos/sales?q=INV000001", token=owner)
    assert len(lst) == 1 and lst[0]["number"] == "CS000002"
    call("GET", f"/pos/sales/{s['id']}", token=stranger, expect=403)
    call("PATCH", f"/shops/{sid}", {"prices_include_vat": False}, owner)
    s3 = call("POST", f"/shops/{sid}/pos/sales", {"items": [{"product_id": mug["id"], "qty": 1}], "payments": [{"method": "qr", "amount_cents": 48150}]}, owner)["sale"]
    assert (s3["vat_cents"], s3["total_cents"], s3["number"]) == (3150, 48150, "CS000003"), s3
    # old documents keep their original VAT basis
    assert call("GET", f"/pos/sales/{s2['id']}", token=owner)["sale"]["prices_include_vat"] is True

    # Online order document: buyer + supplier can print, strangers can't
    buyer = user("buyer")
    call("PATCH", f"/shops/{sid}", {"prices_include_vat": True}, owner)
    admin = call("POST", "/auth/login", {"email": "admin@demo.dev", "password": "password123"})["token"]
    live = call("POST", f"/shops/{sid}/products", {"sku": "ONL", "name": "Online item", "price_cents": 10700, "initial_stock": 5, "status": "active", "category": "home"}, owner)
    if live["review_status"] == "pending":
        call("POST", f"/admin/products/{live['id']}/review", {"action": "approve"}, admin)
    order = call("POST", "/orders/checkout", {"items": [{"product_id": live["id"], "qty": 1}], "shipping_address": {"name": "B", "line1": "x"}}, buyer)[0]
    d = call("GET", f"/orders/{order['id']}/document", token=buyer)
    assert d["vat_cents"] == 700 and d["shop"]["tax_id"] == "0105555555555"
    call("GET", f"/orders/{order['id']}/document", token=owner)
    call("GET", f"/orders/{order['id']}/document", token=stranger, expect=403)
    print("ALL POS TESTS PASSED ✔")


if __name__ == "__main__":
    main()
