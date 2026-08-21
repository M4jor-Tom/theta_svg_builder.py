#!/usr/bin/env node
// The WASM build must render the SAME BYTES as the native one -- not merely the
// same picture. Every render in test/golden/ is pinned to the sha512 of its own
// bytes, so comparing against the corpus proves both at once: the wasm build
// matches native, and it matches what native was pinned to.
//
// Float formatting through geom::fmt is the plausible way two targets diverge
// while both still look correct, which is why this exists at all.
//
//   nix build .#bgsvg-wasm
//   BGSVG_WASM=$PWD/result/nodejs nix develop -c node test/wasm.mjs
import { createHash } from "node:crypto";
import { readdirSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const REPO = dirname(dirname(fileURLToPath(import.meta.url)));
const GOLDEN = join(REPO, "test", "golden");
const SIZE = [1920, 1080]; // what test/golden.py renders at
const JSON_SUFFIX = "_parameters.json";
const SVG_SUFFIX = "_background.svg";
const EXPECTED = 42; // same count tests/configs.rs asserts

const pkg = process.env.BGSVG_WASM;
if (!pkg) {
  console.error("set BGSVG_WASM to a wasm-bindgen --target nodejs output directory");
  console.error("  nix build .#bgsvg-wasm && BGSVG_WASM=$PWD/result/nodejs node test/wasm.mjs");
  process.exit(2);
}
const { render } = await import(join(pkg, "bgsvg_wasm.js"));

const sha = (b) => createHash("sha512").update(b).digest("hex");

// Where two documents first part company. The whole SVG is one line, so a line
// diff says nothing -- point at the byte and quote either side of it.
function firstDiff(want, got) {
  let i = 0;
  while (i < want.length && i < got.length && want[i] === got[i]) i++;
  const lo = Math.max(0, i - 30);
  return (
    `      first differs at byte ${i}, ${want.length} -> ${got.length} bytes\n` +
    `      was ...${want.subarray(lo, i + 40)}...\n` +
    `      now ...${got.subarray(lo, i + 40)}...`
  );
}

const bad = [];
let checked = 0;

for (const dir of readdirSync(GOLDEN).sort()) {
  const names = readdirSync(join(GOLDEN, dir));
  const cfgName = names.find((n) => n.endsWith(JSON_SUFFIX));
  const svgName = names.find((n) => n.endsWith(SVG_SUFFIX));
  if (!cfgName || !svgName) {
    bad.push(`${dir.slice(0, 16)}...: not a golden directory; run test/golden.py first`);
    continue;
  }

  const cfg = readFileSync(join(GOLDEN, dir, cfgName), "utf8");
  const want = readFileSync(join(GOLDEN, dir, svgName));
  checked++;

  let got;
  try {
    got = Buffer.from(render(cfg, ...SIZE), "utf8");
  } catch (e) {
    bad.push(`${cfg.trim()}\n      wasm threw ${e.kind ?? "?"}: ${e.message ?? e}`);
    continue;
  }

  if (!got.equals(want)) {
    bad.push(`${cfg.trim()}\n${firstDiff(want, got)}`);
  } else if (sha(got) !== dir) {
    bad.push(`${cfg.trim()}\n      renders bytes that do not hash to their own directory`);
  }
}

if (checked !== EXPECTED) {
  bad.push(`swept ${checked} configs, expected ${EXPECTED} -- the corpus and the sweep disagree`);
}

if (bad.length) {
  console.error(`wasm: ${bad.length} problem(s) across ${checked} configs`);
  for (const b of bad) console.error(`  ${b}`);
  console.error("\nthe wasm build must match the corpus byte for byte; do NOT regenerate it");
  process.exit(1);
}
console.log(`wasm ok: ${checked} configs render byte-identical to the corpus at ${SIZE.join("x")}`);
