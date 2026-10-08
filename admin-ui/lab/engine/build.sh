#!/usr/bin/env bash
# Build the hosted copy of LosOS Lab: the page plus the qemu-wasm engine and
# the three guest files, ready for any server that sends the two
# cross-origin isolation headers (vercel.json here, serve.py locally).
#
#   admin-ui/lab/engine/build.sh OUT
#
# Needs docker, nix, node/npm, python3 and git; .github/workflows/lab.yml runs
# it on a stock ubuntu runner. It takes a while: qemu-wasm compiles QEMU and
# its libraries with emscripten, and the guest kernel is a real Linux build.
#
# What comes out:
#   OUT/index.html lab.js lab.css plate.png     build.py hosted
#   OUT/qemu/out.js, *.wasm, *.worker.js         qemu-system-x86_64 for wasm32
#   OUT/qemu/pc-bios/                            SeaBIOS and the option ROMs
#   OUT/guest/bzImage                            Linux 6.1 for every guest
#   OUT/guest/rootfs.bin                         the LosOS stand-in (busybox)
#   OUT/guest/gear.bin                           Netzgeräte Betriebssystem (nix)
#   OUT/vendor/xterm.js xterm.css xterm-pty.js   the consoles
#   OUT/vercel.json                              COOP/COEP on every path
set -euo pipefail

OUT=$(realpath -m "${1:?usage: build.sh OUT}")
HERE=$(cd "$(dirname "$0")" && pwd)
LAB=$(dirname "$HERE")
ROOT=$(git -C "$HERE" rev-parse --show-toplevel)
WORK=$(mktemp -d)
trap 'docker rm -f losos-lab-qemu >/dev/null 2>&1 || true; rm -rf "$WORK"' EXIT

# Pinned: the revision the simulator was written and tested against.
QEMU_WASM_REV=0ef7b4e2814b231705d8371dd7997f5b72e70baf
LINUX_VERSION=6.1
BUSYBOX_TARBALL=busybox_1.36.1.orig.tar.bz2
XTERM_VERSION=5.3.0
XTERM_PTY_VERSION=0.10.1

mkdir -p "$OUT"/{qemu/pc-bios,guest,vendor}

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
docker run -d --name losos-lab-qemu -v "$WORK/qemu-wasm":/qemu/:ro losos-lab-qemu-env >/dev/null
EXTRA_CFLAGS="-O3 -Wno-error=unused-command-line-argument -matomics -mbulk-memory -DNDEBUG -DG_DISABLE_ASSERT -D_GNU_SOURCE -sASYNCIFY=1 -pthread -sPROXY_TO_PTHREAD=1 -sFORCE_FILESYSTEM -sALLOW_TABLE_GROWTH -sTOTAL_MEMORY=1024MB -sWASM_BIGINT -sMALLOC=mimalloc --js-library=/build/node_modules/xterm-pty/emscripten-pty.js -sEXPORT_ES6=1 -sASYNCIFY_IMPORTS=ffi_call_js"
docker exec losos-lab-qemu emconfigure /qemu/configure --static --target-list=x86_64-softmmu --cpu=wasm32 --cross-prefix= \
  --without-default-features --enable-system --with-coroutine=fiber \
  --extra-cflags="$EXTRA_CFLAGS" --extra-cxxflags="$EXTRA_CFLAGS" \
  --extra-ldflags="-sEXPORTED_RUNTIME_METHODS=getTempRet0,setTempRet0,addFunction,removeFunction,TTY,FS"
docker exec losos-lab-qemu emmake make -j"$(nproc)" qemu-system-x86_64
docker cp losos-lab-qemu:/build/qemu-system-x86_64 "$OUT/qemu/out.js"
docker cp losos-lab-qemu:/build/qemu-system-x86_64.wasm "$OUT/qemu/"
docker cp losos-lab-qemu:/build/qemu-system-x86_64.worker.js "$OUT/qemu/"
for f in bios-256k.bin vgabios-stdvga.bin kvmvapic.bin linuxboot_dma.bin efi-virtio.rom; do
  cp "$WORK/qemu-wasm/pc-bios/$f" "$OUT/qemu/pc-bios/"
done

echo "== guest kernel and the LosOS stand-in rootfs"
ctx="$WORK/guest"
cp -r "$HERE/guest" "$ctx"
curl -sSfL "https://cdn.kernel.org/pub/linux/kernel/v6.x/linux-$LINUX_VERSION.tar.xz" | tar xJ -C "$ctx"
mv "$ctx/linux-$LINUX_VERSION" "$ctx/linux"
curl -sSfL -o "$ctx/$BUSYBOX_TARBALL" "http://archive.ubuntu.com/ubuntu/pool/main/b/busybox/$BUSYBOX_TARBALL"
docker build -q -o "$WORK/guest-out" "$ctx"
cp "$WORK/guest-out/bzImage" "$WORK/guest-out/rootfs.bin" "$OUT/guest/"

echo "== Netzgeräte Betriebssystem (nix, against the flake's own nixpkgs)"
NIXPKGS=$(nix eval --raw --impure --expr "(builtins.getFlake \"git+file://$ROOT\").inputs.nixpkgs.outPath")
nix-build "$HERE/gear/netzgeraete.nix" --arg nixpkgs "$NIXPKGS" -o "$WORK/gear"
install -m 0644 "$WORK/gear/rootfs.bin" "$OUT/guest/gear.bin"

echo "== xterm $XTERM_VERSION, xterm-pty $XTERM_PTY_VERSION"
(cd "$WORK" && npm pack -s "xterm@$XTERM_VERSION" "xterm-pty@$XTERM_PTY_VERSION" >/dev/null)
mkdir -p "$WORK/xterm" "$WORK/xterm-pty"
tar xzf "$WORK/xterm-$XTERM_VERSION.tgz" -C "$WORK/xterm"
tar xzf "$WORK/xterm-pty-$XTERM_PTY_VERSION.tgz" -C "$WORK/xterm-pty"
cp "$WORK/xterm/package/lib/xterm.js" "$WORK/xterm/package/css/xterm.css" "$OUT/vendor/"
cp "$WORK/xterm-pty/package/index.js" "$OUT/vendor/xterm-pty.js"

echo "== the page"
python3 "$LAB/build.py" hosted "$OUT"
cp "$LAB/vercel.json" "$OUT/vercel.json"
du -sh "$OUT"
