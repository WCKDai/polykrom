#!/usr/bin/env python3
"""Pack a URDF's binary STL meshes into one compact file for the web viewer: shared vertices,
positions quantised to 16 bits within each mesh's box, 16-bit indices where they fit.

    meshpack.py URDF_DIR OUT_BIN OUT_JSON
"""
import json, struct, sys
from pathlib import Path


def read_stl(path):
    data = path.read_bytes()
    n = struct.unpack_from("<I", data, 80)[0]
    verts, index, seen = [], [], {}
    for t in range(n):
        o = 84 + t * 50 + 12
        for v in range(3):
            p = struct.unpack_from("<3f", data, o + v * 12)
            k = seen.get(p)
            if k is None:
                k = seen[p] = len(verts)
                verts.append(p)
            index.append(k)
    return verts, index


def pack(urdf_dir, out_bin, out_json):
    blob, meshes = bytearray(), {}
    for stl in sorted(Path(urdf_dir).glob("*.stl")):
        verts, index = read_stl(stl)
        if not verts:
            continue
        lo = [min(v[i] for v in verts) for i in range(3)]
        hi = [max(v[i] for v in verts) for i in range(3)]
        span = [max(h - l, 1e-12) for l, h in zip(lo, hi)]
        pos_at = len(blob)
        for v in verts:
            blob += struct.pack("<3H", *(round((v[i] - lo[i]) / span[i] * 65535) for i in range(3)))
        while len(blob) % 4:
            blob += b"\0"
        wide = len(verts) > 65535
        idx_at = len(blob)
        blob += struct.pack(f"<{len(index)}{'I' if wide else 'H'}", *index)
        while len(blob) % 4:
            blob += b"\0"
        meshes[stl.name] = {"pos": pos_at, "verts": len(verts), "idx": idx_at, "count": len(index), "wide": wide, "min": lo, "max": hi}
    Path(out_bin).write_bytes(bytes(blob))
    Path(out_json).write_text(json.dumps({"meshes": meshes}, separators=(",", ":")))
    return len(blob), len(meshes)


if __name__ == "__main__":
    size, n = pack(*sys.argv[1:4])
    print(f"{n} meshes, {size / 1e6:.2f} MB")
