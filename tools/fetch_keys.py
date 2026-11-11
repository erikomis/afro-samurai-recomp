#!/usr/bin/env python3
"""Fetch the disc SELF keys Afro Samurai needs from RPCS3's pinned public source.

    tools/fetch_keys.py                 # write the key file if it is not there yet
    tools/fetch_keys.py --force         # download and write it again
    tools/fetch_keys.py --source FILE   # a key_vault.cpp already on disk
    tools/fetch_keys.py --check         # validate the existing file, no download

No key is embedded here. The disc EBOOT (BLUS30264) is an APP SELF with key
revision 0x0001; it has no NPDRM layer, so no RAP or klicensee keys are
needed. key_vault.cpp is checked against a pinned SHA-256 before anything is
written, in the scetool key-file format tools/decrypt_self.py reads.

The key file goes to a per-user folder outside the repository, readable only
by you (mode 600):

    macOS  ~/Library/Application Support/afro-samurai-recomp/keys/self_keys.txt
    other  $XDG_DATA_HOME (~/.local/share)/afro-samurai-recomp/keys/self_keys.txt

--out or AFRO_KEYS put it elsewhere. The path alone is printed on stdout;
progress and errors go to stderr. No key is ever printed.

Only the Python standard library is used, so this runs before the toolkit's
Python packages are installed.
"""
import argparse
import configparser
import hashlib
import os
import re
import ssl
import sys
import tempfile
import urllib.error
import urllib.request

# The last RPCS3 commit that changed key_vault.cpp when this was written
# ("Use std::string_view in crypto code", 2026-06-16), and the SHA-256 of the
# file at that commit. A hash is not a key: it only says which file is right.
RPCS3_COMMIT = 'b8ca6d953f734900c23f2cfbe79a36e598171553'
KEY_VAULT_SHA256 = 'a2947ff6b681258c198552ec6344e16685e2df649018206daa3f41a0a1aadbd7'
KEY_VAULT_URL = ('https://raw.githubusercontent.com/RPCS3/rpcs3/%s/rpcs3/Crypto/key_vault.cpp'
                 % RPCS3_COMMIT)

NEEDED_REVISIONS = {'APP': (0x0001,)}


def default_path():
    if os.environ.get('AFRO_KEYS'):
        return os.environ['AFRO_KEYS']
    home = os.path.expanduser('~')
    if sys.platform == 'darwin':
        base = os.path.join(home, 'Library', 'Application Support')
    else:
        base = os.environ.get('XDG_DATA_HOME') or os.path.join(home, '.local', 'share')
    return os.path.join(base, 'afro-samurai-recomp', 'keys', 'self_keys.txt')


def log(msg):
    print(msg, file=sys.stderr)


class FetchError(Exception):
    pass


def download(url, timeout):
    req = urllib.request.Request(url, headers={'User-Agent': 'fetch_keys.py'})

    def get(ctx):
        with urllib.request.urlopen(req, timeout=timeout, context=ctx) as r:
            return r.read()
    try:
        try:
            return get(ssl.create_default_context())
        except urllib.error.URLError as e:
            if not isinstance(e.reason, ssl.SSLCertVerificationError):
                raise
            # A python.org Python on macOS has no CA certificates until its
            # "Install Certificates.command" is run. The file is checked
            # against its pinned SHA-256 below, so a connection that cannot
            # be verified still cannot hand us a different file.
            log('  (this Python cannot verify GitHub\'s certificate; the file is checked by its SHA-256 instead)')
            return get(ssl._create_unverified_context())
    except (urllib.error.URLError, OSError) as e:
        reason = getattr(e, 'reason', e)
        raise FetchError(
            'could not download the decryption keys from GitHub (%s).\n'
            'Check that this computer is connected to the internet and try again.\n'
            'They come from: %s' % (reason, url))


def parse_self_keys(text, kind):
    """[(revision, erk hex, riv hex)] from a KeyVault table, in file
    order (RPCS3 takes the first entry of a revision; so does decrypt_self.py)."""
    m = re.search(r'void\s+KeyVault::LoadSelf' + kind + r'Keys\s*\(\s*\)\s*\{(.*?)\n\}', text, re.S)
    if not m:
        raise FetchError('key_vault.cpp has no KeyVault::LoadSelf%sKeys' % kind)
    entry = re.compile(
        r'sk_' + kind + r'_arr\.emplace_back\(\s*0x[0-9A-Fa-f]+\s*,\s*0x[0-9A-Fa-f]+\s*,\s*0x([0-9A-Fa-f]+)\s*,'
        r'\s*KEY_' + kind + r'\s*,\s*"([0-9A-Fa-f]*)"\s*,\s*"([0-9A-Fa-f]*)"')
    keys, seen = [], set()
    for rev, erk, riv in entry.findall(m.group(1)):
        rev = int(rev, 16)
        if len(erk) != 64 or len(riv) != 32 or rev in seen:
            continue
        seen.add(rev)
        keys.append((rev, erk.upper(), riv.upper()))
    missing = [r for r in NEEDED_REVISIONS[kind] if r not in seen]
    if missing:
        raise FetchError('key_vault.cpp has no %s key for revision %s'
                         % (kind, ', '.join('0x%04X' % r for r in missing)))
    return keys


def key_file_text(keys, source):
    out = ['# SELF keys for APP programs, in scetool\'s key-file format,',
           '# for tools/decrypt_self.py. Written by tools/fetch_keys.py from',
           '# %s' % source,
           '# Do not share this file or put it in a repository.', '']
    for kind, entries in keys.items():
        for rev, erk, riv in entries:
            out += ['[appldr_%s_%04X]' % (kind.lower(), rev), 'type=SELF',
                    'revision=%04X' % rev, 'self_type=%s' % kind,
                    'erk=%s' % erk, 'riv=%s' % riv, '']
    return '\n'.join(out)


def write_private(path, text):
    d = os.path.dirname(path)
    if d:
        os.makedirs(d, mode=0o700, exist_ok=True)
    # Created 600 from the start, so the keys are never readable by others,
    # not even for a moment.
    fd, tmp = tempfile.mkstemp(prefix='.self_keys-', dir=d or '.')
    try:
        with os.fdopen(fd, 'w', encoding='ascii') as f:
            f.write(text)
        os.replace(tmp, path)
    finally:
        if os.path.exists(tmp):
            os.unlink(tmp)


def usable(path):
    """Every needed (program type, revision) has a 32-byte erk and 16-byte riv.
    Nothing is printed."""
    try:
        with open(path, encoding='ascii', errors='replace') as f:
            text = f.read()
        parser = configparser.ConfigParser(interpolation=None, inline_comment_prefixes=('#', ';'))
        parser.read_string('[named]\n' + text)
        seen = set()
        for section in parser.sections():
            b = parser[section]
            if b.get('type', b.get('category', '')).upper() == 'SELF':
                if len(bytes.fromhex(b.get('erk', ''))) == 32 and len(bytes.fromhex(b.get('riv', ''))) == 16:
                    seen.add((b.get('self_type', b.get('program_type', '')).upper(), int(b.get('revision', ''), 16)))
        return all((kind, rev) in seen for kind, revs in NEEDED_REVISIONS.items() for rev in revs)
    except (OSError, ValueError, configparser.Error):
        return False


def checked_source(path, url, digest, timeout):
    if path:
        with open(path, 'rb') as f:
            raw = f.read()
    else:
        raw = download(url, timeout)
    if hashlib.sha256(raw).hexdigest() != digest:
        raise FetchError('%s is not the expected pinned source (SHA-256 mismatch); nothing was written'
                         % (path or url))
    return raw.decode('utf-8')


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split('\n\n')[0])
    ap.add_argument('--out', default=default_path(), help='key file to write (default: %(default)s)')
    ap.add_argument('--force', action='store_true', help='download and write it even if it is there')
    ap.add_argument('--check', action='store_true', help='validate existing keys without downloading or writing')
    ap.add_argument('--source', help='a key_vault.cpp already on disk (still checked by its SHA-256)')
    ap.add_argument('--timeout', type=float, default=60, help='download timeout, seconds (default 60)')
    a = ap.parse_args(argv)
    path = os.path.abspath(os.path.expanduser(a.out))

    if a.check:
        if not usable(path):
            log('error: missing or incomplete key file: %s' % path)
            return 1
        print(path)
        return 0
    if not a.force and usable(path):
        print(path)
        return 0
    if not a.force and os.path.exists(path):
        log('error: existing key file is incomplete; preserved %s. Use --force to replace it.' % path)
        return 1
    try:
        cpp = checked_source(a.source, KEY_VAULT_URL, KEY_VAULT_SHA256, a.timeout)
        keys = {kind: parse_self_keys(cpp, kind) for kind in NEEDED_REVISIONS}
        write_private(path, key_file_text(keys, a.source or KEY_VAULT_URL))
    except (FetchError, OSError) as e:
        log('error: %s' % e)
        return 1
    log('  %d SELF key sets written to %s' % (sum(map(len, keys.values())), path))
    print(path)
    return 0


if __name__ == '__main__':
    sys.exit(main())
