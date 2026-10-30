#!/usr/bin/env bash
# Build the hosted copy of LosOS Lab: the page plus the qemu-wasm engine and
# the three guest files, ready for any server that sends the two
# cross-origin isolation headers: vercel.json on Vercel, serve.json for
# `bunx serve@14.2.4 OUT` on your own machine.
#
#   admin-ui/lab/engine/build.sh OUT
#
# Needs docker, nix, bun, curl and git; .github/workflows/lab.yml runs it on a
# stock ubuntu runner. It takes a while: qemu-wasm compiles QEMU and its
# libraries with emscripten, and the guest kernel is a real Linux build.
#
# What comes out (the page lives at /diagrams/, as on the box):
#   OUT/diagrams/index.html, OUT/assets/, theme-boot.js   the admin UI's Lab page,
#                                                    VITE_LAB_HOSTED=1
#   OUT/diagrams/qemu/out.js, *.wasm, *.worker.js     qemu-system-x86_64 for wasm32
#   OUT/diagrams/qemu/pc-bios/                        SeaBIOS and the option ROMs
#   OUT/diagrams/guest/bzImage                        Linux 6.1 for every guest
#   OUT/diagrams/guest/rootfs.bin                     the LosOS stand-in (busybox)
#   OUT/diagrams/guest/gear.bin                       Netzgeräte Betriebssystem (nix)
#   OUT/diagrams/vendor/xterm.js xterm.css xterm-pty.js   the consoles
#   OUT/vercel.json                              COOP/COEP on every path
set -euo pipefail

OUT=$(realpath -m "${1:?usage: build.sh OUT}")
HERE=$(cd "$(dirname "$0")" && pwd)
LAB=$(dirname "$HERE")
ROOT=$(git -C "$HERE" rev-parse --show-toplevel)
WORK=$(mktemp -d)
# The container writes into $WORK as root (meson's subprojects), so the
# cleanup must not fail the run over files the runner user cannot remove.
trap 'docker rm -f losos-lab-qemu >/dev/null 2>&1 || true; rm -rf "$WORK" 2>/dev/null || sudo -n rm -rf "$WORK" 2>/dev/null || true' EXIT

# Pinned: the revision the simulator was written and tested against.
QEMU_WASM_REV=0ef7b4e2814b231705d8371dd7997f5b72e70baf
LINUX_VERSION=6.1
BUSYBOX_TARBALL=busybox_1.36.1.orig.tar.bz2
XTERM_VERSION=5.3.0
XTERM_PTY_VERSION=0.10.1

mkdir -p "$OUT"/diagrams
ENG="$OUT/diagrams"
mkdir -p "$ENG"/{qemu/pc-bios,guest,vendor}

echo "== qemu-wasm ($QEMU_WASM_REV)"
git init -q "$WORK/qemu-wasm"
git -C "$WORK/qemu-wasm" fetch -q --depth 1 https://github.com/ktock/qemu-wasm "$QEMU_WASM_REV"
git -C "$WORK/qemu-wasm" checkout -q FETCH_HEAD
# Upstream's own Dockerfile: emsdk plus zlib, glib, libffi, pixman and
# xterm-pty built for wasm32. Its README configures and makes inside it.
# zlib.net keeps only the newest release, so the pinned zlib comes from the
# project's GitHub release instead.
sed -i 's|https://zlib.net/zlib-\$ZLIB_VERSION.tar.xz|https://github.com/madler/zlib/releases/download/v$ZLIB_VERSION/zlib-$ZLIB_VERSION.tar.xz|' \
  "$WORK/qemu-wasm/Dockerfile"
grep -q 'madler/zlib/releases' "$WORK/qemu-wasm/Dockerfile"
docker build -q -t losos-lab-qemu-env - < "$WORK/qemu-wasm/Dockerfile"
# Writable: configure clones the dtc, keycodemapdb and softfloat wraps into
# /qemu/subprojects on first use.
docker run -d --name losos-lab-qemu -v "$WORK/qemu-wasm":/qemu/ losos-lab-qemu-env >/dev/null
EXTRA_CFLAGS="-O3 -Wno-error=unused-command-line-argument -matomics -mbulk-memory -DNDEBUG -DG_DISABLE_ASSERT -D_GNU_SOURCE -sASYNCIFY=1 -pthread -sPROXY_TO_PTHREAD=1 -sFORCE_FILESYSTEM -sALLOW_TABLE_GROWTH -sTOTAL_MEMORY=1024MB -sWASM_BIGINT -sMALLOC=mimalloc --js-library=/build/node_modules/xterm-pty/emscripten-pty.js -sEXPORT_ES6=1 -sASYNCIFY_IMPORTS=ffi_call_js"
docker exec losos-lab-qemu emconfigure /qemu/configure --static --target-list=x86_64-softmmu --cpu=wasm32 --cross-prefix= \
  --without-default-features --enable-system --with-coroutine=fiber \
  --extra-cflags="$EXTRA_CFLAGS" --extra-cxxflags="$EXTRA_CFLAGS" \
  --extra-ldflags="-sEXPORTED_RUNTIME_METHODS=getTempRet0,setTempRet0,addFunction,removeFunction,TTY,FS"
docker exec losos-lab-qemu emmake make -j"$(nproc)" qemu-system-x86_64
docker cp losos-lab-qemu:/build/qemu-system-x86_64 "$ENG/qemu/out.js"
docker cp losos-lab-qemu:/build/qemu-system-x86_64.wasm "$ENG/qemu/"
docker cp losos-lab-qemu:/build/qemu-system-x86_64.worker.js "$ENG/qemu/"
for f in bios-256k.bin vgabios-stdvga.bin kvmvapic.bin linuxboot_dma.bin efi-virtio.rom; do
  cp "$WORK/qemu-wasm/pc-bios/$f" "$ENG/qemu/pc-bios/"
done

echo "== guest kernel and the LosOS stand-in rootfs"
ctx="$WORK/guest"
cp -r "$HERE/guest" "$ctx"
curl -sSfL "https://cdn.kernel.org/pub/linux/kernel/v6.x/linux-$LINUX_VERSION.tar.xz" | tar xJ -C "$ctx"
mv "$ctx/linux-$LINUX_VERSION" "$ctx/linux"
curl -sSfL -o "$ctx/$BUSYBOX_TARBALL" "http://archive.ubuntu.com/ubuntu/pool/main/b/busybox/$BUSYBOX_TARBALL"
docker build -q -o "$WORK/guest-out" "$ctx"
cp "$WORK/guest-out/bzImage" "$WORK/guest-out/rootfs.bin" "$ENG/guest/"

echo "== Netzgeräte Betriebssystem (nix, against the flake's own nixpkgs)"
# The pin is read from flake.lock rather than through getFlake: CI checks the
# repository out shallow, and a git+file flake of a shallow clone has no revCount.
NIXPKGS=$(nix eval --raw --impure --expr "let lock = builtins.fromJSON (builtins.readFile \"$ROOT/flake.lock\"); in (builtins.fetchTree lock.nodes.\${lock.nodes.root.inputs.nixpkgs}.locked).outPath")
nix-build "$HERE/gear/netzgeraete.nix" --arg nixpkgs "$NIXPKGS" -o "$WORK/gear"
install -m 0644 "$WORK/gear/rootfs.bin" "$ENG/guest/gear.bin"

echo "== xterm $XTERM_VERSION, xterm-pty $XTERM_PTY_VERSION"
curl -fsSL -o "$WORK/xterm-$XTERM_VERSION.tgz" "https://registry.npmjs.org/xterm/-/xterm-$XTERM_VERSION.tgz"
curl -fsSL -o "$WORK/xterm-pty-$XTERM_PTY_VERSION.tgz" "https://registry.npmjs.org/xterm-pty/-/xterm-pty-$XTERM_PTY_VERSION.tgz"
mkdir -p "$WORK/xterm" "$WORK/xterm-pty"
tar xzf "$WORK/xterm-$XTERM_VERSION.tgz" -C "$WORK/xterm"
tar xzf "$WORK/xterm-pty-$XTERM_PTY_VERSION.tgz" -C "$WORK/xterm-pty"
cp "$WORK/xterm/package/lib/xterm.js" "$WORK/xterm/package/css/xterm.css" "$ENG/vendor/"
cp "$WORK/xterm-pty/package/index.js" "$ENG/vendor/xterm-pty.js"

echo "== the page"
# The Lab page is admin-ui/app's second Vite entry. Its Rust core, its
# libvirt client and its Bevy canvas (one webgpu/ and one webgl2/ module)
# come from nix (the same derivations the box's build copies in), so the
# runner needs no wasm32 toolchain.
APP="$ROOT/admin-ui/app"
for pkg in core virt render; do
  nix build "$ROOT#losos-lab-$pkg" --out-link "$WORK/lab-$pkg"
  rm -rf "$APP/src/lab/$pkg-pkg"
  mkdir -p "$APP/src/lab/$pkg-pkg"
  cp -r "$WORK/lab-$pkg"/* "$APP/src/lab/$pkg-pkg/"
  chmod -R u+w "$APP/src/lab/$pkg-pkg"
done
# losos-mdx, which the page's documents are drawn with, the same way.
nix build "$ROOT#losos-mdx-wasm" --out-link "$WORK/mdx"
rm -rf "$APP/src/mdx/pkg"
mkdir -p "$APP/src/mdx/pkg"
cp -r "$WORK/mdx"/* "$APP/src/mdx/pkg/"
chmod -R u+w "$APP/src/mdx/pkg"
# The main build only: `bun run build --outDir` would hand the flags to
# the guest call page's build, the script's last step, which then empties
# the folder Diagrams was just built into.
(cd "$APP" && bun install --frozen-lockfile --ignore-scripts && bun run typecheck && VITE_LAB_HOSTED=1 bun --bun vite build --outDir "$WORK/dist" --emptyOutDir)
cp -r "$WORK/dist/diagrams/." "$OUT/diagrams/"
cp -r "$WORK/dist/assets" "$WORK/dist/theme-boot.js" "$OUT/"
cp "$LAB/vercel.json" "$LAB/serve.json" "$OUT/"
du -sh "$OUT"
