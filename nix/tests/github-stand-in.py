"""A stand-in for the parts of GitHub a problem report talks to.

The device flow (`/login/device/code`, `/login/oauth/access_token`), the
issue search and the two ways of filing (`POST .../issues`, `POST
.../issues/<n>/comments`), on one loopback port, for nix/tests/problems.nix
and nix/tests/installer-report.nix. The code is "entered" when the test
creates the approval file; until then the token endpoint answers
`authorization_pending`, as GitHub does. Every request is appended to
requests.jsonl in the state folder, so the test can read what was sent.

    github-stand-in.py PORT STATE_DIR
"""

import json
import os
import sys
import urllib.parse
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

PORT = int(sys.argv[1])
STATE = sys.argv[2]
os.makedirs(STATE, exist_ok=True)
ISSUES = os.path.join(STATE, "issues.json")
APPROVED = os.path.join(STATE, "approved")
CODE = "WDJB-MJHT"
TOKEN = "ghu_standin"
REPO_PAGES = "https://github.com"


def issues():
    try:
        with open(ISSUES) as f:
            return json.load(f)
    except FileNotFoundError:
        return []


def save(items):
    with open(ISSUES, "w") as f:
        json.dump(items, f)


class Handler(BaseHTTPRequestHandler):
    def answer(self, status, body):
        data = json.dumps(body).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def record(self, body):
        with open(os.path.join(STATE, "requests.jsonl"), "a") as f:
            f.write(
                json.dumps(
                    {
                        "method": self.command,
                        "path": self.path,
                        "authorization": self.headers.get("Authorization"),
                        "accept": self.headers.get("Accept"),
                        "body": body,
                    }
                )
                + "\n"
            )

    def body(self):
        length = int(self.headers.get("Content-Length") or 0)
        return self.rfile.read(length).decode() if length else ""

    def authorised(self):
        return self.headers.get("Authorization") == f"Bearer {TOKEN}"

    def do_GET(self):
        self.record("")
        url = urllib.parse.urlparse(self.path)
        if url.path != "/search/issues":
            return self.answer(404, {"message": "Not Found"})
        if not self.authorised():
            return self.answer(401, {"message": "Bad credentials"})
        q = urllib.parse.parse_qs(url.query).get("q", [""])[0]
        start = q.find('"[')
        fingerprint = q[start + 1 : q.find(']"', start) + 1] if start >= 0 else None
        found = [i for i in issues() if fingerprint and i["title"].endswith(fingerprint)]
        return self.answer(200, {"total_count": len(found), "items": found})

    def do_POST(self):
        raw = self.body()
        self.record(raw)
        path = urllib.parse.urlparse(self.path).path
        if path == "/login/device/code":
            form = urllib.parse.parse_qs(raw)
            if form.get("client_id") != ["Iv23test"]:
                return self.answer(200, {"error": "unauthorized_client"})
            return self.answer(
                200,
                {
                    "device_code": "device-1",
                    "user_code": CODE,
                    "verification_uri": "https://github.com/login/device",
                    "expires_in": 900,
                    "interval": 1,
                },
            )
        if path == "/login/oauth/access_token":
            form = urllib.parse.parse_qs(raw)
            if form.get("device_code") != ["device-1"]:
                return self.answer(200, {"error": "bad_verification_code"})
            if os.path.exists(APPROVED):
                return self.answer(200, {"access_token": TOKEN, "token_type": "bearer"})
            return self.answer(200, {"error": "authorization_pending"})
        if not self.authorised():
            return self.answer(401, {"message": "Bad credentials"})
        parts = path.strip("/").split("/")
        # repos/<owner>/<name>/issues[/<n>/comments]
        if len(parts) >= 4 and parts[0] == "repos" and parts[3] == "issues":
            repo = f"{parts[1]}/{parts[2]}"
            body = json.loads(raw)
            items = issues()
            if len(parts) == 4:
                number = len(items) + 1
                url = f"{REPO_PAGES}/{repo}/issues/{number}"
                items.append(
                    {"number": number, "title": body["title"], "body": body["body"], "html_url": url, "comments": []}
                )
                save(items)
                return self.answer(201, {"number": number, "html_url": url})
            if len(parts) == 6 and parts[5] == "comments":
                number = int(parts[4])
                for item in items:
                    if item["number"] == number:
                        item["comments"].append(body["body"])
                        save(items)
                        url = f"{item['html_url']}#issuecomment-{len(item['comments'])}"
                        return self.answer(201, {"html_url": url})
        return self.answer(404, {"message": "Not Found"})

    def log_message(self, *_):
        pass


ThreadingHTTPServer(("127.0.0.1", PORT), Handler).serve_forever()
