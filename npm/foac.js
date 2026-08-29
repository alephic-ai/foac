#!/usr/bin/env node
// `npx foac` and `npm install -g foac` land here. npm installed the one
// @alephic/foac-<platform> package that matches this machine — the others
// are optionalDependencies its os/cpu fields ruled out — and this shim runs
// the binary inside it. Per-platform packages are what keeps an install from
// downloading all six binaries.
"use strict";

const { spawnSync } = require("node:child_process");

const pkg = `@alephic/foac-${process.platform}-${process.arch}`;
const exe = process.platform === "win32" ? "foac.exe" : "foac";
let binary;
try {
  binary = require.resolve(`${pkg}/bin/${exe}`);
} catch {
  console.error(
    `foac: no binary for ${process.platform}-${process.arch}. ` +
      "Other installers: https://github.com/alephic-ai/foac#install",
  );
  process.exit(1);
}

const { status } = spawnSync(binary, process.argv.slice(2), { stdio: "inherit" });
process.exit(status ?? 1);
