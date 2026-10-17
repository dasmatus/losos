# The kubelet's part for nix/tests/cloud-webdav.nix: start the Nextcloud
# static pod from the manifest modules/workloads.nix wrote, and exec into it
# for crictl, without k3s, containerd or the image tarball.
#
# The pod runs as a systemd unit in a root directory of its own that holds
# the image's /bin and nothing else, with the manifest's hostPath volumes
# bound at their mount paths, as the uid and groups its securityContext
# names, every capability dropped and no new privileges, the same shape the
# kubelet gives it. So the test runs the entrypoint, the Apache and php-fpm
# configuration and the mounted config files the box runs, on the mounts the
# manifest asks for: change a mount there and this follows.
#
#   kubelet-stand-in run           start the pod (unit pod-nextcloud)
#   kubelet-stand-in exec CMD...   run CMD in the pod, as crictl exec does
#
# IMAGE_ROOT is the image's root (nix/images.nix, passthru.root); the
# manifest's image reference is not looked up, there is no runtime to ask.
import os
import sys

import yaml

MANIFEST = "/var/lib/losos/k3s-static-pods/nextcloud.yaml"
ROOT = "/run/losos-test-pod/root"
# What the image bakes in (nix/images.nix): its Env and WorkingDir, and
# /run/nextcloud owned by the pod's uid.
IMAGE = os.environ["IMAGE_ROOT"]
WORKDIR = os.environ["IMAGE_WORKDIR"]
RUN_DIR = "/run/losos-test-pod/run-nextcloud"


def properties():
    pod = yaml.safe_load(open(MANIFEST))
    spec = pod["spec"]
    assert spec["hostNetwork"] is True, "the pod must share the host's network"
    (container,) = spec["containers"]
    volumes = {v["name"]: v["hostPath"] for v in spec["volumes"]}
    sc = spec["securityContext"]
    csc = container["securityContext"]

    # The image's root. An image layer holds real directories where buildEnv makes symlinks
    # into the store, and the mounts land under them (/etc, /var/lib), so
    # the directories are made here and only the files are links.
    for here, _, files in os.walk(IMAGE, followlinks=True):
        rel = os.path.relpath(here, IMAGE)
        os.makedirs(os.path.join(ROOT, rel), exist_ok=True)
        for f in files:
            link = os.path.join(ROOT, rel, f)
            if not os.path.lexists(link):
                os.symlink(os.path.realpath(os.path.join(here, f)), link)
    os.makedirs(RUN_DIR, exist_ok=True)
    os.chown(RUN_DIR, sc["runAsUser"], sc["runAsGroup"])
    os.chmod(RUN_DIR, 0o700)

    props = [
        f"RootDirectory={ROOT}",
        "BindReadOnlyPaths=/nix/store",
        # What the kubelet adds to every hostNetwork pod.
        "BindReadOnlyPaths=/etc/resolv.conf /etc/hosts",
        f"BindPaths={RUN_DIR}:/run/nextcloud",
        "PrivateTmp=yes",
        f"User={sc['runAsUser']}",
        f"Group={sc['runAsGroup']}",
        "SupplementaryGroups="
        + " ".join(str(g) for g in [sc["fsGroup"], *sc.get("supplementalGroups", [])]),
        f"WorkingDirectory={WORKDIR}",
        "Environment=PATH=/bin LANG=C.UTF-8",
    ]
    if csc.get("allowPrivilegeEscalation") is False:
        props.append("NoNewPrivileges=yes")
    if "ALL" in csc.get("capabilities", {}).get("drop", []):
        props += ["CapabilityBoundingSet=", "AmbientCapabilities="]
    for mount in container["volumeMounts"]:
        host = volumes[mount["name"]]
        # type: File and Directory are checked, not created, as the kubelet
        # does: a missing path is a pod that does not start.
        kind = host.get("type", "")
        if kind == "File" and not os.path.isfile(host["path"]):
            sys.exit(f"hostPath {host['path']} is not a file")
        if kind == "Directory" and not os.path.isdir(host["path"]):
            sys.exit(f"hostPath {host['path']} is not a directory")
        bind = "BindReadOnlyPaths" if mount.get("readOnly") else "BindPaths"
        props.append(f"{bind}={host['path']}:{mount['mountPath']}")
    return props


def systemd_run(extra, argv):
    args = ["systemd-run", "--quiet"]
    for p in properties():
        args += ["-p", p]
    os.execvp("systemd-run", args + extra + ["--", *argv])


match sys.argv[1:]:
    case ["run"]:
        # restartPolicy: Always.
        systemd_run(
            ["--unit=pod-nextcloud", "-p", "Restart=always", "-p", "RestartSec=2"],
            # The image's Entrypoint, its output on a pipe as a container
            # runtime gives it: php-fpm opens /dev/stderr, which a unit's
            # journal socket refuses.
            [
                "/bin/sh",
                "-c",
                '"$@" 2>&1 | cat',
                "sh",
                "/bin/tini",
                "-g",
                "--",
                "/bin/losos-nextcloud",
            ],
        )
    case ["exec", *argv]:
        systemd_run(["--wait", "--pipe", "--collect"], argv)
    case _:
        sys.exit("usage: kubelet-stand-in run | exec CMD...")
