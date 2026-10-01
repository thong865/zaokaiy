#!/usr/bin/env python3
"""cod_risk module: sellers report refused COD parcels, admins decide the customer's risk level,
checkout refuses COD for blocked customers, and every step is on a verifiable hash chain.

Start the API with ADMIN_EMAILS=admin@demo.dev. For the anchoring part also run
`python3 scripts/mock_anchor.py` and start the API with
  COD_RISK_ANCHOR=webhook COD_RISK_ANCHOR_URL=http://127.0.0.1:9997/anchor COD_RISK_ANCHOR_SECRET=anchorsecret
Optional: PGURL=postgres://… to also check that the database refuses edits to the ledger.
Usage: python3 scripts/cod_risk_test.py [http://localhost:8080] [http://127.0.0.1:9997]
"""
import hashlib, json, os, random, subprocess, sys, urllib.error, urllib.parse, urllib.request, uuid

BASE = sys.argv[1] if len(sys.argv) > 1 else "http://localhost:8080"
GATEWAY = sys.argv[2] if len(sys.argv) > 2 else "http://127.0.0.1:9997"
API = BASE + "/api"
RUN = uuid.uuid4().hex[:6]


def call(method, path, body=None, token=None, expect=200, headers=None, base=None):
    r = urllib.request.Request((base or API) + path, data=json.dumps(body).encode() if body is not None else None, method=method)
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
print("cod_risk module tests")

mods = call("GET", "/modules")
assert mods["cod_risk"]["enabled"], mods
anchoring = mods["cod_risk"]["anchoring"]

admin = account("admin@demo.dev", "Admin")
buyer = account(f"risk-buyer-{RUN}@t.dev", "Buyer")
digits = "".join(random.choice("0123456789") for _ in range(8))
PHONE_LOCAL = f"020 {digits[:4]} {digits[4:]}"        # how the customer typed it
PHONE_INTL = f"+856 20 {digits}"                      # same person, other format
E164 = f"+85620{digits}"


def seller_shop(tag):
    tok = account(f"risk-{tag}-{RUN}@t.dev", f"Seller {tag}")
    shop = call("POST", "/shops", {"slug": f"risk-{tag}-{RUN}", "name": f"Shop {tag} {RUN}", "currency": "LAK"}, tok)
    call("PATCH", f"/admin/shops/{shop['id']}", {"auto_approve": True}, admin)
    call("PUT", f"/shops/{shop['id']}/shipping/hal", {"enabled": True, "fee_payer": "buyer", "fee_cents": 2000000, "cod_enabled": True}, tok)
    prod = call("POST", f"/shops/{shop['id']}/products", {"sku": f"r{tag}{RUN}", "name": f"Lamp {tag}", "price_cents": 10000000, "status": "active", "initial_stock": 50, "category": "handmade", "social_code": "R01"}, tok)
    return tok, shop["id"], prod


sa, sid_a, prod_a = seller_shop("a")
sb, sid_b, prod_b = seller_shop("b")


def cod_order(prod, sid, phone, method="cod"):
    addr = {"name": "Khamla", "phone": phone, "address": "Ban Sisangvone, Vientiane"}
    return call("POST", "/orders/checkout", {"items": [{"product_id": prod["id"], "qty": 1}], "shipping_address": addr,
                                            "delivery": [{"shop_id": sid, "carrier_code": "hal", "payment_method": method}]}, buyer)


def refuse(tok, sid, o, tracking):
    call("POST", f"/shops/{sid}/orders/{o['id']}/status", {"status": "shipped", "tracking_no": tracking}, tok)
    call("POST", f"/shops/{sid}/cod/update", {"items": [{"kind": "order", "id": o["id"]}], "status": "returned"}, tok)


# ---- before shipping: seller sees the customer's risk ----------------------------------------
a1 = cod_order(prod_a, sid_a, PHONE_LOCAL)[0]
a2 = cod_order(prod_a, sid_a, PHONE_LOCAL)[0]
a3 = cod_order(prod_a, sid_a, PHONE_LOCAL)[0]
un = call("GET", f"/shops/{sid_a}/cod-risk/unshipped", token=sa)["rows"]
row = next(r for r in un if r["id"] == a1["id"])
assert row["risk"] == {"level": "none", "confirmed_reports": 0, "shops": 0}, row
call("GET", f"/shops/{sid_a}/cod-risk/unshipped", token=sb, expect=403)
ok("unshipped COD orders list the customer's risk (none yet); other shops can't see them")

# ---- refused parcels → reports -------------------------------------------------------------
refuse(sa, sid_a, a1, "HAL-R1")
refuse(sa, sid_a, a2, "HAL-R2")
ref = call("GET", f"/shops/{sid_a}/cod-risk/refused", token=sa)
r1 = next(r for r in ref["rows"] if r["id"] == a1["id"])
assert r1["reportable"] and r1["report"] is None and r1["tracking_no"] == "HAL-R1" and ref["window_days"] >= 1, r1
assert all(r["id"] != a3["id"] for r in ref["rows"]), "only returned parcels"

rep = lambda tok, sid, o, reason="refused", note="", expect=200, **kw: call(
    "POST", f"/shops/{sid}/cod-risk/reports", {"kind": "order", "order_id": o["id"], "reason": reason, "note": note}, tok, expect=expect, **kw)
rep(sa, sid_a, a1, reason="angry", expect=400)
rep(sa, sid_a, a1, reason="other", note="", expect=400)
e = rep(sa, sid_a, a3, expect=400, headers={"Accept-Language": "lo"})            # not returned
assert e["message"] == "ລາຍງານໄດ້ສະເພາະພັດສະດຸເກັບເງິນປາຍທາງທີ່ຖືກຕີກັບເທົ່ານັ້ນ", e
rep(sb, sid_b, a1, expect=403)                                                   # not their order
ra = rep(sa, sid_a, a1, note="Customer answered the courier and said they don't want it")
assert ra["status"] == "pending" and ra["phone"] == E164 and ra["block_height"] >= 1 and len(ra["customer_key"]) == 64, ra
rep(sa, sid_a, a1, expect=409)
rw = rep(sa, sid_a, a2, reason="unreachable", note="phone off for 3 days")
w = call("POST", f"/cod-risk/reports/{rw['id']}/withdraw", token=sa)
assert w["status"] == "withdrawn"
call("POST", f"/cod-risk/reports/{rw['id']}/withdraw", token=sa, expect=400)
call("POST", f"/cod-risk/reports/{ra['id']}/withdraw", token=sb, expect=403)
mine = call("GET", f"/shops/{sid_a}/cod-risk/reports", token=sa)
assert {r["id"] for r in mine} >= {ra["id"], rw["id"]}
ok("seller reports a refused parcel (validated, one per order, Lao errors); can withdraw while pending")

# Same person refuses at another shop, typing the number differently.
b1 = cod_order(prod_b, sid_b, PHONE_INTL)[0]
refuse(sb, sid_b, b1, "HAL-B1")
rb = rep(sb, sid_b, b1, reason="fake_address")
assert rb["customer_key"] == ra["customer_key"], "same customer across shops and phone formats"
ok("the same customer is recognised across shops and phone formats (keyed hash of E.164)")

# ---- admin review ---------------------------------------------------------------------------
call("GET", "/admin/cod-risk/reports", token=sa, expect=403)
q = call("GET", "/admin/cod-risk/reports?status=pending", token=admin)
mine = [r for r in q["items"] if r["customer_key"] == ra["customer_key"]]
assert {r["id"] for r in mine} == {ra["id"], rb["id"]} and mine[0]["customer_reports"] == 3, mine
byphone = call("GET", "/admin/cod-risk/reports?status=all&q=" + urllib.parse.quote(PHONE_INTL), token=admin)["items"]
assert len(byphone) == 3, byphone
dec = lambda rid, expect=200, **b: call("POST", f"/admin/cod-risk/reports/{rid}/decision", b, admin, expect=expect)
dec(ra["id"], action="approve", expect=400)
dec(ra["id"], action="dismiss", expect=400)                       # dismiss needs a note
dec(rw["id"], action="confirm", expect=400)                       # withdrawn
d = dec(ra["id"], action="confirm")
assert d["stats"]["confirmed"] == 1 and d["suggested_level"] == "low" and d["customer"]["effective_level"] == "none", d
dec(ra["id"], action="confirm", expect=400)
dec(rb["id"], action="confirm", level="high", expect=400)         # a level needs a note
d = dec(rb["id"], action="confirm", note="Fake address confirmed by HAL", level="high", expires_days=180)
assert d["stats"]["confirmed"] == 2 and d["stats"]["shops"] == 2 and d["suggested_level"] == "high", d["stats"]
assert d["customer"]["effective_level"] == "high" and d["customer"]["level_expires_at"], d["customer"]
ok("admin confirms reports; suggested level follows confirmed reports across shops; level set with expiry")

# ---- sellers see the level, checkout refuses COD for blocked customers ----------------------
chk = call("POST", f"/shops/{sid_b}/cod-risk/check", {"phones": [PHONE_LOCAL, "nonsense", "+66 81 234 5678"]}, sb)["results"]
assert chk[0]["valid"] and chk[0]["level"] == "high" and chk[0]["confirmed_reports"] == 2 and chk[0]["shops"] == 2, chk
assert not chk[1]["valid"] and chk[2]["level"] == "none"
assert "note" not in json.dumps(chk) and "Shop a" not in json.dumps(chk), "no other shops' details"
call("POST", f"/shops/{sid_b}/cod-risk/check", {"phones": []}, sb, expect=400)
un = call("GET", f"/shops/{sid_a}/cod-risk/unshipped", token=sa)["rows"]
assert next(r for r in un if r["id"] == a3["id"])["risk"]["level"] == "high"
cod_order(prod_b, sid_b, PHONE_INTL)                                 # high < blocked: COD still allowed
key = ra["customer_key"]
call("POST", f"/admin/cod-risk/customers/{key}/level", {"level": "blocked"}, admin, expect=400)       # note
call("POST", f"/admin/cod-risk/customers/{key}/level", {"level": "blocked", "note": "x", "expires_days": 0}, admin, expect=400)
call("POST", f"/admin/cod-risk/customers/{key}/level", {"level": "blocked", "note": "Three refusals"}, admin)
e = call("POST", "/orders/checkout", {"items": [{"product_id": prod_b["id"], "qty": 1}], "shipping_address": {"name": "K", "phone": f"856 20 {digits}", "address": "Vientiane"},
                                      "delivery": [{"shop_id": sid_b, "carrier_code": "hal", "payment_method": "cod"}]}, buyer, expect=400, headers={"Accept-Language": "lo"})
assert e["message"].startswith("ເບີໂທນີ້ບໍ່ສາມາດໃຊ້ການເກັບເງິນປາຍທາງໄດ້"), e
cod_order(prod_b, sid_b, PHONE_LOCAL, method="prepaid")              # paying online still works
# comment/chat orders are guarded too
call("POST", f"/shops/{sid_b}/social/sessions", {"title": "Live"}, sb)
c = call("POST", f"/shops/{sid_b}/social/simulate", {"user_name": "Khamla", "message": "CF R01"}, sb)
tok = c["checkout_url"].rsplit("/", 1)[1]
call("POST", f"/public/social-orders/{tok}/confirm", {"name": "Khamla", "phone": PHONE_LOCAL, "address": "Ban Sisangvone", "carrier_code": "hal", "cod": True}, expect=400)
call("POST", f"/public/social-orders/{tok}/confirm", {"name": "Khamla", "phone": PHONE_LOCAL, "address": "Ban Sisangvone", "carrier_code": "hal"})
ok("sellers look up a phone (level + counts only); blocked customers can't choose COD (cart + comment checkout), prepaid works")

# appeal upheld: reverse a decision (needs a note), then lift the level
dec(rb["id"], action="dismiss", expect=400)
d = dec(rb["id"], action="dismiss", note="Customer showed HAL mis-delivered to the wrong village")
assert d["stats"]["confirmed"] == 1 and d["suggested_level"] == "low"
call("POST", f"/admin/cod-risk/customers/{key}/level", {"level": "none"}, admin)
cod_order(prod_b, sid_b, PHONE_LOCAL)
cust = call("GET", f"/admin/cod-risk/customers/{key}", token=admin)
events = [b["event"] for b in cust["ledger"]]
assert events == ["report.filed", "report.filed", "report.withdrawn", "report.filed", "report.confirmed", "report.confirmed", "level.set",
                  "level.set", "report.dismissed", "level.set"], events
assert all("phone" not in json.dumps(b["payload"]) and digits not in json.dumps(b["payload"]) for b in cust["ledger"]), "no PII on the ledger"
flagged = call("GET", "/admin/cod-risk/customers?level=all&q=" + urllib.parse.quote(PHONE_LOCAL), token=admin)
assert [c["customer_key"] for c in flagged] == [key]
ok("decisions can be reversed on appeal (with a note); every step is a ledger event without personal data")

# ---- ledger ---------------------------------------------------------------------------------
cs = call("GET", "/admin/cod-risk/chain", token=admin)
assert cs["valid"] and cs["height"] >= len(events) and cs["broken"] is None, cs
ov = call("GET", "/admin/cod-risk/overview", token=admin)
assert ov["chain"]["height"] == cs["height"]
ok(f"hash chain verifies ({cs['height']} blocks)")

pg = os.environ.get("PGURL")
if pg:
    psql = lambda sql: subprocess.run(["psql", pg, "-v", "ON_ERROR_STOP=1", "-qAtc", sql], capture_output=True, text=True)
    r = psql("UPDATE cod_risk_blocks SET payload = '{}' WHERE height = 1")
    assert r.returncode != 0 and "append-only" in r.stderr, r
    r = psql("DELETE FROM cod_risk_blocks WHERE height = 1")
    assert r.returncode != 0 and "append-only" in r.stderr, r
    # Someone with superuser access bypasses the trigger: verification still catches it.
    h = cs["height"]
    orig = psql(f"SELECT payload FROM cod_risk_blocks WHERE height = {h}").stdout.strip()
    psql("ALTER TABLE cod_risk_blocks DISABLE TRIGGER cod_risk_blocks_append_only")
    psql(f"UPDATE cod_risk_blocks SET payload = replace(payload, '\"level\":\"none\"', '\"level\":\"low\"') WHERE height = {h}")
    bad = call("GET", "/admin/cod-risk/chain", token=admin)
    psql(f"UPDATE cod_risk_blocks SET payload = $${orig}$$ WHERE height = {h}")
    psql("ALTER TABLE cod_risk_blocks ENABLE TRIGGER cod_risk_blocks_append_only")
    assert not bad["valid"] and bad["broken"]["height"] == h and bad["broken"]["reason"] == "block content was changed", bad
    assert call("GET", "/admin/cod-risk/chain", token=admin)["valid"]
    ok("the database refuses edits/deletes; a forced edit is detected at the exact block")


def leaf(h):
    return hashlib.sha256(b"\x00" + bytes.fromhex(h)).digest()


def walk(h, proof):
    cur = leaf(h)
    for s in proof:
        sib = bytes.fromhex(s["sibling"])
        cur = hashlib.sha256(b"\x01" + (sib + cur if s["side"] == "left" else cur + sib)).digest()
    return cur.hex()


if anchoring:
    a = call("POST", "/admin/cod-risk/chain/anchor", token=admin)["anchor"]
    assert a["status"] == "anchored" and a["tx_ref"].startswith("0x") and a["to_height"] == cs["height"], a
    got = call("GET", "/", base=GATEWAY)
    onchain = next(x for x in got if x["tx"] == a["tx_ref"])
    assert onchain["merkle_root"] == a["merkle_root"] and onchain["ledger"] == "zaokaiy-cod-risk"
    p = call("GET", f"/admin/cod-risk/proof/{ra['block_height']}", token=admin)
    assert p["hash_ok"] and p["root_ok"] and p["anchor"]["tx_ref"] == a["tx_ref"]
    # Independent check, only from the block, the proof and the root published on the chain:
    b = p["block"]
    assert hashlib.sha256(f"{b['height']}|{b['prev_hash']}|{b['ts_micros']}|{b['event']}|{b['payload']}".encode()).hexdigest() == b["hash"]
    assert walk(b["hash"], p["proof"]) == onchain["merkle_root"]
    assert call("POST", "/admin/cod-risk/chain/anchor", token=admin)["anchor"] is None      # nothing new
    # Gateway down: the batch is kept and retried later.
    call("POST", "/_fail", {"on": True}, base=GATEWAY)
    call("POST", f"/admin/cod-risk/customers/{key}/level", {"level": "low", "note": "watch"}, admin)
    f = call("POST", "/admin/cod-risk/chain/anchor", token=admin)["anchor"]
    assert f["status"] == "failed" and "503" in f["error"], f
    assert call("GET", "/admin/cod-risk/chain", token=admin)["unanchored"] == 1
    call("POST", "/_fail", {"on": False}, base=GATEWAY)
    a2 = call("POST", "/admin/cod-risk/chain/anchor", token=admin)["anchor"]
    assert a2["status"] == "anchored" and a2["from_height"] == a["to_height"] + 1
    ok("batches anchored to the chain gateway (signed); Merkle proof verifies against the on-chain root; outages retried")
else:
    print("  – anchoring not configured: skipped (see the docstring)")

call("GET", f"/admin/cod-risk/customers/{key}", token=sa, expect=403)
print("ALL COD RISK TESTS PASSED ✔")
