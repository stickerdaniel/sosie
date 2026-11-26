#!/usr/bin/env bash
set -euo pipefail

lock="tests/golden/pixels.lock.json"
dest="${GOLDENS_DIR:-target/goldens}"
repo="${GOLDENS_REPO:-${GITHUB_REPOSITORY:-stickerdaniel/sosie}}"
mkdir -p "$dest"

sha256() {
  if command -v sha256sum >/dev/null; then sha256sum "$1"; else shasum -a 256 "$1"; fi | cut -d' ' -f1
}

jq -r '.bundles | to_entries[] | [.key, .value.asset, .value.sha256] | @tsv' "$lock" |
while IFS=$'\t' read -r tag asset want; do
  gh release download "$tag" --repo "$repo" --pattern "$asset" --dir "$dest" --clobber
  got="$(sha256 "$dest/$asset")"
  if [ "$got" != "$want" ]; then
    echo "bundle $tag: $asset has SHA-256 $got, lock pins $want" >&2
    exit 1
  fi
  tar -xzf "$dest/$asset" -C "$dest"
  echo "bundle $tag ok: $asset $want"
done
