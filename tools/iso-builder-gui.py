#!/usr/bin/env python3
"""LosOS ISO Builder GUI — curlable, cross-platform, LosOS design system.

Builds the LosOS installer ISO and copies it to the user's Downloads folder.

Run locally from the repo:
    python3 tools/iso-builder-gui.py

Curl and run:
    curl -fsSL https://raw.githubusercontent.com/dasmatus/losos/main/tools/iso-builder-gui.py -o losos-iso-builder.py
    python3 losos-iso-builder.py

Options:
    --repo DIR      Repository root containing flake.nix (default: auto-detect)
    --port PORT     Local server port (default: 8765)
    --no-open       Do not open a browser automatically
    --host HOST     Bind host (default: 127.0.0.1)
"""
from __future__ import annotations

import argparse
import json
import os
import platform
import shutil
import subprocess
import sys
import threading
import time
import webbrowser
from http import HTTPStatus
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import urlparse

APP_TITLE = "LosOS ISO Builder"
DEFAULT_PORT = 8765
BUILD_ATTR = ".#nixosConfigurations.iso.config.system.build.isoImage"
BUILD_CMD = ["nix", "build", BUILD_ATTR, "--print-out-paths"]

LOSOS_TOKENS_CSS = r"""
:root {
  --ls-bg: #f5f7fa;
  --ls-surface: #ffffff;
  --ls-surface-muted: #f0f2f5;
  --ls-ink: #1c2128;
  --ls-ink-muted: #57606a;
  --ls-border: #d0d5dd;
  --ls-border-faint: #eef1f4;
  --ls-accent: #1f6feb;
  --ls-accent-strong: #1a5fd0;
  --ls-success: #2ecc71;
  --ls-success-tint: #e9f9ef;
  --ls-danger: #e5534b;
  --ls-danger-tint: #fdeceb;
  --ls-danger-deep: #8a2b25;
  --ls-warning: #d4a012;
  --ls-warning-tint: #fff8e6;
  --ls-topbar-link: #d0d5dd;
  --ls-neutral: #8b949e;
  --ls-control-off: #e9e9eb;
  --ls-radius-sm: 6px;
  --ls-radius-md: 8px;
  --ls-radius-lg: 12px;
  --ls-radius-pill: 999px;
  --ls-font: system-ui, -apple-system, "Segoe UI", Roboto, sans-serif;
  --ls-font-mono: ui-monospace, SFMono-Regular, "SF Mono", Menlo, Consolas, monospace;
  --ls-fs-xs: 12px;
  --ls-fs-sm: 13.5px;
  --ls-fs-md: 14px;
  --ls-fs-base: 15px;
  --ls-fs-lg: 16px;
  --ls-fs-xl: 17px;
  --ls-fs-2xl: 20px;
  --ls-shadow-sm: 0 4px 14px rgba(27, 33, 40, 0.10);
  --ls-shadow-lg: 0 12px 40px rgba(0, 0, 0, 0.22);
  --ls-shadow-knob: 0 2px 4px rgba(0, 0, 0, 0.22);
}
"""

LOSOS_COMPONENT_CSS = r"""
* { box-sizing: border-box; }
[hidden] { display: none !important; }
html { -webkit-text-size-adjust: 100%; }
body {
  margin: 0; background: var(--ls-bg); color: var(--ls-ink);
  font-family: var(--ls-font); font-size: var(--ls-fs-base); line-height: 1.45;
}
a { color: var(--ls-accent); text-decoration: none; }
a:hover { text-decoration: underline; }
code { font-family: var(--ls-font-mono); font-size: 0.92em; }
:focus-visible { outline: 2px solid var(--ls-accent); outline-offset: 2px; border-radius: 4px; }
.topbar {
  display: flex; align-items: center; gap: 16px; height: 48px; padding: 0 20px;
  background: var(--ls-ink); color: var(--ls-surface);
}
.topbar .brand { color: var(--ls-surface); font-weight: 700; font-size: var(--ls-fs-xl); letter-spacing: 0.2px; text-decoration: none; }
.topnav { margin-left: auto; display: flex; align-items: center; gap: 4px; }
.topnav span { color: var(--ls-topbar-link); font-size: var(--ls-fs-md); padding: 6px 10px; }
.container { max-width: 920px; margin: 0 auto; padding: 20px 16px 48px; }
.grid { display: grid; grid-template-columns: 1fr; gap: 16px; margin-top: 16px; }
.card {
  background: var(--ls-surface); border: 1px solid var(--ls-border); border-radius: var(--ls-radius-md);
  padding: 16px; display: flex; flex-direction: column; gap: 8px; transition: box-shadow 0.15s ease;
}
.card:hover { box-shadow: var(--ls-shadow-sm); }
.card-head { display: flex; align-items: center; justify-content: space-between; gap: 8px; }
.card-head h2 { margin: 0; font-size: var(--ls-fs-lg); font-weight: 600; }
.desc { margin: 0; color: var(--ls-ink-muted); font-size: var(--ls-fs-sm); }
.status { display: inline-flex; align-items: center; gap: 6px; font-size: var(--ls-fs-xs); color: var(--ls-ink-muted); }
.dot { width: 10px; height: 10px; border-radius: 50%; background: var(--ls-neutral); flex: none; }
.dot[data-state="up"] { background: var(--ls-success); }
.dot[data-state="down"] { background: var(--ls-danger); }
.dot[data-state="checking"] { background: var(--ls-neutral); }
.btn-primary {
  font: inherit; font-weight: 600; color: var(--ls-surface); background: var(--ls-accent);
  border: 0; border-radius: var(--ls-radius-md); padding: 9px 16px; cursor: pointer;
}
.btn-primary:hover:not(:disabled) { background: var(--ls-accent-strong); }
.btn-primary:disabled { opacity: 0.45; cursor: default; }
.btn-outline {
  font: inherit; font-weight: 600; font-size: var(--ls-fs-md); color: var(--ls-ink); background: var(--ls-surface);
  border: 1px solid var(--ls-ink); border-radius: var(--ls-radius-sm); padding: 9px 14px; cursor: pointer;
}
.btn-outline:hover:not(:disabled) { background: var(--ls-ink); color: var(--ls-surface); }
.btn-outline:disabled { opacity: 0.45; cursor: default; }
.actions { display: flex; gap: 10px; flex-wrap: wrap; margin-top: 8px; }
.banner {
  background: var(--ls-surface); border: 1px solid var(--ls-border); border-radius: var(--ls-radius-md);
  padding: 12px 16px; margin-bottom: 4px;
}
.banner[data-kind="building"] { border-color: var(--ls-warning); background: var(--ls-warning-tint); }
.banner[data-kind="done"] { border-color: var(--ls-success); background: var(--ls-success-tint); }
.banner[data-kind="failed"] { border-color: var(--ls-danger); background: var(--ls-danger-tint); }
.banner-row { display: flex; align-items: baseline; gap: 10px; flex-wrap: wrap; }
.banner-msg { font-weight: 600; font-size: var(--ls-fs-md); }
.indbar { margin-top: 10px; height: 4px; border-radius: 2px; background: rgba(0,0,0,0.08); overflow: hidden; display: none; }
.banner[data-kind="building"] .indbar { display: block; }
.indbar span { display: block; width: 30%; height: 100%; border-radius: 2px; background: var(--ls-warning); animation: indslide 1.2s ease-in-out infinite; }
@keyframes indslide { 0% { transform: translateX(-100%); } 50% { transform: translateX(180%); } 100% { transform: translateX(360%); } }
.meta { display: grid; grid-template-columns: 160px 1fr; gap: 4px 12px; font-size: var(--ls-fs-sm); margin: 0; }
.meta dt { color: var(--ls-ink-muted); }
.meta dd { margin: 0; font-family: var(--ls-font-mono); font-size: var(--ls-fs-xs); word-break: break-all; }
.log {
  margin: 0; padding: 12px; background: #0f141a; color: #d0d5dd; border-radius: var(--ls-radius-sm);
  font-family: var(--ls-font-mono); font-size: 12px; line-height: 1.5; max-height: 320px; overflow: auto;
  white-space: pre-wrap; word-break: break-word;
}
@media (max-width: 640px) {
  .topbar { padding: 0 12px; gap: 8px; }
  .meta { grid-template-columns: 1fr; }
}
"""


def find_repo_root(start: Path) -> Path | None:
    cur = start.resolve()
    for candidate in [cur, *cur.parents]:
        flake = candidate / "flake.nix"
        if flake.is_file():
            try:
                text = flake.read_text(encoding="utf-8", errors="ignore")
            except OSError:
                continue
            if "nixosConfigurations" in text and "iso" in text:
                return candidate
    return None


def downloads_dir() -> Path:
    env = os.environ.get("LOSOS_DOWNLOADS_DIR")
    if env:
        return Path(env).expanduser()
    home = Path.home()
    win_profile = os.environ.get("USERPROFILE")
    if platform.system() == "Windows" and win_profile:
        home = Path(win_profile)
    return home / "Downloads"


def find_iso(result_path: Path) -> Path | None:
    candidates: list[Path] = []
    if result_path.is_file() and result_path.suffix.lower() == ".iso":
        candidates.append(result_path)
    if result_path.is_dir():
        candidates.extend(sorted(result_path.glob("iso/*.iso")))
        candidates.extend(sorted(result_path.glob("*.iso")))
    for c in candidates:
        if c.is_file():
            return c.resolve()
    return None


def copy_iso_to_downloads(iso_path: Path, dest_dir: Path) -> Path:
    dest_dir.mkdir(parents=True, exist_ok=True)
    dest = dest_dir / iso_path.name
    if dest.exists():
        stem = dest.stem
        suffix = dest.suffix
        stamp = time.strftime("%Y%m%d-%H%M%S")
        dest = dest_dir / f"{stem}-{stamp}{suffix}"
    shutil.copy2(iso_path, dest)
    return dest


class BuildJob:
    def __init__(self, repo: Path):
        self.repo = repo
        self.lock = threading.Lock()
        self.reset()

    def reset(self) -> None:
        with self.lock:
            self.state = "idle"  # idle|building|done|failed
            self.message = "Ready to build the LosOS ISO."
            self.log_lines: list[str] = []
            self.started_at: float | None = None
            self.finished_at: float | None = None
            self.iso_path: str | None = None
            self.downloads_path: str | None = None
            self.exit_code: int | None = None

    def snapshot(self) -> dict:
        with self.lock:
            return {
                "state": self.state,
                "message": self.message,
                "log": "\n".join(self.log_lines[-400:]),
                "started_at": self.started_at,
                "finished_at": self.finished_at,
                "iso_path": self.iso_path,
                "downloads_path": self.downloads_path,
                "exit_code": self.exit_code,
                "repo": str(self.repo),
                "downloads_dir": str(downloads_dir()),
                "build_cmd": BUILD_CMD,
                "platform": platform.platform(),
            }

    def _append(self, line: str) -> None:
        with self.lock:
            self.log_lines.append(line.rstrip("\n"))
            if len(self.log_lines) > 2000:
                self.log_lines = self.log_lines[-1200:]

    def start(self) -> bool:
        with self.lock:
            if self.state == "building":
                return False
            self.state = "building"
            self.message = "Building ISO… this can take a while."
            self.log_lines = []
            self.started_at = time.time()
            self.finished_at = None
            self.iso_path = None
            self.downloads_path = None
            self.exit_code = None
        t = threading.Thread(target=self._run, daemon=True)
        t.start()
        return True

    def _run(self) -> None:
        repo = self.repo
        self._append(f"$ {' '.join(BUILD_CMD)}")
        self._append(f"repo: {repo}")
        try:
            proc = subprocess.Popen(
                BUILD_CMD,
                cwd=str(repo),
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                text=True,
                bufsize=1,
            )
        except Exception as e:  # noqa: BLE001
            with self.lock:
                self.state = "failed"
                self.message = f"Failed to start nix build: {e}"
                self.finished_at = time.time()
                self.exit_code = -1
            self._append(f"error: {e}")
            return

        assert proc.stdout is not None
        out_paths: list[str] = []
        for line in proc.stdout:
            self._append(line)
            s = line.strip()
            if s.startswith("/nix/store/"):
                out_paths.append(s)
        rc = proc.wait()
        with self.lock:
            self.exit_code = rc

        if rc != 0:
            with self.lock:
                self.state = "failed"
                self.message = f"nix build failed (exit {rc})."
                self.finished_at = time.time()
            return

        iso: Path | None = None
        for p in reversed(out_paths):
            iso = find_iso(Path(p))
            if iso:
                break
        if iso is None:
            # fallback: result symlink in repo
            iso = find_iso(repo / "result")
        if iso is None:
            with self.lock:
                self.state = "failed"
                self.message = "Build succeeded but no ISO file was found in the result."
                self.finished_at = time.time()
            return

        try:
            dest = copy_iso_to_downloads(iso, downloads_dir())
        except Exception as e:  # noqa: BLE001
            with self.lock:
                self.state = "failed"
                self.message = f"ISO built but copying to Downloads failed: {e}"
                self.iso_path = str(iso)
                self.finished_at = time.time()
            self._append(f"copy error: {e}")
            return

        with self.lock:
            self.state = "done"
            self.message = f"ISO copied to Downloads: {dest.name}"
            self.iso_path = str(iso)
            self.downloads_path = str(dest)
            self.finished_at = time.time()
        self._append(f"copied: {iso} -> {dest}")


def render_html() -> str:
    css = LOSOS_TOKENS_CSS + "\n" + LOSOS_COMPONENT_CSS
    return f"""<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>{APP_TITLE}</title>
  <style>{css}</style>
</head>
<body>
  <nav class="topbar" aria-label="Top bar">
    <a class="brand" href="/">LosOS</a>
    <div class="topnav"><span>ISO Builder</span></div>
  </nav>
  <main class="container">
    <div class="grid">
      <section class="card" aria-labelledby="build-title">
        <div class="card-head">
          <h2 id="build-title">Build installer ISO</h2>
          <span class="status"><span class="dot" id="dot" data-state="checking"></span><span id="status-text">Checking</span></span>
        </div>
        <p class="desc">Builds <code>{BUILD_ATTR}</code> with Nix and copies the resulting ISO to your Downloads folder. Styled with the LosOS design system.</p>
        <dl class="meta">
          <dt>Repository</dt><dd id="repo">-</dd>
          <dt>Downloads</dt><dd id="downloads">-</dd>
          <dt>Build command</dt><dd id="cmd">-</dd>
          <dt>Platform</dt><dd id="platform">-</dd>
        </dl>
        <div class="actions">
          <button class="btn-primary" id="build-btn" type="button">Build ISO &amp; move to Downloads</button>
          <button class="btn-outline" id="refresh-btn" type="button">Refresh</button>
        </div>
      </section>

      <section class="banner" id="banner" data-kind="building" aria-live="polite">
        <div class="banner-row">
          <span class="banner-msg" id="banner-msg">Ready.</span>
        </div>
        <div class="indbar" aria-hidden="true"><span></span></div>
      </section>

      <section class="card" aria-labelledby="log-title">
        <div class="card-head">
          <h2 id="log-title">Build log</h2>
        </div>
        <pre class="log" id="log">No build yet.</pre>
      </section>

      <section class="card" aria-labelledby="result-title">
        <div class="card-head">
          <h2 id="result-title">Result</h2>
        </div>
        <dl class="meta">
          <dt>State</dt><dd id="r-state">idle</dd>
          <dt>ISO in store</dt><dd id="r-iso">-</dd>
          <dt>Copied to</dt><dd id="r-dest">-</dd>
          <dt>Exit code</dt><dd id="r-exit">-</dd>
        </dl>
      </section>
    </div>
  </main>
  <script>
    const dot = document.getElementById('dot');
    const statusText = document.getElementById('status-text');
    const banner = document.getElementById('banner');
    const bannerMsg = document.getElementById('banner-msg');
    const logEl = document.getElementById('log');
    const buildBtn = document.getElementById('build-btn');
    const refreshBtn = document.getElementById('refresh-btn');
    function setState(s) {{
      const map = {{ idle: ['checking','Idle'], building: ['checking','Building'], done: ['up','Done'], failed: ['down','Failed'] }};
      const m = map[s.state] || ['checking', s.state];
      dot.dataset.state = m[0];
      statusText.textContent = m[1];
      banner.dataset.kind = s.state === 'idle' ? 'idle' : s.state;
      banner.style.borderColor = s.state === 'idle' ? 'var(--ls-border)' : '';
      bannerMsg.textContent = s.message || '';
      logEl.textContent = s.log || 'No build yet.';
      document.getElementById('repo').textContent = s.repo || '-';
      document.getElementById('downloads').textContent = s.downloads_dir || '-';
      document.getElementById('cmd').textContent = (s.build_cmd || []).join(' ');
      document.getElementById('platform').textContent = s.platform || '-';
      document.getElementById('r-state').textContent = s.state || '-';
      document.getElementById('r-iso').textContent = s.iso_path || '-';
      document.getElementById('r-dest').textContent = s.downloads_path || '-';
      document.getElementById('r-exit').textContent = s.exit_code == null ? '-' : String(s.exit_code);
      buildBtn.disabled = s.state === 'building';
      buildBtn.textContent = s.state === 'building' ? 'Building…' : 'Build ISO & move to Downloads';
    }}
    async function refresh() {{
      try {{
        const r = await fetch('/api/status');
        setState(await r.json());
      }} catch (e) {{
        banner.dataset.kind = 'failed';
        bannerMsg.textContent = 'Cannot reach local app server.';
      }}
    }}
    buildBtn.addEventListener('click', async () => {{
      buildBtn.disabled = true;
      try {{
        const r = await fetch('/api/build', {{ method: 'POST' }});
        setState(await r.json());
      }} catch (e) {{
        banner.dataset.kind = 'failed';
        bannerMsg.textContent = 'Failed to start build.';
      }}
      refresh();
    }});
    refreshBtn.addEventListener('click', refresh);
    refresh();
    setInterval(refresh, 1500);
  </script>
</body>
</html>"""


class Handler(BaseHTTPRequestHandler):
    job: BuildJob

    def log_message(self, fmt: str, *args) -> None:  # quieter
        sys.stderr.write(f"[iso-builder-gui] {self.address_string()} {fmt % args}\n")

    def _send(self, code: int, body: bytes, content_type: str) -> None:
        self.send_response(code)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        self.wfile.write(body)

    def _send_json(self, code: int, payload: dict) -> None:
        body = json.dumps(payload).encode("utf-8")
        self._send(code, body, "application/json; charset=utf-8")

    def do_GET(self) -> None:  # noqa: N802
        path = urlparse(self.path).path
        if path == "/" or path == "/index.html":
            self._send(HTTPStatus.OK, render_html().encode("utf-8"), "text/html; charset=utf-8")
            return
        if path == "/api/status":
            self._send_json(HTTPStatus.OK, self.job.snapshot())
            return
        if path == "/api/config":
            self._send_json(HTTPStatus.OK, self.job.snapshot())
            return
        self._send_json(HTTPStatus.NOT_FOUND, {"error": "not found"})

    def do_POST(self) -> None:  # noqa: N802
        path = urlparse(self.path).path
        if path == "/api/build":
            started = self.job.start()
            snap = self.job.snapshot()
            snap["started"] = started
            self._send_json(HTTPStatus.OK, snap)
            return
        self._send_json(HTTPStatus.NOT_FOUND, {"error": "not found"})


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    p = argparse.ArgumentParser(description=APP_TITLE)
    p.add_argument("--repo", default=None, help="Repository root (default: auto-detect)")
    p.add_argument("--port", type=int, default=DEFAULT_PORT, help=f"Local server port (default: {DEFAULT_PORT})")
    p.add_argument("--host", default="127.0.0.1", help="Bind host (default: 127.0.0.1)")
    p.add_argument("--no-open", action="store_true", help="Do not open browser automatically")
    p.add_argument("--smoke", action="store_true", help="Run a smoke check and exit")
    return p.parse_args(argv)


def resolve_repo(arg: str | None) -> Path:
    if arg:
        return Path(arg).expanduser().resolve()
    detected = find_repo_root(Path.cwd())
    if detected:
        return detected
    script_dir = Path(__file__).resolve().parent
    detected = find_repo_root(script_dir)
    if detected:
        return detected
    return Path.cwd().resolve()


def make_server(host: str, port: int, job: BuildJob) -> ThreadingHTTPServer:
    Handler.job = job
    return ThreadingHTTPServer((host, port), Handler)


def smoke_check(repo: Path) -> int:
    job = BuildJob(repo)
    server = make_server("127.0.0.1", 0, job)
    port = server.server_address[1]
    t = threading.Thread(target=server.serve_forever, daemon=True)
    t.start()
    try:
        import urllib.request
        with urllib.request.urlopen(f"http://127.0.0.1:{port}/", timeout=5) as r:
            html = r.read().decode("utf-8")
        required = [
            "--ls-bg", "--ls-accent", "--ls-surface", "LosOS", "Build ISO",
            "btn-primary", "topbar", "design system", "downloads", BUILD_ATTR,
        ]
        missing = [m for m in required if m.lower() not in html.lower()]
        if missing:
            print(f"SMOKE FAIL: missing markers: {missing}")
            return 1
        with urllib.request.urlopen(f"http://127.0.0.1:{port}/api/status", timeout=5) as r:
            status = json.loads(r.read().decode("utf-8"))
        for key in ["state", "repo", "downloads_dir", "build_cmd"]:
            if key not in status:
                print(f"SMOKE FAIL: missing status key {key}")
                return 1
        print("SMOKE OK")
        return 0
    finally:
        server.shutdown()
        server.server_close()


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    repo = resolve_repo(args.repo)
    if args.smoke:
        return smoke_check(repo)

    job = BuildJob(repo)
    if not (repo / "flake.nix").is_file():
        job._append(f"warning: flake.nix not found in {repo}; set --repo to your LosOS checkout")

    server = make_server(args.host, args.port, job)
    url = f"http://{args.host}:{args.port}/"
    print(f"{APP_TITLE} running at {url}")
    print(f"Repo: {repo}")
    print(f"Downloads: {downloads_dir()}")
    print("Press Ctrl+C to stop.")
    if not args.no_open:
        try:
            webbrowser.open(url)
        except Exception:
            pass
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print("\nStopping...")
    finally:
        server.shutdown()
        server.server_close()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
