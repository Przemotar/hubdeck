"""Minimal read-only asar reader: list files or extract them into an output dir.

    python -I asar.py list <archive.asar> [filter]
    python -I asar.py extract <archive.asar> <out_dir> [filter]
"""
import json
import os
import struct
import sys


def read_header(f):
    f.seek(0)
    _, header_size, _, json_size = struct.unpack("<IIII", f.read(16))
    header = json.loads(f.read(json_size).decode("utf-8"))
    return header, 8 + header_size


def walk(node, prefix=""):
    for name, child in node.get("files", {}).items():
        path = f"{prefix}/{name}" if prefix else name
        if "files" in child:
            yield from walk(child, path)
        else:
            yield path, child


def main():
    cmd, archive = sys.argv[1], sys.argv[2]
    with open(archive, "rb") as f:
        header, base = read_header(f)
        if cmd == "list":
            flt = sys.argv[3] if len(sys.argv) > 3 else ""
            for path, node in walk(header):
                if flt in path:
                    print(f"{node.get('size', 0):>10} {'U ' if node.get('unpacked') else '  '}{path}")
        elif cmd == "extract":
            out, flt = sys.argv[3], (sys.argv[4] if len(sys.argv) > 4 else "")
            n = 0
            for path, node in walk(header):
                if flt not in path or node.get("unpacked") or "offset" not in node:
                    continue
                f.seek(base + int(node["offset"]))
                data = f.read(node["size"])
                dest = os.path.join(out, *path.split("/"))
                os.makedirs(os.path.dirname(dest), exist_ok=True)
                with open(dest, "wb") as o:
                    o.write(data)
                n += 1
            print("extracted", n)


if __name__ == "__main__":
    main()
