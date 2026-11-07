#!/usr/bin/env python3
"""Extract known BLUS30264 v01.00 raw SPURS jobs from a decrypted EBOOT.

No game bytes are embedded here. Validate every slice before writing anything;
unknown executable revisions must be mapped explicitly, never guessed.
"""
import argparse
import hashlib
from pathlib import Path

# File offsets, not guest addresses. Verified against SPU_DUMP_MISS captures.
JOBS = (
    (0x83C080, 17552, '8B2B4776E3A079D8',
     '751102544f71096b47233ed3b2f959d2d279ff7c05594e25829f051558cadf07'),
    (0x840580, 112, '37DA975A7C220171',
     '2da9ca1ddfedf23b2027144920602835bc26c466d7112614273fa88decf03f22'),
)


def extract(eboot, output):
    data = eboot.read_bytes()
    if data[:4] != b'\x7fELF':
        raise ValueError('expected a decrypted ELF, not EBOOT.BIN')
    validated = []
    for offset, size, fingerprint, digest in JOBS:
        blob = data[offset:offset + size]
        if len(blob) != size or hashlib.sha256(blob).hexdigest() != digest:
            raise ValueError(f'job {fingerprint} at {offset:#x} does not match '
                             'BLUS30264 v01.00; no jobs written')
        # Match the toolkit basis exactly (it differs from standard FNV-1a).
        value = 1469598103934665603
        for byte in blob:
            value = ((value ^ byte) * 1099511628211) & 0xffffffffffffffff
        if f'{value:016X}' != fingerprint:
            raise ValueError(f'toolkit fingerprint mismatch for {fingerprint}')
        validated.append((f'spujob_{fingerprint}_{size}.bin', blob))
    output.mkdir(parents=True, exist_ok=True)
    for name, blob in validated:
        target = output / name
        if not target.exists() or target.read_bytes() != blob:
            target.write_bytes(blob)
        print(f'[extract_jobs] {target} ({len(blob)} bytes)')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('eboot', type=Path)
    parser.add_argument('-o', '--output', required=True, type=Path)
    args = parser.parse_args()
    try:
        extract(args.eboot, args.output)
    except (OSError, ValueError) as error:
        parser.exit(1, f'extract_jobs: {error}\n')


if __name__ == '__main__':
    main()
