#!/usr/bin/env bun
// Build the hosted copy of LosOS Lab. Shim target for build.sh.
import { chmodSync, cpSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { cpus } from "node:os";
import { spawnSync } from "node:child_process";

function sh(cmd: string[], opts: { cwd?: string; allowFail?: boolean; capture?: boolean } = {}) {
  const p = spawnSync(cmd[0], cmd.slice(1), { cwd: opts.cwd, stdio: opts.capture ? ["ignore", "pipe", "pipe"] : "inherit", encoding: "utf8", maxBuffer: 128 * 1024 * 1024 });
  if (p.status !== 0 && !opts.allowFail) { if (opts.capture) process.stderr.write((p.stderr||"")+(p.stdout||"")); process.exit(p.status ?? 1); }
  return p;
}

const outArg = Bun.argv[2];
if (!outArg) { console.error("usage: build.ts OUT"); process.exit(2); }
const OUT = resolve(outArg);
const HERE = import.meta.dir;
const LAB = dirname(HERE);
const ROOT = sh(["git", "-C", HERE, "rev-parse", "--show-toplevel"], { capture: true }).stdout.trim();
const WORK = mkdtempSync(join(tmpdir(), "losos-lab-build-"));
process.on("exit", () => {
  spawnSync("docker", ["rm", "-f", "losos-lab-qemu"], { stdio: "ignore" });
  try { rmSync(WORK, { recursive: true, force: true }); } catch { spawnSync("sudo", ["-n", "rm", "-rf", WORK], { stdio: "ignore" }); }
});

const QEMU_WASM_REV = "0ef7b4e2814b231705d8371dd7997f5b72e70baf";
const LINUX_VERSION = "6.1";
const BUSYBOX_TARBALL = "busybox_1.36.1.orig.tar.bz2";
const XTERM_VERSION = "5.3.0";
const XTERM_PTY_VERSION = "0.10.1";

mkdirSync(join(OUT, "diagrams"), { recursive: true });
const ENG = join(OUT, "diagrams");
for (const d of ["qemu/pc-bios", "guest", "vendor"]) mkdirSync(join(ENG, d), { recursive: true });

console.log(`== qemu-wasm (${QEMU_WASM_REV})`);
sh(["git", "init", "-q", join(WORK, "qemu-wasm")]);
sh(["git", "-C", join(WORK, "qemu-wasm"), "fetch", "-q", "--depth", "1", "https://github.com/ktock/qemu-wasm", QEMU_WASM_REV]);
sh(["git", "-C", join(WORK, "qemu-wasm"), "checkout", "-q", "FETCH_HEAD"]);
sh(["sed", "-i", 's|https://zlib.net/zlib-\\$ZLIB_VERSION.tar.xz|https://github.com/madler/zlib/releases/download/v$ZLIB_VERSION/zlib-$ZLIB_VERSION.tar.xz|', join(WORK, "qemu-wasm/Dockerfile")]);
sh(["grep", "-q", "madler/zlib/releases", join(WORK, "qemu-wasm/Dockerfile")]);
const dockerfile = readFileSync(join(WORK, "qemu-wasm/Dockerfile"), "utf8");
const db = spawnSync("docker", ["build", "-q", "-t", "losos-lab-qemu-env", "-"], { input: dockerfile, stdio: ["pipe", "inherit", "inherit"] });
if (db.status !== 0) process.exit(db.status ?? 1);
sh(["docker", "run", "-d", "--name", "losos-lab-qemu", "-v", `${join(WORK,"qemu-wasm")}:/qemu/`, "losos-lab-qemu-env"]);
const EXTRA_CFLAGS = "-O3 -Wno-error=unused-command-line-argument -matomics -mbulk-memory -DNDEBUG -DG_DISABLE_ASSERT -D_GNU_SOURCE -sASYNCIFY=1 -pthread -sPROXY_TO_PTHREAD=1 -sFORCE_FILESYSTEM -sALLOW_TABLE_GROWTH -sTOTAL_MEMORY=1024MB -sWASM_BIGINT -sMALLOC=mimalloc --js-library=/build/node_modules/xterm-pty/emscripten-pty.js -sEXPORT_ES6=1 -sASYNCIFY_IMPORTS=ffi_call_js";
sh(["docker", "exec", "losos-lab-qemu", "emconfigure", "/qemu/configure", "--static", "--target-list=x86_64-softmmu", "--cpu=wasm32", "--cross-prefix=", "--without-default-features", "--enable-system", "--with-coroutine=fiber", `--extra-cflags=${EXTRA_CFLAGS}`, `--extra-cxxflags=${EXTRA_CFLAGS}`, "--extra-ldflags=-sEXPORTED_RUNTIME_METHODS=getTempRet0,setTempRet0,addFunction,removeFunction,TTY,FS"]);
sh(["docker", "exec", "losos-lab-qemu", "emmake", "make", `-j${cpus().length}`, "qemu-system-x86_64"]);
sh(["docker", "cp", "losos-lab-qemu:/build/qemu-system-x86_64", join(ENG, "qemu/out.js")]);
sh(["docker", "cp", "losos-lab-qemu:/build/qemu-system-x86_64.wasm", join(ENG, "qemu/")]);
sh(["docker", "cp", "losos-lab-qemu:/build/qemu-system-x86_64.worker.js", join(ENG, "qemu/")]);
for (const f of ["bios-256k.bin", "vgabios-stdvga.bin", "kvmvapic.bin", "linuxboot_dma.bin", "efi-virtio.rom"]) cpSync(join(WORK,"qemu-wasm/pc-bios",f), join(ENG,"qemu/pc-bios/"));
console.log("== guest kernel and the LosOS stand-in rootfs");
const ctx = join(WORK, "guest");
cpSync(join(HERE, "guest"), ctx, { recursive: true });
sh(["bash", "-lc", `curl -sSfL "https://cdn.kernel.org/pub/linux/kernel/v6.x/linux-${LINUX_VERSION}.tar.xz" | tar xJ -C ${ctx}`]);
sh(["bash", "-lc", `mv "${ctx}/linux-${LINUX_VERSION}" "${ctx}/linux"`]);
sh(["curl", "-sSfL", "-o", join(ctx, BUSYBOX_TARBALL), `http://archive.ubuntu.com/ubuntu/pool/main/b/busybox/${BUSYBOX_TARBALL}`]);
sh(["docker", "build", "-q", "-o", join(WORK, "guest-out"), ctx]);
cpSync(join(WORK,"guest-out/bzImage"), join(ENG,"guest/")); cpSync(join(WORK,"guest-out/rootfs.bin"), join(ENG,"guest/"));
console.log("== Netzgeräte Betriebssystem (nix, against the flake's own nixpkgs)");
const NIXPKGS = sh(["nix", "eval", "--raw", "--impure", "--expr", `let lock = builtins.fromJSON (builtins.readFile "${ROOT}/flake.lock"); in (builtins.fetchTree lock.nodes.\${lock.nodes.root.inputs.nixpkgs}.locked).outPath`], { capture: true }).stdout.trim();
sh(["nix-build", join(HERE, "gear/netzgeraete.nix"), "--arg", "nixpkgs", NIXPKGS, "-o", join(WORK, "gear")]);
sh(["install", "-m", "0644", join(WORK, "gear/rootfs.bin"), join(ENG, "guest/gear.bin")]);
console.log(`== xterm ${XTERM_VERSION}, xterm-pty ${XTERM_PTY_VERSION}`);
sh(["curl", "-fsSL", "-o", join(WORK, `xterm-${XTERM_VERSION}.tgz`), `https://registry.npmjs.org/xterm/-/xterm-${XTERM_VERSION}.tgz`]);
sh(["curl", "-fsSL", "-o", join(WORK, `xterm-pty-${XTERM_PTY_VERSION}.tgz`), `https://registry.npmjs.org/xterm-pty/-/xterm-pty-${XTERM_PTY_VERSION}.tgz`]);
mkdirSync(join(WORK,"xterm"),{recursive:true}); mkdirSync(join(WORK,"xterm-pty"),{recursive:true});
sh(["tar","xzf",join(WORK,`xterm-${XTERM_VERSION}.tgz`),"-C",join(WORK,"xterm")]);
sh(["tar","xzf",join(WORK,`xterm-pty-${XTERM_PTY_VERSION}.tgz`),"-C",join(WORK,"xterm-pty")]);
cpSync(join(WORK,"xterm/package/lib/xterm.js"), join(ENG,"vendor/")); cpSync(join(WORK,"xterm/package/css/xterm.css"), join(ENG,"vendor/"));
cpSync(join(WORK,"xterm-pty/package/index.js"), join(ENG,"vendor/xterm-pty.js"));
console.log("== the page");
const APP = join(ROOT, "admin-ui/app");
for (const pkg of ["core","virt","render"]) {
  sh(["nix", "build", `${ROOT}#losos-lab-${pkg}`, "--out-link", join(WORK, `lab-${pkg}`)]);
  rmSync(join(APP, `src/lab/${pkg}-pkg`), { recursive: true, force: true });
  mkdirSync(join(APP, `src/lab/${pkg}-pkg`), { recursive: true });
  cpSync(join(WORK, `lab-${pkg}`), join(APP, `src/lab/${pkg}-pkg`), { recursive: true });
  chmodSync(join(APP, `src/lab/${pkg}-pkg`), 0o755);
}
sh(["nix","build", `${ROOT}#losos-mdx-wasm`, "--out-link", join(WORK,"mdx")]);
rmSync(join(APP,"src/mdx/pkg"), { recursive: true, force: true });
mkdirSync(join(APP,"src/mdx/pkg"), { recursive: true });
cpSync(join(WORK,"mdx"), join(APP,"src/mdx/pkg"), { recursive: true });
sh(["bash","-lc", `cd ${APP} && bun install --frozen-lockfile --ignore-scripts && VITE_LAB_HOSTED=1 bun run build && cp -r dist ${WORK}/dist`]);
cpSync(join(WORK,"dist/diagrams"), join(OUT,"diagrams"), { recursive: true });
cpSync(join(WORK,"dist/assets"), join(OUT,"assets"), { recursive: true });
cpSync(join(WORK,"dist/theme-boot.js"), join(OUT,"theme-boot.js"));
cpSync(join(LAB,"vercel.json"), join(OUT,"vercel.json")); cpSync(join(LAB,"serve.json"), join(OUT,"serve.json"));
sh(["du","-sh", OUT]);
