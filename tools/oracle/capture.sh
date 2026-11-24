#!/usr/bin/env bash
set -euo pipefail

image="${SOSIE_REF_IMAGE:-sosie-ref:stage0}"
root="$(cd "$(dirname "$0")/../.." && pwd)"
text="tests/golden/synthetic/text"
paint="tests/golden/synthetic/paint"
stage="target/oracle"

cd "$root"
mkdir -p "$text" "$paint" "$stage"

node tools/oracle/corpus.mjs 20261124 "$text/corpus-1.json"
node tools/oracle/corpus.mjs 777 "$text/corpus-2.json"
node tools/oracle/paint-cases.mjs 4242 "$text/corpus-2.json" "$paint/cases-1.json"
node tools/oracle/paint-cases.mjs 99 "$text/corpus-1.json" "$paint/cases-2.json"

oracle() {
  docker run --rm --platform linux/amd64 -v "$root:/work" -w /work "$image" node tools/oracle/text-parity.mjs "$@"
}

oracle geometry "$text/corpus-1.json" "$text/chrome-1.json" | tee "$stage/geometry-1.log"
oracle geometry "$text/corpus-2.json" "$text/chrome-2.json" | tee "$stage/geometry-2.log"
for set in 1 2; do
  rm -rf "$stage/paint-$set"
  oracle paint "$paint/cases-$set.json" "$stage/paint-$set" | tee "$stage/paint-$set.log"
  mv "$stage/paint-$set/boxes.json" "$paint/boxes-$set.json"
done
docker image inspect "$image" --format '{{.Id}}' > "$stage/image-id"
