# direct-fee-pair — C-04's fee completion, as census evidence

**What this campaign is for.** Two Trading routes —
`trading/hot_v3::process_hot_execution_v3` and
`trading/direct_fee_settlement_v1::process_direct_fee_settlement_v1` — have been
executing on five real role ELFs since 2026-08-31 and were **NEVER-EXECUTED to
the census the whole time**, because no `bindings.json` anywhere named them.
`docs/evidence/UNWITNESSED_ROUTES_BY_ROW_2026_09_01.md` lists the settlement
among C-04's four unwitnessed routes and is correct in its own terms: witnessed
means *a campaign binding names it*, and there was none. **The gap was this
directory, never the routes.** Nothing about the program changed to close it.

## What it proves

`FEE_SECOND_TRANSACTION_V1`'s claim is that the fee-bearing Direct trade does
not fit one transaction and does fit two. Both halves, measured here rather than
asserted:

| | CU | packet |
| --- | ---: | ---: |
| tx1, the fee-bearing fill (gross 200, fee 1 per side) | 1,317,145 | 1,167 |
| tx2, the permissionless `DCLTDFS1` settlement | 173,721 | 369 |

and five distinct refusals, each named by its own discriminant rather than by
`is_err()`: `Root` 0x4002, `Content` 0x4003, `FeeNotOwed` 0x400C,
`FeeDestination` 0x400D, `FeeSource` 0x400E.

## How the bindings were authored, which is the part that matters

**From the campaign's own folded evidence, never from what it ought to touch.**
The label is derived in
`programs/dclutch-trading-sbf/program-test/direct-hot/src/waist.rs` —
`submit_v0_observed`, the single funnel every Direct submission passes through —
as the libtest thread name, plus that test's submission ordinal, plus a
**disposition read back from what the runtime reported**. So:

- a label cannot name a transaction other than its own;
- a second submission inside one test cannot collide with the first, which
  matters because every case here submits the fill and then the settlement;
- a refusal that changes code changes *binding* rather than quietly reusing one.

**Child routes are deliberately not bound.** The invoke log names the child
*program* at depth 2 — Claims and Custody under the fill, Custody under the
settlement — but not which selector ran inside it. A binding this campaign
cannot corroborate from its own logs is worse than an absent one, so those rows
do not exist and this paragraph says why.

## Every transaction reports `wire_bytes`

`docs/design/PACKET_LIMIT_2026_09_01.md` records that no C-04 program-test did.
It matters because **ProgramTest submits no packet**: it enforces no 1,232-byte
maximum, so an over-wide frame survives a fast lane untouched — which is exactly
what Found31 was. The waist already computes the exact width for its own
assertion, so it reports that number instead of passing `None`. `None` would be
honest, but it is not evidence.

## Running it

```
tools/gauntlet/direct-fee-pair/run-direct-fee-pair.sh [<elf-dir>]
```

Builds the five role ELFs, **refuses on any `overwrites values in the frame`
diagnostic** rather than measuring on top of undefined behaviour, runs the seven
tests with `DCLUTCH_PROGRAM_TEST_EVIDENCE_DIR` set, and folds the observations
to `target/direct-fee-pair-evidence.json`. Without that variable the same suite
is an ordinary `cargo test` and records nothing, which is the ordinary case.

## Re-authoring after a change

The bindings are a function of the evidence. If a route, a refusal code, a CU
figure or a packet width moves, **re-run the campaign, read the fold, and
re-author the rows from it** — do not edit a row to make the census pass. A
binding edited to match a program is no longer evidence about that program.
