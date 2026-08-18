# Shared path accumulator plan

Status: E0/E4 research scaffold. No source adapter or onchain observation has
been implemented or authenticated.

## 1. Objective

Many Markets may depend on the same authenticated path. Dragon's Clutch should
pay the information lower bound once per frozen source/grid family, then reuse
canonical summaries and immutable Window results. It must never claim that a
summary can answer a predicate whose required information it discarded.

```text
authenticated source states
  -> ordered bucket observations
  -> associative page summaries
  -> immutable WindowResult
  -> closed statistic evaluator
  -> partition cell or conservative compatible set
```

The shared accumulator reduces repeated work from one stream per Market to one
stream plus one bounded fold per distinct Window identity. It cannot remove the
need to observe each required boundary when the source retains no equivalent
authenticated history.

## 2. Identity hierarchy

### `SourceAdapterId`

Binds adapter semantics, source program/deployment identity, accepted account
layouts, orientation, unit normalization, confidence/quorum rules, and upgrade
refusal behavior.

### `FeedSpecId`

Adds subject, quote, grid origin/period, timestamp/slot policy, dispersion policy,
feature family, archive page geometry, and repair policy.

### `BucketId`

Canonical integer index derived from `FeedSpec` and time. A boundary maps to
exactly one bucket or is refused. Wall-clock strings never enter consensus.

### `WindowId`

Content-addresses `FeedSpecId`, exact start/end buckets, feature subset,
StatisticProgram version, and rounding policy. Equivalent requests reuse one
immutable result; merely similar windows do not.

## 3. Observation record

A source adapter produces a conservative semantic record:

```text
Observation {
    feed_spec_id
    bucket_id
    source_slot_or_sequence
    source_time
    value_low
    value_high
    confidence_or_dispersion
    coverage_flags
    evidence_digest
}
```

The adapter, not Eggcrate, parses the external account and authenticates its
program owner, deployment/version, subject, quote, orientation, timestamp, and
source-specific validity conditions. Eggcrate checks canonical bounds and the
monotone summary transition. A client-asserted number is never an Observation.

## 4. Summary algebra

Every admitted page feature needs an identity element, a total bounded `append`,
a total bounded `combine`, and a proof of associativity over well-formed adjacent
intervals. Proposed V1 fields are intentionally redundant enough to detect
invalid joins:

```text
Summary {
    start_bucket
    end_bucket_exclusive
    accepted_count
    missing_count
    first_interval
    last_interval
    min_low
    min_high
    max_low
    max_high
    price_time_integral_low
    price_time_integral_high
    squared_return_sum_low
    squared_return_sum_high
    drawdown_bound
    coverage_digest
    state
}
```

This is a research envelope, not a frozen layout. Every field must earn inclusion
through a named StatisticProgram and a width/storage benchmark. If conservative
interval composition for variance or drawdown is not both correct and useful, V1
must narrow or remove it.

`combine(a,b)` refuses unless intervals are adjacent, identities and units match,
and all arithmetic remains within frozen bounds. Gaps are represented explicitly;
they are never bridged by interpolation unless the immutable source policy names
and bounds such a rule.

## 5. Supported statistic boundary

The first candidate family is:

| Statistic | Required summary | Ordinary result |
|---|---|---|
| terminal interval | last accepted conservative interval | `[low, high]` |
| TWAP interval | integral and exact covered duration | `[low, high]` |
| sampled min/max | conservative extrema | interval |
| relative terminal/TWAP | synchronized summaries and checked ratio rule | interval |
| realized-variance bound | squared-return terms and coverage | interval |
| maximum-drawdown bound | proven associative drawdown summary | interval |
| registered threshold automaton | threshold-specific finite state | categorical/interval |

The generic summary does not promise arbitrary duration-above-threshold,
arbitrary fine-grained crossings, user-provided predicates, or reconstruction of
discarded observations. Those require a registered threshold automaton or a new
versioned feature family.

## 6. Storage and lifecycle

```text
FeedHead(current open summary, next bucket, bookings)
   -> sealed ArchivePage(summary range, reference horizon)
   -> immutable WindowResult(exact WindowId)
   -> recyclable page only after every possible live reference expires
```

The retention proof must derive the last possible reference from admitted Market
windows, repair grace, resolution grace, and any prepaid Series horizon. A page is
not recyclable because a client or index believes it is unused.

One accepted observation should ordinarily write only the FeedHead/open page and
the caller's internal keeper credit. It must not fan out writes to subscribing
Markets or perform one reward-token CPI per bucket.

## 7. Repair and ambiguity

The default path is append-only by bucket. Repair is separately bounded:

- an absent bucket may be filled only within its frozen repair interval;
- an already accepted qualifying record is not rewritten by a later preference;
- multiple source records for one bucket follow one deterministic choice/refusal
  rule frozen in the adapter;
- repaired summaries are recomputed through a bounded page/window work object;
- every WindowResult binds the exact page generations it consumed.

If the final statistic is `[x_low, x_high]`, ordinary one-hot resolution succeeds
only when that entire interval belongs to one partition cell. Otherwise the
Market enters its frozen compatible-set/failure policy. No midpoint convention
converts uncertainty into false precision.

## 8. Liveness accounting

Each admitted Feed epoch books worst-case remaining work before accepting a new
subscription. Required jobs include normal observations, allowed repair,
page sealing/folding, Window closure, and eligible cleanup. Bookings are in the
asset actually needed for unavoidable network costs; a volatile reward token may
supplement but never replace that amount.

Measurements must distinguish:

- reserved maximum, offered bounty, paid bounty, and unused balance;
- unique novel work from duplicate attempts;
- ordinary, repair, catch-up, and cleanup costs;
- source/provider availability from transaction landing probability;
- conditional liveness under the frozen maximum from unconditional guarantees.

No finite bounty proves inclusion under unbounded censorship or congestion.

## 9. Source adapter admission dossier

Every adapter candidate must record:

1. primary specification and exact program/deployment/layout identity;
2. authority and upgrade model;
3. observation retention and timestamp semantics;
4. subject/quote/orientation and decimal normalization;
5. confidence, quorum, manipulation, and stale-data behavior;
6. what a transaction can authenticate without trusting an RPC;
7. missing, duplicated, rolled-back, upgraded, or malformed cases;
8. maximum affected collateral and common-mode dependencies;
9. fixture provenance and independent recomputation method;
10. exact adapter version and refusal surface.

Initial work uses synthetic source fixtures. No real source preset is accepted
because its name is familiar or because a client API returns a price.

## 10. Proof and test obligations

Verus-first targets:

- `append` and `combine` are total within admitted bounds;
- accepted intervals advance monotonically and never overlap;
- `combine` conserves bucket coverage and missingness;
- alternative valid parenthesizations produce the same summary;
- Window identity and generation checks prevent stale-page substitution;
- statistic evaluation uses only fields promised by its feature family;
- uncertainty can only stay equal or become more conservative through a fold;
- duplicate work cannot earn twice.

Rocq/Lean shadow targets encode the interval monoid and its correspondence to
closed statistic semantics. Host/SBF differential tests cover boundary values,
page splits, repair order, source refusal, and maximum-width arithmetic.

## 11. Falsifiers and gates

Required counterexamples include:

- bucket off by one at grid origin or end boundary;
- combine of nonadjacent or differently versioned summaries;
- missing bucket mislabeled covered;
- reordered repair producing a different valid Window;
- interval narrowed by an unsafe midpoint or division;
- source upgrade accepted under an old `SourceAdapterId`;
- archive page recycled while a live Series can reference it;
- shared subscriber withdrawal undercapitalizing remaining work;
- duplicate observation/reward and repair bounty theft;
- maximum accumulated integral/variance overflow.

E4 stops or narrows the feature family if associativity, width, retention, source
authentication, or zero-future-volume capitalization does not close.

## 12. Addendum 2026-08-18: the typed window evidence plane landed

Status: MODEL. Implemented in `crates/clutch-accumulator`, tested offline,
authenticating nothing.

§6 of this plan sketched `immutable WindowResult(exact WindowId)` as a lifecycle
stage. It is now a type rather than a box in a diagram, because
`ADVERSARIAL_REVIEW_V0.md` §P1-D showed the gap was load-bearing: `Summary`
statistics answer for whatever range they cover, so a caller could treat an
accepted-only statistic as a full-window settlement term with nothing but a
comment to stop them.

What exists now:

- `WindowDomain` — the exact expected feed identity, source/evaluator versions,
  grid, `[start, end)` bucket range, maturity bound, repair generation, and
  registered coverage policy. It has a 144-byte canonical preimage encoding and
  publishes the domain-separation tag `dragons-clutch/window-domain/v1`, but
  this crate still owns no hash primitive and computes no `WindowId`.
- `CoveragePolicy` — a closed registry with private fields. V1 registers
  `COMPLETE_REQUIRED` and `bounded_gaps(n)`; `n = 0` is refused because it would
  be a second identifier for the first.
- `WindowAccumulator` — folds already-adjacent page summaries into exactly one
  domain and runs `Open -> Mature -> Sealed`. Maturity needs two separate facts:
  every expected bucket represented, and a witnessed monotone feed cursor that
  has reached the maturity bound. The excess of the maturity bound over the
  window end is the frozen repair grace of §7.
- `WindowResult` — the only domain-bound value, constructible only by sealing
  such an accumulator and passing its coverage policy. `check_domain` refuses a
  wrong window, generation, maturity bound, grid, feed, or coverage policy, and
  names which field differed.

This generalizes the vertical model's host semantics: that model's frozen
`MATURITY_BUCKETS` constant and `sealed` flag become a per-window field of an
immutable domain and a terminal transition, and its `NotMature` / `NotSealed` /
`ObservationAfterSeal` / `AlreadySealed` refusals reappear with `WrongWindow`,
`MismatchedGeneration`, `IncompleteDomain`, `NonContiguous`, `RangeOverflow`,
`NonMonotoneCursor`, and `CoverageRefused` alongside them.

Tests: 24 unit (10 pre-existing algebra, 14 window) plus 2 doctests, including a
`compile_fail` witness that a bare `Summary` cannot be substituted for a
`WindowResult`. Adversarial cases cover the truncated prefix, the gap-tolerant
substitution attempt, the wrong window, the reopened result, the early seal, the
non-monotone cursor, the overrunning page, and the closed policy registry.

Still open from §10 and §11 of this plan: nothing is authenticated, no proof
obligation is discharged, `FeedIdentity` bytes remain opaque, the archive-page
retention proof does not exist, and the source-adapter admission dossier of §9
has no entries. The terms-to-payout function that consumes a `WindowResult`, its
partition/ambiguity/failure policy identifiers, and its refusal classes are
specified in [the resolution evidence plan](implementation/RESOLUTION_EVIDENCE_PLAN.md)
§2 and implemented nowhere. The reference adapter's `Resolve` and
`RedeemInternal` refusals are unchanged.
