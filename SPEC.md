# sosie spec

This file states what the core guarantees, how each guarantee is checked, and the numbers the checks measured. The core is `crates/sosie-core`.

## Scope of the text and paint core

- Input: a paragraph of styled inline spans and inline boxes, laid out at a CSS width.
- Output: line geometry, a display list, and an RGBA pixmap or PNG at a device pixel ratio from 1 to 4.
- The core opens no file, makes no network call, and reads no clock. Every crate root carries `#![forbid(unsafe_code)]`, checked by `tools/check-forbid-unsafe.sh`.

## Fonts

| Pack | Faces | Loading |
| --- | --- | --- |
| core | Noto Sans regular, bold, italic, bold italic; Liberation Mono regular, bold | embedded in the binary |
| emoji | Noto Color Emoji (CBDT) | host registers the bytes |
| cjk | Noto Sans CJK regular | host registers the bytes |
| arabic | Noto Sans Arabic regular | host registers the bytes |
| hebrew | Noto Sans Hebrew regular | host registers the bytes |

- `assets/fonts.lock.json` pins each file by SHA-256, size and Ubuntu package version. `Fonts::register` accepts only bytes with a pinned hash and returns the pack they complete.
- A CSS family list keeps the names it knows and drops the rest. `sans-serif` expands to Noto Sans, Noto Sans CJK and Noto Color Emoji. `serif` maps to Noto Sans, `monospace` to Liberation Mono. In GitHub's stacks only `Noto Sans`, `Liberation Mono`, `sans-serif` and `monospace` match.
- Face choice follows CSS weight matching and the italic flag.
- Fallback runs per grapheme cluster. Clusters with emoji presentation, VS16, a keycap or a regional indicator go to Noto Color Emoji. Other clusters try the primary face, then the family list, then Noto Sans, Noto Sans CJK, Noto Sans Arabic, Noto Sans Hebrew, Liberation Mono and Noto Color Emoji. An unloaded pack is skipped.
- Synthetic bold and synthetic oblique are out of scope. Bold CJK, Arabic and Hebrew render in the regular face, and the layout carries the warning `synthetic-bold-unsupported`. Faces not in the lock are refused.

## Text layout

- Line breaking: ICU4X UAX #14 at `line-break: auto` (Normal strictness), with Blink's ASCII fast path.
- Shaping: harfrust per run of face, size, script and direction. A span boundary splits a run only when padding sits at it, so equal adjacent spans keep kerning and Arabic joining. Paint splits each shaped run back into its spans. A span piece starts at the leftmost layout position of all graphemes its glyph clusters cover, so an RTL ligature paints from its visual left edge. The CJK face gets the `chws` feature. A CJK open bracket after punctuation, and a CJK closing bracket or full stop before punctuation, lose half of their own font size in advance. Paint removes the trim on the glyph's blank side: the left of an open bracket and the right of a closing bracket or full stop. A right-to-left run mirrors brackets, so their sides swap there; full stops and commas keep theirs.
- Sizes are truncated to 26.6 fixed point (`floor64(size * dpr)`). Advances come from that size, or from the strike ppem for a bitmap face.
- Line fitting follows Blink: item widths rounded up to 1/64 px, a fit check with 1/64 px tolerance on every line, a reshape at each candidate break, and an `overflow-wrap: anywhere` fallback.
- `white-space`: `normal`, `pre`, `pre-wrap` and `break-spaces`, with `tab-size`. A span's own `white-space` governs breaking inside it and after its spaces and tabs. A break between two spans after any other character needs the span before it or the strut to allow wrapping. LF and CRLF end a preserved line. A preserved newline at the end of the text adds no line. `break-spaces` allows a break after every preserved space and tab. `pre-wrap` allows one after the last space of a run. `pre-wrap` and `normal` whitespace also allow one before whitespace of another mode. After a soft break, the `normal` spaces that follow hang on the line. The `pre-wrap` spaces and tabs that follow hang too, unless the break follows a tab and their span continues past them, a preserved newline included. A `normal` space after a preserved newline collapses. A tab size of 0 gives a tab no advance.
- Font size: 0 to 256 CSS px, else `layout` returns `LayoutError::FontSize`. 256 px times the maximum dpr of 4 is 1,024 device px, the largest size the glyph raster draws. GitHub's largest text, 32 px, times D6's largest font scale, 2, is 64 px.
- Vertical metrics: `above = round(ascent) + floor((line-height - ascent - descent) / 2)`. A line takes the maximum over the strut and every span on it. An inline box counts as its height above the baseline.
- Bidi: unicode-bidi per paragraph. Runs on a line are placed in visual order. An RTL base paragraph aligns to the right edge.

## Raster

- Paths: vertex y snapped to 1/4 device px. Quadratics are chopped at y extrema and flattened with Skia's `diff_to_shift` (accuracy shift 2, 1 to 6 subdivisions).
- Coverage: analytic area coverage with 16 y-subsamples per pixel.
- Glyphs: x quantized to 1/4 device px, baseline y rounded to the device pixel.
- Mask gamma: Skia's preblend with contrast 0.2 and gamma 1.2. The table is chosen by the top 3 bits of the luminance `(r*54 + g*183 + b*19) >> 8`. The 8 tables are embedded as bytes, so no float library call reaches the output.
- Emoji: the 109 ppem CBDT strike, one 2 by 2 box mip level per halving, then bilinear sampling.
- Blending: `(s*a + d*(255 - a) + 127) / 255` per channel. The page background must be opaque.
- Inline code backgrounds: device rows from `ceil(base - ascent - padding)` to `floor(base + descent + padding)`, horizontal edges rounded.

## Limits

`Limits::default()` accepts a dpr from 1 to 4, a device width up to 8,192 px, a device height up to 32,768 px and up to 64M pixels. `rasterize` checks the device size with saturating arithmetic before it allocates. A fill mask covers only the part of a shape inside the pixmap. Glyph and emoji blits clip with saturating 64-bit spans, so a glyph at any finite position outside the pixmap is skipped. The glyph cache holds at most 12 MiB and the emoji strike cache at most 64 MiB; a cache that would exceed its budget is emptied. A failed emoji lookup is not cached. Text length has no cap yet.

## Goldens

The reference is Chromium 141.0.7390.37 in the container built from `tools/oracle/ref/Dockerfile` (image `sha256:5efae82b8506de988fbb5ce15766350e339cf9da75ee8957cc8f10148e988f9b`, linux/amd64; `capture.sh` refuses any other image id), with the fonts from `assets/fonts.lock.json`, locale en-US, timezone UTC, a fixed clock and dpr 2.

- Text geometry: `tests/golden/synthetic/text`. Two seeded corpora, 4,224 layouts, block heights, line texts and line widths from Chromium. 192 of them, in `pre-tabs` and `mono-wrap`, end in one to three LF or consist of LF only.
- Paint: `tests/golden/synthetic/paint`. Two seeded case sets of 240 cases each, border boxes from Chromium. The PNGs are in the release asset pinned by `tests/golden/pixels.lock.json`; `tools/fetch-goldens.sh` downloads and verifies it.
- `tests/golden/stamps.json` records the hashes of inputs, geometry, harness, fonts lock and bundle. `node tools/golden-stamp.mjs --check` fails on any drift.
- `tools/oracle/capture.sh` recaptures everything in the container.

## Gates

### Text

A layout matches when its height is within 1/64 px of Chromium's block height, every line break falls at the same text offset and every line width is within 4/64 px of Chromium's. Chromium's line list omits lines without text, so the block height carries empty lines. The gate requires the 11 strata of stage 0, at least 2,000 gated layouts, and 0 height, break and width mismatches. CJK is reported and not gated. Every layout's height matches Chromium's exactly.

| Stratum | Layouts | Break mismatches | Width mismatches | Max width diff (px) |
| --- | --- | --- | --- | --- |
| arabic | 336 | 0 | 0 | 0.0153 |
| bold-italic | 336 | 0 | 0 | 0.0276 |
| cyrillic | 336 | 0 | 0 | 0.0152 |
| emoji | 336 | 0 | 0 | 0.0154 |
| greek | 336 | 0 | 0 | 0.0149 |
| hebrew | 336 | 0 | 0 | 0.0153 |
| latin-prose | 336 | 0 | 0 | 0.0154 |
| long-tokens | 336 | 0 | 0 | 0.0211 |
| mixed-mono | 336 | 0 | 0 | 0.0168 |
| mono-wrap | 432 | 0 | 0 | 0.0137 |
| pre-tabs | 432 | 0 | 0 | 0.0269 |
| cjk (reported) | 336 | 2 | 7 | 0.7019 |

Gated layouts: 3,888.

### Pixels

A pixel differs when no pixel within 1 px of it in the Chromium PNG lies within 8 levels on every channel. The strict value uses 2 levels and no neighbour and gates nothing.

A pool is one stratum in one theme, over both case sets: 8 pools. Its ratio is the sum of differing pixels over the sum of counted pixels. Budget B is 0.815 % per pool, the largest pooled held-out ratio measured in stage 0, rounded up. The residual comes from curve edges of the analytic AA approximation: `o` and `e` differ on curve pixels, `l`, `H` and `W` match within 8 levels. A single capture may reach 1.5 %.

The gate covers all 480 captures of `goldens-1` (240 authored cases, light and dark), at their recorded box sizes. Cases are keyed by set and case ID. It requires at least 50 text captures, each pool at or below B, each capture at or below 1.5 %, and each case at Chromium's border box size.

| Stratum | Captures | Dark gate / strict | Light gate / strict | Worst capture |
| --- | --- | --- | --- | --- |
| sans-prose | 144 | 0.8080 / 3.0916 % | 0.7477 / 4.4840 % | 0.9656 % (set-2/sans-prose-dark-4) |
| mono-code | 144 | 0.5719 / 3.2173 % | 0.5269 / 3.9998 % | 0.9422 % (set-1/mono-code-dark-23) |
| emoji | 96 | 0.3282 / 1.7003 % | 0.3173 / 2.1829 % | 0.6069 % (set-1/emoji-light-22) |
| borders-fills | 96 | 0.0849 / 0.2163 % | 0.0913 / 0.1832 % | 0.4464 % (set-1/fills-dark-0) |

Text captures: 384. Fill captures: 96. 56 captures lie above B, none above 1.5 %. The bundle `goldens-2` holds the same 480 PNG files as `goldens-1`, byte for byte, under the paths `synthetic-paint/<set>/<case>.png`.

### Determinism

`tests/determinism.json` pins the PNG SHA-256 of four cases (ASCII, emoji, CJK, RTL) at 784 px and dpr 2. The same hashes hold on linux-x64 and darwin-arm64. CI runs the test on both.

## Bench

`cargo run --release --example bench_text` renders a 10-line comment at 800 px and dpr 2 (1,600 by 500 px) and reports p50 and p95 over 200 warm runs. See the PR that last changed the numbers for the measuring machine.
