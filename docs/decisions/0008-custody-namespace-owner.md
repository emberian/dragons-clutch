# Decision 0008: one owner for a Market's Custody namespace, and it is the Claims aggregate

Status: accepted on 2026-08-27 as the resolution of the P1 the architecture
review found (structural Hoard-namespace divergence). This is an authority
decision about which account states a coordinate. It is not release evidence,
and it does not by itself retire the routes §6 lists as still never executed.

## Context

Custody namespaces every account it owns by a caller-supplied `context`:

```text
replay  = [ "dclutch:custody-replay:v1", market, release_set, context ]
vault   = [ "dclutch:custody-vault:v1",  market, release_set, context, compartment ]
authority = [ "dclutch:custody-authority:v1", market, release_set ]      # no context
```

`market` and `release_set` bracket the context on both sides, so a context is a
*partition of one Market's Custody state*, never a way to reach another
Market's. The transfer authority is deliberately context-free: one Market has
one signer for all of its compartments.

Two things then disagreed about what a founded Market's context is.

**The founding routes use a digest.** `GenericFoundingRequestV1::context` is
caller-owned — any 32 bytes — and the atomic founding pins

```text
context_digest = SHA-256("dclutch:projected-hoard-context:v1" || found.context)
```

at `programs/dclutch-trading-sbf/src/generic_market_founding_v1.rs:358-366`.
Custody creates the Hoard Vault under that digest
(`programs/dclutch-custody-sbf/src/projected.rs:560`, `open_hoard`), and
`RealizeAndClose` (`:865`) rewrites the projection in place as the Market's
normal live replay at the same digest
(`crates/dclutch-custody-contract/src/projected.rs:1515`,
`normal_replay_from_realization_v1`). The Series founding route does the same
thing with the ticket identity (`programs/dclutch-core-sbf/src/series_consume.rs:1065`).

**The Claims aggregate said "the Market address".**
`founding_v5.rs::authenticate_product_core` wrote
`custody_context: request.market()` — in the same instruction that had already
authenticated the true digest against the Core-owned
permit (`:489-492`), the Lock receipt (`:536`), the realization receipt
(`:608`) and the live replay account's own `context` (`:715`). Every payout
route then read that field for the replay and *separately hardcoded the Market
address for the Vault*, which is why the field being wrong was invisible: both
halves agreed with each other and neither agreed with the chain.

No caller-chosen context can reconcile a SHA-256 digest with a raw address. So
for every Market the atomic founding creates, its collateral principal was
unreachable by every payout route.

**This was live, not theoretical.** `apps/dclutch-web/fixtures/live-open-market.json`
holds finalized bytes from the run that opened the first Market:

```text
Market                 4fQNy8k7G7bZ9cak6pb2VnigV2F5fbhs7YnYFWQ2LQYH
aggregate custody_ctx  366990e3…ed9449fa   == the Market address, as base58
funded Hoard Vault     8JdqNuFojqCfKXyrF8kdffpgcTok8G1aoWe1ZAnzN8aB
                       == vault seeds at 54a9dd33cf0e67a8…db657d6c,
                          the founding's context digest
```

Both are finalized on a real chain and they do not agree. That Market's
principal cannot be redeemed by any route in the tree, and no fix to code
changes that for the Market already founded.

## 1. The decision

**The Claims aggregate's existing `custody_context` field is the sole persisted
owner of a Market's Custody namespace.**

1. `FoundingV5` persists the value it authenticated — the permit-derived
   `projected_context` — and never `request.market()`.
2. Every consumer DERIVES both the replay and the Vault from that field. No
   route may re-guess the coordinate, and in particular no route may assume the
   Market address.
3. The field names ONE namespace, used for both the replay PDA and every Vault
   compartment of that Market. It is not "the replay namespace" (which is what
   its doc comment used to say) and it is not "the Hoard namespace".
4. Convenience derivations of a context stay labelled convenience. The local
   campaign derives its own from `dclutch/local-campaign/founding-context/v1`
   (`tools/local-validator/bootstrap/successor/src/market.rs:1216`) so its runs
   are reproducible. That is a campaign-local string, not a protocol constant,
   and no on-chain or browser code may depend on a context being derivable.

## 2. What changed

| Site | Was | Is |
|---|---|---|
| `programs/dclutch-claims-sbf/src/founding_v5.rs:766` | `custody_context: request.market()` | the authenticated `projected_context` |
| `programs/dclutch-claims-sbf/src/rational_terminal_v3.rs:347` | `source_vault_context: input.market` | `input.custody_context` |
| `programs/dclutch-claims-sbf/src/terminal_settlement_v3.rs:603` | `CustodyVaultSeedsV1::new(market, release_set, market, …)` | `… custody_context …` |
| `programs/dclutch-claims-sbf/src/liability_basis_v2.rs` (DCLLBX02) | `market.logical_market` on the HoardPrincipal side | `market.custody_context` — landed inside `6cbcb3b`, and superseded while this record was being written by the lane deleting the route outright (architecture-review P3) |
| `crates/dclutch-rational-representation-v2-operator/src/lib.rs:1661` | `source_vault_context: descriptor.market_id()` | `common.claims_custody_context` |
| `apps/dclutch-web/lib/marketCoreV2.ts` | `MARKET_HOARD_UNDERIVABLE_V1` refusal | `deriveMarketHoardAddressV1` + authentication |

The `context` (replay) coordinate at each of those sites already read the
field. Only the vault coordinate was wrong, at every single one of them.

## 3. The compartment scoping table

`context` is a partition key, and different families deliberately partition at
different grains. This table exists so the next reader can tell a defect from a
design. Every row was traced to its call site; the digest/address column says
where the bytes come from.

**Market-namespace-scoped — must derive from `aggregate.custody_context`:**

| Compartment | Site | Context |
|---|---|---|
| `HoardPrincipal` | `claims-sbf/rational_terminal_v3.rs:347` | aggregate field ✅ |
| `HoardPrincipal` | `claims-sbf/terminal_settlement_v3.rs:603` | aggregate field ✅ |
| `HoardPrincipal` | `claims-sbf/liability_basis_v2.rs` (Split dest / Merge source) | aggregate field ✅ — row survives only as long as DCLLBX02 does |
| `HoardPrincipal` | `rational-representation-v2-operator/src/lib.rs:1675` | aggregate field ✅ |
| `HoardPrincipal` | `market-retirement-v1-operator/src/lib.rs:934` | `replay.context`, cross-checked at `:846` against the aggregate ✅ (already correct) |
| `HoardPrincipal` | `custody-sbf/projected.rs:560, 1790, 1836` | `request.context_digest` — the founding, the producer of the namespace ✅ |
| `HoardPrincipal` | `core-sbf/open_market.rs:315` | the Market address, PINNED at `:324`. The legacy readiness-ladder Open route: its Market's namespace *is* the address, by its own construction. Correct for that route and only that route. |
| replay | `custody-sbf/lib.rs:559, 614`; `core-sbf/open_market.rs:283`; `claims-sbf/rational_terminal_v3.rs:517`, `terminal_settlement_v3.rs:583` | `request.context` — caller-supplied and, in Claims, the aggregate field ✅ |

**Deliberately family-scoped — document, do not change:**

| Compartment | Site | Context | Why |
|---|---|---|---|
| `TradingPrincipal` | `trading-sbf/direct/buy_escrow.rs:839` | the registered-intent record PDA | Direct escrows are per-INTENT, not per-Market. Two intents on one Market must not share a vault; the record address is the correct partition and the Market has no say in it. |
| replay | `trading-sbf/direct/inline.rs:507` | the buyer maker-root PDA | Same grain, per-maker. |
| `TradingPrincipal`, `FeeVault`, `LivenessVault` | `trading-sbf/dealer/{mod.rs:365, v3_composer.rs:559-560, v3_accelerator_accounts.rs:766-772, v3_multi_lp.rs:986}` | the Dealer `child_root` digest | A Dealer child is its own capital domain under one Market. Collapsing these onto the Market namespace would merge every child's principal into one vault. |
| replay | `trading-sbf/dealer/v3_accelerator_accounts.rs:728` | `request.child_root` | Same. |
| `TradingPrincipal`, `FeeVault`, `LivenessVault` | `dealer-sbf/src/lib.rs:974-993` | the Dealer STATE account address | Dealer v1, a *different* partition key from the v2/v3/v4 `child_root` above. Flagged in §6; not changed here. |
| `Settlement` | `tools/local-validator/…/market.rs:1242` | the raw founding action context | The funding source compartment is deliberately at the RAW context while the Hoard is at its digest — one hash apart, so the two can never collide. `ProjectedCustodyRequestV1::validate` refuses `context_digest == funding_source_context` outright. |
| `HoardPrincipal` | `trading-sbf/dealer/{v3_composer.rs:561, v3_accelerator_accounts.rs:778, v3_multi_lp.rs:1008}` | `context.market` / `request.market` | Dealer reaching the MARKET's Hoard, so by §1 this should read the aggregate field. Dealer has no aggregate in its frame today. Named in §6 as the one carried instance of the defect class. |

**Compartments with no seed-composition site anywhere:** `RecoveryReserve` (8)
and `SeriesEscrow` (7). `External` (1) always pairs with a zero vault context —
it is the non-vault side of a transfer and produces no PDA. `None` (0) is a
request-shape assertion only.

**One alias worth knowing about:** `ProjectedCustodyStateSeedsV1`
(`crates/dclutch-custody-contract/src/projected.rs:199`) returns
`[CUSTODY_REPLAY_PDA_DOMAIN_V1, market, release_set, context_digest]` — the
replay namespace under a different type name, at nine call sites that a grep
for `CustodyReplaySeedsV1` does not find. Separation from live replays rests
entirely on the digest never colliding with a live `request.context`.

## 4. Evidence

Red, then green, on the real ELFs:

```text
612fda2  the rational-representation-v2 campaign funded its Hoard at
         [market, release_set, MARKET_ADDRESS, HoardPrincipal] — the same
         coordinate the program derived. Moved to the honest founded prestate:
         winning terminal redemption refuses 0x5002 (ClaimsSbfError::Identity)
         at 323,843 CU with Custody never invoked.
3fbed64  four composition sites derive from the field; 4 passed.
ab38128  three hostile namespaces — the Market address, a second founding's
         digest, and the raw unhashed action context — each refuses at
         0x5002 with the Hoard, recipient and replay byte-identical and no
         successful Custody transfer. 7 passed.
b23f02f  the browser derives and authenticates, and the recorded live Market
         is kept as the case that proves the defect was live.
```

Non-vacuous by construction: the same instruction against the honest 32 bytes
commits and pays one atom out of the Hoard.

## 5. What this does NOT establish

`claims/terminal_settlement_v3::process` is still in `tools/gauntlet/blocked.json`
with no campaign driving it. The fix to that route is derivation-identical to
`rational_terminal_v3`'s, which IS executed, but "identical to something that
executes" is not execution. It stays a blocked row.

## 6. Carried, with owners

1. **No route creates a Claims-role Custody replay.** `CustodyReplayV1::advance`
   requires `request.caller_role == self.caller_role` and matching
   `caller_program`, and the replay PDA has no role in its seeds — so one
   context admits exactly one role. The projected founding realizes a
   **Trading**-role replay at the namespace
   (`normal_replay_from_realization_v1`) and legacy Open creates a **Core**-role
   one; Claims payout requests are **Claims**-role. Nothing in the tree makes
   the transition. This is orthogonal to the namespace — it would bite equally
   at `context = market` — and it is an authority decision, not a patch.
   *Recommended answer:* put the role in the replay seeds, so one Market carries
   one replay per executing role at one namespace and the founding stops
   implicitly claiming the Market's whole Custody replay for Trading. The
   alternative (a Core-authorized role handoff route) adds a transition whose
   only job is to relabel an account.
2. **Dealer's `HoardPrincipal` sites still name the Market address**
   (`v3_composer.rs:561`, `v3_accelerator_accounts.rs:778`, `v3_multi_lp.rs:1008`).
   Same defect class as §2's rows; not fixed here because the Dealer frames do
   not carry the Claims aggregate and adding it is a frame change with a CU
   cost. Owner: tranche-A Dealer.
3. **Dealer v1 partitions by the Dealer STATE address** where v2/v3/v4 partition
   by `child_root` (`dealer-sbf/src/lib.rs:974`). Two conventions for one
   family. Owner: tranche-A Dealer, with the v1-supersession question.
4. **The first open Market cannot be redeemed.** Its aggregate is written and
   `custody_context` is not mutable. The Market is a demo artifact and the
   honest options are to re-found at a generation whose FoundingV5 writes the
   truth, or to keep it as the recorded witness it now is in the web fixture.
   Owner: ember.
