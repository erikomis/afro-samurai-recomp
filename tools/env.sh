# Shared settings for the shell scripts in tools/. Sourced, not run (zsh).
#
#   AFRO_ROOT      this repository (default: the parent of tools/)
#   PS3RECOMP_DIR  the toolkit checkout (default: ../ps3recomp-afro: upstream
#                  v0.19.0 + this port's memory fixes, branch afro/v0.19; it
#                  is a worktree of ../ps3recomp-up-r8, not shared with the
#                  other ports; title choices stay env-gated in port/main.cpp)
#   AFRO_GAME      the game's files and the decrypted EBOOT (default:
#                  $AFRO_ROOT/game): EBOOT.elf, EBOOT.BIN, PARAM.SFO
#   AFRO_EBOOT     the decrypted PPU ELF the loader and lifter read
#   Disc data      the ISO stays in ~/Downloads behind $AFRO_ROOT/game_iso;
#                  run.sh mounts it read-only and links PS3_GAME into the VFS
#   AFRO_KEYS      the disc SELF key file (tools/fetch_keys.py writes it to a
#                  per-user folder outside the repository)
#   PY             the Python in this repository's venv (.venv, made by
#                  tools/setup.sh)
export AFRO_ROOT=${AFRO_ROOT:-${0:A:h:h}}
export PS3RECOMP_DIR=${PS3RECOMP_DIR:-$AFRO_ROOT/../ps3recomp-afro}
export AFRO_TITLE_ID=BLUS30264
export AFRO_GAME=${AFRO_GAME:-$AFRO_ROOT/game}
export AFRO_EBOOT=${AFRO_EBOOT:-$AFRO_GAME/EBOOT.elf}
export AFRO_EBOOT_BIN=${AFRO_EBOOT_BIN:-$AFRO_GAME/EBOOT.BIN}
export AFRO_ANALYSIS=${AFRO_ANALYSIS:-$AFRO_ROOT/analysis/$AFRO_TITLE_ID}
export AFRO_RECOMP=${AFRO_RECOMP:-$AFRO_ROOT/recompiled/$AFRO_TITLE_ID}
export AFRO_BUILD=${AFRO_BUILD:-$AFRO_ROOT/build/$AFRO_TITLE_ID}
export AFRO_ISO_DIR=${AFRO_ISO_DIR:-$AFRO_ROOT/game_iso}
export AFRO_MOUNT=${AFRO_MOUNT:-/Volumes/AFRO_SAMURAI}
# Disc SELF (key_rev 0x0001, APP). tools/fetch_keys.py writes it here.
if [[ $OSTYPE == darwin* ]]; then _afro_data=$HOME/Library/Application\ Support
else _afro_data=${XDG_DATA_HOME:-$HOME/.local/share}; fi
export AFRO_KEYS=${AFRO_KEYS:-$_afro_data/afro-samurai-recomp/keys/self_keys.txt}
unset _afro_data
# The VFS tree run.sh assembles: the disc's PS3_GAME by leaf symlinks, plus the
# save/cache trees the title creates under this repo, not under RPCS3's install.
export AFRO_VFS_ROOT=${AFRO_VFS_ROOT:-$AFRO_ROOT/rpcs3/dev_hdd0/disc/$AFRO_TITLE_ID}
export AFRO_VENV=${AFRO_VENV:-$AFRO_ROOT/.venv}
export PY=${PY:-$AFRO_VENV/bin/python}
export PATH=/opt/homebrew/bin:$PATH
