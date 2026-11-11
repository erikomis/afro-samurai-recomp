#!/usr/bin/env python3
"""Fail when the launcher's version is not the release tag's.

    python3 launcher/scripts/check_version.py v0.3.0

The version is written in three places (src-tauri/tauri.conf.json, which the
app and the updater go by, src-tauri/Cargo.toml and package.json); all of
them must be the tag's X.Y.Z. The updater signature carries the bundle's
version and installed apps refuse a download whose signed version is not
the one latest.json announces, so a tag on a commit whose files still say
the old version would publish an update nobody can install. Standard
library only.
"""
import json, os, re, sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)


def versions(root=ROOT):
    with open(os.path.join(root, 'src-tauri', 'tauri.conf.json'), encoding='utf-8') as f:
        tauri = json.load(f)['version']
    with open(os.path.join(root, 'package.json'), encoding='utf-8') as f:
        npm = json.load(f)['version']
    with open(os.path.join(root, 'src-tauri', 'Cargo.toml'), encoding='utf-8') as f:
        m = re.search(r'^\[package\][^\[]*?^version\s*=\s*"([^"]+)"', f.read(), re.M | re.S)
    cargo = m.group(1) if m else None
    return {'src-tauri/tauri.conf.json': tauri, 'src-tauri/Cargo.toml': cargo, 'package.json': npm}


def main(argv):
    if len(argv) != 1 or not re.match(r'^v\d+\.\d+\.\d+$', argv[0]):
        print('usage: check_version.py vX.Y.Z', file=sys.stderr)
        return 2
    want = argv[0][1:]
    bad = {k: v for k, v in versions().items() if v != want}
    for k, v in bad.items():
        print('check_version: %s says %s, the tag %s' % (k, v, argv[0]), file=sys.stderr)
    if not bad:
        print('version %s everywhere' % want)
    return 1 if bad else 0


if __name__ == '__main__':
    sys.exit(main(sys.argv[1:]))
