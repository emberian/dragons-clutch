# Native B-spline resolution in SBF

Status: **DEGREE-SELECTED CREATION + AUTHENTICATED RESOLVE + INTERNAL AND
BEARER REDEMPTION IMPLEMENTED; PROMOTION GATES REMAIN** (2026-08-19).

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

## Resolution transition

The native path authenticates the same market, Hoard, kernel aggregate,
market-wide supply ledger, immutable Terms, Resolution PDA, Feed head,
canonical SourceSpec, sealed SourceArchive, compatibility evidence projection,
and complete outcome-mint vector as categorical resolution. Terms derive the
exact source-adapter identity and versions, statistic, ambiguity and edge
policies, grid, window domain, degree, knots, denominator, and generation. The
SourceSpec and archive must be exact program-owned PDAs; the archive receipt
binds its bump, feed, canonical window, sealed cursor, page commitment, and
publish lineage. The compatibility blob must project every archived bucket and
interval exactly before its folded `WindowResult` is checked against the same
domain. A caller-supplied window label is no longer authority.

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

An exact retry re-authenticates the archive, re-folds, and re-derives the
complete expected record, requires byte-equivalent semantic fields, and
performs no write. A different archive, window identity, point, vector, domain,
generation, or binding refuses.

## Internal redemption

`RedeemInternal` is now a single recorded-resolution-only ABI. Its fixed state
prefix is nine accounts—actor, Market, Hoard, Position, kernel, Replay,
SupplyLedger, immutable Terms, and immutable Resolution—followed by seven
collateral-admission/custody roles and the complete canonical mint vector. It
accepts no Feed, SourceArchive, or caller evidence buffer after resolution.
Immutable Terms select v2, v3, or v4 record bytes; the selected record plus
kernel mode/vector/index are the sole payout authority. In native mode it:

1. decodes the v3 record and binds it to the exact market, terms, feed, bump,
   mode, outcome count, denominator, maturity cursor, end bucket, and repair
   generation;
2. requires the record's window identity to equal the canonical identity
   recomputed from the immutable market Terms;
3. reconstructs a temporary resolved `MarketState` in `DerivedBasis` mode from
   the kernel aggregate and record-owned vector;
4. runs the unchanged exact kernel redemption; and
5. discards the vector after writing only aggregate supply, collateral,
   position cash/claims, and replay sequence.

Categorical v2 likewise reads the persisted payout index rather than re-folding
evidence. Occupation v4 reads the persisted record-owned vector and provenance
fields without requiring the historical archive to remain live on every
redemption. Fractional payouts are never rounded. `quantity * weight` must be divisible by
the common denominator or the kernel returns `RemainderRequired`; an exact
quantity succeeds. The collateral token accounts are still checked for zero
movement because internal redemption converts locked backing into Position
cash rather than withdrawing it.

Two reusable crate-local seams prevent either redemption path from inventing a
parallel vector truth:

- `observe_resolve::bound_native_resolution` is the single hostile-byte and
  terms/market binding projection from a v3 record to its exact vector; and
- `observe_resolve::reconstruct_native_market` creates the ephemeral derived
  kernel state from that vector. Its caller checks the reconstructed market in
  a separate frame before any transition, avoiding a live SBF frame overlap.

## Bearer redemption

`RedeemExternal` now selects the Resolution ABI from immutable Terms just as
Resolve and `RedeemInternal` do. Degree zero keeps the v2 preset path. Degrees
one through three require the exact 319-byte v3 record, bind it through
`bound_native_resolution`, reconstruct the derived market ephemerally, place
only the presented bearer quantity into a temporary `Position.external`, and
call the unchanged exact `redeem_external` kernel transition. No owner Position
or replay account participates: possession of the admitted Token-2022 account
and its authenticated owner signature is the authority.

The complete canonical outcome-mint vector is observed before mutation. The
selected holder token account and selected mint are writable; every other mint
is read-only. An exact successful transaction:

1. requires `quantity * weight` to divide the common denominator, otherwise
   returns `RemainderRequired` before burn;
2. burns exactly `quantity` claims from the holder and the selected mint;
3. transfers exactly the computed payout from the Hoard token account to the
   holder's collateral account under the market PDA;
4. verifies the exact post-CPI token and mint deltas;
5. synchronizes the market-wide external-supply mirror and aggregate supply;
   and
6. decreases `HoardAccount.collateral_atoms` by exactly the payout while
   retaining the one-sided Hoard-token coverage floor.

Unsolicited collateral surplus is therefore conservative: it may strengthen
the token-account coverage floor, but it does not inflate recorded collateral
or the computed payout. No numerator credit, rounded payout, second persisted
vector, midpoint, or preset-index fallback exists. A late Token-2022 transfer
failure after a successful burn rolls the burn and every protocol write back
atomically.

## Real-SBF evidence

`programs/clutch-sbf/svm-tests/tests/native_resolution.rs` drives the built ELF
inside an Agave bank. At denominator 64 and raw point 4 it records:

| degree | distinct breakpoints | exact persisted weights | resolve CU | retry CU | internal redeem CU | bearer redeem CU |
|---:|---|---|---:|---:|---:|---:|
| 1 | `0,8,16,24` | `32,32,0,0` | 1,088,275 | 934,630 | 778,952 | 788,049 |
| 2 | `0,8,16` | `16,40,8,0` | 1,092,178 | 938,533 | 776,495 | 785,349 |
| 3 | `0,8` | `8,24,24,8` | 1,100,560 | 946,915 | 776,385 | 784,554 |

The measured exact redemption quantities are 2, 4, and 8 claims respectively,
each paying one collateral atom. The campaign also checks:

- byte-identical idempotent retry;
- non-point refusal and byte-identical rollback for all three degrees;
- compatibility-buffer/archive mismatch and canonical-archive substitution
  refusals;
- exact fractional-remainder refusal without consuming replay;
- immutable Terms presentation, account-alias, and missing-mint-vector
  refusals;
- exact refusal of the retired Feed/buffer-expanded redemption account list;
- wrong record mode, terms binding, and canonical window refusals without
  writes;
- whole-transaction rollback when a first internal redemption succeeds and a
  stale replay fails later in the same transaction;
- full canonical outcome-mint presentation; and
- transaction rollback after lifecycle/kernel mutation when synchronizing a
  hostile `u64::MAX` bearer mint supply fails late;
- positionless d1/d2/d3 bearer exits at the minimal exact lots 2, 4, and 8,
  including exact burn, supply, collateral, and token deltas;
- sub-lot bearer refusal with byte-identical protocol and token state; and
- rollback of a successful Token-2022 burn when the later collateral transfer
  overflows.

The separate blank-bank campaign constructs the account family from public
sealed artifacts: degree zero creates v2/165 bytes in 916,052 CU and degree one
creates v3/319 bytes in 909,302 CU. The focused hostile-resolution fixture
still injects its deliberately chosen prestate at genesis; it is not the
constructor proof.

`cargo build-sbf --offline` completes and no frame diagnostic names
`external_exit`, `recorded_redeem`, `apply_recorded_redemption`, or
`reconstruct_native_market`. The latest joined provisional build log names no
first-party live program function; the final-LTO survivor audit remains the
authority for whether any dependency diagnostic survives the deployed ELF and
is still a program-wide release STOP even though the focused real-SBF
transactions above execute successfully. The
longstanding unreachable offline-reference and host-only layout diagnostics
remain separately documented by the SBF program.

## Remaining promotion gates

The following must close before native settlement is described as generally
available:

1. close every final-LTO stack-overwrite survivor reported by the artifact
   audit, then rerun that audit against the final ELF;
2. audit every other post-resolution instruction so none reconstructs smooth
   state as `FinitePreset` index zero;
3. rebuild the committed ELF, update its reproducibility manifest, and run the
   full signed 22-step lifecycle plus native scenarios from a clean tree.

Until those gates close, the accurate claim is: permissionless production
construction, source-authenticated native degree-one through degree-three point
and occupation resolution, and exact record-only internal plus positionless
bearer redemption are implemented with an explicit v2/v3/v4 split and execute in the real SBF program;
the shared program artifact is not promoted while its remaining final-LTO
stack diagnostics and whole-lifecycle integration gate remain open.
