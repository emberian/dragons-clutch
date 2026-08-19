# Native quantized-basis occupation resolution V1

Status: **executable host preflight / live persistence and routing STOP**, 2026-08-19.

Semantic owner: `crates/clutch-bspline-accumulator`.  Runtime preflight:
`programs/clutch-sbf/program/src/native_window.rs`.

## 1. What is defined

This design reserves two immutable Terms statistic identities for native
degree-one through degree-three markets:

| id | exact meaning | final boundary |
|---:|---|---|
| 6 | average of the per-bucket **quantized native B-spline basis vectors** | `ExactOnly` |
| 7 | average of the per-bucket **quantized native B-spline basis vectors** | `LargestRemainderV1`, descending remainder with lowest-index exact ties |

The code names them
`STAT_QUANTIZED_BASIS_OCCUPATION_EXACT_06` and
`STAT_QUANTIZED_BASIS_OCCUPATION_LARGEST_REMAINDER_07`.  They are not aliases
for terminal point (1), sampled minimum (2), sampled maximum (3), TWAP (4),
evaluate-at-TWAP, or the exact-rational occupation control arm in
`research/bspline-window-semantics`.  Changing between ids 6 and 7 changes the
digest-bound Terms bytes.

For canonical buckets `j` and frozen denominator `D`, the preflight computes:

```text
W_j = QuantizedNativeBasis_D(x_j),  sum_i W_j[i] = D
M_i = sum_j W_j[i]
```

It then applies exactly the finalizer selected by Terms.  Quantization happens
once per bucket inside the registered point evaluator.  Statistic 7 may apply
one further, separately named largest-remainder boundary to `M_i / coverage`.
That is deliberately different from integrating exact rational basis values
and quantizing once at the end.

## 2. Canonical archive consumption and the no-midpoint rule

`preflight_sealed_archive` accepts a `SealedArchiveReceiptV1` and the exact
`ArchiveAccountViewV1` that produced it.  The receipt is constructible only by
the source/archive verifier after checking the canonical key, Dragon's Clutch
owner, non-executable metadata, SourceSpec, deployment/parser release, full
window domain, record lineage, seal, and recomputed page commitment.  Preflight
then rechecks:

- Terms self-certification and degree `1..=3`;
- exact statistic/finalizer registry membership;
- basis, edge, ambiguity, repair, failure, and evaluator versions;
- Terms feed and the canonical hash of the Terms-derived `WindowDomain`;
- exact start/end range and maturity cursor;
- nonzero exact archive page commitment; and
- every record's expected bucket, account key/owner, and committed bytes.

SourceArchive V1 records contain `(low, high)` conservative intervals and have
no authenticated missing-record kind.  Occupation preflight admits a record
only when `low == high`.  Any positive-width interval returns
`NonPointObservation`; there is no midpoint calculation and no endpoint
selection.

The source-neutral `CanonicalBucketV1` fold has an explicit `Gap` variant.
`Gap` increments `sample_count` but not `coverage_count`, contributes zero
mass, and survives every associative combine.  Both current finalizers refuse
when `gap_count != 0`.  Thus a future archive gap cannot be dropped or silently
renormalized.  SourceArchive V1 itself still requires complete accepted
coverage, so authenticated gaps require a new archive record/version before
they can reach this preflight from a live account.

## 3. Bounded algebra and stack shape

The current archive bound is 32 buckets and the outcome bound is 16.  The fold
uses no allocation, float, or unbounded search:

| operation | fixed bound |
|---|---:|
| archive verification | one 2,560-byte page and at most 32 records |
| accepted bucket | one degree-1--3 exact native point evaluation |
| summary combine | 16 checked `u128` additions after validation |
| exact finalizer | 16 division/remainder checks |
| largest-remainder finalizer | at most 15 winner scans over 16 entries |

Known host type sizes from the accumulator evidence are `BasisSpec = 288`,
`BasisDomain = 368`, `Summary = 656`, and `FinalWeights = 144` bytes.  The
runtime adapter separates domain construction, archive folding, singleton
append, and final conversion with `#[inline(never)]`; this is an engineering
measure, not SBF frame evidence.  `cargo-build-sbf` must still report no frame
over 4,096 bytes on the joined source.

The present preflight uses the existing `archived_observation` capability for
each record.  That function conservatively recomputes the 2,560-byte page
commitment on each call.  This is bounded but needlessly expensive for a live
instruction.  The production source/archive seam should return a lifetime-
bound verified page view after one commitment check and expose indexed record
reads through that view.  It must not replace the repeated check with an
unchecked slice or caller assertion.

No CU number is claimed.  Host tests do not measure SBF compute, and this
module is not routed into the bank fixture.

## 4. Exact remaining live ABI

The existing 319-byte native Resolution v3 record means
`RESOLUTION_MODE_DERIVED_POINT`: its `resolved_value: u128` is the exact point
from which the vector must be rederived.  Writing an occupation vector into
that mode with `resolved_value = 0`, an average coordinate, a midpoint, or a
sentinel would overload point semantics.  `require_live_persistence` therefore
returns `LivePersistenceUnavailable` for every preflight candidate.

The required occupation record is Resolution v4, exact length **383 bytes**.
It preserves the v3 prefix through the vector and adds 64 bytes of occupation
provenance before moving bump/flags to the end:

| offset | bytes | field | occupation rule |
|---:|---:|---|---|
| 0 | 1 | Resolution tag | existing tag 16 |
| 1 | 1 | version | 4 |
| 2 | 32 | market | canonical market id |
| 34 | 32 | terms | exact Terms digest |
| 66 | 32 | feed | exact SourceSpec/feed id |
| 98 | 32 | window | canonical `WindowDomain` id, not page content |
| 130 | 8 | sealed feed cursor | at least Terms maturity |
| 138 | 8 | sealed end bucket | equals Terms exclusive end |
| 146 | 8 | repair generation | equals Terms generation |
| 154 | 8 | resolved slot | first successful record slot; stable on retry |
| 162 | 1 | mode | new `DERIVED_QUANTIZED_OCCUPATION = 3` |
| 163 | 1 | payout index | unresolved sentinel, never preset search |
| 164 | 1 | outcome count | equals Terms |
| 165 | 16 | resolved value | canonical zero; inactive in this distinct mode |
| 181 | 8 | denominator | equals Terms and vector sum |
| 189 | 128 | weights | 16 `u64`, zero padded, sole persisted vector |
| 317 | 32 | archive commitment | exact sealed 2,560-byte page commitment |
| 349 | 2 | statistic id | exactly 6 or 7; checked against Terms |
| 351 | 1 | finalization | 1 `ExactOnly`, 2 `LargestRemainderV1`; checked against statistic |
| 352 | 2 | basis evaluator version | 1 |
| 354 | 2 | occupation summary version | 1 |
| 356 | 8 | sample count | exactly `end - start` |
| 364 | 8 | coverage count | accepted exact points |
| 372 | 8 | gap count | exactly `sample - coverage`; zero for successful V1 finalization |
| 380 | 1 | stored bump | canonical Resolution PDA bump |
| 381 | 1 | flags | zero |
| 382 | 1 | reserved | zero |

Unresolved v4 uses the same account length and has mode 0, zero window/archive/
cursor/range/generation/slot/statistic/finalizer/version/count/vector fields,
the unresolved payout sentinel, and nonzero market/terms/feed plus the stored
bump.  A successful exact retry must reauthenticate the same archive and
rederive byte-identical fields.  A different archive commitment, statistic,
finalizer, version, count, or vector is a conflict.

The account stays the sole persisted vector owner.  Kernel reconstruction and
internal/external redemption may copy the vector only into ephemeral stack
values.  They must recognize both v3 point and v4 occupation modes explicitly;
neither may infer a mode from vector contents, `resolved_value`, a payout-set
match, or account length without first checking digest-bound Terms.

### Instruction and account plane

No new resolver-chosen instruction field is needed.  Terms selects point v3 or
occupation v4 by its statistic id; the Resolve request retains global sequence
and the unresolved payout sentinel.

The occupation Resolve prefix is **10 accounts**, followed by the canonical
`n` outcome mints:

1. actor signer;
2. Market (writable);
3. Hoard accounting (read-only);
4. kernel aggregate (writable);
5. SupplyLedger (writable);
6. immutable Terms (read-only);
7. 383-byte Resolution v4 (writable);
8. Feed head (read-only);
9. immutable SourceSpec (read-only); and
10. sealed SourceArchive (read-only).

The current point route's eleventh hostile legacy evidence buffer is absent.
Occupation folds the canonical archive directly and accepts no duplicate
caller projection.  The SourceSpec and SourceArchive PDA derivations remain
the ones already selected by Terms feed and canonical window id.

### Required code joins

Promotion requires one serialized change after the concurrent source/native
lanes settle:

1. add and hostile-test the 383-byte v4 codec;
2. make market construction choose v4 for statistic 6/7 and use runtime Rent
   for the exact 64-byte increase over v3;
3. expose a one-verification, lifetime-bound sealed archive record view;
4. route statistic 6/7 directly from the ten-account plane to this fold;
5. persist/retry v4 and resolve the kernel with its vector;
6. audit every post-resolution consumer for explicit v3/v4 mode handling;
7. run exact/sub-lot internal and bearer redemption for degrees 1, 2, and 3;
8. add hostile interval, gap, wrong archive commitment, wrong mode/version,
   wrong Terms, repeated resolve, and late-failure rollback cases; and
9. rebuild one SBF ELF and record per-degree resolve/retry/redemption CU,
   transaction account count, 383-byte rent, frame diagnostics, and ELF digest.

## 5. Evidence and STOP

The isolated tests exercise statistic separation, no-midpoint refusal,
explicit-gap retention, associativity, exact-only refusal, canonical largest
remainder, archive ownership/content capability use, and the unconditional
live-persistence refusal.  They are **HOST-TESTED** evidence once run.

There is currently no routed instruction, v4 codec, bank transaction, CU
measurement, rent measurement, or SBF ELF evidence for this mode.  It must not
be described as live resolution, and statistic ids 6/7 must not be used to
found a value-bearing market until the entire ABI and joined bank gate above
lands.
