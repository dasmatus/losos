#!/usr/bin/env bun
// Probe an edge the way a box meets it. Shim target for check.sh.
import { spawnSync } from "node:child_process";

const edge = (Bun.argv[2] || "https://register.dasmat.us").replace(/\/$/, "");
const host = edge.replace(/^[^:]+:\/\//, "").replace(/[/:].*$/, "");
const tunnelPort = Bun.env.TUNNEL_PORT || "2333";
let fails = 0;
const pass = (s: string) => console.log(`ok    ${s}`);
const fail = (s: string) => { console.log(`FAIL  ${s}`); fails++; };

function get(path: string) {
  const p = spawnSync("curl", ["-sS", "-m", "15", "-w", "\n%{http_code}", `${edge}${path}`], { encoding: "utf8" });
  const out = (p.stdout || "") + (p.stderr || "");
  const i = out.lastIndexOf("\n");
  return { body: out.slice(0, i), code: out.slice(i + 1).trim() };
}
function checkStatus(path: string, want: string, label: string) {
  const { code } = get(path);
  if (code === want) pass(`${label} (${path}, ${code})`); else fail(`${label} (${path}): wanted ${want}, got ${code}`);
}

checkStatus("/health", "200", "control plane answers");
checkStatus("/tunnel-pin", "200", "tunnel certificate pin is published");
checkStatus("/passport/edge-key", "200", "gateway key is published");
checkStatus("/manage/", "200", "manage page is served");

const reg = spawnSync("curl", ["-sS", "-m", "15", "-w", "\n%{http_code}", "-H", "content-type: application/json",
  "-d", '{"appliance_id":"live-edge-check","token":"not-a-real-token","hostname":"live-edge-check.invalid"}', `${edge}/register`], { encoding: "utf8" });
const regOut = (reg.stdout || "") + (reg.stderr || "");
const regCode = regOut.slice(regOut.lastIndexOf("\n") + 1).trim();
if (regCode === "401") pass("unknown box is refused (POST /register, 401)"); else fail(`POST /register with a bad token: wanted 401, got ${regCode}`);

if (Bun.env.SKIP_TUNNEL === "1") {
  console.log("skip  tunnel probes (SKIP_TUNNEL=1)");
} else {
  const tcp = spawnSync("timeout", ["8", "bash", "-c", `exec 3<>/dev/tcp/${host}/${tunnelPort}`], { stdio: "ignore" });
  if (tcp.status === 0) pass(`tunnel fallback door open (TCP ${tunnelPort})`); else fail(`tunnel fallback door closed or filtered (TCP ${tunnelPort})`);
  const py = spawnSync("python3", ["-I", "-", host, tunnelPort], { input: `import os, socket, sys
host, port = sys.argv[1], int(sys.argv[2])
pkt = bytes([0xC3]) + (0x1A2A3A4A).to_bytes(4, "big") + bytes([8]) + os.urandom(8) + bytes([8]) + os.urandom(8)
pkt += bytes(1200 - len(pkt))
s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
s.settimeout(5)
s.sendto(pkt, (host, port))
try:
    data, _ = s.recvfrom(2048)
    print("reply" if data[1:5] == bytes(4) else "other")
except OSError:
    print("silent")
`, encoding: "utf8" });
  const reply = (py.stdout || "").trim();
  if (reply === "reply") pass(`tunnel answers QUIC (UDP ${tunnelPort})`); else fail(`no QUIC answer on UDP ${tunnelPort} (${reply})`);
}

if (fails === 0) console.log(`\nedge ${edge}: all checks passed`);
else console.log(`\nedge ${edge}: ${fails} check(s) failed`);
process.exit(fails > 0 ? 1 : 0);
