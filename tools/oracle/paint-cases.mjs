import { readFileSync, writeFileSync } from "node:fs";

import { THEMES } from "./themes.mjs";

const [seedArg, srcPath, outPath] = process.argv.slice(2);
let seed = Number(seedArg);
const rnd = () => {
  seed |= 0; seed = (seed + 0x6d2b79f5) | 0;
  let t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
  t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
  return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
};
const pick = (a) => a[Math.floor(rnd() * a.length)];
const int = (lo, hi) => lo + Math.floor(rnd() * (hi - lo + 1));


const corpus = JSON.parse(readFileSync(srcPath, "utf8")).filter((l) => l.width === 390);
const take = (stratum, n) => corpus.filter((l) => l.stratum === stratum).slice(0, n);
const PER = 24;
const cases = [];
for (const theme of Object.keys(THEMES)) {
  const widths = [310, 454];
  take("latin-prose", PER).forEach((l, i) => cases.push({ id: `sans-prose-${theme}-${i}`, stratum: "sans-prose", theme, kind: "text", block: "p", width: widths[i % 2], spans: l.spans }));
  take("bold-italic", PER / 2).forEach((l, i) => cases.push({ id: `sans-prose-${theme}-bi${i}`, stratum: "sans-prose", theme, kind: "text", block: "p", width: widths[i % 2], spans: l.spans }));
  take("mixed-mono", PER).forEach((l, i) => cases.push({ id: `mono-code-${theme}-${i}`, stratum: "mono-code", theme, kind: "text", block: "p", width: widths[i % 2], spans: l.spans }));
  take("pre-tabs", PER / 2).forEach((l, i) => cases.push({ id: `mono-code-${theme}-pre${i}`, stratum: "mono-code", theme, kind: "text", block: "pre", width: 420, spans: l.spans }));
  take("emoji", PER).forEach((l, i) => cases.push({ id: `emoji-${theme}-${i}`, stratum: "emoji", theme, kind: "text", block: "p", width: widths[i % 2], spans: l.spans }));
  for (let i = 0; i < PER; i++) {
    const boxes = [];
    for (let k = 0; k < int(3, 7); k++) {
      const w = int(20, 200), h = int(12, 90);
      boxes.push({ x: int(0, 300 - w), y: int(0, 160 - h), w, h, r: pick([0, 0, 2, 6, 6, 12, 2000]), bw: pick([0, 1, 1, 2]), fill: pick(THEMES[theme].fills), border: pick([THEMES[theme].border, THEMES[theme].fills[0], THEMES[theme].fills[5]]) });
    }
    cases.push({ id: `fills-${theme}-${i}`, stratum: "borders-fills", theme, kind: "shapes", width: 300, height: 160, boxes });
  }
}
writeFileSync(outPath, JSON.stringify(cases));
console.log(cases.length, "paint cases");
