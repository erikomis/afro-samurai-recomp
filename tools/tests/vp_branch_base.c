/* Standalone regression probe; synthetic instructions, no game data.
 * cc -I "$PS3RECOMP_DIR/include" -I "$PS3RECOMP_DIR/libs/video" \
 *   tools/tests/vp_branch_base.c \
 *   "$PS3RECOMP_DIR/libs/video/rsx_vp_decompiler.c" -o /tmp/vp_branch_base
 */
#include "rsx_vp_decompiler.h"
#include <stdio.h>
#include <string.h>

static void word(u8 *p, u32 w)
{
    for (unsigned i = 0; i < 4; ++i) p[i] = (u8)(w >> (i * 8));
}

static int decompile(const u8 *code, u32 base, char *out, u32 cap)
{
    rsx_vp_set_branch_base(base);
    rsx_vp_set_branch_bits(0);
    return rsx_vp_decompile_compact_ex(code, 64, 0, 0xffff, out, cap);
}

int main(void)
{
    /* Four instructions, BRI to absolute slot 3 followed by NOPs.
     * At start 0 the destination is local 3; at start 1 it is local 2.
     * Both destinations are inside their respective programs.
     */
    u8 code[64] = {0};
    word(code, 7u << 10);          /* unconditional */
    word(code + 4, 9u << 27);     /* scalar BRI */
    word(code + 12, 3u << 29);    /* absolute destination 3 */
    word(code + 60, 1);           /* final instruction */
    char a[65536], b[65536], again[65536];
    if (decompile(code, 0, a, sizeof a) != 4 ||
        decompile(code, 1, b, sizeof b) != 4 ||
        decompile(code, 0, again, sizeof again) != 4) {
        fputs("FAIL: decompilation\n", stderr); return 1;
    }
    if (!strcmp(a, b) || strcmp(a, again)) {
        fputs("FAIL: expected base-dependent, repeatable output\n", stderr); return 1;
    }
    if (!strstr(a, "? 3 : 1;") || !strstr(b, "? 2 : 1;")) {
        fputs("FAIL: unexpected branch destinations\n", stderr); return 1;
    }
    puts("PASS: identical bytecode, base 0 -> target 3, base 1 -> target 2");
    puts("PASS: restoring base 0 reproduces the original HLSL");
    /* A straight-line control should not depend on the start slot. */
    word(code, 0); word(code + 4, 0); word(code + 12, 0);
    if (decompile(code, 0, a, sizeof a) != 4 ||
        decompile(code, 1, b, sizeof b) != 4 || strcmp(a, b)) {
        fputs("FAIL: straight-line control\n", stderr); return 1;
    }
    puts("PASS: straight-line control is unchanged across bases");
    return 0;
}
