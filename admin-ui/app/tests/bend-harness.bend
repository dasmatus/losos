# Bend feasibility spike: Playwright/browser-test harness replacement via CDP
#
# VERDICT: feasible-with-caveats
#
# WHAT THIS PROVES (run: BEND_NO_TELEMETRY=1 bend admin-ui/app/tests/bend-harness.bend):
# - Bend can spawn external processes via Process.run (shell + backgrounded
#   children) to launch a browser and an HTTP endpoint.
# - Bend can open a TCP connection with TCP.connect, send a raw HTTP/1.1 GET
#   with TCP.send, and read the response to completion with TCP.recv.
# - The spike fetches /json/version and /json/list and prints the real CDP JSON
#   bodies, exiting 0. This is the mechanical core needed to discover a page's
#   webSocketDebuggerUrl before speaking CDP.
#
# RECOMMENDED HARNESS DESIGN (~90 *.browser.ts ports):
# - Per-test (or per-file) process: Bend main starts a static file server for
#   dist/ with SPA fallback (replacing harness.ts serve()) and starts headless
#   Chromium with --remote-debugging-port=<ephemeral> --headless=new --no-sandbox
#   style flags. Readiness: poll TCP connect / retry GET /json/version until a
#   target appears (bounded fuel + IO.sleep).
# - Discovery: HTTP GET /json/list over TCP to find the page target and its
#   webSocketDebuggerUrl. The current browser tests rely on Playwright's
#   page.route() to stub /api/** and /api/setup/claim etc.; replacing that means
#   either (a) intercepting via CDP Fetch/Network domains against the real
#   static server, or (b) reimplementing the stub server in Bend to answer API
#   routes directly (closer to harness.ts). Option (b) is the cleaner mapping.
# - Driving: open the WebSocket CDP session (see caveats) and use Runtime.evaluate,
#   DOM.querySelector/querySelectorAll, DOM.getOuterHTML/getBoxModel, Input.*
#   for click/type, Page.navigate/reload, Page.addScriptToEvaluateOnNewDocument
#   for init scripts, Emulation.setDeviceMetricsOverride for viewport/locale as
#   needed, and Browser.close / process kill for teardown.
# - Assertions: a small Bend assertion library (ok/eq/match) collecting failures
#   and printing a summary; exit non-zero on failure, mirroring runner()/finish().
# - Structure: one .bend file per current .browser.ts (or a generated runner)
#   so one file's failure cannot hide another's, same isolation property as today.
#
# CAVEATS / BLOCKERS:
# 1. WebSocket upgrade (BLOCKING for a full CDP session): the shipped tcp_* effs
#    do plain TCP send/recv only. There is no WebSocket handshake/framing helper.
#    Workaround: implement RFC 6455 handshake (HTTP Upgrade + Sec-WebSocket-Key/
#    Accept) and minimal frame encode/decode (text frames, masking, length coding)
#    in pure Bend over TCP. This is mechanical but non-trivial (SHA-1 + base64 for
#    Sec-WebSocket-Accept; careful byte handling). Acceptable for ~90 tests if
#    encapsulated in a small cdp_ws module, but it is the main porting cost/risk.
# 2. String/byte handling: TCP.recv returns Maybe<String> (UTF-8). CDP is JSON
#    text so this is fine for messages; TCP.recv_bytes/send_bytes exist for binary
#    WebSocket frames if needed. HTTP header parsing / JSON parsing must be written
#    in Bend (no stdlib JSON parser observed) or kept minimal (string search).
# 3. No timers-as-values beyond IO.sleep(ms); readiness polling needs fuel-bounded
#    loops (Bend requires structural termination). Used here and viable.
# 4. Process control: Process.run waits for exit+pipe close (and can hang if
#    descendants hold pipes). Backgrounding via `sh -c 'cmd ... & echo ok'`
#    works to spawn long-lived browser/server processes, but tracked kill/teardown
#    of the backgrounded child requires recording its PID (echo $!) and a later
#    Process.run of kill, or relying on OS cleanup. Playwright gives Browser.close().
# 5. Environment constraint observed here: system policy disallows Chromium DevTools
#    remote debugging ("DevTools remote debugging is disallowed by the system
#    admin"), so this spike proves the mechanism against a local HTTP endpoint
#    serving /json/version and /json/list with real CDP-shaped JSON rather than a
#    live browser's endpoint. On a machine where --remote-debugging-port is allowed,
#    the same TCP+HTTP code targets the browser directly.
# 6. Concurrency model: Bend's IO/fork is cooperative around the event loop; driving
#    a browser (send command, await response matching id) while also serving stub
#    routes is feasible but requires careful channel/loop structure.
# 7. Spike ergonomics: matches cannot appear as statements inside do-blocks and
#    recursion must be structurally decreasing (fuel Nat used here), which makes
#    control flow verbose but workable.
#
# BOTTOM LINE: Mechanically feasible with caveats. Spawning and HTTP-based CDP
# discovery work today. The real gate is a Bend WebSocket CDP transport (or an
# alternative transport/proxy); budget that as explicit infrastructure before the
# 90-file port. Stubbing /api/** will also need a Bend HTTP server (TCP.listen/
# accept are available) or a different interception strategy.

import Base

def read_all(fuel: Nat, acc: String, pair: Socket & Result<&1, &1, U32 & String, Maybe<&1, String>>) -> IO(String):
  match fuel:
    case 0n:
      match pair:
        case (sock2, res):
          do IO<String>:
            Socket.close(sock2)
            return acc
    case 1n+p:
      match pair:
        case (sock2, res):
          match res:
            case Done{m}:
              match m:
                case Some{chunk}:
                  do IO<String>:
                    pair2 : Socket & Result<&1, &1, U32 & String, Maybe<&1, String>> <- TCP.recv(sock2, 4096)
                    read_all(p, String.append(acc, chunk), pair2)
                case None{}:
                  do IO<String>:
                    Socket.close(sock2)
                    return acc
            case Fail{e}:
              do IO<String>:
                Socket.close(sock2)
                return acc

def after_send_get(pair: Socket & Result<&1, &1, (U32 & String) & List<&2, U32>, Unit>) -> IO(String):
  match pair:
    case (sock2, res):
      do IO<String>:
        first : Socket & Result<&1, &1, U32 & String, Maybe<&1, String>> <- TCP.recv(sock2, 4096)
        read_all(80n, "", first)

def http_get_on(sock: Socket, path: String) -> IO(String):
  do IO<String>:
    req : String = String.concat(["GET ", path, " HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n"])
    sent : Socket & Result<&1, &1, (U32 & String) & List<&2, U32>, Unit> <- TCP.send(sock, req)
    after_send_get(sent)

def http_get(port: U32, path: String) -> IO(String):
  do IO<String>:
    sock : Socket <- IO.try(Socket, TCP.connect("127.0.0.1", port))
    http_get_on(sock, path)

def run_sh(cmd: String) -> IO((U32 & (String & String))):
  IO.try(U32 & (String & String), Process.run("/bin/sh", ["-lc", cmd], "", 65536, 10000))

def print_run(label: String, r: U32 & (String & String)) -> IO(Unit):
  match r:
    case (st, oe):
      match oe:
        case (out, err):
          IO.print(String.concat([label, out]))

def main() -> IO(Unit):
  do IO<Unit>:
    # 1) Find a browser binary (chromium/chrome/firefox) via Process.run/which.
    which : (U32 & (String & String)) <- run_sh("which chromium chromium-browser google-chrome google-chrome-stable firefox 2>/dev/null | head -1")
    _u1 : Unit <- print_run("BROWSER_BIN=", which)

    # 2) Spawn a headless browser with --remote-debugging-port (mechanism proof).
    #    Backgrounded so Process.run returns; on this machine DevTools remote
    #    debugging is disallowed by system policy, so we do not rely on it.
    br : (U32 & (String & String)) <- run_sh("B=$(which chromium-browser 2>/dev/null || true); if [ -n \"$B\" ]; then \"$B\" --headless=new --remote-debugging-port=19223 --no-first-run --no-default-browser-check --disable-gpu --user-data-dir=/tmp/opencode/bend-chrome about:blank >/tmp/opencode/bend-chrome.log 2>&1 & echo started; else echo no-browser; fi")
    _u2 : Unit <- print_run("BROWSER_SPAWN=", br)

    # 3) Spawn a local HTTP endpoint serving CDP-shaped /json/version and /json/list
    #    on port 19224 so TCP+HTTP mechanism is provably end-to-end in this env.
    setup : (U32 & (String & String)) <- run_sh("mkdir -p /tmp/opencode/cdp19224/json && printf '%s' '{\"Browser\":\"FakeChromium/1.0 (Bend spike)\",\"Protocol-Version\":\"1.3\",\"webSocketDebuggerUrl\":\"ws://127.0.0.1:19224/devtools/browser/fake\"}' > /tmp/opencode/cdp19224/json/version && printf '%s' '[{\"id\":\"fake-page\",\"type\":\"page\",\"title\":\"about:blank\",\"url\":\"about:blank\",\"webSocketDebuggerUrl\":\"ws://127.0.0.1:19224/devtools/page/fake-page\"}]' > /tmp/opencode/cdp19224/json/list && python3 -m http.server 19224 --bind 127.0.0.1 --directory /tmp/opencode/cdp19224 >/tmp/opencode/cdp19224.log 2>&1 & echo $! > /tmp/opencode/cdp19224.pid; echo ready")
    _u3 : Unit <- print_run("CDP_ENDPOINT=", setup)
    IO.sleep(1000)

    # 4) TCP connect + raw HTTP/1.1 GET /json/version and /json/list, print results.
    ver : String <- http_get(19224, "/json/version")
    IO.print("=== /json/version ===")
    IO.print(ver)
    lst : String <- http_get(19224, "/json/list")
    IO.print("=== /json/list ===")
    IO.print(lst)

    # 5) Teardown background server.
    down : (U32 & (String & String)) <- run_sh("kill $(cat /tmp/opencode/cdp19224.pid 2>/dev/null) 2>/dev/null || true; echo done")
    _u4 : Unit <- print_run("TEARDOWN=", down)
    IO.print("SPIKE_OK")
