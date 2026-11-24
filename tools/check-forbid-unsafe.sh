#!/usr/bin/env bash
set -euo pipefail

status=0
for manifest in crates/*/Cargo.toml; do
  dir="$(dirname "$manifest")"
  for root in "$dir/src/lib.rs" "$dir/src/main.rs"; do
    [ -f "$root" ] || continue
    if ! grep -qx '#!\[forbid(unsafe_code)\]' "$root"; then
      echo "$root lacks #![forbid(unsafe_code)]" >&2
      status=1
    fi
  done
done
exit "$status"
