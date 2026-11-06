#!/bin/zsh
# Run the recompiled Afro Samurai build. Extra arguments are passed through as
# VAR=value environment overrides, e.g.  tools/run.sh PS3_FSLOG=1
#
# A disc title: the ISO is mounted read-only (tools/mount.sh) and its
# PS3_GAME is linked into the VFS root at leaf level, so /dev_bdvd resolves into
# the disc. /dev_hdd0 and /dev_hdd1 are this repo's own writable trees under
# rpcs3/, never RPCS3's install, so test runs cannot touch the owner's saves.
#
# Headless test run (no window over the owner's work):
#   tools/run.sh PS3RECOMP_METAL_HEADLESS=1 PS3RECOMP_METAL_FRAME_DUMP=/path/f.ppm
set -e
source "${0:A:h}/env.sh"
if [ ! -x "$AFRO_BUILD/AfroSamuraiRecomp" ]; then
    echo "run: o executavel ainda nao existe ($AFRO_BUILD/AfroSamuraiRecomp)." >&2
    [ -d "$AFRO_ROOT/cache/build.lock" ] && echo "run: a build esta em andamento; espere ela terminar." >&2 \
        || echo "run: compile primeiro com tools/build.sh" >&2
    exit 1
fi
"${0:A:h}/mount.sh"

mkdir -p "$AFRO_VFS_ROOT"
ln -sfn "$AFRO_MOUNT/PS3_DISC.SFB" "$AFRO_VFS_ROOT/PS3_DISC.SFB"
ln -sfn "$AFRO_MOUNT/PS3_GAME" "$AFRO_VFS_ROOT/PS3_GAME"
export PS3_VFS_ROOT=$AFRO_VFS_ROOT

# /dev_hdd0: game data and saves the title creates.
export PS3_HDD0_ROOT=${PS3_HDD0_ROOT:-$AFRO_ROOT/rpcs3/dev_hdd0}
mkdir -p "$PS3_HDD0_ROOT/game" "$PS3_HDD0_ROOT/home/00000001/savedata"
# /dev_hdd1: the CACHE partition cellSysCacheMount mounts; must be writable.
export PS3_HDD1_ROOT=${PS3_HDD1_ROOT:-$AFRO_ROOT/rpcs3/dev_hdd1}
mkdir -p "$PS3_HDD1_ROOT/cache"
# cellSaveData appends /dev_hdd0/home/00000001/savedata to this root.
export PS3_GAMEDATA_ROOT=${PS3_GAMEDATA_ROOT:-$AFRO_ROOT/rpcs3}
export PS3_BOOT_PATH=${PS3_BOOT_PATH:-/dev_bdvd/PS3_GAME/USRDIR/EBOOT.BIN}
export RSX_MSL_CACHE=${RSX_MSL_CACHE:-$AFRO_ROOT/cache/msl}
mkdir -p "$RSX_MSL_CACHE"
# Firmware tree, read-only: FMOD may load its MP3 codec from /dev_flash.
[ -d "$HOME/Library/Application Support/rpcs3/dev_flash" ] && \
    export PS3_DEV_FLASH="${PS3_DEV_FLASH:-$HOME/Library/Application Support/rpcs3/dev_flash}"
export PS3_TITLE=${PS3_TITLE:-"Afro Samurai"}
# A redirected stderr means verbose by default (runtime/ps3_log.h); keep it quiet
# unless asked: PS3_VERBOSE=1.
export PS3_VERBOSE=${PS3_VERBOSE:-0}
export AUDIO_ELASTIC=${AUDIO_ELASTIC:-1}

cd "$AFRO_ROOT"
# AFRO_WRAP=<program>: run the game under it (a debugger wrapper script), which
# gets the executable and its argument.
exec env "$@" ${AFRO_WRAP:+"$AFRO_WRAP"} "$AFRO_BUILD/AfroSamuraiRecomp" "$AFRO_EBOOT"
