#!/usr/bin/env python3
"""Boot the installer ISO in QEMU and wait until losos-install is running.

    tests/iso-boot.py --iso result/iso/*.iso --firmware uefi --media usb

This is the check that the medium actually boots, which `nix build` of the
ISO cannot tell you: v0.1.1 built green and was then reported "not bootable in
UEFI". It drives the *shipped* image unchanged, so it needs a signal that the
image already gives off without a serial console or a test backdoor.

That signal is the network. Booting the ISO autologs root in on tty1, whose
login shell is losos-install; the installer picks a target disk and then
resolves the flake's host (github.com) before cloning it. So the first DNS
query for that host proves the whole chain: firmware loaded the bootloader,
the bootloader loaded the kernel and initrd, stage 2 came up, getty logged
root in, and the installer got as far as the network. QEMU's filter-dump
writes every frame the guest sends to a pcap, and this script polls it.

The guest network cannot be `restrict=on`: QEMU then leaves the DNS server
out of its DHCP offer, the guest has no resolver to ask, and no query is ever
sent. So the query is answered, and the VM is killed within one poll (2 s)
of it, before the installer has done more than start a clone of the public
repo. The disk it would format is a sparse temp file thrown away afterwards.

Firmware:
  uefi       OVMF with Secure Boot off
  bios       SeaBIOS (QEMU's default)
  uefi-sb    OVMF with Secure Boot on and Microsoft's keys enrolled. The ISO is
             not signed, so this is expected to fail until it is; it is here so
             the failure can be shown rather than described.

On timeout it saves a screenshot next to the pcap so CI can upload what the
screen showed.
"""

import argparse
import os
import shutil
import socket
import struct
import subprocess
import sys
import tempfile
import time
import zlib

# Ubuntu's ovmf package. Overridable because other distros put it elsewhere.
OVMF_DIR = os.environ.get("OVMF_DIR", "/usr/share/OVMF")
FIRMWARE = {
    "uefi": ("OVMF_CODE_4M.fd", "OVMF_VARS_4M.fd"),
    "uefi-sb": ("OVMF_CODE_4M.secboot.fd", "OVMF_VARS_4M.ms.fd"),
    "bios": None,
}


def dns_queries(pcap: bytes):
    """Yield the query name of every DNS query in a pcap of Ethernet frames."""
    if len(pcap) < 24:
        return
    magic = pcap[:4]
    endian = "<" if magic in (b"\xd4\xc3\xb2\xa1", b"\x4d\x3c\xb2\xa1") else ">"
    off = 24
    while off + 16 <= len(pcap):
        incl = struct.unpack(endian + "I", pcap[off + 8 : off + 12])[0]
        frame = pcap[off + 16 : off + 16 + incl]
        off += 16 + incl
        ethertype, ip = frame[12:14], frame[14:]
        if ethertype == b"\x08\x00" and len(ip) >= 20 and ip[9] == 17:  # IPv4 UDP
            udp = ip[(ip[0] & 0x0F) * 4 :]
        elif ethertype == b"\x86\xdd" and len(ip) >= 40 and ip[6] == 17:  # IPv6 UDP
            udp = ip[40:]
        else:
            continue
        if len(udp) < 8 + 12:
            continue
        if struct.unpack(">H", udp[2:4])[0] != 53:
            continue
        dns = udp[8:]
        if len(dns) < 12 or dns[2] & 0x80:  # a response, not a query
            continue
        labels, i = [], 12
        while i < len(dns) and dns[i] != 0 and dns[i] < 64:
            labels.append(dns[i + 1 : i + 1 + dns[i]].decode("ascii", "replace"))
            i += 1 + dns[i]
        yield ".".join(labels).lower()


def screendump(monitor: str, out: str) -> None:
    """Ask QEMU's monitor for a screenshot and write it as a PNG."""
    ppm = out + ".ppm"
    s = socket.socket(socket.AF_UNIX)
    s.connect(monitor)
    s.settimeout(5)
    time.sleep(0.5)
    s.recv(4096)
    s.sendall(f"screendump {ppm}\n".encode())
    for _ in range(20):
        time.sleep(0.5)
        if os.path.exists(ppm) and os.path.getsize(ppm) > 0:
            break
    s.close()
    data = open(ppm, "rb").read()
    head = data.split(b"\n", 3)
    w, h = map(int, head[1].split())
    px = head[3]
    rows = b"".join(b"\0" + px[y * w * 3 : (y + 1) * w * 3] for y in range(h))

    def chunk(tag, body):
        crc = zlib.crc32(tag + body) & 0xFFFFFFFF
        return struct.pack(">I", len(body)) + tag + body + struct.pack(">I", crc)

    ihdr = struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0)
    with open(out, "wb") as f:
        f.write(b"\x89PNG\r\n\x1a\n")
        f.write(chunk(b"IHDR", ihdr) + chunk(b"IDAT", zlib.compress(rows)) + chunk(b"IEND", b""))
    os.unlink(ppm)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--iso", required=True)
    ap.add_argument("--firmware", choices=sorted(FIRMWARE), default="uefi")
    ap.add_argument("--media", choices=["usb", "cdrom"], default="usb")
    ap.add_argument("--host", default="github.com", help="the flake host the installer resolves")
    ap.add_argument("--timeout", type=int, default=600, help="seconds")
    ap.add_argument("--out", default=".", help="where the pcap and a failure screenshot go")
    args = ap.parse_args()

    name = f"{args.firmware}-{args.media}"
    os.makedirs(args.out, exist_ok=True)
    pcap = os.path.join(args.out, f"{name}.pcap")
    shot = os.path.join(args.out, f"{name}.png")
    work = tempfile.mkdtemp(prefix=f"iso-boot-{name}-")
    monitor = os.path.join(work, "monitor.sock")
    disk = os.path.join(work, "scratch.img")
    # Over the installer's 1 GB floor, so drive detection picks it and moves
    # on to the network rather than stopping at "no candidate fixed disks".
    with open(disk, "wb") as f:
        f.truncate(4 << 30)

    kvm = os.access("/dev/kvm", os.R_OK | os.W_OK)
    cmd = ["qemu-system-x86_64", "-m", "3072", "-smp", "2", "-display", "none", "-vga", "std"]
    cmd += ["-accel", "kvm"] if kvm else ["-accel", "tcg"]
    fw = FIRMWARE[args.firmware]
    if fw is None:
        cmd += ["-machine", "q35"]
    else:
        code, vars_ = (os.path.join(OVMF_DIR, f) for f in fw)
        local_vars = os.path.join(work, "vars.fd")
        shutil.copyfile(vars_, local_vars)
        # Secure Boot in OVMF needs SMM, and SMM needs the flash marked secure.
        cmd += ["-machine", "q35,smm=on", "-global", "driver=cfi.pflash01,property=secure,value=on"]
        cmd += ["-drive", f"if=pflash,format=raw,readonly=on,file={code}"]
        cmd += ["-drive", f"if=pflash,format=raw,file={local_vars}"]
    if args.media == "usb":
        cmd += ["-drive", f"if=none,id=stick,format=raw,readonly=on,file={args.iso}"]
        cmd += ["-device", "qemu-xhci", "-device", "usb-storage,drive=stick,bootindex=0"]
    else:
        cmd += ["-drive", f"if=none,id=cd,format=raw,readonly=on,media=cdrom,file={args.iso}"]
        cmd += ["-device", "ide-cd,drive=cd,bootindex=0"]
    cmd += ["-drive", f"if=virtio,format=raw,file={disk}"]
    cmd += ["-netdev", "user,id=n0", "-device", "virtio-net-pci,netdev=n0"]
    cmd += ["-object", f"filter-dump,id=f0,netdev=n0,file={pcap}"]
    cmd += ["-monitor", f"unix:{monitor},server,nowait"]

    print(f"{name}: booting {args.iso} ({'kvm' if kvm else 'tcg, slow'})", flush=True)
    qemu = subprocess.Popen(cmd)
    start = time.monotonic()
    want = args.host.lower()
    try:
        while time.monotonic() - start < args.timeout:
            time.sleep(2)
            if qemu.poll() is not None:
                print(f"{name}: qemu exited with {qemu.returncode} before the installer ran", file=sys.stderr)
                return 1
            if os.path.exists(pcap):
                seen = set(dns_queries(open(pcap, "rb").read()))
                if want in seen:
                    took = time.monotonic() - start
                    print(f"{name}: OK, the installer resolved {want} {took:.0f}s after power-on")
                    return 0
        print(f"{name}: no DNS query for {want} within {args.timeout}s; screenshot in {shot}", file=sys.stderr)
        try:
            screendump(monitor, shot)
        except OSError as e:
            print(f"{name}: could not take a screenshot: {e}", file=sys.stderr)
        return 1
    finally:
        qemu.kill()
        qemu.wait()
        shutil.rmtree(work, ignore_errors=True)


if __name__ == "__main__":
    sys.exit(main())
