# Compute budgets for the golden transactions

`CU_BUDGETS.json` is the file. This is what it is for, what it can and cannot
catch, and how to change a number in it without lying.

## Why it exists

`DCLTGMF1` — the atomic founding, five stages in one rollback domain — cost
**1,184,132** CU on 2026-08-26 and **1,278,747** the next evening. That is
84.6% to 91.3% of Solana's 1,400,000 per-transaction maximum, in one evening,
from other lanes' concurrent changes to Core, Claims and Trading. Nothing in
the founding lane moved it and **nothing was watching it**.

There is no headroom to buy: the campaign already requests the maximum. At the
ceiling the failure is not slow, it is a **hard refusal with no partial
result** — which is exactly how Found31 died before `c61376d`, when it hashed
Core's 1.0 MB ELF twice on chain and exhausted 1,400,000 outright.

So: budgets, checked in, asserted by the gauntlet, with the transaction named
and the delta printed when one is exceeded.

## The shape

One file, one owner. A tier does **not** carry a copy of a number that lives
here; it carries one witness that names its campaign:

```json
{
  "id": "the-golden-transactions-are-inside-their-cu-budgets",
  "kind": "cu-budget",
  "campaign": "tier1",
  "provenance": "…"
}
```

The evaluator is a new **kind** inside the shared `tier1/check-witnesses.sh`,
not a new script — a second evaluator is a parallel authority path and the two
copies diverge on the day one of them learns something. The witness expands to
one row per budget entry, because "the campaign got more expensive" is useless
unless it says which transaction.

Five things are red, not just being over budget:

| verdict | what it means |
|---|---|
| `OVER` | the campaign consumed more than the budget. The row names the transaction and the delta. |
| `CEILING` | the budget is **above 1,400,000**. The transaction has stopped fitting and no tolerance can be written for it. |
| `MISSING` | the budget matched no transaction in the campaign. A budget that matches nothing overstates coverage, the same rule `bindings.json` lives under. |
| `AMBIGUOUS` | two transactions carry the label. A budget must name exactly one. |
| `SCHEMA` | `budget` is not `measured + tolerance`, or an enforced entry has a scope that is neither `transaction` nor `stage`. A hand-edited budget cannot drift from its stated basis. |

`RECORDED` rows are entries with `enforced: false`. They carry a required
`unenforced_reason` and are printed but never asserted.

A `stage` budget reads the chain's **own inner accounting lines**. The
evaluator walks the finalized log recovering each `consumed` line's depth from
the surrounding `invoke [n]` / `success` / `failed` lines, and a stage is the
n-th depth-2 invocation. No program address appears in the budgets file, so
stage budgets survive a run whose gauntlet-local addresses move.

## The noise, which is the whole problem

**These numbers are not deterministic, and the reason is exact.** Every campaign
here generates fresh signing keypairs per run — `Keypair::new()` in the
successor bootstrap, ProgramTest's own genesis payer in the fast lane. That
changes how many iterations `find_program_address` needs to find a bump, and
each iteration is one `sol_create_program_address` syscall at **1,500 CU**.

Every run-to-run delta this lane measured is an exact multiple of 1,500 (a
handful of ±4 residuals aside), which is what makes this a measurement rather
than a story.

Measured bands:

| what | band | how |
|---|---:|---|
| tier-1 `DCLTGMF1` | 58,494 | two campaigns 23 minutes apart, `d9f79bb` 08:27Z and `3b0c588` 08:50Z on 2026-08-27, seven ELFs byte-identical except Trading's line-number metadata |
| tier-1 `DCLTPCB1` | 51,005 | same pair |
| tier-1 `DCLTPCB1`, **within one campaign** | **79,500** | the `d9f79bb` campaign stages the ladder twice at different generations; the two differ by exactly 53 iterations on ONE binary |
| tier-1 reordered-tail refusal | 55,503 | same pair |
| tier-1 per-role activation | 1,500 – 9,000 | same pair |
| tier-1 Found31, its rollback case, the profile init, the non-terminal `DCLTPCB1` refusal | **0** | same pair |
| tier-4 founding case | 24,000 | six runs of the same ProgramTest campaign on the same ELFs |
| tier-4 late-Hoard refusal | 15,000 | same six |
| tier-4 substituted-ProgramData refusal | 6,000 | seven runs; it was 1,500 over the first six, and the seventh — taken after the 2026-08-27 Custody frame change — drew 4,500 higher. Three iterations. This lane did not distinguish noise from that change. |

The tolerance rule follows from that, mechanically:

```
tolerance = roundup(observed_band, 10_000) + 10_000, floor 15_000
budget    = measured + tolerance
measured  = the HIGHEST draw observed, never a single run
```

Pinning the highest draw is what keeps ordinary noise from producing a red row.

## What this catches, and what it does not

**A tolerance that exceeds the band cannot also catch a regression smaller than
the band.** On tier 1's founding transactions the band is 58,494–79,500, so a
+30,000 regression to `DCLTGMF1`'s **whole-transaction** number is not reliably
caught, and this file does not pretend otherwise.

Where the 30,000-scale teeth actually are, proved by injecting a 30,000 cut and
reading which rows go red (see "The injected-red proof" below):

- the four `DCLTGMF1` **stage** budgets — tolerances 20,000–40,000, so a
  regression localised to Realize, Claims or the outer's own join is red even
  when the whole-transaction row is not;
- every entry whose measured band is **zero**: Found31, the Found31 rollback
  case, the infrastructure profile init, the non-terminal `DCLTPCB1` refusal;
- the tier-4 fast lane, which is also the one that runs pre-campaign.

And the thing W1f actually asked for is caught unconditionally: **the moment
`DCLTGMF1` gets close enough to the ceiling that its budget can no longer be
written down.** Its budget is 1,348,747, which is 51,253 CU below 1,400,000.
When the measured value passes 1,330,000 the entry becomes a `CEILING` red row
and the campaign is refused. That refusal is the point. The number 1,278,747 is
not being blessed.

## The budgets

Ceiling: **1,400,000** — Solana's per-transaction `MAX_COMPUTE_UNIT_LIMIT`. The
chain's number, not ours.

`current` is the pinned value, which is the highest draw observed on
2026-08-27. `headroom` is what is left to the ceiling from that draw.

### tier 1 — `tools/gauntlet/run.sh --mode full`, real validator

| budget | budget CU | current | tolerance | headroom to ceiling |
|---|---:|---:|---:|---:|
| `dcltgmf1-whole` | 1,348,747 | 1,278,747 | 70,000 | **121,253 (8.7%)** |
| `dcltgmf1-stage-1-custody-lock` | 184,840 | 144,840 | 40,000 | 1,255,160 (89.7%) |
| `dcltgmf1-stage-2-core-found-and-permit` | 463,129 | 433,129 | 30,000 | 966,871 (69.1%) |
| `dcltgmf1-stage-3-custody-realize` | 123,858 | 103,858 | 20,000 | 1,296,142 (92.6%) |
| `dcltgmf1-stage-4-claims-foundingv5` | 287,951 | 267,951 | 20,000 | 1,132,049 (80.9%) |
| `dcltgmf1-stage-5-open-and-outer-joins` | — | — | — | RECORDED, not enforced |
| `dcltgmf1-hostile-rollback` | 52,686 | 32,686 | 20,000 | 1,367,314 (97.7%) |
| `dcltpcb1-whole` | 935,307 | 845,307 | 90,000 | 554,693 (39.6%) |
| `dcltpcb1-stage-1-custody-initialize` | 384,337 | 354,337 | 30,000 | 1,045,663 (74.7%) |
| `dcltpcb1-stage-2-custody-openhoard` | 135,594 | 115,594 | 20,000 | 1,284,406 (91.7%) |
| `dcltpcb1-stage-3-custody-opensourcecompartment` | 209,108 | 159,108 | 50,000 | 1,240,892 (88.6%) |
| `dcltpcb1-second-prestate-whole` | 882,807 | 792,807 | 90,000 | 607,193 (43.4%) |
| `dcltpcb1-reordered-tail-refusal` | 851,102 | 781,102 | 70,000 | 618,898 (44.2%) |
| `dcltpcb1-non-terminal-refusal` | 37,176 | 22,176 | 15,000 | 1,377,824 (98.4%) |
| `found31-whole` | 252,041 | 237,041 | 15,000 | 1,162,959 (83.1%) |
| `found31-substituted-market-rollback` | 158,399 | 143,399 | 15,000 | 1,256,601 (89.8%) |
| `core-infrastructure-profile-init` | 244,835 | 229,835 | 15,000 | 1,170,165 (83.6%) |
| `activation-role-core` | 566,984 | 546,984 | 20,000 | 853,016 (60.9%) |
| `activation-role-claims` | 593,441 | 573,441 | 20,000 | 826,559 (59.0%) |
| `activation-role-trading` | 741,945 | 721,945 | 20,000 | 678,055 (48.4%) |
| `activation-role-resolution` | 313,713 | 293,713 | 20,000 | 1,106,287 (79.0%) |
| `activation-role-custody` | 255,103 | 235,103 | 20,000 | 1,164,897 (83.2%) |
| `activation-refuses-pre-revocation-core` | 555,927 | 535,927 | 20,000 | 864,073 (61.7%) |
| `dcltpca1-unwind` | 189,496 | 159,496 | 30,000 | 1,240,504 (88.6%) |
| `dcltpca1-pre-expiry-refusal` | 162,166 | 142,166 | 20,000 | 1,257,834 (89.8%) |

`DCLTGMF1` is the only row whose headroom is in single-digit percent, and it is
**shrinking**: 15.4% at `cd05331`, 8.7% at `d9f79bb`, in one evening.

### tier 4 — `tools/gauntlet/tier4/run-campaign.sh`, ProgramTest, no validator

This is the **pre-campaign** check. It drives Core's `found` plus the one-shot
permit — the same Core code that is the 433,129-CU stage 2 of `DCLTGMF1` — with
no validator and no port, in well under a minute. A Core founding regression
surfaces here instead of after a six-minute campaign that needs the single
global `127.0.0.1:20890` slot.

| budget | budget CU | current | tolerance | headroom to ceiling |
|---|---:|---:|---:|---:|
| `series-consume-founds-with-permit` | 784,795 | 744,795 | 40,000 | 655,205 (46.8%) |
| `series-consume-founds-with-permit-replay-campaign` | 767,295 | 737,295 | 30,000 | 662,705 (47.3%) |
| `series-consume-late-hoard-refusal` | 722,942 | 692,942 | 30,000 | 707,058 (50.5%) |
| `series-consume-replayed-ticket-refusal` | 390,694 | 370,694 | 20,000 | 1,029,306 (73.5%) |
| `series-consume-substituted-programdata-refusal` | 209,223 | 189,223 | 20,000 | 1,210,777 (86.5%) |

### Recorded, not enforced

- `dcltgmf1-stage-5-open-and-outer-joins` — the RPC truncates the finalized log
  before the commit-last Open stage's own accounting line, so the only figure
  available is arithmetic (whole − 300 for the two ComputeBudget instructions −
  the four measured stages): 328,669 at `d9f79bb`, 322,681 at `3b0c588`. A
  subtraction inherits every other row's noise and would red-row on all of it at
  once. Budgetable the day a producer surfaces the untruncated log.
- `hot-canonical-bundle-phase-subtotals` — there is no green number to pin. The
  canonical Hot bundle does not pass at HEAD (tail over the 32,768-byte heap at
  phase 7; W2i's gate), and its phase subtotals need `--features hot-cu-profile`
  to turn the ten `hot_cu_checkpoint!` sites from no-ops into log lines. The
  entry carries ADR 0005's own measured table instead of a re-measurement, per
  the close-out doctrine. It becomes enforced the day W2i's heap gate is green.

## Re-pinning a number

A budget goes UP only with a reason, and the reason goes in `provenance`. The
honest sequence:

1. Run the campaign. If the row is `OVER`, first ask whether it is noise — a
   single draw above the band happens, and the fix for a genuinely wider band is
   a wider tolerance with the new band recorded, not a higher `measured`.
2. If it is structural, say what changed and where. "Core grew" is not a reason;
   "Core re-authenticates the Registry once more per Found stage" is.
3. Update `measured`, `tolerance` and `budget` together. The evaluator refuses
   `budget != measured + tolerance`, so they cannot drift apart silently.
4. If `measured + tolerance` now exceeds 1,400,000 you are not re-pinning a
   budget, you are recording that a transaction has stopped fitting. Do not
   shrink the tolerance to make it fit; that is the alarm going off.

## The injected-red proof

The gate has to be shown to be capable of failing, so it was cut and re-run
against real evidence. `DCLUTCH_CU_BUDGETS_OVERRIDE` points the evaluator at a
different budgets file and makes it print a three-line banner saying the run is
a demonstration and not a gate.

Against the `d9f79bb` campaign evidence, 24 witnesses:

| budgets | witness result | red rows |
|---|---|---|
| canonical | 24 checked, 0 failed, exit 0 | 0 of 23 |
| every enforced budget cut 30,000 | 24 checked, 1 failed, exit 1 | **15 of 23** |
| every enforced budget cut 100,000 | 24 checked, 1 failed, exit 1 | **23 of 23** |

A cut of `N` simulates a `+N` regression. Which rows survive a 30,000 cut is the
honest map of this gate's resolution: `dcltgmf1-whole` (tolerance 70,000),
`dcltpcb1-whole` (90,000) and the reordered-tail refusal (70,000) stay green,
while three of the four `DCLTGMF1` stage rows and every zero-band row go red.

A red row reads:

```
  OVER      found31-whole                                        237041     222041      +15000
            OVER BUDGET by 15000 CU: create canonical Found31 Market
```

## The owner-decision this lane surfaced and did not take

**Seed the campaign fixtures and every tolerance here collapses.**

If `dclutch-local-successor-bootstrap run` took a `--keypair-seed`, the tier-1
band would go to zero, every tolerance could drop to the 15,000 floor, and a
+30,000 regression to `DCLTGMF1` would be red on every run instead of on most.
The same is true of ProgramTest's genesis payer for the tier-4 fast lane.

Those are `tools/local-validator/bootstrap/successor/` (W1f) and
`programs/dclutch-core-sbf/tests/found_program_test.rs` (a protocol crate), and
this lane is read-only toward both. It is queued, not done, and it is the single
change that would most improve this gate.
