# Macro and micro optimization — handoff to codex

Written 2026-08-22 against the cycle-G seal (`0d52c561…`, 2,149,672 B,
manifest `902e8d2`). Every number below is a sealed or campaign-measured
baseline at that identity; re-measure before and after any change — a
number without its ELF hash is a rumor.

## Ground rules (read before touching anything)

1. **Any source byte forks the ELF identity** and forces a reseal
   (seal → manifest → attestation). Optimization work therefore lands in
   **batched waves**, not drive-by commits. Coordinate a wave, land it
   whole, reseal once (the cycle-G protocol in
   `research/liveness-policy-profile/artifacts/0d52c561909cedef/audit/`
   is the template).
2. **Measure first, always.** The CU meters live in the svm-tests
   campaigns (`scale_common::Meter` prints `scale.*/route CU: n` rows;
   the committed-walk runner prints per-step CU; the profile's
   `evidence.json` carries the sealed rows). The frame probe is the
   `.stack_sizes` method in STREAMING_RELATION_DESIGN §9. Two quanta to
   net out of any single-observation comparison: **1,500 CU per failed
   PDA probe** (fixture-key dependent) and the **150 CU heap-frame
   rider** on clearing-walk transactions.
3. **NEVER run unfiltered suites.** `--test <name>` or `-E 'test(...)'`,
   narrowest thing that could refute you. Full-suite runs only as a
   wave's final gate, once.
4. **The claim planes hold.** An optimization that widens admission,
   weakens a refusal, or trades exactness for cycles is not an
   optimization — the exact-integer discipline (conservation to the
   atom, exact considerations, the divisibility refusals) is
   load-bearing and non-negotiable.
5. House rules: rustfmt named files only (the tree is not fmt-clean);
   no stash; no `add -A`; boxed decode idiom for large accounts; zero
   `clutch_sbf` frame diagnostics is a sealed invariant (the audit
   refuses regressions).

## The macro map (architecture-level, biggest first)

### M1. ClearWork is the whale: 50,054-byte account, 47,846-byte codec

The streaming relation's checkpoint dominates the walk's fixed floor:
every `AdvanceClearWork` decodes and re-encodes it (~250–300k CU of each
300–440k advance is codec + relation, measured across the batch-size
range). Two recorded options, in preference order:

- **Partial decode**: an advance touches the cursor region, the fold
  state, and one pass's working set — not the ~44 KB of bulk arrays.
  A region-addressed codec (decode header + cursors, patch in place,
  re-digest incrementally) could cut the fixed floor by half or more.
  The tamper stack (ResumeFoldMismatch + header binding + the weld)
  must survive untouched — the incremental digest is the hard part and
  the reason this was deferred.
- **The Pod fallback, already sanctioned**: TIER2 plan T2-1 records
  `repr(C)+Pod` as the fallback if codec CU exceeds budget, re-gated by
  the (now 322-point) equivalence/resume corpus. It trades the
  validated-decode property for speed; take it only with the corpus
  green and the hostile-byte battery extended to the Pod path.

### M2. EntitleSlice is O(book), should be O(slice)

Sealed shape-keyed rows: **217,235 (1 page) / 416,385 (2) / 759,892 (4
pages)**. The route re-walks the complete bound page set per call to
re-derive two live orders' ranks. A per-order locator — rank → (page,
slot) witness table, computed once at freeze or carried in the sealed
feed — makes entitlement flat. At 32 receipts on a 4-page book this is
the difference between ~24M CU of entitlement and ~7M. The locator must
be digest-bound (candidate/feed plane) so it adds no trust; verify-
against-walk on first touch is the honest construction.

### M3. FreezeEpoch peaks the venue: 988,469 CU at 4 pages / 64 orders

88% of the 1,120,000 raw admission bound — the closest thing to a
ceiling problem we have. Options: per-page freeze steps (staged freeze,
V3-precedent shape), or accept and cap book size at admission (the
current de-facto state). If books ever grow past 4 pages, this is the
first wall.

### M4. Fold batching: the wire admits more records per fold than we send

The real packet bound is **6 folds/tx** (measured, keeper lane), plan
`[6,6,6,6,6,2]`, 486,413 CU/batch-tx. But packet cost is independent of
`record_count` and `MAX_FOLD_RECORDS_V1 = 4` — 24 records at 574,260 CU
means the CU bound, not the packet, is now binding. Raising records/fold
(8? 12?) cuts transaction count further; measure the CU curve and stop
at ~1.0M. Cold outlay baseline to beat: **15,291,920 lamports**.

### M5. Rent diet on the walk-plane accounts

TerminalClosure makes rent a float, not a burn — but cold outlay prices
keeper participation. The candidates, with their sealed sizes:
CandidateFeed **6,266 B** (fills/slices width vs actual book sizes),
OrderPage **4,012 B** (16 fixed slots — fine), ClearWork **50,054 B**
(see M1 — a partial-decode codec could also shrink the persisted
tail). Any width change is a format version bump + reseal + the
account_len re-pin chain; batch with M1.

### M6. Parked, deliberately (do not reopen without the stated trigger)

- **opt-z**: 31 stack-overflow regressions at `opt-level=z` recorded;
  the Tier-0 boxed idiom scales but the campaign is parked **unless a
  real rent-per-byte bill appears** (deployment). Roadmap Phase O2.
- **Refusal-path CU**: refusals are cheap by design (a refused walk
  short-circuits at the first pass boundary: one advance + close,
  ~430k total). Do not "optimize" refusal paths at the cost of the
  short-circuit structure.

## The micro map (function-level)

| site | baseline | note |
|---|---|---|
| `relation_v1::settle_cash` | frame 6,208 B (grew +128 with fees) | the largest relation frame; the batch plane has no frame gate but keep it under watch; the composite-numerator `#[inline(never)]` extraction (recovered 128 B in `end_pass`) is the pattern |
| `relation_v1_stream::end_pass` | frame 1,024 B (was 832) | same pattern applied; further splits possible if the stream ever gains a frame gate |
| `composite_fee_quote` / `finalize_composite_numerators` | 448 / 192 B | fine; checked u128 throughout — do not "optimize" into unchecked |
| PDA derivations | 1,500 CU per failed probe | stored-bump discipline: every runtime hot path should `create_program_address` with the stored bump; audit remaining `find_program_address` call sites in handlers (creation routes legitimately pay it once) |
| syscall substitution | the precedent: software `sha2::compress256` → `sol_sha256` gave **3–8× on every instruction** and dissolved two compute STOPs | audit for any remaining software implementations of syscall-available primitives (the ten-symbol import surface is the ledger; `sol_memmove_` was the last admission) |
| `Intent::decode` | buffer 402 B (`MAX_INTENT_BYTES`), no diagnostic | fine |
| big-account decodes | boxed idiom (`decode_final_pot_boxed` precedent) | mandatory for anything ClearWork-sized; the r10 audit (max 4,096, deepest `claim_truth::observe_outcome_mints`) refuses regressions |
| resolve/redeem family | 166–198k / 136–153k per degree row | dominated by basis evaluation + token CPI; the degree-2/3 rows are within ~8% of degree-0 — the spline evaluator is not the cost; don't chase it |

## Measurement protocol for any change

1. Build at the canonical path (`/Users/ember/dev/dragons-clutch`),
   twice, byte-identical, both profiles.
2. Run the narrowest campaign that covers the touched route with
   `Meter` labels ON (the scale_* files show the idiom); three runs;
   report min–max with the PDA quantum netted.
3. Frame probe before/after; the diagnostic set must stay at the
   sealed 28-symbols/zero-`clutch_sbf` shape.
4. The touched plane's conservation battery + hostile battery green.
5. Full suites once, under a wave's final gate.
6. Reseal rides the wave; the profile's W1 rows re-derive (their teeth
   refuse stale maxima automatically).

## Also queued for the next reseal wave (cheap riders, not optimization)

- The doc-window's two files (`orders_batch.rs`, `reservation.rs` doc
  links) close the standing 100/101 manifest contradiction.
- `programs/solana-layout/src/lib.rs:4653` stale prose ("310").
- The `dependency_packages` two-graphs audit fix (42 linked vs 101
  workspace).
- The upstream-ledger coverage regression (18 rows / 63 files).

— Claude Fable 5, for codex. The meters are honest; leave them that way.
