# Native quantized-basis occupation resolution V1

Status: **routed Resolution-v4 SBF subset / source ingestion and internal-buffer STOP**, 2026-08-19.

Semantic owner: `crates/clutch-bspline-accumulator`. Runtime fold:
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
| accepted bucket | one degree-1--3 exact native point evaluation plus 16 checked `u128` additions |
| sequential fold | one domain validation, then adjacent appends; no singleton-summary construction/combine |
| exact finalizer | 16 division/remainder checks |
| largest-remainder finalizer | at most 15 winner scans over 16 entries |

Known host type sizes from the accumulator evidence are `BasisSpec = 288`,
`BasisDomain = 384`, `Summary = 672`, and `FinalWeights = 144` bytes.  The
runtime adapter separates domain construction, archive folding, singleton
append, and final conversion with `#[inline(never)]`; this is an engineering
measure, not SBF frame evidence.  `cargo-build-sbf` must still report no frame
over 4,096 bytes on the joined source.

The live route obtains `VerifiedSealedArchiveViewV1<'a>` only after one complete
key/owner/lineage/seal/commitment verification. Its private constructor and
borrowed lifetime prevent an unchecked slice escape or mutation during the
fold. Indexed reads remain bounded by the authenticated record count and do
not rehash the page.

The production fold uses an unforgeable validated basis capability stored in
the private occupation domain and a private-state sequential builder. Smooth
degree-two and degree-three evaluation has one fixed-denominator production
path: respectively `2*h^2` and `12*h^3` on the admitted uniform grid.
Power-of-two factors are cancelled by shifts and the only possible odd factor
is three. The original reduced-`Fraction` Cox--de Boor evaluator is compiled
only in tests as a differential oracle; it has no production dispatch arm.
This changes neither per-bucket quantization nor final largest-remainder
selection.

Final-LTO SBF diagnostics name no `clutch_sbf` function over the 4,096-byte
frame limit. This is narrower than saying every backend diagnostic is clean:
known nonresident reference/layout functions still appear before final LTO and
must be classified by the artifact audit.

## 4. Live ABI

The existing 319-byte native Resolution v3 record means
`RESOLUTION_MODE_DERIVED_POINT`: its `resolved_value: u128` is the exact point
from which the vector must be rederived.  Writing an occupation vector into
that mode with `resolved_value = 0`, an average coordinate, a midpoint, or a
sentinel would overload point semantics. The routed statistic-6/7 path instead
requires the separate v4 shape below.

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

Market construction selects v4 only for digest-bound statistic 6/7 and funds
its exact 383 bytes from runtime Rent. Resolve accepts the ten-account prefix
above, verifies the canonical sealed page once, folds it, writes v4, and drives
the native kernel with the record-owned vector. Exact retry must rederive the
same record. Internal and external consumers decode v3 or v4 from Terms-selected
semantics and reconstruct the chosen vector only ephemerally.

## 5. SBF evidence and remaining STOPs

The real-ELF ProgramTest campaign keeps the existing v3 point cases and adds
three independent occupation degrees. Each v4 Resolve has 14 transaction
accounts (ten fixed accounts plus four canonical outcome mints), a 383-byte
Resolution account, and a runtime rent minimum of 3,556,560 lamports.

| degree | Resolve CU | exact retry CU | internal exact-lot CU | bearer exact-lot CU |
|---:|---:|---:|---:|---:|
| 1 | 1,240,370 | 1,086,756 | 774,666 | 783,687 |
| 2 | 1,253,040 | 1,099,426 | 778,209 | 786,987 |
| 3 | 1,262,471 | 1,108,857 | 776,599 | 784,692 |

A separate degree-two statistic-7 transaction records finalization byte 2 and
measures 1,251,516 CU. The table's degree sweep uses statistic 6; host algebra
tests independently exercise a non-exact average where statistic 6 refuses and
statistic 7 applies the canonical lowest-index-tie largest-remainder rule.

The same real-bank test refuses positive-width observations instead of taking
a midpoint; refuses an incomplete/gapped V1 page, a same-byte substitute
archive at the wrong key, a redundant caller projection, the v3 length, a
wrong v4 mode, and a conflicting archive commitment; and proves late Resolve
and bearer failures roll back all watched accounts. Exact/sub-lot internal and
bearer cases run for degrees one through three. Blank-bank construction also
creates an unresolved statistic-6 v4 account and measures 932,585 CU.

The validated fixed-denominator/sequential-fold optimization removes 15,860,
68,161, and 112,422 CU from the prior span-three initial degree-one, -two, and
-three rows. It does not close the chosen operating-headroom gate. The exact
gate is `units * 5 / 4 <= 1,400,000`, hence at most 1,120,000 measured CU.
A focused same-ELF campaign reconstructs distinct canonical sealed archives
and Terms for spans one and two rather than truncating the span-three bytes:

| exact record span | degree 1 initial CU | degree 2 initial CU | degree 3 initial CU | admitted at 25% headroom |
|---:|---:|---:|---:|---|
| 1 | 1,242,858 | 1,252,676 | 1,252,357 | **NONE** |
| 2 | 1,236,364 | 1,246,108 | 1,252,164 | **NONE** |
| 3 | 1,240,370 | 1,253,040 | 1,262,471 | **NONE** |

The small nonmonotonic differences are why this evidence does not extrapolate
an unmeasured record-count formula. The honest end-to-end initial-Resolve
admission profile is **NONE for every measured degree 1--3 and exact span
1--3**; spans 4--32 remain unadmitted and unmeasured, not inferred failures.
All span-three exact retries happen to clear 1,120,000 CU, but a retry cannot
make the first resolution reachable. On the same ELF, point-v3 initial Resolve
measures 1,088,245 / 1,092,118 / 1,100,512 CU for degrees one through three.
The remaining occupation overhead is therefore dominated by its distinct
source/archive and v4 account plane rather than the smooth evaluator: the
degree-three-versus-degree-one occupation spread is only 22,101 CU, far below
the 120,370 CU still needed even by the cheapest measured occupation row.

### Read-only liveness options beyond evaluator optimization

The following are design candidates only. Neither is implemented by the
current route, and the checks they discuss remain mandatory.

**Claim-neutral mint-sync omission.** Resolve currently reads and reconciles
the complete outcome-mint vector before committing the immutable payout
authority. Omitting that reconciliation is sound only if a protocol proof
establishes all of the following: every mint has the canonical protocol PDA as
its sole effective mint authority; no extension, delegate, hook, migration, or
alternate instruction can increase supply; every protocol mint transition
first increases the cached SupplyLedger value; external transitions can only
burn, making a stale cache an upper bound; and resolution/solvency use only
that conservative upper bound, never equality to live supply. Terminal-state
rules must also prohibit any later mint increase. Under those prerequisites,
removing `n` mint reads should remove `n` transaction accounts and save a
roughly per-outcome amount of parsing/check work (expected tens of thousands
of CU for the present 2/4/8-mint fixtures). That estimate is not bank evidence
and is not expected by itself to erase the full degree-three headroom deficit.

**Prepaid canonical `ResolutionWork` cursor PDA.** A separate program-owned
work account could split a bounded archive fold across transactions. It would
immutably bind market, Terms, feed, window, exact archive key and commitment,
statistic/finalizer and evaluator/summary versions; retain start/end and the
single next bucket cursor; and store checked sample/coverage/gap counts plus
all sixteen `u128` masses. Each chunk would reauthenticate the sealed immutable
archive once for that transaction, accept only the exact next contiguous
records, and atomically advance the cursor. Finalize would require the exact
end cursor, revalidate the accumulated invariant, create v4 once, drive the
kernel once, and irreversibly mark the work record finalized; retries could
only confirm byte identity. A direct fixed-width encoding is expected to be
about 480--520 bytes, one additional writable PDA, and approximately
4.23--4.51 million lamports at the currently measured rent schedule. Chunking
makes fold CU approximately linear in the chosen records per transaction but
adds dispatch/account verification to every chunk; the final transaction still
contains mint sync, kernel resolution, and v4 persistence. Exact frame/CU/rent
measurements, payer/close-recipient rules, conflict/rollback tests, and a proof
that no skip, duplicate, archive substitution, partial-finalize, or abandoned
work account can alter semantics are prerequisites to promotion.

One independent operatorless gap remains outside this cut, while a follow-up
closed the second:

- provider ingestion and public SourceArchive construction are not routed; the
  real-bank archive is canonical and verified but installed at genesis, so this
  is not a provider-ingestion claim; and
- **Closed by `RECORDED_REDEMPTION_SBF.md`:** `RedeemInternal` now consumes
  immutable Terms plus the persisted v2/v3/v4 Resolution record and accepts no
  Feed or caller evidence buffer. The retired expanded account list refuses.
