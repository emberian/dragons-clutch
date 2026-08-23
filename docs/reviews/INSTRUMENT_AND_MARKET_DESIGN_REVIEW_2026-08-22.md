# Instrument and market-design review — 2026-08-22

Status: **REVIEW / DESIGN DIRECTION / NO RUNTIME CLAIM**.

This review asks a different question from the runtime and architecture audits:
not merely whether Dragon's Clutch conserves atoms, but whether the instrument,
market structure, and surrounding product are worth having. It compares the
current tree with the human-authored intent recovered from local `cv` sessions.
It deliberately proposes an ambitious continuation rather than a smaller
"shipping" subset. Regulatory-filing work is out of scope.

## Executive verdict

Dragon's Clutch has built a meaningful financial primitive: a fully
collateralized family of bounded state-contingent claims, a native atomic
portfolio language, and a permissionless call-auction relation that can price
and exchange the claims as one coupled state space. The degree-zero case is an
Arrow-like finite state market. Degrees one through three replace cliff-like
bins with overlapping, locally supported B-spline claims whose payouts form an
exact partition of unity. That is a real instrument, not decorative proof work.

It is not yet a desirable venue. A venue becomes desirable when a user can name
a risk, obtain a credible price, find a counterparty, transfer or close the
position, and trust settlement. The tree has much of the settlement and
verification substrate but no demonstrated demand, production source profile,
live passive liquidity authority, wallet transaction client, or economic
criterion that proves a selected candidate is a good clearing rather than a
valid clearing. Its strongest product layers—recurring Series, payoff
compilation, structured bearer claims, ScoreV2-Q, two-window candidate timing,
and the exact smooth-price witness—are still models or designs.

The right response is not to remove the sophisticated surface. It is to make
the sophistication compositional and economically legible:

1. preserve the state-space compiler, native smooth basis, general portfolio
   relation, wrappers, recurring Series, and future Clear/Shielded/Dark
   modalities;
2. separate feasibility, price coherence, clearing quality, and solver payment
   into four independently versioned certificates;
3. turn one-off Markets into standardized recurring risk surfaces with exact
   refinement and roll operations;
4. make liquidity a first-class, fully capitalized policy family rather than an
   assumption about counterparties; and
5. validate product demand with hedge-quality, quoting, and user workflows,
   while continuing the deep correctness work.

The protocol is best described as a **compiler and exchange for bounded
state-contingent assets**, not a generic prediction site. Its most credible
initial economic wedge is recurring token-tail and path-risk protection in
stable collateral. Its larger research destination includes composable
multivariate state spaces and confidential specialized clearing such as energy
dispatch. Neither destination requires pretending that current prices are
objective probabilities or that current candidate selection is optimal.

## 1. Evidence and uncertainty boundary

### Recovered human intent

The session corpus and de-duplication problem are already documented in
[`PROJECT_INTENT_ARCHAEOLOGY_2026-08-22.md`](PROJECT_INTENT_ARCHAEOLOGY_2026-08-22.md).
This review independently inspected the relevant user messages with `cv show`
and `cv search`. The economically decisive messages were:

| Local session | Human direction recovered | Economic implication |
| --- | --- | --- |
| Codex `01a012a7-4913-7ed0-abff-241482bcb0c4` and parent `01a00a3d-5612-7253-b858-7b244522a16e` | Deposit collateral, issue a complete set over an exhaustive disjoint objective future state, forbid debt/margin/liquidation/discretionary resolution, and support token prices, ranges, crossings, and path statistics. | The original object was a fully funded state-claim system, not a sportsbook. |
| Codex parent `01a00a3d...`, message rendered near the `Did we end up implementing the full B-spline semantics?` turn | The user explicitly rejected leaving smooth claims as categorical portfolio sugar: shaped dynamics were “vital.” | Native degree-one through degree-three semantics are a product requirement, not optional mathematical ornament. |
| Codex parent `01a00a3d...`, the large Isometric comparison prompt | The protocol should be at least as general as graded range-payout and passive-liquidity designs encountered during development. | Range liquidity is a benchmark to subsume, while preserving exact collateral and a larger coefficient language. |
| Codex `01a00f44-c3fe-75e2-a89e-639b7239e9a6` | Clear, Shielded, and Dark were intended as modalities; the public Solana protocol should remain useful while Dark execution hides books and policies. | Privacy is a future execution modality over related semantics, not a reason to weaken the public system. |
| Codex parent `01a00a3d...`, energy correction rendered near message 13768 | The Dark/FHE motivation included allowing energy providers to settle an efficient plan without revealing operational information. | Confidential dispatch is a specialized optimization relation. It is broader than a state-price market and must not be collapsed into one. |
| Claude `c37f7ac1-cd60-402f-af26-a45d5b50204a` and later coordinators | The public protocol should be open, operatorless, collateral-generic, static-client accessible, and seriously verified. | DREGG is a profile; a backend, privileged matcher, or discretionary result would violate the thesis. |
| Current Codex `01a02ad0-653f-7a63-aeeb-6151f7d65ca7` | Improve everything, lift accidental restrictions, use real/local infrastructure, and spend multiple swarm cycles pursuing the sophisticated version rather than cutting the view to ship. | Capacity profiles should be compilation targets, and priority ordering must not be mistaken for scope deletion. |

`cv` stores swarm-child prompts, task notifications, and repeated `/goal`
envelopes as user-role records, so raw message counts are not economic votes.
The table relies on human text and the prior archaeology's de-duplication rather
than counting inherited prompts.

### Current-state evidence used

This review treats code and named execution evidence as stronger than prose.
The principal current sources are:

- [`CURRENT_TRUTH.md`](../../CURRENT_TRUTH.md), especially §§3–6;
- [`ARCHITECTURE_REVIEW_2026-08-22.md`](ARCHITECTURE_REVIEW_2026-08-22.md);
- [`PRODUCT_COMPILER_AND_SERIES_V1.md`](../design/PRODUCT_COMPILER_AND_SERIES_V1.md);
- [`PRICE_MEASURE_WITNESS_V2.md`](../design/PRICE_MEASURE_WITNESS_V2.md);
- [`research/score-v2`](../../research/score-v2/README.md);
- [`research/structured-claim-wrapper`](../../research/structured-claim-wrapper/README.md);
- [`research/liquidity-policy-model`](../../research/liquidity-policy-model/README.md);
- the current commit history through the capability-profile and product-model
  wave beginning at `6586507` and ending, when inspected, at `f2cb7e4`.

No deployed-market, mainnet, user-demand, production-source, audit, or formal
whole-system claim is made. Local signed validator campaigns are execution
evidence, not evidence that a public provider, solver, maker, or customer will
appear.

### Original design versus the present system

| Original economic intent | What survived or improved | What drifted or remains absent |
| --- | --- | --- |
| Fully collateralized complete claims over objective states | Protected Hoard accounting, complete-set split/merge, exact supply planes, and refusal semantics are stronger and more explicit than the early concept. | Full terminal ownership, fractional bearer credits, and protocol-wide retirement are not complete. |
| Properly shaped distributions, not only fixed bins | Native degree-one through degree-three B-spline evaluation, point resolution, exact coefficient algebra, a shape compiler, and substantial model/finite evidence landed. | Non-point smooth evidence, total fragment policy, transferable named shapes, and exact multi-span price admission are not runtime-complete. |
| A market at least as general as graded range-liquidity systems | The atomic coefficient language can express a larger bounded payoff family than one fixed range LP, and the general relation can clear portfolio orders. | The comparison system's important user feature—passive quoted liquidity—exists here only as a bounded schedule model. Algorithmic expressivity is not product superiority without executable depth. |
| One coherent distribution rather than unrelated binaries | Complete-set algebra, coupled prices, virtual conversions, and general batch verification are real. | The current score does not establish welfare or price quality; external materialized books can diverge from the coupled surface. |
| Operatorless source, clearing, settlement, and client | Public transition design, prepaid-work models, local signed execution, and static clients preserve the direction. | Production source identity, public availability, live keeper economics, wallet construction, and a release-bound chain-reading client remain STOPs. |
| Collateral-generic protocol with DREGG as dogfood | Realm and kernel semantics keep DREGG out of the economic core; collateral adapter profiles are now explicitly versioned. | A released real Realm profile does not exist, and practical adapter support is narrower than the conceptual model. |
| Clear, Shielded, and Dark modalities | Verify-not-find relations, fixed artifacts, and exact public semantics are appropriate Clear reference foundations. | No private order relation, confidentiality mechanism, leakage theorem, or Dark deployment exists. |
| Confidential efficient planning for energy providers | The generic certificate, protected-pool, and settlement vocabulary could support the output of such a relation. | No energy dispatch/commitment relation exists. Current Clutch markets alone do not solve confidential planning. |
| Static, approachable, self-verifying frontend | Offline Glass, Operator benches, and claim-honest static content exist. | The product is still presented more readily to protocol engineers than to a hedge buyer or maker. |

The system therefore did not betray the original instrument. It overdeveloped
the correctness and transition substrate before completing the product
compiler, liquidity, privacy, and user-facing joins. Some new machinery also
introduced genuine scar tissue—ScoreV1, a single candidate deadline,
Terms-bound source recurrence, maximum-width storage, and replay-unsafe
deletion—but the current successor designs identify rather than conceal it.

## 2. What financial instrument actually exists

Let an authenticated evidence program produce a bounded state `x`. A Market
freezes nonnegative basis functions

```text
phi_0(x), ..., phi_(n-1)(x)
```

at denominator `D`, with the exact complete-set identity

```text
sum_i phi_i(x) = D.
```

One atom of Egg `i` pays `phi_i(x) / D` collateral under the market's exact lot
or credit policy. One complete set therefore pays one collateral atom in every
admitted state. Splitting collateral creates equal units of every Egg; merging
equal units returns collateral before resolution. Hoard principal backs that
identity and is unavailable for fees, rent, bounties, or treasury use.

This yields four related instruments rather than one:

### Degree zero: finite state claims

The state partition is exhaustive, disjoint, ordered, and canonical. Exactly
one categorical Egg pays in a resolved state. Economically these are bounded
Arrow-Debreu-style state claims. A vector of their prices that sums to one is a
state-price vector, subject to fees, collateral denomination, risk premia,
liquidity, and market power.

This is the cleanest profile for digitals, ranges, and explicit discrete
regimes. Its weakness is a payout cliff at a boundary: an economically tiny
move can transfer the entire payout between adjacent states.

### Degrees one through three: smooth local-support claims

Open-clamped B-spline Eggs overlap. A resolved point usually pays several
nearby Eggs, with exact nonnegative integer weights summing to `D`. Increasing
degree produces smoother local payoff dynamics while preserving a complete
set. These claims are economically useful when “how close” matters or when a
hard state boundary creates manipulation or hedging error.

They are not just prettier bins. Their tradable span is a finite spline space;
coefficient vectors represent every payoff in that space exactly. A shaped
payoff outside the span may be compiled only with an explicit approximation
certificate and named error norm.

Smooth prices also require more than simplex normalization. At degree two or
three, an arbitrary nonnegative price vector summing to one need not be the
expectation of the basis under any nonnegative measure. The current V1b gate is
a sound outer screen, not a complete membership decision on multi-span grids.
The exact five-claim counterexample in `PRICE_MEASURE_WITNESS_V2.md` shows a
nonnegative payoff `(3x - 1)^2` acquiring a negative price while V1b accepts.
The new measure-witness model supplies the right certificate shape but is not
yet an SBF authority.

### Atomic coefficient portfolios

A nonnegative coefficient vector over the native Eggs defines a bounded payoff
shape. The general clearing relation can admit the whole vector as one order,
value it with one exact dot product and one rounding boundary, reserve it, and
settle it without leg risk. This is economically more important than presenting
each Egg as an isolated prediction token: it lets users trade “crash,” “tent,”
“range,” or “capped call” as one intention.

Settlement currently credits separable Eggs. An atomic order is therefore not
yet a transferable named asset. That is an honest and useful separation: order
atomicity solves execution risk; a wrapper solves custody, transfer, venue, and
integration needs.

### Structured bearer wrappers

The wrapper model content-addresses a canonical coefficient claim, compresses
its complete-set floor into cash, flattens composition to native Eggs, and
backs every Token-2022 wrapper atom with the corresponding native position.
This can turn “a tent over this Series instance” into a wallet-transferable
asset, a generic venue instrument, or lending-vault collateral. It also creates
new mint authority, custody, retirement, and fragmentation surface. The model
is sophisticated and useful; no live SBF wrapper exists.

### The precise taxonomy

Dragon's Clutch is closest to a fully funded family of bounded exotic options
or state-contingent insurance claims with a native combinatorial call auction.
It differs from ordinary options because it has no margin writer, liquidation,
or continuous delta-hedging promise. It differs from a binary prediction venue
because one basis and one collateral unit support many payoff shapes. It
differs from an AMM because the current venue verifies submitted batch
candidates and has no live endogenous cost function. It differs from insurance
because no protection exists until somebody fully funds or sells the claim.

## 3. What the system accomplishes, component by component

| Component | Economic role | Actual present status | Product consequence |
| --- | --- | --- | --- |
| Eggcrate and protected pools | Fully funded issuance, split/merge, supply and payout conservation | Host-tested kernel pieces plus separately named model proofs and finite correspondence evidence | This is the strongest reason the instrument is trustworthy in principle. The adapter/runtime boundary remains real. |
| Native degree-0–3 basis | Defines categorical or smooth bounded state claims | Exact evaluator host-tested; point resolution and exact-lot exits SBF-executed in focused local evidence; non-point smooth semantics and total fragments remain open | The crown-jewel instrument exists at a bounded point-settlement profile, but the full path/TWAP/occupation promise is not live. |
| Shape compiler | Converts human payoff goals into exact/certified coefficients | Host model | The useful user abstraction exists mathematically but not in market creation or wallet flow. |
| General clearing relation | Verifies pages, single-Egg and portfolio orders, virtual complete-set conversion, exact allocations, fees, and settlement | Broad local-bank SBF evidence, current source unsealed | A real coupled venue substrate exists. It selects the best valid submitted checked candidate under the frozen policy, not a globally optimal clearing. |
| ScoreV1 | Ranks checked candidates | Runtime legacy/experimental policy | Economically unsound for public control because complete-set wash and key fragmentation can inflate it. |
| ScoreV2-Q | Measures representation-invariant noncash risk range `rho(d)` and demotes complete-set churn | Exact host model only | A much better identity-free flow objective, but it deliberately does not certify price quality or welfare. |
| Two-window candidate lifecycle | Separates submission from verification and pre-funds exhaustive candidate work | ADR and executable model only | Repairs submission fairness, cleanup enumeration, and withheld-work liveness without privileging a keeper. |
| Source/archive and resolution | Authenticates observation custody and resolves deterministic evidence | Captured real provider programs executed locally over synthetic local guardians/observation; no production release | Strong integration progress; no public data-availability or production identity follows. |
| Product compiler, Template, Instance, Series | Standardizes state semantics and creates recurring markets | Host model only; current Feed/Terms architecture blocks recurrence and cross-Realm sharing | This is the missing bridge from protocol machinery to a repeatable product. |
| Liquidity policy | Compiles bounded quote schedules and maintains exact reserve/inventory/fee accounting | Host model only, explicitly no live authority | There is no passive liquidity product today. The model refuses leverage and uncapitalized promises correctly. |
| Wrapper | Makes a shaped claim a transferable bearer asset | Host models only | External composability remains a plan; settlement Eggs themselves are transferable only individually. |
| Static Glass/Operator | Explains and exercises protocol state | Offline/static clients and loopback benches; no release-bound wallet client | A developer can inspect local evidence, but an ordinary user cannot yet execute the product. |

The machinery is unusually advanced relative to the user workflow. That is not
a reason to delete it. It means the next deep work should join the components
around one exact economic path and make every reusable abstraction real.

## 4. Is the instrument useful and desirable?

### Genuinely useful properties

1. **Bounded loss without liquidation.** A buyer knows the worst cash outlay
   and terminal payout. A seller cannot create an unfunded liability.
2. **Many shapes from one funded basis.** One state surface supports crash,
   range, tent, digital, and capped directional views without separately
   capitalizing unrelated binaries.
3. **Path-risk products for assets without options markets.** Drawdown,
   volatility-regime, crossing, and sustained-threshold claims may express
   risks that spot and perpetuals handle poorly.
4. **Atomic shape execution.** Portfolio orders eliminate leg risk inside the
   admitted relation.
5. **Objective, reusable settlement.** Many Series instances and statistics
   can share authenticated raw observations if SourcePlane V3 lands.
6. **No operator continuity assumption.** Prepaid finite work and public
   transition rights can make settlement independent of the founding team,
   subject to network inclusion and a correct frozen cost envelope.
7. **Agent-native semantics.** Exact artifacts and self-verifying state are
   well suited to machine traders that can compile and check payoff programs.

### Real economic costs

1. **Full collateral is expensive.** It removes default and liquidation risk
   by foregoing the capital efficiency of margin. For common, liquid options,
   centralized or margined competitors may quote tighter.
2. **A new basis fragments attention.** Sixteen outcome tokens, multiple
   horizons, multiple degrees, and arbitrary user-created partitions can
   disperse already scarce liquidity.
3. **Wrong-way collateral can erase practical protection.** A crash claim
   collateralized by the crashing community token may remain nominally solvent
   but fail the buyer's purchasing-power objective. DREGG is legitimate
   dogfood, not the default economic answer. Stable-collateral Realms are the
   credible protection profile.
4. **Oracle and path semantics are part of the product.** A “drawdown hedge” is
   only as good as its frozen sampling, confidence, gap, interval, and repair
   rules. Labels cannot hide missing observations.
5. **External Egg markets may violate coupled coherence.** Materialization
   enables composability, but unrelated spot books need not preserve complete-
   set or smooth measure coherence after fees and route costs.
6. **Expiry-specific products repeatedly need new liquidity.** Series automates
   creation and data work; it does not magically roll orders or maker capital.

### Demand verdict

The most credible wedge is a standardized, recurring, stable-collateral tail
risk surface for thin or community tokens:

- four to eight states or a small smooth basis;
- terminal and maximum-drawdown Series at fixed daily/weekly horizons;
- named crash/range/tent shapes compiled into atomic orders and optional
  wrappers;
- treasury- or sponsor-capitalized quote schedules with visible maximum loss;
- one authenticated source family and reusable raw windows; and
- a static client that leads with payoff and worst-case cash, not protocol
  account vocabulary.

This wedge has an intelligible buyer: a holder or treasury seeking bounded
crash/path protection where no conventional options book exists. It has an
intelligible seller: a sponsor or maker willing to commit stable collateral for
premium. It also creates recurring observations from which solver and maker
behavior can improve.

Demand remains unproved. A technically excellent state market can still have
no natural seller, no buyer at the seller's required premium, or too little
volume to justify transaction and opportunity costs. Synthetic simulation can
falsify bad market structures; it cannot establish willingness to pay.

## 5. Market-structure review

### The coupled frequent batch auction is a good fit

A basis market should not be decomposed into unrelated books if the protocol
wants a coherent state-price surface. One batch can:

- price the basis together;
- use split/merge as virtual inventory;
- execute shaped portfolios atomically;
- reduce latency races relative to continuous price-time matching; and
- verify exact conservation independently of the solver's search algorithm.

The verify-not-find split is excellent architecture. It permits exhaustive,
LP, heuristic, or future succinct solvers to compete against one consensus
relation. It should be preserved.

### Feasibility is not clearing quality

The current relation answers “is this candidate valid?” Candidate ranking must
then answer several different questions which should not be collapsed:

1. **Risk flow:** how much noncash contingent exposure changed hands?
2. **Trader welfare:** how much executable surplus relative to signed limits
   did the candidate realize?
3. **Price quality:** is the state-price vector coherent and, among coherent
   vectors, how is a marginal price selected?
4. **Fair allocation:** how are ties and marginal fills distributed without a
   fragmentation advantage?
5. **Operational cost:** how much verification, storage, conversion, and
   settlement work did the candidate create?

ScoreV1 mixes manipulable flow, price-dependent weight, distinct public keys,
and churn. ScoreV2-Q correctly strips the complete-set component and refuses to
pretend keys are persons. But `rho(d)` alone can prefer a large low-surplus
cross over a smaller high-surplus one, and it intentionally says nothing about
whether `p` is informative.

The successor should use a lexicographic certificate stack rather than one
clever scalar:

```text
valid relation
  -> coherent price certificate
  -> certified welfare or explicitly named flow objective
  -> canonical marginal allocation
  -> minimum quotient-free churn / work
  -> full candidate digest
```

For the restricted divisible linear fragment, an exact primal/dual optimum
certificate is a worthy target. That profile could honestly say “optimal under
Relation/Objective Vn.” The general heuristic profile should continue to say
“best valid submitted candidate.” Both can coexist as capability profiles.

### Smooth-market price coherence must become mandatory

Degree-zero and degree-one simplex prices have the required representing-
measure interpretation. Multi-span degree-two and degree-three markets need the
new exact measure witness or a deliberately sufficient inner price lattice.
Leaving the V1b outer approximation as the public acceptance boundary would
permit an executable negative-price nonnegative payoff.

This certificate is independent of clearing optimality. A coherent but poor
price and an incoherent high-volume price are different failures and should
produce different refusals and diagnostics.

### A state-price vector is not automatically a probability forecast

The docs sometimes call normalized prices an “implied probability
distribution.” That is too strong without assumptions. Prices reflect risk
aversion, collateral opportunity cost, wrong-way collateral, fees, maker
inventory, order constraints, and illiquidity. In smooth bases, the vector is a
set of basis moments and may not uniquely identify a continuous distribution.

Public language should use **state-price surface** or **market-implied state
weights** by default. A probability projection may be displayed only with its
normalization/model assumptions and non-uniqueness. This correction makes the
product more credible, not less interesting.

## 6. Liquidity and maker economics

### What complete-set issuance helps

A maker can split collateral into one unit of every Egg and sell whichever
states traders demand. Unsold Eggs plus cash proceeds remain a bounded
inventory. Virtual split/merge lets the auction reason about that conversion
without executing gratuitous token operations per fill. This is a real capital
and coherence advantage over separately funded binaries.

It does not create demand or remove inventory risk. A seller of crash Eggs is
short crash protection; full collateral prevents default but does not protect
the seller's equity.

### The bounded schedule policy is a strong first liquidity family

The landed model compiles hard ranges, triangles, or exact coefficient vectors
into at most eight ordinary portfolio-shaped quotes. It maintains conservative
encumbrance

```text
E(q,s,B) = B + max_i(q_i + s_i)
```

without buy/sell netting, leverage, or future-fee assumptions. This is more
honest than describing a mutable LP curve whose solvency is not proved.

Its current integration cost is high. If reserve cash and delivery Eggs must be
separately preowned to avoid counting one collateral atom both as tranche
reserve and Hoard backing, the maker may fund more gross capital than the
economic worst case. A live integration should investigate an atomic,
semantically explicit reclassification transition:

```text
tranche cash -> Market Hoard principal -> tranche-owned complete-set Eggs
```

The transition must change the reserve representation and re-prove withdrawal
encumbrance; it cannot count both forms simultaneously. If that theorem and
runtime authority are too costly for one profile, the separately funded route
remains an honest capacity profile.

### A cost-function maker remains worthwhile research

A future continuous policy can offer always-available depth if it has one exact
convex potential, endpoint-difference charges, a finite worst-case-loss
certificate funded before trading, exact complete-set invariance, and
telescoping rounding. It should be another policy family, not an overlay that
silently disagrees with batch prices.

The ambitious system should support both:

- scheduled capital with transparent finite commitments; and
- a proof-constrained convex cost-function maker for products where continuous
  availability justifies the capital and complexity.

### Bootstrap without fake volume

Maker rebates, points, emissions, or public-key counts must not manufacture the
appearance of demand. Better bootstrap instruments are:

- sponsor-funded quote schedules with a declared maximum loss;
- trader-funded RFQ solver budgets;
- recurring Series that reuse market-making code and capital policies;
- canonical crash/range wrappers that reduce discovery cost; and
- transparent reports of quoted depth, executable spread, fill rate, and
  realized hedge error.

## 7. Solver incentives

The protocol needs three separate payments:

1. **Verification and cleanup reimbursement.** Pay exact bounded monotone work
   from prepaid candidate/epoch budgets. The two-window lifecycle model gets
   this right. The amount should not depend on verdict direction or reported
   volume.
2. **Candidate discovery reward.** If a Series sponsor or order explicitly
   funds a prize, pay for a checked improvement under a frozen objective. A
   primal/dual certificate is the cleanest basis for a welfare reward.
3. **Maker compensation.** Pay makers through signed spread/fee economics, not
   by relabeling them as solvers or keepers.

A percentage of `rho`, owner count, or claimed volume invites wash. A bounty
for improvement over a manipulable baseline also invites the sponsor or solver
to worsen the baseline. A defensible discovery prize should therefore be
bounded ex ante and earned by one of:

- a checked optimum certificate for the admitted relation;
- a checked objective improvement over a canonical deterministic fallback;
- or a fixed winner prize independent of the candidate's self-reported volume.

The winner prize is not liveness capital. Verification and finalization remain
prepaid even when no solver discovers a useful candidate.

## 8. What Series and wrappers change

### Series turns research machinery into a product

Content-addressed Templates and deterministic Instances create standardization.
Finite prepayment makes repeated creation honest. Identical Instances from two
Series converge rather than fragment. Source-only raw windows can be reused by
terminal, drawdown, volatility, and other statistic children.

This improves operational cost and user habit, but source sharing is not
liquidity sharing. Every expiry still needs orders, maker commitments, rolls,
and close paths. The Series layer should therefore compile not only Instances
but also canonical **roll intents** and liquidity-blueprint activations.

### Wrappers can concentrate or fragment liquidity

A canonical wrapper gives a named payoff one mint, one balance, and generic
Token-2022 composability. Ten slightly different wrappers create ten new books.
Permissionless creation is compatible with product discipline if wrapper
identity is content-addressed and the client distinguishes:

- canonical Template-published shapes;
- exact user-defined shapes;
- approximated shapes with certificates; and
- wrappers with no recognized venue or liquidity.

The protocol should not require wrappers for native smooth settlement. It
should offer them when transfer, custody, lending, or external venue support
justifies the authority and rent surface.

### Exact refinement is the largest unexploited composability opportunity

Standardized Series should support exact maps between compatible state spaces:

- a coarse degree-zero claim equals the sum of its fine child claims when the
  source, window, statistic, and boundaries are exactly nested;
- B-spline knot insertion gives an exact linear map from a coarse basis into a
  refined basis; and
- a coefficient payoff may be re-expressed under that map without changing its
  pointwise payoff.

An authenticated refinement converter could migrate maker inventory, user
positions, and wrapper backing between coarse and fine Instances without a
market sale. The converter must freeze rational scaling, lot rules, backing,
and exact identity; incompatible horizons or statistics must refuse. This can
make ambition improve liquidity rather than fragment it.

## 9. Sophisticated extensions worth pursuing

### Product-space compiler

Extend Templates from one statistic axis to bounded products such as terminal
price × drawdown or relative performance × volatility regime. Tensor products
of exact partitions of unity remain partitions of unity, but basis count grows
multiplicatively. The research problem is not merely evaluation: it is sparse
payoff representation, exact price coherence, account/rent profiles, and a
certificate that no omitted state can mint a liability.

A promising direction is a typed sparse product circuit which lowers to a full
bounded basis only at the liability boundary. Factorization may compress
descriptions and solvers, but it must never make the minted state partition
non-exhaustive or permit two semantic owners for one payout fact.

### Cross-Series portfolios and rolls

Atomic orders should compose claims across compatible expiries and Instances:
calendar spreads, roll trades, and multi-horizon tail ladders. These are not
single-Market complete sets, so each Market Hoard remains segregated. The
portfolio relation can atomically exchange independently backed assets without
netting their Hoards.

### Separately capitalized cross-market risk vaults

Base Markets should keep non-netted Hoards. A higher-level vault may hold Eggs
from several Markets and issue fully backed portfolio shares. If it wants to
write across-market risk with less than gross collateral, it needs a frozen
joint-state worst-case certificate and its own reserve; it may not borrow
claimant principal from underlying Hoards. This preserves base solvency while
allowing sophisticated portfolio capital.

### Price-measure and welfare certificates as first-class artifacts

Make certificate type part of the capability profile:

| Profile | Price certificate | Clearing-quality certificate |
| --- | --- | --- |
| categorical/degree one | exact simplex | best-submitted ScoreV2-Q or exact LP dual |
| single-span smooth | exact local Hausdorff checks | same choices |
| multi-span smooth | exact measure witness | same choices |
| large general book | exact streamed witness or succinct verifier | certified objective named by the profile |

This turns profiles into honest compilation targets rather than arbitrary
feature cuts.

### Agent-facing strategy artifacts

Expose canonical read-only artifacts for:

- payoff basis and worst-state payout;
- source and gap semantics;
- state-price and measure-witness diagnostics;
- executable order limits and fee bounds;
- candidate relation/objective/certificate identity; and
- roll/refinement maps.

Agents should construct unsigned transactions or plans from these artifacts;
they must not need a privileged Dragon service.

## 10. Relationship to the original Dark energy intent

The current public Clutch is not a confidential energy-dispatch protocol.
Issuing claims over an energy price or dispatch outcome would capture only the
settlement layer. The recovered intent was stronger:

```text
private provider bids and operational constraints
  -> specialized confidential optimization relation
  -> efficient feasible plan and settlement quantities
  -> bounded public disclosures and correctness certificate
```

That relation may include cost curves, ramp limits, outages, inventory,
commitment constraints, hedge books, or network constraints which providers do
not wish to reveal. A Dark implementation must also analyze leakage through
the chosen plan, prices, timing, participation, repeated queries, and aborts;
encrypting the inputs is not a privacy theorem.

Dragon's Clutch contributes useful shared structure:

- exact bounded asset and settlement semantics;
- verify-not-find candidate competition;
- prepaid finite verification/finalization;
- versioned source/statistic/artifact identities;
- protected pools and exact integer accounting; and
- a transparent reference modality against which Shielded/Dark results can be
  compared.

But the energy optimizer needs its own admitted relation and feasibility or
optimality certificate. It should be a specialized compiler target, not a
generic “arbitrary dark computer,” and it should not import historical project
code without a later explicit provenance/license decision. The right ambitious
architecture is one semantic family with Clear, Shielded, and Dark execution
profiles—not a claim that today's public SBF program already implements the
Dark relation.

## 11. Scar tissue: keep, replace, and reframe

### Keep

- one semantic owner for every persisted fact;
- strict Hoard/liveness/fee/rent separation;
- exact integer units and one named rounding boundary;
- internal balances with on-demand Token-2022 materialization;
- verify-not-find clearing with public candidate competition;
- native degree-zero through degree-three semantics;
- atomic portfolio orders plus optional wrappers;
- deterministic Template/Instance/Series identity;
- refusal on source, collateral, price, or evidence ambiguity; and
- capability profiles as exact deployable artifacts.

### Replace or supersede

- ScoreV1 with a quotient-invariant objective plus separate price/welfare
  certificates;
- the shared submission/verification deadline with the two-window lifecycle;
- the Terms-bound singleton Feed with reusable SourcePlane V3;
- caller-projected 64-bit market nonces with full Instance identity;
- V1b-only multi-span smooth price admission with the exact measure witness;
- one-off hard-coded Friday products with compiler-created recurring Series;
- fixed maximum-width ClearWork serialization with active-width canonical
  storage; and
- deletion routes without counted children and monotone identities with the
  replay-safe retirement successor.

### Reframe, not delete

- `MAX_OUTCOMES = 16` and capability ELFs are current deployment profiles, not
  conceptual limits;
- per-Market non-netted Hoards are intentional safety, while separately funded
  higher-layer vaults may research portfolio capital efficiency;
- exact terminal/rent work is invisible product infrastructure, not wasted
  machinery;
- fee geometry is an experimental family until live behavior selects a rate;
  it should not be advertised as user value by itself; and
- Direct and General clearing should share semantic artifacts and compile into
  different cost/capacity profiles rather than evolve as two unrelated truths.

## 12. Machinery without demonstrated demand

The following work is technically legitimate but has no demonstrated customer
pull yet:

- degree-two/three smooth measure certificates;
- generic structured wrapper factories;
- several candidate/clearing generations;
- elaborate fee-base and revenue allocation policies while runtime fees remain
  zero;
- deep release-manifest and terminal-account machinery; and
- broad general-order capacity beyond a first recurring product family.

The conclusion is not “stop.” Each item either protects correctness or enables
the intended advanced surface. The conclusion is to bind every next increment
to a product falsifier:

| Technical work | Product falsifier it should answer |
| --- | --- |
| Smooth basis and witness | Does a smooth claim materially reduce boundary risk or hedge error relative to categorical bins? |
| Wrapper | Does one named transferable shape reduce transactions/discovery or unlock a real external integration? |
| General clearing | Do atomic portfolios clear more useful risk or tighter all-in prices than legged books? |
| Series | Do repeated standardized windows concentrate maker/solver reuse rather than multiply empty markets? |
| Liquidity policy | Can a sponsor quote visible depth at an acceptable fully funded worst-case return? |
| Fee policy | At what fee do executable spread, fill rate, maker return, and protocol contribution remain acceptable? |
| Dark relation | What exact information remains hidden under repeated execution, and can the public certificate prove the intended plan property? |

## 13. Three-day ambitious research/implementation program

This is a dependency order, not a scope reduction.

### Wave A — repair market meaning

1. Promote the exact multi-span smooth price witness into a fixed-layout Rust
   checker, derive its arithmetic bounds, and integrate it with the separate
   submission/verification lifecycle.
2. Promote ScoreV2-Q only alongside a named price-quality rule and an explicit
   decision for the restricted exact-welfare profile.
3. Implement the two-window candidate index, prepaid work economics, and
   replay-safe counted retirement as one versioned lifecycle.
4. Measure active-width ClearWork and capability profiles without weakening the
   full relation.

### Wave B — turn markets into products

1. Land SourcePlane V3 ownership: source-only head, reusable raw pages, exact
   windows, statistic children, leases, and retention.
2. Land Template/Instance/Series codecs with full Instance identity and finite
   segregated funding.
3. Join terminal and drawdown compiler outputs to current Terms/Market creation,
   then remove the lossy compatibility projection in the successor account
   generation.
4. Make the payoff compiler construct an actual atomic order and display exact
   worst-state payout and error certificate.
5. Implement one canonical structured-claim wrapper path, including complete-
   set compression, only after the native position path is joined.

### Wave C — make liquidity and use observable

1. Promote the bounded schedule-tranche authority with explicit cash-to-Hoard-
   to-Egg reclassification or honestly separate funding.
2. Add canonical roll and exact refinement intents for compatible Series.
3. Run historical local source replays and synthetic books for terminal and
   drawdown products; measure hedge error, quoted depth, executable spread,
   fill rate, solver success, and maker worst-case return.
4. Exercise the complete source → Series → order → candidate → settlement →
   wrapper/exit path on the local validator with fresh ephemeral local signers.
5. Put that path in the static client with exact unsigned transaction
   construction and release-manifest binding.

### Parallel research wave — expand the frontier

1. Specify exact nested partition and B-spline knot-refinement converters.
2. Define a bounded multivariate Template and sparse product-payoff circuit.
3. Define the exact restricted clearing objective and primal/dual certificate.
4. Define the proof-constrained convex cost-function maker.
5. Write the specialized confidential energy-planning relation and its leakage
   model as a fresh design, without importing historical code.

## 14. Go/no-go questions after the swarm window

The project should pop its head up after the three-day wave and answer these
with evidence:

1. Can a user create, understand, trade, transfer or close, and redeem one
   recurring tail/path product without a privileged service?
2. Is the selected price coherent, and is the clearing-quality claim named and
   actually certified?
3. Can a maker state its exact maximum loss, capital encumbrance, fee return,
   and exit path?
4. Do Series, wrappers, and refinement reduce coordination cost or merely add
   more empty identities?
5. Does smooth settlement improve a measured hedge/manipulation objective over
   categorical bins?
6. Does the whole local lifecycle survive adversarial restart, withholding,
   rollback, transfer, burn, and cleanup?
7. Which restrictions are mathematical, which are Solana artifact profiles,
   and which are only historical layout accidents?
8. What piece of the Dark energy relation can be specified and checked without
   making a privacy or optimality claim the evidence cannot support?

## Bottom line

Dragon's Clutch is meaningful because it makes a finite state space itself the
asset substrate: fully funded basis claims, exact payoff programs, coupled
clearing, and objective settlement. Its native smooth construction is a
substantive extension of ordinary categorical outcome tokens. Series and
wrappers can turn that substrate into recurring, transferable products.

The protocol is useful where bounded path or tail exposure is unavailable or
where liquidation/default risk matters more than collateral efficiency. It is
desirable only if standardized products attract real buyers and fully funded
sellers at executable prices. No amount of verification can supply that
counterparty, and no amount of volume can repair an incoherent price or
unfunded obligation.

The ambitious direction is therefore justified: preserve the full compiler,
smooth basis, general relation, liquidity families, wrappers, multivariate
research, and Clear/Shielded/Dark horizon. Make each layer carry its exact
certificate and cost. The next standard is not “more code” or “ship a tiny
market”; it is a joined system in which the sophisticated semantics produce a
better hedge, a better price claim, a better maker contract, or a genuinely
new confidential coordination capability.
