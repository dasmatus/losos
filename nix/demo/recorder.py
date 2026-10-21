# Recording hooks for a VM test, prepended to its testScript.
#
# With LOSOS_RECORD_DIR set in the environment of the test driver
# (nix/demo/record.sh sets it), record() writes one line per step to
# <dir>/timeline.jsonl and waits until nix/demo/record.mjs has photographed
# the step's pages through the box's real nginx and admin UI, forwarded to
# the host. Without it, record() only prints the caption, so the test runs
# the same in CI and in `nix build`.
#
#   record_port(box, 18080, 80)              forward a guest port to the host
#   record_note("box.token", token)          leave a file for the recorder
#   record("apps", "The Apps pane", [
#     {"name": "apps-pane", "port": 18080, "path": "/settings/apps",
#      "token": "box.token", "wait": "text=Running"},
#   ])
#
# A shot names a host port and a path. With "token" the page is signed in
# with that box's admin key (sessionStorage, as the spare-key dialog does);
# "wait" is a Playwright selector to wait for before the picture.
import json as _rec_json
import os as _rec_os
import time as _rec_time

_rec_dir = _rec_os.environ.get("LOSOS_RECORD_DIR")
_rec_seq = 0


def record_note(name, text):
    if _rec_dir:
        with open(_rec_os.path.join(_rec_dir, name), "w") as f:
            f.write(text)


def record_port(machine, host_port, guest_port):
    if _rec_dir:
        machine.forward_port(host_port=host_port, guest_port=guest_port)


def record(step, caption, shots, wait=300):
    global _rec_seq
    _rec_seq += 1
    print(f"=== {step}: {caption}")
    if not _rec_dir:
        return
    line = {"n": _rec_seq, "step": step, "caption": caption, "t": _rec_time.time(), "shots": shots}
    with open(_rec_os.path.join(_rec_dir, "timeline.jsonl"), "a") as f:
        f.write(_rec_json.dumps(line) + "\n")
    ack = _rec_os.path.join(_rec_dir, f"ack-{_rec_seq}")
    for _ in range(wait):
        if _rec_os.path.exists(ack):
            return
        _rec_time.sleep(1)
    print(f"the recorder did not acknowledge step {_rec_seq}; carrying on")

