# A stand-in for Ollama on 127.0.0.1:11434, for nix/tests/app-builder.nix.
#
# The VM has no network to pull the model from, and Qwen2.5-Coder under TCG
# would take hours for one answer, so this replays answers the real model
# wrote: Qwen2.5-Coder 1.5B, through lososd's own builder, for the request
# the test types and for the fix lososd asks for after the page tried it.
# The first chat gets the first answer, the second the second, and so on. It answers the two calls lososd makes (backend/src/
# builder.rs): GET /api/tags, which lists the model, and POST /api/chat,
# which streams the answer as NDJSON chunks the way Ollama does. The pace is
# the real model's on the smallest box LosOS supports (two cores, no GPU):
# a pause for reading the prompt, then a token at a time.
#
#   ollama-stand-in.py PORT MODEL PROMPT_TOKENS_PER_S TOKENS_PER_S ANSWER_FILE...
#
# Every request it gets is appended to /tmp/ollama-requests.jsonl, so the
# test can check what lososd sent.

import http.server
import json
import sys
import time

port, model = int(sys.argv[1]), sys.argv[2]
prompt_rate, rate = float(sys.argv[3]), float(sys.argv[4])
answers = [open(f, encoding="utf-8").read() for f in sys.argv[5:]]
asked = 0

# About four characters to a token, as Qwen's tokenizer cuts English and code.
CHARS = 4


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
            self.reply({"models": [{"name": model, "model": model, "size": 986062089}]})
        else:
            self.send_error(404)

    def do_POST(self):
        if self.path != "/api/chat":
            self.send_error(404)
            return
        global asked
        request = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        answer = answers[min(asked, len(answers) - 1)]
        asked += 1
        with open("/tmp/ollama-requests.jsonl", "a") as log:
            log.write(json.dumps(request) + "\n")
        prompt_chars = sum(len(m.get("content", "")) for m in request.get("messages", []))
        prompt_tokens = prompt_chars // CHARS
        self.send_response(200)
        self.send_header("Content-Type", "application/x-ndjson")
        self.send_header("Transfer-Encoding", "chunked")
        self.end_headers()

        def chunk(obj):
            line = (json.dumps(obj) + "\n").encode()
            self.wfile.write(f"{len(line):x}\r\n".encode() + line + b"\r\n")
            self.wfile.flush()

        time.sleep(prompt_tokens / prompt_rate)
        pieces = [answer[i : i + CHARS] for i in range(0, len(answer), CHARS)]
        for piece in pieces:
            chunk({"model": model, "message": {"role": "assistant", "content": piece}, "done": False})
            time.sleep(1 / rate)
        chunk(
            {
                "model": model,
                "message": {"role": "assistant", "content": ""},
                "done": True,
                "done_reason": "stop",
                "prompt_eval_count": prompt_tokens,
                "eval_count": len(pieces),
            }
        )
        self.wfile.write(b"0\r\n\r\n")
        self.wfile.flush()


http.server.ThreadingHTTPServer(("127.0.0.1", port), Ollama).serve_forever()
