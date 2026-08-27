# V1 benchmark and research laboratory

Status: experiment design. No results in this document are measurements.

## 1. Measurement rule

Every result binds an immutable scenario manifest, source revision, dependency
locks, toolchains, machine/runtime description, fixture digest, warmup policy,
sample count, raw observations, and analysis script. A chart without its raw data
and derivation manifest is a presentation artifact, not release evidence.

Experiments use synthetic assets and offline/local validators unless a separate
current authorization explicitly widens the boundary. Simulated fills are never
called executable liquidity, and local-validator compute is never presented as a
mainnet landing guarantee.

## 2. Scenario manifest

```text
Scenario {
    scenario_version
    source_revision
    toolchain_lock_digest
    fixture_manifest_digest
    realm_profile_id
    market_template_id
    outcome_count
    order_family_mix
    order_count
    page_count_and_shards
    price_scale
    fee_policy
    allocation_policy
    source_and_window_profile
    adversary_profile
    repetitions_and_seed
}
```

Exact atom units accompany every financial field. Report quantiles and full raw
samples where practical; do not average distinct failure classes together.

## 3. Kernel resource matrix

Measure host execution and pinned SBF program-test for:

| Axis | Values |
|---|---|
| outcomes | 2, 4, 8, 16; refusal at 17 |
| quantity/coefficient | 0, 1, typical, maximum admitted |
| partition location | below, on every boundary, above, ambiguous interval |
| claim transition | split, merge, materialize, dematerialize, redeem |
| codec | canonical, truncated, extended, bad tag, bad padding, max values |
| build | Verus annotated/erased and equivalent unannotated control |

Record instruction data, account count, writable lockset, CPI/trace count,
actual/requested CU, stack/heap, ELF bytes, account-data delta, refundable rent,
and transaction count. Thresholds must be derived from pinned protocol limits and
a disclosed safety margin, not a convenient green number.

## 4. Batch relation matrix

### Correctness corpus

- exact exhaustive books small enough to enumerate;
- single-Egg curves with and without virtual split/merge;
- proportional portfolios compared with exact-rational reference search;
- empty, one-sided, crossed, tied, and dust-only books;
- invalid, valid inferior, tied, and improving candidate sequences;
- marginal sets with nonzero remainders under every shard/page permutation;
- settlement in every order with retries.

### Scaling axes

```text
outcomes: 2, 4, 8, 16
orders:   0, 1, 32, 128, 512, then refusal/next page
pages:    1, 4, 16
shards:   1, 2, 8, 16
portfolio share: 0%, 10%, 50%, 100%
```

Measure verifier complexity separately from solver time. Search quality is the
gap to the exact small-book oracle under the frozen score, not a claim that the
oracle scales. Report the count and economic mass of valid but unsubmitted better
candidates when the test can enumerate them.

### Market-structure measures

- filled risk mass and exact surplus components;
- simplex coherence and executable complete-set band after all costs;
- price and allocation sensitivity to one order/atom;
- candidate replacement count and withheld-candidate loss;
- maker/taker concentration and self-cross rejection;
- fragmentation advantage under fee and remainder policies;
- external independent-Egg control versus coupled relation.

## 5. Shared accumulator matrix

Test source-neutral summaries before real adapters:

- 1, 4, and 16 archive pages;
- every valid parenthesization/page split for small paths;
- regular coverage, gaps, repairs, duplicates, stale and future records;
- minimum/maximum source intervals and maximum horizon widths;
- terminal, TWAP, extrema, variance-bound, drawdown-bound, and registered
  threshold features independently;
- one versus many Markets sharing the same Window;
- retention/recycling at exact last-reference boundaries.

Measure CU, bytes/bucket, bytes/page, writable accounts, fold transactions,
capital booked per unfinished job, and work saved by sharing. Feature families
that do not compose associatively or exceed width/resource bounds are removed or
narrowed.

## 6. Cryptoeconomic laboratory

### Solvency and protected pools

Generate randomized reachable traces across two incompatible synthetic Realm
profiles. After every transition recompute maximum liability, total categorical
supply, Hoard balance, order reservations, fees, liveness bookings, rent bonds,
and treasury. Inject direct external burns, donation reconciliation, partial
failure, retries, and maximum values.

Success means no invariant failure on admitted traces and every invalid trace is
refused before an external effect. It is evidence against known bugs, not a proof
of Solana or Token-2022 behavior.

### Fee arms

Compare zero, flat notional, decomposed per-Egg, and atomic simplex-dispersion
policies across the parameter grid in [ENGINEERING_PLAN.md](ENGINEERING_PLAN.md).
Measure exact all-in cost, route leakage, depth/fill response, fee carry, complete-
set neutrality, partition refinement, fragmentation, and wash-loop net loss.

Promotion requires a simple explanation and closed conservation properties as
well as favorable simulation. Novel geometry is not a reason to retain a policy.

### Liveness arms

Replay synthetic and recorded fee distributions only from a provenance manifest.
Stress keeper absence, contention, theft attempts, source outage, reward-asset
collapse, collateral collapse, and zero future volume. Report separately:

- landing-cost distribution;
- offered versus paid bounty;
- time-to-land conditional on inclusion opportunity;
- unspent booked resources;
- terminal failure path and affected notional.

Never claim a finite bounty guarantees liveness under unbounded censorship.

## 7. Static-client laboratory

Build and test Glass without secrets or remote JavaScript:

- reproducible bundle and asset hash equality from a fresh checkout;
- strict CSP and outbound-request inventory;
- malicious/lying RPC and index responses;
- unknown release, program, Realm, Market, source, and schema versions;
- exact human and raw transaction preview without submission;
- wallet rejection/cancellation and no background signing;
- keyboard-only, screen-reader, zoom, reduced-motion, and text-equivalent flows;
- service-worker upgrade/cache pinning behavior;
- local download, GitHub Pages artifact, and IPFS-ready artifact equality.

Client correctness never substitutes for program checks. Deliberately malicious
clients must fail to produce an accepted invariant-violating transition.

## 8. Stop and promotion criteria

Each experiment declares in advance:

- the hypothesis it can falsify;
- an absolute protocol/resource ceiling;
- a regression budget relative to the pinned baseline;
- a quality threshold, if one is defensible;
- what redesign or scope reduction follows a failure.

Examples:

- outcome 17 must refuse rather than silently exceed the transaction envelope;
- a single-source Verus approach stops if executable SBF divergence is required;
- portfolio intents defer if exact verification or allocation is not practical;
- a path statistic is removed if its summary cannot preserve conservative
  semantics associatively;
- the native venue narrows to single-Egg coupled clearing if arbitrary portfolio
  search pressures documentation into an unproved optimality claim;
- no benchmark, proof, audit, or static build opens Gate L0 or authorizes public-
  network deployment.

## 9. Result directory contract

Future measured outputs should follow:

```text
research/results/<date>-<scenario-id>/
  scenario.json
  environment.json
  fixture-manifest.json
  raw/
  derived/
  report.md
  checksums.sha256
  README.md
```

`research/results/` contains no secrets, wallet material, proprietary provider
credentials, or unexplained copied datasets. Large generated artifacts may be
content-addressed externally only after their availability and license policy is
documented.
