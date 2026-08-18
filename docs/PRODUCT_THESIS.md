# Product thesis: a state-space compiler

## 1. The correction

Dragon's Clutch is not primarily a prediction-market exchange. Complete-set
issuance already exists in Gnosis Conditional Tokens, and Solana already has
tokenized prediction products, hosted matching engines, and high-quality onchain
orderbooks. Competing as another list of YES/NO questions would make liquidity and
distribution the product while leaving the protocol itself derivative.

Dragon's Clutch should instead be the canonical open compiler from an objective
onchain state space into fully funded, composable bounded-payoff assets:

```text
authenticated observations
  -> deterministic statistic
  -> exhaustive disjoint state partition
  -> complete basis of outcome assets
  -> atomic bounded-payoff portfolios
```

A Market does not merely ask a question. It defines a finite basis over possible
future states. The resulting Eggs can express insurance, ranges, digitals,
distribution tails, capped directional exposure, and path-dependent risk without
margin or liquidation.

## 2. The valuable service

The protocol makes risks tradeable that spot and perpetual markets express poorly:

- terminal price distributions rather than one directional point estimate;
- maximum-drawdown insurance;
- realized-volatility regimes;
- sustained sampled-threshold states;
- token migration, authority, or pool-lifecycle states;
- relative performance of two authenticated assets;
- bounded combinations of terminal and path state.

For a terminal-price Clutch with eight exhaustive bins, a user can synthesize:

- crash protection by acquiring the low-price Eggs;
- a digital call from every Egg above a strike;
- a range position from adjacent middle Eggs;
- a capped call spread from a selected tail segment;
- a straddle-like payoff from both tails;
- a complete implied probability distribution from the clearing prices.

One Clutch supports all of those views while conserving one unit of collateral.
It is more information- and capital-coherent than eight unrelated binary markets.

## 3. Users

### Token traders

Acquire precise bounded exposure without liquidation, funding rates, or an option
writer who can default. Trade payoff shapes rather than only long/short direction.

### Token treasuries and communities

Fund transparent crash protection, milestone distributions, migration insurance,
or recurring market surfaces. The Market cannot be changed after users enter.

### Market makers and solvers

Quote a complete probability simplex, use split/merge as virtual inventory, solve
multi-outcome batches, and earn explicit maker/clearing flows without a privileged
matching endpoint.

### Developers and researchers

Compile a versioned state-partition template, instantiate it permissionlessly,
consume standard Token-2022 Eggs, and verify all semantics from onchain state. A
conformance suite is as important as the reference client.

### Keepers

Perform finite prepaid observation, repair, folding, clearing, finalization, and
cleanup jobs through the same public instructions available to every participant.

## 4. Collateral is a Realm parameter

Eggcrate contains no DREGG-specific economic branch. A Realm freezes one vetted
collateral mint/profile and all Markets in that Realm settle in its atoms.

Potential Realms include:

- DREGG as the house, dogfood, community, and high-volatility collateral Realm;
- USDC or another appropriately profiled stable collateral Realm;
- SOL or LST collateral where its token/program semantics are explicitly handled;
- community collateral Realms deployed by others.

The protocol proves nominal solvency in the chosen collateral, not purchasing
power. A collapsing collateral asset may make payouts economically unattractive
without making the Hoard insolvent in its own units.

DREGG retains real utility in its Realm and may fund supplemental keepers,
creation bonds, public-goods revenue, or reference-client policies. It is not
forced into every Market merely to manufacture demand.

## 5. Product layers

### Eggcrate

Collateral-generic issuance, internal/materialized supply, bounded payout bases,
protected pools, and settlement theorem.

### Partition compiler

A small audited language compiles source, time window, statistic, and exhaustive
boundaries into canonical accumulator features and outcome semantics.

### Hatchery

Shared authenticated feeds and WindowResults amortize the information required by
many Market instances and Templates.

### Simplex auction

A native frequent batch auction clears a coupled price vector whose outcome prices
sum to one, automatically uses complete-set split/merge, and accepts atomic bounded
portfolio intents under an explicitly tractable verification model.

### Venue adapters

Materialized Eggs can trade on generic infrastructure such as Manifest, AMMs, RFQ
systems, or future Jupiter routes. The protocol does not pretend external routing
will pay its reference-venue fee.

### Static Glass

An IPFS/GitHub Pages client compiles payoff shapes, explains state partitions,
verifies manifests, and exposes permissionless work without a Dragon backend.

## 6. Market surfaces, not isolated markets

A `Template` defines the compiler program and partition. An `Instance` binds an
exact observation window, collateral Realm, capitalization, and outcome mints. A
prepaid `Series` may permit anyone to instantiate successive windows without an
operator.

Several Instances can share one Feed:

- hourly, daily, and weekly terminal distributions;
- terminal distribution plus drawdown regime;
- the same statistic with coarse and fine partitions;
- several tokens sharing a quote/source family.

The client renders these as a surface over horizon, state, and implied probability,
not a feed of unrelated betting headlines. This is the bridge to a field-oriented
view of markets without claiming the visualization is physical truth.

## 7. What makes the project excellent even without liquidity

The artifact succeeds as an engineering and scientific public good if it provides:

- a clean formal algebra of conditional assets and bounded payoff portfolios;
- an executable Verus-verified Rust kernel with precise trust boundaries;
- a novel, reproducible simplex-clearing mechanism and conformance suite;
- the cheapest measured Solana implementation at its chosen composability point;
- a permissionless accumulator with honest missing-data semantics;
- a static, accessible, self-verifying client;
- reusable source, venue, and collateral interfaces;
- adversarial economics that expose rather than conceal failure incentives;
- reproducible proof/build/benchmark artifacts for later protocol designers.

Bootstrap, volume, and treasury revenue are product outcomes. They do not determine
whether the work was intellectually or technically worthwhile.

## 8. North-star demonstration

The best first demonstration is one synthetic and then one objective token-native
distribution Market:

1. compile a terminal price into 8 exhaustive bins;
2. prepay its feed/window work;
3. issue a complete internal Clutch in generic synthetic collateral;
4. submit single-Egg and atomic portfolio intents;
5. clear a coherent price vector on the simplex;
6. materialize selected Eggs as ordinary Token-2022 assets;
7. settle from authenticated evidence;
8. redeem crash insurance, range exposure, and a complete set;
9. reproduce every transition in Rocq, Verus host execution, and SBF tests;
10. run the entire client from an immutable static bundle.

