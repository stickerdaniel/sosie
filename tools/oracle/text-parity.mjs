import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { launch, newPage, FIXED_CLOCK, LAUNCH_ARGS } from "./lib.mjs";
import { THEMES } from "./themes.mjs";

export const SANS = `"Mona Sans VF", -apple-system, BlinkMacSystemFont, "Segoe UI", "Noto Sans", Helvetica, Arial, sans-serif, "Apple Color Emoji", "Segoe UI Emoji"`;
export const MONO = `ui-monospace, SFMono-Regular, "SF Mono", Menlo, Consolas, "Liberation Mono", monospace`;

const esc = (s) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");

const GEOMETRY_CSS = `
body{margin:0}
.blk{margin:0 0 8px 0}
.p{font-family:${SANS};font-size:14px;line-height:21px;overflow-wrap:anywhere;word-break:normal;white-space:normal;tab-size:8}
.p code{font-family:${MONO};font-size:85%;padding:.2em .4em;margin:0;white-space:break-spaces;border-radius:6px;background:#eee;tab-size:4}
.pre{font-family:${MONO};font-size:11.9px;line-height:1.45;white-space:pre;overflow-wrap:normal;tab-size:4}
.cell{font-family:${MONO};font-size:12px;line-height:18px;white-space:pre-wrap;overflow-wrap:anywhere;word-break:normal;tab-size:8}
`;

const PAINT_CSS = `
body{margin:0}
.case{position:absolute;left:0;padding:8px}
.p{font-family:${SANS};font-size:14px;line-height:21px;overflow-wrap:anywhere;word-break:normal;white-space:normal;tab-size:8;margin:0}
.p code{font-family:${MONO};font-size:85%;padding:.2em .4em;margin:0;white-space:break-spaces;border-radius:6px;tab-size:4}
.pre{font-family:${MONO};font-size:11.9px;line-height:1.45;white-space:pre;overflow-wrap:normal;tab-size:4;padding:16px;border-radius:6px;margin:0}
.shapes{position:relative;padding:0}
.shapes div{position:absolute;box-sizing:border-box;border-style:solid}
`;

function spansHtml(spans, codeStyle) {
  return spans
    .map((s) => {
      if (s.k === "code") return `<code${codeStyle}>${esc(s.t)}</code>`;
      if (s.k === "mono") return `<span>${esc(s.t)}</span>`;
      let h = `<span>${esc(s.t)}</span>`;
      if (s.i) h = `<em>${h}</em>`;
      if (s.w === 600) h = `<strong style="font-weight:600">${h}</strong>`;
      return h;
    })
    .join("");
}

function geometryBlock(l) {
  const dir = /[֐-ࣿ]/.test(l.spans.map((s) => s.t).join("")) ? ' dir="auto"' : "";
  return `<div class="blk ${l.block}" style="width:${l.width}px"${dir} data-id="${l.id}">${spansHtml(l.spans, "")}</div>`;
}

function extractLines() {
  const seg = new Intl.Segmenter(undefined, { granularity: "grapheme" });
  const out = [];
  for (const blk of document.querySelectorAll(".blk")) {
    const box = blk.getBoundingClientRect();
    const walker = document.createTreeWalker(blk, NodeFilter.SHOW_TEXT);
    const lines = [];
    let cur = null;
    const range = document.createRange();
    for (let node = walker.nextNode(); node; node = walker.nextNode()) {
      for (const { segment, index } of seg.segment(node.data)) {
        if (segment === "\n") {
          cur = null;
          continue;
        }
        range.setStart(node, index);
        range.setEnd(node, index + segment.length);
        const rects = [...range.getClientRects()].filter((r) => r.width > 0 || r.height > 0);
        if (!rects.length) {
          if (cur) cur.text += segment;
          continue;
        }
        const r = rects[0];
        const space = /^\s+$/.test(segment);
        if (cur && r.top < cur.bottom - 1 && r.bottom > cur.top + 1) {
          cur.text += segment;
        } else {
          cur = { text: segment, left: Infinity, right: -Infinity, top: r.top, bottom: r.bottom };
          lines.push(cur);
        }
        cur.top = Math.min(cur.top, r.top);
        cur.bottom = Math.max(cur.bottom, r.bottom);
        if (!space) {
          cur.left = Math.min(cur.left, r.left);
          cur.right = Math.max(cur.right, r.right);
        }
      }
    }
    out.push({ id: blk.dataset.id, height: box.height, lines: lines.map((l) => ({ text: l.text, w: Number.isFinite(l.left) ? l.right - l.left : 0 })) });
  }
  return out;
}

function paintCase(c, top) {
  const t = THEMES[c.theme];
  if (c.kind === "shapes") {
    const boxes = c.boxes
      .map((b) => `<div style="left:${b.x}px;top:${b.y}px;width:${b.w}px;height:${b.h}px;border-radius:${b.r}px;border-width:${b.bw}px;border-color:${b.border};background:${b.fill}"></div>`)
      .join("");
    return `<div class="case" data-id="${c.id}" style="top:${top}px;background:${t.bg}"><div class="shapes" style="width:${c.width}px;height:${c.height}px">${boxes}</div></div>`;
  }
  const extra = c.block === "pre" ? `;background:${t.pre}` : "";
  return `<div class="case" data-id="${c.id}" style="top:${top}px;background:${t.bg};color:${t.fg}"><div class="${c.block}" style="width:${c.width}px${extra}">${spansHtml(c.spans, ` style="background:${t.code}"`)}</div></div>`;
}

async function geometry(corpusPath, outPath) {
  const corpus = JSON.parse(readFileSync(corpusPath, "utf8"));
  const browser = await launch();
  const { page } = await newPage(browser, { width: 1000, height: 800, dpr: 2 });
  const results = [];
  for (let i = 0; i < corpus.length; i += 200) {
    const html = `<!DOCTYPE html><html lang="en"><head><meta charset="utf-8"><style>${GEOMETRY_CSS}</style></head><body>${corpus.slice(i, i + 200).map(geometryBlock).join("\n")}</body></html>`;
    await page.setContent(html, { waitUntil: "load" });
    await page.evaluate(() => document.fonts.ready);
    results.push(...(await page.evaluate(extractLines)));
  }
  const version = browser.version();
  await browser.close();
  writeFileSync(outPath, JSON.stringify(results));
  return { layouts: results.length, browser: version };
}

async function paint(casesPath, outDir) {
  const cases = JSON.parse(readFileSync(casesPath, "utf8"));
  mkdirSync(outDir, { recursive: true });
  const browser = await launch();
  const { page } = await newPage(browser, { width: 800, height: 800, dpr: 2 });
  const boxes = [];
  for (let i = 0; i < cases.length; i += 40) {
    const batch = cases.slice(i, i + 40);
    const html = batch.map((c, k) => paintCase(c, k * 1000)).join("");
    await page.setContent(`<!DOCTYPE html><html lang="en"><head><meta charset="utf-8"><style>${PAINT_CSS}</style></head><body style="height:${batch.length * 1000}px">${html}</body></html>`, { waitUntil: "load" });
    await page.evaluate(() => document.fonts.ready);
    for (const [k, c] of batch.entries()) {
      const box = await page.evaluate((id) => {
        const r = document.querySelector(`[data-id="${id}"]`).getBoundingClientRect();
        return { w: r.width, h: r.height };
      }, c.id);
      const clip = { x: 0, y: k * 1000, width: Math.ceil(box.w), height: Math.ceil(box.h) };
      await page.screenshot({ path: `${outDir}/${c.id}.png`, clip, fullPage: true });
      boxes.push({ id: c.id, w: clip.width, h: clip.height, rawH: box.h });
    }
  }
  const version = browser.version();
  await browser.close();
  writeFileSync(`${outDir}/boxes.json`, JSON.stringify(boxes));
  return { cases: boxes.length, browser: version };
}

const [mode, input, output] = process.argv.slice(2);
const run = { geometry, paint }[mode];
if (!run) {
  console.error("usage: text-parity.mjs geometry <corpus.json> <out.json> | paint <cases.json> <out-dir>");
  process.exit(2);
}
const result = await run(input, output);
console.log(JSON.stringify({ mode, input, ...result, clock: FIXED_CLOCK.toISOString(), args: LAUNCH_ARGS }));
