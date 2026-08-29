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

At `bc5da76`, with `dclutch_general_accelerator_sbf.so` built by
`cargo build-sbf` at SHA-256
`f71fecbc162961a6528027c4352fc3b0f98ac2bc59cb88d67e26a605aa1a6dbb` and the test
caller at `6358c4f506582eb1d79547d9823092adf4be465ab7ba28084c7c9cbf4f4cf9c3`.
Both builds emit **zero SBF stack-frame diagnostics**. The accelerator digest
moved from `46ad714b...` when the root-lifecycle conjunct landed; the caller,
which authenticates no artifact, is byte-identical.

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
| `Consider` | 56,180 | 33 | 811 | 3 |
| `Freeze` | 32,608 | 31 | 745 | 3 |
| `InitializeSettlement` | 81,392 | 89 | 867 | 3 |
| `Collect` | 56,924 / 58,106 / 58,129 | 70 | 848 | 3 |
| `Materialize` | 53,125 | 68 | 814 | 3 |
| `Distribute` | 56,876 / 58,112 / 58,081 | 70 | 848 | 3 |
| `Close` | 61,290 | 87 | 833 | 3 |

### N = 258

| action | CU | accounts | legacy packet | scratch pages |
|---|---:|---:|---:|---:|
| `Consider` | 528,648 | 47 | 1,273 | 17 |
| `Freeze` | 65,019 | 45 | 1,207 | 17 |
| `InitializeSettlement` | 618,229 | 103 | **1,329** | 17 |
| `Collect` | 146,880 / 147,307 / 148,101 | 84 | 1,310 | 17 |
| `Materialize` | 141,356 | 82 | 1,276 | 17 |
| `Distribute` | 144,519 / 146,542 / 145,740 | 84 | 1,310 | 17 |
| `Close` | 155,744 | 101 | 1,295 | 17 |

Repeated rows are the three settlement orders the campaign drives, in order.

**The CU column is no longer joint, and it has been re-taken.** The caveat this
paragraph used to carry was that the run sat on a tree holding GEN-V3ACT's
uncommitted `GENERAL_HOT_COMMON_SCALARS_V3` 88 -> 90. That work is committed
(`37d873f`, and the conjunct it made room for in `2e890d4`), and every number
above is from one clean run at that HEAD. The `accounts`, `legacy packet` and
`scratch pages` columns did not move at all, which is the load-bearing part: two
more scalars is sixteen more bytes of bank and it crossed no boundary.

**What the root-lifecycle refusal costs is 16 CU per action.** Five actions at
N=1 and four at N=258 moved by exactly +16 against the previous run, which is
the price of one `ProjectDataU8`, one `load_const` and one `scalar_eq` in a
shared prelude. `Materialize` at N=258 moved +18 and `Close` at N=258 moved
**-20**; the repeated settlement rows cannot be compared positionally because
the prior table's row order within an action is not recoverable. Neither
outlier was chased: the worst action is 44% of the ceiling and no decision in
this family turns on twenty compute units.

**Re-measured after the Custody callee coordinate landed.** The five actions
that route to Custody carry one more account than the tables above did when
TA-GEN first wrote them, because the topology now declares the release-selected
Custody program the Hot executor resolves those routes through; before that they
could not be invoked at all. `Consider` and `Freeze` route to no child, carry no
callee, and their account counts are unchanged -- their CU moved by exactly +2,
the cost of the extra checked add in `general_effect_account_count_v3`. Every
other delta is +1 account, +1 legacy byte, and a few hundred CU.

**Compute is not this family's blocker.** The worst action at the canonical
width is `InitializeSettlement` at 618,229 CU — 44% of the 1,400,000 ceiling,
and that is the accelerator's own consumption inside a caller CPI, not the
whole Trading transaction. The N=1 → N=258 growth is sublinear in every action
except the two that fold the whole candidate (`Consider`, `Initialize`).

> **Superseded on 2026-08-27 by *The hash was 74% of the worst action* below.**
> The sentence was true, and it was true for a reason nobody had checked: 86%
> of `Consider` and 74% of `InitializeSettlement` were a software SHA-256 that
> did not need to be in the program at all. The worst action is now 164,400 CU,
> **11.7%** of the ceiling. The numbers in this section are the pre-conversion
> ones and are kept because the addendum is a comparison against them.

## Why this is not a fast lane

`tools/gauntlet/TIERS.md` admits `solana-program-test` behind a tier's fast
lane only when the tier states that **every** clause holds. One does not.

> The tier does not depend on packet serialisation limits. ProgramTest does not
> submit a packet, so it cannot catch a 1,242-byte frame against a 1,232-byte
> limit. Found31 is exactly this defect; it survived every fixture test.

Solana's legacy packet maximum is **1,232 bytes**. **Six of the seven N=258
actions exceed it** in this transport — 1,273, 1,276, 1,295, 1,310, 1,310 and
1,330 bytes. Only `Freeze`, at 1,207, fits. The campaign submits a legacy
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

## Standing gap — CLOSED for construction, 2026-08-27

The N=258 account sets needed the ALT-backed v0 plan **measured**, not
asserted. `docs/evidence/GENERAL_ALT_PACKET_WITNESS_2026_08_27.md` measures it:
all seven actions compile packet-safe through `compile_general_hot_v0`, widest
918 of 1,232 bytes, and the same account set with no lookup table refuses
`PacketTooLarge`. The derivation there reproduces every instruction-account
count in the tables above, which is this campaign acting as its control.

What that closes is the *construction* clause — the wire the operator would
submit. It is not execution, and it does not make this campaign a fast lane:
this campaign still submits legacy messages, so the tier clause it fails is
still failed here. The N = 1 fast lane described above remains admissible and
unbuilt.

## Addendum — the candidate now comes from a real batch (GEN-COLLECT, 2026-08-27)

Everything above was executed against an order flow that did not exist. The
candidate's `batch_id` was the literal `[0xb2; 32]`, and each Execution row's
`AuthenticatedOrderTermsV2` — the `max_lots` and `max_quote_debit_per_lot` the
verifier enforces `ExcessLots` and `QuoteLimit` against — was a value the
fixture asserted directly. Both are recorded as holes A and B of `M-12` in
`docs/decisions/0009-general-batch-collection.md`.

At `e898d56` the same seven actions run against a batch that was really opened
and orders that were really placed. `terminal_fixture` now opens a
`GeneralBatchV1` against a real `GeneralRootV2` (consuming the root's exact next
sequence), admits three `GeneralOrderV1` records with maker funding checked
against each order's worst case, closes the batch, and asserts the root returns
to `open_batches == 0`. The candidate's `batch_id` is the digest of that
opening; the row terms come from `authenticate_order_execution_v1`.

**This does not re-take the tables above, because it did not move them.**
`accounts`, `legacy packet` and `scratch pages` are identical in all fourteen
rows. That is the control: exchanging fabricated identities for real ones
changes the inputs' *provenance*, not the frame's geometry, and without the
table above there would have been nothing to check that against.

Two deltas, neither of which changes a conclusion here:

- **Every action costs exactly one CU less** than the tables above — all
  fourteen rows, uniformly −1. This is not from the collection half, which
  changes only which identities the fixture feeds in. The tree carried
  uncommitted edits to `dclutch-capability-program-contract`, which this
  program links. Flagged, not chased.
- **The repeated settlement rows permute.** Pages are now laid out in real
  identity order. The note above already says these rows "cannot be compared
  positionally"; that caveat is now load-bearing rather than precautionary.

**One defect found, of the kind only real identities can find.**
`runtime_verify::le_numeric_id` (`:1345`) orders a 32-byte identity as a
**little-endian 256-bit integer** — byte 31 first — which is not the
lexicographic order of `[u8; 32]`. The old fixture's identities were
`[low, 0, 0, …]`, where every high byte is zero and the two orderings agree, so
the distinction was untestable. A real `order_id` is a SHA-256 digest, where
they disagree almost always: sorting by Rust's own `Ord` refused three of the
four suites outright with `NonCanonicalOrder`. **A candidate builder must sort
by the protocol's identity order**, and the fixture now does, with the reason
written beside the sort.

**The census is unmoved and this addendum does not claim otherwise.** This is
still `solana-program-test` submitting legacy messages, so it still fails the
same tier clause for the same measured reason — six of seven N=258 actions
exceed the 1,232-byte limit. `general-accelerator/process_instruction` remains
NEVER-EXECUTED. What changed is what the accelerator was fed, not the transport
it was fed through.

## The hash was 74% of the worst action (SHASEAM, 2026-08-27)

The accelerator computed SHA-256 **in software**. Not by decision: the General
contract crates are deliberately SDK-free, and a `no_std` library that cannot
name the Solana SDK cannot name the `sol_sha256` syscall either, so they linked
`sha2`. A software compression loop costs roughly **104.75 CU per byte**; the
runtime's syscall costs **85 CU plus about half a CU per byte**. One digest of
the 4,288-byte verified candidate is the difference between 456,008 CU and
2,234.

`crates/dclutch-sha256-adapter` is now the one named place that knows a runtime
exists, and every General digest goes through it. **SHA-256 is SHA-256**, so
every digest VALUE is unchanged, nothing was regenerated, and no identity,
artifact or fixture moved.

Reproduced with the same command at `6d1ee60c`, accelerator ELF
`ead59b2264647649456bbd787f59e777b319887803fee3d189298e72b324dcf9`, caller
`3e9621e99612fca5fca98d98c15d1bf2a46b0c8d6637a1427cf6511ce01b7038`, both with
**zero SBF stack-frame diagnostics**. Suites 4/4 + 2/2 + 3/3, unchanged.

### N = 1 (re-taken)

| action | CU | Δ | accounts | legacy packet | scratch pages |
|---|---:|---:|---:|---:|---:|
| `Consider` | 36,071 | **−20,109** | 33 | 811 | 3 |
| `Freeze` | 32,643 | +35 | 31 | 745 | 3 |
| `InitializeSettlement` | 61,267 | **−20,125** | 89 | 867 | 3 |
| `Collect` | 56,953 / 58,135 / 58,158 | +29 | 70 | 848 | 3 |
| `Materialize` | 53,151 | +26 | 68 | 814 | 3 |
| `Distribute` | 56,903 / 58,108 / 58,139 | +27 | 70 | 848 | 3 |
| `Close` | 61,314 | +24 | 87 | 833 | 3 |

### N = 258 (re-taken)

| action | CU | Δ | accounts | legacy packet | scratch pages |
|---|---:|---:|---:|---:|---:|
| `Consider` | 74,835 | **−453,813 (−85.8%)** | 47 | 1,273 | 17 |
| `Freeze` | 65,054 | +35 | 45 | 1,207 | 17 |
| `InitializeSettlement` | 164,400 | **−453,829 (−73.4%)** | 103 | **1,329** | 17 |
| `Collect` | 146,909 / 147,336 / 148,130 | +29 | 84 | 1,310 | 17 |
| `Materialize` | 141,382 | +26 | 82 | 1,276 | 17 |
| `Distribute` | 144,546 / 145,767 / 146,569 | — | 84 | 1,310 | 17 |
| `Close` | 155,768 | +24 | 101 | 1,295 | 17 |

`accounts`, `legacy packet` and `scratch pages` are **identical in all fourteen
rows**, as they have been through every re-take of this table. Removing a hash
implementation moves compute and nothing else, and the three unmoved columns
are what says so.

> **Superseded on 2026-08-29 by *The rent-refund account* below.** The two
> `InitializeSettlement` rows are stale by exactly one account and one legacy
> byte: `f581af6b` widened Custody `InitializeReplay` from twelve accounts to
> thirteen on 2026-08-28, and `InitializeSettlement` is the only General action
> that embeds that operation. The other twelve rows are still exact. These
> tables are kept because the Δ column above is a comparison against them.

### The control is the slope, not the constant

Sixteen commits touched this program's surface between the baseline tables and
this run, so a raw CU difference is not attributable to any one of them. The
per-outcome slope is:

| action | CU/outcome before | after | |
|---|---:|---:|---|
| `Consider` | 1,838.4 | 150.8 | **12.2× cheaper** |
| `InitializeSettlement` | 2,088.9 | 401.3 | **5.2× cheaper** |
| `Freeze` | 126.1 | 126.1 | unchanged |
| `Materialize` | 343.3 | 343.3 | unchanged |
| `Close` | 367.5 | 367.5 | unchanged |

**Five of the seven actions have a bit-identical slope and moved by a uniform
+24 to +35 constant** — that constant is the other sixteen commits, and it is
not this change. The two that moved are exactly the two that fold the whole
candidate, which are exactly the two that digest it. That is the attribution.

The independent prediction was 456,008 CU of software hash against 2,234 by
syscall, i.e. **453,774 CU removable**. Measured: 453,813 and 453,829. The two
agree to within 55 CU on a 450,000 CU quantity, from opposite directions —
instruction-count arithmetic over the compression loop, and a real ELF in a
real runtime.

### Headroom, and what actually binds

`InitializeSettlement` is the binding action at both ends:

| | before | after |
|---|---:|---:|
| N = 258 share of the 1,400,000 ceiling | 44.2% | **11.7%** |
| outcomes before the ceiling (linear in N) | ~632 | **~3,337** |

**This headroom is not the operative limit and this document should not be read
as if it were.** Six of the seven N=258 actions still exceed Solana's
1,232-byte legacy packet maximum, unchanged by this work, and that is still why
the census row does not flip. Compute stopped being close to a wall; the packet
never stopped being one.

### Binary size

The accelerator ELF is **193,968 → 139,136 bytes, −54,832 (−28.3%)**. The
SHA-256 round-constant table is gone from `.rodata` and `sol_sha256` appears in
the dynamic imports. `dclutch_core_sbf.so` fell by the same order in the same
change, 989,104 → 933,328. Trading still carries the implementation: three
`shadow_digest_v3` functions take unbounded slices whose preimages cannot be
restated inside a 4,096-byte SBF frame, and they need a caller-supplied scratch
rather than a bound. See the commit message at `6d1ee60c` for the arithmetic.

## Addendum, 2026-08-29 — the rent-refund account

**One account joined `InitializeSettlement`, and this document could not say
so.** The tables above have carried a `commit` for the *run* since the first
re-take, but never one per *row*, so a number could go stale without anything
in the document changing. It did. From 2026-08-28 16:56 EDT until this re-take,
the `InitializeSettlement` account count here described no code.

It was caught by the evidence-pinned control in
`crates/dclutch-operator/src/general_hot_v3.rs`,
`the_derived_geometry_reproduces_the_executed_campaign_frame`, whose seven
literals are *this document's* numbers and are deliberately not derived from
that crate. Six of the seven still reconciled; the seventh asserted 104 against
a recorded 103. **A control that agreed with the code would have said nothing**
— it is precisely because those literals come from outside the crate that the
drift was visible at all.

### Provenance

Re-taken at `bb4e83ca`, accelerator ELF
`ce180115ccf17a12d07a7108f425ace9329a9922cd6f80baab4cf34a7391afe7`, caller
`3e9621e99612fca5fca98d98c15d1bf2a46b0c8d6637a1427cf6511ce01b7038`, both with
**zero SBF stack-frame diagnostics**. The accelerator digest moved from
`ead59b22...`; the caller is byte-identical to the `6d1ee60c` run, as it has
been through every re-take, because it authenticates no artifact.

**This was a filtered run: `--test lifecycle` only, 4/4.** `freeze.rs` (2) and
the `joined_artifacts` unit suite (3) were not re-run, because neither
contributes a row to these tables — `lifecycle.rs` alone drives all seven
actions at both widths. The three suite counts quoted earlier in this document
belong to their own runs and are not restated here.

### N = 1 (re-taken at `bb4e83ca`)

| action | CU | accounts | legacy packet | scratch pages | count set by |
|---|---:|---:|---:|---:|---|
| `Consider` | 36,113 | 33 | 811 | 3 | unmoved since `6d1ee60c` |
| `Freeze` | 32,659 | 31 | 745 | 3 | unmoved since `6d1ee60c` |
| `InitializeSettlement` | 61,753 | **90** | **868** | 3 | **`f581af6b`** |
| `Collect` | 56,991 / 58,196 / 58,173 | 70 | 848 | 3 | unmoved since `6d1ee60c` |
| `Materialize` | 53,171 | 68 | 814 | 3 | unmoved since `6d1ee60c` |
| `Distribute` | 56,942 / 58,147 / 58,178 | 70 | 848 | 3 | unmoved since `6d1ee60c` |
| `Close` | 61,334 | 87 | 833 | 3 | unmoved since `6d1ee60c` |

### N = 258 (re-taken at `bb4e83ca`)

| action | CU | accounts | legacy packet | scratch pages | count set by |
|---|---:|---:|---:|---:|---|
| `Consider` | 74,877 | 47 | 1,273 | 17 | unmoved since `6d1ee60c` |
| `Freeze` | 65,070 | 45 | 1,207 | 17 | unmoved since `6d1ee60c` |
| `InitializeSettlement` | 164,970 | **104** | **1,330** | 17 | **`f581af6b`** |
| `Collect` | 146,947 / 147,374 / 148,168 | 84 | 1,310 | 17 | unmoved since `6d1ee60c` |
| `Materialize` | 141,402 | 82 | 1,276 | 17 | unmoved since `6d1ee60c` |
| `Distribute` | 144,585 / 146,608 / 145,806 | 84 | 1,310 | 17 | unmoved since `6d1ee60c` |
| `Close` | 155,786 | 101 | 1,295 | 17 | unmoved since `6d1ee60c` |

The `InitializeSettlement` N=258 row is measured twice per run, by two
independent test functions, and both report 164,970 / 104 / 1,330.

### Why exactly one row moved

`f581af6b` ("direct: make replay setup and trade exterior callable") appended
`CustodyFrameRoleV1::RentRefund` at coordinate 12 of Custody `InitializeReplay`
and raised `INITIALIZE_REPLAY_ACCOUNT_COUNT_V1` from 12 to 13. It is **appended
past the existing frame, so it renumbered nothing**, and it is reached from
General through one edge only:

```text
general_effect_account_count_v3(InitializeSettlement)
  = general_child_account_start_v3        11   (8 prefix + 3 readonly evidence)
  + PROTOCOL_POSITION_ADMIT_ACCOUNT_COUNT_V1  26
  + INITIALIZE_REPLAY_ACCOUNT_COUNT_V1        13   <- 12 before f581af6b
  + OPEN_VAULT_ACCOUNT_COUNT_V1               16
  + general_custody_callee_account_count_v3    1
  = 67                                              (66 before)
```

and the campaign frame is `2 + ADMITTED_RUNTIME_ACCOUNTS_START_V3 + 67 +
general_scratch_pages_v3(N)`. `Close` embeds `CloseReplay`, the three transfer
actions embed `Transfer`, and `Consider` and `Freeze` route to no child at all —
none of which `f581af6b` touched. **Six of seven reconciling was not a puzzle;
it was the signature of a single-operation frame change.** It is also what lets
the shared inputs be exonerated by arithmetic rather than by bisection: the
prefix and the callee coordinate are shared with rows that still reconciled, so
neither could have moved without moving those too.

### What the account buys, and what it costs

The old path called `create_account`, which **fails outright if the account
already holds lamports** — so any stranger could permanently block replay
creation by sending dust to the PDA. The new path transfers any excess above
exact rent to the named beneficiary, charges the payer only the shortfall, then
`allocate`s and `assign`s at exact rent. The program authenticates the account
rather than trusting it: `rent_refund.key.to_bytes() != request.rent_refund`,
and distinctness from both payer and replay, refuse with `AccountFrame`.

The price, isolated by this table: **+1 account, +1 legacy byte, and +570 CU**
on `InitializeSettlement` at N=258 (164,400 → 164,970). Every other action moved
by +16 to +42 CU, the shared-prelude drift of a day of unrelated commits, with
`accounts`, `legacy packet` and `scratch pages` bit-identical. `InitializeSettlement`
remains the binding action at **11.8%** of the 1,400,000 ceiling.

**The packet conclusion is unchanged and the number is not.** Six of the seven
N=258 actions still exceed the 1,232-byte legacy maximum; the largest is now
1,330 rather than 1,329. The census row does not flip for the same reason it did
not flip before.

### The blast radius is Custody-wide

`f581af6b` changed a **Custody** frame, not a General one. Any family frame or
evidence document that embeds Custody `InitializeReplay` and was measured before
2026-08-28 16:56 EDT is stale by exactly one account. General's is the one that
was found, and it was found only because General had a control pinned to
evidence rather than to itself. **A family with no such control has no signal**
— which is the argument for the `count set by` column above, and for putting one
in every table that records a width.
