/*
 * Afro Samurai (PS3, BLUS30264) -- ps3recomp runner. Based on the PES 2013
 * port's runner, itself the toolkit template.
 *
 * This is the runner. It boots a lifted title through the toolkit's PPU
 * scaffold: allocate the flat guest VM, load the PPU ELF named on the command
 * line, register the lifted function table and the HLE NID handlers, start a
 * frame clock, and dispatch the entry OPD.
 *
 * The scaffold does the work; nothing below reimplements it. ppu_load_elf,
 * ppu_recomp_register, ppu_hle_init, ppu_sysprx_register, ppu_fs_register,
 * lv2_init_syscalls and ppu_run all come from the toolkit, compiled into this
 * project from PS3RECOMP_DIR (see CMakeLists.txt). What is left here is the
 * part a port owns: which backend to present through, how the frame clock is
 * paced, and whatever diagnostics the title turns out to need.
 *
 * runtime/ppu/tests/boot_main.cpp in the toolkit is the same boot in its
 * fully-instrumented form -- crash filter, hang watchdog, guest-PC sampling
 * profiler. Read it when a boot goes wrong; copy from it what the title
 * needs. It is deliberately not what a fresh project starts with.
 *
 * The one part that IS duplicated from it is the frame clock below, because a
 * port is expected to change its pacing and a shared one would be the wrong
 * shape for that. It was copied faithfully, comments and all. If a pacing bug
 * is fixed in one of the two, look at the other.
 *
 * This builds on Windows, macOS and Linux. CI builds and runs it against the
 * boot smoke title on the last two, and compile-checks it against clang-cl on
 * the first (tools/check_ppu_scaffold.py).
 */

/* The lifter's generated header, which declares ppu_context and the function
 * table. It comes first: everything below is written against that struct. */
#include <algorithm>
#include <pthread.h>
#include <time.h>
#include "ppu_recomp.h"

/* The Win32 names -- Sleep, GetTickCount64, CreateThread, InterlockedIncrement,
 * VirtualAlloc, the scalar typedefs -- from one place. On Windows this is a
 * passthrough to <windows.h>; off Windows the toolkit's shim supplies the same
 * names over pthreads and mmap, so the call sites below are written once. */
#include "win32_compat.h"

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#ifndef _WIN32
#include <sys/mman.h>   /* mmap: the guest arena, see alloc_guest_vm */
#include <signal.h>
#include <unistd.h>     /* _exit */
#endif

static int quit_signal_take(void);   /* SIGINT/SIGTERM asked to quit (below) */

#if defined(__APPLE__)
#include <pthread.h>
#include <pthread/qos.h>
#include <CoreFoundation/CoreFoundation.h>   /* the main thread's run loop */
#endif

#ifdef _WIN32
/* timeBeginPeriod: <windows.h> arrives with WIN32_LEAN_AND_MEAN set, which
 * excludes the multimedia timer API, so it is asked for by name -- and after
 * windows.h, since timeapi.h uses UINT. No shim exists off Windows because
 * there is nothing to shim: the 15.6 ms default timer granularity this works
 * around is a Windows problem. */
#include <timeapi.h>
#endif

/* ---------------------------------------------------------------------------
 * What the scaffold gives this file
 * -----------------------------------------------------------------------*/
extern "C" {
uint32_t ppu_load_elf(const char* path);      /* ELF -> guest RAM, returns entry OPD */
void     ppu_recomp_register(void);           /* generated: lifted table -> address map */
void     ppu_hle_init(void);                  /* firmware import NID -> HLE handler */
void     ppu_sysprx_register(void);           /* boot-critical CRT (sys_initialize_tls, ...) */
void     ppu_fs_register(void);               /* cellFs over the real game directory */
void     lv2_init_syscalls(void);             /* the lv2 syscall table */

/* libs/system/cellGame.c: title id / category / version out of PARAM.SFO.
 * Without it cellGame answers cellGameBootCheck with the placeholder
 * 'BLES00000', and every /dev_hdd0/game/<title>_INSTALL path the title builds
 * from that id names a directory that cannot exist. */
void     cellGame_init_from_paramsfo(const char* sfo_path);
int      ppu_run(uint32_t entry_opd, uint32_t stack_top);

/* This project's own: load whatever real system PRX modules the title needs
 * into guest RAM and register their exports. stubs.cpp has the empty version
 * a game with no lifted PRX wants. */
void     ps3_load_prx_modules(void);

extern const char* ppu_vfs_root;              /* host dir the PS3 mount points map into */

void     ps3_host_quit(const char* why);      /* cellSysutil.c: REQUEST_EXITGAME, then exit 0 */

/* The status the guest handed to sys_process_exit. A title that exits that way
 * never returns through ppu_run, so this is only read on the path where the
 * entry function unwound instead. */
extern int     g_sys_process_exit_called;
extern int32_t g_sys_process_exit_code;

/* Host-provided symbols the runtime and the HLE libraries link against. */
uint8_t* vm_base = nullptr;
extern uint32_t ppu_vm_size;                  /* ppu_loader.cpp: the OOB guard */

/* Guest-callback dispatch. g_ps3_guest_caller is the hook the HLE runtime
 * calls back into recompiled code through -- cellSysutil events and the GCM
 * vblank/flip handlers. ppu_guest_call does the OPD -> dispatch. */
typedef void (*ps3_guest_caller_fn)(uint32_t, uint64_t, uint64_t, uint64_t, uint64_t,
                                    uint64_t, uint64_t, uint64_t, uint64_t);
extern ps3_guest_caller_fn g_ps3_guest_caller;
uint64_t ppu_guest_call(uint32_t, uint64_t, uint64_t, uint64_t, uint64_t,
                        uint64_t, uint64_t, uint64_t, uint64_t);

/* The GCM half of the frame clock. */
void     cellGcmTickVBlank(void);
void     cellGcmTickFlip(void);
int      cellGcm_take_flip_pending(void);
void     cellGcm_rsx_process_fifo(void);      /* drain get -> put */
unsigned cellGcm_flip_request_count(void);
}

/* ---------------------------------------------------------------------------
 * RSX present backend
 * -----------------------------------------------------------------------*/
/* D3D12 on Windows, Metal on Apple, and the null backend's headless software
 * path anywhere else -- the same selection runtime/host/host_posix.c and the
 * toolkit's boot harness make. All three expose the same three entry points,
 * so the frame clock below is backend-agnostic. */
#if defined(_WIN32)
extern "C" int  rsx_d3d12_backend_init(uint32_t w, uint32_t h, const char* title);
extern "C" void rsx_d3d12_backend_present(void);
extern "C" int  rsx_d3d12_backend_pump_messages(void);
#  define rsx_backend_init    rsx_d3d12_backend_init
#  define rsx_backend_present rsx_d3d12_backend_present
#  define rsx_backend_pump    rsx_d3d12_backend_pump_messages
#  define RSX_BACKEND_NAME    "D3D12"
#elif defined(__APPLE__)
extern "C" int  rsx_metal_backend_init(uint32_t w, uint32_t h, const char* title);
extern "C" void rsx_metal_backend_present(void);
extern "C" int  rsx_metal_backend_pump_messages(void);
#  define rsx_backend_init    rsx_metal_backend_init
#  define rsx_backend_present rsx_metal_backend_present
#  define rsx_backend_pump    rsx_metal_backend_pump_messages
#  define RSX_BACKEND_NAME    "Metal"
#else
extern "C" int  rsx_null_backend_init(uint32_t w, uint32_t h, const char* title);
extern "C" void rsx_null_backend_present(void);
extern "C" int  rsx_null_backend_pump_messages(void);
#  define rsx_backend_init    rsx_null_backend_init
#  define rsx_backend_present rsx_null_backend_present
#  define rsx_backend_pump    rsx_null_backend_pump_messages
#  define RSX_BACKEND_NAME    "null (headless software)"
#endif

/* ---------------------------------------------------------------------------
 * Configuration
 * -----------------------------------------------------------------------*/
/* The flat VM must span every region the PS3 memory map uses, not just the
 * game image: sys_ppu_thread_create puts thread stacks at 0xD0000000, so a
 * smaller arena makes every spawned thread's stack access out of bounds. */
#define VM_SIZE       0x100010000ull   /* the full 32-bit guest space + a 64K guard */
#define STACK_TOP     0x0FF00000u      /* main-thread stack, below the 0x10000000 segment */
#define WINDOW_WIDTH  1280
#define WINDOW_HEIGHT 720

/* ---------------------------------------------------------------------------
 * Frame clock
 * -----------------------------------------------------------------------*/
/* On real hardware the RSX raises a vblank interrupt about 60 times a second
 * and that is what drives the game's frame loop. With no RSX it is synthesized
 * here: a host thread calls cellGcmTickVBlank/TickFlip, which invoke the
 * handlers the guest registered, and drains the GCM FIFO. Without this the
 * title initialises, registers its handlers and then waits forever.
 *
 * The ticks are driven off real elapsed time rather than off how long present()
 * took. A hidden or occluded window makes present block hard, and pacing the
 * ticks behind it paces the whole game behind it. */
static volatile LONG g_frames_presented = 0;

/* Frames handed to the backend at a guest FLIP boundary -- one per frame the
 * guest actually finished. The presents made before the guest's first flip, so
 * a fresh window is not left blank through a long boot, carry no guest frame
 * and are deliberately not counted. */
extern "C" unsigned ppu_boot_frames_presented(void)
{
    return (unsigned)g_frames_presented;
}

/* Monotonic microseconds. GetTickCount64 has only 1 ms steps, too coarse to pace a
 * 16.683 ms vblank or to see a frame that came 3 ms late. */
static unsigned long long mono_us(void)
{
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (unsigned long long)ts.tv_sec * 1000000ull + (unsigned long long)ts.tv_nsec / 1000u;
}

/* RSX_PACE_LOG=<seconds>: frame pacing, which an average frame rate hides. Each line
 * has the time between presents (median, 95th and 99th percentile, worst), how many
 * were late against 60 Hz (> 20 ms) and against 30 Hz (> 37 ms), and the worst time
 * the present call itself blocked this thread, which also drains the FIFO. */
static void pace_note(unsigned long long t0, unsigned long long t1)
{
    enum { CAP = 4096 };
    static long secs = -1;
    static unsigned iv[CAP];
    static unsigned n, worst_present;
    static unsigned long long last, window_start;
    if (secs < 0) { const char* e = getenv("RSX_PACE_LOG"); secs = e ? atol(e) : 0; }
    if (!secs) return;
    if (last && n < CAP) iv[n++] = (unsigned)(t1 - last);
    last = t1;
    if (t1 - t0 > worst_present) worst_present = (unsigned)(t1 - t0);
    if (!window_start) window_start = t1;
    if (t1 - window_start < (unsigned long long)secs * 1000000ull || !n) return;
    std::sort(iv, iv + n);
    unsigned late60 = 0, late30 = 0;
    for (unsigned i = 0; i < n; i++) { late60 += iv[i] > 20000u; late30 += iv[i] > 37000u; }
    fprintf(stderr, "[pace] %.1f fps | p50 %.1f p95 %.1f p99 %.1f max %.1f ms | >20ms %u >37ms %u | present max %.1f ms\n",
            n * 1000000.0 / (double)(t1 - window_start), iv[n / 2] / 1000.0, iv[n * 95 / 100] / 1000.0,
            iv[n * 99 / 100] / 1000.0, iv[n - 1] / 1000.0, late60, late30, worst_present / 1000.0);
    n = 0; worst_present = 0; window_start = t1;
}

static void present_guest_frame(void)
{
    const unsigned long long t0 = mono_us();
    rsx_backend_present();
    pace_note(t0, mono_us());
    /* This thread increments and guest threads read, so interlocked rather
     * than a volatile ++, which on arm64 is neither atomic nor a fence. */
    InterlockedIncrement(&g_frames_presented);
}

/* The frame clock sleeps on this between passes; an SPU storing the RSX `put` rings
 * it (g_gcm_put_doorbell) so the drain runs within microseconds of the move instead
 * of up to a pass later. Everything still runs on this thread. */
/* Weak: the r8 toolkit rings it from SPU stores of `put`; upstream v0.19 has
 * no doorbell, and then the clock just keeps its fixed cadence. */
extern "C" { __attribute__((weak)) void (*g_gcm_put_doorbell)(void) = nullptr; }
static pthread_mutex_t s_db_mtx = PTHREAD_MUTEX_INITIALIZER;
static pthread_cond_t  s_db_cv  = PTHREAD_COND_INITIALIZER;
static int s_db_rung = 0;
static void put_doorbell(void)
{
    pthread_mutex_lock(&s_db_mtx);
    s_db_rung = 1;
    pthread_cond_signal(&s_db_cv);
    pthread_mutex_unlock(&s_db_mtx);
}
static void frame_clock_wait(int us)
{
    pthread_mutex_lock(&s_db_mtx);
    if (!s_db_rung) {
        struct timespec ts;
        clock_gettime(CLOCK_REALTIME, &ts);
        ts.tv_nsec += (long)us * 1000;
        ts.tv_sec += ts.tv_nsec / 1000000000L;
        ts.tv_nsec %= 1000000000L;
        pthread_cond_timedwait(&s_db_cv, &s_db_mtx, &ts);
    }
    s_db_rung = 0;
    pthread_mutex_unlock(&s_db_mtx);
}

static DWORD WINAPI frame_clock(LPVOID)
{
    const char* title = getenv("PS3_TITLE");
    if (!title || !*title) title = "ps3recomp";

    int rsx_ok = (rsx_backend_init(WINDOW_WIDTH, WINDOW_HEIGHT, title) == 0);
    fprintf(stderr, "[rsx] %s backend init %s\n", RSX_BACKEND_NAME,
            rsx_ok ? "OK -- window open" : "FAILED");

    /* PS3_FRAMECLOCK_US: the sleep between passes. The title fences every render pass
     * on a label the drain writes, and it moves `put` from the PPU too, which rings no
     * doorbell, so this is added to each of those waits. A pass with nothing to do is
     * cheap. */
    int clock_us = 1000;
    { const char* e = getenv("PS3_FRAMECLOCK_US"); if (e && atoi(e) > 0) clock_us = atoi(e); }
    /* PS3_VBLANK_US: 59.94 Hz, as the console and a 60 Hz display. The whole 16 ms this
     * used to be is 62.5 Hz, which runs the title 4% fast and drifts against the display:
     * a repeated or dropped frame about every half second. */
    unsigned long long vblank_us = 16683;
    { const char* e = getenv("PS3_VBLANK_US"); if (e && atoi(e) > 0) vblank_us = (unsigned)atoi(e); }
    unsigned long long next_tick = mono_us(), last_pump = 0;

    for (;;) {
        frame_clock_wait(clock_us);
        const unsigned long long now = mono_us();
        if (quit_signal_take()) ps3_host_quit("SIGINT/SIGTERM");

        int fired = 0;
        while ((long long)(now - next_tick) >= 0 && fired < 240) {
            cellGcmTickVBlank();
            cellGcmTickFlip();
            /* Present a pending flip BEFORE draining any further. The flip
             * fires at a get==put frame boundary on the guest thread, so the
             * batch held right now is exactly the completed frame; presenting
             * after the drain races the guest's next-frame writes and shows a
             * mixed one. */
            if (rsx_ok && cellGcm_take_flip_pending())
                present_guest_frame();
            /* Drain the FIFO every tick. This is what writes the RSX sync-fence
             * labels the game's per-frame logic blocks on, so it has to keep
             * advancing at 60 Hz even while present() throttles. */
            if (rsx_ok) cellGcm_rsx_process_fifo();
            next_tick += vblank_us;
            fired++;
        }
        if (fired >= 240) next_tick = now;   /* fell too far behind -- resync */

        /* Drain at the outer cadence too, not only on the tick: titles fence every
         * render pass on a label the drain writes, and one wave of those at 16 ms
         * apiece paces the guest into single-figure frame rates. The real RSX writes
         * them in microseconds. */
        if (rsx_ok) {
            if (cellGcm_take_flip_pending())
                present_guest_frame();
            cellGcm_rsx_process_fifo();

            /* Window events at most every 4 ms: a windowed pump is a dispatch_sync to
             * the main thread, and this loop runs far more often than that. */
            const int pump_due = now - last_pump >= 4000;
            if (pump_due) {
                last_pump = now;
                if (rsx_backend_pump() != 0) {
                    rsx_ok = 0;              /* window closed */
                    /* Quit the way the console's Quit Game does (and Cmd+Q
                     * already did): REQUEST_EXITGAME to the title's sysutil
                     * callbacks, then exit 0 after PS3_QUIT_GRACE_MS, never
                     * while a save is being written. Without it the guest
                     * kept running with no window, the process left behind
                     * until killed. */
                    ps3_host_quit("window closed");
                    continue;
                }
            }
            /* Present a flip the drain just reached. Before the first flip present
             * at the pump's cadence, so the window is not blank during boot.
             *
             * Also present when the flip request count moved (the v0.19 template
             * does the same): upstream's call-time cellGcmSetPrepareFlip counts
             * the flip without raising the pending flag, so with only the flag
             * this title flipped 600 times a second and nothing reached the
             * screen. At most one present per vblank, so the window paces. */
            static unsigned last_fc = 0;
            static unsigned long long last_fc_present = 0;
            const unsigned fc = cellGcm_flip_request_count();
            if (cellGcm_take_flip_pending()) {
                present_guest_frame();
                last_fc = fc; last_fc_present = now;
            } else if (fc != last_fc && now - last_fc_present >= vblank_us) {
                present_guest_frame();
                last_fc = fc; last_fc_present = now;
            } else if (pump_due && fc == 0)
                rsx_backend_present();
        }
    }
    return 0;
}

/* ---------------------------------------------------------------------------
 * VFS root
 * -----------------------------------------------------------------------*/
/* Derive the directory holding PS3_GAME from the EBOOT path
 * <root>/PS3_GAME/USRDIR/EBOOT.elf -> <root>. $PS3_VFS_ROOT overrides. */
static char s_vfs_root[1024];

static void derive_vfs_root(const char* eboot)
{
    const char* env = getenv("PS3_VFS_ROOT");
    if (env && *env) { ppu_vfs_root = env; return; }

    strncpy(s_vfs_root, eboot, sizeof s_vfs_root - 1);
    for (char* p = s_vfs_root; *p; p++) if (*p == '\\') *p = '/';
    /* strip EBOOT.elf, USRDIR and PS3_GAME */
    for (int i = 0; i < 3; i++) { char* s = strrchr(s_vfs_root, '/'); if (s) *s = 0; }
    if (!s_vfs_root[0]) strcpy(s_vfs_root, ".");
    ppu_vfs_root = s_vfs_root;
}

/* ---------------------------------------------------------------------------
 * Guest VM
 * -----------------------------------------------------------------------*/
#ifdef _WIN32
/* Demand-paging for the flat VM: reserve the whole 4 GB guest space, which
 * costs no commit, and commit each 64 KB page on first access. Every 32-bit
 * guest offset is then valid, so a garbage guest pointer reads zero instead of
 * killing the process. Faults outside the arena fall through untouched. */
static LONG WINAPI vm_commit_veh(EXCEPTION_POINTERS* ep)
{
    if (ep->ExceptionRecord->ExceptionCode == EXCEPTION_ACCESS_VIOLATION) {
        ULONG_PTR fault = ep->ExceptionRecord->ExceptionInformation[1];
        uintptr_t base  = (uintptr_t)vm_base;
        if (vm_base && fault >= base && fault < base + VM_SIZE) {
            void* page = (void*)(fault & ~(uintptr_t)0xFFFF);
            if (VirtualAlloc(page, 0x10000, MEM_COMMIT, PAGE_READWRITE))
                return EXCEPTION_CONTINUE_EXECUTION;
        }
    }
    return EXCEPTION_CONTINUE_SEARCH;
}
#endif

static bool alloc_guest_vm(void)
{
#ifdef _WIN32
    AddVectoredExceptionHandler(1, vm_commit_veh);
    vm_base = (uint8_t*)VirtualAlloc(NULL, VM_SIZE, MEM_RESERVE, PAGE_READWRITE);
    ppu_vm_size = 0;              /* the whole space is backed; no OOB guard needed */
#else
    /* No vectored exception handlers off Windows, so the arena is mapped up
     * front and lazily backed by the OS: only pages the title touches cost
     * anything. It stops below the 0xE0000000 mark, so ppu_vm_size arms the
     * loader's bounds check for what is left.
     *
     * mmap, not calloc: vm_commit() hands vm_base + addr to mprotect, which
     * takes page-aligned addresses only. glibc serves an allocation this big
     * from its own mmap chunk and returns a pointer 16 bytes past the page
     * start, so every commit failed with EINVAL and sys_memory_allocate
     * answered ENOMEM to every call. mmap's base is page-aligned and zeroed. */
    void* p = mmap(NULL, 0xE0000000u, PROT_READ | PROT_WRITE,
                   MAP_PRIVATE | MAP_ANONYMOUS | MAP_NORESERVE, -1, 0);
    vm_base = (p == MAP_FAILED) ? nullptr : (uint8_t*)p;
    ppu_vm_size = 0xE0000000u;
#endif
    return vm_base != nullptr;
}

/* AFRO_TEXT_RO=1 (diagnostic): make the guest's text segment read-only once
 * the loader and HLE setup are done. Lifted code never reads instructions,
 * but switch tables live inline in .text, and some of them read back zero
 * mid-game ("unresolved indirect call" to a table base) while PPU_WWATCH sees
 * no guest store. A host write (HLE memcpy, SPU DMA, file read) then faults
 * here with a host backtrace and the guest caller. */
#if !defined(_WIN32)
#include <execinfo.h>
#include <signal.h>
extern "C" void ppu_guest_caller(char* out, size_t n);
static uintptr_t s_ro_lo, s_ro_hi;
static struct sigaction s_ro_prev_bus, s_ro_prev_segv;
static void text_ro_fault(int sig, siginfo_t* si, void* uc)
{
    uintptr_t a = (uintptr_t)si->si_addr;
    if (a >= s_ro_lo && a < s_ro_hi) {
        char who[128]; ppu_guest_caller(who, sizeof who);
        fprintf(stderr, "\n[text-ro] WRITE to guest 0x%08X (text) guest caller: %s\n",
                (unsigned)(a - (uintptr_t)vm_base), who);
        void* bt[48]; int n = backtrace(bt, 48);
        backtrace_symbols_fd(bt, n, 2);
        fflush(stderr);
        if (getenv("AFRO_TEXT_RO_CONT")) {   /* report each writer once, keep going */
            mprotect((void*)s_ro_lo, s_ro_hi - s_ro_lo, PROT_READ | PROT_WRITE);
            return;
        }
        abort();
    }
    const struct sigaction& p = sig == SIGBUS ? s_ro_prev_bus : s_ro_prev_segv;
    if ((p.sa_flags & SA_SIGINFO) && p.sa_sigaction) { p.sa_sigaction(sig, si, uc); return; }
    signal(sig, SIG_DFL);
}
/* Ctrl+C in the terminal, or a plain kill: the first SIGINT/SIGTERM asks the
 * game to quit as Quit Game does (ps3_host_quit, from the vblank loop: a
 * signal handler may not print or start threads); a second one ends the
 * process at once. Before this, either signal killed it mid-frame, a save
 * being written included. */
static volatile sig_atomic_t s_quit_sig;
static void quit_signal(int sig)
{
    if (s_quit_sig) _exit(128 + sig);
    s_quit_sig = sig;
}
static int quit_signal_take(void)
{
    static int taken;
    if (!s_quit_sig || taken) return 0;
    taken = 1;
    return 1;
}
static void quit_signal_arm(void)
{
    struct sigaction sa; memset(&sa, 0, sizeof sa);
    sa.sa_handler = quit_signal; sigemptyset(&sa.sa_mask);
    sigaction(SIGINT, &sa, nullptr);
    sigaction(SIGTERM, &sa, nullptr);
}
static void text_ro_arm(void)
{
    const char* e = getenv("AFRO_TEXT_RO");
    if (!e || *e != '1') return;
    s_ro_lo = (uintptr_t)vm_base + 0x10000u;
    s_ro_hi = ((uintptr_t)vm_base + 0x10000u + 0x88DBC8u) & ~(uintptr_t)0x3FFF;
    struct sigaction sa; memset(&sa, 0, sizeof sa);
    sa.sa_sigaction = text_ro_fault; sa.sa_flags = SA_SIGINFO; sigemptyset(&sa.sa_mask);
    sigaction(SIGBUS, &sa, &s_ro_prev_bus);
    sigaction(SIGSEGV, &sa, &s_ro_prev_segv);
    int rc = mprotect((void*)s_ro_lo, s_ro_hi - s_ro_lo, PROT_READ);
    fprintf(stderr, "[text-ro] guest 0x10000..0x%08X read-only (rc=%d)\n",
            (unsigned)(s_ro_hi - (uintptr_t)vm_base), rc);
}
#else
static int  quit_signal_take(void) { return 0; }
static void quit_signal_arm(void) {}
static void text_ro_arm(void) {}
#endif

/* ---------------------------------------------------------------------------
 * Main
 * -----------------------------------------------------------------------*/
static void harness_guest_caller(uint32_t opd, uint64_t a0, uint64_t a1,
                                 uint64_t a2, uint64_t a3, uint64_t a4,
                                 uint64_t a5, uint64_t a6, uint64_t a7)
{
    ppu_guest_call(opd, a0, a1, a2, a3, a4, a5, a6, a7);
}

/* Afro Samurai's choices among the toolkit's env-gated behaviours. Set before
 * anything reads them; an explicit value in the environment still wins.
 *
 * The toolkit (../ps3recomp-afro, upstream v0.19.0 + this port's memory
 * changes) already keeps the command buffer's last word for the wrap JUMP
 * (#648) and reports 249 MB of VRAM (#660), both of which this title needs. */
static void afro_defaults(void)
{
    /* PS3_MMAPPER_TRACK    real map/unmap/free bookkeeping. The title creates
     *     and destroys 256 KB mempools constantly and places its own 1 MB heap
     *     pages with map_memory; the untracked cursor overlapped the two
     *     (a block array memset over live list nodes) and then ran out of
     *     address space and the title abort()ed. */
    setenv("PS3_MMAPPER_TRACK", "1", 0);
    /* PS3_MAPPER_WIDE      room for its six ascending address reservations
     *     (512 MB + 5 x 256 MB); two came back ENOMEM otherwise. They may
     *     enclose the runtime's own windows, which the tracked mapper never
     *     hands out. The title routes free() by reservation range, so they
     *     must ascend. */
    setenv("PS3_MAPPER_WIDE", "1", 0);
    /* SPU_INTERP_UNLIFTED  FMOD's mixer task loads its DSP/codec plugins as
     *     overlays and branches into them (LS 0x25A18); with no lifted code
     *     there the job ended, and FMOD's event thread then waited for good on
     *     the mixer's event flag while holding the lwmutex the main thread
     *     needed. Interpret what has no lifted body instead (as Deadpool). */
    setenv("SPU_INTERP_UNLIFTED", "1", 0);
    /* SAVEDATA_CBRESULT_STRICT  with no save yet the title's stat callback
     *     answers ERR_NODATA; firmware turns that into ERROR_CBRESULT, which
     *     is how it learns to create one. ERROR_NODATA made it show "save data
     *     appears to be damaged" and stop. */
    setenv("SAVEDATA_CBRESULT_STRICT", "1", 0);
    /* SPURS_JG_WAIT_FOREVER  the skinning job chain is kicked at boot and sits
     *     on its guard until a game starts; the 20 s give-up ended it, and New
     *     Game then waited for a job-done event from a chain nobody walked. */
    setenv("SPURS_JG_WAIT_FOREVER", "1", 0);
    /* SPURS_JC_EVENT_FROM_JOB  the skinning ring's last job sends the one
     *     event per lap the render thread waits for (depth-1 queue); synthetic
     *     per-job events filled the queue and the real one was dropped. */
    setenv("SPURS_JC_EVENT_FROM_JOB", "1", 0);
    /* SPURS_JOB_CTX_V1  the skinning job reads numIoBuffer/numCacheBuffer at
     *     ctx+0x68/0x6A and the DMA tag at ctx+0x74 (older context layout);
     *     without them it halted on its guard every run, and a run whose
     *     garbage passed PUT its local store over the PPU text at 0x10000. */
    setenv("SPURS_JOB_CTX_V1", "1", 0);
    /* Frame pacing (30 fps). The title prepares each frame with
     * cellGcmSetPrepareFlip, folds the returned buffer id into its frame-ready
     * label, flips it from its vblank handler every 2nd vblank with
     * cellGcmSetFlipImmediate, and its flip handler frees the label the RSX
     * ACQUIREs before going on. Prepare-as-flip with the handler at call time
     * and a non-blocking ACQUIRE let it run ~45-48 frames a second (1.5x).
     *   GCM_PREPAREFLIP_FIFO=2  prepare returns the buffer id and does not flip
     *   GCM_FLIPCB_ONTICK=1     flip handler at completion, after the vblank
     *                           handler has recorded which buffer it flipped
     *   GCM_SEMA_ACQUIRE=1      the RSX waits on ACQUIRE (bounded, below) */
    setenv("GCM_PREPAREFLIP_FIFO", "2", 0);
    setenv("GCM_FLIPCB_ONTICK", "1", 0);
    setenv("GCM_SEMA_ACQUIRE", "1", 0);
    setenv("GCM_SEMA_ACQUIRE_MS", "250", 0);
    /* GCM_ENGINE_3D_SUBCH  the title's 2D blits (NV3089 on subchannel 5) must
     *     not reach the draw engine's NV4097 register file: their 0x0304-
     *     0x030C words switched the alpha test off, and grass and foliage
     *     cards drew as dark rectangles. */
    setenv("GCM_ENGINE_3D_SUBCH", "1", 0);
}

int main(int argc, char** argv)
{
    afro_defaults();
#ifdef __APPLE__
    /* A game, not background work: new threads inherit this. By default macOS lets a
     * thread that is not saturating a core drift to the efficiency cores and low clocks,
     * which here took one frame's RSX work from about 10 ms to about 24 ms, and the frame
     * from 60 fps to 30. PS3_QOS=0 leaves the default. */
    if (!getenv("PS3_QOS") || getenv("PS3_QOS")[0] != '0')
        pthread_set_qos_class_self_np(QOS_CLASS_USER_INTERACTIVE, 0);
#endif
    if (argc < 2) {
        printf("usage: %s <PPU ELF>\n", argv[0]);
        return 2;
    }

#ifdef _WIN32
#pragma comment(lib, "winmm.lib")
    /* 1 ms timer resolution. The default granularity is about 15.6 ms, which
     * inflates every shorter wait the title makes and throttles the whole
     * thing. POSIX timers are already fine-grained. */
    timeBeginPeriod(1);
    setvbuf(stdout, NULL, _IONBF, 0);   /* unbuffered: do not lose prints on a kill */
#endif

    printf("=== ps3recomp game runner ===\n");

    if (!alloc_guest_vm()) {
        fprintf(stderr, "ERROR: could not allocate the guest address space\n");
        return 1;
    }

    uint32_t entry = ppu_load_elf(argv[1]);
    if (!entry) {
        fprintf(stderr, "ERROR: could not load %s\n", argv[1]);
        return 1;
    }

    derive_vfs_root(argv[1]);
    printf("[boot] VFS root: %s\n", ppu_vfs_root);

    /* Tell cellGame who we are before the guest can ask. Two candidates: the
     * disc's own copy under the mounted image, then the one staged beside the
     * ELF. A title that builds "/dev_hdd0/game/<id>..." paths from the id it
     * reads back out of cellGameBootCheck goes looking for a directory that
     * can never be there when that id is the placeholder. */
    {
        static const char* const cand[] = { "/PS3_GAME/PARAM.SFO", "/game/PARAM.SFO" };
        char sfo[1024];
        for (unsigned i = 0; i < sizeof cand / sizeof cand[0]; i++) {
            snprintf(sfo, sizeof sfo, "%s%s", ppu_vfs_root, cand[i]);
            FILE* f = fopen(sfo, "rb");
            if (!f) continue;
            fclose(f);
            cellGame_init_from_paramsfo(sfo);
            break;
        }
    }

    ppu_recomp_register();   /* lifted function table -> address map */
    ps3_load_prx_modules();  /* this game's lifted system PRX, if it has any */
    ppu_hle_init();          /* firmware import NID -> HLE handlers */
    ppu_sysprx_register();   /* boot-critical CRT */
    ppu_fs_register();       /* cellFs over the game directory */
    lv2_init_syscalls();     /* the lv2 syscall table */
    text_ro_arm();           /* AFRO_TEXT_RO=1 diagnostic */
    quit_signal_arm();       /* SIGINT/SIGTERM: quit like Quit Game */

    /* Install the guest-callback hook, then start the frame clock. It no-ops
     * until the title registers its vblank and flip handlers during init. */
    g_ps3_guest_caller = harness_guest_caller;
    if (!getenv("GCM_PUT_DOORBELL") || getenv("GCM_PUT_DOORBELL")[0] != '0')
        g_gcm_put_doorbell = put_doorbell;
    CreateThread(NULL, 4u * 1024 * 1024, frame_clock, NULL, 0, NULL);

    printf("\n[boot] dispatching entry OPD 0x%08X (stack top 0x%08X)\n\n",
           entry, STACK_TOP);

    int rc;
#if defined(__APPLE__)
    /* The Metal backend pumps the window's events with dispatch_sync onto the
     * main queue. With the guest on the main thread nothing ever services
     * that queue, so the frame clock blocks in its first event pump, the FIFO
     * never drains, and the guest spins for ever waiting for get to reach its
     * put (the first boot died exactly there: "GCM control spin", get stuck
     * at 0). The guest runs on its own thread instead (256 MB host stack,
     * like every other guest thread) and the main thread turns the run loop.
     * Same fix as the Deadpool, Resistance 2 and inFAMOUS 2 ports. */
    {
        struct boot_args { uint32_t entry; uint32_t stack_top; int rc; volatile int done; };
        static boot_args ba;
        ba.entry = entry; ba.stack_top = STACK_TOP; ba.rc = 0; ba.done = 0;
        pthread_attr_t attr;
        pthread_attr_init(&attr);
        pthread_attr_setstacksize(&attr, 256u * 1024u * 1024u);
        pthread_t th;
        auto body = [](void* a) -> void* {
            boot_args* b = (boot_args*)a;
            b->rc = ppu_run(b->entry, b->stack_top);
            __atomic_store_n(&b->done, 1, __ATOMIC_RELEASE);
            CFRunLoopStop(CFRunLoopGetMain());
            return nullptr;
        };
        if (pthread_create(&th, &attr, body, &ba) != 0) {
            fprintf(stderr, "[boot] could not start the guest thread\n");
            return 1;
        }
        pthread_attr_destroy(&attr);
        while (!__atomic_load_n(&ba.done, __ATOMIC_ACQUIRE))
            CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.05, false);
        pthread_join(th, nullptr);
        rc = ba.rc;
    }
#else
    rc = ppu_run(entry, STACK_TOP);
#endif
    printf("\n[boot] ppu_run returned %d (entry function unwound)\n", rc);

    /* A title that called sys_process_exit never reaches this line: that path
     * ends in the host exit(). Getting here means the entry function returned
     * instead, so hand back the status the guest published if it published
     * one. Returning a hardcoded 0 would report success for a run that never
     * got anywhere. */
    return g_sys_process_exit_called ? (int)g_sys_process_exit_code : 0;
}
