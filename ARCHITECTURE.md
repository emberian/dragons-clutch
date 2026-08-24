# dClutch architecture

Status: architectural baseline for the new repository. It is not a deployment
or completeness claim.

## Product thesis

dClutch issues fully collateralized claims over a bounded canonical state
domain. User-facing ranges, tents, crash protection, and other bounded shapes
compile into exact native claim coefficients. The protocol should make a simple
claim simple while retaining atomic portfolios and sophisticated clearing as
optional execution capabilities.

The first product target is recurring stable-collateral tail and range
protection using an objective, release-bound source. DREGG may be a dogfood
collateral profile; it is not protocol ontology.

## The thin waist

Every implementation layer joins through four immutable identities:

1. `MarketIdentity`: Realm, terms, claim basis, resolution policy, generation.
2. `CapabilitySet`: the exact optional facilities admitted for this Market.
3. `ExecutionReceipt`: an exact, replay-protected liability/cash transition
   produced under one admitted venue policy.
4. `ResolutionReceipt`: an exact terminal state produced under the Market's
   admitted source and resolution policy.

The receipt schemas remain undecided until the first vertical slice proves the
smallest sufficient account and replay contract. Their names are architectural
roles, not permission to invent caller-authored authority.

## Universal Market Core

The target universal physical state is:

- a compact `MarketRoot` that persists as the terminal replay authority;
- one market-local collateral Hoard;
- one exact SupplyLedger, separate only if measurement shows that doing so
  materially reduces write contention or layout churn; and
- at most one segregated funding escrow whose internal compartments retain
  distinct ownership.

Positions are created on demand. The root stores content identities, phase,
generation, capability commitment, and exact outstanding-child counts. It does
not store a standard maximum-width list of every possible subsystem account.

Keeping a compact terminal root is preferred to deleting the final authority
and reconstructing replay safety through a forest of anchors and close routes.

## Capability children

Optional children include:

- Direct signed-intent execution;
- General frequent-batch portfolio execution;
- covered Dealer liquidity;
- bearer/native outcome mints and custody;
- Fractional and Structured wrappers;
- source-specific resolution adapters; and
- deeper recovery profiles.

The capability graph is immutable, canonical, acyclic, and part of Market
identity. Each selected child has a semantic owner, dependencies, exact present
principal, and activation deadline. A child may be physically lazy only when
its creation principal is already segregated. Disabled children cannot appear.

## Claims and collateral

For categorical claims with total supply `T_i` and Hoard atoms `V`, solvency is

```text
V >= max_i T_i.
```

For a finite payout set `P`, the general invariant is

```text
V >= max_{p in P} liability(p).
```

Complete-set split adds the same quantity to every native claim and to the
Hoard. Merge reverses it. Trading, materialization, and internal transfer do not
change total liabilities. Resolution and redemption reduce liabilities and the
Hoard under one explicit integer rounding policy.

Hoard principal cannot fund execution, liquidity, liveness, fees, rent,
insurance, or cleanup.

## Execution venues

### Direct first

The default venue is signed-intent atomic settlement. An untrusted matcher may
order compatible authorized intents but cannot invent price, quantity, owner,
expiry, or balance. The core supports ordinary transfer, complementary buys
that split backing, and complementary sells that merge backing.

Onchain reservation is an optional stronger order type, not a prerequisite for
every quote.

### General when it earns its complexity

General is an optional frequent batch venue for exact simplex prices, atomic
coefficient portfolios, virtual complete-set conversion, and permissionless
candidate competition. Paginated verification is expected; historical wire
actions and parallel authority paths are not.

### Dealer as a capital facility

Dealer attaches segregated cash, Egg inventory, sponsor loss capital, realized
fees, and service funding to one Market generation. Pricing parameters are
immutable within a capital epoch. Entry, exit, and reconfiguration require
quiescence and an exact old/new value transfer.

## Resolution

A source adapter authenticates provider release, feed identity, units,
confidence, staleness, schedule, and observation bytes. A Product resolution
policy determines the window, statistic, edge rules, repair path, and terminal
failure result. Provider transport is not Product truth.

Pyth is the first intended real adapter. Local fixtures must execute the real
provider ABI/cryptographic boundary and remain labeled synthetic observations;
there is no mock provider authority in a release profile.

## Deployment boundary

Semantic modules are separate crates. Whether Direct, General, Dealer, Source,
and Core become separate programs or capability-specific ELFs is a measured
deployment decision. Separate programs reduce universal program size and make
trust boundaries explicit but add CPI and release-binding costs. No module may
assume the answer before CU, loader rent, atomicity, and upgrade-authority
measurements exist.

## User-facing lifecycle

Clients expose stable workflows rather than wire action numbers:

```text
discover -> quote -> fund -> trade -> settle -> resolve -> redeem/retire
```

Every status is derived from hostile-decoded chain owners. The operator reports
observed slot, release, expiry, required signers, exact debits, account/rent/CU
geometry, and refusal reason before constructing an unsigned transaction.
