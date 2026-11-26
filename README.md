# sosie

sosie renders GitHub write actions, such as a comment or a review, as a PNG that matches github.com. It runs as a Rust core, with no browser and no network.

The repository is private and under construction.

## Layout

| Path | Content |
| --- | --- |
| `crates/sosie-core` | text layout, font selection, display list and raster |
| `assets/fonts` | the pinned fonts and their licenses |
| `tests/golden` | Chromium goldens: geometry in git, PNGs in a pinned release asset |
| `tools/oracle` | the reference container and the capture harness |

`SPEC.md` lists the rules the core follows and the gate numbers.

Pixel parity against Chromium: budget B is 0.815 % of pixels per pool of stratum and theme, and each capture may reach 1.5 %. On the synthetic suite the worst pool is sans-prose dark at 0.8080 %, and the worst capture is 0.9656 %.

## Checks

```sh
tools/fetch-goldens.sh
cargo test --workspace --release
node tools/golden-stamp.mjs --check
cargo run --release --example bench_text
```

`tools/fetch-goldens.sh` needs `gh` and `jq`. Without the bundle the pixel test skips locally and fails in CI.

## License

Apache-2.0. See `LICENSE` and `NOTICE`. The fonts in `assets/fonts` are under the SIL Open Font License 1.1.
