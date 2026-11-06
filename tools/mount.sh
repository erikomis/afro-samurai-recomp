#!/bin/zsh
# Mount the Afro Samurai disc image read-only at $AFRO_MOUNT if it is not
# mounted yet. The ISO is never written to.
set -e
source "${0:A:h}/env.sh"
if [ -d "$AFRO_MOUNT/PS3_GAME" ]; then exit 0; fi
iso=("$AFRO_ISO_DIR"/*.iso(N))
[ -f "$iso[1]" ] || { echo "ISO not found under $AFRO_ISO_DIR (check the game_iso symlink)" >&2; exit 1; }
hdiutil attach -readonly -nobrowse -noverify -mountpoint "$AFRO_MOUNT" "$iso[1]" >/dev/null
[ -d "$AFRO_MOUNT/PS3_GAME" ] || { echo "mount failed: $AFRO_MOUNT" >&2; exit 1; }
