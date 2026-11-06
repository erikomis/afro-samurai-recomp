#!/bin/zsh
# Lift the SPU images embedded in the Afro Samurai EBOOT and register them for
# the SPURS/workload dispatch (port/spu/spu_workloads.c).
#
#   tools/lift_spu.sh
#
# The images are the SPU ELFs extract_spu_images.py writes to
# analysis/BLUS30264/spu/ (rebuild.sh spu). Their file bytes are what the
# runtime fingerprints at dispatch time, so the content fingerprints match.
#
#   spu_0000  EDGE ZLIB inflate task (SPURS task; package decompression)
#   spu_0001  FMOD Ex SPURS mixer (fmodex_spurs.elf)
#   0002-0019 FMOD codec and DSP plugins (adpcm, mpeg, chorus, echo, reverb...)
#
# Images whose executable segment carries a data tail get --code-end (end of
# the last real function), set in SPU_CODE_END below when one is found.
#
# Output: port/spu/<image>/spu_recomp.{c,h} + port/spu/spu_workloads.c. Never
# commit these: they are derived from game code.
set -e
here=${0:A:h}
source "$here/env.sh"

SPU="$AFRO_ROOT/port/spu"
IMG="$AFRO_ANALYSIS/spu"
KIT="$PS3RECOMP_DIR/tools"

typeset -A SPU_CODE_END
# SPU_CODE_END[spu_00NN]=0x....

# Extra lifter options per image.
#   spu_0001  FMOD's mixer calls the DSP/codec plugins it overlays into local
#             store (interpreted: SPU_INTERP_UNLIFTED) with bisl, and a plugin
#             returns into the MIDDLE of a mixer function. With no lifted entry
#             there, the return resumed at the next entry down (0x17718 ->
#             0x17860), ran the wrong code into a garbage lr, and the mixer job
#             ended -- the first menu sound then froze the game on FMOD's flags.
#             --return-entries makes every call's return address an entry;
#             --merge-chunks keeps straight-line code in one function.
typeset -A SPU_EXTRA
SPU_EXTRA[spu_0001]="--return-entries --merge-chunks"

mkdir -p "$SPU"
for elf in "$IMG"/spu_*.elf; do
    stem=${elf:t:r}
    [[ -n "${ONLY_SPU:-}" && "${stem%%_at_*}" != "$ONLY_SPU" ]] && continue
    ce=${SPU_CODE_END[${stem%%_at_*}]}
    cmd=("$PY" "$KIT/spu_lifter.py" "$elf" --auto-functions "$elf"
         --symbol-prefix "${stem}_" -o "$SPU/$stem"
         --source-name spu_recomp.c --header-name spu_recomp.h)
    [[ -n "$ce" ]] && cmd+=(--code-end "$ce")
    extra=${SPU_EXTRA[${stem%%_at_*}]}
    [[ -n "$extra" ]] && cmd+=(${=extra})
    echo "[lift_spu] ${stem}${ce:+ (code-end $ce)}"
    "${cmd[@]}" > "$AFRO_ROOT/cache/lift_spu_${stem}.log" 2>&1 || {
        echo "  FAILED, see cache/lift_spu_${stem}.log" >&2; exit 1; }
done

"$PY" "$KIT/build_spu_workloads.py" --images "$IMG" \
    --lifted "$SPU" --out "$SPU/spu_workloads.c" --constructor --skip-lift
