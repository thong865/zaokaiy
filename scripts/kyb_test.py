#!/usr/bin/env python3
"""Corporate KYC (business verification): profile, people, encrypted documents, submission,
admin review, publish gate, audit trail, privacy.
Start the API with ADMIN_EMAILS=admin@demo.dev. Usage: python3 scripts/kyb_test.py [http://localhost:8080] [private_dir]
"""
import json, os, struct, sys, urllib.error, urllib.request, uuid, zlib

BASE = sys.argv[1] if len(sys.argv) > 1 else "http://localhost:8080"
PRIVATE_DIR = sys.argv[2] if len(sys.argv) > 2 else os.path.join(os.path.dirname(__file__), "..", "backend", "private")
API = BASE + "/api"
RUN = uuid.uuid4().hex[:6]
LO = {"Accept-Language": "lo"}


def raw(method, path, data=None, token=None, headers=None):
    r = urllib.request.Request(API + path, data=data, method=method)
    for k, v in (headers or {}).items():
        r.add_header(k, v)
    if token:
        r.add_header("Authorization", f"Bearer {token}")
    try:
        with urllib.request.urlopen(r) as resp:
            return resp.status, resp.read(), {k.lower(): v for k, v in resp.headers.items()}
    except urllib.error.HTTPError as e:
        return e.code, e.read(), {k.lower(): v for k, v in e.headers.items()}


def call(method, path, body=None, token=None, expect=200, headers=None):
    code, payload, _ = raw(method, path, json.dumps(body).encode() if body is not None else None, token,
                           {"Content-Type": "application/json", **(headers or {})})
    out = json.loads(payload or b"null")
    assert code == expect, f"{method} {path} -> {code} (expected {expect}): {out}"
    return out


def upload(shop_id, kind, data, token, name="doc.pdf", expires_on=None, expect=200, headers=None):
    b = uuid.uuid4().hex
    parts = [f'--{b}\r\nContent-Disposition: form-data; name="kind"\r\n\r\n{kind}\r\n'.encode()]
    if expires_on:
        parts.append(f'--{b}\r\nContent-Disposition: form-data; name="expires_on"\r\n\r\n{expires_on}\r\n'.encode())
    parts.append(f'--{b}\r\nContent-Disposition: form-data; name="file"; filename="{name}"\r\nContent-Type: application/octet-stream\r\n\r\n'.encode() + data + b"\r\n")
    parts.append(f"--{b}--\r\n".encode())
    code, payload, _ = raw("POST", f"/shops/{shop_id}/kyb/documents", b"".join(parts), token,
                           {"Content-Type": f"multipart/form-data; boundary={b}", **(headers or {})})
    out = json.loads(payload or b"null")
    assert code == expect, f"upload {kind} -> {code} (expected {expect}): {out}"
    return out


def png(w=40, h=30):
    rows = b"".join(b"\0" + bytes([200, 60, 60] * w) for _ in range(h))
    chunk = lambda t, d: struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d) & 0xffffffff)
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(rows)) + chunk(b"IEND", b"")


PDF = b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n1 0 obj<</Type/Catalog>>endobj\ntrailer<</Root 1 0 R>>\n%%EOF\n"


def account(email, name):
    try:
        return call("POST", "/auth/login", {"email": email, "password": "password123"})["token"]
    except AssertionError:
        return call("POST", "/auth/register", {"email": email, "password": "password123", "display_name": name})["token"]


ok = lambda m: print("  ✔", m)
print("corporate KYC tests")
admin = account("admin@demo.dev", "Admin")
seller = account(f"kyb-{RUN}@t.dev", "Seller")
other = account(f"kyb2-{RUN}@t.dev", "Other")
cats = {c["slug"]: c["id"] for c in call("GET", "/categories")}

# --- shop type
call("POST", "/shops", {"slug": f"x-{RUN}", "name": "x", "entity_type": "corp"}, seller, expect=400)
shop = call("POST", "/shops", {"slug": f"biz-{RUN}", "name": f"Biz {RUN}", "entity_type": "business", "currency": "LAK"}, seller)
sid = shop["id"]
assert shop["entity_type"] == "business" and shop["kyb_status"] == "none" and shop["kyb_verified_at"] is None
ind = call("POST", "/shops", {"slug": f"ind-{RUN}", "name": f"Ind {RUN}"}, seller)
assert ind["entity_type"] == "individual"
e = call("PUT", f"/shops/{ind['id']}/kyb", {"legal_name": "X"}, seller, expect=400, headers=LO)
assert e["message"] == "ກະລຸນາຕັ້ງປະເພດຮ້ານເປັນນິຕິບຸກຄົນກ່ອນ", e
ok("shop type individual / business; KYB only for business shops")

# --- publish gate for unverified business shops
prod = call("POST", f"/shops/{sid}/products", {"sku": "P1", "name": "Rice", "price_cents": 100000, "status": "draft",
             "initial_stock": 3, "category_id": cats["food-local"]}, seller)
e = call("PATCH", f"/products/{prod['id']}", {"status": "active"}, seller, expect=400, headers=LO)
assert e["message"] == "ກະລຸນາຢືນຢັນທຸລະກິດຂອງທ່ານກ່ອນເຜີຍແຜ່ສິນຄ້າ", e
call("POST", f"/shops/{sid}/products", {"sku": "P2", "name": "Tea", "price_cents": 1, "status": "active", "category_id": cats["food-local"]}, seller, expect=400)
p2 = call("POST", f"/shops/{ind['id']}/products", {"sku": "P2", "name": "Tea", "price_cents": 1, "status": "active", "category_id": cats["food-local"]}, seller)
assert p2["review_status"] in ("pending", "approved")
ok("unverified business shops can keep drafts but not publish; individual shops unaffected")

# --- empty state
v = call("GET", f"/shops/{sid}/kyb", token=seller)
assert v["profile"] is None and v["missing"] == ["company"] and v["editable"] and "limited_company" in v["options"]["company_types"]
call("GET", f"/shops/{sid}/kyb", token=other, expect=403)
call("POST", f"/shops/{sid}/kyb/submit", token=seller, expect=400)

# --- save profile: validation
base = {"company_type": "limited_company", "legal_name": "Biz Trading Co., Ltd.", "legal_name_local": "ບໍລິສັດ ບິສ ເທຣດດິ້ງ ຈຳກັດ",
        "registration_no": f"01-{RUN} ", "registration_date": "2019-05-01", "tax_id": "123456789-000", "country": "la",
        "province": "Vientiane Capital", "registered_address": "Ban Phonthan, Saysettha", "business_activity": "Retail of food products",
        "website": "https://biz.la", "contact_email": "Info@Biz.la", "contact_phone": "+856 21 123 456",
        "bank_name": "BCEL", "bank_account_name": "Biz Trading Co., Ltd.", "bank_account_number": "010-12-00-12345678",
        "persons": [{"roles": ["representative", "director"], "full_name": "Somphone Vongsa", "title": "Managing Director",
                     "nationality": "LA", "date_of_birth": "1985-03-02", "id_type": "national_id", "id_number": "01-2345678"},
                    {"roles": ["ubo"], "full_name": "Khamla Inthavong", "nationality": "TH", "id_type": "passport",
                     "id_number": "AA1234567", "ownership_bps": 6000, "is_pep": True}]}
for patch, msg in [({"company_type": "llc"}, "ບໍ່ຮູ້ຈັກປະເພດວິສາຫະກິດນີ້"),
                   ({"country": "LAO"}, "ປະເທດຕ້ອງເປັນລະຫັດ 2 ຕົວອັກສອນ ເຊັ່ນ LA ຫຼື TH"),
                   ({"website": "biz.la"}, "ເວັບໄຊຕ້ອງເລີ່ມດ້ວຍ https://"),
                   ({"registration_date": "2999-01-01"}, "ວັນທີຈົດທະບຽນຕ້ອງບໍ່ເປັນວັນໃນອະນາຄົດ"),
                   ({"bank_account_number": "12"}, "ເລກບັດ ແລະ ເລກບັນຊີຕ້ອງເປັນຕົວອັກສອນ ຫຼື ຕົວເລກ 4-40 ຕົວ"),
                   ({"persons": [{"roles": [], "full_name": "A"}]}, "ແຕ່ລະຄົນຕ້ອງມີບົດບາດ: ຜູ້ຕາງໜ້າ, ຜູ້ອຳນວຍການ ຫຼື ເຈົ້າຂອງຜູ້ໄດ້ຮັບຜົນປະໂຫຍດ"),
                   ({"persons": [{"roles": ["ubo"], "full_name": "A", "ownership_bps": 12000}]}, "ສັດສ່ວນຮຸ້ນຕ້ອງຢູ່ລະຫວ່າງ 0 ຫາ 100%")]:
    e = call("PUT", f"/shops/{sid}/kyb", {**base, **patch}, seller, expect=400, headers=LO)
    assert e["message"] == msg, (patch, e)
v = call("PUT", f"/shops/{sid}/kyb", base, seller)
p = v["profile"]
assert p["registration_no"] == f"01-{RUN}".upper() and p["country"] == "LA" and p["contact_email"] == "info@biz.la"
assert p["bank_account_last4"] == "5678" and p["bank_account_number"] is None, "owner view is masked"
assert [x["id_last4"] for x in v["persons"]] == ["5678", "4567"] and all(x["id_number"] is None and x["has_id_number"] for x in v["persons"])
assert v["shop"]["kyb_status"] == "draft"
assert v["missing"] == ["doc:registration_certificate", "doc:tax_certificate", "doc:representative_id", "doc:shareholder_register"], v["missing"]
ok("profile + people saved; ID and bank numbers masked for the owner; checklist lists missing documents")

# --- numbers encrypted at rest (not in the database or API in clear)
blob = json.dumps(call("GET", f"/shops/{sid}/kyb", token=seller))
assert "010-12-00-12345678" not in blob and "AA1234567" not in blob and "01-2345678" not in blob
# re-saving without numbers keeps them
keep = json.loads(json.dumps(base))
keep.pop("bank_account_number")
for i, pp in enumerate(keep["persons"]):
    pp.pop("id_number")
    pp["id"] = v["persons"][i]["id"]
v = call("PUT", f"/shops/{sid}/kyb", keep, seller)
assert v["profile"]["bank_account_last4"] == "5678" and all(x["has_id_number"] for x in v["persons"])
ok("stored numbers survive a re-save that omits them")

# --- documents
e = upload(sid, "registration_certificate", b"MZ\x90\0\x03\0\0\0" * 20, seller, name="x.exe", expect=400, headers=LO)
assert e["message"] == "ເອກະສານຕ້ອງເປັນໄຟລ໌ JPG, PNG, WebP ຫຼື PDF", e
upload(sid, "registration_certificate", b"<html><script>alert(1)</script></html>", seller, name="x.html", expect=400)
upload(sid, "passport_selfie", PDF, seller, expect=400)
upload(sid, "tax_certificate", PDF, seller, expires_on="2020-01-01", expect=400)
upload(sid, "tax_certificate", b"%PDF-1.4\n" + b"0" * (10 * 1048576 + 10), seller, expect=400)
upload(sid, "tax_certificate", PDF, other, expect=403)
reg = upload(sid, "registration_certificate", PDF, seller, name="ERC.pdf")
tax = upload(sid, "tax_certificate", png(), seller, name="tax.png", expires_on="2030-12-31")
rid = upload(sid, "representative_id", png(60, 40), seller, name="id-card.png")
assert reg["content_type"] == "application/pdf" and tax["content_type"] == "image/png" and tax["expires_on"] == "2030-12-31"
# file served decrypted to owner/admin only, never cached
code, body, h = raw("GET", f"/kyb/documents/{reg['id']}/file", token=seller)
assert code == 200 and body == PDF and h["content-type"] == "application/pdf" and "no-store" in h["cache-control"], h
assert h.get("x-content-type-options") == "nosniff" and "sandbox" in h.get("content-security-policy", "")
assert raw("GET", f"/kyb/documents/{reg['id']}/file", token=other)[0] == 403
assert raw("GET", f"/kyb/documents/{reg['id']}/file")[0] == 401
code, body, _ = raw("GET", f"/kyb/documents/{tax['id']}/file", token=admin)
assert code == 200 and body == png()
# at rest: encrypted, outside the public media directory
if os.path.isdir(PRIVATE_DIR):
    files = [os.path.join(dp, f) for dp, _, fs in os.walk(os.path.join(PRIVATE_DIR, "kyb", sid)) for f in fs]
    assert len(files) == 3, files
    for f in files:
        d = open(f, "rb").read()
        assert d[:2] == b"k1" and b"%PDF" not in d and b"PNG" not in d, "documents are encrypted at rest"
    ok("documents encrypted at rest in the private directory")
try:
    urllib.request.urlopen(f"{BASE}/media/kyb/{sid}")
    raise AssertionError("KYC files must not be reachable under /media")
except urllib.error.HTTPError as e:
    assert e.code == 404
tmp = upload(sid, "other", PDF, seller, name="old.pdf")
call("DELETE", f"/kyb/documents/{tmp['id']}", token=other, expect=403)
call("DELETE", f"/kyb/documents/{tmp['id']}", token=seller)
assert raw("GET", f"/kyb/documents/{tmp['id']}/file", token=seller)[0] == 404
ok("uploads checked by content (JPG/PNG/WebP/PDF ≤ 10 MB), private, decrypted only for owner/admin")

v = call("GET", f"/shops/{sid}/kyb", token=seller)
assert v["missing"] == ["doc:shareholder_register"], v["missing"]
e = call("POST", f"/shops/{sid}/kyb/submit", token=seller, expect=400, headers=LO)
assert e["message"] == "ຂໍ້ມູນ ຫຼື ເອກະສານທີ່ຈຳເປັນຍັງບໍ່ຄົບ"
upload(sid, "shareholder_register", PDF, seller, name="shareholders.pdf")

# --- submit → locked
v = call("POST", f"/shops/{sid}/kyb/submit", token=seller)
assert v["shop"]["kyb_status"] == "submitted" and not v["editable"] and v["missing"] == []
assert set(v["profile"]["risk_flags"]) == {"pep", "foreign_person"}, v["profile"]["risk_flags"]
call("PUT", f"/shops/{sid}/kyb", base, seller, expect=409)
upload(sid, "other", PDF, seller, expect=409)
call("POST", f"/shops/{sid}/kyb/submit", token=seller, expect=409)
e = call("PATCH", f"/shops/{sid}", {"entity_type": "individual"}, seller, expect=400, headers=LO)
assert e["message"] == "ຮ້ານນິຕິບຸກຄົນທີ່ຢືນຢັນແລ້ວ ຫຼື ກຳລັງກວດ ບໍ່ສາມາດປ່ຽນເປັນບຸກຄົນໄດ້"
ok("submission locks editing; risk flags (PEP, foreign person) computed; can't dodge by switching to individual")

# --- admin queue + detail (full numbers, audited)
call("GET", "/admin/kyb", token=seller, expect=403)
q = call("GET", "/admin/kyb?status=submitted", token=admin)
row = next(x for x in q["items"] if x["shop_id"] == sid)
assert row["legal_name"] == "Biz Trading Co., Ltd." and row["documents"] == 4 and "pep" in row["risk_flags"]
assert q["counts"]["submitted"] >= 1
d = call("GET", f"/admin/kyb/{sid}", token=admin)
assert d["profile"]["bank_account_number"] == "010-12-00-12345678"
assert sorted(x["id_number"] for x in d["persons"]) == ["01-2345678", "AA1234567"]
assert d["owner"]["email"] == f"kyb-{RUN}@t.dev"
acts = [x["action"] for x in d["events"]]
assert "viewed" in acts and "document_viewed" in acts and "submitted" in acts
assert "viewed" not in [x["action"] for x in call("GET", f"/shops/{sid}/kyb", token=seller)["events"]], "admin views are hidden from the seller"
assert call("GET", "/admin/overview", token=admin)["kyb_pending"] >= 1
ok("admin queue + detail with decrypted numbers; every admin view written to the audit log")

# --- request changes with a rejected document
call("POST", f"/admin/kyb/{sid}/decision", {"action": "request_changes"}, admin, expect=400)
call("POST", f"/admin/kyb/{sid}/decision", {"action": "approve", "documents": [{"id": rid["id"], "status": "rejected"}]}, admin, expect=400)
call("POST", f"/admin/kyb/{sid}/decision", {"action": "maybe"}, admin, expect=400)
d = call("POST", f"/admin/kyb/{sid}/decision", {"action": "request_changes", "note": "ID card photo is blurry",
          "documents": [{"id": rid["id"], "status": "rejected", "note": "Blurry — photograph both sides in daylight"}, {"id": reg["id"], "status": "accepted"}]}, admin)
assert d["shop"]["kyb_status"] == "changes_requested"
v = call("GET", f"/shops/{sid}/kyb", token=seller)
assert v["editable"] and v["profile"]["review_note"] == "ID card photo is blurry"
assert v["missing"] == ["doc:representative_id"]
docs = {x["id"]: x for x in v["documents"]}
assert docs[rid["id"]]["status"] == "rejected" and docs[rid["id"]]["review_note"].startswith("Blurry") and docs[reg["id"]]["status"] == "accepted"
call("DELETE", f"/kyb/documents/{reg['id']}", token=seller, expect=400)
v = call("PUT", f"/shops/{sid}/kyb", keep, seller)
assert v["shop"]["kyb_status"] == "changes_requested", "saving keeps the reviewer's note visible"
upload(sid, "representative_id", png(80, 50), seller, name="id-card-v2.png")
v = call("POST", f"/shops/{sid}/kyb/submit", token=seller)
assert v["shop"]["kyb_status"] == "submitted"
ok("request changes: per-document rejection with reasons, seller fixes and resubmits")

# --- approve → verified badge, products can go live
d = call("POST", f"/admin/kyb/{sid}/decision", {"action": "approve", "note": "All good"}, admin)
assert d["shop"]["kyb_status"] == "approved" and d["shop"]["kyb_verified_at"] and d["profile"]["review_due_at"]
assert all(x["status"] in ("accepted", "rejected") for x in d["documents"])
call("POST", f"/admin/kyb/{sid}/decision", {"action": "approve"}, admin, expect=400)
call("PATCH", f"/admin/shops/{sid}", {"auto_approve": True}, admin)
pr = call("PATCH", f"/products/{prod['id']}", {"status": "active"}, seller)
assert pr["review_status"] == "approved"
sf = call("GET", f"/storefront/biz-{RUN}")
assert sf["shop"]["kyb_verified_at"] and sf["shop"]["entity_type"] == "business"
assert call("GET", f"/catalog/products/{prod['id']}")["product"]["shop_verified"] is True
assert call("GET", f"/catalog/products/{p2['id']}")["product"]["shop_verified"] is False if p2["review_status"] == "approved" else True
ok("approval verifies the shop (badge data public), sets the review date and unlocks publishing")

# --- update while verified: stays verified; rejected update keeps verification
v = call("PUT", f"/shops/{sid}/kyb", {**keep, "registered_address": "New office, Dongdok"}, seller)
assert v["shop"]["kyb_status"] == "draft" and v["shop"]["kyb_verified_at"]
call("POST", f"/shops/{sid}/kyb/submit", token=seller)
d = call("POST", f"/admin/kyb/{sid}/decision", {"action": "reject", "note": "Address proof missing"}, admin)
assert d["shop"]["kyb_status"] == "approved" and d["shop"]["kyb_verified_at"], "a rejected update keeps the earlier verification"
ok("verified shops stay verified while an update is reviewed")

# --- duplicate registration flagged for another owner
dup = call("POST", "/shops", {"slug": f"dup-{RUN}", "name": "Dup", "entity_type": "business"}, other)
call("PUT", f"/shops/{dup['id']}/kyb", {**base, "persons": [{"roles": ["representative", "director"], "full_name": "X Y", "id_number": "99-8887776"},
     {"roles": ["ubo"], "full_name": "X Y", "id_number": "99-8887776", "ownership_bps": 10000}]}, other)
for k in ["registration_certificate", "tax_certificate", "representative_id", "shareholder_register"]:
    upload(dup["id"], k, PDF, other)
v = call("POST", f"/shops/{dup['id']}/kyb/submit", token=other)
assert "duplicate_registration" in v["profile"]["risk_flags"], v["profile"]["risk_flags"]
d = call("POST", f"/admin/kyb/{dup['id']}/decision", {"action": "reject", "note": "Company already registered by another account"}, admin)
assert d["shop"]["kyb_status"] == "rejected" and d["shop"]["kyb_verified_at"] is None
ok("same registration number under another account is flagged; rejection")

# --- revoke: verification removed, products off sale
call("POST", f"/admin/kyb/{dup['id']}/decision", {"action": "revoke", "note": "x"}, admin, expect=400)
call("POST", f"/admin/kyb/{sid}/decision", {"action": "revoke"}, admin, expect=400)
d = call("POST", f"/admin/kyb/{sid}/decision", {"action": "revoke", "note": "Licence withdrawn"}, admin)
assert d["shop"]["kyb_status"] == "revoked" and d["shop"]["kyb_verified_at"] is None
call("GET", f"/catalog/products/{prod['id']}", expect=404)
e = call("PATCH", f"/products/{prod['id']}", {"status": "active", "name": "Rice 2"}, seller, expect=400)
acts = [x["action"] for x in call("GET", f"/shops/{sid}/kyb", token=seller)["events"]]
assert acts[0] == "revoked" and "approved" in acts and "changes_requested" in acts
ok("revoke removes verification and takes products off sale; full history for the seller")

print("ALL KYB TESTS PASSED ✔")
