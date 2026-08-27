# Operator guide

Running a dClutch market means choosing its immutable identities, funding it,
and then getting out of the way. You are not a resolver, you hold no upgrade
lever over the market, and after founding, everything that happens is either
permissionless or refused. This guide walks the decisions that are yours.

Nothing here is deployed; founding today happens against a local validator
(the tier-1 gauntlet campaign is the executable form of this whole guide).
Exact costs, routes, and codes live in the
[generated reference](../reference/README.md).

## Founding

Founding is two golden transactions, and their costs are pinned in
[the budgets reference](../reference/budgets.md):

1. **`DCLTPCB1` — the projected-Custody founding prestate.** Custody
   initializes its replay compartment (including Core's `ProjectFound`
   projection of the Market address), opens the Hoard vault through
   Token-2022, and opens the Source funding compartment. After this the
   collateral and source funding physically exist, segregated, before any
   liability does.
2. **`DCLTGMF1` — the atomic generic founding.** Five stages in one rollback
   domain: Custody **Lock** (locking the Hoard and closing the Source
   compartment), Core **Found** with its one-shot permit, Custody **Realize**,
   Claims **founding**, and Core **Open — last**. Either the market opens
   whole or nothing happened; the campaign's hostile case substitutes a
   Claims request mid-founding and watches the entire founding roll back.
   The transaction rides a finalized address lookup table as a v0 packet,
   and its measured cost sits above 90% of Solana's 1.4M CU ceiling — which
   is why the budgets file watches it by name.

What you fix at founding, forever: the Realm (collateral and admission
namespace), the Product and its claim basis (the cell partition), the
resolution policy and release-bound Source identity, the capability
manifest, and the execution release set. All of them are content-addressed
records published through the Registry before founding; the founding
authenticates them and binds their identities into the Market. There is no
post-founding governance surface to operate.

## Capabilities

Execution is split across fixed roles — Core, Claims, Trading, Resolution,
Custody, plus the Registry and Rent infrastructure
([decision 0003](../decisions/0003-fixed-role-capability-execution.md); the
per-program reference is [programs.md](../reference/programs.md)). Your
capability manifest selects exact releases for the roles the market uses;
activation authenticates content identities, not authorities
([decision 0005](../decisions/0005-per-market-authentication-cache.md)), and
the founding capability root is derived at founding and created afterwards by
the ordinary activation route
([decision 0004](../decisions/0004-founding-capability-root.md)) — there is
no special founding-time backdoor to operate or to defend.

## Funding compartments

Custody names every token-account role with an economic compartment tag, and
the tags are deliberately not interchangeable: a receipt for Hoard principal
can never be accepted as a fee, liveness, recovery, or rent effect. The
compartments (`CompartmentV1`, `crates/dclutch-custody-contract`):

| compartment | holds |
|---|---|
| `HoardPrincipal` | market collateral — pays claimants, never anything else |
| `TradingPrincipal` | Direct/Dealer trading principal |
| `Settlement` | general settlement inventory |
| `FeeVault` | physically segregated realized fees |
| `LivenessVault` | present funded-liveness capital (the failure walk's funding) |
| `RecoveryReserve` | present recovery-reserve capital |
| `SeriesEscrow` | Series ticket principal before its market exists |
| `External` | externally owned depositor / recipient / beneficiary accounts |

Funding is quoted immutably (`DCLTFQ01`) and tracked mutably (`DCLTCFS1`),
per compartment; native lamports and Realm collateral are distinct
dimensions and no operation sums or converts them. When you fund a market,
you are funding *named obligations* — the walk bounty, the resolution work,
the rent — not topping up a pooled balance. An escrow one lamport short of
its named amount refuses.

## Resolution windows

A terminal window is `[start, end]` and **must have width**: a market that
can only be answered on one exact second is answered essentially never, and
walks to its failure outcome instead of resolving. Choose the width against
your source's real publication cadence. For devnet SOL/USD Pyth (measured
p50 ≈ 313 s between publications), the probability the window contains at
least one publication is approximately `1 − exp(−W/313)` (provisional
model):

| width `W` | shape | P(≥ 1 publication) |
| --- | --- | ---: |
| 1 s | the old forced shape | ~0.3% |
| 300 s | one cadence | ~62% |
| 600 s | two cadences | ~85% |
| 1,250 s | four cadences | ~98% |
| 1,800 s | 30 minutes | ~99.7% |

Operative guidance: **at least four cadences (~21 minutes), and 30 minutes
for a market that should not fail for provider reasons.** Source and full
derivation: [`docs/design/MAINNET_STATE_RELAY.md`](../design/MAINNET_STATE_RELAY.md)
§12.3.

`max_age_seconds` is a separate budget: it bounds submission latency — how
old an observation may be when a keeper lands it — not publication cadence,
and it also sets the market's deadline at `end + max_age`. A window wide
enough to be published into is useless if no keeper can land inside
`max_age` of the publication.

Two properties you get for free and should not try to buy again:

- **Exactly one answer.** The first admissible observation terminalizes;
  every later one refuses without being inspected. Machine-checked in
  `formal/dclutch-semantics`
  (`two_admissible_observations_cannot_both_terminalize`).
- **No dead gap.** The last second an observation may resolve the market
  and the first second the failure walk may take it are adjacent by
  construction: both are `end + max_age`.

## The failure walk

The failure outcome is **pre-disclosed and pre-funded**: you name it and
fund its liveness capital before the market opens, and if the source is
silent through the window and its grace, anyone may walk the market there
and collect the named bounty. The campaign witnesses the boundary cases: a
walk before the deadline refuses, a second walk cannot collect twice, a
live compartment that is not the escrow refuses, and an escrow one lamport
short refuses (see the resolution rows in
[the routes reference](../reference/routes.md)).

The walk is not a defect at any window width — it is the honest outcome of
true provider silence. Your job as operator is to make it *rare* (window
width, `max_age`, a source that actually publishes) and *survivable* (a
failure outcome you disclosed, funding that is really there). It should be
reached because nothing published, never because the market asked an
unanswerable question.
