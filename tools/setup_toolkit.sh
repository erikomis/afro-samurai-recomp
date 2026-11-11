#!/bin/zsh
# Put the toolkit this port builds against in place: upstream ps3recomp
# v0.19.0 (ae92407b) plus patches/ps3recomp-ae92407b-afro.patch.
#
#   tools/setup_toolkit.sh            clone into $PS3RECOMP_DIR (../ps3recomp-afro)
#   tools/setup_toolkit.sh --check    only say whether $PS3RECOMP_DIR matches the patch
#
#   PS3RECOMP_URL   where to clone from (default: the upstream GitHub repository)
#   PS3RECOMP_DIR   the checkout (tools/env.sh)
#
# An existing checkout is never overwritten: if it is already at ae92407b with
# exactly the patch applied it is left alone, otherwise the script stops and
# says why. tools/export_toolkit_patch.sh turns a checkout back into the patch.
set -eu
here=${0:A:h}; source "$here/env.sh"
BASE=ae92407b
PATCH=$AFRO_ROOT/patches/ps3recomp-ae92407b-afro.patch
URL=${PS3RECOMP_URL:-https://github.com/sp00nznet/ps3recomp.git}
fail() { print -u2 -r -- "setup_toolkit: $*"; exit 1; }
check=0
case ${1:-} in
    --check) check=1 ;;
    -h|--help) sed -n '2,14p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    '') ;;
    *) fail "unknown option: $1 (use --help)" ;;
esac
command -v git >/dev/null || fail "git is missing"
[[ -f $PATCH ]] || fail "patch missing: $PATCH"

matches() {   # the checkout is at BASE and its changes are exactly the patch
    local d=$1 head
    head=$(git -C "$d" rev-parse --verify -q HEAD) || return 1
    [[ $head == $(git -C "$d" rev-parse --verify -q "$BASE^{commit}") ]] || return 1
    cmp -s <("$here/export_toolkit_patch.sh" --stdout "$d") "$PATCH"
}

if [[ -e $PS3RECOMP_DIR ]]; then
    [[ -d $PS3RECOMP_DIR/.git || -f $PS3RECOMP_DIR/.git ]] || fail "$PS3RECOMP_DIR exists and is not a git checkout; preserved"
    if matches "$PS3RECOMP_DIR"; then
        print -r -- "setup_toolkit: $PS3RECOMP_DIR is at $BASE with the port patch applied."
        exit 0
    fi
    fail "$PS3RECOMP_DIR differs from $BASE + the port patch; preserved.
  Export its changes with tools/export_toolkit_patch.sh, or point PS3RECOMP_DIR elsewhere."
fi
(( ! check )) || fail "no toolkit checkout at $PS3RECOMP_DIR (run tools/setup_toolkit.sh)"

tmp=$PS3RECOMP_DIR.partial
[[ ! -e $tmp ]] || fail "$tmp is left from an earlier run; remove it first"
print -r -- "setup_toolkit: cloning $URL"
git clone --quiet "$URL" "$tmp"
git -C "$tmp" checkout --quiet -b afro/v0.19 "$BASE"
git -C "$tmp" apply --binary --whitespace=nowarn "$PATCH" || { print -u2 "setup_toolkit: the patch does not apply; left $tmp for a look"; exit 1; }
mv "$tmp" "$PS3RECOMP_DIR"
matches "$PS3RECOMP_DIR" || fail "$PS3RECOMP_DIR does not round-trip to the patch"
print -r -- "setup_toolkit: ready: $PS3RECOMP_DIR ($BASE + $(git -C "$PS3RECOMP_DIR" status --porcelain | wc -l | tr -d ' ') changed files)"
