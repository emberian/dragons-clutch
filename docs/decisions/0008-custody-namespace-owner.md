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

(The replay seeds gained a `caller_role` component in the addendum below. §1–§5
describe the state of the tree when this record was accepted; read §7 for what
the seeds are now.)

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
(`crates/dclutch-custody-contract/src/projected.rs`) returned
`[CUSTODY_REPLAY_PDA_DOMAIN_V1, market, release_set, context_digest]` — the
replay namespace under a different type name, at nine call sites that a grep
for `CustodyReplaySeedsV1` does not find. Separation from live replays rests
entirely on the digest never colliding with a live `request.context`. §7
composes it out of `CustodyReplaySeedsV1` instead of restating it.

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

1. **RESOLVED by §7 (2026-08-27).** ~~No route creates a Claims-role Custody replay.~~ `CustodyReplayV1::advance`
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
   *Ruled:* the recommendation, plus a Claims-owned first-use creation route for
   the compartment it makes addressable. See §7.
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

## 7. Addendum (2026-08-27): the caller role joins the replay seeds

Status: ruled by ember on the recommendation in §6 item 1. Implemented in
`f915999`, `cdd934e`, `2701a3c`, `72fb6a8`.

### 7.1 The seed change

```text
replay = [ "dclutch:custody-replay:v1", market, release_set, CALLER_ROLE, context ]
vault  = [ "dclutch:custody-vault:v1",  market, release_set,              context, compartment ]
```

`caller_role` is one byte, `ExecutionRoleV1 as u8`, in the same position and the
same encoding `CallerAuthoritySeedsV1` has used for the executing role since it
was written. One `(market, release_set, context)` now names one replay
compartment PER EXECUTING ROLE, so a founding stops implicitly claiming the
Market's whole Custody replay for Trading.

Two things this buys beyond reachability:

1. **Bytes and address agree by construction.** `CustodyReplaySeedsV1::from_request`
   and `CustodyReplayV1::initialize` read the same `request.caller_role` field, so
   a replay at role R's address decodes as role R. Before, the two were
   independent and only the payout routes' `caller_role != Claims` guard stood
   between a founding's Trading cursor and a redemption.
2. **Two authors must agree before an account exists.** The creating program and
   Custody derive the address independently from the same request.

### 7.2 The asymmetry: replays take the role, Vaults do not

This is the design, not an omission.

**A Market's Hoard is ONE principal pool.** The founding funds it, Claims pays
redemptions out of it, retirement sweeps what is left. Role-seeding the Vault
would split one Market's collateral into as many physically distinct pools as
there are roles that touch it, and the founding's principal would land in a pool
no payout route could spend — which is the defect §1 exists to fix, reintroduced
at a different coordinate.

**A replay is the opposite kind of object**: a per-caller optimistic cursor whose
entire content is one role's revision history, its bound caller program, and its
rent refund. Two roles cannot share one cursor, because `advance` binds
`caller_role` and `caller_program` and both are immutable once written.

So: the Vault is *what is owned* and the replay is *who is spending, and how far
along*. Partitioning the second does not partition the first.

**Vault lifecycle still cannot cross roles, and not because of a seed.**
`OpenVault` increments the opening replay's `open_vault_count` and `CloseVault`
decrements the closing replay's, so a role that never opened a Vault under a
context underflows to `Error::VaultCountMismatch` rather than closing another
role's. The Claims-role replay this addendum makes creatable is born with
`open_vault_count == 0`; it can move atoms out of the Hoard and can never close
it. Transfers out of a shared pool are exactly what is shared.

### 7.3 Where the Claims-role replay comes from

**Who is allowed to create it is settled by the code.** Custody's
`InitializeReplay` requires a `CallerAuthority` PDA derived under
`request.caller_program` and SIGNED, and separately authenticates through the
Registry activation cache that `caller_program` is the activated program for
`request.caller_role`. A Claims-role caller authority can only be signed by the
Claims program. The founding cannot create it. Core's legacy Open cannot create
it. A wallet cannot create it. There is exactly one place it can live.

**What shape it takes is settled the same way.** Direct's escrow plan opens with
an `InitializeReplay` (`trading-sbf/direct/buy_escrow.rs`), Series' does
(`trading-sbf/series/custody_v3.rs`), and Core's legacy Open dispatches one as
its own outer route (`core-sbf/open_market.rs`). No route in the tree creates a
replay as a side effect of a transfer. Folding creation into a payout would put a
rent payer and a variable-width frame inside the fixed frame of an economic
transition that must not depend on either, on the most account-dense route Claims
has.

**The ruling: a standalone Claims route, first use, fully prepaid.**
`programs/dclutch-claims-sbf/src/custody_replay_v1.rs`. Decision 0001 permits
physical lazy creation only for an "already selected, canonically addressed,
fully prepaid child", and all three are structural rather than promised:

| 0001's condition | How it holds |
|---|---|
| already selected | the role is Claims and the namespace is `aggregate.custody_context` (§1); neither is a caller field |
| canonically addressed | `CustodyReplaySeedsV1` under Custody, derived independently by Claims and by Custody |
| fully prepaid | `rent_lamports` must equal the Rent sysvar minimum for the replay width; the payer signs; the payer is written in as the immutable `rent_refund` |

The wire (`dclutch-claims-svm/src/custody_replay_v1.rs`, 48 bytes) carries the
Market and nothing else — it is what ADDRESSES the aggregate. `expected_request_v1`
builds all twenty-two Custody coordinates from the aggregate, the Rent sysvar and
the payer account, and is public: it is the single author for the program and for
every builder, so a campaign or operator cannot become a second place the
namespace is re-guessed. Creation is permissionless without being permissive —
two callers submit the same 48 bytes but for the payer, and the second finds the
account already there.

The instruction measures **711 bytes** against the 1,232-byte legacy limit, which
is asserted: creating this cursor stands ahead of a redemption and must not
require a published address-lookup table.

**Terminal settlement's plan therefore gains an explicit create step**, emitted
when the replay is absent. The mechanism is lazy creation; the plan makes it a
step. The two options §6 offered are the same answer seen from two ends.

### 7.4 Site inventory

Composition sites, all now deriving through the owning type:

| Site | Role | Note |
|---|---|---|
| `crates/dclutch-custody-contract/src/lib.rs` | — | owner: `CustodyReplaySeedsV1::new` / `from_request` |
| `crates/dclutch-custody-contract/src/projected.rs` | Trading | `ProjectedCustodyStateSeedsV1` composes the owner, role PINNED to Trading — `normal_replay_from_realization_v1` mints a Trading replay out of that account |
| `crates/dclutch-custody-contract/src/projected.rs` | Trading | new `ProjectedCustodySourceReplaySeedsV1` for the source compartment `open_source_compartment` mints, restated by hand at three sites before |
| `programs/dclutch-custody-sbf/src/lib.rs` | request | replay identity + the `invoke_signed` seed list |
| `programs/dclutch-custody-sbf/src/projected.rs` | Trading | source-replay creation, source-frame authentication (x2) |
| `programs/dclutch-claims-sbf/src/rational_terminal_v3.rs` | Claims | `from_request`, already correct |
| `programs/dclutch-claims-sbf/src/terminal_settlement_v3.rs` | Claims | restated array → `CustodyReplaySeedsV1::new(..., Claims, ...)` |
| `programs/dclutch-claims-sbf/src/custody_replay_v1.rs` | Claims | new: the creation route |
| `programs/dclutch-core-sbf/src/open_market.rs` | request (Core) | `from_request` |
| `programs/dclutch-core-sbf/src/generic_founding_v1.rs` | Trading | restated array → the owning type |
| `programs/dclutch-trading-sbf/src/direct/{buy_escrow,inline}.rs` | Trading | `from_request` |
| `programs/dclutch-trading-sbf/src/dealer/v3_accelerator_accounts.rs` | Trading | `from_request` |
| `programs/dclutch-trading-sbf/src/projected_custody_composition_v4.rs` | Trading | restated array → `ProjectedCustodySourceReplaySeedsV1` |
| the four operator crates, `dclutch-svm-harness` tests | request | `from_request` |
| `tools/local-validator/bootstrap/successor/src/market.rs` | Trading | two restated arrays → the owning type |
| `programs/dclutch-trading-sbf/program-test/direct-hot/src/fixture.rs` | Trading | restated array → the owning type |

**Vault seed sites: NOT in scope, and unchanged.** §3's compartment scoping table
still reads exactly as written; nothing in it moves.

A restated seed array is the one edit shape that keeps deriving the old address
after the seeds move, with everything green — `cargo check` cannot see it, and
neither can a reviewer reading the diff of the type that moved. Every site a
`CUSTODY_REPLAY_PDA_DOMAIN_V1` grep finds is now the owning type.

### 7.5 Evidence

`programs/dclutch-claims-sbf/tests/rational_representation_v2_program_test.rs`,
real ELFs, real Custody CPI, real Token-2022: **7 passed → 10 passed**.

The campaign used to `add_account` a Claims-role replay into the ledger. Every
terminal test now creates it by submitting the route, and the redemption consumes
the cursor that account actually carries (revision 1, zero open Vaults). Three
hostile cases:

- a Trading-role replay forged into the Claims-role account — refuses `0x5002`,
  Hoard and recipient byte-identical;
- creation against a substituted aggregate, and against the Trading compartment's
  own address offered as the place to write a Claims cursor — both refuse
  `0x5002` and neither account exists afterwards;
- a second creation — refuses before Custody is reached, the live cursor is
  untouched, the first prepayer keeps the refund.

### 7.6 What this does NOT establish

- **The already-founded Market is still unredeemable** (§6 item 4). Its
  `custody_context` is its own address and is not mutable. This addendum changes
  what a Market founded from here can do; it changes nothing for that one.
- **Dealer's three `HoardPrincipal` sites still name the Market address** (§6
  item 2) and Dealer v1 still partitions by the Dealer state address (§6 item 3).
  Both are Vault-side and untouched by a replay-seed change.
- **`claims/terminal_settlement_v3::process` remains a blocked row.** Its replay
  coordinate is now role-seeded and its plan gains the create step, but no
  campaign drives it (§5).
- **No live-validator evidence.** ProgramTest is a fast lane; see
  `tools/gauntlet/TIERS.md`.
