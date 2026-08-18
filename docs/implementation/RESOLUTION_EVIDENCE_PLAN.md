# Resolution evidence plane

> **Update 2026-08-19:** the evidence gate specified here has LANDED in
> `programs/solana-reference` (`apply_with_evidence`): resolution is now
> evidence-gated, with the evidence-absent path still returning
> `ResolutionEvidenceUnavailable`. V1 pins the ordinal partition; a threshold
> boundary table is inexpressible in the frozen TermsAccount — there is no
> field to carry one, so a threshold market cannot resolve here, and every
> non-pinned registry value that the terms *can* carry is refused
> (obligation 18 - needs a TermsAccount revision).
> Status lines below describing an unconditional refusal or "not implemented
> anywhere" describe the pre-landing state; §2.1's digest claim is superseded
> by the landed TermsAccount.

Status: MODEL/PROPOSED offline typed-evidence plane (2026-08-18). This document
and the code it describes are an offline research prototype. Nothing here is
authenticated, deployed, proved, or chain-evidence. **No adapter refusal is
relaxed by this wave.** `programs/solana-reference` still returns
`ResolutionEvidenceUnavailable` unconditionally for both `Action::Resolve` and
`Action::RedeemInternal`, and that crate was not modified.

The purpose of this wave is narrower and deliberately unglamorous: build the
*shape* of the evidence a future adapter would need, so that when resolution is
eventually reintroduced there is a typed object to bind it to rather than a
signer and a hope. `docs/implementation/SOLANA_REFERENCE_ADAPTER.md` obligation
8 names that object; §P1-D of `docs/implementation/ADVERSARIAL_REVIEW_V0.md`
names the composition hole that made it necessary; the "prefix resolution before
maturity/sealing" repair records the promotion gate this plan is written
against.

Three things landed:

1. a typed, domain-bound `WindowResult` in `crates/clutch-accumulator`
   (§1) — implemented, tested, and gated;
2. a terms-to-payout derivation specification (§2) — **specification only**,
   precise enough for the next wave to implement in the reference adapter; and
3. a decided parent/child collateral-digest relation with a Python
   implementation and cross-language golden vectors (§3) — the P1-G join.

Claim vocabulary throughout: MODEL or PROPOSED. "The evidence plane exists"
means the types and refusals exist and are tested offline. It does not mean any
observation, feed, source, market, or digest is authenticated by anything.

---

## 1. The window evidence plane (landed)

### 1.1 The hole this closes

`Summary` is an associative interval-summary monoid. It answers `terminal`,
`twap`, `sampled_min`, `sampled_max`, and `relative_terminal_to_twap` for
*whatever bucket range it happens to cover*, complete or gapped. That is the
correct behavior for an accumulator and the wrong behavior for a settlement
term. §P1-D: a caller can accidentally treat an accepted-only statistic as a
full-window one, because the statistic API binds no coverage policy and no
expected range.

The fix is typed, not documentary. A settlement-facing function names
`&WindowResult`. There is no public constructor from a `Summary` to a
`WindowResult`, so the substitution is a compile error:

```rust
fn payout_index(_evidence: &WindowResult) -> u8 { 0 }

let summary = Summary::empty(grid);
let _ = payout_index(&summary);   // does not compile
```

That exact snippet is checked as a `compile_fail` doctest in the crate. The
remaining substitutions (a truncated prefix, a gap-tolerant variant, another
window, another generation, another maturity bound, a reopened result) are not
type errors — they are values of the same type with different domains — so each
is an explicit named refusal with a test.

### 1.2 API surface added

All of it is `no_std`, dependency-free, allocator-free, checked, and re-exported
from the crate root. It uses only the existing public `Summary` API, so the
window plane holds no privileged access to summary internals.

| Item | Kind | Role |
| --- | --- | --- |
| `IDENTITY_BYTES` | `usize` = 32 | width of one opaque adapter identity |
| `FeedIdentity` | struct | `source_adapter_id`, `feed_spec_id`, `source_version`, `evaluator_version`; refuses zero identities and zero versions |
| `CoveragePolicy` | struct (private fields) | the crate's closed policy registry |
| `COVERAGE_POLICY_COMPLETE_REQUIRED` | `u16` = 1 | registered id |
| `COVERAGE_POLICY_BOUNDED_GAPS` | `u16` = 2 | registered id |
| `CoveragePolicy::COMPLETE_REQUIRED` | const | every expected bucket accepted |
| `CoveragePolicy::bounded_gaps(n)` | ctor | at most `n` explicit gaps; `n = 0` refused |
| `CoveragePolicy::from_registry(id, param)` | ctor | rebuild from decoded bytes; unregistered ids refuse |
| `WindowDomain` | struct | feed + grid + `[start, end)` + maturity bound + generation + coverage policy |
| `WindowDomain::encode_canonical` | fn | exactly 144 canonical preimage bytes |
| `WINDOW_DOMAIN_TAG` | `&[u8]` | domain-separation string for a hashing adapter |
| `WindowPhase` | enum | `Open`, `Mature`, `Sealed` |
| `WindowAccumulator` | struct | folds pages into exactly one domain; runs the state machine |
| `WindowResult` | struct | the only domain-bound settlement-facing value |
| `WindowResult::check_domain` | fn | refuse unless bound to exactly the expected domain |
| `WindowError` | enum | 23 named refusals |

`WindowResult` exposes the same closed statistic evaluators as `Summary`
(`terminal`, `price_time_integral`, `twap`, `sampled_min`, `sampled_max`,
`relative_terminal_to_twap`) and the same three conservative refusals
(`threshold_crossings`, `maximum_drawdown`, `realized_variance`). It also
exposes `unbound_summary()` for diagnostics and independent recomputation; the
name is a warning, and no settlement, payout, or resolution API may accept that
type.

### 1.3 State machine

This generalizes the vertical model's host semantics. That model froze one
`MATURITY_BUCKETS = 3` constant and one `sealed: bool` for one market, with
`observe`/`seal_observations`/`resolve_from_summary` refusing `NotMature`,
`AlreadySealed`, `ObservationAfterSeal`, and `NotSealed`. Here the horizon is a
per-window field of an immutable domain, the cursor is an explicitly witnessed
feed fact rather than an implicit consequence of counting, and sealing is
terminal.

```text
                 absorb(page) / observe(bucket)      witness_feed_cursor(b)
                 (contiguous, in range, in grid)      (monotone, refuses backwards)
                            |                                   |
                            v                                   v
   open(domain) ------> [ Open ] ------------------------> [ Mature ] --seal()--> [ Sealed ]
                            ^                                   |                     |
                            |                                   |                     | result()
      cursor != end  OR  feed_cursor < maturity_bucket_exclusive |                     v
                                                                              [ WindowResult ]
                                                                        (only if the registered
                                                                         coverage policy admits)
```

- `Mature` requires **both** that every expected bucket is represented (accepted
  or explicitly missing) and that the witnessed feed cursor has reached
  `maturity_bucket_exclusive`. The two are different facts: the first says the
  window is *covered*, the second says the feed's repair interval for it has
  *closed*. `maturity_bucket_exclusive >= end_bucket_exclusive`; the excess is
  the frozen repair grace.
- `seal()` is terminal. After it, `absorb`, `observe`, and
  `witness_feed_cursor` all return `ObservationAfterSeal`, and a second `seal()`
  returns `AlreadySealed`. There is no reopen transition and no API that
  produces one.
- `result()` is the only constructor of a `WindowResult`. It re-checks that the
  fold spans exactly the expected range and then applies the registered coverage
  policy. A `CompleteRequired` window with one explicit gap returns
  `CoverageRefused` even though the underlying bare summary would happily answer
  `terminal`.

### 1.4 Refusal inventory

| Refusal | Raised when |
| --- | --- |
| `ZeroIdentity`, `UnversionedIdentity` | a 32-byte identity is zero, or a source/evaluator version is zero |
| `InvalidRange`, `InvalidMaturity` | empty/reversed/oversized range, or a maturity bound before the window end |
| `UnknownCoveragePolicy`, `InvalidPolicyParameter` | unregistered policy id, or a parameter outside its registered domain |
| `MismatchedGrid` | a page or expectation uses a different semantic grid |
| `NonContiguous`, `RangeOverflow` | a page does not begin at the cursor, or would extend past the window end |
| `NonMonotoneCursor` | a witnessed feed cursor moved backwards |
| `IncompleteDomain` | seal attempted over a truncated prefix |
| `NotMature` | seal attempted before the maturity bound |
| `AlreadySealed`, `ObservationAfterSeal`, `NotSealed` | seal/reopen/premature-read attempts |
| `CoverageRefused` | the registered coverage policy refuses the observed coverage |
| `MismatchedFeed`, `WrongWindow`, `MismatchedMaturity`, `MismatchedGeneration`, `MismatchedCoveragePolicy` | `check_domain` against a different expected domain; the error names the field |
| `MalformedResult` | a sealed fold failed its own consistency re-check |
| `Summary(_)` | the underlying summary algebra refused |

### 1.5 Canonical domain bytes

`WindowDomain::encode_canonical` writes exactly 144 bytes. The accumulator owns
no hash primitive and computes no identity, on purpose: it publishes the tag and
the exact preimage so a hashing adapter and an independent recomputation cannot
disagree about either.

```text
WindowId := HASH( WINDOW_DOMAIN_TAG || encode_canonical() )
            where WINDOW_DOMAIN_TAG = "dragons-clutch/window-domain/v1"
```

| Offset | Bytes | Field |
| ---: | ---: | --- |
| 0 | 8 | magic, ASCII `DCWINR1` followed by one zero byte |
| 8 | 2 | schema version, little-endian `u16` (`1`) |
| 10 | 2 | coverage policy id, little-endian `u16` |
| 12 | 8 | coverage policy parameter, little-endian `u64` |
| 20 | 32 | `source_adapter_id` |
| 52 | 32 | `feed_spec_id` |
| 84 | 4 | `source_version`, little-endian `u32` |
| 88 | 4 | `evaluator_version`, little-endian `u32` |
| 92 | 4 | grid `family_id`, little-endian `u32` |
| 96 | 2 | grid `version`, little-endian `u16` |
| 98 | 8 | grid `bucket_seconds`, little-endian `u64` |
| 106 | 8 | `start_bucket`, little-endian `u64` |
| 114 | 8 | `end_bucket_exclusive`, little-endian `u64` |
| 122 | 8 | `maturity_bucket_exclusive`, little-endian `u64` |
| 130 | 8 | `generation`, little-endian `u64` |
| 138 | 6 | zero reserved bytes |

The selected hash primitive is not decided here. `programs/solana-layout`
already carries a dependency-free SHA-256 for canonical IDs and correctly states
that no deployment has selected that primitive until a profile says so; the same
caveat applies to `WindowId`.

### 1.6 What this does *not* establish

- Nothing authenticates the observations that were folded. `Observation` values
  still arrive from a caller. A `WindowResult` is honest evidence about a fold,
  never evidence about a source.
- `FeedIdentity`'s two 32-byte values are opaque. The crate cannot check that
  they correspond to a real adapter, program, deployment, subject, quote, or
  orientation. That is the source-adapter admission dossier of
  `docs/ACCUMULATOR_PLAN.md` §9, which remains unstarted.
- The maturity bound is a bucket index, not a clock. Nothing here relates a
  bucket to a slot, a timestamp, or wall-clock time.
- No proof obligation from `docs/ACCUMULATOR_PLAN.md` §10 is discharged. The
  window plane is tested, not verified.

---

## 2. Terms-to-payout derivation (specification only)

**Not implemented anywhere.** This section specifies the typed function the next
wave should implement in the reference adapter, behind the still-unconditional
refusal, so it can be reviewed before it can be reached.

```text
derive_payout :
    (ResolutionTerms, WindowResult) -> Result<PayoutIndex, ResolutionRefusal>
```

Both inputs are immutable. `ResolutionTerms` is frozen at market creation and
digested into `MarketAccount.terms`; `WindowResult` is the §1 type. The function
is total, allocation-free, and performs at most 15 comparisons plus one bounded
statistic evaluation. It reads no clock, no signer, and no account.

### 2.1 `ResolutionTerms` (PROPOSED)

```text
ResolutionTerms {
    market_id:            [u8; 32]      // == MarketAccount.market
    window:               WindowDomain  // the exact expected domain, §1.5
    statistic:            StatisticId   // closed enum, §2.3
    cell_count:           u8            // n, in 2..=MAX_OUTCOMES
    boundaries:           [u128; 15]    // b_0 .. b_{n-2}; entries >= n-1 are zero
    payout_map:           [u8; 16]      // cell i -> PayoutSet index; entries >= n are 0xFF
    ambiguity_policy:     AmbiguityPolicyId
    failure_policy:       FailurePolicyId
    generation_policy:    GenerationPolicyId
}
```

Canonical bytes, 432 total, little-endian throughout:

| Offset | Bytes | Field |
| ---: | ---: | --- |
| 0 | 8 | magic, ASCII `DCTERM1` followed by one zero byte |
| 8 | 2 | schema version `u16` (`1`) |
| 10 | 2 | flags `u16` (zero in V1) |
| 12 | 2 | statistic id `u16` |
| 14 | 1 | cell count `n` |
| 15 | 1 | ambiguity policy id `u8` |
| 16 | 1 | failure policy id `u8` |
| 17 | 1 | generation policy id `u8` |
| 18 | 2 | zero reserved |
| 20 | 144 | `WindowDomain` canonical bytes (§1.5) |
| 164 | 240 | boundary table, 15 × `u128`; entries at index `>= n-1` must be zero |
| 404 | 16 | payout map, 16 × `u8`; entries at index `>= n` must be `0xFF` |
| 420 | 12 | zero reserved |

```text
TermsDigest := HASH( "dragons-clutch/market-terms/v1" || terms_bytes[432] )
```

and the adapter must require `TermsDigest == MarketAccount.terms`. Decoding
requires the exact length, known magic/schema/flags, a registered statistic and
policy triple, a well-formed partition (§2.2), a payout map whose live entries
are all `< PayoutSet.count`, zero padding, and byte-for-byte re-encoding. The
`WindowDomain` subrange must decode through `CoveragePolicy::from_registry`, so
an unregistered coverage policy fails at terms decode rather than at resolution.

### 2.2 Partition and exhaustive cell selection

The boundary table induces the ordered half-open family over the admitted value
domain `[0, MAX_VALUE]`:

```text
C_0     = [0,       b_0)
C_i     = [b_{i-1}, b_i)        for 1 <= i <= n-2
C_{n-1} = [b_{n-2}, MAX_VALUE]     (closed at the top)
```

Well-formedness, checked at decode and re-checked before use:

```text
2 <= n <= MAX_OUTCOMES
0 < b_0 < b_1 < ... < b_{n-2} <= MAX_VALUE
```

Strict increase gives disjointness and ordering; `0 < b_0` and the closed top
cell give exhaustiveness with every cell non-empty. An empty cell is refused
because it would mint a liability that can never pay. This is the executable
form of `docs/PARTITION_ALGEBRA.md` §1 and of the AGENTS.md rule that a
partition must be exhaustive, disjoint, ordered, and canonical before it can
mint liabilities.

Selection over a conservative interval `[lo, hi]` produced by the statistic:

```text
cell_of(v) := the unique i with v in C_i          // linear scan, at most 15 compares
i := cell_of(lo)
j := cell_of(hi)
if i == j  -> Selected(i)
else       -> Ambiguous(first = i, last = j)
```

`Selected(i)` yields `payout_map[i]`, which must be `< PayoutSet.count` or the
call refuses `R-09`. Note the asymmetry in the kernel bounds: `MAX_OUTCOMES` is
16 but `MAX_PAYOUTS` is 8, so a 16-cell partition necessarily maps several cells
onto one payout vector. That is legitimate — distinct states can pay the same
thing — but it means `payout_map` is not injective and must never be inverted to
recover a cell. One-hot resolution therefore succeeds only when the entire
conservative interval lies inside one cell — exactly the rule
`docs/ACCUMULATOR_PLAN.md` §7 states, and the rule the vertical model's
`AmbiguousTerminalInterval` refusal already implements for the binary case.

### 2.3 Statistic identifiers and comparison rules

| Id | Name | Value shape | Boundary comparison |
| ---: | --- | --- | --- |
| 1 | `STAT-TERMINAL-01` | `ValueInterval` in normalized atoms | direct `u128` compare |
| 2 | `STAT-SAMPLED-MIN-02` | `ValueInterval` | direct `u128` compare |
| 3 | `STAT-SAMPLED-MAX-03` | `ValueInterval` | direct `u128` compare |
| 4 | `STAT-TWAP-04` | `RatioInterval`, common denominator `D > 0` | compare `numerator` against `boundary * D`, checked |
| 5 | `STAT-RELATIVE-TERMINAL-TWAP-05` | `FractionInterval`, two denominators | **not admitted in V1**, see below |

The `STAT-TWAP-04` product is bounded: `boundary <= MAX_VALUE = 10^24` and
`D = covered_duration <= MAX_BUCKETS * MAX_BUCKET_SECONDS = 8.64 x 10^10`, so
`boundary * D <= 8.64 x 10^34 < 2^128`. The multiplication is still written
checked, and overflow still refuses `R-11`; the bound is why the refusal is
unreachable for admitted terms, not a reason to drop the check.

`STAT-RELATIVE-TERMINAL-TWAP-05` is **deferred**. Its comparison needs
`low_numerator * scale` against `boundary * low_denominator`, where
`low_numerator` and `low_denominator` are already as large as
`MAX_VALUE * covered_duration ~ 8.64 x 10^34`. With `u128` that leaves a headroom
factor of only about `3.9 x 10^3`, so admitting it requires either a checked
256-bit comparison, a narrowed `MAX_VALUE`, or a frozen scale small enough to
prove the bound. Pick one and write the proof before registering it; do not
register it with an unchecked or a wrapping compare.

Any statistic whose required information the summary family discarded stays
refused (`threshold_crossings`, `maximum_drawdown`, `realized_variance`). A
registered threshold automaton or a new versioned feature family is the only way
in, per `docs/ACCUMULATOR_PLAN.md` §5.

### 2.4 Named policy identifiers

**Ambiguity policies** — what happens when the conservative interval straddles
two or more cells.

| Id | Name | V1 | Behavior |
| ---: | --- | --- | --- |
| 1 | `AMBIG-REFUSE-01` | **default** | refuse `R-06`; control passes to the failure policy. Deterministic, no discretion, no invented precision. |
| 2 | `AMBIG-COMPATIBLE-SET-02` | no | return the compatible cell range `[i, j]`. Blocked: the kernel has no representation for a compatible set (its `PayoutVector` weights are exact fractions summing to the denominator, which is a different object), and it inherits P1-A. |
| — | `AMBIG-MIDPOINT`, `AMBIG-CONSERVATIVE-LOW`, `AMBIG-CONSERVATIVE-HIGH` | **forbidden** | never registered. Each converts uncertainty into a definite claim with no evidence. Named here only so a reviewer who finds one in a diff recognizes it as a violation rather than an unregistered idea. |

**Failure policies** — what the Market does once resolution refuses.

| Id | Name | V1 | Behavior |
| ---: | --- | --- | --- |
| 1 | `FAIL-UNIFORM-REFUND-01` | candidate | resolve to the uniform vector `PayoutVector { denominator: n, weights: [1; n] }`. Kernel-expressible and exactly collateral-conserving. **Precondition:** the vector must already be a member of the immutable `PayoutSet` frozen at market creation — a failure payout is never invented at resolution time. **Inherits P1-A:** fractional payouts may leave unredeemable remainders until the redemption-lots decision lands. |
| 2 | `FAIL-EXTENDED-WINDOW-02` | proposed | terms name exactly one successor `WindowDomain` with the same feed and a strictly later maturity bound, and a bounded extension count (candidate: exactly one). Every successor is named in the immutable terms; nothing is chosen at runtime. |
| — | `FAIL-HOLD` | **forbidden** | leaving the Market active pending a discretionary authority reintroduces exactly the seizure/discretion surface the protocol refuses. |

**Generation policies** — how terms bind the repair generation.

| Id | Name | V1 | Behavior |
| ---: | --- | --- | --- |
| 1 | `GEN-EXACT-01` | **default** | terms pin the exact generation inside the `WindowDomain`. Any repair that changes it makes the pinned window unresolvable and the Market enters its failure policy. Conservative and fully deterministic; the cost is that a single repair voids the Market. |
| 2 | `GEN-FINAL-AT-MATURITY-02` | proposed | terms pin the maturity bound and a repair-policy id instead of a generation number, and the adapter substitutes the feed's final generation as of that bound. **Blocked** on a sealed feed-epoch object that can prove "no further repair is possible after maturity". That object does not exist. |

### 2.5 Refusal classes

Every path out of `derive_payout` other than `Ok(PayoutIndex)` is one of these.
They are distinguishable on purpose: "the window was wrong" and "the interval
was ambiguous" have different operational responses.

| Id | Class | Source |
| ---: | --- | --- |
| R-01 | `TermsDigestMismatch` | recomputed terms digest != `MarketAccount.terms` |
| R-02 | `TermsMalformed` | length, magic, schema, padding, unregistered policy/statistic id |
| R-03 | `PartitionMalformed` | `n` out of range, non-increasing or zero boundaries, empty cell |
| R-04 | `WindowDomainMismatch` | `WindowResult::check_domain` refused; carries the accumulator's field-level reason (`MismatchedFeed`, `WrongWindow`, `MismatchedMaturity`, `MismatchedGeneration`, `MismatchedCoveragePolicy`, `MismatchedGrid`) |
| R-05 | `StatisticUnsupported` | the statistic id is registered but not admitted for this feature family, or is a discarded-information predicate |
| R-06 | `AmbiguousInterval` | the conservative interval straddles cells and `AMBIG-REFUSE-01` applies |
| R-07 | `NoAcceptedCoverage` | the sealed window carries no accepted bucket (reachable under a bounded-gap policy) |
| R-08 | `AmbiguousDenominator` | a ratio statistic whose denominator interval includes zero |
| R-09 | `PayoutIndexOutOfRange` | `payout_map[i] >= PayoutSet.count` |
| R-10 | `MarketNotActive` | the kernel Market is not in `Phase::Active` |
| R-11 | `ArithmeticOverflow` | a checked comparison product overflowed |

`NotSealed`, `NotMature`, and `IncompleteDomain` cannot appear here: a
`WindowResult` cannot exist in those states. They surface earlier, in whatever
decodes evidence into a `WindowResult`, and that decoder is itself unwritten.

### 2.6 Worked example

Binary market, threshold 50, three-bucket window, `CompleteRequired`, one-hot
payouts. This is the vertical model's existing case re-expressed in the typed
form, and it is a useful first differential fixture for the next wave.

```text
n = 2, boundaries = [50], payout_map = [0, 1]
C_0 = [0, 50)   -> payout 0
C_1 = [50, MAX] -> payout 1

terminal = [47, 49] -> cell_of(47) = 0, cell_of(49) = 0 -> Selected(0) -> payout 0
terminal = [50, 50] -> cell_of(50) = 1, cell_of(50) = 1 -> Selected(1) -> payout 1
terminal = [49, 51] -> cell_of(49) = 0, cell_of(51) = 1 -> Ambiguous  -> R-06
```

The vertical model's `resolve_from_summary` already produces exactly these three
outcomes (`Resolved(0)`, `Resolved(1)`, `Refused(AmbiguousTerminalInterval)`)
for the binary case with a hard-coded threshold. The specification above is that
behavior generalized to `n` cells and moved into immutable terms.

### 2.7 Implementation guidance for the next wave

- Implement `derive_payout` as a pure function over already-decoded values.
  Decoding hostile bytes, PDA derivation, account authentication, and aliasing
  are separate and must happen first.
- Keep it behind the existing refusal. Land the function, its decoder, and its
  adversarial tests *while* `Action::Resolve` still returns
  `ResolutionEvidenceUnavailable`. Do not relax the refusal in the same change
  that introduces the code path.
- Required adversarial fixtures, at minimum: every refusal class R-01..R-11; a
  boundary-exact interval on both sides of every boundary; a straddling
  interval; a `CompleteRequired` terms digest presented with a bounded-gap
  window result; a correct window at the wrong generation; a correct window with
  a later maturity bound; a payout map pointing outside the frozen `PayoutSet`;
  and a terms blob whose 144-byte window subrange decodes but names another
  feed.
- Do not weaken a refusal to make an integration test pass.

---

## 3. Cross-language profile identity (P1-G, decided and implemented in Python)

### 3.1 The decision

§P1-G recorded two unjoined digest algorithms:

```text
Python collateral policy:  SHA-256("dragons-clutch/collateral-profile/v1" || 0x00 || 266 bytes)
Rust layout ProfileHash:   SHA-256("dragons-clutch/profile/v1" || profile_bytes)
```

**Decision: the collateral-policy digest is NOT the Realm's Profile ID. It is
one domain-separated subfield inside a broader parent Profile whose canonical
bytes are hashed by the already-frozen Rust rule.**

The consequence worth stating plainly: `canonical_profile_hash` in
`programs/solana-layout` does not change. Its domain string does not change and
its algorithm does not change. What this decision freezes is *which bytes it
consumes*. The child digest rule likewise does not change, and the existing
266-byte golden vectors are untouched.

Why a subfield rather than an alias: a Realm Profile will eventually commit to
more than collateral (batch relation parameters, fee policy, window/feed
admission). If the Profile ID *were* the collateral digest, adding any of those
would either silently change every existing Profile ID or create a second
identity. A tagged subfield inside a versioned parent lets the parent grow
through a schema bump with an explicit compatibility decision, which is the
same discipline the collateral extension matrix already uses.

### 3.2 Parent canonical bytes (64) and composition rule

```text
D_col  := SHA-256( "dragons-clutch/collateral-profile/v1" || 0x00 || collateral_bytes[266] )
P      := parent_bytes[64], with D_col embedded at offset 16 under subfield tag 1
ProfileHash := SHA-256( "dragons-clutch/profile/v1" || P )
```

| Offset | Bytes | Field |
| ---: | ---: | --- |
| 0 | 8 | magic, ASCII `DCPROF1` followed by one zero byte |
| 8 | 2 | parent schema version, little-endian `u16` (`1`) |
| 10 | 2 | parent flags, little-endian `u16` (zero in V1) |
| 12 | 2 | subfield tag, little-endian `u16` (`1` = collateral policy) |
| 14 | 2 | subfield schema version, little-endian `u16` (mirrors the child's schema) |
| 16 | 32 | `D_col`, the collateral-policy digest |
| 48 | 16 | zero reserved bytes |

Note the deliberate asymmetry: the child domain string carries a trailing
`0x00`, the parent's does not. Both are unambiguous because each payload has a
fixed length (266 and 64) and a distinct magic, and the parent rule is the
pre-existing frozen Rust one. Any future variable-length payload under either
domain must re-derive prefix-freeness rather than inherit this note.

The subfield schema version is inside the parent preimage on purpose: a future
collateral schema (267 bytes, or a widened flag word) changes the parent ID even
if it happened to produce the same child digest.

### 3.3 Golden vectors

Computed by `research/collateral-profiles/model.py`, frozen in
`research/collateral-profiles/identity_vectors.json` with a derivation manifest,
and recomputed from the model by the checking tests.

| Profile | Child digest `D_col` | Parent `ProfileHash` |
| --- | --- | --- |
| generic Token-2022 | `aafb2252…c5c32` | `8180f42830d90ef060ec2e4d91c6c19145db9cd9e2dbfd759045770930831688` |
| DREGG dogfood | `ef63ccd0…54343` | `31cd82668ac7846bbf6bf38d25107d0301bc468d40816bf9a565ac93766f93b3` |
| legacy, SOL fee | (in the vectors file) | `f2ea9b4747076c06c1adb6b5ce3bb5fbecdeacd2b7f03d6c131cc10b0ce85db6` |

The two child digests are unchanged from `COLLATERAL_PROFILES.md`. The DREGG row
is still an offline example built from assumed decimals and an assumed ceiling;
it is not a chain fact and not a deployment manifest.

The vectors file also freezes 9 decode refusals (wrong magic, nonzero reserved,
swapped flag/tag fields, unknown subfield tag, unsupported parent schema,
unsupported subfield schema, zero child digest, truncated, extended), 3 binding
refusals (bit-flipped child digest, swapped child-digest halves, a well-formed
parent carrying a *different* Realm's collateral policy), and 4
domain-separation confusions (child domain over parent bytes, parent domain over
child bytes, undomained parent bytes, parent domain with a spurious separator
byte). Every confusion digest is asserted distinct from both real digests and
from each other.

The binding refusals are the important ones. A well-formed parent profile is not
evidence of the right subfield: `ProfileIdentity.from_canonical_bytes` accepts
the DREGG parent happily, and only `verify_profile_identity(profile, bytes)` —
which recomputes `D_col` from the actual collateral policy and compares —
rejects it.

### 3.4 Rust-side field placement (coordination with the `solana-layout` lane)

`programs/solana-layout` is owned by a concurrent lane this wave and was not
touched here. That lane landed the reserved bytes; this section records what
landed, and what the two lanes still owe each other.

**What that lane landed.** `ProfileAccount` gained
`collateral_policy_digest: Hash32` at byte offset 66, taking
`account_len::PROFILE` from 68 to 100 bytes and bumping `account_version::PROFILE`
to 2:

```text
tag(1) version(1) profile(32) realm(32) collateral_policy_digest(32) version(1) flags(1)
```

It is zero until frozen, and `PROFILE_FLAG_POLICY_FROZEN` (flags bit 0) is set
exactly when it is nonzero; the decoder refuses every other combination, so
"unfrozen" cannot be silently confused with "frozen to zero" and a stray nonzero
digest cannot pass as unfrozen. That lane deliberately added no derivation
function and its comment defers the digest *algorithm* to this lane. §3.1-§3.3
above are that algorithm, and this document is its owner.

**Field placement matches.** The 32 reserved bytes are the child digest `D_col`
of §3.2 offset 16 — not the parent identity, and not any other digest. Nothing
else should start carrying `D_col` as an identity: `RealmAccount.profile` and
`MarketAccount.profile` continue to hold the *parent* `ProfileHash`, so there
remains exactly one semantic owner of "the Profile ID".

**Still owed on the Rust side, in a later wave:**

1. `canonical_profile_hash` stays exactly as it is — same domain string, same
   algorithm. When the parent construction is frozen, its input becomes the 64
   bytes of §3.2 and it must additionally **require the input length to be
   exactly 64**. Today it accepts any `&[u8]`, which is the one real
   prefix-freeness hazard on the Rust side.
2. Add the §3.2 parent encoder/decoder with the discipline the crate already
   applies: exact length, magic, known schema, known subfield tag, zero reserved
   bytes, byte-for-byte re-encode.
3. Add a checked binding rule. Decoding is not checking: a well-formed parent
   can commit to another Realm's collateral policy (§3.3). The Rust analogue of
   `verify_profile_identity` must recompute `D_col` from an actual decoded
   266-byte policy and compare, and a frozen `ProfileAccount` whose digest does
   not match must refuse rather than warn.
4. Add the three positive vectors of §3.3 and at least the three binding
   refusals as Rust golden tests, so the two languages are pinned to the same
   bytes and the same digests. A round-trip test alone would not catch a domain
   or field-order divergence.

The Rust work is **not** authorized by this document to relax anything either.
An admitted layout Profile still does not imply admission by the collateral
model until an adapter authenticates a real mint and a real Hoard token account,
which is outside every offline crate.

### 3.5 Addendum 2026-08-18 (later wave): obligations 2, 3, and 4 discharged

Status: implemented in `programs/solana-layout/src/collateral.rs` and gated
(74 unit tests + 2 doc tests green, `clippy --all-targets -D warnings` clean,
`rustdoc -D warnings` clean, `cargo fmt --check` clean; the downstream
`clutch-solana-reference` and `clutch-sbf` workspaces still build). The
algorithm, the domain strings, and the vectors of 3.1-3.3 are unchanged; this
section only records the Rust side catching up.

| 3.4 obligation | State |
| --- | --- |
| 1. exactly-64-byte input to `canonical_profile_hash` | landed earlier; unchanged |
| 2. parent encoder/decoder | `collateral::ParentProfile` |
| 3. checked binding that recomputes `D_col` | `collateral::verify_collateral_binding`, plus `verify_profile_identity` |
| 4. cross-language golden tests | 3 positive vectors, 9 parent decode refusals, 3 binding refusals, 4 domain-separation confusions |

The golden bytes and digests are transcribed as raw hex fixtures from
`identity_vectors.json`, not recomputed in Rust. A Rust round trip would agree
with itself even if the domain string or field order had drifted, which is
precisely the divergence obligation 4 exists to catch. On the first run the
Rust build reproduced all three child digests, all three 64-byte parent
preimages, all three parent identities, and all four confusion digests exactly.

**Decoding still authenticates nothing.** `CollateralPolicy::decode` accepts any
well-formed policy and `ParentProfile::decode` accepts any well-formed parent,
including a real parent belonging to another Realm. The Rust tests reproduce the
load-bearing negative directly: the DREGG parent decodes, binds the DREGG policy,
and is refused against the generic policy. Only `verify_collateral_binding`
recomputes and compares.

`verify_profile_identity` goes one step further and requires the account's stored
Profile ID to be the parent hash over the same digest. That is sound *only*
because the V1 parent preimage carries exactly one subfield, making the identity
a total function of `D_col`. A future parent schema with a second subfield must
move behind a new composition rather than relax this check; the subfield schema
version living inside the preimage (3.2) is what makes that a version bump
rather than a silent reinterpretation.

#### Refusal parity with `model.py`

Every refusal of `RealmCollateralProfile.from_canonical_bytes`,
`CurrencyRef.__post_init__`, and `RealmCollateralProfile.__post_init__` is ported
and has an adversarial Rust test. The taxonomy codes are the frozen ones from
`VECTOR_SPINE_PROPOSAL.md` 2.3 via `CodecError::code`.

| Python refusal | Rust refusal | Code |
| --- | --- | ---: |
| not exactly 266 bytes (short) | `Truncated` | 2011 |
| not exactly 266 bytes (long) | `TrailingBytes` | 2012 |
| invalid collateral-profile magic | `WrongTag` | 2030 |
| reserved bytes must be zero | `NonCanonicalPadding` | 2022 |
| unknown currency kind | `InvalidEnum` | 2021 |
| native SOL must use zero program and mint | `NonCanonicalPadding` | 2022 |
| native SOL decimals must be nine | `InvalidCount` | 2040 |
| currency token program / mint cannot be zero | `ZeroIdentity` | 4009 |
| unsupported token program | `InvalidEnum` | 2021 |
| unsupported collateral-profile schema | `WrongVersion` | 2031 |
| V1 collateral must be an SPL token | `InvalidEnum` | 2021 |
| maximum supply must be a positive u64 | `ZeroValue` | 2046 |
| profile contains unknown flags | `InvalidEnum` | 2021 |
| Realm cannot weaken the V1 authority/state policy | `InvalidEnum` | 2021 |
| V1 fee currency must be collateral or native SOL | `MismatchedBinding` | 4011 |
| V1 liveness currency must be native SOL | `MismatchedBinding` | 4011 |
| extension mask contains unknown bits | `InvalidEnum` | 2021 |
| required extensions must also be allowed | `InvalidEnum` | 2021 |
| Realm cannot expand the mint-extension ceiling | `InvalidEnum` | 2021 |
| Realm cannot expand the account-extension ceiling | `InvalidEnum` | 2021 |
| legacy SPL Token profile cannot declare extensions | `InvalidEnum` | 2021 |
| non-canonical collateral-profile encoding | `NonCanonicalPadding` | 2022 |

Three Python refusals have no Rust counterpart because the Rust types make them
unrepresentable rather than unreachable: "currency reference must be 66 bytes"
(fixed offsets in a fixed-length buffer), "extension mask must fit u64", and
"currency decimals must fit u8".

The Rust taxonomy is coarser than the Python messages, so several distinct policy
faults share `InvalidEnum`. What is preserved exactly is the *verdict* and the
*order*: on the decode path the three currency references are validated before
any policy-level constraint, mirroring `from_canonical_bytes` constructing
`CurrencyRef`s before `RealmCollateralProfile`, so a multi-fault input reports
the same fault in both languages. `CollateralPolicy::validate` called directly on
an in-memory value checks its own schema first; that path has no byte input and
no Python counterpart.

The nine parent decode refusals map: wrong magic to `WrongTag`, nonzero reserved
to `NonCanonicalPadding`, swapped flags/tag to `InvalidEnum` (nonzero parent
flags), unknown subfield tag to `WrongTag`, unsupported parent schema and
unsupported subfield schema to `WrongVersion`, zero child digest to
`ZeroIdentity`, truncated to `Truncated`, and extended to `TrailingBytes`.

#### The `collateral_cap` finding: the policy has no cap field

Commit `1d0c257` recorded that `CreateMarket` writes
`MarketAccount.collateral_cap = 0` because nothing could decode the collateral
policy, and that a market created today therefore cannot accept collateral. The
decoder now exists, and the honest answer is that **it does not unblock the cap**.

`MarketAccount.collateral_cap` is a per-market limit on Hoard atoms; both
`clutch-solana-reference` and the SBF `split` refuse a split whose resulting
collateral would exceed it. The 266-byte policy carries no per-market field.
`max_supply_atoms` is a *Realm-wide admission constraint on the mint*, and
`COLLATERAL_PROFILES.md` states in as many words that "the supply ceiling is not
a solvency proof". Mapping it onto `collateral_cap` would grant every market in
a Realm permission to absorb the entire admitted mint supply, which bounds
nothing in aggregate and misstates what the field means. It would also be a
worse failure mode than zero, because it looks like a risk limit.

What the policy does supply is a sound necessary condition, since a market can
never hold more atoms of a mint than that mint is admitted to have.
`CollateralPolicy::market_cap_ceiling_atoms` and `check_market_cap` expose that
and nothing more: a cap above the ceiling is refusable, a cap at or below it is
merely not refuted, and a policy whose ceiling is `u64::MAX` refutes nothing.

**The cap needs a terms field, not a policy field.** Neither the frozen
`CreateMarket` intent (realm, profile, market nonce, outcome count, terms, feed)
nor `TermsAccount` has anywhere for it to live. Closing this needs one of:

1. a `collateral_cap` field in a new immutable terms schema, checked against
   `check_market_cap` at creation — the option that keeps the cap immutable and
   inside the digest the market already binds; or
2. a new `CreateMarket` intent version carrying the cap, also checked against
   the ceiling.

Both are shared-file schema decisions outside this lane. Until one lands, zero
remains the fail-closed value and the residue stands: a market created today
exists and cannot accept collateral.

#### Wiring note: `require_frozen_collateral_policy` in `clutch-solana-reference`

`programs/solana-reference/src/lib.rs` was **owned by a concurrent
merge-semantics lane during this wave** (a live working-tree diff adding the
`Intent::Merge` arm), so this lane did not edit it. The reference side is
unchanged and its comment correctly still says the binding check is unwritten
*there*. What that lane, or the next one to own the file, should do:

- `require_frozen_collateral_policy(&ProfileAccount)` today checks only
  freeze discipline: the flag is set and the digest is nonzero. It cannot tell
  whether the digest is the right one, and its own doc comment says so.
- Take the 266 policy bytes as a **new evidence input** to
  `validate_market_init` (its only callers are that file's own tests; the SBF
  program re-composes rather than calls it, so the signature change is local),
  and replace the body with
  `clutch_solana_layout::collateral::verify_collateral_binding(policy_bytes, &profile)`,
  mapping `CodecError` through the existing `Error::Layout`.
- Keep `Error::CollateralPolicyNotFrozen` for the unfrozen case
  (`CodecError::ZeroIdentity` from that function) so the taxonomy does not move.
- The test fixture is already most of the way there: `fixture()` builds
  `profile_hash` as `canonical_profile_hash(&parent_profile_bytes(h(0xc0)))`
  with a local 64-byte parent builder. Replacing the placeholder `h(0xc0)` with
  the generic Token-2022 golden policy's child digest, and carrying the 266
  golden bytes in `Fixture`, makes every existing call site bind a real policy
  and lets `verify_profile_identity` be used instead if that lane wants the
  stronger check.
- Adversarial tests owed there: a foreign well-formed policy refused, a
  bit-flipped stored digest refused, and hostile policy bytes surfacing the
  decoder's refusal rather than a generic mismatch.

---

## 4. Promotion gates before any refusal is relaxed

Obligation 8 of `SOLANA_REFERENCE_ADAPTER.md` requires a non-discretionary
authenticated resolution path carrying typed and checked maturity, a sealed
`WindowResult`, feed/source/generation identity, market-terms binding, and
payout-set membership. This wave supplies the *offline typed* half of exactly
one of those items. The remaining gates, in dependency order:

1. **Evidence decoding.** Something must turn authenticated account bytes into a
   `WindowResult`. Today the only constructor is an in-process
   `WindowAccumulator`, which means the evidence has to be produced by the same
   program that consumes it. A persisted, authenticated `WindowResult` account
   plus its decoder does not exist.
2. **Feed authentication.** The `FeedIdentity` bytes are opaque. The source
   adapter admission dossier (`ACCUMULATOR_PLAN.md` §9) has no entries.
3. **Feed epochs and repair sealing.** `GEN-FINAL-AT-MATURITY-02` and any
   non-voiding repair policy need a sealed feed-epoch object.
4. **Terms binding.** `ResolutionTerms` decoding and the `MarketAccount.terms`
   digest check are specified in §2 and implemented nowhere.
5. **Payout representation.** `AMBIG-COMPATIBLE-SET-02` and
   `FAIL-UNIFORM-REFUND-01` both depend on the one-hot-versus-lots/remainder
   decision (recovery order item 6) and on P1-A.
6. **Hostile-input adapter.** Everything in obligations 1-7 and 9-14.

Until at least items 1, 2, and 4 have checked artifacts, the honest description
of the resolution path is "unconditionally refused", and both refusals should
stay byte-identical.

---

## 5. Evidence and gates

Run from the repository root. These crates are intentionally not in a root
workspace, so each is gated independently.

```sh
cargo test    --manifest-path crates/clutch-accumulator/Cargo.toml --offline --locked
cargo clippy  --manifest-path crates/clutch-accumulator/Cargo.toml --offline --locked --all-targets -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --manifest-path crates/clutch-accumulator/Cargo.toml --offline --locked --no-deps
cargo fmt     --manifest-path crates/clutch-accumulator/Cargo.toml -- --check

python3 -m unittest discover -s research/collateral-profiles -p 'test_*.py'
python3 research/collateral-profiles/run_lab.py
```

Result of this wave: accumulator 24 unit tests (10 pre-existing summary-algebra
tests plus 14 window tests) and 2 doctests, one of which is the `compile_fail`
substitution witness; collateral profiles 28 tests (19 pre-existing plus 9),
covering 3 positive identity vectors, 9 decode refusals, 3 binding refusals, and
4 domain-separation confusions. All green, all sub-second, all offline.

What that evidence is: deterministic host-side unit tests of an offline
prototype. What it is not: verification, a refinement relation, mutation or fuzz
evidence, a cross-runtime vector manifest, SBF evidence, or any statement about
a chain, a source, a mint, or real collateral.
