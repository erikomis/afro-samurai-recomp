#!/bin/zsh
# Headless test run with a known PID, a time limit, a log and frame dumps.
#
#   tools/probe.sh <label> <seconds> [VAR=value ...]
#
# Writes cache/probe/<label>.log and cache/probe/<label>.ppm (a frame every
# PS3RECOMP_METAL_FRAME_DUMP_EVERY presents, default 120). Stops only the
# process it started. Input: PAD_SCRIPT="<sec>:<mask>,..." (wall-clock seconds,
# or flips with PAD_CLOCK=flips) passes through: START 0x0008, UP 0x0010,
# DOWN 0x0040, CROSS 0x4000, CIRCLE 0x2000, SQUARE 0x8000, TRIANGLE 0x1000.
set -u
here=${0:A:h}; source "$here/env.sh"
label=${1:?label}; secs=${2:?seconds}; shift 2
out=$AFRO_ROOT/cache/probe; mkdir -p "$out"
log=$out/$label.log
caffeinate -i "$here/run.sh" PS3RECOMP_METAL_HEADLESS=1 \
    PS3RECOMP_METAL_FRAME_DUMP="$out/$label.ppm" "$@" > "$log" 2>&1 &
pid=$!
echo "probe: pid $pid, log $log"
t=0
# PROBE_SNAP=<s>: keep a copy of the latest dump every <s> seconds as
# <label>_t<sec>.ppm, so a run shows a sequence rather than its last frame.
snap=${PROBE_SNAP:-0}
while kill -0 $pid 2>/dev/null && (( t < secs )); do
    sleep 1; (( t++ ))
    if (( snap > 0 && t % snap == 0 )) && [ -f "$out/$label.ppm" ]; then
        cp "$out/$label.ppm" "$out/${label}_t$(printf %03d $t).ppm"
    fi
done
if kill -0 $pid 2>/dev/null; then
    # caffeinate's child is the game; stop both, children first.
    pkill -TERM -P $pid 2>/dev/null; sleep 1; kill -TERM $pid 2>/dev/null
    sleep 1; pkill -KILL -P $pid 2>/dev/null; kill -KILL $pid 2>/dev/null
    echo "probe: stopped after ${secs}s"
else
    wait $pid; echo "probe: exited rc=$? after ${t}s"
fi
echo "probe: $(wc -l < "$log") log lines, $(ls "$out"/$label*.ppm 2>/dev/null | wc -l | tr -d ' ') frame dumps"
