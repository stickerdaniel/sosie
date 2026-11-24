#!/usr/bin/env bash
set -euo pipefail

pinned="sha256:5efae82b8506de988fbb5ce15766350e339cf9da75ee8957cc8f10148e988f9b"
image="${SOSIE_REF_IMAGE:-$pinned}"
root="$(cd "$(dirname "$0")/../.." && pwd)"
text="tests/golden/synthetic/text"
paint="tests/golden/synthetic/paint"
stage="target/oracle"

cd "$root"
id="$(docker image inspect "$image" --format '{{.Id}}')"
if [ "$id" != "$pinned" ]; then
  echo "reference image $image is $id, capture.sh pins $pinned" >&2
  exit 1
fi
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
echo "$id" > "$stage/image-id"
