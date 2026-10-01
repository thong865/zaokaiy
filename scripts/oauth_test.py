#!/usr/bin/env python3
"""Social login tests: Google, Facebook (OAuth) and WhatsApp (one-time code).

Run scripts/mock_graph.py, then start the API with:
  META_GRAPH_URL=http://127.0.0.1:9998 PUBLIC_WEB_URL=http://localhost:3000 PUBLIC_API_URL=http://localhost:8080/api \\
  GOOGLE_CLIENT_ID=gid GOOGLE_CLIENT_SECRET=gsecret FACEBOOK_APP_ID=fbid FACEBOOK_APP_SECRET=fbsecret \\
  GOOGLE_AUTH_URL=http://127.0.0.1:9998/google/auth GOOGLE_TOKEN_URL=http://127.0.0.1:9998/google/token \\
  GOOGLE_USERINFO_URL=http://127.0.0.1:9998/google/userinfo FACEBOOK_DIALOG_URL=http://127.0.0.1:9998/fb/dialog \\
  WHATSAPP_TOKEN=watok WHATSAPP_PHONE_NUMBER_ID=12345 OTP_DEV_ECHO=true OTP_RESEND_SECONDS=2
Usage: python3 scripts/oauth_test.py [http://localhost:8080]
"""
import json, random, sys, time, urllib.error, urllib.request, uuid
from urllib.parse import parse_qs, urlparse

BASE = sys.argv[1] if len(sys.argv) > 1 else "http://localhost:8080"
API = BASE + "/api"
GRAPH = "http://127.0.0.1:9998"
WEB = "http://localhost:3000"
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


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *a, **k):
        return None


opener = urllib.request.build_opener(NoRedirect)


def location(url):
    """GET without following redirects; return the Location header."""
    try:
        opener.open(url)
    except urllib.error.HTTPError as e:
        assert e.code in (302, 303, 307), f"{url} -> {e.code}"
        return e.headers["location"]
    raise AssertionError(f"no redirect from {url}")


def oauth(provider, user, token=None, link=False, redirect="/dashboard", extra="", email=None):
    """Full browser round trip. Returns the web callback query dict."""
    start = call("POST", f"/auth/oauth/{provider}/start", {"redirect": redirect, "link": link}, token=token)
    url = start["url"] + f"&mock_user={user}" + (f"&mock_email={email}" if email else "") + extra
    to_api = location(url)
    assert to_api.startswith(f"{API}/auth/oauth/{provider}/callback?"), to_api
    to_web = location(to_api)
    assert to_web.startswith(WEB + "/auth/callback?"), to_web
    return {k: v[0] for k, v in parse_qs(urlparse(to_web).query).items()}


def login_via(provider, user, **kw):
    q = oauth(provider, user, **kw)
    assert "code" in q, q
    return call("POST", "/auth/exchange", {"code": q["code"]})


def graph(method="GET"):
    r = urllib.request.Request(GRAPH + "/_log", method=method)
    with urllib.request.urlopen(r) as resp:
        return json.loads(resp.read())


def phone():
    return "+85620" + "".join(random.choice("0123456789") for _ in range(8))


ok = lambda m: print("  ✔", m)
print("social login tests")

p = call("GET", "/auth/providers")
assert p == {"password": True, "google": True, "facebook": True, "whatsapp": True, "captcha_site_key": None}, p
ok("providers endpoint lists enabled methods")

# ---- Google -------------------------------------------------------------------------------
g1 = login_via("google", f"gnew{RUN}")
assert g1["is_new"] and g1["redirect"] == "/dashboard" and g1["user"]["email"] == f"gnew{RUN}@gmail.test", g1
assert g1["user"]["display_name"] == f"gnew{RUN}".title() + " G" and g1["user"]["avatar_url"]
me = call("GET", "/auth/me", token=g1["token"])
assert me["id"] == g1["user"]["id"]
ok("Google: new account created, PKCE verified, token works")

g2 = login_via("google", f"gnew{RUN}")
assert not g2["is_new"] and g2["user"]["id"] == g1["user"]["id"]
ok("Google: returning user signs into the same account")

pw_email = f"pw{RUN}@gmail.test"
pw = call("POST", "/auth/register", {"email": pw_email, "password": "password123", "display_name": "Pw User"})
g3 = login_via("google", f"pw{RUN}")  # same verified e-mail
assert not g3["is_new"] and g3["user"]["id"] == pw["user"]["id"]
ok("Google: verified e-mail joins the existing password account")

# ---- Facebook -----------------------------------------------------------------------------
f1 = login_via("facebook", f"fnew{RUN}")
assert f1["is_new"] and f1["user"]["email"] is None and f1["user"]["display_name"] == f"fnew{RUN}".title() + " F"
ok("Facebook: new account (appsecret_proof checked; FB e-mail not trusted as account e-mail)")

q = oauth("facebook", f"clash{RUN}", email=pw_email)
assert q.get("error") == "email_exists" and q.get("provider") == "facebook", q
ok("Facebook: e-mail of an existing account → email_exists (no silent takeover)")

lk = login_via("facebook", f"link{RUN}", token=pw["token"], link=True, redirect="/account")
assert lk["linked"] == "facebook" and lk["user"]["id"] == pw["user"]["id"] and lk["redirect"] == "/account"
again = login_via("facebook", f"link{RUN}")
assert again["user"]["id"] == pw["user"]["id"] and not again["is_new"]
ok("Facebook: connect to signed-in account, then sign in with it")

q = oauth("facebook", f"link{RUN}", token=g1["token"], link=True)
assert q.get("error") == "already_linked", q
q = oauth("google", f"other{RUN}", token=pw["token"], link=True)
assert q.get("error") == "already_linked", q  # pw account already has a Google identity
ok("linking an identity owned by someone else / a second Google account → already_linked")

ids = call("GET", "/auth/identities", token=pw["token"])
assert ids["has_password"] and sorted(i["provider"] for i in ids["identities"]) == ["facebook", "google"], ids
ok("identities lists connected methods")

# ---- OAuth failure modes -----------------------------------------------------------------
q = {k: v[0] for k, v in parse_qs(urlparse(location(f"{API}/auth/oauth/google/callback?code=x&state=nope")).query).items()}
assert q["error"] == "expired", q
q = oauth("google", "denier", extra="&mock_deny=1")
assert q["error"] == "cancelled", q
q = oauth("google", f"reuse{RUN}")
call("POST", "/auth/exchange", {"code": q["code"]})
call("POST", "/auth/exchange", {"code": q["code"]}, expect=400)
r = login_via("google", f"redir{RUN}", redirect="//evil.example")
assert r["redirect"] == "/", r
call("POST", "/auth/oauth/github/start", {}, expect=404)
call("POST", "/auth/oauth/google/start", {"link": True}, expect=401)
ok("bad state, cancelled consent, reused code, open redirect, unknown provider, link without session")

# ---- WhatsApp ----------------------------------------------------------------------------
call("POST", "/auth/whatsapp/send", {"phone": "020 5555 1234"}, expect=400)
graph("DELETE")
ph = phone()
s = call("POST", "/auth/whatsapp/send", {"phone": ph.replace("+856", "+856 "), "lang": "lo"})
assert s["phone"] == ph and len(s["dev_code"]) == 6 and s["resend_in"] > 0
msg = graph()[-1]
assert msg["path"] == "/12345/messages" and msg["auth"] == "Bearer watok", msg
assert msg["body"]["to"] == ph[1:] and msg["body"]["template"]["name"] == "zaokaiy_login"
assert msg["body"]["template"]["components"][0]["parameters"][0]["text"] == s["dev_code"]
ok("WhatsApp: authentication template sent with the code")

e = call("POST", "/auth/whatsapp/send", {"phone": ph}, expect=400)
assert "wait" in e["message"], e
e = call("POST", "/auth/whatsapp/send", {"phone": ph}, expect=400, headers={"Accept-Language": "lo"})
assert "ລໍຖ້າ" in e["message"], e
ok("WhatsApp: resend cooldown (and Lao error text)")

call("POST", "/auth/whatsapp/verify", {"phone": ph, "code": "000000" if s["dev_code"] != "000000" else "111111"}, expect=400)
w = call("POST", "/auth/whatsapp/verify", {"phone": ph, "code": s["dev_code"], "display_name": "Nok"})
assert w["is_new"] and w["user"]["phone"] == ph and w["user"]["display_name"] == "Nok" and w["user"]["email"] is None
call("POST", "/auth/whatsapp/verify", {"phone": ph, "code": s["dev_code"]}, expect=400)  # single use
ok("WhatsApp: wrong code rejected, right code creates account, code is single-use")

ph2 = phone()
s2 = call("POST", "/auth/whatsapp/send", {"phone": ph2})
for _ in range(5):
    call("POST", "/auth/whatsapp/verify", {"phone": ph2, "code": "999999" if s2["dev_code"] != "999999" else "888888"}, expect=400)
e = call("POST", "/auth/whatsapp/verify", {"phone": ph2, "code": s2["dev_code"]}, expect=400)
assert "too many" in e["message"], e
ok("WhatsApp: locked after 5 wrong attempts")

call("POST", "/auth/whatsapp/send", {"phone": "+8560000000000"}, expect=502)
ok("WhatsApp: delivery failure reported (502) and not counted as sent")

ph3 = phone()
s3 = call("POST", "/auth/whatsapp/send", {"phone": ph3})
lw = call("POST", "/auth/whatsapp/verify", {"phone": ph3, "code": s3["dev_code"], "link": True}, token=g1["token"])
assert lw["linked"] == "whatsapp" and lw["user"]["id"] == g1["user"]["id"] and lw["user"]["phone"] == ph3
time.sleep(2.2)  # API runs with OTP_RESEND_SECONDS=2
s3b = call("POST", "/auth/whatsapp/send", {"phone": ph}, expect=200)  # ph belongs to Nok
e = call("POST", "/auth/whatsapp/verify", {"phone": ph, "code": s3b["dev_code"], "link": True}, token=g1["token"], expect=409)
ok("WhatsApp: connect a number to the signed-in account; someone else's number → 409")

# ---- Password + unlink --------------------------------------------------------------------
e = call("DELETE", "/auth/identities/facebook", token=f1["token"], expect=400)
assert "password" in e["message"]
call("POST", "/auth/password", {"password": "newpassword1"}, token=f1["token"], expect=400)  # no e-mail/phone
call("POST", "/auth/password", {"password": "newpassword1"}, token=w["token"])
call("POST", "/auth/login", {"email": ph, "password": "newpassword1"})
call("POST", "/auth/password", {"password": "another123", "current": "wrong"}, token=w["token"], expect=400)
call("DELETE", "/auth/identities/whatsapp", token=w["token"], expect=400)  # password would be unusable without the phone
call("DELETE", "/auth/identities/whatsapp", token=g1["token"])  # g1 still has Google
me = call("GET", "/auth/me", token=g1["token"])
assert me["phone"] is None
call("DELETE", "/auth/identities/whatsapp", token=g1["token"], expect=404)
call("DELETE", "/auth/identities/facebook", token=pw["token"])
ok("set password, sign in with phone + password, disconnect, guard against removing the last method")

call("POST", "/auth/login", {"email": pw_email, "password": "password123"})
ok("password login still works for linked accounts")

print("ALL SOCIAL LOGIN TESTS PASSED ✔")
