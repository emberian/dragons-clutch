# Macro and micro optimization — handoff to codex

Written 2026-08-22 from the cycle-G seal (`0d52c561…`, 2,149,672 B,
manifest `902e8d2`). Sealed rows retain that identity. Newer measurements are
explicitly labeled `UNSEALED_CURRENT_TREE` and name their own ELF and source
closure; they do not rewrite Cycle-G evidence. Re-measure before and after any
change—a number without its ELF hash is a rumor.

## Review corrections — 2026-08-22

The sealed rows below remain evidence for their named shapes. Several proposed
interpretations and one runtime/model consistency assumption do not.

1. **M2 is O(book + witness), not only O(book).** A live-rank locator removes
   `locate_pair`'s complete page-set walk, but `scan_witness` still scans as many
   as 416 slices to recompute both order totals, pair multiplicity, and
   exclusivity. Virtual slices run `scan_end_total`, and some inexact shapes
   also scan the fill vector. An O(slice) successor needs both an authenticated
   frozen-set location index and a candidate-bound order-to-slice aggregate /
   adjacency index. A locator alone is a useful partial optimization only if it
   is measured and named as such.
2. **M4 changes economics as well as batching.** V1 charges and rewards per
   successful Fold call, not per record. Raising `MAX_FOLD_RECORDS_V1` changes
   worker compensation and the prepaid deposit/outlay geometry even if the wire
   and CU limits admit it. Measure the CU curve and revise/version the cost
   schedule together; do not present a constant edit as a wire-only win.
3. **The fixed-book witness-width refutation passed.** Receipt validation
   carried a stale 128-index cap (fixed in the post-Cycle-G source). A new
   maximum-book campaign holds four pages/64 orders fixed while varying only
   witness width: Entitle measures 745,595 CU at one slice, 763,615 at 128,
   763,755 for target slice 128 in a 129-slice witness, and 803,935 at 416.
   The earlier 1.64M projection was invalid because its source rows co-varied
   page and witness width. Direct per-slice Entitle now has an executed maximum
   witness row; maximum-page portfolio full-pair, virtual, and inexact branches
   still need equivalent campaigns before the broader settlement envelope is
   claimed.
4. **The liveness-accounting mismatch and current Fold plan are measured.** Runtime
   validation requires 49,431,920 lamports for the 32-record minimum deposit at
   the accepted constants, because it budgets 32 successful Fold-call rewards.
   The projection now derives this separately from actual named-plan payouts /
   refunds and the external keeper budget; `runtime_schedule_matches_policy`
   is gone. The old 15,291,920 figure remains only under the explicit label
   `INVALID_RENT_PLUS_EXTERNAL_KEEPER_BUDGET_NOT_RUNTIME_PREFUND`. A separate
   `UNSEALED_CURRENT_TREE` campaign at ELF `a6381fbe…` executes the Fold(4)
   `[6,2]` plan at 514,332 CU / 1,228 bytes and 171,765 CU / 704 bytes. Its two
   external Fold-transaction quotes total 1,090,000 lamports; measured Begin,
   both folds, and Finalize total 1,610,000. The sealed Cycle-G row correctly
   remains a STOP because it has no composed measurement at its own identity.
5. **M1's codec attribution is a hypothesis.** The account/body sizes and route
   totals are measured; “250–300k codec + relation” and “cut the fixed floor by
   half” are not isolated measurements. Add codec-only SBF probes before
   selecting a representation change.
6. **The old ELF contained two identical 48,328-byte idle checkpoints.** In exact
   `a6381fbe…` unstripped symbols, `clear_walk::boxed_idle_checkpoint::IDLE`
   and the static used by `ClearWorkV1::encode_idle_into` have identical bytes
   and SHA-256. The current source gives `clutch-batch` one immutable accessor
   and removes the adapter-local copy. The combined repair/optimization wave
   passed three byte-identical artifact builds and reduced the stripped ELF by
   54,344 bytes, from 2,160,072 to 2,105,728, saving 378,234,240 lamports of
   persistent loader rent. That measured delta also includes the smaller
   terminal-refusal handlers, so 48,328 bytes remains the attributable static
   upper bound rather than a separately isolated result.
7. **Ten SOL is an architecture target, not a micro-optimization target.** At
   default rent the complete loader-v3 resident must be at most 1,436,444
   bytes. The second-pass runtime artifact audit at `169a1ba` produced a
   2,082,320-byte
   ELF, leaving another 645,876-byte / 31.02% reduction. The original
   `a6381fbe…` gap was 723,628 bytes / 33.50%. Exact `opt-level=s`
   produced 1,725,512 bytes / 12.01190904 SOL, but is RED: final
   `artifact::validate_artifact` reaches `r10-5064`. Rehabilitating that profile
   requires a named function split and the complete same-ELF stack/bank/CU gate.
8. **Account overhead is often larger than payload.** One account costs a
   128-byte rent overhead before its data. The largest current structural wins
   are versioned receipt pages, active-width ClearWork/CandidateFeed codecs,
   embedded mandatory funding tails, and specialized OrderPages. These change
   account formats, contention, and close geometry; they are macro work with
   explicit CU/rent/terminal tests, not packing tricks.
9. **The CreateMarket decode round trip was a real but bounded micro win.** The
   decoder now parses once, checks exact end-of-wire, and shares one semantic
   field validator with the encoder instead of allocating a scratch buffer and
   calling `Intent::encode`. The complete audit removed `Intent::encode` and
   `Intent::encoded_len` from the final ELF and measured 23,408 fewer bytes /
   162,919,680 fewer persistent-rent lamports than the prior 2,105,728-byte
   artifact. The resulting default ELF is
   `193c08723eaefeff9a1c2aa53c9e3feb58960a919fb0bbb7ca5da3bd817aa95b`;
   all 165 default and 168 mock-profile bank tests pass. This is worth keeping,
   but its scale reinforces rather than weakens the capability-profile verdict.

The broader generality findings — collateral-program mismatch, owner-rounding
admission restrictions, capacity profiles, and the payoff compiler — live in
[`docs/reviews/ARCHITECTURE_REVIEW_2026-08-22.md`](docs/reviews/ARCHITECTURE_REVIEW_2026-08-22.md).

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
every `AdvanceClearWork` decodes and re-encodes it. The 300–440k route rows are
measured, but the codec's share has not been isolated. Two recorded options, in
preference order:

- **Partial decode**: an advance touches the cursor region, the fold
  state, and one pass's working set — not the ~44 KB of bulk arrays.
  A region-addressed codec (decode header + cursors, patch in place,
  re-digest incrementally) may reduce the fixed floor.
  The tamper stack (ResumeFoldMismatch + header binding + the weld)
  must survive untouched — the incremental digest is the hard part and
  the reason this was deferred.
- **The Pod fallback, already sanctioned**: TIER2 plan T2-1 records
  `repr(C)+Pod` as the fallback if codec CU exceeds budget, re-gated by
  the (now 322-point) equivalence/resume corpus. It trades the
  validated-decode property for speed; take it only with the corpus
  green and the hostile-byte battery extended to the Pod path.

### M2. EntitleSlice is O(book + witness), but the book term dominates

Sealed shape-keyed rows: **217,235 (1 page) / 416,385 (2) / 759,892 (4
pages)**. The route re-walks the complete bound page set per call to
re-derive two live orders' ranks, then re-walks the selected candidate's
witness to authenticate the ends' aggregate fill and exclusivity. A
per-order locator — rank → (page, slot), computed during the authenticated
walk — flattens the first term only. A candidate-bound adjacency/aggregate
index flattens the second. Both indexes need a versioned digest/account
binding, and their first construction must be checked against the complete
walk. The new fixed-book measurements put the witness slope near 140 CU per
additional explicit slice: 745,595 CU at width 1 versus 803,935 at width 416.
The full 416-slice route therefore fits; an index is a throughput/cost design,
not an immediate single-transaction liveness rescue. At 32 receipts on a
4-page book, the complete design still targets the difference between roughly
24M CU of entitlement and 7M, but a locator-only change must report its actual
smaller result and include index construction/rent/closure before promotion.

### M3. FreezeEpoch peaks the venue: 988,469 CU at 4 pages / 64 orders

88% of the 1,120,000 raw admission bound — the closest measured route to a
ceiling. The safest first change is not a new staged ABI: FreezeEpoch currently
walks the pages for commitment/owners and then re-walks the sealed pages for
binding/horizon facts. Return all authenticated prestate facts from one
specialized traversal and replace the post-seal re-walk with cheap header
checks. Only then evaluate per-page staged freeze. If books grow past four
pages, this remains the first measured wall.

### M4. Fold batching and compensation: the route may admit wider folds

The current keeper packet bound is **6 Fold instructions/tx**. Against the
audited unsealed default ELF `a6381fbe…`, the `[6,2]` current-ABI plan executes:
six Fold(4) calls consume 514,332 CU / 1,228 bytes, and two consume 171,765 CU /
704 bytes. The first packet has only four bytes of legacy-packet headroom. The
combined external Fold quote is 1,090,000 lamports; Begin + both Fold sends +
Finalize is 1,610,000. Work/Reserve/Resolution bytes match eight singleton
Fold(4) transactions, and an invalid fourth instruction rolls the entire
six-call transaction back.

The runtime minimum-deposit baseline remains **49,431,920 lamports**. Eight
successful Fold(4) calls plus Finalize pay 10,790,000 and refund 38,641,920;
those protocol ledger facts are independent of the external transaction quote.
If a wider successor is still useful, width 6 is the first boundary worth
testing; there is no evidence-based reason to jump to 8 or 12. Every width
change also alters per-call rewards, `fold_count`, progress granularity,
required deposit, and refund behavior. The current result is unsealed and must
not be copied into the sealed Cycle-G row.

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

### M7. Product-driven capability profiles are required for deployment rent

The current `193c0872…` ELF is 2,082,320 bytes with 1,942,200 bytes of `.text`.
Dependency pruning is not the lever. Exact final-symbol attribution gives this
first-order ownership map; LTO means only actual profile builds can establish
the resulting artifact sizes:

| capability family | resident bytes |
| --- | ---: |
| general clearing | 501,952 |
| Direct V3 | 250,360 |
| occupation/resumable resolution | 186,440 |
| Direct V2 | 101,656 |
| Source V2/Pyth | 66,024 |
| legacy Source V1 | 44,176 |
| all-tags `Intent::decode` | 44,776 |

The exact duplicate-code ceiling is only 2,984 bytes—0.46% of the 645,876-byte
gap. A measured V1/V2 `canonical_window_id` alias confirmed why source-level
similarity is not enough: both 184-byte symbols already shared one address, and
the refactor saved zero ELF bytes and zero lamports. It was reverted.

Build and measure these strict siblings before considering CPI decomposition:

1. **Direct V3 + Source V2, categorical/point.** Removing general clearing
   except the shared 31,392-byte relation implementation, Direct V2, Source V1,
   and occupation/resumable resolution has about 802,832 bytes of direct
   attribution. This is the strongest sub-ten-SOL candidate.
2. **General clearing + Source V2, categorical/point.** Removing Direct V2,
   Direct V3, Source V1, and occupation/resumable resolution has 582,632 bytes
   of direct attribution, leaving a 63,244-byte first-order gap. Pruning the
   disabled variants from the decoder and support tree may close it; only an
   exact profile build can say.
3. **General clearing + Source V2 + occupation.** Direct attribution removes
   only 396,192 bytes. It is not a credible ten-SOL profile without another
   capability removal or a program split.

A real profile must gate intent variants and strict decoding, dispatch arms,
account codecs, reference request/action decoders, generation dispatch, and
source generations together. Gating handler modules alone leaves the
44,776-byte all-tags `Intent::decode` and much of the layout surface
resident. These profiles select deployable products; they must not change the
shared Eggcrate semantics or pretend that a nonresident capability was tested.
Splitting into CPI programs comes only if measured selective monoliths cannot
meet the target, because multiple deployed siblings may increase aggregate rent
and change program IDs, PDA namespaces, atomicity, and upgrade coordination.

## The micro map (function-level)

| site | baseline | note |
|---|---|---|
| `relation_v1::settle_cash` | frame 6,208 B (grew +128 with fees) | the largest relation frame; the batch plane has no frame gate but keep it under watch; the composite-numerator `#[inline(never)]` extraction (recovered 128 B in `end_pass`) is the pattern |
| `relation_v1_stream::end_pass` | frame 1,024 B (was 832) | same pattern applied; further splits possible if the stream ever gains a frame gate |
| `composite_fee_quote` / `finalize_composite_numerators` | 448 / 192 B | fine; checked u128 throughout — do not "optimize" into unchecked |
| PDA derivations | 1,500 CU per failed probe | keep canonical `find_program_address` under the current contract. A stored-bump path changes the trust boundary and malformed-bump refusal; consider it only with a constructor/preservation proof, named genesis/upgrade assumption, pinned SDK helper, and hostile bank matrix |
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
