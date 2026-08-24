# Product/theory redirection — 2026-08-24

Status: **DECISION-GRADE DESIGN / NO DEPLOYMENT OR RUNTIME CLAIM**.

This is the critical product/research lane requested after the integrated
successor work. It updates, rather than repeats,
[`INSTRUMENT_AND_MARKET_DESIGN_REVIEW_2026-08-22.md`](INSTRUMENT_AND_MARKET_DESIGN_REVIEW_2026-08-22.md).
The code snapshot inspected was `6b11b834a7666d3694655d80f1b771bc455fdc35`.
Parallel lifecycle work may already be newer. The recommendations below are
versioned successors; they are not authority to fork or disrupt the current
action-14/action-15 integration.

The original product intent was re-read from the local `cv` corpus, especially
the origin session `01a00eb3-156b-7183-a013-84f84a95433f`, the Clear/Shielded/
Dark session `01a00f44-ab00-7c42-988b-4d40ef6cecb8`, and the parent history
catalogued in
[`PROJECT_INTENT_ARCHAEOLOGY_2026-08-22.md`](PROJECT_INTENT_ARCHAEOLOGY_2026-08-22.md).
That intent was not a small prediction-market demo. It was one exact algebra
for bounded state claims, recurring objective risk products, atomic portfolios,
interchangeable execution modalities, real funded liquidity, and public
operatorless settlement. DREGG was a collateral profile and dogfood case, not
the protocol's hard-coded asset or only market.

## Uncompromising verdict

Dragon's Clutch now has a meaningful financial primitive, and it is more
complete than the August 22 review credited:

- degree-zero categorical claims and degree-one through degree-three exact
  clamped B-spline claims form a fully funded bounded payoff basis;
- nonnegative coefficient portfolios are native to the same RelationV2 as
  single-Egg orders, so shaped execution need not incur leg risk;
- candidate prices can be checked against an exact finite atom-mixture
  representing measure, rather than only an outer approximation; and
- the covered Dealer has real cash and Egg custody, a bounded signed quadratic
  potential, worst-case capitalization gates, and a terminal LP entitlement
  path. It is not merely a research curve any longer.

That is a genuine **compiler and exchange for bounded state-contingent
assets**. Its best first wedge remains recurring tail and path-risk protection
in stable collateral for assets that have no useful options market.

But the venue is not yet economically finished merely because its transitions
can be made callable. Four product defects remain:

1. The current score selects the best valid submitted candidate under a
   risk-flow heuristic. It does not maximize declared trader surplus and does
   not establish a globally optimal clearing.
2. A market's V4 foundation has a 50-slot capacity and makes 18 non-outcome
   accounts mandatory, including General, Failure, Fractional, and General
   treasury roles. The active count is `18 + 2 * outcome_count`, hence 22 even
   for two outcomes and 50 at the maximum width. This is a major rent and
   creation-complexity policy disguised as a layout.
3. `ProductTemplateV4` describes one source, one statistic, one time window,
   and one basis. `SeriesPlanV5` repeats it on one uniform stride with one
   constant market cap. These are useful compilation targets, not the product
   model's natural limit.
4. Fully funded liquidity still pays for certainty with rigid capital. Dealer
   LP ownership is frozen after activation, one quadratic family owns the
   curve semantics, and every expiry must independently rediscover liquidity.

The correct response is neither to delete the sophisticated machinery nor to
declare it done. Finish the current lifecycle spine, then version the following
successors before a public release makes the accidental restrictions expensive
to change.

## What the instrument does—and does not promise

For an exhaustive, disjoint state partition with exact basis values
`phi_i(x) / D` and `sum_i phi_i(x) = D`, one unit of every native claim is a
complete set. Collateral split into a complete set can always be recombined;
after objective resolution, the complete set pays the original collateral.
Hoard principal backs that identity and cannot become fees, rent, bounty,
reserve, or treasury money.

The economically useful objects are:

- categorical state claims for explicit finite regimes;
- smooth local-support claims that reduce cliff exposure around category
  boundaries;
- exact shapes such as crash, range, tent, capped-directional, drawdown, and
  regime payoffs compiled to one coefficient vector;
- atomic portfolio orders across that basis; and
- bearer wrappers when transfer or external composability justifies another
  mint and custody surface.

The protocol removes under-collateralization, liquidation, and discretionary
settlement risk. It does not remove collateral opportunity cost, source risk,
wrong-way collateral, illiquidity, or the need for a natural seller of
protection. State-price vectors are **state-price weights**, not objective
probabilities. Full collateral makes the product most attractive where bounded
loss and credible settlement matter more than margin efficiency.

The economic wedge should therefore be tested as a product family, not as a
generic market-creation page:

```text
stable-collateral Realm
  + daily/weekly terminal-price and maximum-drawdown Series
  + one canonical crash/range shape catalog
  + covered executable depth and visible maximum maker loss
  + exact roll/refinement paths
  + chain-derived static client
```

DREGG-collateral crash protection is valid dogfood but economically wrong-way
for many buyers. It must not be the reference claim that the product is useful.

## What changed since the prior review

| Area | Current semantic fact | Consequence |
| --- | --- | --- |
| Smooth price coherence | [`clutch-price-measure`](../../crates/clutch-price-measure/src/lib.rs) now has V3 exact quantized witnesses, a full-support atom solver, and fixed 2048-bit fraction-free arithmetic. General and covered Dealer consume the checker. | “Multi-span smooth price coherence is missing” is stale. The remaining work is callable lifecycle ownership, capacity measurement, and honest certificate labeling. |
| General relation | [`relation_v2.rs`](../../crates/clutch-batch/src/relation_v2.rs) is owner-blind and verifies exact vector fills, limits, AON/minimum-fill semantics, virtual split/merge, and conservation. | This is a good common Clear relation. Ownership, fees, and venue authority should remain adapters around it. |
| Candidate rank | ScoreV2-Q maximizes quotient-risk range, then minimizes direct complete-set-equivalent flow and virtual churn. The cost successor adds owner-net cost only as a later tie-break. | This is deterministic and exact, but it is not user surplus, price discovery, or optimality. |
| Product recurrence | Template V4, Genesis V2, finite Series V5, funding V6, and foundation V4 form a much stronger market factory than the earlier host model. | The next work is generalization and capital/rent reduction, not rebuilding recurrence from scratch. |
| Source | Reusable SourcePlane generations and chain-derived repair/reopen paths supersede the old Terms-bound singleton shape. | Product composition and shared work capitalization, not another source rewrite, are the remaining theory gaps. |
| Dealer | The covered quadratic Dealer is funded, custody-backed, joined to exact prices, and terminally distributable. | The question is now whether its capital and LP lifecycle are competitive, not whether liquidity is a mock. |

## Restrictions: preserve, profile, or remove

| Class | Restrictions | Decision |
| --- | --- | --- |
| Mathematical safety | Full collateral; exhaustive/disjoint/canonical state partition before minting; exact partition of unity; exact price coherence; immutable Realm collateral; Hoard segregation; deterministic failure; prepaid liveness. | Preserve. These define the instrument. |
| Kernel boundary | `no_std`, `no_alloc`, safe fixed-layout Eggcrate; no Solana SDK, token SDK, CPI, account memory, floats, unchecked casts, or proof-only preconditions. | Preserve. Add capacity profiles around it, not exceptions inside it. |
| Deployment profile | At most 16 native outcomes, 64 orders, degree at most 3, bounded atom count, fixed SBF frames and page counts. | Keep as named ELF/profile limits. Do not describe them as conceptual protocol limits. Compile other widths into other reviewed profiles. |
| Prototype accident | One source/statistic/window per Template; one uniform Series stride and cap; no exact roll/refinement; mandatory 18-account non-outcome foundation; frozen Dealer LP set; one hard-coded potential family; ScoreV2-Q as the only quality policy; owner-grouped cost in consensus rank; a nonzero tail fee floor as an assumed eventual default. | Supersede with the designs below. |

## Ranked redesign queue

The rankings combine user value, architectural leverage, and feasibility. P0
does not mean “pause the active lifecycle work”; it means the successor should
be designed now and owned immediately after that spine is coherent.

| Rank | Redesign | Value | Feasibility | Primary reason |
| --- | --- | --- | --- | --- |
| P0.1 | Declared-surplus rank plus exact dual optimality certificate | Very high | High | Makes candidate selection serve traders and enables an honest, narrow optimality claim. |
| P0.2 | Capability-indexed lazy Market foundation | Very high | Medium | Cuts rent and creation fanout without weakening enabled lifecycles. |
| P0.3 | Canonical roll and exact basis-refinement operations | Very high | Medium | Lets recurring sophistication concentrate rather than fragment liquidity. |
| P1.1 | Product semantic graph and piecewise Series schedule | Very high | Medium | Lifts the one-source/one-stat/uniform-cadence ceiling and enables differentiated risk products. |
| P1.2 | Dealer capital epochs and potential-family contract | High | Medium | Makes covered liquidity reusable, enterable, and economically comparable. |
| P1.3 | Shared Source-work epoch reserve | High | Medium | Pays once for one reusable observation transition without relying on future subscribers. |
| P1.4 | Explicit fee experiment profiles | High | High | Avoids making cheap tail protection unusable or pretending one rate is theory-derived. |
| P2 | Clear/Shielded/Dark relation family and specialized confidential optimizers | Transformative | Low today | Preserves the highest vision, but must not block a complete useful Clear protocol. |

### P0.1 — DeclaredSurplusV1 and ExactSurplusOptimalityV1

The current rank's first coordinate, quotient-risk range, answers “how much
noncash risk changed hands?” It can prefer a larger low-surplus cross over a
smaller high-surplus cross. The later `owner_net_cost_atoms` coordinate in
[`candidate_cost.rs`](../../crates/clutch-general-v2-runtime/src/candidate_cost.rs)
is carefully named, but it is grouped by externally authenticated owner and
uses terminal floors. Splitting economically identical orders among identities
can therefore change it. It must not be advertised as welfare, market quality,
or sybil-neutral capital efficiency.

Use exact declared limit surplus as a breaking score policy. For order `j`, let
`c_j` be its coefficient vector, `f_j` its fill, `q_j` its quantity, `L_j` its
limit value, `p` a checked state-price vector, and `S = sum_i p_i` the complete-
set price scale. If `X` is virtual split and `M` virtual merge, define:

```text
U = sum_buy  L_j f_j - sum_sell L_j f_j - S X + S M
```

Relation conservation implies, with `V_j = dot(c_j, p)`:

```text
U = sum_buy f_j (L_j - V_j) + sum_sell f_j (V_j - L_j).
```

For any exact simplex vector `p`, the following is an upper bound on every
conserved fill of the same frozen book:

```text
D(p) = sum_buy  q_j max(L_j - V_j, 0)
     + sum_sell q_j max(V_j - L_j, 0).
```

Therefore a relation-valid candidate satisfying `U = D(p)` is globally optimal
for this exact declared-surplus objective. The equality is sufficient even
with integer fills, minimum fills, or AON orders; it may simply be unavailable
when those constraints prevent complementary slackness. No solver-authored
proof bytes are needed: the checker recomputes both sides from the frozen book,
exact fill witness, virtual conversions, and authenticated coherent price.

The claim must be exactly: **optimal under DeclaredSurplusV1 for this frozen
book and relation**. Limits are declared willingness, not measured social
welfare, and the supporting state-price vector need not be unique.

Concrete ownership and changes:

| Concern | Decision |
| --- | --- |
| Semantic owner | Add `clutch-batch::declared_surplus_v1`; it consumes a verified RelationV2 projection and no owner, signer, fee, account, or Dealer fact. |
| Schema | New score-policy identity and `SurplusCertificateV1 { primal, dual_bound, optimality }`. Use fixed-limb arithmetic or a policy-proved bound large enough for `u128 limit * u64 quantity`; do not reduce economic capacity merely to fit an incidental accumulator. |
| Rank | Maximize `U`; prefer an exact `U == D` certificate; then minimize representation-neutral work/churn; then canonical candidate identity and first-admitted duplicate. Keep ScoreV2-Q as a distinct legacy/profile choice. |
| Price | Exact price-measure coherence remains a separate mandatory precondition. A supporting dual price is economically meaningful but still a state-price surface, not a probability oracle. |
| Operator | Solver reports `U`, `D`, and the exact gap; it may use LP/MILP/heuristics, but the chain recomputes the certificate. Frontend labels either “exact optimum under DeclaredSurplusV1” or “best valid submitted by declared surplus.” |
| Migration | Do not mutate current Window/Node ranks during action-14/action-15 integration. Introduce a new capability/score profile and new Market identity. Existing markets finish under their frozen ScoreV2-Q policy. |

New adversarial invariants:

- conservation must derive both forms of `U` exactly, including the signs of
  virtual split and merge;
- complete-set creation costs exactly `S` and merge credits exactly `S`;
- the dual bound covers unfilled orders and both buy/sell directions;
- `U <= D` for every accepted candidate, with equality checked byte-for-byte;
- splitting an identical order into copies with the same coefficients/limit
  and the same aggregate quantity does not change the objective or bound;
- no owner, fee recipient, candidate account, or submitter identity changes
  the economic coordinate;
- arithmetic overflow refuses rather than wraps; and
- optimality labeling refuses when the certificate profile, book identity,
  price policy, or relation identity differs by one byte.

The general convex-market literature supports keeping feasibility, a realizable
price domain, and objective certificates separate. See the primary paper
[Efficient Market Making via Convex Optimization and a Connection to Online
Learning](https://www.microsoft.com/en-us/research/publication/efficient-market-making-via-convex-optimization-and-a-connection-to-online-learning/).
Frequent batching itself remains appropriate: the official summary of the
Budish–Cramton–Shim work describes replacing speed competition with price
competition in frequent batch auctions ([Chicago Booth](https://www.chicagobooth.edu/research/fama-miller/finance-research/funding/2011-12/the-high-frequency-trading-arms-race-implications-for-financial-market-design)).

### P0.2 — FoundationGraphProfileV1

[`MarketFoundationScheduleV4`](../../crates/clutch-product-series/src/market_foundation_v4.rs)
requires all 15 core slots and all three General treasury slots to be nonzero;
only the inactive tails of the 16 mint and 16 custody slots disappear. Thus
every market currently instantiates General bindings/runtime, five Failure
roles, Fractional policy/ledger, resolution, replay anchors, Hoard/ledger/vault,
and treasury service infrastructure whether or not the selected product uses
every family.

Replace this with an immutable typed account-dependency graph:

- a small universal liability core;
- explicit capability nodes for General venue, covered Dealer, Fractional,
  Structured, fee treasury, recovery depth, and optional bearer
  materialization; and
- exact child obligations and prefunded principal for every enabled node.

“Lazy” must mean **precommitted and prepaid, created when first needed**. It
must never mean funded from future fees, a later user, or Hoard principal.

Concrete ownership and changes:

| Concern | Decision |
| --- | --- |
| Semantic owner | Product compiler owns `FoundationGraphProfileV1`; each component contract owns only its node schema and transition. Root/Link counted-obligation semantics own graph completion and retirement. |
| Schema | Canonically sorted bounded nodes `(role, generation, parent, dependency_mask, principal, activation_deadline)` plus an immutable capability bitset and exact total principal. No zero-filled maximum-width list is treated as active. |
| Algorithm | Validate acyclic dependencies, unique role ownership, reachability from the Market root, exact principal sum, and closure order. Create a child atomically from its already reserved component balance and increment the parent's counted obligation. |
| Operator | Market creation displays universal rent, selected capability rent, deferred-but-reserved rent, and active account count separately. It derives every PDA/account from graph state; it never guesses a standard 50-account layout. |
| Migration | V4 markets remain V4 and close through the V4 graph. New V5 markets use only the graph profile. No in-place reinterpretation and no dual graph authority. |

The first node-minimization decision should be evidence driven. Resolution and
a minimal failure route may be universal because every liability needs a
terminal answer. Fractional, General treasury, and possibly covered venue
state are clearly profile-dependent. General clearing can remain the default
profile without being compulsory protocol ontology.

New adversarial invariants:

- every enabled node is reachable, uniquely owned, fully prepaid, and counted;
- a disabled role cannot be created or referenced by a live transition;
- creating one child consumes exactly its reserved principal and cannot alter
  another component's reserve;
- creation failure and foundation abort refund exactly the still-unconsumed
  source, never Hoard principal;
- a root cannot retire with an instantiated or reserved child outstanding;
- graph order and inactive padding are canonical; and
- different capability graphs necessarily produce different Product/Market
  identities.

### P0.3 — RollIntentV1 and RefinementMapV1

Recurring Series create habit but also create one new liquidity island per
expiry. The protocol already has the algebra to do better. Add two canonical,
fully backed conversion families:

1. A roll is an atomic cross-Market portfolio exchange between independently
   backed expiries. It never nets their Hoards.
2. A refinement converts compatible coarse claims to fine claims under an
   exact payout-preserving linear map. For categorical nested partitions, one
   coarse claim is the sum of its children. For smooth bases, canonical knot
   insertion supplies the rational map.

Concrete ownership and changes:

| Concern | Decision |
| --- | --- |
| Semantic owner | Product compiler owns compatibility and `RefinementMapV1`; a cross-Market portfolio relation owns atomic roll fills; each Hoard remains sole owner of its own principal. |
| Schema | Map identity binds both Market semantics, source/statistic/window equivalence, exact rational matrix, lot scaling, and directionality. `RollIntentV1` binds both market generations and expiry policy. |
| Algorithm | Re-evaluate every source-basis row under the map and prove exact payout equality before admission. Conversion burns one representation and mints the other atomically with no claim on a foreign Hoard. |
| Operator | Product catalog publishes canonical successor/compatibility edges and quotes roll all-in cost. Wallets distinguish exact conversion from a market trade or approximation. |
| Migration | Existing compatible markets can opt into a separately deployed converter only if their frozen identities are exactly supported. No compatibility is inferred from labels. |

New invariants include exact payoff equality at every admitted primitive state,
canonical rational reduction, no rounding except the named bearer-lot boundary,
no cross-Hoard principal movement, conservation under round-trip conversion,
and refusal for different source, statistic, repair, edge, or window semantics.

### P1.1 — ProductSemanticGraphV1 and SeriesScheduleV2

`ProductTemplateV4` hard-codes a single Source → Statistic → Basis chain. The
larger product thesis needs bounded composition such as relative performance,
price × drawdown, or drawdown × volatility regime. A product should be a
content-addressed acyclic semantic graph:

```text
SourceWindow nodes
  -> exact Statistic nodes
  -> bounded Combine/Product nodes
  -> exhaustive native basis
  -> optional named coefficient programs/wrappers
```

Factorized descriptions and sparse programs may reduce storage and solver work,
but no factorization may mint liabilities until it lowers to an exhaustive,
disjoint, canonical payout space with exact partition-of-unity evidence.

`SeriesScheduleV2` should be a bounded sequence of piecewise-arithmetic
segments, not an unbounded calendar interpreter. Each segment carries count,
first start, stride, creation lead, cap, and attachment override. A small fixed
segment cap preserves deterministic SBF work while supporting weekdays,
changing maturity cadence, staged maker capacity, and finite product seasons.
All occurrences remain finite and fully funded before admission.

Concrete ownership and changes:

| Concern | Decision |
| --- | --- |
| Semantic owner | Product crate owns graph and lowering identities. Source owns raw observations; statistic evaluators own exact transforms; the basis artifact remains sole owner of payout rows. |
| Schema | Bounded topologically ordered graph with typed node references, exact output domains, one root, and canonical unused padding. New `SeriesScheduleV2` has bounded sorted segments and an exact aggregate funding quote. |
| Algorithm | Check acyclicity, type compatibility, full dependency hashes, exact domain coverage, partition sum, collision-free instance derivation, segment non-overlap, and total funding across all instances. |
| Operator | A product compiler emits the graph, human payoff explanation, exact worst-state payout, source/gap contract, and unsigned creation plan. It never asks users to hand-author account DTOs. |
| Migration | TemplateV4/SeriesV5 remain supported frozen schemas. Graph products receive new identities and cannot silently project down to V4. Simple one-axis products compile canonically to one-node graphs. |

Adversarial invariants: omitted state cannot mint; duplicate semantic nodes
deduplicate by content rather than authority; graph order cannot change identity;
source repair generation propagates to every dependent statistic; tensor width
overflow refuses; piecewise segments neither overlap nor create infinite
liabilities; and attachment changes never rewrite economic Market identity.

### P1.2 — DealerCapitalEpochV2 and PotentialPolicyV2

The current Dealer is soundly conservative but economically rigid. LPs fund
fixed per-share cash and Egg units before activation; the ownership set then
freezes until terminal. A sponsor-funded subsidy is real donated capital, not
future revenue. This is safe, but it discourages reusable liquidity and forces
capital decisions before any price discovery.

Take two steps:

1. Permit exact LP-share transfer whenever no active lease forbids the
   ownership mutation. This changes ownership, not facility assets or policy.
2. Add quiescent capital epochs. An epoch boundary requires no open General
   epoch, lease, quote pot, unprocessed fee entitlement, or terminal work. The
   transition scales assets, shares, inventory coordinates, depth, boxes, and
   subsidy rights under one exact rational factor, producing a new policy
   generation. A conservative initial exit profile may require `q = 0` and
   exact divisibility; broader proportional rebasing needs its own proof.

Also replace “quadratic is the Dealer” with a potential-family interface. A
policy supplies exact potential differences, a state-price-gradient
certificate, complete-set translation behavior, a bounded state box, and a
fully paid worst-case-loss bound. Quadratic remains the first reviewed family.
Other convex cost makers are admitted only through new identities and exact
arithmetic. The measurable-space cost-function literature is the relevant
general foundation ([Microsoft Research](https://www.microsoft.com/en-us/research/publication/cost-function-market-makers-for-measurable-spaces/)).

Concrete ownership and changes:

| Concern | Decision |
| --- | --- |
| Semantic owner | Dealer policy owns one capital generation and one potential-family ID. LP pages remain the sole ownership/share truth. General owns only leases and selected candidates. |
| Schema | `DealerCapitalEpochV2` binds before/after policy IDs, exact scale numerator/denominator, asset vector, share root, quiescence witness IDs, and sponsor-right disposition. |
| Algorithm | Recompute pro-rata assets and every scaled policy bound; require exact division or a single named neutral residual boundary; rerun worst-case capitalization before activation. |
| Operator | Show deposited assets, encumbered assets, maximum loss, current inventory, available exit condition, subsidy ownership, and realized fee return separately. |
| Migration | Existing Dealer V1 facilities remain frozen. V2 is a new facility/policy generation or an explicitly authenticated terminal migration; never edit an active V1 body. |

Adversarial invariants: no asset is both Hoard backing and Dealer capital; no
join/exit during a live commitment; proportional state and potential differences
are preserved; sponsor subsidy cannot silently become LP-contributed capital;
share splitting creates no additional entitlement; every terminal residual has
one owner; and the rebase cannot widen a price or inventory box without funding
the new worst case.

### P1.3 — SourceWorkEpochV1

Reusable SourcePlane data should not imply that each subscribing market must
pay the full cost of the same novel observation transition. Conversely, a
market cannot depend on later subscribers arriving. Introduce a shared,
finite, fully funded source-work epoch:

- an initial sponsor deposits the complete maximum work amount;
- markets buy immutable subscription receipts before the epoch closes;
- one accepted raw transition is performed once, without writing to every
  market;
- reimbursements/refunds use exact prefix allocation with one neutral residual;
  and
- all markets can settle from the shared immutable source fact even if no
  additional subscriber appears.

Source owns the observation and work epoch. Product owns only its subscription
receipt. Funding owns exact debit/refund state. The operator should show marginal
subscription cost separately from the already capitalized reserve.

Adversarial invariants: booked maximum work never exceeds present reserve; a
novel transition is paid at most once; subscriber fanout cannot make source
execution O(number of markets); early exit cannot strand remaining markets;
the sponsor cannot reclaim encumbered work; no market receives two refunds;
and failure/repair work uses a separately named reserve or an exact committed
sub-budget.

### P1.4 — fee profiles that protect the product wedge

The composite fee kernel is mathematically sophisticated and production rates
are correctly still zero/undecided. Its dispersion arm reduces to the familiar
`q p(1-p)` shape for a single claim. The price-free range floor charges risk at
zero price. That blocks a zero-price fee channel, but it has a serious product
cost: as a tail claim's price approaches zero, a fixed fee on payout range can
dominate the premium. Cheap tail insurance is the most credible wedge, so this
must be an experiment, not an assumed default.

Use at least three immutable experiment profiles:

- zero fee;
- dispersion-only with exact carry; and
- dispersion plus explicitly capped range floor.

Charge liveness/spam work from prepaid SOL budgets. Do not distort risk fees to
recover work costs, and do not use future fees as liveness capitalization.
Never charge split, merge, settlement, or redemption against Hoard principal.
The operator must display maximum all-in fee before signature and attribute
protocol fee, maker spread, solver prize, and work prepayment separately.

The current Polymarket documentation is useful only as a contemporary
implementation comparison: it also documents a `p(1-p)`-shaped fee and maker
rebates ([official fee documentation](https://docs.polymarket.com/trading/fees)).
It is not evidence that the same categories, rates, rebates, or economics are
right for Dragon's Clutch.

Adversarial invariants: fragmentation cannot reduce fees because carry follows
the semantic owner; complete-set translation does not change the risk fee;
identity splitting cannot improve allocation; a displayed maximum is a true
upper bound; zero-price spam remains bounded by prepaid work; and terminal
rounding has exactly one named residual owner.

### P2 — one semantic family for Clear, Shielded, and Dark

The highest vision remains valid: execution modalities should share product,
collateral, payoff, conservation, and settlement semantics. Do not make three
protocols. A future modality profile should state exactly which inputs are
public, committed, encrypted, or revealed and should emit the same typed
economic verdict consumed by the settlement owner.

Dark execution is not “encrypt the order book.” It needs a leakage model for
repeated prices, fills, timing, participation, aborts, and selected plans. The
energy-provider idea is a specialized confidential dispatch/commitment
relation with its own constraints and feasibility/optimality certificate; it
cannot be honestly represented as a generic Clutch state-price auction.

This is a parallel research contract after Clear is useful. No public lifecycle
should wait for FHE tooling, and no Dark claim should be made from a simulated
or merely private transport.

## Exact owner task packets

These packets are deliberately narrow enough to assign without creating
parallel semantic owners.

| Owner lane | Implementation packet | Must not touch |
| --- | --- | --- |
| Batch/General kernel | Specify and implement `DeclaredSurplusV1`, fixed-width arithmetic bounds, dual certificate, rank encoder, and adversarial identity/split tests. | Current action-14/action-15 rank integration until its owner lands; fees, accounts, submitter rewards. |
| Product foundation | Inventory actual per-role rent/CU, define the universal core, then implement `FoundationGraphProfileV1` and exact funding/abort/retirement joins as a V5 family. | Existing V4 market interpretation or live Product account authority. |
| Product compiler | Define `RefinementMapV1`, `RollIntentV1`, `ProductSemanticGraphV1`, and `SeriesScheduleV2`; start with exact categorical refinement before spline knot insertion. | Source observation ownership, Hoard netting, or adapter-created product facts. |
| Dealer | First permit no-asset-change LP-share transfer under exact quiescence; then specify capital rebase and potential-family contracts. | General book/rank truth, Hoard principal, active V1 policy mutation. |
| Source/Funding | Model one shared finite work epoch, subscription receipt, and exact prefix refund/reimbursement policy. | Market settlement state or assumptions about future subscribers. |
| Operator/frontend | Surface certificate type, declared surplus/gap, state-price-not-probability language, rent decomposition, maker maximum loss/exit, source contract, and roll/refinement compatibility. | Static indexes as authority or “optimal” without the exact certificate. |

## Product falsifiers and measurements

Implementation correctness is necessary but not sufficient. The next local
campaign should produce decision data, not a success-only demo:

| Hypothesis | Measurement that can falsify it |
| --- | --- |
| Smooth claims are better than categorical bins for the wedge. | Compare terminal hedge error, boundary discontinuity, source sensitivity, and all-in capital/rent at equal claim count. |
| Atomic portfolios create better execution. | Compare fill rate, declared surplus, leg risk, verification CU, and fee against legged single-Egg orders on identical synthetic books. |
| Exact optimality is attainable often enough to matter. | Measure `U/D` gap, certificate rate, solver time, and failure cases across partial, minimum-fill, AON, and Dealer books. |
| Foundation V5 is worth schema churn. | Measure active accounts, rent principal, creation transactions/CU, abort cost, and close work for each capability profile against V4. |
| Covered Dealer produces desirable liquidity. | Report executable spread/depth, utilization, donated subsidy, LP capital return, worst drawdown, and exit latency—never only volume. |
| Recurrence concentrates liquidity. | Measure maker capital reuse, roll completion, repeated-user retention, and orders per expiry; count empty Series as failures. |
| Shared Source work is capital efficient. | Compare total reserved and spent work per unique raw transition, including failure/repair, with per-market SourceWork. |
| A fee profile is viable. | Compare fill rate, solver participation, maker return, protocol revenue, and fee/premium ratio by claim price, especially the tail. |

Synthetic and historical replay can reject bad geometry. It cannot demonstrate
willingness to pay. A public devnet execution proves deployability and account
behavior, not demand or mainnet safety.

## Dependency order for the active swarm

1. Finish and integrate the already identified Product/General/Failure/Source/
   Dealer callable lifecycle spine, deleting superseded authority paths.
2. In parallel, write the exact DeclaredSurplus and FoundationGraph schemas and
   freeze their invariants; do not splice them into half-integrated actions.
3. Before public deployment, decide whether FoundationV4's rent profile is
   acceptable even for an experimental market. If not, land V5 now rather than
   migrate public state later.
4. Add exact categorical refinement and canonical rolls, then measure whether
   recurring products concentrate execution.
5. Add Dealer share transfer/quiescent epochs and the source-work reserve only
   after the current terminal and funding owners are singular.
6. Use the local validator for adversarial full-path measurements; use devnet
   afterward for provider/RPC/wallet behavior. Devnet SOL is not an
   implementation blocker.
7. Let the README, GitHub Pages, and operator UI describe the measured product:
   payoff, worst-case cash, source, executable depth, fee, certificate type,
   and exit—not internal action numbers.

## Bottom line

Dragon's Clutch is not meaningless proof machinery. It has a distinctive,
coherent asset primitive: exact fully backed basis claims, smooth bounded
payoffs, atomic portfolio clearing, exact state-price coherence, objective
source settlement, and covered liquidity. That can be genuinely useful for
recurring tail and path risk that existing spot/perpetual markets do not serve.

The present danger is different: sophisticated prototype layouts may harden
into permanent economic policy. The 18-account non-outcome foundation, the
one-axis/uniform-Series model, rigid Dealer capital, and a risk-flow heuristic
as the only clearing-quality policy are not sacred consequences of exactness.
They are liftable.

The ambitious target is a protocol where every restriction is one of three
things: a mathematical invariant, a named reviewed capacity profile, or a
versioned product choice. Nothing else should survive merely because it was
convenient in an earlier account graph.
