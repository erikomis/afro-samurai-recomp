#!/bin/zsh
# Turn a toolkit checkout back into patches/ps3recomp-ae92407b-afro.patch: every
# change from ae92407b in the working tree, new files included, as one binary
# git diff. The checkout's own index is not touched.
#
#   tools/export_toolkit_patch.sh                write the patch from $PS3RECOMP_DIR
#   tools/export_toolkit_patch.sh --stdout [DIR] print it instead (DIR: another checkout)
#
# Refuses when the checkout is not at ae92407b, so a patch never silently
# carries upstream commits. Run it after every toolkit change that goes in a
# commit of this repository.
set -eu
here=${0:A:h}; source "$here/env.sh"
BASE=ae92407b
PATCH=$AFRO_ROOT/patches/ps3recomp-ae92407b-afro.patch
fail() { print -u2 -r -- "export_toolkit_patch: $*"; exit 1; }
out=1
[[ ${1:-} == --stdout ]] && { out=0; shift; }
dir=${1:-$PS3RECOMP_DIR}
head=$(git -C "$dir" rev-parse --verify -q HEAD) || fail "$dir is not a git checkout"
[[ $head == $(git -C "$dir" rev-parse --verify -q "$BASE^{commit}") ]] \
    || fail "$dir is at ${head[1,8]}, not $BASE; this port's patch is against $BASE only"
idx=$(mktemp -t afro-toolkit-index)
trap 'rm -f "$idx"' EXIT
gitdir=$(git -C "$dir" rev-parse --git-dir)
[[ $gitdir == /* ]] || gitdir=$dir/$gitdir
cp "$gitdir/index" "$idx"
# A throwaway index with every non-ignored file added, so untracked new files
# are in the diff too.
GIT_INDEX_FILE=$idx git -C "$dir" add -A
diff() { GIT_INDEX_FILE=$idx git -C "$dir" diff --cached --binary --no-color "$BASE"; }
if (( out )); then
    diff > "$PATCH.partial"
    mv "$PATCH.partial" "$PATCH"
    print -r -- "export_toolkit_patch: $PATCH ($(grep -c '^diff --git' "$PATCH") files, $(wc -c < "$PATCH" | tr -d ' ') bytes)"
else
    diff
fi
