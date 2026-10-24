"""Does each computer find the box on its own network, and open its files?

CI's discovery jobs (.github/workflows/ci.yml) run this on a Linux, a
macOS and a Windows runner. It boots the test box (nix/tests/discovery/box.nix)
in QEMU on a link shared with the runner, claims it as its owner, and then
asks the runner's own operating system, with the tools a person's computer
uses:

  found       the box is listed: Bonjour or Avahi browse `_smb._tcp`, and on
              Windows File Explorer's Network view (WS-Discovery)
  SMB         the owner's share mounts with their LosOS name and password; a
              file written there is read back, and shows up in LosOS Cloud
  WebDAV      LosOS Cloud's WebDAV address mounts (or, where the runner has
              no WebDAV client, answers PROPFIND, PUT and GET); a file
              written there is read back over SMB
  off         with the share switched off, the SMB entry and port are gone
              and Bonjour and Avahi list LosOS Cloud's `_webdavs._tcp` entry
              in its place, which still opens

Every step prints what it saw; the first that fails ends the run non-zero.

    python3 check.py --os linux|macos|windows --box DIR --netdev ARGS [--kvm]
"""

import argparse
import base64
import json
import os
import platform
import re
import shutil
import socket
import ssl
import subprocess
import sys
import tempfile
import time
import urllib.parse
import urllib.request
from pathlib import Path

HOST = "mattbox"
LOCAL = f"{HOST}.local"
BOX_IP = "192.168.77.2"
OWNER = "matus"
PASSWORD = "Correct-Horse-Battery-9"
SERIAL_PORT = 4555
WORK = Path(tempfile.mkdtemp(prefix="losos-discovery-"))


def say(msg):
    print(f"[{time.strftime('%H:%M:%S')}] {msg}", flush=True)


def run(cmd, check=True, timeout=300, quiet=False, **kw):
    shell = isinstance(cmd, str)
    if not quiet:
        say(f"$ {cmd if shell else ' '.join(cmd)}")
    p = subprocess.run(cmd, shell=shell, capture_output=True, text=True, timeout=timeout, **kw)
    out = (p.stdout or "") + (p.stderr or "")
    if not quiet and out.strip():
        print(out.rstrip(), flush=True)
    if check and p.returncode != 0:
        raise SystemExit(f"failed ({p.returncode}): {cmd}")
    return p.returncode, out


def until(what, fn, timeout=600, every=5):
    """Calls fn until it returns something truthy; fails after timeout."""
    end = time.time() + timeout
    last = None
    while time.time() < end:
        try:
            got = fn()
            if got:
                say(f"ok: {what}")
                return got
        except Exception as e:  # noqa: BLE001 - every attempt may fail until it does not
            last = e
        time.sleep(every)
    raise SystemExit(f"timed out: {what} (last error: {last})")


class Serial:
    """The box's root shell on its serial port, for what no client can do."""

    def __init__(self):
        self.sock = until(
            "serial console",
            lambda: socket.create_connection(("127.0.0.1", SERIAL_PORT), timeout=5),
            timeout=120,
            every=2,
        )
        self.sock.settimeout(2)
        self.buf = b""

    def read(self):
        try:
            chunk = self.sock.recv(65536)
            self.buf += chunk
            return chunk
        except socket.timeout:
            return b""

    def sh(self, cmd, timeout=600):
        """Runs cmd as root on the box and returns its output."""
        tag = f"M{int(time.time() * 1000)}"
        line = f"echo {tag}B; ( {cmd} ) 2>&1 | base64 -w0; echo; echo {tag}E $?\n"
        self.buf = b""
        self.sock.sendall(line.encode())
        end = time.time() + timeout
        pat = re.compile(rf"{tag}B\r?\n([A-Za-z0-9+/=\r\n]*)\r?\n{tag}E (\d+)".encode())
        while time.time() < end:
            self.read()
            m = pat.search(self.buf)
            if m:
                return base64.b64decode(m.group(1).replace(b"\r", b"").replace(b"\n", b"")).decode(errors="replace")
        raise SystemExit(f"timed out on the box: {cmd}")

    def wait_shell(self, timeout=1800):
        def ready():
            self.sock.sendall(b"\n")
            time.sleep(2)
            self.read()
            return b"root@" in self.buf

        until("root shell on the box's serial port", ready, timeout=timeout, every=10)


def boot(args):
    box = Path(args.box)
    cmdline = (box / "cmdline").read_text().strip()
    accel = "kvm" if args.kvm else "tcg,thread=multi"
    cmd = [
        args.qemu,
        "-machine", "q35",
        "-accel", accel,
        "-cpu", "host" if args.kvm else "max",
        "-smp", str(args.cpus),
        "-m", "4096",
        "-kernel", str(box / "bzImage"),
        "-initrd", str(box / "initrd"),
        "-append", cmdline,
        "-display", "none",
        "-serial", f"tcp:127.0.0.1:{SERIAL_PORT},server=on,wait=off",
        "-netdev", args.netdev,
        "-device", "virtio-net-pci,netdev=lan,mac=52:54:00:77:00:02",
    ]
    if args.sudo:
        cmd = ["sudo"] + cmd
    say("booting the box: " + " ".join(cmd))
    log = open(WORK / "qemu.log", "w")
    return subprocess.Popen(cmd, stdout=log, stderr=subprocess.STDOUT)


def https_ctx(ca):
    ctx = ssl.create_default_context(cafile=str(ca))
    # Python 3.13 checks RFC 5280 to the letter by default; the box's
    # certificate is its own issuer, which browsers and the three systems
    # accept once trusted.
    ctx.verify_flags &= ~getattr(ssl, "VERIFY_X509_STRICT", 0)
    return ctx


def http(method, url, ca, body=None, auth=None, headers=None):
    h = dict(headers or {})
    data = None
    if body is not None:
        data = body if isinstance(body, bytes) else json.dumps(body).encode()
        h.setdefault("Content-Type", "application/json")
    if auth:
        h["Authorization"] = "Basic " + base64.b64encode(f"{auth[0]}:{auth[1]}".encode()).decode()
    req = urllib.request.Request(url, data=data, method=method, headers=h)
    try:
        with urllib.request.urlopen(req, context=https_ctx(ca), timeout=60) as r:
            return r.status, r.read()
    except urllib.error.HTTPError as e:
        return e.code, e.read()


# --- the three operating systems -------------------------------------------


class Linux:
    def link_up(self):
        return True

    def dav_client(self):
        return True

    smb_mount = Path("/mnt/losos-smb")
    dav_mount = Path("/mnt/losos-dav")

    def trust(self, ca):
        self.ca = ca
        run(f"sudo cp {ca} /usr/local/share/ca-certificates/losos-box.crt && sudo update-ca-certificates")

    def resolve(self):
        _, out = run(f"getent hosts {LOCAL}", check=False)
        return BOX_IP in out

    def browse(self, kind):
        _, out = run(f"avahi-browse -prt {kind}", check=False, timeout=60)
        return [l for l in out.splitlines() if l.startswith("=") and "IPv4" in l]

    def found(self):
        smb = [l for l in self.browse("_smb._tcp") if f";{HOST};" in l]
        dav = [l for l in self.browse("_webdavs._tcp") if HOST in l]
        say(f"Avahi: {len(smb)} SMB entry, {len(dav)} WebDAV entry for {HOST}")
        # wsdd's discovery client sends the probe File Explorer sends.
        _, wsd = run(
            "rm -f /tmp/wsdd.sock; timeout 20 wsdd --discovery --no-host --listen /tmp/wsdd.sock & sleep 12;"
            " echo list | socat -t 5 - UNIX-CONNECT:/tmp/wsdd.sock; wait",
            check=False,
            timeout=60,
        )
        return len(smb) == 1 and not dav and HOST in wsd.lower()

    def off(self):
        smb = [l for l in self.browse("_smb._tcp") if f";{HOST};" in l]
        dav = [l for l in self.browse("_webdavs._tcp") if HOST in l]
        say(f"Avahi: {len(smb)} SMB entry, {len(dav)} WebDAV entry for {HOST}")
        return not smb and len(dav) == 1

    def mount_smb(self):
        run(f"sudo mkdir -p {self.smb_mount}")
        run(
            f"sudo mount -t cifs //{LOCAL}/{OWNER} {self.smb_mount}"
            f" -o username={OWNER},password={PASSWORD},vers=3.1.1,seal,uid=$(id -u),gid=$(id -g)"
        )
        return self.smb_mount

    def unmount_smb(self):
        run(f"sudo umount {self.smb_mount}", check=False)

    def mount_dav(self, url):
        run(f"sudo mkdir -p {self.dav_mount}")
        secrets = WORK / "davfs-secrets"
        secrets.write_text(f"{url} {OWNER} {PASSWORD}\n")
        run(f"sudo chown root:root {secrets} && sudo chmod 600 {secrets}")
        run(
            "printf 'use_locks 0\\nsecrets %s\\ntrust_server_cert %s\\n' "
            f"{secrets} {self.ca} | sudo tee /etc/davfs2/davfs2.conf >/dev/null"
        )
        run(f"sudo mount -t davfs {url} {self.dav_mount} -o uid=$(id -u),gid=$(id -g)", timeout=120)
        return self.dav_mount

    def unmount_dav(self):
        run(f"sudo umount {self.dav_mount}", check=False, timeout=120)

    def smb_closed(self):
        _, out = run(f"timeout 10 bash -c '</dev/tcp/{BOX_IP}/445' && echo open || echo closed", check=False)
        return "closed" in out


class MacOS:
    def link_up(self):
        return True

    def dav_client(self):
        return True

    smb_mount = Path.home() / "losos-smb"
    dav_mount = Path.home() / "losos-dav"

    def trust(self, ca):
        run(f"sudo security add-trusted-cert -d -r trustRoot -k /Library/Keychains/System.keychain {ca}")

    def resolve(self):
        _, out = run(f"dscacheutil -q host -a name {LOCAL}", check=False)
        return BOX_IP in out

    def browse(self, kind):
        # dns-sd never exits on its own.
        _, out = run(f"dns-sd -B {kind} local. & sleep 8; kill $!", check=False, timeout=60)
        return [l for l in out.splitlines() if " Add " in l]

    def found(self):
        smb = [l for l in self.browse("_smb._tcp") if l.rstrip().endswith(HOST)]
        dav = [l for l in self.browse("_webdavs._tcp") if HOST in l]
        say(f"Bonjour: {len(smb)} SMB entry, {len(dav)} WebDAV entry for {HOST}")
        return len(smb) == 1 and not dav

    def off(self):
        smb = [l for l in self.browse("_smb._tcp") if l.rstrip().endswith(HOST)]
        dav = [l for l in self.browse("_webdavs._tcp") if HOST in l]
        say(f"Bonjour: {len(smb)} SMB entry, {len(dav)} WebDAV entry for {HOST}")
        return not smb and len(dav) == 1

    def mount_smb(self):
        self.smb_mount.mkdir(exist_ok=True)
        pw = urllib.parse.quote(PASSWORD, safe="")
        run(f"mount_smbfs -N '//{OWNER}:{pw}@{LOCAL}/{OWNER}' {self.smb_mount}", quiet=True)
        say(f"mounted //{OWNER}@{LOCAL}/{OWNER} on {self.smb_mount}")
        run(f"smbutil statshares -m {self.smb_mount}", check=False)
        return self.smb_mount

    def unmount_smb(self):
        run(f"umount {self.smb_mount}", check=False)

    def mount_dav(self, url):
        self.dav_mount.mkdir(exist_ok=True)
        # mount_webdav reads the name and password from a descriptor: each
        # as a 4-byte length in network order followed by the bytes.
        import struct

        blob = b"".join(struct.pack(">I", len(x)) + x for x in (OWNER.encode(), PASSWORD.encode()))
        r, w = os.pipe()
        os.write(w, blob)
        os.close(w)
        say(f"$ mount_webdav -s -a {r} {url} {self.dav_mount}")
        p = subprocess.run(
            ["mount_webdav", "-s", "-a", str(r), url, str(self.dav_mount)],
            pass_fds=(r,),
            capture_output=True,
            text=True,
            timeout=180,
        )
        os.close(r)
        print((p.stdout + p.stderr).rstrip(), flush=True)
        if p.returncode != 0:
            raise SystemExit(f"mount_webdav failed ({p.returncode})")
        return self.dav_mount

    def unmount_dav(self):
        run(f"umount {self.dav_mount}", check=False, timeout=120)

    def smb_closed(self):
        _, out = run(f"nc -z -G 5 {BOX_IP} 445 && echo open || echo closed", check=False)
        return "closed" in out


class Windows:
    smb_drive = "S:"
    dav_drive = "W:"

    def ps(self, script, check=True, timeout=300):
        return run(["powershell", "-NoProfile", "-NonInteractive", "-Command", script], check=check, timeout=timeout)

    def trust(self, ca):
        run(["certutil", "-addstore", "-f", "Root", str(ca)])

    def resolve(self):
        _, out = self.ps(f"(Resolve-DnsName {LOCAL} -Type A -ErrorAction Stop).IPAddress", check=False)
        return BOX_IP in out

    def network_view(self):
        _, out = self.ps(
            "(New-Object -ComObject Shell.Application).NameSpace(18).Items() | ForEach-Object { $_.Name }",
            check=False,
            timeout=120,
        )
        return [l.strip() for l in out.splitlines() if l.strip()]

    def found(self):
        items = self.network_view()
        say(f"File Explorer, Network: {items}")
        return any(i.upper() == HOST.upper() for i in items)

    def off(self):
        items = self.network_view()
        say(f"File Explorer, Network: {items}")
        return not any(i.upper() == HOST.upper() for i in items)

    def mount_smb(self):
        run(["net", "use", self.smb_drive, f"\\\\{HOST.upper()}\\{OWNER}", PASSWORD, f"/user:{OWNER}", "/persistent:no"])
        self.ps(f"Get-SmbConnection | Format-List ServerName,ShareName,Dialect,UserName", check=False)
        return Path(self.smb_drive + "\\")

    def unmount_smb(self):
        run(["net", "use", self.smb_drive, "/delete", "/y"], check=False)

    def link_up(self):
        # The TAP adapter is an unidentified network, so Public, until QEMU
        # connects it; a computer at home calls its LAN Private, and only
        # then does File Explorer look for other computers on it.
        code, out = self.ps(
            "Set-NetConnectionProfile -InterfaceAlias losos-lan -NetworkCategory Private -ErrorAction Stop;"
            " (Get-NetConnectionProfile -InterfaceAlias losos-lan).NetworkCategory",
            check=False,
        )
        return code == 0 and "Private" in out

    def dav_client(self):
        # Windows Server ships the WebDAV redirector as a feature that wants
        # a restart, which a runner cannot take.
        code, out = self.ps("Start-Service WebClient; (Get-Service WebClient).Status", check=False)
        return code == 0 and "Running" in out

    def mount_dav(self, url):
        unc = url.replace("https://", "\\\\").replace(f"{LOCAL}/", f"{LOCAL}@SSL\\").replace("/", "\\").rstrip("\\")
        run(["net", "use", self.dav_drive, unc, PASSWORD, f"/user:{OWNER}", "/persistent:no"], timeout=180)
        return Path(self.dav_drive + "\\")

    def unmount_dav(self):
        run(["net", "use", self.dav_drive, "/delete", "/y"], check=False)

    def smb_closed(self):
        _, out = self.ps(f"(Test-NetConnection {BOX_IP} -Port 445 -WarningAction SilentlyContinue).TcpTestSucceeded", check=False)
        return "False" in out


# --- the run ----------------------------------------------------------------


def roundtrip(folder, name, text):
    """Writes a file through a mounted folder and reads it back."""
    path = Path(folder) / name
    path.write_text(text)
    back = until(f"{name} reads back through {folder}", lambda: path.read_text() == text and path.read_text(), timeout=60)
    say(f"wrote and read back {path}: {back!r}")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--os", required=True, choices=["linux", "macos", "windows"])
    ap.add_argument("--box", required=True)
    ap.add_argument("--netdev", required=True)
    ap.add_argument("--qemu", default="qemu-system-x86_64")
    ap.add_argument("--cpus", type=int, default=os.cpu_count() or 2)
    ap.add_argument("--kvm", action="store_true")
    ap.add_argument("--sudo", action="store_true", help="run QEMU as root (vmnet on macOS)")
    ap.add_argument("--no-webdav-mount", action="store_true", help="check WebDAV over HTTP, not a mount")
    args = ap.parse_args()
    client = {"linux": Linux, "macos": MacOS, "windows": Windows}[args.os]()
    say(f"{platform.platform()}, work in {WORK}")

    qemu = boot(args)
    try:
        until("the link to the box is up", client.link_up, timeout=300)
        serial = Serial()
        serial.wait_shell()
        until(
            "nginx, lososd, LosOS Cloud, Samba and wsdd running",
            lambda: serial.sh("systemctl is-active nginx lososd losos-cloud samba-smbd samba-wsdd").split().count("active") == 5,
            timeout=1800,
            every=15,
        )
        ca = WORK / "losos-box.crt"
        ca.write_text(serial.sh("cat /var/lib/losos-tls/cert.pem"))
        client.trust(ca)

        until(f"{LOCAL} resolves to {BOX_IP} over mDNS", client.resolve, timeout=300)
        base = f"https://{LOCAL}"
        until(
            "the box offers its claim",
            lambda: json.loads(http("GET", f"{base}/api/setup/claim", ca)[1]).get("ready"),
            timeout=1200,
            every=10,
        )
        status, body = http("POST", f"{base}/api/setup/claim", ca, {"name": OWNER, "password": PASSWORD})
        assert status == 200, (status, body)
        say("claimed the box as its owner")
        until("Samba knows the owner", lambda: OWNER in serial.sh("cat /var/lib/losos-windows/users.map"), timeout=300)

        until(f"{HOST} is found, once, as an SMB share", client.found, timeout=600, every=15)

        dav = f"{base}/remote.php/webdav/"
        smb = client.mount_smb()
        try:
            roundtrip(smb, "from-smb.txt", f"Written over SMB by {args.os}\n")
            # The same file, from LosOS Cloud's side.
            until(
                "the file written over SMB is in LosOS Cloud",
                lambda: http("GET", dav + "from-smb.txt", ca, auth=(OWNER, PASSWORD))[1].decode().startswith("Written over SMB"),
                timeout=120,
            )
        finally:
            client.unmount_smb()

        mount_dav = not args.no_webdav_mount and client.dav_client()
        if not mount_dav:
            say("no WebDAV client on this computer: WebDAV checked over HTTP")
            status, _ = http("PUT", dav + "from-webdav.txt", ca, body=f"Written over WebDAV by {args.os}\n".encode(), auth=(OWNER, PASSWORD), headers={"Content-Type": "text/plain"})
            assert status in (201, 204), status
            status, body = http("PROPFIND", dav, ca, auth=(OWNER, PASSWORD), headers={"Depth": "1"})
            assert status == 207 and b"from-webdav.txt" in body, (status, body[:500])
            say("WebDAV: PUT and PROPFIND as the owner")
        else:
            mount = client.mount_dav(dav)
            try:
                roundtrip(mount, "from-webdav.txt", f"Written over WebDAV by {args.os}\n")
            finally:
                client.unmount_dav()
        smb = client.mount_smb()
        try:
            text = until("the file written over WebDAV reads over SMB", lambda: (Path(smb) / "from-webdav.txt").read_text(), timeout=120)
            assert text.startswith("Written over WebDAV"), text
        finally:
            client.unmount_smb()

        say("switching the share off on the box")
        serial.sh("/run/current-system/specialisation/off/bin/switch-to-configuration test", timeout=900)
        until("the SMB port is closed", client.smb_closed, timeout=300)
        until("the SMB entry is gone and LosOS Cloud's WebDAV entry is listed", client.off, timeout=600, every=15)
        status, body = http("PROPFIND", dav, ca, auth=(OWNER, PASSWORD), headers={"Depth": "1"})
        assert status == 207 and b"from-smb.txt" in body, (status, body[:500])
        say("WebDAV still opens with the share off")
        say("all good")
    finally:
        qemu.terminate()
        try:
            qemu.wait(timeout=30)
        except subprocess.TimeoutExpired:
            qemu.kill()
        print((WORK / "qemu.log").read_text()[-4000:])


if __name__ == "__main__":
    main()
