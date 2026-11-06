#!/bin/zsh
# The one way to compile this port, for people and for agents working side by
# side. Ported from ../pes13-recomp/tools/build.sh: each lifted chunk is tens
# of MB of C++ and clang takes 3-4 GB on a chunk; uncontrolled builds
# kernel-panicked this 24 GB Mac twice on 2026-11-01.
#
#   tools/build.sh           build, one at a time, watching disk and memory
#   tools/build.sh --check   only say what would be rebuilt
#
#   BUILD_OWNER=name         shown to whoever finds the lock (default: $USER)
#   BUILD_JOBS=n             parallel compiles (default 2)
#   BUILD_ALLOW_FULL=1       allow rebuilding more than MAX_CHUNKS lifted chunks
#
# Exit codes: 0 built, 1 build failed, 2 refused (nothing was started),
# 99 stopped by the memory guard.
export PATH=/opt/homebrew/bin:$PATH
here=${0:A:h}; source "$here/env.sh"
LOCK=$AFRO_ROOT/cache/build.lock
MIN_DISK_GB=4 MIN_MEM_PCT=15 MAX_CHUNKS=6
JOBS=${BUILD_JOBS:-2} OWNER=${BUILD_OWNER:-$USER}
EXE=AfroSamuraiRecomp

refuse() { print -u2 "build: REFUSED: $*"; exit 2; }

mkdir -p "$AFRO_ROOT/cache"
[ -f "$AFRO_BUILD/build.ninja" ] || refuse "no $AFRO_BUILD/build.ninja; configure first (tools/rebuild.sh build)"

others=$(pgrep -x ninja | while read p; do ps -o pid=,etime=,command= -p $p; done)
[ -n "$others" ] && refuse "another ninja is running, wait for it:
$others"

free_gb=$(df -g "$AFRO_ROOT" | awk 'NR==2{print $4}')
[ "$free_gb" -lt $MIN_DISK_GB ] && refuse "only ${free_gb} GB free on disk (minimum $MIN_DISK_GB)"

if ! mkdir "$LOCK" 2>/dev/null; then
    holder=$(cat "$LOCK/pid" 2>/dev/null)
    if [ -n "$holder" ] && kill -0 "$holder" 2>/dev/null; then
        refuse "held by $(cat "$LOCK/owner" 2>/dev/null) (pid $holder)"
    fi
    rm -rf "$LOCK"; mkdir "$LOCK" || refuse "cannot take $LOCK"
fi
echo $$ > "$LOCK/pid"; echo "$OWNER" > "$LOCK/owner"
trap 'rm -rf "$LOCK"' EXIT

# ninja -n stops at CMake's glob re-check when the manifest is build.ninja, so
# regenerate first and count against a copy under another name.
cd "$AFRO_BUILD" || exit 2
ninja build.ninja > /dev/null || { print -u2 "build: regenerating build.ninja failed"; exit 1; }
tmp=dry_$$.ninja plan=$(mktemp)
cp build.ninja $tmp; ninja -f $tmp -n $EXE > "$plan" 2>&1; rm -f $tmp
steps=$(grep -c '^\[' "$plan"); chunks=$(grep -o 'ppu_recomp_[0-9]*\.cpp\.o' "$plan" | sort -u | wc -l | tr -d ' ')
rm -f $plan
echo "build: $steps steps, $chunks lifted chunks, ${free_gb} GB free on disk"
[ "$1" = "--check" ] && exit 0
[ "$chunks" -gt $MAX_CHUNKS ] && [ -z "$BUILD_ALLOW_FULL" ] && \
    refuse "$chunks lifted chunks would rebuild. A target-wide flag or a reconfigure does this; fix that, or set BUILD_ALLOW_FULL=1"

log=$AFRO_ROOT/cache/build-last.log
ninja -j$JOBS $EXE > "$log" 2>&1 &
pid=$!
low=100
while kill -0 $pid 2>/dev/null; do
    f=$(memory_pressure 2>/dev/null | awk -F': ' '/free percentage/{print int($2)}')
    [ -n "$f" ] && [ "$f" -lt "$low" ] && low=$f
    if [ -n "$f" ] && [ "$f" -lt $MIN_MEM_PCT ]; then
        print -u2 "build: GUARD: ${f}% memory free, stopping this build"
        kill $pid; wait $pid; exit 99
    fi
    sleep 5
done
wait $pid; rc=$?
echo "build: ninja rc=$rc, lowest free memory ${low}%, log $log"
[ $rc -eq 0 ] || { grep -E "error:|FAILED" "$log" | cut -c1-240 | head -20; exit 1; }
tail -1 "$log" | cut -c1-120
