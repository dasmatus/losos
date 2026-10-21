# A stand-in for Ollama on 127.0.0.1:11434, for nix/tests/git-reviewer.nix.
#
# The VM has no network to pull the model from, and Qwen2.5-Coder under TCG
# would take hours, so this replays what the real model answered: Qwen2.5-
# Coder 1.5B, asked by `losos-git review-diff` the same questions about the
# same pull request. Answers are picked by the kind of question (the first
# words of the system prompt) and the file it names. A question nothing
# was recorded for gets "LGTM", the answer that adds no comment.
#
#   ollama-stand-in.py PORT MODEL ANSWERS.json PROMPT_TOKENS_PER_S
#
# It waits as long as the model took to read the question on the smallest
# box (two cores), so the review's progress can be seen. Every request is
# appended to /tmp/ollama-requests.jsonl for the test to read.

import http.server
import json
import re
import sys
import time

port, model = int(sys.argv[1]), sys.argv[2]
answers = json.load(open(sys.argv[3], encoding="utf-8"))
prompt_rate = float(sys.argv[4])

KINDS = {
    "You are a careful code reviewer": "find",
    "You describe one file": "describe",
    "You write the overview": "overview",
}


def recorded(system, user):
    kind = next((k for p, k in KINDS.items() if system.startswith(p)), None)
    m = re.search(r"^File: (\S+)", user, re.M)
    path = m.group(1) if m else ""
    for a in answers:
        if a["kind"] == kind and a.get("path", "") == path:
            return a["answer"]
    return "LGTM"


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
            self.reply({"models": [{"name": model, "model": model}]})
        else:
            self.send_error(404)

    def do_POST(self):
        if self.path != "/api/chat":
            self.send_error(404)
            return
        request = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        with open("/tmp/ollama-requests.jsonl", "a") as log:
            log.write(json.dumps(request) + "\n")
        system = request["messages"][0]["content"]
        user = request["messages"][1]["content"]
        tokens = len(user) // 4
        time.sleep(min(tokens / prompt_rate, 20))
        self.reply({
            "model": model,
            "message": {"role": "assistant", "content": recorded(system, user)},
            "done": True,
            "done_reason": "stop",
            "prompt_eval_count": tokens,
            "eval_count": 20,
        })


http.server.ThreadingHTTPServer(("127.0.0.1", port), Ollama).serve_forever()
