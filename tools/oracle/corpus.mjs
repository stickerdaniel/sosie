import { writeFileSync } from "node:fs";

const [seedArg, outPath] = process.argv.slice(2);
let seed = Number(seedArg);
const rnd = () => {
  seed |= 0; seed = (seed + 0x6d2b79f5) | 0;
  let t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
  t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
  return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
};
const pick = (a) => a[Math.floor(rnd() * a.length)];
const int = (lo, hi) => lo + Math.floor(rnd() * (hi - lo + 1));

const EN = "the a renderer draws each comment review thread line box with its own font size and weight before the agent posts it to github so that nobody needs a browser for that step while layout follows css rules exactly as chromium does when it wraps text at the edge of a narrow column which is why every width matters here including small ones like these words: offset, kerning, glyph, advance, baseline, fallback, emoji, unicode, segment, cluster, shaping, harfbuzz, deterministic, snapshot, fixture, canary, pixel, budget, ratio, threshold, measurement".split(/[\s,:]+/).filter(Boolean);
const PUNCT = [",", ".", ";", ":", "!", "?", " -", " –", " (", ")", "'s", "\"", "/"];
const CODE = ["gh api", "renderReview()", "start_line", "--dry-run", "Vec<String>", "cargo test", "src/greet.rs", "null", "x.unwrap_or(0)", "POST /markdown", "a == b", "#[derive(Debug)]", "fn main() {}", "npm i -D sosie", "HEAD~1", "foo_bar_baz", "0x7f", "i += 1"];
const LONG = ["https://github.com/stickerdaniel/sosie-canary/blob/main/src/greet.rs#L1-L3", "a_very_long_identifier_name_that_does_not_fit_on_a_narrow_line_at_all", "SupercalifragilisticexpialidociousWithoutAnyBreakOpportunityInside", "sha256:562111b88caa79c71391c0c4d3c2114a7d5c35915c20561fd893cb1e07929e6b", "/Users/someone/Code/project/scratch/very/deep/directory/structure/file.txt", "user.name@example-domain-with-a-long-name.com", "ThisIsCamelCaseThatGoesOnAndOnAndOnWithoutSpaces", "--option-with-many-hyphens-in-its-name-that-keeps-going"];
const RU = "рендерер рисует каждый комментарий и ветку обзора с собственным размером шрифта перед тем как агент отправит его на гитхаб чтобы никому не нужен был браузер для этого шага при этом раскладка следует правилам css точно так же как хромиум переносит текст у края узкой колонки поэтому важна каждая ширина".split(" ");
const EL = "ο αποδότης σχεδιάζει κάθε σχόλιο και νήμα αναθεώρησης με το δικό του μέγεθος γραμματοσειράς πριν ο πράκτορας το δημοσιεύσει ώστε κανείς να μη χρειάζεται πρόγραμμα περιήγησης για αυτό το βήμα ενώ η διάταξη ακολουθεί τους κανόνες όπως ακριβώς το chromium αναδιπλώνει κείμενο στην άκρη μιας στενής στήλης".split(" ");
const EMOJI = ["👍", "🎉", "🚀", "😀", "😂", "❤️", "✅", "⚠️", "🔥", "👀", "👩‍💻", "👨‍👩‍👧‍👦", "🏳️‍🌈", "🇩🇪", "🇯🇵", "🇺🇸", "1️⃣", "#️⃣", "✌🏽", "👋🏿", "🤷‍♀️", "☺️", "✨", "🙏🏼", "🧑🏽‍🚀", "🐛", "📦", "©️", "™️", "↩️"];
const JA = "レンダラーはエージェントが投稿する前に、各コメントとレビュースレッドを独自のフォントサイズで描画します。ブラウザは不要です。「日本語」の行分割規則（禁則処理）も確認します。".split("");
const ZH = "渲染器在代理发布之前，用自己的字号绘制每条评论和审查线程。这样任何人都不需要浏览器。布局遵循与浏览器相同的规则，在窄列的边缘换行。".split("");
const AR = "يرسم العارض كل تعليق ومراجعة بحجم الخط الخاص به قبل أن ينشره الوكيل على جيتهاب حتى لا يحتاج أحد إلى متصفح لهذه الخطوة بينما يتبع التخطيط قواعد css تماما".split(" ");
const HE = "המעבד מצייר כל תגובה ושרשור סקירה בגודל הגופן שלו לפני שהסוכן מפרסם אותו בגיטהאב כך שאף אחד לא צריך דפדפן לשלב הזה בזמן שהפריסה עוקבת אחר כללי css בדיוק".split(" ");

const words = (list, n) => Array.from({ length: n }, () => pick(list));
const sentence = (list, n) => {
  let s = words(list, n).join(" ");
  if (rnd() < 0.5) s = s[0].toUpperCase() + s.slice(1);
  return s + pick([".", ".", ",", "!", "?", ":"]);
};
const prose = (list, lo, hi) => {
  const out = [];
  let n = int(lo, hi);
  while (n > 0) { const k = Math.min(n, int(4, 14)); out.push(sentence(list, k)); n -= k; }
  return out.join(" ");
};
const sans = (t, w = 400, i = false) => ({ t, k: "sans", w, i });
const code = (t) => ({ t, k: "code", w: 400, i: false });

const STRATA = {
  "latin-prose": () => ({ block: "p", spans: [sans(prose(EN, 8, 90).replace(/ (\w)/g, (m, c) => (rnd() < 0.05 ? pick(PUNCT) + " " + c : m)))] }),
  "mixed-mono": () => {
    const spans = [];
    const n = int(2, 6);
    for (let i = 0; i < n; i++) { spans.push(sans((i ? " " : "") + prose(EN, 2, 14) + " ")); spans.push(code(pick(CODE))); }
    spans.push(sans(pick([".", " and more text after it.", ", then " + prose(EN, 2, 8)])));
    return { block: "p", spans };
  },
  "bold-italic": () => {
    const spans = [];
    const n = int(2, 7);
    for (let i = 0; i < n; i++) {
      spans.push(sans((i ? " " : "") + prose(EN, 1, 10) + " "));
      const r = rnd();
      spans.push(sans(words(EN, int(1, 5)).join(" "), r < 0.45 ? 600 : r < 0.8 ? 400 : 600, r >= 0.45));
    }
    spans.push(sans("."));
    return { block: "p", spans };
  },
  "long-tokens": () => {
    const spans = [sans(prose(EN, 1, 12) + " ")];
    const r = rnd();
    if (r < 0.3) spans.push(code(pick(LONG)));
    else spans.push(sans(pick(LONG), rnd() < 0.2 ? 600 : 400));
    spans.push(sans(" " + prose(EN, 1, 12)));
    return { block: "p", spans };
  },
  "pre-tabs": () => {
    const lines = [];
    for (let i = 0; i < int(2, 8); i++) {
      const indent = pick(["", "\t", "\t\t", "    ", "\t  ", "  \t"]);
      const body = words(["let", "x", "=", "foo(bar);", "if", "{", "}", "return", "a\tb", "c", "// note", "match", "=>", "Some(v)", "\"str\""], int(1, 9)).join(pick([" ", " ", "\t"]));
      lines.push(indent + body + (rnd() < 0.2 ? "\t// trailing" : ""));
    }
    return { block: "pre", spans: [{ t: lines.join("\n"), k: "mono", w: 400, i: false }] };
  },
  "mono-wrap": () => {
    const parts = [];
    for (let i = 0; i < int(1, 4); i++) parts.push(pick(["    ", "\t", ""]) + words(["let", "mut", "out", "=", "String::with_capacity(name.len()", "+", "10);", "format!(\"Goodbye, {name}!\")", "self.items.iter().map(|x| x.value * 2).collect::<Vec<_>>();", "if", "cond", "{", "}", "a_very_long_identifier_name_that_does_not_fit_anywhere", "//", "comment", "text"], int(3, 16)).join(" "));
    return { block: "cell", spans: [{ t: parts.join("\n"), k: "mono", w: 400, i: false }] };
  },
  emoji: () => {
    const out = [];
    for (let i = 0; i < int(4, 30); i++) out.push(rnd() < 0.4 ? pick(EMOJI) + (rnd() < 0.3 ? pick(EMOJI) : "") : pick(EN));
    return { block: "p", spans: [sans(out.join(" ") + ".")] };
  },
  cyrillic: () => ({ block: "p", spans: [sans(prose(RU, 6, 70))] }),
  greek: () => ({ block: "p", spans: [sans(prose(EL, 6, 70))] }),
  cjk: () => {
    const src = rnd() < 0.5 ? JA : ZH;
    const n = int(10, 120);
    const s = int(0, src.length - 1);
    let t = "";
    for (let i = 0; i < n; i++) t += src[(s + i) % src.length];
    if (rnd() < 0.3) t = t.slice(0, n >> 1) + " GitHub " + t.slice(n >> 1);
    return { block: "p", spans: [sans(t)] };
  },
  arabic: () => ({ block: "p", spans: [sans(prose(AR, 4, 50).replace(/[.!?:,]$/, "."))] }),
  hebrew: () => ({ block: "p", spans: [sans(prose(HE, 4, 50))] }),
};

const PER = 84;
const layouts = [];
for (const [stratum, gen] of Object.entries(STRATA)) {
  for (let i = 0; i < PER; i++) {
    const item = gen();
    for (const width of [390, 800]) layouts.push({ id: `${stratum}-${i}-${width}`, stratum, width, ...item });
  }
}
const TRAILING_LF = 24;
for (const stratum of ["pre-tabs", "mono-wrap"]) {
  for (let i = 0; i < TRAILING_LF; i++) {
    const item = STRATA[stratum]();
    const span = item.spans[0];
    span.t = rnd() < 0.2 ? pick(["\n", "\n\n"]) : span.t + pick(["\n", "\n", "\n\n", "\n\n\n"]);
    for (const width of [390, 800]) layouts.push({ id: `${stratum}-lf${i}-${width}`, stratum, width, ...item });
  }
}
writeFileSync(outPath, JSON.stringify(layouts));
console.log(layouts.length, "layouts,", Object.keys(STRATA).length, "strata");
