#!/usr/bin/env python3
"""Render the per-phase spend+heap table from a `hot_tail_profile` log.

Produce the log this reads with the profile test, which lifts the heap
diagnostically so the phases are separable:

    SBF_OUT_DIR=/private/tmp/dclutch-hot-cu/elf \\
    DCLUTCH_FIXTURE_SEED=0 \\
    cargo test --manifest-path programs/dclutch-trading-sbf/program-test/Cargo.toml \\
        --test hot_tail_profile -- --nocapture > /tmp/profile.log 2>&1
    tools/gauntlet/hot-cu/hot-tail-table.py /tmp/profile.log

W2p's checkpoints and marks log THREE numbers: total outstanding, the upward
bump position, and the bytes outstanding at the scratch (high) end.  The total
is the heap requirement; the split is there so a release is visible as the drop
in the scratch column rather than as an unexplained jump.

The `cu spent` column is a per-phase DIFFERENCE of the runtime's remaining-units
counter, so it inherits everything ledger M-61 says about the sweep's totals:
the bump-search draw lands inside whichever phase does the deriving, and it
moves when the trading ELF's digest moves.  Read the shape of the table -- which
phase dominates, where the heap peaks -- not the individual figures.

This utility deliberately uses only Python's standard library, as
`tools/sbf-footprint.py` and `tools/sbf-frame-sizes.py` do.
"""

import re
import sys

CU = re.compile(r"Program log: dclutch-hot-cu:(\S+)")
HEAP = re.compile(r"Program log: dclutch-hot-heap:(\S+)")
# The fourth word is the CEILING the allocator is enforcing, and it is why this
# pattern does not require it to be zero. A `hot_cu_checkpoint` logs zero there;
# a `hot_heap_mark` has logged the live capacity since 2026-09-01, so on chain
# it logs `0x10000`. The old pattern demanded a literal `0x0` in that position
# and therefore matched NO HEAP MARK AT ALL on a real run -- the marks were
# dropped in silence while the CU checkpoints still rendered, so the table
# looked complete and answered a different question than the one asked. Found
# 2026-09-02 by the lane that needed the heap column to attribute a
# 65,536-byte wall, and which read the raw log by hand instead.
NUMS = re.compile(
    r"Program log: (0x[0-9a-f]+), (0x[0-9a-f]+), (0x[0-9a-f]+), (0x[0-9a-f]+), 0x0")
CONS = re.compile(r"Program consumption: (\d+) units remaining")


def main(path):
    rows = []
    ceilings = set()
    pending = None
    remaining = None
    with open(path) as handle:
        for line in handle:
            m = CONS.search(line)
            if m:
                remaining = int(m.group(1))
                if pending and pending[0] == "cu" and pending[2] is None:
                    pending = ("cu", pending[1], remaining)
                continue
            m = CU.search(line)
            if m:
                pending = ("cu", m.group(1), None)
                continue
            m = HEAP.search(line)
            if m:
                pending = ("heap", m.group(1), None)
                continue
            m = NUMS.search(line)
            if m and pending:
                kind, label, rem = pending
                total = int(m.group(1), 16)
                scratch = int(m.group(3), 16)
                ceiling = int(m.group(4), 16)
                if ceiling:
                    ceilings.add(ceiling)
                rows.append((kind, label, rem, total, scratch))
                pending = None

    if not rows:
        raise SystemExit(
            f"{path}: no dclutch-hot-cu / dclutch-hot-heap marks found. "
            "This renders a `hot_tail_profile` log run with --nocapture; a "
            "`hot_heap_frame_is_inert` log carries no marks."
        )

    header = f"{'phase':<36}{'cu spent':>12}{'heap':>10}{'d-heap':>9}{'scratch':>10}"
    print(header)
    prev_rem = None
    prev_heap = None
    peak = 0
    peak_label = ""
    total_spent = 0
    for kind, label, rem, heap, scratch in rows:
        spent = ""
        if kind == "cu" and rem is not None and prev_rem is not None:
            total_spent += prev_rem - rem
            spent = f"{prev_rem - rem:,}"
        d = "" if prev_heap is None else f"{heap - prev_heap:+,}"
        indent = "  " if kind == "heap" else ""
        marker = f"{scratch:,}" if scratch else ""
        print(f"{indent + label:<36}{spent:>12}{heap:>10,}{d:>9}{marker:>10}")
        if kind == "cu" and rem is not None:
            prev_rem = rem
        prev_heap = heap
        if heap > peak:
            peak, peak_label = heap, label

    print()
    # The ceiling is READ, not assumed. It was hardcoded at 32,768 here, which
    # is the SDK default and not what Trading runs on: `admit_heap_frame_v1`
    # lifts the allocator to whatever the transaction granted, so the same peak
    # is exhaustion under one ceiling and 47% of another. The heap marks carry
    # the live capacity in their fourth word; a log with none (an off-chain
    # build, or checkpoints only) says so rather than quoting a number nobody
    # measured.
    if len(ceilings) == 1:
        ceiling = next(iter(ceilings))
        print(f"HEAP PEAK {peak:,} at {peak_label}   "
              f"margin against {ceiling:,} = {ceiling - peak:,}")
    elif ceilings:
        print(f"HEAP PEAK {peak:,} at {peak_label}   "
              f"ceilings observed: {', '.join(f'{c:,}' for c in sorted(ceilings))}")
    else:
        print(f"HEAP PEAK {peak:,} at {peak_label}   "
              "ceiling not reported by any mark in this log")
    print(f"CU spent across the profiled phases: {total_spent:,}")


if __name__ == "__main__":
    if len(sys.argv) != 2:
        raise SystemExit(__doc__)
    main(sys.argv[1])
