import { createHash } from "node:crypto";
import { readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { join, relative } from "node:path";

const STAMPS = "tests/golden/stamps.json";
const LOCK = "tests/golden/pixels.lock.json";
const GOLDEN_ROOT = "tests/golden/synthetic";

const sha = (path) => createHash("sha256").update(readFileSync(path)).digest("hex");
const hashes = (paths) => Object.fromEntries(paths.map((p) => [p, sha(p)]));

function files(dir) {
  return readdirSync(dir).flatMap((name) => {
    const p = join(dir, name);
    return statSync(p).isDirectory() ? files(p) : [relative(".", p)];
  });
}

function capture(logs) {
  const runs = logs.map((l) => JSON.parse(readFileSync(l, "utf8").trim().split("\n").pop()));
  const browsers = [...new Set(runs.map((r) => r.browser))];
  if (browsers.length !== 1) throw new Error(`captures used several browsers: ${browsers}`);
  return {
    image: readFileSync("target/oracle/image-id", "utf8").trim(),
    browser: browsers[0],
    launchArgs: runs[0].args,
    clock: runs[0].clock,
    locale: "en-US",
    timezone: "UTC",
    dpr: 2,
    fonts: { "assets/fonts.lock.json": sha("assets/fonts.lock.json") },
    masks: [],
    date: new Date().toISOString().slice(0, 10),
  };
}

const HARNESS = [
  "tools/oracle/text-parity.mjs",
  "tools/oracle/lib.mjs",
  "tools/oracle/themes.mjs",
  "tools/oracle/capture.sh",
  "tools/oracle/ref/Dockerfile",
  "tools/oracle/ref/fonts.conf",
];

function describe() {
  return {
    "synthetic-text": {
      class: "synthetic",
      viewport: { width: 1000, height: 800 },
      crop: "none: line geometry only",
      inputs: hashes(["tests/golden/synthetic/text/corpus-1.json", "tests/golden/synthetic/text/corpus-2.json"]),
      geometry: hashes(["tests/golden/synthetic/text/chrome-1.json", "tests/golden/synthetic/text/chrome-2.json"]),
      harness: hashes([...HARNESS, "tools/oracle/corpus.mjs"]),
      logs: ["target/oracle/geometry-1.log", "target/oracle/geometry-2.log"],
    },
    "synthetic-paint": {
      class: "synthetic",
      viewport: { width: 800, height: 800 },
      crop: "border box of each case, rounded up to whole CSS px",
      inputs: hashes(["tests/golden/synthetic/paint/cases-1.json", "tests/golden/synthetic/paint/cases-2.json"]),
      geometry: hashes(["tests/golden/synthetic/paint/boxes-1.json", "tests/golden/synthetic/paint/boxes-2.json"]),
      harness: hashes([...HARNESS, "tools/oracle/paint-cases.mjs"]),
      logs: ["target/oracle/paint-1.log", "target/oracle/paint-2.log"],
    },
  };
}

function write() {
  const lock = JSON.parse(readFileSync(LOCK, "utf8"));
  const goldens = {};
  for (const [name, { logs, ...rest }] of Object.entries(describe())) {
    const tag = lock.goldens[name];
    goldens[name] = { ...rest, capture: capture(logs), bundle: tag ? { tag, ...lock.bundles[tag] } : null };
  }
  writeFileSync(STAMPS, JSON.stringify({ version: 1, goldens }, null, 2) + "\n");
  console.log(`wrote ${STAMPS}`);
}

function check() {
  const stamps = JSON.parse(readFileSync(STAMPS, "utf8")).goldens;
  const lock = JSON.parse(readFileSync(LOCK, "utf8"));
  const errors = [];
  const stamped = new Set();
  for (const [name, stamp] of Object.entries(stamps)) {
    for (const group of ["inputs", "geometry", "harness"]) {
      for (const [path, want] of Object.entries(stamp[group])) {
        stamped.add(path);
        let got;
        try {
          got = sha(path);
        } catch {
          got = "missing";
        }
        if (got !== want) errors.push(`${name}: ${path} is ${got}, stamp has ${want}`);
      }
    }
    for (const [path, want] of Object.entries(stamp.capture.fonts)) {
      if (sha(path) !== want) errors.push(`${name}: ${path} changed after the capture`);
    }
    const tag = lock.goldens[name];
    const bundle = tag ? { tag, ...lock.bundles[tag] } : null;
    if (JSON.stringify(bundle) !== JSON.stringify(stamp.bundle)) errors.push(`${name}: pixel bundle ${JSON.stringify(bundle)} differs from stamp ${JSON.stringify(stamp.bundle)}`);
  }
  for (const path of files(GOLDEN_ROOT)) {
    if (!stamped.has(path)) errors.push(`${path} has no stamp`);
  }
  for (const name of Object.keys(lock.goldens)) {
    if (!stamps[name]) errors.push(`${name} is in ${LOCK} without a stamp`);
  }
  if (errors.length) {
    console.error(errors.join("\n"));
    process.exit(1);
  }
  console.log(`stamps ok: ${Object.keys(stamps).length} goldens, ${stamped.size} files`);
}

const mode = process.argv[2];
if (mode === "--write") write();
else if (mode === "--check") check();
else {
  console.error("usage: golden-stamp.mjs --write | --check");
  process.exit(2);
}
