/*
 * bootstrap.sh is documented as `curl ... | sh` for end users on machines
 * that may not have Bun. Keep bootstrap.sh as the shell implementation
 * (end-user bootstrap contract). This TS module is a development/checkout
 * entrypoint that executes the shell implementation.
 */
import { spawnSync } from "node:child_process";
import { dirname, join } from "node:path";
const p = spawnSync("bash", [join(dirname(import.meta.path), "bootstrap.sh"), ...Bun.argv.slice(2)], { stdio: "inherit" });
process.exit(p.status ?? 1);
