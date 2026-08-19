# Native resolution persistence

Status: **PRE-DEPLOYMENT ABI CUT / ISOLATED CODEC** (2026-08-19).

This design closes the last reference-only half of native B-spline settlement:
an arbitrary derived vector can be computed, but the live version-two
`ResolutionAccount` can persist only an index into at most eight terms
presets. The version-three codec is isolated in
`programs/solana-layout/src/native_resolution.rs`; it does not become the live
ABI until the shared layout and SBF integration steps below land together.

## Decision

The immutable Resolution account is the sole persisted owner of a derived
payout vector. A Kernel account never stores a second copy.

There are exactly three canonical modes:

| mode | terms basis | account-owned resolution payload |
|---|---:|---|
| `UNRESOLVED` | any admitted basis | none; all resolution fields are zero |
| `PRESET` | degree 0 | `payout_index`; terms own the selected vector |
| `DERIVED_POINT` | degree 1–3 | raw integer statistic point, outcome count, denominator, weights |

The mode is not author discretion. Once terms are joined, degree zero implies
`PRESET` and degrees one through three imply `DERIVED_POINT`. This removes the
old derived-to-preset membership bridge from the program ABI. A degree-one
vector that happens to equal a preset is still persisted as a native derived
vector, so there is one rule for every smooth degree.

`resolved_value` is the exact raw integer statistic point before the frozen
edge policy is applied. Storing the raw point preserves the evidence fact;
replaying the evaluator applies the same terms-owned clamp/refuse policy. The
resolution instruction must require point evidence for every derived degree
in this first persisted ABI. The pure reference's safe degree-one
equal-quantized-interval convenience is therefore deliberately narrower on
chain: it cannot be represented as a point without inventing one.

The record still binds the market, terms digest, feed, sealed-window digest,
feed cursor, exact window end, repair generation, and recording slot. The
window digest is not self-authenticating. Creation must remain downstream of
the source/archive authentication and exact `WindowResult` fold.

## Version-three bytes

Tag 16 is unchanged; version advances from 2 to 3 because the byte shape
changes. Version-two bytes must refuse rather than be interpreted under the
new semantics. This is pre-deployment, so the preferred path is a clean ABI
replacement, not an onchain migration.

| offset | bytes | field |
|---:|---:|---|
| 0 | 1 | Resolution tag (`16`) |
| 1 | 1 | account version (`3`) |
| 2 | 32 | market |
| 34 | 32 | terms digest |
| 66 | 32 | feed |
| 98 | 32 | sealed-window digest |
| 130 | 8 | feed cursor |
| 138 | 8 | sealed end bucket, exclusive |
| 146 | 8 | repair generation |
| 154 | 8 | resolved slot |
| 162 | 1 | mode |
| 163 | 1 | payout index |
| 164 | 1 | active outcome count |
| 165 | 16 | raw resolved value |
| 181 | 8 | vector denominator |
| 189 | 128 | sixteen `u64` weights |
| 317 | 1 | stored PDA bump |
| 318 | 1 | reserved flags |

Exact wire length: **319 bytes**, up from 165 bytes (`+154`). No Rust struct
layout is a wire fact.

Canonical padding is mode-specific:

- unresolved: window and all temporal facts zero; payout index is `0xff`;
  outcome count, value, denominator, and weights zero;
- preset: nonzero window and valid temporal header; payout index `< 8`;
  outcome count, value, denominator, and weights zero; and
- derived point: nonzero window and valid temporal header; payout index is
  `0xff`; outcome count is `2..=16`; weights beyond it are zero; denominator is
  nonzero; every active weight is at most the denominator; active weights sum
  exactly to the denominator.

The codec also requires `feed_cursor >= sealed_end_bucket_exclusive`. Its
terms join strengthens that to the frozen maturity boundary, requires exact
window end and repair generation, and checks mode, outcome count, and common
denominator against the terms. Its market join checks market/terms/feed
identities and active/resolved lifecycle agreement.

## Resolution transition

The atomic resolution instruction should perform this order:

1. authenticate the Market, Terms, Feed/source archive, Resolution PDA, and
   exact sealed-window artifact;
2. fully validate terms once and bind the exact source/evaluator/window
   domain;
3. derive the registered statistic conservatively;
4. for degree zero, select the payout-map member and construct `PRESET`;
5. for degree one through three, require `low == high` before edge handling,
   evaluate `clutch-bspline` at that raw point, validate the exact returned
   vector, and construct `DERIVED_POINT`;
6. encode and re-decode the candidate version-three record or otherwise run
   the same validation before mutation;
7. update Market lifecycle and any minimal kernel phase/index cache only in
   the same successful transaction; and
8. on repeat, derive the full expected record and require byte equality.

No midpoint, endpoint choice, vector approximation, or preset search is
allowed. A derived TWAP still refuses. Any mismatch leaves every account
byte-identical.

The point/vector redundancy is intentional evidence for replay, not two
authorities. At creation, the exact evaluator must prove they agree. On later
reads, program ownership and immutability after resolution make the stored
vector authoritative; audit/replay can recompute it from the point and terms.
An upgrade or instruction capable of editing a resolved record would break
that argument and must not exist.

## Kernel reconstruction and redemption

The persisted kernel projection may retain its current phase byte and, for a
degree-zero preset, the selected payout index. In derived mode its index is
canonical zero and it stores no vector. Reconstruction joins immutable terms
and the Resolution record:

```text
basis mode       = degree(terms) == 0 ? FinitePreset : DerivedBasis
effective vector = PRESET        ? terms.payouts[payout_index]
                 : DERIVED_POINT ? resolution.vector
                 :                none
```

The effective vector is copied only into an ephemeral `MarketState` used for
the checked transition and discarded after encoding post-state. Every
post-resolution path that needs payout semantics—including internal and
external bearer redemption—must present the Resolution account. It must not
fall back to `KernelAccount.resolved_payout` for a derived market.

Existing kernel redemption remains exact: `quantity * weight / denominator`
must have zero remainder or return `RemainderRequired`. Complete-set exit
remains exact because the persisted weights sum to the denominator. At knots
or clamped endpoints a derived vector may be one-hot; it takes the same exact
redemption path without being relabeled as a categorical preset.

## Hostile-byte and binding surface

The isolated codec tests pin:

- exact round trips for all three modes;
- short, long, mistagged, and version-two input refusals;
- canonical unresolved and preset padding;
- derived index sentinel, count, zero padding, denominator, per-weight bound,
  and exact-sum checks;
- nonzero resolved window, nonzero end, and cursor ordering; and
- zero as a valid derived statistic point.

Integration tests must additionally flip every market/terms/feed/window
binding, degree/mode pair, outcome count, denominator, end bucket, generation,
maturity cursor, point, and weight. They must test transaction rollback after
the evaluator but before each write, exact idempotent replay, one-hot derived
vectors, fractional remainder refusal, complete-set exit, direct bearer
redemption, and source-record substitution.

## Size, rent, compute, and frame impact

- Wire data grows from 165 to 319 bytes. It remains tiny relative to Solana's
  ordinary account limits and fits one System Program create-account CPI.
- Under the conventional `Rent::default` constants (128 bytes of storage
  overhead, 3,480 lamports per byte-year, 2-year exemption), the indicative
  exemption moves from `2,039,280` to `3,111,120` lamports: a refundable delta
  of `1,071,840` lamports (`0.00107184 SOL`). The program must query the actual
  runtime Rent sysvar; these numbers are not a cluster promise.
- Each decoded record adds 154 wire bytes and one 136-byte vector copy. The
  codec supplies `decode_into` so an SBF caller with caller-owned storage does
  not create a second account-sized return temporary.
- A 319-byte wire account is not itself a 4 KiB frame risk. The joined
  instruction already handles the roughly 1.6 KiB Terms account and kernel
  state, so helpers must remain `inline(never)` at the same measured frame
  boundaries and avoid placing duplicate Terms/Resolution values together.
- Resolution adds an exact degree-two/three rational evaluator and a 128-byte
  record write. No compute-unit claim is made until the pinned SBF ELF runs in
  the local committed bank. Redemption should not rerun the spline evaluator;
  it structurally validates and uses the immutable persisted vector.

## Exact integration checklist

The live cut must be one coherent commit series:

1. export `native_resolution`; set `account_version::RESOLUTION = 3` and
   `account_len::RESOLUTION = NATIVE_RESOLUTION_LEN`; replace/re-export the
   old `ResolutionAccount` name with `NativeResolutionAccount`; remove the old
   version-two implementation and length truth;
2. update golden lengths and every fixture; assert version 2 refuses;
3. add the point-and-vector reference seam, requiring point evidence for all
   derived persisted resolutions;
4. reconstruct kernel basis mode from terms and derived vector from the
   Resolution account, never from a persisted kernel copy;
5. update market creation to allocate and initialize exactly 319 bytes;
6. update resolve, internal redemption, external redemption, idempotency, and
   evidence verification to the new mode discipline;
7. add SBF rollback/differential tests and measure instruction CU, stack
   frames, account creation, and rent in the committed local bank; and
8. regenerate the audited ELF and manifest only after all refusal and
   lifecycle gates pass.

Until that list lands, the live SBF remains finite-preset only and must refuse
native derived resolution honestly.
