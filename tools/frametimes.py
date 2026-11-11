#!/usr/bin/env python3
"""Frame-time percentiles from a PS3_FTRACE timeline.

    tools/frametimes.py <trace.txt> [--kind present|flip] [--skip S]

PS3_FTRACE=<start s>:<seconds>:<path> (the toolkit's include/ps3emu/ps3_ftrace.h)
records a PRESENT span each time the host shows a guest frame and a FLIP_REQ
instant each time the title asks for a flip. The intervals between successive
events are the frame times; this prints their count, mean, p50/p95/p99/max
and how many exceed 1.5x and 2x the 33.4 ms of a 30 fps frame. --skip drops
the first S seconds of the window (a scene change, a load).
"""
import argparse
import statistics
import sys

KINDS = {'present': 10, 'flip': 9}
TARGET_MS = 1000.0 / 29.97


def pct(sorted_vals, q):
    if not sorted_vals:
        return float('nan')
    i = min(len(sorted_vals) - 1, max(0, int(round(q / 100.0 * (len(sorted_vals) - 1)))))
    return sorted_vals[i]


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split('\n\n')[0])
    ap.add_argument('trace')
    ap.add_argument('--kind', choices=KINDS, default='present')
    ap.add_argument('--skip', type=float, default=0.0, help='seconds to drop at the start')
    a = ap.parse_args(argv)
    kind = KINDS[a.kind]
    ts = []
    with open(a.trace) as f:
        for line in f:
            if line[0] in '#T':
                continue
            p = line.split()
            if len(p) == 6 and int(p[0]) == kind:
                ts.append(float(p[5] if kind == 10 else p[4]))
    ts.sort()
    if ts:
        ts = [t for t in ts if t >= ts[0] + a.skip * 1e6]
    if len(ts) < 3:
        print('frametimes: fewer than 3 %s events in %s' % (a.kind, a.trace), file=sys.stderr)
        return 1
    d = [(b - x) / 1000.0 for x, b in zip(ts, ts[1:])]
    s = sorted(d)
    span = (ts[-1] - ts[0]) / 1e6
    print('%s: %d frames over %.1f s = %.2f fps' % (a.kind, len(d), span, len(d) / span))
    print('  mean %.2f ms  p50 %.2f  p95 %.2f  p99 %.2f  max %.2f  stdev %.2f'
          % (statistics.mean(d), pct(s, 50), pct(s, 95), pct(s, 99), s[-1], statistics.pstdev(d)))
    for k in (1.5, 2.0):
        n = sum(1 for x in d if x > k * TARGET_MS)
        print('  > %.0f ms (%.1fx a 30 fps frame): %d (%.2f%%)' % (k * TARGET_MS, k, n, 100.0 * n / len(d)))
    return 0


if __name__ == '__main__':
    sys.exit(main())
