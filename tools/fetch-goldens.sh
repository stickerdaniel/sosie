#!/usr/bin/env bash
set -euo pipefail

lock="tests/golden/pixels.lock.json"
dest="${GOLDENS_DIR:-target/goldens}"
repo="${GOLDENS_REPO:-${GITHUB_REPOSITORY:-stickerdaniel/sosie}}"
mkdir -p "$dest"

jq -r '.bundles | to_entries[] | [.key, .value.asset, .value.sha256] | @tsv' "$lock" |
while IFS=$'\t' read -r tag asset sha; do
  gh release download "$tag" --repo "$repo" --pattern "$asset" --dir "$dest" --clobber
  echo "$sha  $dest/$asset" | sha256sum --check --strict
  tar -xzf "$dest/$asset" -C "$dest"
  echo "bundle $tag ok: $asset $sha"
done
