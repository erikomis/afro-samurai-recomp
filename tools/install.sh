#!/bin/zsh
# Install the port from your own disc in one go, the way the launcher's
# "Install the game" button does:
#
#   tools/install.sh                         the ISO behind game_iso/
#   AFRO_MOUNT=/Volumes/<disc> tools/install.sh   a disc mounted elsewhere
#
#   check    the Command Line Tools and Homebrew's build tools are there,
#            and the disk has room (AFRO_NEED_GB, default 6)
#   setup    tools/setup.sh: the toolkit (../ps3recomp-afro at the pinned
#            commit + patch), the venv and the disc SELF keys
#   stage, decrypt, analyse, lift, spu, build: tools/rebuild.sh's steps
#
# Run it again after a failure or a quit and it continues: stage and decrypt
# skip a file that is there (so the disc is needed only until the EBOOT is
# decrypted), analyse, lift and spu are skipped when they finished before with
# the same inputs (the EBOOT, the toolkit pin and patch, the port's scripts;
# stamps in cache/install/), and the build is incremental. The first build
# compiles every lifted chunk (BUILD_ALLOW_FULL=1); tools/build.sh's memory,
# disk and one-build-at-a-time guards still apply.
#
# Environment: as tools/rebuild.sh (AFRO_MOUNT, JOBS, BUILD_JOBS, PY).
set -e
setopt pipefail
here=${0:A:h}
say() { print -r -- "==> $*"; }
fail() { print -u2 -r -- ""; print -u2 -r -- "ERROR: $*"; exit 1; }
source "$here/env.sh"
cd "$AFRO_ROOT"

# One install at a time in this checkout.
ST=$AFRO_ROOT/cache/install
mkdir -p "$ST"
if [ -f "$ST/pid" ] && kill -0 "$(cat "$ST/pid")" 2>/dev/null; then
    fail "another install is already running in $AFRO_ROOT (process $(cat "$ST/pid")). Wait for it, or stop it, and run this again."
fi
print -r -- $$ >"$ST/pid"
trap 'rm -f "$ST/pid"' EXIT

# What a full install leaves on the disk: the lifted sources 1.4 GB, the build
# 0.4 GB, the toolkit and the venv about 0.3 GB; tools/build.sh then wants
# 4 GB free while it compiles.
NEED_GB=${AFRO_NEED_GB:-6}
if [ ! -x "$AFRO_BUILD/AfroSamuraiRecomp" ]; then
    free_gb=$(( $(df -k "$AFRO_ROOT" | awk 'NR==2 {print $4}') / 1024 / 1024 ))
    (( free_gb >= NEED_GB )) ||
        fail "not enough free disk space: $free_gb GB free, about $NEED_GB GB are needed. Free some space and run this again."
fi
xcode-select -p >/dev/null 2>&1 || fail "Apple's Command Line Tools are missing: run xcode-select --install, then this again."
missing=()
for f in cmake ninja sdl2 glslang spirv-cross spirv-tools ffmpeg python@3.12; do
    brew list --versions $f >/dev/null 2>&1 || missing+=($f)
done
(( ${#missing} == 0 )) ||
    fail "Homebrew's build tools are missing: brew install ${missing[*]} (Homebrew itself: https://brew.sh), then run this again."

say "setup (the toolkit, Python and the disc keys)"
"$here/setup.sh"
export PY=$AFRO_VENV/bin/python

# stage and decrypt: only while the decrypted EBOOT is missing.
if [ ! -f "$AFRO_EBOOT" ]; then
    if [ ! -f "$AFRO_EBOOT_BIN" ] && [ ! -d "$AFRO_MOUNT/PS3_GAME" ] && ! ls "$AFRO_ISO_DIR"/*.iso >/dev/null 2>&1; then
        fail "the disc is not mounted: mount your Afro Samurai (BLUS30264) disc image, or put it at game_iso/, and run this again."
    fi
    "$here/rebuild.sh" stage decrypt
else
    say "stage (the EBOOT is there, skipped)"
    say "decrypt (the EBOOT is decrypted, skipped)"
fi

# analyse, lift, spu: skipped when they finished with the same inputs. Each
# step's key chains the one before it, so a new analysis re-lifts too.
key=
step() {   # <step> <input file...>
    local name=$1; shift
    key=$({ print -r -- "$key"; cat -- "$@"; } | shasum -a 256 | cut -d' ' -f1)
    if [ "$(cat "$ST/$name" 2>/dev/null)" = "$key" ]; then
        say "$name (done before with the same inputs, skipped)"
        return 0
    fi
    rm -f "$ST/$name"
    "$here/rebuild.sh" "$name"
    print -r -- "$key" >"$ST/$name"
}
step analyse "$AFRO_EBOOT" "$here/setup_toolkit.sh" "$AFRO_ROOT"/patches/*.patch(N)
step lift "$here/rebuild.sh" "$here/augment_functions.py" "$here/jumptables.txt"
step spu "$here/lift_spu.sh" "$here/lift_jobs.sh" "$here/lift_fmod_plugins.sh" "$here/extract_jobs.py"

BUILD_ALLOW_FULL=1 "$here/rebuild.sh" build
say "done"
