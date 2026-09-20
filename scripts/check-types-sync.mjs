#!/usr/bin/env node
/**
 * Verifies that the committed generated types are in sync with the Rust
 * `types` crate. Exits non-zero when they differ (used in CI).
 *
 * Usage: `node scripts/check-types-sync.mjs`
 */

import { spawn } from "node:child_process";
import { readFile, rm } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = fileURLToPath(new URL(".", import.meta.url));
const OUT_FILE = join(__dirname, "..", "src", "lib", "generated", "types.ts");

// Regenerate into a temp file and compare.
const tempFile = `${OUT_FILE}.tmp`;
const child = spawn("node", [join(__dirname, "gen-types.mjs")], {
  env: { ...process.env, GEN_TYPES_OUT: tempFile },
});

// `exitCode` is null until the process exits, then becomes the exit code.
while (child.exitCode === null) {
  await new Promise((resolve) => setTimeout(resolve, 50));
}
if (child.exitCode !== 0) {
  console.error("Failed to regenerate types.");
  process.exit(1);
}

const committed = await readFile(OUT_FILE, "utf8");
const fresh = await readFile(tempFile, "utf8");
await rm(tempFile);

if (committed !== fresh) {
  console.error(
    "Generated types are out of sync with the Rust types crate.\n" +
      "Run `node scripts/gen-types.mjs` and commit the result.",
  );
  process.exit(1);
}

console.log("Generated types are in sync.");