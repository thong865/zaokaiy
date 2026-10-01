#!/usr/bin/env python3
"""Fake blockchain gateway for the cod_risk module (COD_RISK_ANCHOR=webhook).

Checks the X-Signature HMAC, "mines" the Merkle root into a fake transaction and answers
{"tx": "0x…"}. GET / lists what it received; POST /_fail {"on": true} simulates an outage.
A real gateway does the same thing but submits the root to a chain — e.g. MBlock
(system.remark / a small anchoring pallet) or an EVM contract — and returns the tx hash.

Usage: COD_RISK_ANCHOR_SECRET=anchorsecret python3 scripts/mock_anchor.py [port=9997]
"""
import hashlib, hmac, json, os, sys
from http.server import BaseHTTPRequestHandler, HTTPServer

SECRET = os.environ.get("COD_RISK_ANCHOR_SECRET", "anchorsecret").encode()
PORT = int(sys.argv[1]) if len(sys.argv) > 1 else 9997
received = []
state = {"fail": False}


class H(BaseHTTPRequestHandler):
    def log_message(self, *a):
        pass

    def reply(self, code, obj):
        body = json.dumps(obj).encode()
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        self.reply(200, received)

    def do_POST(self):
        raw = self.rfile.read(int(self.headers.get("Content-Length", 0)))
        if self.path == "/_fail":  # test switch: simulate the chain node being down
            state["fail"] = json.loads(raw or b"{}").get("on", True)
            return self.reply(200, state)
        want = "sha256=" + hmac.new(SECRET, raw, hashlib.sha256).hexdigest()
        if not hmac.compare_digest(want, self.headers.get("X-Signature", "")):
            return self.reply(401, {"error": "bad signature"})
        body = json.loads(raw)
        if state["fail"]:
            return self.reply(503, {"error": "node unavailable"})
        tx = "0x" + hashlib.sha256(("tx:" + body["merkle_root"]).encode()).hexdigest()
        received.append({**body, "tx": tx})
        self.reply(200, {"tx": tx})


print(f"mock anchor gateway on :{PORT}")
HTTPServer(("127.0.0.1", PORT), H).serve_forever()
