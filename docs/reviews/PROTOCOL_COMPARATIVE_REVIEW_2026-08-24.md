# Comparative protocol and market-architecture review — 2026-08-24

Status: **RESEARCH REVIEW / NO RUNTIME CLAIM / NO EXTERNAL ACTION**.

This review compares the Dragon's Clutch source and design tree through
`7163a900` with official primary material for Raydium, Meteora, Gnosis
Conditional Tokens, Polymarket, Kalshi, Pyth, and Isometric Technical Overview
v4. It does not promote a release, deployment, proof, economic demand claim, or
live-liquidity claim. No competitor code was copied. No runtime source was
changed for this review.

## Executive thesis

Dragon's Clutch is not a worse spot AMM waiting to acquire bins. It is a
compiler, fully funded issuer, exchange, and resolver for bounded
state-contingent assets. Its native finite basis, exact coefficient portfolios,
complete-set accounting, best-valid-candidate relation, recurring Series,
prepaid work, and separated Source/Failure authorities are a more appropriate
semantic foundation for contingent claims than importing `x*y=k`, CLMM ticks,
or DLMM bins.

The project is nevertheless behind mature venues at the surfaces that make a
sound protocol usable: discovery, quoting, transaction construction, order
lifecycle, liquidity-position inspection, execution status, and documented
recovery. Those are not cosmetic gaps. They determine whether a trader or
maker can safely use the instrument.

The highest-value direction is therefore:

1. finish the canonical Product -> Source -> Root/Link -> General/Dealer
   authority path and real custody before adding another pricing family;
2. expose a release-bound, finalized, hostile-decoded operator surface with
   typed action geometry, quotes, transaction builders, and lifecycle events;
3. generalize complement crossing from binary Yes/No to complete native
   portfolios, so complementary demand can mint or merge backing inside one
   atomic candidate;
4. promote a genuinely capitalized Dealer and the already selected exact
   quadratic facility, with explicit capital ownership and withdrawal rules;
5. treat Pyth as a hostile pull-source transport whose feed, release, schedule,
   confidence, staleness, and window are frozen by Product Terms; and
6. reject Isometric's dynamic per-bin LMSR and Gaussian issuance as currently
   specified. They do not establish one integrable cost potential, a stable
   loss bound under LP reconfiguration, or a portfolio liability bound.

## What Dragon's Clutch actually offers

The economically meaningful object is a fully collateralized claim family over
an immutable, bounded objective state space:

- a `Realm` selects collateral without hard-coding DREGG;
- Product artifacts compile an exhaustive, ordered canonical claim domain;
- degree-zero claims form disjoint categorical states, while degree-one through
  degree-three native B-spline Eggs are nonnegative, locally supported, and
  form an exact integer partition of unity at the named rounding boundary;
- exact coefficient portfolios express shaped payoffs over that common basis;
- complete-set split and merge turn collateral into or out of one unit of every
  native liability without relying on a counterparty;
- the General relation checks bounded order pages, simplex prices, portfolio
  limits, virtual complete-set conversions, fees, and asset closure, then
  selects the best valid submitted candidate under the frozen policy;
- Series and Funding prepay repeated creation and finite work rather than
  assuming future fees will keep the protocol alive;
- Source owns authenticated observations and windows; Failure owns explicit
  recovery and terminal paths; and static/operator clients are untrusted
  projections of those onchain owners.

This is meaningful because a single coupled state space can support categorical
contracts, ranges, tents, smooth distributions, and atomic coefficient baskets
without fragmenting each user-facing payoff into an unrelated pool. A price
simplex is a collateral-consistent vector, not automatically an objective
probability. A checked candidate is feasible and policy-ranked, not necessarily
globally optimal.

The current limitation is product completion, not lack of mathematical surface.
The snapshot inspected here has a durable current Series bootstrap through
action14 and a hostile-reopen action15 input, but not the complete live
Product/Source/Root/Link/Failure/Dealer join. The selected liquidity facilities
remain models without live custody. Therefore the honest description is
"sophisticated instrument and execution substrate in active integration," not
"finished venue."

## Comparative map

| System | Canonical mechanism | Capital and trust boundary | What Dragon's Clutch should learn |
| --- | --- | --- | --- |
| Raydium CPMM/CLMM/routing | Pairwise spot curves; range liquidity; a pure CPI router delegates math to child pools | LP token reserves price the spot swap; routes and quotes are constructed from indexed pool state; each child program owns its math and rounding | Adopt typed SDK/IDL/API, pool/action discovery, quote freshness, explicit account geometry, route limits, and atomic orchestration. Do not import pairwise spot invariants as claim prices. |
| Meteora DLMM | Liquidity is placed in discrete price bins; swaps walk bins; fees and LP positions are exposed through rich builders and inspection APIs | LPs own bin-range positions and bear inventory/reconfiguration risk; clients discover active bins and required bin arrays | Adapt range-selection UX, per-position capital visibility, exact target-account discovery, close/claim builders, and compute/rent previews. Keep bins as a quoting strategy, not the semantic claim basis. |
| Gnosis Conditional Tokens | Collateral-backed positions split and merge only through valid disjoint partitions; deeper conditional positions burn shallower ones | The conditional-token contract owns collateral and token accounting; an external oracle prepares/reports payouts | Generalize its excellent partition/composition UX to Dragon's Clutch's richer basis. Preserve DC's stricter canonical state compilation and source/failure lifecycle. |
| Polymarket CTF + CLOB + UMA | Fully backed Yes/No tokens; offchain signed-order matching; onchain atomic normal, mint, or merge settlement; optimistic resolution | The exchange operator orders matches but cannot invent price or unauthorized size; UMA proposers/disputers and adapter/admin powers remain explicit trust boundaries | Adapt signed intent lifecycle, complement crossing, balance reservation, order flags, status/retry UX, and atomic settlement. Do not adopt binary-only expressiveness or hide operator/resolution powers. |
| Kalshi | Central limit order book with an explicit initialized/active/inactive/closed/determined/disputed/amended/finalized lifecycle and market-specific rules | A regulated centralized exchange controls listing, pauses, matching, determination, and disputes | Adopt its legible lifecycle/status schema, close-versus-expiry distinction, reactivation cancellation semantics, and visible settlement timer. Keep DC permissionless and chain-derived. |
| Pyth | Permissionless pull updates: caller fetches an authenticated update, submits it, and consumes it, with feed identity, confidence, and staleness checks | Publishers and Pyth aggregation own the observation; Hermes transports bytes; the consuming protocol owns suitability, schedule, window, and refusal policy | Adopt update-plus-consume ergonomics and explicit confidence/staleness handling. Never let provider brand, Hermes, or an indexer define Product truth. |
| Isometric v4 | Proposed continuous-range market using per-bin dynamic-liquidity LMSR, an additional sigmoid entry curve, Gaussian payouts, single-sided LP, margin/liquidation, and fee-funded insurance | The document assigns mutable economic parameters and oracle fallbacks to program/governance layers, but does not give a complete liability or LP reconfiguration invariant | Retain the user insight—graded ranges and understandable passive liquidity—but reject the economic equations as an authority until integrability, solvency, exact arithmetic, and capital ownership are specified and proved. |

## What to adopt from Raydium and Meteora

### Operator surfaces, not spot semantics

Raydium's routing documentation makes a strong ownership choice: the router is
a pure orchestrator. It chains child-program calls and final slippage checks,
while each CPMM/CLMM/Stable child owns its curve, fee, and rounding. Its
integration surface separately documents discovery, quoting, split/multi-hop
routing, address lookup tables, congestion, account blocks, and transaction
assembly. The SDK/API surface offers REST, typed SDK, Anchor IDL, and CPI paths.

Dragon's Clutch should use the same separation:

```text
finalized canonical snapshot
  -> family-owned quote/candidate projection
  -> pure action/route planner
  -> exact per-action account and limit contract
  -> wallet-signed transaction
  -> hostile onchain reauthentication and atomic execution
```

The planner must not become a second semantic owner. It should report the
release key, slot, source account digests, action family/version, required and
optional roles, writable/signing flags, address-lookup requirements, compute
estimate, present rent and prepaid work debit, candidate expiry, and exact
refusal reason. Quotes need an observed slot and expiry/staleness boundary, not
an evergreen JSON number.

Meteora's strongest lesson is position operations. Its SDK exposes active-bin
inspection, bins around a range, per-position balances and fees, required bin
arrays, exact-in/exact-out quote results, price-impact guards, add/remove
liquidity, claim, rebalance, and close builders. Dragon's Clutch needs the
analogous user view for a Dealer or facility tranche:

- contributed cash, existing Eggs, and sponsor loss capital by semantic owner;
- current encumbrance, inventory, realized fees/spread, maximum remaining loss,
  and withdrawable amount;
- the exact current generation and claim domain;
- close/halt/buyback/resolution state and the next permissionless action;
- required target accounts, expected rent movement, and neutral-sink behavior;
- a transaction builder that refuses stale or incomplete projections.

This is a direct UX transfer. DLMM's price bins should not become DC's payoff
bins. A spot bin holds two fungible reserves at a fixed local exchange rate;
an Egg has a terminal state-contingent payoff and participates in a coupled
complete-set liability. Pairwise reserve ratios neither preserve the native
simplex nor capitalize terminal liabilities across arbitrary coefficient
portfolios.

## What to adopt from Gnosis, Polymarket, and Kalshi

### Complete-set and complement crossing

Gnosis's `splitPosition` accepts a valid disjoint partition and obtains deeper
positions by burning a shallower position; merge reverses that operation.
Polymarket's CTF makes the binary case product-legible: one collateral unit can
mint one Yes and one No, and equal Yes/No balances can merge back to collateral.
Its exchange can match ordinary transfers, complementary buys that mint a
complete set, and complementary sells that merge one.

Dragon's Clutch already owns the more general algebra. It should expose the
more general execution primitive:

- if compatible buy intents collectively demand an exact complete native
  portfolio, mint the complete set from their aggregate collateral inside the
  candidate;
- if compatible sell intents collectively provide an exact complete native
  portfolio, merge it and distribute collateral inside the candidate;
- permit partial portfolio completion through one canonical virtual
  split/merge vector, with no solver-chosen leg ordering;
- apply one symmetric fee and rounding policy invariant under addition or
  removal of a risk-free complete-set component; and
- make the resulting backing, supply, reservation, and settlement receipts
  visible through the operator API.

That mechanism can create depth without an LP predicting which complements
users will request. It also makes Dragon's Clutch's smooth coefficient language
economically useful instead of merely more expressive.

### Signed order and lifecycle ergonomics

Polymarket's hybrid design is useful precisely because it names the operator
boundary: users sign price and size; the operator matches and orders authorized
intents; the contracts settle atomically. Dragon's Clutch should retain
permissionless candidate submission, but adopt mature intent controls:

- nonce and generation replay protection;
- absolute expiry plus market-close constraint;
- partial-fill floor and cumulative-fill receipt;
- cancel and cancel-on-pause/reopen behavior;
- post-only, good-till-cancelled, good-till-time, fill-or-kill, and
  fill-and-kill policies where they have an exact batch meaning;
- explicit reserved balance and released remainder;
- submitted, retained, verified, selected, settled, lapsed, cancelled, and
  retryable/terminal failure statuses.

Kalshi's lifecycle is an excellent client contract even though its authority is
centralized. Dragon's Clutch should expose distinct trading close, expected
result time, latest result time, determination, dispute/recovery, amendment,
finalization, and retirement phases. If a market reopens, stale resting orders
should not silently revive; the immutable order policy must either bind the
reopen generation or require cancellation. Every displayed status must be
derived from authenticated owners rather than copied from an indexer label.

## What to adopt from Pyth

Pyth's pull model allows anyone to fetch an update, submit it to the onchain
price-feed contract, and consume it in the same transaction. The update is
authenticated; transport through Hermes is not truth. Pyth publishes a price
and confidence, and its client guidance explicitly requires staleness refusal.

This maps cleanly to Dragon's Clutch's Source plane:

- freeze the exact provider program, ProgramData/release, receiver/parser,
  feed/product identity, schedule, exponent/units, confidence policy, and
  source-work budget in Product artifacts;
- authenticate the update and receiver write before Source accepts an
  occurrence;
- use exact source timestamps/slots and Product-defined windows rather than the
  transaction's convenient present time;
- reject stale, out-of-schedule, wrong-feed, wrong-release, over-confidence,
  ambiguous exponent, or insufficient-summary inputs;
- let any actor carry valid bytes and collect only already-prepaid work; and
- keep provider observations separate from Product payout compilation and
  Failure recovery.

The important generalization is not “support Pyth.” It is a versioned hostile
Source adapter contract under which Pyth is one release-bound implementation.
Fallbacks require their own semantics. “Use Switchboard if Pyth looks odd” is
not a deterministic resolution rule.

## Isometric v4 mathematical red-team

The [Isometric Technical Overview v4](https://www.isomkts.com/Isometric_Technical_Overview_v4.pdf)
is a product overview, not a complete protocol specification. The following
findings are therefore refusals to treat its advertised properties as
established, not claims that an unpublished implementation cannot repair them.

### 1. Heterogeneous per-bin LMSR is not shown to be integrable

Standard LMSR has one constant liquidity parameter:

```text
C(q) = b log(sum_i exp(q_i / b)).
```

Its prices are the gradient of that single convex potential, which supplies
path independence and the familiar finite loss bound. Isometric instead states
`b_i = b_base + alpha*LP_i`, presents a sum of per-bin logarithmic trade costs,
and also says maximum loss remains `b*log(n)` and prices remain path independent.
Those conclusions do not follow from the displayed equations.

Under the natural heterogeneous-softmax interpretation,

```text
z_i = exp(q_i / b_i)
p_i = z_i / sum_k z_k.
```

For `i != j`, holding the `b_i` fixed,

```text
partial p_i / partial q_j = -p_i*p_j / b_j
partial p_j / partial q_i = -p_i*p_j / b_i.
```

The cross-partials differ whenever `b_i != b_j`. The price field is then not
conservative and cannot be the gradient of one twice-differentiable,
path-independent cost potential on that domain. If the overview's `S_i` is
instead frozen while one coordinate trades, its per-bin expression becomes
separable for that frozen snapshot but no longer represents joint LMSR state:
another coordinate changes `S_i`, so simultaneous range trades and order
permutations still need a single specified endpoint potential. None is given.

Changing LP deposits changes `b_i` while `q` is already nonzero. That is an
economic state transition even without a trade: it changes prices, the value of
inventory, and any loss bound. A valid dynamic-depth design needs an exact
reconfiguration cash adjustment, old/new potential ownership, loss transfer,
and withdrawal solvency rule. Otherwise an LP can in principle deposit to
flatten its own trade, trade at the altered curve, and withdraw, or exit just
before adverse information while remaining risk is left to other capital.

The standard `b*log(n)` bound applies to the standard common-`b` cost function.
It cannot simply be relabeled for heterogeneous, mutable `b_i`. Dragon's Clutch
should keep depth immutable per facility generation until a replacement proves:

1. one canonical endpoint potential;
2. symmetric mixed partials or an equivalent discrete path-independence result;
3. exact integer rounding with telescoping endpoint differences;
4. a worst-case loss bound after every admitted reconfiguration;
5. a physical cash/inventory recipe; and
6. capital ownership and withdrawal safety.

The additional sigmoid entry price in Isometric v4 creates a second pricing
authority. Unless it is merely a presentation of the derivative of the same
potential, LMSR cost and sigmoid price can disagree and create path/order
arbitrage. A market needs one settlement cost owner, not two appealing curves.

### 2. Gaussian positions do not state a liability invariant

Let a position `s` promise payoff `f_s(y)` per share at resolved outcome `y`,
and let `N_s` shares be outstanding. A fully funded issuer must reserve at
least

```text
K >= sup_y sum_s N_s * f_s(y),
```

using the protocol's exact integer payout and rounding convention. Isometric's
point Gaussian and integrated range Gaussian overlap: at one outcome, many
nearby positions can all pay positive amounts. Nothing in the overview bounds
their aggregate by deposited collateral, defines a finite partition of unity,
or prices and escrows the exact worst-case portfolio liability.

The displayed point formula has maximum value one for every `sigma`, so by
itself it does not implement the prose claim that a tight sigma has a higher
maximum and a wide sigma a lower maximum. The unnormalized range integral
generally grows with range width. Adding an amplitude or normalization can fix
product shape, but it must be immutable and included in issuance liability.

Normalizing payouts after seeing the outstanding book would be worse: new
issuance could dilute existing claims. The safe alternatives are:

- compile every claim into a fixed finite native payoff vector and require the
  exact maximum-liability deposit before minting; or
- define a canonical nonnegative payoff basis whose weights sum to the payout
  scale at every outcome, then issue exact coefficient portfolios over it.

Dragon's Clutch already implements the second semantic foundation and models
the first general maximum-liability rule. Gaussian-like user shapes can be
compiled into immutable B-spline coefficients with a disclosed approximation
certificate; they do not need a new floating `exp`/`erf` settlement authority.

### 3. LP capital and insurance ownership are underspecified

The overview says a single-sided USDC LP increases per-bin depth, earns
risk-weighted fees, and receives insurance protection for “impermanent loss”
above five percent. It does not specify:

- which exact terminal liabilities the deposit owns;
- whether the LP can withdraw while shares remain outstanding;
- how P&L is allocated when LPs enter at different inventory states;
- what cash adjustment accompanies a depth change;
- whether an LP owns fees, AMM inventory, a senior claim, or a pro-rata vault
  share;
- the maximum insured shortfall and present capital that funds it; or
- what happens when multiple protected LP claims exceed the insurance fund.

Fee income, token-creator fees, liquidation surplus, and future treasury
allocations are contingent future flows. They are not present solvency or
liveness capitalization. A five-percent floor is a liability and must be
backed by a separately owned reserve sized to its exact worst case, or explicitly
refused. Calling adverse-selection loss “impermanent loss” obscures the risk:
the LP is subsidizing informed state-contingent issuance, and the loss can be
permanent at resolution.

Dragon's Clutch should retain segregated cash, existing-Egg inventory, sponsor
curve-loss capital, realized-fee ledger, expense/liveness compartment, and
withdrawal gate per facility generation. No component may borrow Hoard
principal, rent, another sponsor's facility, or projected fees.

### 4. Oracle, margin, and governance rules conflict with canonical ownership

The Isometric overview proposes Pyth, a Switchboard fallback, TWAP/median
sampling, a rolling three-standard-deviation outlier rejection, optimistic
non-price resolution, and governance-adjustable oracle configuration. It does
not give precedence, frozen identities, confidence/staleness bounds, sampling
availability rules, or a deterministic failure payout. A three-sigma filter can
also reject the legitimate regime shift that a derivative is meant to settle.

Its VaR margin, liquidation auction, 85% auction floor, and insurance shortfall
replace exact maximum liability with a statistical and operational dependency.
That is a different instrument. Dragon's Clutch should reject leverage and
liquidation for its canonical fully funded claims. A separately branded future
margin venue would need new debt, priority, liquidation, bankruptcy, oracle,
and failure semantics; it must not weaken the current Hoard invariant.

Governance may approve new immutable templates and policy generations. It
should not change live `b`, payoff normalization, collateral ratio, fee,
settlement source, or withdrawal priority after users have funded a market.

## Prioritized change set

### P0 — adopt now, on the current critical path

1. **Complete one canonical authority path.** Finish action15 from its persisted
   bootstrap through exact Funding reservation, Source occurrence, RootV3,
   LinkV3, General/Failure destinations, and current Dealer custody. Delete or
   refuse every parallel/mock authority. Inner composers are not a product.
2. **Ship a chain-derived operator contract.** Provide finalized bounded
   snapshot endpoints, release/session binding, hostile decoder results,
   action/capability discovery, exact account roles, transaction construction,
   quote freshness, CU/rent/work forecasts, and structured refusal output.
3. **Make complete portfolios cross.** Generalize CTF's complement mint/merge
   matching across the native DC basis and coefficient portfolios, with one
   canonical net flow and symmetric exact fee rule.
4. **Finish real Dealer capital.** Implement custody and execution for cash,
   existing Eggs, sponsor loss capital, realized fees, and expense/liveness
   balances. Expose encumbrance and withdrawal state. No negative inventory or
   future-fee capitalization.
5. **Make the Pyth route ordinary production infrastructure.** Release-bind the
   provider/receiver/parser/feed/schedule; carry authenticated pull updates into
   Source; enforce confidence, staleness, window, and rollback; retain local
   validator and captured-fixture modes as evidence, not authority.

### P1 — adapt after the canonical path is callable

6. **Order controls and lifecycle events.** Add nonce/expiry, cumulative partial
   fills, cancel, post-only, GTC/GTT/FOK/FAK where exact, cancel-on-reopen, and a
   stable event/status projection modeled on Polymarket and Kalshi.
7. **Pure route planning.** Let each family own its math and receipts while one
   router constructs bounded atomic action sequences with per-hop limits,
   account/CU budgets, and Address Lookup Table support.
8. **LP/facility position UX.** Present capital, inventory, maximum remaining
   loss, realized return, service state, close route, and withdrawable amount as
   clearly as a mature CLMM/DLMM position manager.
9. **Promote the exact quadratic facility before another AMM.** Its immutable
   rational potential, endpoint ceiling, simplex prices, physical
   split/merge recipe, and present loss capital fit DC's rules. Integrate it as
   one nonlinear batch leg, not a sequenced side venue.
10. **Prepaid market-making incentives.** Reward measured two-sided service or
    candidate work from present named balances. Already collected fees may be
    distributed; expected future fees never guarantee service.

### P2 — research without blocking P0/P1

11. **Bounded multi-facility routing** with permutation-invariant aggregation
    and segregated loss capital.
12. **Canonical roll intents** across compatible Series instances, with no
    assumption that old capital returns before new activation.
13. **Certified approximation products** for Gaussian-like, digital, range,
    tent, and capped-linear shapes over one native basis.
14. **Checked clearing-quality certificates** that are separately versioned
    from feasibility. Until one exists, retain “best valid submitted candidate.”

### Reject unless a replacement closes the named theorem

- pairwise CPMM/CLMM/DLMM invariants as canonical contingent-claim pricing;
- dynamic liquidity parameters that reprice open inventory without an exact
  reconfiguration transfer and loss theorem;
- heterogeneous per-bin LMSR with the standard common-`b` loss claim;
- simultaneous LMSR and sigmoid price authorities;
- Gaussian share issuance without a fixed payoff basis or exact worst-case
  liability reserve;
- margin/liquidation inside the fully funded canonical instrument;
- insurance, liveness, rent, or withdrawal rights funded by hoped-for fees;
- oracle-brand or indexer authority;
- mutable governance over live economic terms; and
- “optimal clearing” without a checked optimality certificate.

## Acceptance tests for the product direction

The next architecture review should be able to answer all of these from one
release-bound chain snapshot:

1. What exact payoff vector does one position own, and what present collateral
   covers the worst portfolio payout?
2. Can complementary intents atomically create or destroy complete backing
   without a privileged maker?
3. Which account owns each dollar/atom of claimant backing, LP inventory,
   sponsor loss capital, realized fee, rent, and prepaid work?
4. What can each actor withdraw now, and why does that preserve every future
   payout and terminal action?
5. What source release, feed, schedule, confidence, staleness, window, and
   failure rule determine settlement?
6. Which candidate was selected, under which score and tie-break, and does the
   claim stop at “best valid submitted candidate”?
7. Can a new wallet discover the market, obtain a fresh exact quote, construct
   the transaction, understand all signatures/debits, observe final status, and
   recover from a refusal without manually supplied account bytes?
8. Can every nonterminal state identify a finite next action whose execution is
   funded from a present named balance rather than future revenue?

If any answer requires a maintainer's memory, pasted hex, an indexer's opinion,
or a future fee, the protocol is not coherently enabled yet.

## Primary sources

All competitor descriptions above were taken from official documentation or
official project repositories, inspected 2026-08-24:

- Raydium: [routing overview](https://docs.raydium.io/products/routing/index),
  [routing math](https://docs.raydium.io/products/routing/math),
  [aggregator integration](https://docs.raydium.io/integration-guides/aggregator),
  [protocol/product overview](https://docs.raydium.io/introduction/what-is-raydium),
  and [SDK/API surface](https://docs.raydium.io/sdk-api).
- Meteora: [official DLMM developer documentation source](https://github.com/MeteoraAg/docs/blob/main/developer-guides/dlmm/index.mdx),
  [TypeScript SDK reference](https://github.com/MeteoraAg/docs/blob/main/developer-guides/dlmm/typescript-sdk/reference.mdx),
  and [official DLMM SDK](https://github.com/MeteoraAg/dlmm-sdk).
- Gnosis: [Conditional Tokens split/merge guide](https://ct-docs.gnosis.io/conditionaltokens/docs/devguide05)
  and [Conditional Tokens contracts/developer guide](https://github.com/gnosis/conditional-tokens-contracts/blob/master/docs/developer-guide.rst).
- Polymarket: [positions and tokens](https://docs.polymarket.com/concepts/positions-tokens),
  [trading overview](https://docs.polymarket.com/trading/overview),
  [order lifecycle](https://docs.polymarket.com/concepts/order-lifecycle),
  [resolution](https://docs.polymarket.com/concepts/resolution),
  [deployed contracts and audits](https://docs.polymarket.com/resources/contracts),
  and [CTF Exchange design](https://github.com/Polymarket/ctf-exchange/blob/main/docs/Overview.md).
- Kalshi: [market lifecycle](https://docs.kalshi.com/getting_started/market_lifecycle),
  [market settlement](https://docs.kalshi.com/getting_started/market_settlement),
  [RFQ lifecycle](https://docs.kalshi.com/getting_started/rfqs), and
  [fixed-point API migration](https://docs.kalshi.com/getting_started/fixed_point_migration).
- Pyth: [pull updates](https://docs.pyth.network/price-feeds/core/pull-updates),
  [why updates are required](https://docs.pyth.network/price-feeds/core/why-update-prices),
  [aggregation architecture](https://docs.pyth.network/price-feeds/core/how-pyth-works),
  and [integration best practices](https://docs.pyth.network/price-feeds/core/best-practices).
- Isometric: [Technical Overview v4, version 1.2, April 2026](https://www.isomkts.com/Isometric_Technical_Overview_v4.pdf).

Repository sources used to characterize Dragon's Clutch include
[`CURRENT_TRUTH.md`](../../CURRENT_TRUTH.md),
[`ARCHITECTURE.md`](../ARCHITECTURE.md),
[`INSTRUMENT_AND_MARKET_DESIGN_REVIEW_2026-08-22.md`](INSTRUMENT_AND_MARKET_DESIGN_REVIEW_2026-08-22.md),
[`BOUNDED_LIQUIDITY_FACILITY_V2.md`](../design/BOUNDED_LIQUIDITY_FACILITY_V2.md),
and the [continuous-claims design](../design/continuous-claims/README.md).
