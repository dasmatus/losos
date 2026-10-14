#!/usr/bin/env node
/* losos: manage LosOS boxes and edges from a terminal, without SSH. */

import { run } from "./cli";

run(process.argv.slice(2)).then(
  (code) => {
    process.exitCode = code;
  },
  (error: unknown) => {
    process.stderr.write(`losos: ${error instanceof Error ? error.message : String(error)}\n`);
    process.exitCode = 1;
  },
);
