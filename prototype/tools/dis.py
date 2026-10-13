"""Disassemble exported functions of an x64 DLL (read-only, never loads the DLL).

    python -I dis.py <dll> <export> [max_insns]
Follows a leading `jmp` thunk once.
"""
import sys

import pefile
from capstone import CS_ARCH_X86, CS_MODE_64, Cs


def main():
    path, name = sys.argv[1], sys.argv[2]
    limit = int(sys.argv[3]) if len(sys.argv) > 3 else 400
    pe = pefile.PE(path)
    base = pe.OPTIONAL_HEADER.ImageBase
    exports = {e.name.decode(): e.address for e in pe.DIRECTORY_ENTRY_EXPORT.symbols if e.name}
    names_by_rva = {v: k for k, v in exports.items()}
    rva = exports[name]
    md = Cs(CS_ARCH_X86, CS_MODE_64)
    image = pe.get_memory_mapped_image()
    for hop in range(2):
        code = image[rva : rva + 0x2000]
        first = next(md.disasm(code, base + rva))
        if first.mnemonic == "jmp" and first.op_str.startswith("0x"):
            rva = int(first.op_str, 16) - base
            continue
        break
    print(f"; {name} @ rva {rva:#x}")
    n = 0
    for ins in md.disasm(image[rva : rva + 0x4000], base + rva):
        note = ""
        if ins.mnemonic == "call" and ins.op_str.startswith("0x"):
            target = int(ins.op_str, 16) - base
            note = f"   ; -> {names_by_rva.get(target, hex(target))}"
        print(f"{ins.address - base:08x}  {ins.mnemonic:6s} {ins.op_str}{note}")
        n += 1
        if ins.mnemonic in ("ret", "int3") or n >= limit:
            break


if __name__ == "__main__":
    main()
