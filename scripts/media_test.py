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
    r = upload(shop["id"], [("hero_shot.png", png(3000, 1000)), ("logo.png", png(64, 64, True)), ("notes.txt", b"hello world")], tok, product_id=p["id"])
    assert len(r["assets"]) == 2 and len(r["errors"]) == 1, r
    hero, logo = r["assets"]
    assert hero["mime"] == "image/jpeg" and hero["width"] == 2000 and hero["height"] == 667, hero
    assert hero["alt"] == "hero shot" and hero["thumb_url"], hero
    assert logo["mime"] == "image/png", logo
    # File is served
    with urllib.request.urlopen(hero["url"]) as f:
        assert f.headers["Content-Type"].startswith("image/jpeg") and "immutable" in f.headers["Cache-Control"]
    with urllib.request.urlopen(hero["thumb_url"]) as f:
        f.read()

    # Gallery attached in upload order, catalog cache synced
    g = call("GET", f"/products/{p['id']}/media", token=tok)
    assert [x["id"] for x in g] == [hero["id"], logo["id"]]
    assert call("GET", f"/products/{p['id']}", token=tok)["images"] == [hero["url"], logo["url"]]

    # External URL asset + reorder (logo becomes cover), then detach hero
    ext = call("POST", f"/shops/{shop['id']}/media/url", {"url": "https://cdn.example.com/clip.mp4"}, tok)
    assert ext["kind"] == "video"
    g = call("PUT", f"/products/{p['id']}/media", {"asset_ids": [logo["id"], ext["id"], hero["id"]]}, tok)
    assert [x["position"] for x in g] == [0, 1, 2] and g[0]["id"] == logo["id"]
    detail = call("GET", f"/catalog/products/{p['id']}")
    assert [m["kind"] for m in detail["media"]] == ["image", "video", "image"]
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
    try:
        urllib.request.urlopen(hero["url"])
        raise AssertionError("file should be gone")
    except urllib.error.HTTPError as e:
        assert e.code == 404

    # Bulk delete unused
    call("PUT", f"/products/{p['id']}/media", {"asset_ids": []}, tok)
    out = call("POST", f"/shops/{shop['id']}/media/delete", {"ids": [logo["id"], ext["id"]]}, tok)
    assert out["deleted"] == 2
    assert call("GET", f"/products/{p['id']}", token=tok)["images"] == []

    # Gallery limit
    many = upload(shop["id"], [(f"i{i}.png", png(8, 8)) for i in range(16)], tok)["assets"]
    call("PUT", f"/products/{p['id']}/media", {"asset_ids": [a["id"] for a in many]}, tok, expect=400)
    print("ALL MEDIA TESTS PASSED ✔")


if __name__ == "__main__":
    main()
