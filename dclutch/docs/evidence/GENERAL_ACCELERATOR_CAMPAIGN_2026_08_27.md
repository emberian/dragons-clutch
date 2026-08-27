# General accelerator — seven-action real-ELF campaign, 2026-08-27

This records what the General family's real-ELF campaign executes, what it
measures, and the one measured fact that keeps its census row from flipping.

**What this is.** `solana-program-test` execution of the real
`dclutch_general_accelerator_sbf.so`, invoked by CPI from a real caller ELF,
across all seven General actions at runtime widths **N = 1** and **N = 258**,
plus the joined seven-action artifact graph those actions authenticate against.

**What this is not.** It is not local-validator execution and it is not devnet
or mainnet evidence. `census observe` has recorded nothing from it, and
`general-accelerator/process_instruction` remains **NEVER-EXECUTED** in the
census. See *Why this is not a fast lane* — the reason is a measurement in this
document, not an omission.

## Reproducing it

```sh
programs/dclutch-general-accelerator-sbf/program-test/run-program-test.sh
```

At `1619124`, with `dclutch_general_accelerator_sbf.so` built by
`cargo build-sbf` at SHA-256
`46ad714bd475e046fa80a0a0c2ba3b154aae190ff1ee3e841e24261856aee343` and the test
caller at `6358c4f506582eb1d79547d9823092adf4be465ab7ba28084c7c9cbf4f4cf9c3`.
Both builds emit **zero SBF stack-frame diagnostics**.

## What executes

| suite | cases |
|---|---|
| `program-test/tests/lifecycle.rs` | 4 |
| `program-test/tests/freeze.rs` | 2 |
| `program-test/src/joined_artifacts.rs` (unit) | 3 |

- **All seven actions**, at both widths: `Consider` replaces an incumbent with
  the best valid submitted candidate and then `Freeze` seals it;
  `InitializeSettlement` → three `Collect` rows → `Materialize` → three
  `Distribute` rows → `Close` runs the settlement to a zero-inventory terminal.
- **Adversarial cases that refuse without a runtime write**: a manifest row
  whose source coordinates were substituted; a caller skipping to order three;
  a candidate substituted under a frozen selection at N=258; a substituted
  Product record; a substituted price scale; a corrupted scratch page; and a
  late child-precondition failure (nonzero Position table at terminal close),
  which refuses the whole candidate after every other check has passed.
- **The readonly property is asserted, not assumed.** After every execution the
  harness re-reads each runtime account and requires its bytes unchanged.
- **The joined artifact graph** — `CapabilityProgramSetV2`, seven
  `CapabilityProgramV4` descriptors, Profile13, LifecycleV5, RequestProfile,
  the admitted-AOT strategy, its certificate and its Registry admission —
  is generated from public semantic-owner encoders only, caches no identity,
  and is re-authenticated end to end at both widths. **This is green for the
  first time at `f9bf093`**; before it, `Close` refused admission on
  `LifecyclePolicy`.

## Measurements

Compute limit **1,400,000**, heap **32,768** (ProgramTest default; the campaign
requests no heap frame, so this is the canonical ceiling and not an adjusted
one). `legacy_packet` is one short-vector signature-count byte, one signature,
and the exact canonical message bytes.

### N = 1

| action | CU | accounts | legacy packet | scratch pages |
|---|---:|---:|---:|---:|
| `Consider` | 56,162 | 33 | 811 | 3 |
| `Freeze` | 32,590 | 31 | 745 | 3 |
| `InitializeSettlement` | 80,944 | 88 | 866 | 3 |
| `Collect` | 56,473 / 57,655 / 57,678 | 69 | 847 | 3 |
| `Materialize` | 52,674 | 67 | 813 | 3 |
| `Distribute` | 56,425 / 57,630 / 57,661 | 69 | 847 | 3 |
| `Close` | 60,840 | 86 | 832 | 3 |

### N = 258

| action | CU | accounts | legacy packet | scratch pages |
|---|---:|---:|---:|---:|
| `Consider` | 528,630 | 47 | 1,273 | 17 |
| `Freeze` | 65,001 | 45 | 1,207 | 17 |
| `InitializeSettlement` | 617,698 | 102 | **1,328** | 17 |
| `Collect` | 146,345 / 146,772 / 147,566 | 83 | 1,309 | 17 |
| `Materialize` | 140,819 | 81 | 1,275 | 17 |
| `Distribute` | 143,984 / 145,205 / 146,007 | 83 | 1,309 | 17 |
| `Close` | 155,245 | 100 | 1,294 | 17 |

Repeated rows are the three settlement orders the campaign drives, in order.

**Compute is not this family's blocker.** The worst action at the canonical
width is `InitializeSettlement` at 617,698 CU — 44% of the 1,400,000 ceiling,
and that is the accelerator's own consumption inside a caller CPI, not the
whole Trading transaction. The N=1 → N=258 growth is sublinear in every action
except the two that fold the whole candidate (`Consider`, `Initialize`).

## Why this is not a fast lane

`tools/gauntlet/TIERS.md` admits `solana-program-test` behind a tier's fast
lane only when the tier states that **every** clause holds. One does not.

> The tier does not depend on packet serialisation limits. ProgramTest does not
> submit a packet, so it cannot catch a 1,242-byte frame against a 1,232-byte
> limit. Found31 is exactly this defect; it survived every fixture test.

Solana's legacy packet maximum is **1,232 bytes**. **Six of the seven N=258
actions exceed it** in this transport — 1,273, 1,275, 1,294, 1,309, 1,309 and
1,328 bytes. Only `Freeze`, at 1,207, fits. The campaign submits a legacy
message deliberately, so the accelerator can see every scratch page directly;
the production operator's plan is ALT-backed v0, where the account keys do not
ride inline. **That plan is not exercised here**, so this campaign cannot
discharge the packet clause for itself — and it is the exact defect class that
put Found31 ten bytes over the limit after passing every fixture test.

At N = 1 every packet is 745–866 bytes and the clause is satisfied, so a fast
lane restricted to N = 1 is admissible on this clause.

The machinery for such a lane now exists: `ea4954a` landed
`tools/gauntlet/program-test-evidence`, deliberately general rather than
Series-specific, whose `record` emits one file per transaction in the shape the
census consumes. What a General N = 1 fast lane still needs is exactly four
things: `record` called from the campaign's `submit`; a `bindings.json` binding
each label to `general-accelerator/process_instruction`; a `witnesses.json`
whose provenance is independent of the code under test; and a stage in
`tools/gauntlet/run.sh`.

**That last one is why this lane did not build it.** `run.sh`, `census/src/**`
and `tier2/**` are all under live uncommitted edits by the Claims/Custody lane
as of this writing, `tier3/` is already claimed, and `TIERS.md` says in as many
words never to edit `run.sh` while a run is in flight. The correct handoff is
this document plus the `blocked.json` entry, not a collision.

**What would flip the row properly**, fast lane or not: the accelerator deployed
by a tier and invoked through the real Trading Hot path on a validator. A fast
lane is always *additional* evidence — a route whose only observation came from
one is recorded with that campaign name, and the report shows it. The Hot path
is downstream of the Trading heap wall, which is not this family's to move.

## Standing gap

The N=258 account sets need the ALT-backed v0 plan **measured**, not asserted.
Until some campaign submits those six actions as v0 transactions and records
their real wire size, the sentence "the production operator separately proves
the same account set packet-safe" — which the suite's own comment carries — is
a claim without a witness. It is the first thing a General tier should measure.
