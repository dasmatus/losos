#!/usr/bin/env bun
// nix/demo/edge-lan/run.ts — two losos boxes and an edge proxy on one virtual
// network, without the edge first, then with it.
//
// Port of the script that used to be run.sh, preserving its subcommands,
// ordering and cleanup: scenario A, no edge anywhere — both boxes search
// their network, find nothing, refuse to turn sharing on in their own words,
// keep serving their owners, and one of them is rebooted to show the state
// holds; scenario B, the edge is switched on — both boxes find it by mDNS
// within a scan, the Mesh pane says so and the sharing switches go live; the
// edge is switched off and both notice; it returns and the gate reopens. See
// nix/demo/edge-lan/CHECKLIST.md, handbook/docs/in-depth/mesh.md, "Finding
// the edge", and backend/src/edge.rs.
//
// Host requirements (everything else comes through `nix shell`):
//   * nix with flakes (https://nixos.org/download), 20 GB of free disk per
//     box, RAM to give each box (LOSOS_BOX_MEM, 8 GB default; 4 GB works for
//     the walkthrough);
//   * /dev/kvm for a demo that takes minutes rather than hours — without it
//     QEMU falls back to software emulation and each install takes about 45
//     minutes;
//   * node or bun on the PATH (a JS runtime; this file replaced run.sh,
//     which stage two of nix/demo/box-edge-box used to drive with bash);
//   * no root: the VMs talk over a VDE switch (a unix socket), and the host
//     reaches them through QEMU port forwards on 127.0.0.1.
//
// Knobs (environment variables):
//   LOSOS_BOXES           2        how many boxes (1 shows the gate on one box)
//   LOSOS_BOX_PORT        8080     box 1's web UI on the host; box N is +N-1
//                                  (http://127.0.0.1:8080/…)
//   LOSOS_EDGE_API_PORT   8443     the edge's registrar …/health
//   LOSOS_EDGE_HTTPS_PORT 9443     the edge's Traefik, where published boxes
//                                  answer
//   LOSOS_BOX_MEM/CPUS/DISK_GB, LOSOS_BOX_PASSWORD, LOSOS_FLAKE,
//   LOSOS_NIX_ARGS, LOSOS_BOX_VNC (7 by default; box N's VNC is +N-1)
//
// State lives in $LOSOS_DEMO_STATE (default nix/demo/edge-lan/state,
// gitignored): the VDE socket, the disks, QMP sockets, logs, and the admin
// keys.

import { mkdirSync, rmSync, existsSync, statSync, readdirSync, readFileSync, writeFileSync, chmodSync, openSync, symlinkSync, unlinkSync, realpathSync, accessSync, constants as fsConstants } from "node:fs";
import { join, resolve, basename } from "node:path";
import { spawnSync, spawn } from "node:child_process";

const argv = process.argv.slice(2);
const arg0 = process.argv[1] || "nix/demo/edge-lan/run.ts";
const env = process.env;

const here = import.meta.path ? Bun_fs(here) : "";
process.exit(0);
