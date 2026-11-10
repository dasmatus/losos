"""Boots an unclaimed appliance for the adversary, and leaves it running.

The adversary workflow (.github/workflows/opencode-adversary.yml) runs this
before the OpenCode agent's step, in the same job: it boots the same test box
the discovery check uses (nix/tests/discovery/box.nix, built as
`losos-disk-discovery-box`) on the runner's tap, waits until nginx, lososd
and the rest are up and the box offers its claim, and writes a state file.
Everything the agent needs is in that state file: the address, the box's TLS
certificate, and QEMU's PID. It then exits and leaves the box running for
the agent's step and report step -- nobody signs the box in for the agent;
the claim is on the table, the first caller to take it becomes the owner.
That is the threat model's first-contact situation, and the agent's problem.

    python3 boot.py --box discovery-box --state run/state.json [--kvm]

`kill` tears the box down, from the state file:

    python3 boot.py kill --state run/state.json
"""

import argparse
import importlib.util
import json
import os
import signal
import tempfile
import time
from pathlib import Path

# Everything about booting and talking to the box comes from the discovery
# check: import it for its helpers and keep our own CLI.
_SPEC = importlib.util.spec_from_file_location(
    "discovery_check", Path(__file__).resolve().parent.parent / "discovery" / "check.py"
)
_disco = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(_disco)  # noqa: E402


def boot(args):
    """Boots the box and waits until it offers its claim; writes the state."""
    box = Path(args.box)
    work = Path(tempfile.mkdtemp(prefix="losos-adversary-"))
    # boot() logs QEMU into the work dir named in the discovery module's one
    # global; point it at this run's.
    _disco.WORK = work

    qemu = _disco.boot(args)
    try:
        serial = _disco.Serial()
        serial.wait_shell()
        _disco.until(
            "nginx, lososd and LosOS Cloud running",
            lambda: serial
            .sh("systemctl is-active nginx lososd losos-cloud")
            .split()
            .count("active")
            == 3,
            timeout=1800,
            every=15,
        )
        cert = work / "losos-box.crt"
        cert.write_text(serial.sh("cat /var/lib/losos-tls/cert.pem"))
        base = f"https://{_disco.BOX_IP}"
        _disco.until(
            "the unclaimed box offers its claim",
            lambda: json.loads(_disco.http("GET", f"{base}/api/setup/claim", cert)[1]).get("ready"),
            timeout=1200,
            every=10,
        )
        state = {
            "base": base,
            "ip": _disco.BOX_IP,
            "lan": "192.168.77.0/24, the runner at .1; both are the box's LAN",
            "cert": str(cert),
            "qemu_pid": qemu.pid,
            "qemu_log": str(_disco.WORK / "qemu.log"),
            "claimed": False,
        }
        state_path = Path(args.state)
        state_path.parent.mkdir(parents=True, exist_ok=True)
        state_path.write_text(json.dumps(state, indent=2) + "\n")
        _disco.say(f"the box is up and unclaimed; state in {state_path}")
        _disco.say("it stays running; the adversary takes the claim or the walls around it")
    except BaseException:
        qemu.terminate()
        try:
            qemu.wait(timeout=30)
        except Exception:
            qemu.kill()
        raise
    return qemu, work


def kill(args):
    """Stops the box from the state file; right by SIGTERM, then SIGKILL."""
    state = json.loads(Path(args.state).read_text())
    pid = int(state["qemu_pid"])
    try:
        os.kill(pid, signal.SIGTERM)
        print(f"SIGTERM to QEMU pid {pid}")
        for _ in range(30):
            os.kill(pid, 0)
            time.sleep(1)
        raise RuntimeError(f"QEMU pid {pid} ignored SIGTERM")
    except ProcessLookupError:
        print(f"QEMU pid {pid} already gone")
        return
    os.kill(pid, signal.SIGKILL)
    print(f"SIGKILL to QEMU pid {pid}")


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--box", required=True, help="the copied losos-disk-discovery-box output")
    ap.add_argument("--state", required=True, help="run-state JSON to write, or read for kill")
    ap.add_argument("--netdev", default="tap,id=lan,ifname=tap0,script=no,downscript=no")
    ap.add_argument("--qemu", default="qemu-system-x86_64")
    ap.add_argument("--cpus", type=int, default=4)
    ap.add_argument("--kvm", action=argparse.BooleanOptionalAction, default=os.path.exists("/dev/kvm"))
    ap.add_argument("--sudo", action="store_true")
    sub = ap.add_subparsers(dest="command", required=True)
    sub.add_parser("boot", help="boot the box, wait, write the state, leave it running")
    sub.add_parser("kill", help="stop the box in the state file")
    args = ap.parse_args()
    if args.command == "boot":
        boot(args)
    else:
        kill(args)


if __name__ == "__main__":
    main()
