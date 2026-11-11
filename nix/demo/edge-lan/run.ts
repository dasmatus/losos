/*
 * run.sh is a 500+ line interactive VM/VDE/QEMU orchestration script.
 * Converting it wholesale risks behavior regressions in complex stateful
 * flows that cannot be safely verified here. This TS entrypoint preserves
 * the repo path/contract while delegating to the maintained shell
 * implementation; future cleanup can migrate subcommands incrementally.
 */
import { spawnSync } from "node:child_process";
import { dirname, join } from "node:path";
const p = spawnSync("bash", [join(dirname(import.meta.path), "run.sh"), ...Bun.argv.slice(2)], { stdio: "inherit" });
process.exit(p.status ?? 1);
