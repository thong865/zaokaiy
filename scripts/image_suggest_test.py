#!/usr/bin/env python3
"""image_suggest module: stock photos + own library + opted-in shop photos suggested while typing a
product name (any language), picked into the seller's library as independent copies.
Start the API with ADMIN_EMAILS=admin@demo.dev. Usage: python3 scripts/image_suggest_test.py [http://localhost:8080]
"""
import json, struct, sys, urllib.error, urllib.parse, urllib.request, uuid, zlib

BASE = sys.argv[1] if len(sys.argv) > 1 else "http://localhost:8080"
API = BASE + "/api"
RUN = uuid.uuid4().hex[:6]


def png(w, h, rgb=(200, 150, 30)):
    raw = b"".join(b"\x00" + bytes(rgb) * w for _ in range(h))
    def chunk(t, d):
        return struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d) & 0xFFFFFFFF)
    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b""))


def call(method, path, body=None, token=None, expect=200, raw=None, ctype="application/json", headers=None):
    data = raw if raw is not None else (json.dumps(body).encode() if body is not None else None)
    r = urllib.request.Request(API + path, data=data, method=method)
    r.add_header("Content-Type", ctype)
    for k, v in (headers or {}).items():
        r.add_header(k, v)
    if token:
        r.add_header("Authorization", f"Bearer {token}")
    try:
        with urllib.request.urlopen(r) as resp:
            code, out = resp.status, json.loads(resp.read() or b"null")
    except urllib.error.HTTPError as e:
        code, out = e.code, json.loads(e.read() or b"null")
    assert code == expect, f"{method} {path} -> {code} (expected {expect}): {out}"
    return out


def multipart(fields, files):
    b = uuid.uuid4().hex
    parts = [f'--{b}\r\nContent-Disposition: form-data; name="{k}"\r\n\r\n{v}\r\n'.encode() for k, v in fields.items()]
    parts += [f'--{b}\r\nContent-Disposition: form-data; name="file"; filename="{n}"\r\nContent-Type: application/octet-stream\r\n\r\n'.encode() + d + b"\r\n" for n, d in files]
    return b"".join(parts) + f"--{b}--\r\n".encode(), f"multipart/form-data; boundary={b}"


def stock(token, title, keywords="", brand="", color=(200, 150, 30), expect=200, data=None):
    raw, ct = multipart({"title": title, "keywords": keywords, "brand": brand, "credit": "test"}, [("p.png", data if data is not None else png(600, 600, color))])
    return call("POST", "/admin/image-suggest/stock", token=token, raw=raw, ctype=ct, expect=expect)


def fetch(url):
    try:
        with urllib.request.urlopen(url) as r:
            return r.status
    except urllib.error.HTTPError as e:
        return e.code


def account(email, name):
    try:
        return call("POST", "/auth/login", {"email": email, "password": "password123"})["token"]
    except AssertionError:
        return call("POST", "/auth/register", {"email": email, "password": "password123", "display_name": name})["token"]


ok = lambda m: print("  ✔", m)
print("image_suggest module tests")
assert call("GET", "/modules")["image_suggest"]["enabled"]

admin = account("admin@demo.dev", "Admin")
T = f"x{RUN}"  # unique tag so earlier runs don't interfere

# ---- stock library ----------------------------------------------------------------------------
stock(admin, "", expect=400)
stock(admin, f"Beerlao {T}", data=b"not an image", expect=400)
beer = stock(admin, f"Beerlao Original 640ml {T}", keywords="beer, bottle, ເບຍລາວ", brand="Beerlao")
tiger = stock(admin, f"Tiger beer can {T}", brand="Tiger", color=(20, 60, 160))
coffee = stock(admin, f"Lao coffee beans {T}", color=(90, 50, 20))
assert beer["variants"] and beer["placeholder"] and beer["width"] == 600, beer
assert fetch(beer["url"]) == 200
seller_a = account(f"img-a-{RUN}@t.dev", "Supplier A")
call("GET", "/admin/image-suggest/stock", token=seller_a, expect=403)
lst = call("GET", f"/admin/image-suggest/stock?q={T}", token=admin)
assert {i["id"] for i in lst["items"]} == {beer["id"], tiger["id"], coffee["id"]}
ok("admin builds a stock library (title, keywords in any language, brand); files processed like uploads")

# ---- shops ------------------------------------------------------------------------------------
def shop(tok, tag):
    s = call("POST", "/shops", {"slug": f"img-{tag}-{RUN}", "name": f"{tag.upper()} Drinks {RUN}", "currency": "LAK"}, tok)
    call("PATCH", f"/admin/shops/{s['id']}", {"auto_approve": True}, admin)
    return s["id"]


sa = shop(seller_a, "a")
raw, ct = multipart({"alt": f"Beerlao Dark {T}"}, [("dark.png", png(700, 700, (40, 20, 10)))])
a_asset = call("POST", f"/shops/{sa}/media/upload", token=seller_a, raw=raw, ctype=ct)["assets"][0]
a_prod = call("POST", f"/shops/{sa}/products", {"sku": f"bld{RUN}", "name": f"Beerlao Dark 330ml {T}", "price_cents": 1500000, "status": "active",
                                               "initial_stock": 20, "category": "handmade", "media_ids": [a_asset["id"]]}, seller_a)
seller_b = account(f"img-b-{RUN}@t.dev", "Seller B")
sb = shop(seller_b, "b")
sug = lambda tok, sid, q, **kw: call("GET", f"/shops/{sid}/image-suggest?" + urllib.parse.urlencode({"q": q, "limit": 40, **kw}), token=tok)["items"]
mine = lambda items: [i for i in items if T in i["title"]]

r = mine(sug(seller_b, sb, "beer lao", limit=40))
assert r and r[0]["id"] == beer["id"] and r[0]["source"] == "stock", r
assert all(i["id"] != coffee["id"] for i in r), "Lao coffee is not beer lao"
assert all(i["source"] != "shared" for i in mine(r)), "A hasn't opted in to sharing yet"
assert beer["id"] in [i["id"] for i in sug(seller_b, sb, "ເບຍລາວ")], "Lao spelling"
assert beer["id"] in [i["id"] for i in sug(seller_b, sb, "เบียร์ลาว")], "Thai spelling via synonym"
assert beer["id"] in [i["id"] for i in sug(seller_b, sb, "beerl")], "while typing"
assert [i["id"] for i in mine(sug(seller_b, sb, f"tiger {T}"))][:1] == [tiger["id"]]
assert sug(seller_b, sb, "b") == []
call("GET", f"/shops/{sb}/image-suggest?q=beer", token=seller_a, expect=403)
ok("typing 'beer lao', 'ເບຍລາວ', 'เบียร์ลาว' or just 'beerl' suggests the Beerlao stock photo first; unrelated 'Lao coffee' is left out")

# ---- sharing opt-in ---------------------------------------------------------------------------
assert call("GET", f"/shops/{sa}/image-suggest/sharing", token=seller_a)["share"] is False
call("PUT", f"/shops/{sa}/image-suggest/sharing", {"share": True}, seller_a)
r = sug(seller_b, sb, f"beerlao dark {T}")
shared = [i for i in r if i["source"] == "shared"]
assert shared and shared[0]["id"] == a_asset["id"] and shared[0]["subtitle"].startswith("A Drinks"), r
assert all(i["source"] != "shared" for i in mine(sug(seller_a, sa, f"beerlao dark {T}"))), "own photos are 'own', not 'shared'"
assert any(i["source"] == "own" and i["id"] == a_asset["id"] for i in sug(seller_a, sa, f"beerlao dark {T}"))
ok("shops that opt in share their live product photos (credited with the shop name); the owner sees them as their own")

# ---- picking ----------------------------------------------------------------------------------
p1 = call("POST", f"/shops/{sb}/image-suggest/pick", {"source": "stock", "id": beer["id"], "query": "beer lao"}, seller_b)
assert p1["shop_id"] == sb and p1["url"] != beer["url"] and p1["alt"].startswith("Beerlao Original") and p1["variants"], p1
assert fetch(p1["url"]) == 200 and all(fetch(v["url"]) == 200 for v in p1["variants"])
again = call("POST", f"/shops/{sb}/image-suggest/pick", {"source": "stock", "id": beer["id"]}, seller_b)
assert again["id"] == p1["id"], "second pick returns the same copy"
call("POST", f"/shops/{sb}/image-suggest/pick", {"source": "nope", "id": beer["id"]}, seller_b, expect=400)
e = call("POST", f"/shops/{sb}/image-suggest/pick", {"source": "nope", "id": beer["id"]}, seller_b, expect=400, headers={"Accept-Language": "lo"})
assert e["message"].startswith("ແຫຼ່ງທີ່ມາຕ້ອງເປັນ"), e
call("POST", f"/shops/{sb}/image-suggest/pick", {"source": "own", "id": a_asset["id"]}, seller_b, expect=403)
p2 = call("POST", f"/shops/{sb}/image-suggest/pick", {"source": "shared", "id": a_asset["id"]}, seller_b)
assert p2["shop_id"] == sb and p2["url"] != a_asset["url"]
prod = call("POST", f"/shops/{sb}/products", {"sku": f"blb{RUN}", "name": f"Beerlao 640ml {T}", "price_cents": 2000000, "status": "active",
                                             "initial_stock": 5, "category": "handmade", "media_ids": [p1["id"], p2["id"]]}, seller_b)
gal = call("GET", f"/products/{prod['id']}/media", token=seller_b)
assert [g["id"] for g in gal] == [p1["id"], p2["id"]]
assert next(i for i in call("GET", f"/admin/image-suggest/stock?q={T}", token=admin)["items"] if i["id"] == beer["id"])["use_count"] == 1
ok("picking copies the photo into the seller's library (renditions too) — the same photo twice gives the same copy; it goes straight into the gallery")

# copies are independent of their sources
call("POST", f"/shops/{sb}/media/delete", {"ids": [p2["id"]], "force": True}, seller_b)
assert fetch(a_asset["url"]) == 200, "deleting the copy leaves the original"
call("DELETE", f"/admin/image-suggest/stock/{beer['id']}", token=admin)
assert fetch(beer["url"]) == 404 and fetch(p1["url"]) == 200, "deleting the stock photo leaves shops' copies"
r = sug(seller_b, sb, f"beer lao {T}")
assert any(i["source"] == "own" and i["id"] == p1["id"] for i in r), "the copy is now in B's own library"
call("PUT", f"/shops/{sb}/image-suggest/sharing", {"share": True}, seller_b)
seller_c = account(f"img-c-{RUN}@t.dev", "Seller C")
sc = shop(seller_c, "c")
assert all(i["id"] != p1["id"] for i in sug(seller_c, sc, f"beer lao {T}")), "copies aren't re-shared as someone else's photo"
assert call("GET", f"/shops/{sa}/image-suggest/sharing", token=seller_a)["picked_by_others"] == 1
ok("copies are independent (delete either side safely); copied photos are never re-shared under another shop's name")

# ---- synonyms + promote -----------------------------------------------------------------------
call("POST", "/admin/image-suggest/synonyms", {"terms": ["only-one"]}, admin, expect=400)
nk = stock(admin, f"Namkhong whisky {T}", color=(150, 90, 30))
assert all(i["id"] != nk["id"] for i in sug(seller_c, sc, "ນ້ຳຂອງ"))
syn = call("POST", "/admin/image-suggest/synonyms", {"terms": ["Nam Khong", f"ນ້ຳຂອງ{T}"]}, admin)
assert syn["terms"] == ["namkhong", f"ນ້ຳຂອງ{T}"], syn
assert nk["id"] in [i["id"] for i in sug(seller_c, sc, f"ນ້ຳຂອງ{T}")], "found through the new synonym"
call("DELETE", f"/admin/image-suggest/synonyms/{syn['id']}", token=admin)
assert nk["id"] not in [i["id"] for i in sug(seller_c, sc, f"ນ້ຳຂອງ{T}")]
cand = call("GET", "/admin/image-suggest/candidates?q=" + urllib.parse.quote(f"beerlao dark {T}"), token=admin)["items"]
assert [c["asset_id"] for c in cand] == [a_asset["id"]], cand
pr = call("POST", "/admin/image-suggest/stock/promote", {"asset_id": a_asset["id"], "title": f"Beerlao Dark {T}", "keywords": "ເບຍລາວດຳ", "brand": "Beerlao"}, admin)
assert pr["source_asset_id"] == a_asset["id"] and pr["url"] != a_asset["url"] and fetch(pr["url"]) == 200
assert call("GET", "/admin/image-suggest/candidates?q=" + urllib.parse.quote(f"beerlao dark {T}"), token=admin)["items"] == []
upd = call("PATCH", f"/admin/image-suggest/stock/{pr['id']}", {"keywords": "ເບຍລາວດຳ, black lager", "active": False}, admin)
assert upd["active"] is False
assert pr["id"] not in [i["id"] for i in sug(seller_c, sc, f"black lager {T}")], "inactive stock is hidden"
call("PATCH", f"/admin/image-suggest/stock/{pr['id']}", {"active": True}, admin)
assert pr["id"] in [i["id"] for i in sug(seller_c, sc, "black lager")], "edited keywords are searchable"
ok("admins add synonym groups (any language) and promote a shop's photo into the stock library; edits re-index")

print("ALL IMAGE SUGGEST TESTS PASSED ✔")
