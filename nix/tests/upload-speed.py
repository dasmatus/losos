"""How fast a box takes files in: run on the box itself by
nix/tests/upload-speed.nix, through nginx, as a person's computer would
send them. Prints one JSON document and writes it to the path given."""

import concurrent.futures
import hashlib
import json
import os
import shutil
import statistics
import subprocess
import sys
import time

BASE = "http://127.0.0.1"
DAV = f"{BASE}/remote.php/dav/files/perf"
UPLOADS = f"{BASE}/remote.php/dav/uploads/perf"
WORK = "/var/tmp/upload-speed"
RUNS = int(os.environ.get("RUNS", "3"))
MiB = 1 << 20


def curl(*args):
    out = subprocess.run(
        ["curl", "-sS", "-o", "/dev/null", "-w", "%{http_code}", *args],
        check=True,
        capture_output=True,
        text=True,
    ).stdout
    if not out.startswith("2"):
        raise SystemExit(f"curl {args}: HTTP {out}")


def timed(f):
    t = time.monotonic()
    f()
    return time.monotonic() - t


def drop_caches():
    subprocess.run(["sync"], check=True)
    with open("/proc/sys/vm/drop_caches", "w") as f:
        f.write("3\n")


def make(path, size):
    with open(path, "wb") as f:
        left = size
        while left:
            k = min(left, 4 * MiB)
            f.write(os.urandom(k))
            left -= k


def sha1(path):
    h = hashlib.sha1()
    with open(path, "rb") as f:
        while chunk := f.read(MiB):
            h.update(chunk)
    return h.hexdigest()


def stored(name):
    return f"/var/lib/perf-cloud/perf/files/{name}"


def one_large(big):
    times = []
    for i in range(RUNS):
        drop_caches()
        times.append(timed(lambda: curl("-T", big, f"{DAV}/large-{i}.bin")))
        assert sha1(stored(f"large-{i}.bin")) == sha1(big), "large upload differs"
    return times


def many_small(small_dir, names):
    times = []
    for i in range(RUNS):
        curl("-X", "MKCOL", f"{DAV}/small-{i}")
        drop_caches()

        def send(n):
            curl("-T", f"{small_dir}/{n}", f"{DAV}/small-{i}/{n}")

        def all_of_them():
            with concurrent.futures.ThreadPoolExecutor(4) as pool:
                list(pool.map(send, names))

        times.append(timed(all_of_them))
        assert len(os.listdir(stored(f"small-{i}"))) == len(names)
    return times


def chunked(big, chunk):
    """The desktop client's way for a large file: chunks into an upload
    folder, then one MOVE that joins them."""
    size = os.path.getsize(big)
    parts = []
    with open(big, "rb") as f:
        n = 0
        while data := f.read(chunk):
            n += 1
            p = f"{WORK}/chunk-{n:05d}"
            with open(p, "wb") as out:
                out.write(data)
            parts.append(p)
    send_times, join_times = [], []
    for i in range(RUNS):
        up = f"{UPLOADS}/run{i}"
        dest = f"{DAV}/chunked-{i}.bin"
        curl("-X", "MKCOL", "-H", f"Destination: {dest}", up)
        drop_caches()

        def send():
            for k, p in enumerate(parts, 1):
                curl("-T", p, "-H", f"Destination: {dest}", f"{up}/{k:05d}")

        send_times.append(timed(send))
        join_times.append(
            timed(
                lambda: curl(
                    "-X", "MOVE",
                    "-H", f"Destination: {dest}",
                    "-H", f"OC-Total-Length: {size}",
                    f"{up}/.file",
                )
            )
        )
        assert sha1(stored(f"chunked-{i}.bin")) == sha1(big), "joined upload differs"
    for p in parts:
        os.remove(p)
    return send_times, join_times


def push(blob_size):
    """A push of a repository with one large file in it, to a repository
    that does not exist yet."""
    repo = f"{WORK}/repo"
    shutil.rmtree(repo, ignore_errors=True)
    env = dict(os.environ, GIT_AUTHOR_NAME="p", GIT_AUTHOR_EMAIL="p@x",
               GIT_COMMITTER_NAME="p", GIT_COMMITTER_EMAIL="p@x")
    subprocess.run(["git", "init", "-q", "-b", "main", repo], check=True)
    make(f"{repo}/blob.bin", blob_size)
    for k in range(200):
        with open(f"{repo}/note-{k}.txt", "w") as f:
            f.write(f"note {k}\n" * 50)
    subprocess.run(["git", "-C", repo, "add", "."], check=True, env=env)
    subprocess.run(["git", "-C", repo, "commit", "-qm", "files"], check=True, env=env)
    times = []
    for i in range(RUNS):
        drop_caches()
        times.append(
            timed(
                lambda: subprocess.run(
                    ["git", "-C", repo, "push", "-q",
                     f"{BASE}/git/perf/push-{i}.git", "main"],
                    check=True, env=env,
                )
            )
        )
    return times


def peak_rss(unit):
    pid = subprocess.run(
        ["systemctl", "show", "-p", "MainPID", "--value", unit],
        check=True, capture_output=True, text=True,
    ).stdout.strip()
    with open(f"/proc/{pid}/status") as f:
        for line in f:
            if line.startswith("VmHWM:"):
                return int(line.split()[1]) // 1024
    return None


def summary(times, size):
    m = statistics.median(times)
    return {"seconds": [round(t, 3) for t in times], "median": round(m, 3),
            "mibPerSecond": round(size / MiB / m, 1)}


def main(out):
    os.makedirs(WORK, exist_ok=True)
    big_size = int(os.environ.get("LARGE_MIB", "512")) * MiB
    big = f"{WORK}/large.bin"
    make(big, big_size)
    small_dir = f"{WORK}/small"
    os.makedirs(small_dir, exist_ok=True)
    names = [f"photo-{k:03d}.jpg" for k in range(200)]
    for n in names:
        make(f"{small_dir}/{n}", 256 * 1024)

    result = {"large": summary(one_large(big), big_size)}
    result["small"] = summary(many_small(small_dir, names), 200 * 256 * 1024)
    result["small"]["files"] = len(names)
    send, join = chunked(big, 10 * MiB)
    result["chunkedSend"] = summary(send, big_size)
    result["chunkedJoin"] = summary(join, big_size)
    os.remove(big)
    push_size = int(os.environ.get("PUSH_MIB", "256")) * MiB
    result["push"] = summary(push(push_size), push_size)
    result["peakMiB"] = {
        "losos-cloud": peak_rss("perf-cloud.service"),
        "losos-git": peak_rss("perf-git.service"),
    }
    text = json.dumps(result, indent=2)
    print(text)
    with open(out, "w") as f:
        f.write(text + "\n")


if __name__ == "__main__":
    main(sys.argv[1])
