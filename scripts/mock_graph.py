#!/usr/bin/env python3
"""Tiny stand-in for the Meta Graph API and Google OAuth, used by social_test.py and oauth_test.py.

Start the API with META_GRAPH_URL=http://127.0.0.1:9998 (and, for social login,
GOOGLE_AUTH_URL=http://127.0.0.1:9998/google/auth GOOGLE_TOKEN_URL=http://127.0.0.1:9998/google/token
GOOGLE_USERINFO_URL=http://127.0.0.1:9998/google/userinfo FACEBOOK_DIALOG_URL=http://127.0.0.1:9998/fb/dialog)
and run:  python3 scripts/mock_graph.py

GET /_log returns every POST received; DELETE /_log clears it.
Fake users: add &mock_user=<name> to the provider login URL (Google e-mail <name>@gmail.test,
Facebook e-mail <name>@fb.test; mock_email=<addr> overrides it)."""
import base64, hashlib, hmac, json
from http.server import BaseHTTPRequestHandler, HTTPServer
from urllib.parse import parse_qs, urlencode, urlparse

LOG = []
CODES = {}  # auth code -> {"user", "email", "challenge"}
FB_APP_SECRET = "fbsecret"
GOOGLE_SECRET = "gsecret"


def q1(qs, k, d=""):
    return qs.get(k, [d])[0]


class H(BaseHTTPRequestHandler):
    def _send(self, code, obj, headers=None):
        data = json.dumps(obj).encode()
        self.send_response(code)
        self.send_header("content-type", "application/json")
        for k, v in (headers or {}).items():
            self.send_header(k, v)
        self.end_headers()
        self.wfile.write(data)

    def _redirect(self, url):
        self.send_response(302)
        self.send_header("location", url)
        self.end_headers()

    def do_GET(self):
        u = urlparse(self.path)
        qs = parse_qs(u.query)
        if u.path == "/_log":
            return self._send(200, LOG)
        # --- Google authorize / Facebook login dialog: approve immediately ---
        if u.path in ("/google/auth", "/fb/dialog"):
            g = u.path.startswith("/google")
            user = q1(qs, "mock_user", "alice")
            if q1(qs, "mock_deny"):
                return self._redirect(q1(qs, "redirect_uri") + "?" + urlencode({"error": "access_denied", "state": q1(qs, "state")}))
            code = ("g_" if g else "f_") + user + "_" + str(len(CODES))
            CODES[code] = {
                "user": user,
                "email": q1(qs, "mock_email") or f"{user}@{'gmail' if g else 'fb'}.test",
                "challenge": q1(qs, "code_challenge"),
                "redirect_uri": q1(qs, "redirect_uri"),
            }
            return self._redirect(q1(qs, "redirect_uri") + "?" + urlencode({"code": code, "state": q1(qs, "state")}))
        if u.path == "/google/userinfo":
            tok = self.headers.get("authorization", "").removeprefix("Bearer ")
            c = CODES.get(tok.removeprefix("gat_"))
            if not c:
                return self._send(401, {"error": "invalid_token"})
            return self._send(200, {"sub": "g-" + c["user"], "email": c["email"], "email_verified": True,
                                    "name": c["user"].title() + " G", "picture": "https://img.test/" + c["user"]})
        # --- Facebook Graph ---
        if u.path == "/oauth/access_token":
            c = CODES.get(q1(qs, "code"))
            if not c or q1(qs, "client_secret") != FB_APP_SECRET or q1(qs, "redirect_uri") != c["redirect_uri"]:
                return self._send(400, {"error": {"message": "invalid code"}})
            return self._send(200, {"access_token": "fat_" + q1(qs, "code"), "token_type": "bearer"})
        if u.path == "/me":
            tok = q1(qs, "access_token")
            proof = hmac.new(FB_APP_SECRET.encode(), tok.encode(), hashlib.sha256).hexdigest()
            c = CODES.get(tok.removeprefix("fat_"))
            if not c or q1(qs, "appsecret_proof") != proof:
                return self._send(400, {"error": {"message": "bad token or appsecret_proof"}})
            return self._send(200, {"id": "fb-" + c["user"], "name": c["user"].title() + " F", "email": c["email"],
                                    "picture": {"data": {"url": "https://img.test/fb/" + c["user"]}}})
        self._send(200, {})

    def do_DELETE(self):
        LOG.clear()
        self._send(200, {"ok": True})

    def do_POST(self):
        raw = self.rfile.read(int(self.headers.get("content-length", 0))) or b""
        u = urlparse(self.path)
        if u.path == "/google/token":
            f = parse_qs(raw.decode())
            c = CODES.get(q1(f, "code"))
            verifier = q1(f, "code_verifier")
            challenge = base64.urlsafe_b64encode(hashlib.sha256(verifier.encode()).digest()).rstrip(b"=").decode()
            if not c or q1(f, "client_secret") != GOOGLE_SECRET or challenge != c["challenge"] or q1(f, "redirect_uri") != c["redirect_uri"]:
                return self._send(400, {"error": "invalid_grant"})
            return self._send(200, {"access_token": "gat_" + q1(f, "code"), "token_type": "Bearer", "expires_in": 3599})
        body = json.loads(raw or b"{}")
        LOG.append({"path": self.path, "auth": self.headers.get("authorization", ""), "body": body})
        if body.get("to") == "8560000000000":  # simulate a WhatsApp delivery failure
            return self._send(400, {"error": {"message": "Recipient phone number not in allowed list"}})
        self._send(200, {"id": "mock_%d" % len(LOG)})

    def log_message(self, *a):
        pass


if __name__ == "__main__":
    HTTPServer(("127.0.0.1", 9998), H).serve_forever()
