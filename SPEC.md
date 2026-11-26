# sosie spec

This file states what the core guarantees, how each guarantee is checked, and the numbers the checks measured. The core is `crates/sosie-core`.

## Scope of the text and paint core

- Input: a paragraph of styled inline spans and inline boxes, laid out at a CSS width.
- Output: line geometry, a display list, and an RGBA pixmap, WebP or PNG at a device pixel ratio from 1 to 4.
- The core opens no file, makes no network call, and reads no clock. Every crate root carries `#![forbid(unsafe_code)]`, checked by `tools/check-forbid-unsafe.sh`.

## Output

`encode(&pixmap, Format)` writes WebP or PNG. Both are lossless and decode to the pixmap byte for byte, checked by `encoded_images_decode_to_the_pixmap`.

- WebP: VP8L lossless from image-webp 0.2.4, a Rust port of libwebp 1.6.0's lossless encoder (`github.com/marcbachmann/image-webp`, commit `f99a7c20abce962bfcc7950792fe27e1d1a9b6bc`). Settings: method 4, quality 75, `exact` on, so fully transparent pixels keep their RGB, near-lossless 100, fast mode off. Each side is at most 16,383 px, else `EncodeError::TooLarge`.
- PNG: 8-bit RGBA, compressed at png's `Balanced` level with `Adaptive` filters by flate2's pure-Rust backend. `encode_png` returns the same bytes.
- `encode` checks the pixmap before it calls an encoder: a zero side is `EncodeError::Empty`, an RGBA length other than width × height × 4 is `EncodeError::Buffer`. An encoder failure or empty output is `EncodeError::Encoder`.
- The port computes some entropy estimates with `f64::ln`, a system libm call, for histogram counts of 65,536 and more. The raster rule that no float library call reaches the output therefore does not cover WebP bytes. The cross-platform hash gate in Determinism covers them.
- The port's SIMD kernels use `unsafe`. On x86_64 it picks AVX2 at runtime unless `IMAGE_WEBP_NO_AVX2` is set.

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
- Shaping: harfrust per run of face, size, script and direction. A span boundary splits a run only when padding sits at it, so equal adjacent spans keep kerning and Arabic joining. Paint splits each shaped run back into its spans. A span piece starts at the leftmost layout position of all graphemes its glyph clusters cover, so an RTL ligature paints from its visual left edge. The CJK face gets the `chws` feature. A CJK open bracket after punctuation, and a CJK closing bracket or full stop before punctuation, lose half of their own font size in advance, at most their whole advance. Paint removes the trim on the glyph's blank side: the left of an open bracket and the right of a closing bracket or full stop. A right-to-left run mirrors brackets, so their sides swap there; full stops and commas keep theirs. A closing bracket at the end of an overflowing line loses half its size too, on any line under a wrapping strut and on a soft-wrapped line under a `pre` strut. An RTL paragraph aligns the line by its trimmed width.
- Sizes are truncated to 26.6 fixed point (`floor64(size * dpr)`). Advances come from that size, or from the strike ppem for a bitmap face.
- Line fitting follows Blink: item widths rounded up to 1/64 px, a fit check with 1/64 px tolerance on every line, a reshape at each candidate break, and an `overflow-wrap: anywhere` fallback. Each line starts without that fallback and turns it on when a wrapping span or the strut applies its style on that line. An overflow in a `pre` span that starts the line therefore gets no retry. A candidate whose cut HarfBuzz marks safe to break reuses the last reshape's advances. Measuring one line's candidates reshapes at most 6 times over the normal pass and the `overflow-wrap: anywhere` pass together, and later candidates reuse the last reshape's advances. The fallback measures each prefix with the same 1/64 px tolerance. It breaks only where the white-space rules allow a break, and at the first such position when no break fits.
- `white-space`: `normal`, `pre`, `pre-wrap` and `break-spaces`, with `tab-size`. Line breaking ports Blink's `LineBreaker` from Chromium 141.0.7390.37 (`line_breaker.cc`, `shaping_line_breaker.cc`); NOTICE carries Chromium's license.
  - Items: every span gives an open and a close item that carry its inline padding, text items split at span, bidi level, tab and newline boundaries, tab runs, CR and LF items, and one item per inline box. Spaces that start the paragraph or follow a preserved newline in a `pre-wrap` or `break-spaces` span get a generated break opportunity after them and form their own item, so their width rounds up to 1/64 px on its own.
  - Lines: each line walks the items through Blink's states (continue, trailing, overflow, done) and handlers for text, trailing spaces, forced breaks, controls, inline boxes, open and close tags, overflow and rewind. After an overflow, the breaker walks back to the last break opportunity and breaks text inside an item where it may. `normal` trailing spaces collapse, `pre-wrap` trailing spaces hang, and `break-spaces` trailing spaces take space and may break after each one.
  - Opportunities: after a run of spaces, tabs and newlines (UAX #14 LB7 and LB18), after each of them under `break-spaces` (CSS Text 3), never at another control character, and else UAX #14. A space run also allows a break before a CRLF that follows it, as before a lone CR. Between `-` and a digit only when an ASCII letter or digit precedes the `-`, as Chromium 141 breaks. A close tag moves the opportunity of the item before it behind itself. A close tag of a wrapping span allows a break before a following space or tab; under a `pre-wrap` or `break-spaces` strut only when a space or tab also precedes it.
  - Line boxes: a line takes height when its last item carries Blink's `should_create_line_box`. Text, preserved trailing spaces, a forced break, an inline box and a span edge with padding set it, and every later item on the line keeps it.
  - A run of tabs in one span at one bidi level forms one item and rounds up to 1/64 px as a whole. In LTR its first tab, in RTL its last tab, advances to the next tab stop of the strut's font from the run's position on the line; every other tab advances by the full tab width. That position counts the padding of a span with no text only when the span's open item is on the same line. LF and CRLF end a preserved line, and one at the end of the text adds no line. A CR and an LF in different spans stay two characters with no break opportunity between them. The `overflow-wrap: anywhere` retry breaks at every grapheme boundary, so it may break between a lone CR and a CRLF that follows it. A lone preserved CR paints nothing, takes no width and adds no break opportunity. A `normal` space after a preserved newline collapses. A `normal` space at a line start, alone or after preserved CRs, paints nothing and takes no width, also when the breaker measures the line. A tab size of 0 gives a tab no advance.
  - Collapse follows Blink's `InlineItemsBuilder`: a collapsible run that holds a newline disappears when U+200B precedes or follows it, also across spans. The generated break opportunity after spaces that start the paragraph or follow a preserved newline in a `pre-wrap` or `break-spaces` span counts as a U+200B before such a run. A space that an earlier span lost this way comes back when later text or an inline box follows. An empty text span is transparent to collapsing. A collapsible space at the end of the paragraph is removed, also behind close items.
  - Padding: a span whose text collapses to nothing keeps both paddings before the next content, on the line that also holds its open or close item. The padding of a `normal` trailing space removed at a line end stays on the line.
- Font size: 0 to 256 CSS px, else `layout` returns `LayoutError::FontSize`. 256 px times the maximum dpr of 4 is 1,024 device px, the largest size the glyph raster draws. GitHub's largest text, 32 px, times D6's largest font scale, 2, is 64 px.
- Vertical metrics: ascent and descent round to whole px, as Blink's `FontMetrics::AscentDescentWithHacks` does. Above a `Paragraph::dpr` of 1, Chromium on Linux turns on subpixel positioning, and a descent that rounded down then takes 1 px from the ascent. `above = ascent + floor((line-height - ascent - descent) / 2)`. A line takes the maximum over the strut, every span with a grapheme or an open or close item on it, and the span of the preserved newline that ends it. An inline box counts as its height above the baseline.
- Bidi: unicode-bidi per paragraph, with rule L1, so tabs and the whitespace before them take the paragraph level. U+200B and other BN characters take the level of the next character before a paragraph or segment separator. A preserved CR takes the level of the next character in its span other than CR or LF, else of the previous character in its span, else of the next character, else of the previous one unless that is an LF. CRs at the end of a line with nothing after them take the paragraph level. A `normal` space removed at a line start, and the CRs before it, take the level of what follows. Runs on a line are placed in visual order. Each visual run of one span carries the span's inline-start and inline-end padding; in an RTL paragraph inline-start is on the right. A `normal` space that stays in the line results at a line end and is followed by preserved hanging whitespace forms its own run at the paragraph level, and that run carries the end padding of its span. The same holds when a CR follows that space on the line: the space takes the paragraph level and its run carries the end padding. Before an LF alone the end padding stays with the span's text. A `normal` space that the breaker skips at the end of an overflowing line, after an `overflow-wrap: anywhere` break, belongs to no line result: it takes no width, takes the level of the text before it in its span, and leaves the end padding with that text. An RTL base paragraph aligns to the right edge.

## Raster

- Paths: vertex y snapped to 1/4 device px. Quadratics are chopped at y extrema and flattened with Skia's `diff_to_shift` (accuracy shift 2, 1 to 6 subdivisions).
- Coverage: analytic area coverage with 16 y-subsamples per pixel.
- Glyphs: x quantized to 1/4 device px, baseline y rounded to the device pixel.
- Mask gamma: Skia's preblend with contrast 0.2 and gamma 1.2. The table is chosen by the top 3 bits of the luminance `(r*54 + g*183 + b*19) >> 8`. The 8 tables are embedded as bytes, so no float library call reaches the output.
- Emoji: the 109 ppem CBDT strike, one 2 by 2 box mip level per halving, then bilinear sampling.
- Blending: `(s*a + d*(255 - a) + 127) / 255` per channel. The page background must be opaque.
- Clipped fills: the fill's mask and the clip's mask cover the fill's device bounds, and each pixel takes `(a*b + 127) / 255` of both.
- Inline code backgrounds: device rows from `ceil(base - ascent - padding)` to `floor(base + descent + padding)`, horizontal edges rounded.

## Limits

`Limits::default()` accepts a dpr from 1 to 4, a device width up to 8,192 px, a device height up to 32,768 px and up to 64M pixels. `rasterize` checks the device size with saturating arithmetic before it allocates. A size whose RGBA byte count exceeds `isize::MAX` fails with `LimitError::Bytes`. A fill mask covers only the part of a shape inside the pixmap. Glyph and emoji blits clip with saturating 64-bit spans, so a glyph at any finite position outside the pixmap is skipped. The glyph cache holds at most 12 MiB and the emoji strike cache at most 64 MiB; a cache that would exceed its budget is emptied. A failed emoji lookup is not cached. Text length has no cap yet.

## Chart

`chart::render` draws GitHub-style metric bars from a `Chart` at a whole CSS width W. The reference uses W = 814 px. It ports the HTML template of `metric-bars.ts` in agent-setup `shared/skills/pr-media`. Colors are the 12 `@primer/primitives` 11.10.0 tokens in `assets/primer-tokens.json`; `node tools/primer-tokens.mjs` refreshes the file from npm, and a test compares it with `Palette::LIGHT` and `Palette::DARK`. Text uses the primer system font stack, which resolves to Noto Sans.

- Box: a 1 px `borderColor-default` border with radius 6, on `bgColor-default`. Content starts 17 px inside each side.
- Header, only for a non-empty title: 12 px padding above and below, 14/21 px text at weight 600 wrapped at W - 34, a `bgColor-muted` background with top radii 5, and a 1 px `borderColor-default` bottom border.
- Row: 12 px padding above, 14 px below, and a 1 px `borderColor-muted` top border on every row after the first. The name, 14/21 px at weight 600, wraps at W - 50 minus the change's advance. The change, 14/21 px at weight 600 in the tone color, does not wrap, ends at W - 17 and shares the name's first baseline.
- Bars: a before bar and an after bar, 22 px each, 6 px below the name. The track is 10 px high, W - 194 px wide, with radius 6, which CSS fits to 5. The fill takes max(|v| / max(|before|, |after|), 0.5 %) of the track, its own radius 6 fits to min(5, width / 2), and the track's rounded rect clips it. Before fills are `borderColor-emphasis`, after fills follow the tone.
- Value column: 144 px from W - 161, with the key `Before` or `After` on the left and the value on the right, 12/18 px, centered in the bar.
- Height: 1 + header + rows + separators + 1. The reference input of four metrics with a title is 814 by 439 CSS px.
- Formatting: `format_value` matches `Intl.NumberFormat('en-US', { maximumFractionDigits: 2 })` with U+2212 as minus. It rounds the decimal that JS `Number.prototype.toString` picks: the shortest round-trip digit count, and of those the decimal nearest the binary value, ties to even. The value text collapses whitespace runs to one space and trims both ends, as `white-space: nowrap`, and stays on one line. `percent_change` rounds the exact binary value to 0 digits at 10 % or more and to 1 digit below, ties away from zero, as `toFixed`. `tests/chart-format.json`, generated by Bun from the JS functions, pins both.
- Tone: neutral at 0 %, success when the sign of the change matches `lower_is_better`, danger otherwise.
- Errors: no metrics, an empty name, a non-finite value, before 0, a change that is not finite or reaches 1e21 (JS would switch to exponent notation), W outside 310 to 4096 CSS px, a dpr 2 surface beyond `Limits::default()`, a title or name line wider than its width plus the line breaker's 1/64 px tolerance, and key plus value wider than 144 px. A metric error carries the metric index.
- The Chromium gates do not cover charts. Tests check the geometry against numbers derived from the CSS.

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

`crates/sosie-core/tests/determinism.json` pins the PNG and the WebP SHA-256 of each case under `<case>.png` and `<case>.webp`:

- four text cases (ASCII, emoji, CJK, RTL) at 784 px and the reference chart in light and dark at 814 px, all at dpr 2;
- `gradient-333x77`, an odd-sized gradient;
- `transparent-64x48`, fully transparent pixels with non-zero RGB;
- `flat-400x200`, more than 256 colors and 78,400 pixels of one color, which reaches the port's `f64::ln` path.

One hash set holds on linux-x64 and darwin-arm64. CI runs the test on both, and on linux-x64 a second time with `IMAGE_WEBP_NO_AVX2=1`, which turns off the port's AVX2 kernels.

## Bench

`cargo run --release --example bench_text` renders a 10-line comment at 800 px and dpr 2 (1,600 by 500 px) and reports p50 and p95 over 200 warm runs. See the PR that last changed the numbers for the measuring machine.
