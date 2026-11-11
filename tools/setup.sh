#!/bin/zsh
# Everything the port needs before tools/rebuild.sh, and no build: the
# toolkit checkout, this repository's own Python venv and the disc SELF keys.
#
#   tools/setup.sh            set up what is missing
#   tools/setup.sh --check    only validate; no clone, install or download
#   tools/setup.sh --keys-source key_vault.cpp
#                             take RPCS3's pinned key_vault.cpp from disk, offline
#
# Environment (tools/env.sh): PS3RECOMP_DIR, AFRO_VENV (default .venv here),
# AFRO_KEYS (default: a per-user folder outside the repository, mode 600),
# PY, PYBIN (the Python that makes the venv; default python3.12).
# No key constant is in this script or anywhere in the repository.
set -eu
here=${0:A:h}; source "$here/env.sh"
fail() { print -u2 -r -- "setup: $*"; exit 1; }
check=0 key_source=()
while (( $# )); do
    case $1 in
        --check) check=1; shift ;;
        --keys-source) (( $# >= 2 )) || fail "$1 needs a path"; key_source=(--source "$2"); shift 2 ;;
        -h|--help) sed -n '2,14p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) fail "unknown option: $1 (use --help)" ;;
    esac
done
for dependency in git cmake ninja clang; do
    command -v $dependency >/dev/null || fail "install $dependency first and add it to PATH"
done

if (( check )); then
    "$here/setup_toolkit.sh" --check
    [[ -x $PY ]] || fail "Python environment missing: $PY (run tools/setup.sh)"
    "$PY" -c 'import Crypto.Cipher.AES, capstone, construct, tabulate, tqdm' || fail "Python packages missing (run tools/setup.sh)"
    "$PY" "$here/fetch_keys.py" --out "$AFRO_KEYS" --check >/dev/null || fail "key file missing or incomplete: $AFRO_KEYS"
else
    "$here/setup_toolkit.sh"
    if [[ ! -x $PY ]]; then
        [[ $PY == $AFRO_VENV/bin/python ]] || fail "custom PY does not exist: $PY"
        pybin=${PYBIN:-python3.12}
        command -v $pybin >/dev/null || fail "Python 3.12 missing (brew install python@3.12) or set PYBIN"
        $pybin -m venv "$AFRO_VENV"
    fi
    "$PY" -m pip install --quiet -r "$PS3RECOMP_DIR/requirements.txt"
    "$PY" "$here/fetch_keys.py" --out "$AFRO_KEYS" "${key_source[@]}" >/dev/null
fi
print -r -- "setup: ready (toolkit, Python packages, keys)."
print -r -- "  toolkit  $PS3RECOMP_DIR"
print -r -- "  python   $PY"
print -r -- "  keys     $AFRO_KEYS"
print -r -- "Next: put your disc's ISO at game_iso/, then tools/rebuild.sh."
