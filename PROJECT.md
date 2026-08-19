# Dragon's Clutch: canonical project brief

## 1. Purpose

Dragon's Clutch compiles a bounded objective state space into a conserved basis
of onchain payoff assets. A participant deposits a Realm's collateral and
receives one claim for every member of a frozen partition-of-unity basis. The
complete set can always be recombined into its collateral before resolution.
After a frozen observation program derives the realized statistic, consensus
evaluates the basis and each Egg redeems according to its exact integer weight.

Degree zero is the categorical case: the source domain is divided into an
exhaustive, disjoint, ordered partition and exactly one Egg receives full
weight. Degrees one through three are native open-clamped B-spline semantics:
nearby Eggs overlap, remain nonnegative, and their weights sum exactly to one.
They are not sampled categorical claims and must not be silently lowered into
one-hot bins.

The protocol is not a leveraged exchange. It creates no debt, margin call,
liquidation order, insurance deficit, or socialized loss. Its central promise is
stronger and narrower:

> For every reachable protocol state, the market-local Hoard covers the maximum
> payout allowed by the market's immutable terms.

Exact coefficient vectors over the selected native basis form bounded payoff
algebra. They can express crash insurance, digitals, ranges, triangles, capped
directional exposure, and tails without creating a new vault for each
human-language question. A target outside the finite spline span requires an
explicit approximation certificate. Sampling the target over degree-zero Eggs
is a compatibility lowering, not the definition of the native shaped claim.

V1 targets token-native state partitions derived from an onchain fact or a
deterministic function of authenticated onchain price history: terminal bands,
relative performance, sampled crossings, drawdown or volatility regimes,
lifecycle transitions, pool state, and similarly bounded predicates.

## 2. Vocabulary

| Term | Meaning |
|---|---|
| Dragon's Clutch | Protocol and public project |
| Eggcrate | Pure deterministic verified Rust transition kernel |
| Realm | Immutable collateral profile and protocol-version namespace |
| Hoard | Market-local collateral vault reserved exclusively for claimants |
| Egg | One native basis claim; categorical at degree zero, smooth at degrees one through three |
| Clutch | One complete exhaustive set containing one unit of every Egg |
| Hatch | Immutable evidence-derived resolution and payout-vector transition |
| Feed | Shared authenticated observation series |
| Window | Immutable derived result over a frozen feed interval and feature set |
| Epoch | Reference-venue order collection and deterministic batch clearing unit |

## 3. Collateral Realms

Eggcrate is collateral-generic. A Realm freezes one collateral mint, token program,
decimals, extension/authority profile, protocol version, and economic presets.
The solvency theorem is denominated in that collateral's atoms; it does not promise
stable purchasing power.

Deployers may create appropriately vetted DREGG, USDC, SOL/LST, or community
collateral Realms. No collateral receives a hidden branch in the
verification-target kernel.

### Reference DREGG Realm

The proposed house Realm references
`XkeTXo1125vz5H9svJpGiw4JvLbN8VmMu9cmMvspump` as collateral. This repository has
not authenticated its token program, decimals, authorities, extensions, or
current supply as chain facts, and no DREGG Realm is frozen. Runtime Realm
initialization must independently verify those properties against an approved
collateral profile and immutable Realm configuration. No source document,
offline vector, or client assertion can substitute for that onchain check.

DREGG's utility in the house Realm is concrete:

- collateral and settlement denomination;
- market-creation and observation endowment material;
- reference-venue fees;
- maker, clearer, and supplemental keeper income;
- transparent protocol treasury revenue.

DREGG is not governance weight, required staking yield, a promise of profit, or a
license to alter live markets.

## 4. V1 product surface

V1 consists of seven separable layers:

1. **Eggcrate kernel.** Collateral-generic state transitions, integer arithmetic,
   codecs, and invariants; no Solana SDK, allocation, unsafe code, oracle SDK, or
   CPI.
2. **Basis and coefficient compilers.** A small audited language turns source,
   window, statistic, domain, degree, knots, denominator, edge policy, and
   rounding rule into a canonical Market Template and native basis of Eggs.
   A separate certified compiler constructs exact or explicitly approximate
   coefficient artifacts over that basis.
3. **Solana adapter.** Hostile-byte parsing, signer/owner/PDA checks, clock and
   source authentication, persistence, and narrow Token-2022 CPIs.
4. **Shared accumulator.** Frozen source adapters, bounded observation pages,
   associative summaries, repair windows, and reusable Window results.
5. **Simplex auction.** Internal balances and dense public order pages enter one
   specialized, versioned batch relation: a coupled basis-price vector summing
   to one, complete-set conversion, bounded atomic coefficient intents,
   permissionless candidate competition, exact witness verification, and lazy
   settlement. It is not a generic matching VM and makes no V1 privacy claim.
6. **Venue adapters.** Materialized Eggs can trade on Manifest, AMMs, RFQs, and
   future Jupiter routes without making those venues authoritative.
7. **Static Glass.** A replaceable IPFS/GitHub Pages application that reads any
   RPC, constructs transactions, verifies manifests, and exposes permissionless
   work. It owns no truth or authority.

The neutral issuance kernel and standard outcome assets do not depend on the
simplex auction. External pools and routes may trade materialized Eggs without
paying Dragon's Clutch fees. A generic orderbook is not part of the native venue's
charter.

## 5. Hybrid claim representation

Each native basis member has a canonical Token-2022 mint, but native users need
not mint all of them. A program-owned Position contains fixed internal
balances. A complete internal split reclassifies pooled collateral and credits
every basis member. Materializing one Egg debits the internal balance and mints
that Token-2022 asset. Dematerialization burns the external asset and restores
the internal balance.

For outcome `i`:

```text
total_i = materialized_supply_i + aggregate_internal_balance_i
```

Materialization preserves `total_i`; internal and external transfers preserve it;
splitting increases every `total_i` and Hoard collateral by the same quantity;
merging and redemption decrease liabilities and Hoard collateral according to
the frozen rules. Direct holder burns are safe donations and make the primary
solvency relation an inequality rather than an equality.

This hybrid is a deliberate Pareto choice. An internal-only ledger would be
cheaper but noncomposable. Mandatory full materialization would be composable but
would impose one mint CPI and generally one token account per outcome on every
split. Dragon's Clutch pays that boundary cost only when a user requests it.

## 6. Resolution and evidence

Every Market freezes a versioned `FeedSpec` or a terminal-state adapter. The
configuration identifies subject and quote, orientation and decimals, source
accounts/program/version, time grid, repair deadline, coverage and dispersion
bounds, arithmetic and rounding, allowed metric, and deterministic ambiguity
rule.

No reporter chooses the value. A transaction either carries uniquely qualifying
authenticated evidence or it is rejected. Shared feed updates advance
monotonically. Windows consume sealed summaries. Resolution occurs only after its
observation and repair conditions have closed.

An oracle or DEX upgrade, missing evidence, inadequate quorum, excessive
dispersion, or arithmetic refusal follows a frozen failure rule; it never grants
a resolver discretion. The failure payout is itself an adversarial surface and
remains an explicit design gate.

Degree-zero resolution selects one cell of the Market's exhaustive categorical
partition. Degree-one through degree-three resolution evaluates the frozen
open-clamped B-spline basis at the admitted statistic and normally produces a
fractional payout vector. In both cases the integer weights are nonnegative and
sum to the frozen denominator, so prices of a complete basis live on a simplex.

The active rounding rule is deterministic largest remainder with lowest-index
ties. Interval evidence may be admitted only by a separately specified
conservative rule; an adapter may never invent a midpoint. Exact coefficient
algebra is native. Degree-zero sampling remains an explicitly disclosed
compatibility path with an error certificate where needed.

## 7. Sustainability

There are two distinct meanings:

- **Protocol sustainability:** every admitted Market can observe, repair,
  finalize, and settle from prepaid resources even if later volume is zero.
- **Maintainer sustainability:** optional venue fees and service premia may fund
  audits, development, and public infrastructure.

Only the first is a protocol invariant. Expected future volume, collateral or
reward-token appreciation, new subscribers, or treasury intervention never
capitalize an existing Market.

## 8. Deployment and operation

The repository will publish programs, proofs, reproducible builds, static client,
and deployment instructions. It does not require its authors to deploy or operate
anything. A deployment is strongest when its settlement programs are immutable,
its program bytes match a verified build, and its client manifest binds the
program/spec/schema versions.

RPCs, gateways, oracle publishers, and DEX programs remain ecosystem
infrastructure, but none is uniquely operated or trusted by Dragon. Anyone may
submit paid observation, repair, clear, finalize, or cleanup work.

## 9. Explicit non-goals

- Subjective political or social adjudication.
- Margin, undercollateralized leverage, liquidation, lending, or cross-market
  collateral netting.
- A continuous dark order book or V1 dependency on FHE/MPC/TEE infrastructure.
- An admin-operated matching, oracle, indexing, or settlement service.
- A privileged hosted API required by the client.
- Claims that source verification, a mathematical model, or host tests prove
  the SBF compiler, Solana runtime, or an external Token-2022 program.
- Entanglement with leanuweave, minidregg, breadstuffs, Oracle Pit, or historical
  DREGG prototypes. Those may inspire humans; they are not dependencies.

## 10. Definition of success

The first honest success is not a mainnet market. It is one reproducible local
walk proving that the same frozen terms can:

1. initialize a realm and market;
2. prepay all mandatory work;
3. compile one exhaustive degree-zero partition and one admitted native smooth
   basis, with exact domain, knot, evaluator, and rounding identities;
4. split internally;
5. materialize and dematerialize one Egg;
6. accept authenticated observations into a shared feed;
7. derive and seal one Window;
8. clear one coupled simplex batch containing single-Egg and atomic
   coefficient-vector intents;
9. resolve and redeem categorical and native smooth payoff shapes without
   compatibility lowering;
10. close every accounting identity;
11. reproduce each claimed layer in its honestly named evidence plane: checked
    model theorems where they exist, host differentials and mutation campaigns,
    and a signed committed SBF integration walk. No one plane substitutes for
    another.
