#!/usr/bin/env python3
"""Categories + product moderation tests. Requires the API started with ADMIN_EMAILS=admin@demo.dev.
Usage: python3 scripts/review_test.py [http://localhost:8080]"""
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


def account(email, name="x"):
    try:
        return call("POST", "/auth/login", {"email": email, "password": "password123"})["token"]
    except AssertionError:
        return call("POST", "/auth/register", {"email": email, "password": "password123", "display_name": name})["token"]


def main():
    admin = account("admin@demo.dev", "Admin")
    assert call("GET", "/auth/me", token=admin)["role"] == "admin", "start the API with ADMIN_EMAILS=admin@demo.dev"
    seller = account(f"seller-{RUN}@t.dev")
    other = account(f"other-{RUN}@t.dev")
    buyer = account(f"buyer-{RUN}@t.dev")
    shop = call("POST", "/shops", {"slug": f"rv-{RUN}", "name": "Review Shop"}, seller)
    oshop = call("POST", "/shops", {"slug": f"ro-{RUN}", "name": "Other Shop"}, other)

    # ---------------- Marketplace categories (admin) ----------------
    call("GET", "/admin/overview", token=seller, expect=403)
    call("POST", "/admin/categories", {"name": "Nope"}, seller, expect=403)
    top = call("POST", "/admin/categories", {"name": f"Pets {RUN}"}, admin)
    sub = call("POST", "/admin/categories", {"name": "Cat Toys", "parent_id": top["id"]}, admin)
    subsub = call("POST", "/admin/categories", {"name": "Feathers", "parent_id": sub["id"]}, admin)
    call("POST", "/admin/categories", {"name": "Too deep", "parent_id": subsub["id"]}, admin, expect=400)
    call("PATCH", f"/categories/{top['id']}", {"parent_id": subsub["id"]}, admin, expect=400)   # cycle
    call("PATCH", f"/categories/{sub['id']}", {"name": "x"}, seller, expect=403)                 # seller can't edit marketplace
    tree = call("GET", "/categories")
    node = next(n for n in tree if n["id"] == subsub["id"])
    assert node["depth"] == 2 and node["path"].endswith("Cat Toys › Feathers"), node
    thai = call("POST", "/admin/categories", {"name": "ของใช้สัตว์เลี้ยง", "parent_id": top["id"]}, admin)
    assert thai["slug"].startswith("c-"), thai

    # ---------------- Shop categories (seller) ----------------
    face = call("POST", f"/shops/{shop['id']}/categories", {"name": "Face"}, seller)
    serums = call("POST", f"/shops/{shop['id']}/categories", {"name": "Serums", "parent_id": face["id"]}, seller)
    call("POST", f"/shops/{shop['id']}/categories", {"name": "Face"}, seller)                     # dup name -> new slug
    call("POST", f"/shops/{oshop['id']}/categories", {"name": "x"}, seller, expect=403)
    call("POST", f"/shops/{oshop['id']}/categories", {"name": "x", "parent_id": face["id"]}, other, expect=400)  # other tree
    call("POST", f"/shops/{shop['id']}/categories", {"name": "bad", "parent_id": sub["id"]}, seller, expect=400)
    body = call("POST", f"/shops/{shop['id']}/categories", {"name": "Body"}, seller)
    call("POST", "/categories/reorder", {"ids": [serums["id"], face["id"]]}, seller, expect=400)  # not siblings
    call("POST", "/categories/reorder", {"ids": [body["id"], face["id"]]}, seller)
    lst = call("GET", f"/shops/{shop['id']}/categories", token=seller)
    assert [n["name"] for n in lst if n["depth"] == 0][:2] == ["Body", "Face"], lst
    assert lst[2]["name"] == "Serums" and lst[2]["depth"] == 1                                    # pre-order tree

    # ---------------- Review workflow ----------------
    draft = call("POST", f"/shops/{shop['id']}/products", {"sku": "D1", "name": "Draft", "price_cents": 100}, seller)
    assert draft["review_status"] == "not_submitted"
    call("POST", f"/products/{draft['id']}/submit", token=seller, expect=400)                    # no category yet
    p = call("POST", f"/shops/{shop['id']}/products", {
        "sku": "P1", "name": "Feather Wand", "price_cents": 25000, "status": "active", "initial_stock": 5,
        "category_id": subsub["id"], "shop_category_id": serums["id"]}, seller)
    assert p["review_status"] == "pending" and p["submitted_at"], p
    call("GET", f"/catalog/products/{p['id']}", expect=404)
    call("POST", "/orders/checkout", {"items": [{"product_id": p["id"], "qty": 1}], "shipping_address": {}}, buyer, expect=400)
    queue = call("GET", "/admin/products?review_status=pending", token=admin)
    assert any(q["id"] == p["id"] for q in queue)

    call("POST", f"/admin/products/{p['id']}/review", {"action": "reject"}, admin, expect=400)    # reason required
    rej = call("POST", f"/admin/products/{p['id']}/review", {"action": "reject", "note": "Photos missing"}, admin)
    assert rej["review_status"] == "rejected" and rej["review_note"] == "Photos missing"
    fixed = call("PATCH", f"/products/{p['id']}", {"description": "Now with details"}, seller)
    assert fixed["review_status"] == "pending" and fixed["review_note"] == "", fixed
    call("POST", f"/admin/products/{p['id']}/review", {"action": "approve"}, admin)
    live = call("GET", f"/catalog/products/{p['id']}")
    assert [b["name"] for b in live["breadcrumb"]] == [f"Pets {RUN}", "Cat Toys", "Feathers"]

    # category filter includes sub-categories; storefront exposes shop categories
    assert any(x["id"] == p["id"] for x in call("GET", f"/catalog/products?category={top['slug']}"))
    front = call("GET", f"/storefront/rv-{RUN}")
    assert {c["name"] for c in front["categories"]} >= {"Face", "Serums"}
    assert next(c for c in front["categories"] if c["id"] == face["id"])["total_count"] == 1

    # price/stock edits stay live; content edits go back to review
    assert call("PATCH", f"/products/{p['id']}", {"price_cents": 26000}, seller)["review_status"] == "approved"
    assert call("PATCH", f"/products/{p['id']}", {"name": "Feather Wand XL"}, seller)["review_status"] == "pending"
    call("GET", f"/catalog/products/{p['id']}", expect=404)
    call("POST", "/admin/products/review", {"ids": [p["id"]], "action": "approve"}, admin)
    call("GET", f"/catalog/products/{p['id']}")

    hist = call("GET", f"/products/{p['id']}/reviews", token=seller)
    assert [h["action"] for h in hist][:3] == ["approved", "resubmitted", "approved"], hist
    assert hist[0]["actor"] == "Marketplace team"
    detail = call("GET", f"/admin/products/{p['id']}", token=admin)
    assert detail["category_path"].endswith("Feathers") and detail["history"][0]["actor"] not in (None, "Marketplace team")

    # trusted shop auto-approves
    call("PATCH", f"/admin/shops/{shop['id']}", {"auto_approve": True}, admin)
    auto = call("POST", f"/shops/{shop['id']}/products", {"sku": "A1", "name": "Auto", "price_cents": 100, "status": "active", "category_id": sub["id"]}, seller)
    assert auto["review_status"] == "approved"

    # ---------------- Category deletion rules ----------------
    call("DELETE", f"/categories/{face['id']}", token=seller, expect=409)                         # has children
    call("DELETE", f"/categories/{serums['id']}", token=seller)                                   # products become uncategorised
    assert call("GET", f"/products/{p['id']}", token=seller)["shop_category_id"] is None
    call("DELETE", f"/categories/{subsub['id']}", token=admin, expect=409)                        # products use it
    call("DELETE", f"/categories/{subsub['id']}?reassign_to={sub['id']}", token=admin)
    assert call("GET", f"/products/{p['id']}", token=seller)["category_id"] == sub["id"]
    call("PATCH", f"/categories/{thai['id']}", {"active": False}, admin)
    assert all(n["id"] != thai["id"] for n in call("GET", "/categories"))
    call("POST", f"/shops/{shop['id']}/products", {"sku": "X9", "name": "x", "price_cents": 1, "category_id": thai["id"]}, seller, expect=400)
    print("ALL CATEGORY & REVIEW TESTS PASSED ✔")


if __name__ == "__main__":
    main()
