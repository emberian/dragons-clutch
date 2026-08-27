# Recorded-resolution-only internal redemption

Status: **IMPLEMENTED AND EXECUTED FOR V2/V3/V4; ARTIFACT PROMOTION GATES
REMAIN** (2026-08-19).

`RedeemInternal` no longer accepts a live Feed head or a caller-owned evidence
buffer after a market is resolved. There is one canonical account ABI and one
post-resolution payout authority. Resolution authenticates the source plane
once and persists the result; redemption consumes that immutable result.

## Canonical account plane

The fixed prefix is 16 accounts, followed by every canonical outcome mint:

| index | role | access |
|---:|---|---|
| 0 | owner actor | signer, read-only |
| 1 | Market | writable |
| 2 | Hoard | writable |
| 3 | owner Position | writable |
| 4 | kernel aggregate | writable |
| 5 | generation Replay | writable |
| 6 | market SupplyLedger | writable |
| 7 | immutable Terms | read-only |
| 8 | immutable Resolution v2/v3/v4 | read-only |
| 9 | immutable Profile | read-only |
| 10 | pinned Token-2022 program | read-only, executable |
| 11 | immutable collateral policy | read-only |
| 12 | admitted collateral mint | read-only |
| 13 | owner's collateral token account | writable |
| 14 | Hoard signing authority PDA | read-only |
| 15 | Hoard collateral token account | writable |
| 16.. | complete ordered outcome-mint vector | read-only |

The retired 18-account state/custody prefix inserted Feed and evidence at
indices 9 and 10. Exact account-count validation refuses that list; it is not
a compatibility path and extra accounts are not silently ignored.

The wire `Action::RedeemInternal { outcome, quantity }` is unchanged. This is
one canonical replacement of its account plane, not a second action that could
leave two payout authorities live.

## Version-selected authority

Immutable Terms select exactly one Resolution codec:

| Terms mode | record | bytes | persisted payout authority | rent exemption |
|---|---|---:|---|---:|
| degree 0 | v2 `ResolutionAccount` | 165 | categorical payout index | 2,039,280 lamports |
| degree 1–3 point statistic | v3 `NativeResolutionAccount` | 319 | exact full native vector | 3,111,120 lamports |
| degree 1–3 occupation statistic | v4 `OccupationResolutionAccount` | 383 | exact full native vector plus sealed archive provenance | 3,556,560 lamports |

For every version, the record must bind the exact Market, immutable Terms,
Feed identity, stored PDA bump, generation, domain end, and canonical window.
The window identity is recomputed from immutable market Terms; it is not
supplied by the redeemer. V3 requires derived-point mode. V4 requires its named
occupation statistic and finalizer. The kernel must already be resolved in the
corresponding finite-preset or derived-basis mode.

V3 and v4 vectors are reconstructed into an ephemeral `MarketState` through
the shared `bound_native_resolution` and `reconstruct_native_market` seams.
They are never copied into a second persisted vector. V2 uses the recorded
index. None of the three modes re-folds observations, uses a midpoint, searches
native vectors through presets, or materializes claims after resolution as a
redemption workaround.

## Transition and refusal order

The owner transition retains the reference order:

1. hostile decode;
2. stored-bump and cross-account links;
3. inactive-slot canonicality;
4. pre-state aggregate closure;
5. exact owner replay sequence;
6. kernel/mode/vector invariants;
7. immutable Terms and Resolution presentation;
8. owner signature and Position ownership;
9. Terms/payout-set/Resolution/window binding;
10. exact kernel redemption;
11. exact Hoard and SupplyLedger delta;
12. post-state aggregate closure; and
13. byte-exact writes.

Account count, aliasing, owners, lengths, mutability, PDAs, Profile/policy,
Token-2022 custody, and the full mint vector are authenticated before the
semantic transition. Replay remains step 5; removing caller evidence did not
move it earlier or create an alternative retry rule.

Internal redemption transfers no Token-2022 atoms. It reclassifies locked
Hoard backing as owner Position cash. The adapter snapshots both collateral
accounts, requires exact zero token deltas afterward, and proves that pooled
custody still covers recorded collateral. Unsolicited donations remain
conservative surplus and cannot create a Position credit.

For fractional native weights, `quantity * weight` must divide the denominator
exactly. A sub-lot returns `RemainderRequired` before replay or claims are
consumed. There is no numerator-credit runtime and no rounding boundary in
redemption.

## Executed real-SBF evidence

The real ELF is loaded by `solana-program-test` with real Token-2022. Focused
tests are in:

- `programs/clutch-sbf/svm-tests/tests/collateral_leg.rs` for categorical v2;
- `programs/clutch-sbf/svm-tests/tests/native_resolution.rs` for point v3 and
  occupation v4; and
- `programs/clutch-sbf/svm-tests/tests/native_full_lifecycle.rs` for public
  blank-bank construction through zero-Hoard terminal state.

Measured successful internal redemption:

| path | exact quantity | compute units |
|---|---:|---:|
| categorical v2 winning/losing | 6 / 4 | 604,877 / 604,877 |
| point v3 degree 1 | 2 | 778,952 |
| point v3 degree 2 | 4 | 776,495 |
| point v3 degree 3 | 8 | 776,385 |
| occupation v4 degree 1 | 2 | 774,666 |
| occupation v4 degree 2 | 4 | 778,209 |
| occupation v4 degree 3 | 8 | 776,599 |

The joined blank-bank point lifecycle independently measured its four internal
redemptions per degree at 758,487–768,444 CU. Its genesis retains the source
accounts required by the earlier Resolve, but it creates no separate empty
redemption evidence buffer and presents none at redemption.

The final focused rerun in this lane used joined provisional ELF
`b5b740b193af09a1f1a5a28c1cde59688ae69d11b63eec8fceb1066ce05e0649`
(842,704 bytes). Concurrent source/native-window inputs were still uncommitted,
so this digest identifies executed evidence only and is not a clean-tree
release or liveness identity.

Adversarial real-SBF coverage includes:

- the retired Feed/buffer-expanded account list and incomplete mint vectors;
- immutable-account mutability and account aliases;
- wrong record mode, Terms binding, and canonical window;
- categorical, point, and occupation sub-lot/refusal rollback;
- byte-identical resolution records across redemption; and
- a two-instruction transaction where the first redemption succeeds and the
  second uses a stale replay sequence, proving the late refusal rolls the
  first instruction back too.

## Promotion boundary

This implementation is evidence for the account and semantic plane, not a
mainnet release claim. Promotion still requires a clean-tree artifact build,
the final-LTO stack survivor audit, a pinned ELF digest and toolchain/source
manifest, and the full signed committed lifecycle against that exact ELF. The
SBF build log must remain free of diagnostics naming `recorded_redeem`,
`apply_recorded_redemption`, `bound_native_resolution`, or
`reconstruct_native_market`; program-wide survivors elsewhere remain release
STOPs until the artifact audit closes them.
