# Dragon's Clutch: canonical project brief

## 1. Purpose

Dragon's Clutch compiles a bounded objective state space into a conserved basis of
onchain payoff assets. A participant deposits a Realm's collateral and receives
one claim for each cell in an exhaustive disjoint state partition. The complete
set can always be recombined into its collateral before resolution. After a frozen
observation program identifies the realized cell, that Egg redeems.

The protocol is not a leveraged exchange. It creates no debt, margin call,
liquidation order, insurance deficit, or socialized loss. Its central promise is
stronger and narrower:

> For every reachable protocol state, the market-local Hoard covers the maximum
> payout allowed by the market's immutable terms.

Portfolios of Eggs form arbitrary nonnegative bounded payoffs over the partition.
One terminal-price distribution can therefore express crash insurance, digitals,
ranges, capped directional exposure, and tail positions without creating a new
vault for each human-language question.

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
| Egg | One categorical outcome claim |
| Clutch | One complete exhaustive set containing one unit of every Egg |
| Hatch | Immutable resolution and payout-vector transition |
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
2. **Partition compiler.** A small audited language turns source, window,
   statistic, and exhaustive boundaries into a canonical Market Template and
   basis of Eggs.
3. **Solana adapter.** Hostile-byte parsing, signer/owner/PDA checks, clock and
   source authentication, persistence, and narrow Token-2022 CPIs.
4. **Shared accumulator.** Frozen source adapters, bounded observation pages,
   associative summaries, repair windows, and reusable Window results.
5. **Simplex auction.** Internal balances and dense public order pages lower into
   one specialized, versioned batch relation: a coupled outcome price vector
   summing to one, complete-set conversion, bounded atomic payoff intents,
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

Each outcome has a canonical Token-2022 mint, but native users need not mint all
of them. A program-owned Position contains fixed internal balances. A complete
internal split performs one collateral transfer and credits every outcome.
Materializing one outcome debits the internal balance and mints that Token-2022
asset. Dematerialization burns the external asset and restores the internal
balance.

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

Normal resolution selects one cell of the Market's exhaustive partition. Prices
of the basis Eggs live on a simplex, and bounded payoff shapes are exact portfolio
vectors over them. Fractional payout vectors are reserved for explicitly admitted
ambiguity policies, not the ordinary product model.

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
- Claims that source verification proves the SBF compiler, Solana runtime, or an
  external Token-2022 program.
- Entanglement with leanuweave, minidregg, breadstuffs, Oracle Pit, or historical
  DREGG prototypes. Those may inspire humans; they are not dependencies.

## 10. Definition of success

The first honest success is not a mainnet market. It is one reproducible local
walk proving that the same frozen terms can:

1. initialize a realm and market;
2. prepay all mandatory work;
3. compile and prove one exhaustive state partition;
4. split internally;
5. materialize and dematerialize one Egg;
6. accept authenticated observations into a shared feed;
7. derive and seal one Window;
8. clear one coupled simplex batch containing single-Egg and portfolio intents;
9. resolve and redeem several payoff shapes;
10. close every accounting identity;
11. reproduce the result in the Rocq model, Verus-verified host kernel, and SBF
    integration harness.
