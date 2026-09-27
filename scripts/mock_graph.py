#!/usr/bin/env python3
"""Tiny stand-in for the Meta Graph API, used by social_test.py.
Start the API with META_GRAPH_URL=http://127.0.0.1:9998 and run:  python3 scripts/mock_graph.py
GET /_log returns every request received; DELETE /_log clears it."""
import json
from http.server import BaseHTTPRequestHandler, HTTPServer

LOG = []


class H(BaseHTTPRequestHandler):
    def _send(self, code, obj):
        data = json.dumps(obj).encode()
        self.send_response(code)
        self.send_header("content-type", "application/json")
        self.end_headers()
        self.wfile.write(data)

    def do_GET(self):
        self._send(200, LOG if self.path == "/_log" else {})

    def do_DELETE(self):
        LOG.clear()
        self._send(200, {"ok": True})

    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers.get("content-length", 0))) or b"{}")
        LOG.append({"path": self.path, "auth": self.headers.get("authorization", ""), "body": body})
        self._send(200, {"id": "mock_%d" % len(LOG)})

    def log_message(self, *a):
        pass


if __name__ == "__main__":
    HTTPServer(("127.0.0.1", 9998), H).serve_forever()
