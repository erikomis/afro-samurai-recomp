#!/usr/bin/env python3
"""Add jump-table case targets to the lifter's function list.

    augment_functions.py EBOOT.elf functions.json jumptables.txt out.json

The lifter resolves a `switch` only over the cases inside the function it
lifted, and it may cut a large function where another branch target begins.
Cases past that cut go through the indirect-call fallback, which needs a lifted
FUNCTION at the case address; a plain case label is not one, so the call is
dropped ("[ppu] unresolved indirect call -> 0x...") and the title runs on with
garbage. Afro Samurai's message dispatcher (func_001EA288, 4191 cases) lost a
case that way, and New Game then spun for ever on a NULL object.

This splits the function list at every target of the listed tables. A case
that falls through into the next one is still correct: the lifter emits a
fall-through tail call when a function ends without a terminator.

jumptables.txt, one table per line (hex, '#' comments):
    <r2 offset of the module TOC slot> <module offset of the table pointer> <entries>
Each entry is a signed 32-bit offset from the table base (GCC's PPC64 tables).
"""
import json
import struct
import sys

TOC = 0x8FFFE0   # r2 of BLUS30264; the tables are reached through module TOCs


def load_segments(path):
    e = open(path, "rb").read()
    ph = struct.unpack(">Q", e[32:40])[0]
    phs, phn = struct.unpack(">HH", e[54:58])
    segs = [struct.unpack(">IIQQQQQQ", e[ph + i * phs:ph + i * phs + 56]) for i in range(phn)]
    def r32(va):
        for kind, _, off, v, _, fs, _, _ in segs:
            if kind == 1 and v <= va < v + fs:
                return struct.unpack(">I", e[off + va - v:off + va - v + 4])[0]
        raise SystemExit(f"0x{va:08X} is not in a loaded segment")
    return r32


TEXT_LO, TEXT_HI = 0x10000, 0x10000 + 0x88DBC8   # BLUS30264 executable segment


def auto_tables(eboot, r32):
    """Find GCC PPC64 jump tables: within a few dozen instructions before a
    bctr, `lwz rM,k(r2)` (module TOC) ... `lwz rT,d(rM)` (table base) ...
    `lwzx`/`add`/`mtctr`, bounded by the nearest `cmplwi crN,rX,imm`.
    Returns (table, count) pairs whose entries all land in the text segment."""
    e = open(eboot, "rb").read()
    n = (TEXT_HI - TEXT_LO) // 4
    W = struct.unpack(">%dI" % n, e[0:n * 4])
    found = {}
    for i, x in enumerate(W):
        if x != 0x4E800420:          # bctr
            continue
        lo = max(0, i - 48)
        # nearest cmplwi before the bctr: opcode 10, L=0
        imm = None
        for j in range(i - 1, lo, -1):
            y = W[j]
            if (y >> 26) == 10 and ((y >> 21) & 1) == 0:
                imm = y & 0xFFFF
                break
        if imm is None:
            continue
        # lwz rT, d(rM) with rM itself loaded as lwz rM, k(r2) earlier
        bases = []
        for j in range(i - 1, lo, -1):
            y = W[j]
            if (y >> 26) != 32:
                continue
            rt, rm, d = (y >> 21) & 31, (y >> 16) & 31, y & 0xFFFF
            d = d - 0x10000 if d & 0x8000 else d
            for k in range(j - 1, max(0, j - 400), -1):
                z = W[k]
                if (z >> 26) == 32 and ((z >> 16) & 31) == 2 and ((z >> 21) & 31) == rm:
                    ko = z & 0xFFFF
                    ko = ko - 0x10000 if ko & 0x8000 else ko
                    try:
                        module = r32((TOC + ko) & 0xFFFFFFFF)
                        table = r32((module + d) & 0xFFFFFFFF)
                    except SystemExit:
                        break
                    bases.append(table)
                    break
        for table in bases:
            count = imm + 1
            try:
                offs = [r32(table + 4 * c) for c in range(count)]
            except SystemExit:
                continue
            tg = [(table + (o - (1 << 32) if o & 0x80000000 else o)) & 0xFFFFFFFF for o in offs]
            if all(TEXT_LO <= t < TEXT_HI and not (t & 3) for t in tg):
                found[table] = count
                break
    return sorted(found.items())


def main():
    eboot, funcs_path, tables_path, out_path = sys.argv[1:5]
    r32 = load_segments(eboot)
    funcs = json.load(open(funcs_path))
    targets = set()
    # Only tables whose cases span at least MIN_SPAN bytes: those are the ones
    # that run past where the lifter cuts a function. Small switches stay inside
    # their function, where the lifter already resolves every case as a label.
    MIN_SPAN = 0x1000
    used = 0
    for table, count in auto_tables(eboot, r32):
        tg = []
        for c in range(count):
            o = r32(table + 4 * c)
            o = o - (1 << 32) if o & 0x80000000 else o
            tg.append((table + o) & 0xFFFFFFFF)
        if max(tg) - min(tg) < MIN_SPAN:
            continue
        used += 1
        targets.update(tg)
    print(f"[augment] auto-detected jump tables used: {used} (span >= 0x{MIN_SPAN:X}), {len(targets)} target(s)")
    for line in open(tables_path):
        line = line.split("#", 1)[0].strip()
        if not line:
            continue
        toc_off, mod_off, count = (int(x, 16) for x in line.split())
        toc_off = toc_off - (1 << 32) if toc_off > 0x7FFFFFFF else toc_off   # "-7EB0" or "FFFF8150"
        mod_off = mod_off - (1 << 32) if mod_off > 0x7FFFFFFF else mod_off
        module = r32((TOC + toc_off) & 0xFFFFFFFF)
        table = r32((module + mod_off) & 0xFFFFFFFF)
        for i in range(count):
            off = r32(table + 4 * i)
            off = off - (1 << 32) if off & 0x80000000 else off
            targets.add((table + off) & 0xFFFFFFFF)
        print(f"[augment] table 0x{table:08X}: {count} entries")

    starts = {int(str(f["start"]), 0) for f in funcs}
    new = sorted(targets - starts)
    out = []
    for f in funcs:
        s, e = int(str(f["start"]), 0), int(str(f["end"]), 0)
        cuts = [t for t in new if s < t < e]
        if not cuts:
            out.append(f)
            continue
        bounds = [s, *cuts, e]
        for a, b in zip(bounds, bounds[1:]):
            g = dict(f)
            g["start"], g["end"] = hex(a), hex(b)
            if a != s:
                g.pop("opd", None)       # interior entries have no descriptor
            out.append(g)
    json.dump(out, open(out_path, "w"), indent=0)
    print(f"[augment] {len(new)} new function start(s); {len(funcs)} -> {len(out)} entries")


if __name__ == "__main__":
    main()
