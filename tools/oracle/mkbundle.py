import gzip
import hashlib
import io
import os
import sys
import tarfile

stage, out = sys.argv[1], sys.argv[2]
items = []
for s in ("1", "2"):
    src = os.path.join(stage, f"paint-{s}")
    items += [(os.path.join(src, n), f"synthetic-paint/{s}/{n}") for n in sorted(os.listdir(src)) if n.endswith(".png")]

buf = io.BytesIO()
with tarfile.open(fileobj=buf, mode="w", format=tarfile.PAX_FORMAT) as t:
    for path, name in items:
        data = open(path, "rb").read()
        info = tarfile.TarInfo(name)
        info.size, info.mtime, info.mode, info.uid, info.gid, info.uname, info.gname = len(data), 0, 0o644, 0, 0, "", ""
        t.addfile(info, io.BytesIO(data))

packed = io.BytesIO()
with gzip.GzipFile(fileobj=packed, mode="wb", mtime=0, compresslevel=9, filename="") as g:
    g.write(buf.getvalue())
data = packed.getvalue()
open(out, "wb").write(data)
print(len(items), len(data), hashlib.sha256(data).hexdigest())
