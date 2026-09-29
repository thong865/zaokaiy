#!/usr/bin/env python3
"""Media library tests (stdlib only). Run after the API is up:  python3 scripts/media_test.py"""
import json, struct, sys, urllib.request, urllib.error, uuid, zlib

BASE = (sys.argv[1] if len(sys.argv) > 1 else "http://localhost:8080")
API = BASE + "/api"
RUN = uuid.uuid4().hex[:6]


def png(w, h, rgba=False):
    """Minimal valid PNG (solid colour) without Pillow."""
    ch = 4 if rgba else 3
    px = bytes([255, 77, 46, 128][:ch])
    raw = b"".join(b"\x00" + px * w for _ in range(h))
    def chunk(t, d):
        return struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d) & 0xFFFFFFFF)
    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 6 if rgba else 2, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b""))


def call(method, path, body=None, token=None, expect=200, raw=None, ctype="application/json"):
    data = raw if raw is not None else (json.dumps(body).encode() if body is not None else None)
    r = urllib.request.Request(API + path, data=data, method=method)
    r.add_header("Content-Type", ctype)
    if token:
        r.add_header("Authorization", f"Bearer {token}")
    try:
        with urllib.request.urlopen(r) as resp:
            code, out = resp.status, json.loads(resp.read() or b"null")
    except urllib.error.HTTPError as e:
        code, out = e.code, json.loads(e.read() or b"null")
    assert code == expect, f"{method} {path} -> {code} (expected {expect}): {out}"
    return out


def upload(shop_id, files, token, product_id=None, expect=200):
    b = uuid.uuid4().hex
    parts = []
    for name, data in files:
        parts.append(f'--{b}\r\nContent-Disposition: form-data; name="file"; filename="{name}"\r\n'
                     f"Content-Type: application/octet-stream\r\n\r\n".encode() + data + b"\r\n")
    body = b"".join(parts) + f"--{b}--\r\n".encode()
    q = f"?product_id={product_id}" if product_id else ""
    return call("POST", f"/shops/{shop_id}/media/upload{q}", token=token, raw=body,
                ctype=f"multipart/form-data; boundary={b}", expect=expect)


def main():
    tok = call("POST", "/auth/register", {"email": f"m-{RUN}@t.dev", "password": "password123", "display_name": "M"})["token"]
    other = call("POST", "/auth/register", {"email": f"o-{RUN}@t.dev", "password": "password123", "display_name": "O"})["token"]
    shop = call("POST", "/shops", {"slug": f"media-{RUN}", "name": "Media Shop"}, tok)
    oshop = call("POST", "/shops", {"slug": f"other-{RUN}", "name": "Other"}, other)
    # Trusted shop: skip the moderation queue so gallery edits stay live (needs ADMIN_EMAILS=admin@demo.dev)
    try:
        admin = call("POST", "/auth/login", {"email": "admin@demo.dev", "password": "password123"})["token"]
    except AssertionError:
        admin = call("POST", "/auth/register", {"email": "admin@demo.dev", "password": "password123", "display_name": "Admin"})["token"]
    call("PATCH", f"/admin/shops/{shop['id']}", {"auto_approve": True}, admin)
    p = call("POST", f"/shops/{shop['id']}/products", {"sku": "M1", "name": "Media Product", "price_cents": 1000, "status": "active", "initial_stock": 3, "category": "handmade"}, tok)
    assert p["review_status"] == "approved", p

    # Upload: big opaque PNG -> resized JPEG, alpha PNG stays PNG, junk rejected per-file
    cfg = call("GET", "/media/config")
    assert cfg["min_image_edge"] == 500 and cfg["variant_widths"] == [320, 640, 960, 1280, 1920], cfg
    r = upload(shop["id"], [("hero_shot.png", png(3000, 1000)), ("logo.png", png(600, 600, True)), ("notes.txt", b"hello world"),
                            ("tiny.png", png(300, 400))], tok, product_id=p["id"])
    assert len(r["assets"]) == 2 and len(r["errors"]) == 2, r
    assert any("too small (300×400 px)" in e["message"] for e in r["errors"]), r["errors"]
    hero, logo = r["assets"]
    assert hero["mime"] == "image/jpeg" and hero["width"] == 2000 and hero["height"] == 667, hero
    assert hero["alt"] == "hero shot" and hero["thumb_url"], hero
    assert logo["mime"] == "image/png", logo
    # Responsive WebP renditions (never upscaled), placeholder + colour for lazy loading
    assert [v["w"] for v in hero["variants"]] == [1920, 1280, 960, 640, 320], hero["variants"]
    assert hero["variants"][3]["h"] == 213
    assert [v["w"] for v in logo["variants"]] == [600, 320], logo["variants"]
    assert hero["placeholder"].startswith("data:image/jpeg;base64,") and hero["dominant_color"] == "#ff4d2e", hero["dominant_color"]
    with urllib.request.urlopen(hero["variants"][3]["url"]) as f:
        body = f.read()
        assert f.headers["Content-Type"].startswith("image/webp") and "immutable" in f.headers["Cache-Control"]
        assert body[8:12] == b"WEBP" and len(body) < 20_000, len(body)
    # File is served
    with urllib.request.urlopen(hero["url"]) as f:
        assert f.headers["Content-Type"].startswith("image/jpeg") and "immutable" in f.headers["Cache-Control"]
    with urllib.request.urlopen(hero["thumb_url"]) as f:
        f.read()

    # Gallery attached in upload order, catalog cache synced
    g = call("GET", f"/products/{p['id']}/media", token=tok)
    assert [x["id"] for x in g] == [hero["id"], logo["id"]]
    prod = call("GET", f"/products/{p['id']}", token=tok)
    assert prod["images"] == [hero["url"], logo["url"]]
    assert prod["cover"]["url"] == hero["url"] and len(prod["cover"]["variants"]) == 5 and prod["cover"]["placeholder"], prod["cover"]
    card = next(c for c in call("GET", f"/catalog/products?q=Media%20Product") if c["id"] == p["id"])
    assert card["cover"]["variants"][0]["w"] == 1920, card

    # External URL asset + reorder (logo becomes cover), then detach hero
    ext = call("POST", f"/shops/{shop['id']}/media/url", {"url": "https://cdn.example.com/clip.mp4"}, tok)
    assert ext["kind"] == "video"
    g = call("PUT", f"/products/{p['id']}/media", {"asset_ids": [logo["id"], ext["id"], hero["id"]]}, tok)
    assert [x["position"] for x in g] == [0, 1, 2] and g[0]["id"] == logo["id"]
    detail = call("GET", f"/catalog/products/{p['id']}")
    assert [m["kind"] for m in detail["media"]] == ["image", "video", "image"]
    assert detail["media"][2]["variants"][0]["w"] == 1920 and detail["media"][0]["color"]
    assert detail["product"]["cover"]["url"] == logo["url"]
    assert detail["product"]["images"] == [logo["url"], hero["url"]]

    # Security: other shops can't read, attach or delete
    call("GET", f"/media/{hero['id']}", token=other, expect=403)
    call("PUT", f"/products/{p['id']}/media", {"asset_ids": [hero["id"]]}, other, expect=403)
    oprod = call("POST", f"/shops/{oshop['id']}/products", {"sku": "X", "name": "X", "price_cents": 1}, other)
    call("PUT", f"/products/{oprod['id']}/media", {"asset_ids": [hero["id"]]}, other, expect=400)

    # Library listing & filters
    lib = call("GET", f"/shops/{shop['id']}/media", token=tok)
    assert lib["stats"]["count"] == 3 and lib["stats"]["videos"] == 1
    assert call("GET", f"/shops/{shop['id']}/media?kind=video", token=tok)["items"][0]["id"] == ext["id"]
    call("PATCH", f"/media/{hero['id']}", {"alt": "Front view"}, tok)
    assert call("GET", f"/media/{hero['id']}", token=tok)["products"][0]["name"] == "Media Product"

    # Delete: blocked while in use, force removes everywhere
    call("DELETE", f"/media/{hero['id']}", token=tok, expect=409)
    call("DELETE", f"/media/{hero['id']}?force=true", token=tok)
    assert call("GET", f"/products/{p['id']}", token=tok)["images"] == [logo["url"]]
    for gone in [hero["url"], hero["variants"][0]["url"], hero["variants"][4]["url"]]:
        try:
            urllib.request.urlopen(gone)
            raise AssertionError(f"file should be gone: {gone}")
        except urllib.error.HTTPError as e:
            assert e.code == 404

    # Bulk delete unused
    call("PUT", f"/products/{p['id']}/media", {"asset_ids": []}, tok)
    out = call("POST", f"/shops/{shop['id']}/media/delete", {"ids": [logo["id"], ext["id"]]}, tok)
    assert out["deleted"] == 2
    empty = call("GET", f"/products/{p['id']}", token=tok)
    assert empty["images"] == [] and empty["cover"] is None

    # Gallery limit
    many = upload(shop["id"], [(f"i{i}.png", png(500, 500)) for i in range(16)], tok)["assets"]
    call("PUT", f"/products/{p['id']}/media", {"asset_ids": [a["id"] for a in many]}, tok, expect=400)
    # Renditions backfill for older uploads (admin only)
    call("POST", "/admin/media/backfill", token=tok, expect=403)
    bf = call("POST", "/admin/media/backfill?limit=5", token=admin)
    assert bf["failed"] == 0 and "remaining" in bf, bf
    print("ALL MEDIA TESTS PASSED ✔")


if __name__ == "__main__":
    main()
