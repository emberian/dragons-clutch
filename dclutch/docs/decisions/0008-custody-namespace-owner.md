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
| `TradingPrincipal` | `direct-codec/registered_effect_artifacts_v4.rs:750, 793` | the registered-intent record PDA | Direct escrows are per-INTENT, not per-Market. Two intents on one Market must not share a vault; the record address is the correct partition and the Market has no say in it. |
| replay | `direct-codec/ordinary_effect_artifacts_v3.rs:568-570` (`CONTEXT` bound to `IDENTITY_BUYER_MAKER_ROOT_V3`) | the buyer maker-root PDA | Same grain, per-maker. |
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

**What shape it takes is settled the same way.** The registered Buy's emitted
Effect opens with an `InitializeReplay`
(`direct-codec/registered_effect_artifacts_v4.rs:249-256`), Series' does
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

**About "an explicit create step in terminal settlement's plan": there is no
such plan to add it to.** `claims/terminal_settlement_v3::process` has no
operator, no builder and no campaign anywhere in the tree — it is §5's blocked
row, and a grep for its magic finds only the codec that defines it and the
dispatcher that routes it. So the two options §6 offered are not alternatives:
the mechanism is lazy creation, and the step is what a plan will have to open
with once someone writes one. Recorded as an obligation on whoever does:
**a terminal-settlement plan opens with the replay-creation instruction when the
Claims-role replay is absent, and the same is true of any redemption builder,
including the browser's.** The route is already exercised in that position by the
campaign, which is the closest thing to a plan that exists.

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

## 8. Addendum (2026-08-27): the redemption's step two, and who authorizes it

Status: implemented. §7.3 recorded an obligation on "any redemption builder,
including the browser's". This section is what that obligation ran into, and the
ruling that closes it.

### 8.1 The gap, stated from the code

Step one of a redemption — creating the Market's Claims-role Custody replay — has
been reachable from a wallet since §7, and a browser has executed it against a
live chain (`docs/evidence/FIRST_BROWSER_EXECUTION_2026_08_27.md`). Step two had
no route.

`claims/terminal_settlement_v3::process` is the family-neutral Product terminal
settlement. It already computes the exact thing a wallet redemption needs: the
winning coordinate's translated basis weight at the certificate's resolved value,
times the quantity debited, with global solvency proved before and after
(`product_basis_terminal_v3::encode_product_claims_terminal_signed_delta_v3`). It
already debits the Position and the aggregate through one SignedDeltaV3 packet,
and pays out through the Claims-role Custody replay §7 made creatable.

Its request's `caller_role` byte decoded `0 => Core` and `2 => Trading` and
nothing else, and the SignedDeltaV3 frame pins coordinate 0 to a SIGNER whose
address is `CallerAuthoritySeedsV1` under the caller PROGRAM
(`frame_spec_v1::signed_delta`, index 0 = `SIGNER`). Only the Registry-activated
program of that role can sign such a PDA. So the route was structurally
CPI-only, and the one party who could never reach it was the party that owns the
claims.

The Positions this bites are real and already on chain. The only production
producer of a `User`-owner-kind LBV2 Position is `claims/founding_v5`, which
mints the FOUNDER's Position when a Market is founded. Every founded Market has
a wallet holding claims that nothing could pay.

### 8.2 The ruling: widen the admission, do not add a route

The alternative was a second Claims route. It was rejected on inspection, not on
taste: such a route would need the same 35-account frame, the same evaluator, the
same Custody transfer and the same receipt. It would be
`terminal_settlement_v3.rs` re-typed, which is the restated-array defect §7.4
exists to name, at 780 lines instead of six.

**`CallerRole` gains `Claims = 1`** — `ExecutionRoleV1::Claims`'s own
discriminant, so a byte on a Claims wire and a byte in a caller-authority seed
list keep meaning the same thing. It names the case that was never spellable: the
Claims program executing a top-level route, with no external caller program in
the chain at all.

**Which wires admit it is each wire's own decision, and three of the four still
refuse it.** Every Claims wire already owned a private `decode_role`; that was
not an accident and it is what keeps the blast radius at one route.
`signed_delta_v3::decode_role` admits `1`; `affine_batch_v2`,
`sparse_native_transfer_v1` and `ClaimsPlanV1` do not, because their authority is
a caller-program PDA and nothing else, so the role that means "no caller program"
has no proof to offer there. `signed_delta_v3::process` — the top-level submitted
plan — refuses role `Claims` explicitly rather than leaving it unsatisfiable by
accident.

### 8.3 What stands at coordinate 0, and why it is enough

The coordinate's meaning has always been *the party entitled to move these claims
proved it, and the proof is a signature*. There are exactly two kinds of entitled
party in this protocol and only one of them had a spelling:

| role | coordinate 0 | entitled party |
|---|---|---|
| `Core` | `CallerAuthoritySeedsV1` PDA under the Core program | a lifecycle orchestration |
| `Trading` | the same PDA under the activated Trading program | a venue transition |
| `Claims` | the Position owner's own signature | the wallet that holds the claims |

**A signature at coordinate 0 equal to `request.owner` is by itself the proof
that the Position is wallet-held.** A program-derived address has no private key,
so neither a Trading record owner nor a Claims capability owner can produce it.
The route therefore needs no owner-kind tag, no admission-record read and no
thirty-sixth account to tell the families apart — the discriminator is the
signature it already demands.

The rest of the chain closes with no new guard:

- `product_basis_terminal_v3::validate_joins` refuses unless the Position
  header's `owner` equals `request.owner`;
- `signed_delta_v3::build_candidates` refuses unless the Position ACCOUNT is the
  canonical `ProtocolPositionSeedsV2` PDA under `(aggregate, owner)`;
- `authenticate_releases` requests Core and Claims always and Trading only when
  the caller actually is Trading, and under role `Claims` it pins coordinates
  14/15 to this program and its ProgramData — so nothing in the frame is left
  unauthenticated where the caller-program coordinates used to carry a venue.

**No lien is bypassed, because LBV2 has none.** A Position is a header plus a
balance vector with a zero-checked reserved word: no escrow field, no lock, no
delegate. Owner authorization at terminal cannot skip an encumbrance that the
state cannot express. Direct's inline trade moves claims by
`SparseNativeTransferV1` between Positions that must already be live; it never
pledges one.

### 8.4 The one privilege that had to move

The frame spec pins coordinate 0 to a READONLY signer. That is exactly right for
a caller-authority PDA — never a fee payer, never written — and it is
**unsatisfiable for an owner signature**: an account that is both a transaction's
fee payer and a readonly signer compiles to one WRITABLE signer entry, so there
is no message in which a single wallet can present itself readonly. A browser has
one wallet, and it pays.

Writability at that coordinate carries no authority. The program never borrows
the account beyond `key` and `is_signer`; it cannot be the aggregate or a
Position (both are Claims-owned PDAs that cannot sign); and the runtime refuses a
write to an account this program does not own. So the pin is relaxed for
`CallerRole::Claims` ONLY and only along that one axis — signer stays required,
executable stays refused. `the_position_owner_pays_its_own_fee_and_still_authorizes_the_payout`
is the campaign transaction that keeps the relaxation from being dead code.

### 8.5 Evidence

`programs/dclutch-claims-sbf/tests/rational_representation_v2_program_test.rs`,
real ELFs, real Custody CPI, real Token-2022.

The fixture needed no new prestate. It has ALWAYS carried `actor_position` — a
canonical LBV2 Position at `(aggregate, actor.pubkey())` owned by an ordinary
keypair — and nothing in the tree could pay it. The gap was sitting inside the
campaign that proves the rest of the family works. One constant moved:
`ACTOR_CLAIMS` gained a claim at a LOSING coordinate, because that is the only
way to reach the zero-payout branch from a wallet.

Executed:

- the wallet payout itself — Hoard and recipient move by exactly
  `quantity × payout`, the winning coordinate is burned out of the Position, the
  aggregate's supply falls by the same atoms, no other coordinate moves, both
  revisions advance by one, and the Claims-role cursor advances from the value
  `InitializeReplay` wrote;
- the same payout with the OWNER as fee payer (§8.4);
- a losing coordinate: pays zero, burns the claim, Custody is never invoked, the
  Hoard and recipient are byte-identical and the cursor does not move. This is
  `terminal_settlement_v3`'s zero-payout branch, which nothing had ever executed;
- a stale Custody cursor after a partial redemption — the double-spend shape,
  refused by the replay;
- seven hostiles, each byte-identical afterwards: the owner named but not
  signing; a signer who is not the owner; a stranger claiming to own the
  Position; another party's Position at this same Market; a substituted terminal
  certificate; role `Claims` with a foreign caller program; role `Trading`
  offering an owner signature where its release-pinned PDA belongs;
- an unresolved Market, walked back to `Phase::Open` through the bank.

**§5's blocked row is closed.** `claims/terminal_settlement_v3::process` has a
campaign.

### 8.6 What this does NOT establish

- **The wallet Position is still not CLOSABLE.** `protocol_position_v2`'s Close
  admits an exact-zero balance vector, and a terminal redemption debits ONE
  coordinate — so a wallet holding losing claims alongside the winner must burn
  each of them before the vector is zero, and even then the close join at
  `composition_v3::require_sparse_close_join` is `TradingRecord`-only. Reclaiming
  a wallet Position's prefunded rent has no route. Named, not fixed.
- **Nothing here creates a wallet Position.** The `User` branch of the
  ProtocolPosition Admit route is reachable in principle and has no shipped
  caller; `founding_v5` is the only producer, and it allocates the accounts
  itself. FE-TRADER's "wallet-side Position admission route" is still open.
- **`parent_context` stays caller-owned** under role `Claims`. It is not read by
  any derivation in this mode — the authority is the owner, not a PDA under it —
  so it is entropy in the request digest and nothing more.
- **No live-validator evidence.** ProgramTest is a fast lane; see
  `tools/gauntlet/TIERS.md`.
