# A stand-in for Ollama on 127.0.0.1:11434, for nix/tests/apps-together.nix.
#
# The VM has no network to pull a model from, and a 3B model under TCG takes
# far longer than a test may, so this answers the two calls the assistant
# plugin makes (backend/plugins/assistant): GET /api/tags, which lists the
# model, and POST /api/chat, which streams the answer as NDJSON chunks the
# way Ollama does. Which answer it gives depends on the material: the first
# rule whose word is in the person's turn wins, so a file and a mail thread
# get different answers and the test can tell which one was read.
#
#   assistant-model-stand-in.py PORT MODEL RULES_JSON
#
# RULES_JSON is a list of {"when": word, "answer": text}. Every request is
# appended to /var/lib/assistant-stand-in/requests.jsonl, so the test can
# check what lososd sent and that nothing else did.

import http.server
import json
import os
import sys
import time

port, model = int(sys.argv[1]), sys.argv[2]
rules = json.load(open(sys.argv[3], encoding="utf-8"))
log_dir = os.environ.get("STATE_DIRECTORY", "/tmp")

# About four characters to a token.
CHARS = 4
# Quick enough for an emulated machine to keep up with the page polling it.
TOKENS_PER_S = 40


class Ollama(http.server.BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, *args):
        pass

    def reply(self, body):
        data = json.dumps(body).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def do_GET(self):
        if self.path == "/api/tags":
            self.reply({"models": [{"name": model, "model": model, "size": 2019393189}]})
        else:
            self.send_error(404)

    def do_POST(self):
        if self.path != "/api/chat":
            self.send_error(404)
            return
        request = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        with open(os.path.join(log_dir, "requests.jsonl"), "a") as log:
            log.write(json.dumps(request) + "\n")
        turn = "".join(m.get("content", "") for m in request.get("messages", []) if m.get("role") == "user")
        answer = next((r["answer"] for r in rules if r["when"] in turn), "The material does not say.")
        self.send_response(200)
        self.send_header("Content-Type", "application/x-ndjson")
        self.send_header("Transfer-Encoding", "chunked")
        self.end_headers()

        def chunk(obj):
            line = (json.dumps(obj) + "\n").encode()
            self.wfile.write(f"{len(line):x}\r\n".encode() + line + b"\r\n")
            self.wfile.flush()

        time.sleep(1)
        for i in range(0, len(answer), CHARS):
            chunk({"model": model, "message": {"role": "assistant", "content": answer[i : i + CHARS]}, "done": False})
            time.sleep(1 / TOKENS_PER_S)
        chunk({"model": model, "message": {"role": "assistant", "content": ""}, "done": True, "done_reason": "stop"})
        self.wfile.write(b"0\r\n\r\n")
        self.wfile.flush()


http.server.ThreadingHTTPServer(("127.0.0.1", port), Ollama).serve_forever()
