# Native B-spline resolution in SBF

Status: **DEGREE-SELECTED CREATION + RESOLVE + INTERNAL REDEMPTION; NATIVE
BEARER EXIT STILL BLOCKED** (2026-08-19).

The production SBF instruction now has an explicitly versioned native path for
degree-one through degree-three point settlement.  It does not reinterpret the
legacy account:

- degree zero requires the 165-byte version-two `ResolutionAccount` and keeps
  the existing preset-index transition;
- degrees one through three require the 319-byte
  `NativeResolutionAccount` version three and the request's historical payout
  byte must be the `0xff` mode sentinel; and
- a terms/account-length mismatch refuses before any state write. Version-two
  bytes are never decoded as version three and smooth terms never search the
  preset set.

`CreateMarket` now reads canonical sealed Terms and selects the Resolution ABI
without changing its account order or PDA: degree zero creates the 165-byte v2
record, while degrees one through three create a canonical 319-byte unresolved
v3 record. The focused resolution fixtures still install v3 at genesis because
that permits precise hostile resolved and near-resolved prestates. This is not
a fallback around a missing production constructor.

The remaining live account-width gap is `RedeemExternal`, which still admits
and decodes only v2. Native bearer exit therefore remains fail-closed rather
than interpreting the aggregate's canonical index zero as a payout.

## Resolution transition

The native path authenticates the same market, Hoard, kernel aggregate,
market-wide supply ledger, immutable Terms, Resolution PDA, Feed head,
evidence buffer, and complete outcome-mint vector as categorical resolution.
Terms derive the exact source-adapter identity and versions, statistic,
ambiguity and edge policies, grid, window domain, degree, knots, denominator,
and generation. The folded `WindowResult` must match that whole domain.

Every native persisted resolution requires raw point evidence for every smooth
degree, including degree one. A non-point interval refuses even if its two
endpoints happen to quantize to the same vector. Degree two and degree three
retain the reference model's point-only rule. No midpoint, preferred endpoint,
or nearest preset exists.

The exact vector is obtained only from
`clutch_solana_reference::derive_payout_vector`, whose evaluator is
`clutch-bspline` and whose quantizer is `WEIGHT-ROUND-01`: floor every exact
scaled basis weight, distribute the residual by descending fractional
remainder, and break a tie toward the lowest outcome index. Before mutation,
the adapter constructs and validates the complete version-three record:

- market, terms, feed, and declared window identity;
- sealed cursor, exact end bucket, repair generation, and recorded slot;
- derived-point mode, `0xff` payout sentinel, and full active outcome count;
- the raw pre-edge statistic point; and
- the denominator plus all sixteen weight slots, including canonical zero
  padding.

The record is the sole persisted owner of the native vector. The reference-only
kernel aggregate stores phase, canonical resolved index zero, payout presets,
and aggregate supply, but no copy of the vector. Resolution reconstructs a
temporary `MarketState` in `DerivedBasis` mode and calls
`resolve_with_vector`; write-back discards the temporary vector and persists
only the phase/index projection plus the version-three record.

An exact retry re-folds and re-derives the complete expected record, requires
byte-equivalent semantic fields, and performs no write. A different window
identity, point, vector, domain, generation, or binding refuses. As in the
existing evidence plane, the window identity itself is a declared nonzero
label rather than an authenticated archive commitment; the payout decision is
bound to the fully checked `WindowResult` domain, but completing source/archive
authentication remains a separate gate.

## Internal redemption

`RedeemInternal` chooses the account version from immutable Terms. In native
mode it:

1. decodes the v3 record and binds it to the exact market, terms, feed, bump,
   mode, outcome count, denominator, maturity cursor, end bucket, and repair
   generation;
2. requires the record's declared window identity to equal the read-only
   redemption evidence label;
3. reconstructs a temporary resolved `MarketState` in `DerivedBasis` mode from
   the kernel aggregate and record-owned vector;
4. runs the unchanged exact kernel redemption; and
5. discards the vector after writing only aggregate supply, collateral,
   position cash/claims, and replay sequence.

Fractional payouts are never rounded. `quantity * weight` must be divisible by
the common denominator or the kernel returns `RemainderRequired`; an exact
quantity succeeds. The collateral token accounts are still checked for zero
movement because internal redemption converts locked backing into Position
cash rather than withdrawing it.

Two reusable crate-local seams prevent a future bearer implementation from
inventing a parallel vector truth:

- `observe_resolve::bound_native_resolution` is the single hostile-byte and
  terms/market binding projection from a v3 record to its exact vector; and
- `observe_resolve::reconstruct_native_market` creates the ephemeral derived
  kernel state from that vector.

`external_exit` must add a degree-selected 319-byte role, call those seams,
place bearer quantity in `Position.external`, call `redeem_external`, burn the
real Token-2022 claim, and prove the exact mint/supply/collateral deltas in one
transaction. Until that lands, native bearer redemption is a named STOP, not a
fallback to index zero.

## Real-SBF evidence

`programs/clutch-sbf/svm-tests/tests/native_resolution.rs` drives the built ELF
inside an Agave bank. At denominator 64 and raw point 4 it records:

| degree | distinct breakpoints | exact persisted weights | resolve CU | retry CU | internal redeem CU |
|---:|---|---|---:|---:|---:|
| 1 | `0,8,16,24` | `32,32,0,0` | 802,909 | 648,089 | 707,029 |
| 2 | `0,8,16` | `16,40,8,0` | 845,517 | 690,697 | 709,029 |
| 3 | `0,8` | `8,24,24,8` | 880,340 | 725,520 | 708,704 |

The measured exact redemption quantities are 2, 4, and 8 claims respectively,
each paying one collateral atom. The campaign also checks:

- byte-identical idempotent retry;
- non-point refusal and byte-identical rollback for all three degrees;
- conflicting window-identity retry refusal;
- exact fractional-remainder refusal without consuming replay;
- immutable Terms presentation, account-alias, and missing-mint-vector
  refusals;
- full canonical outcome-mint presentation; and
- transaction rollback after lifecycle/kernel mutation when synchronizing a
  hostile `u64::MAX` bearer mint supply fails late.

`cargo build-sbf --offline` completes and no frame diagnostic names a native
resolution helper. Any separate first-party diagnostic from another concurrent
instruction lane remains a release gate for that lane; it is not silently
attributed to this campaign. The longstanding unreachable offline-reference
and host-only layout diagnostics remain documented by the SBF program.

## Remaining promotion gates

The following must close before native settlement is described as generally
available:

1. complete the focused blank-account real-bank campaign for degree-selected
   `CreateMarket`, including prefunded predictable targets and late-failure
   rollback; the constructor and its host campaign are landed, while that
   runtime fixture remains in flight;
2. wire `RedeemExternal` through the two reusable native seams and add real-SBF
   burn, remainder-refusal, donation, and rollback cases;
3. audit every other post-resolution instruction so none reconstructs smooth
   state as `FinitePreset` index zero;
4. join Resolve to the new canonical source archive and sealed receipt rather
   than treating a caller-declared window identity as proof; and
5. rebuild the committed ELF, update its reproducibility manifest, and run the
   full signed 22-step lifecycle plus native scenarios from a clean tree.

Until those gates close, the accurate claim is: permissionless production
construction, native degree-one through degree-three point resolution, and
exact internal redemption are implemented with an explicit v2/v3 split;
resolution and redemption execute in the real SBF program, while the new
constructor awaits its focused blank-bank runtime proof and native bearer exit
remains fail-closed.
