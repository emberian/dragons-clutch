# Release-set read-site census

Commit 1 of `docs/design/RELEASE_LINEAGE_MIGRATION_V1.md`. Read at
`/Users/ember/dev/dclutch`, HEAD `6c84b33a`, `STATE_BYTES = 360`.

The design splits one field into two: `selected_release_set` — the market's
name, seed component 6 of 9, never written after founding — and a new
`active_release_set`, the set the market authenticates its roles against. This
census classifies every site that reads a market's release set, so the split
is applied by evidence rather than by search-and-replace.

**The classification rule (M1).** A site is **ACTIVE** if and only if its value
reaches `ACTIVATION_PDA_DOMAIN_V1` derivation or the authentication of a role
out of that cache. Every other site is **FOUNDING**: PDA seeds, child-request
coordinates, stored copies, and consistency comparisons among those.

Every row was read at its own `file:line`. Where the census contradicts the
design, the contradiction is stated first.

---

## 1. What this census changes about the design

### 1.1 The ACTIVE surface is about 39 on-chain sites, not 6

The design predicts the sites needing `active_release_set` are six lines in
`dclutch-market-core-codec`. Those six are real, and §3 confirms them. But that
crate never touches an account: `ACTIVATION_PDA_DOMAIN_V1` does not appear in
it once. The six are the codec-side *shadow* of the binding. The binding itself
lives in the SBF adapters, and there are roughly thirty-nine of them across
seven programs (§4, §5).

The mechanism is one line. `crates/dclutch-registry-activation-auth-v1/src/lib.rs:186`
derives the cache from `[ACTIVATION_PDA_DOMAIN_V1, release_set_id]` and `:199`
requires the decoded cache to name that same id — so **whatever 32 bytes reach
the `release_set_id` parameter is the execution binding**, wherever they came
from. Core's own wrapper does exactly this at
`programs/dclutch-core-sbf/src/release.rs:128-144`, and its callers pass
`state.identity.selected_release_set.to_bytes()` literally.

### 1.2 Five persisted accounts carry their own execution binding

Commit 1's required verification (b) asks whether any persisted state stores an
activation-cache digest or a release-set id used as an execution binding, or
whether those are per-transaction request fields only.

**No persisted account stores a cache digest or a cache address** — every one of
those is instruction data or an ephemeral signer seed (§6.2). **But five
persisted account families store a release-set id that is used as the cache
seed** (§6.1). That is the same staleness hazard under a different name, and
the design's §7.2 asserts the opposite about one of them.

### 1.3 The lifecycle rent credit is the one that cannot be fixed in place

§7.2 says: *"Rent credit. Address `[domain, market, generation]`; stored
`release_set` equals `selected`. Untouched — which matters, because the rent
credit is where every other account's recovered rent lands."*

It is not untouched. `programs/dclutch-rent-sbf/src/lib.rs:690-691` says so in
its own words —

> the credit's own `release_set()` names the activation generation the address
> must be derived from

— and then does it, at `:697`:

```rust
let receipt = authenticate_activated_role_v1(
    accounts.registry_program,
    accounts.activation_cache,
    &state.release_set().to_bytes(),
    ExecutionRoleV1::Core,
```

and again at `:735`. The stored copy is not a founding coordinate that stays
equal; it is the execution binding, and after an upgrade it names a superseded
cache.

**And the credit cannot read the market to fix it.** The close frame does carry
`retired_market` (`rent-sbf/src/lib.rs:437`), but
`LifecycleRetiredMarketObservationV2::validate` refuses unless the market has
`data_len == 0 && lamports == 0`
(`crates/dclutch-rent-contract/src/lifecycle_v2.rs:150-151`) — the market must
be *fully closed* before its credit may close. So `active_release_set` is
unreadable by construction at exactly the moment the credit needs it.

**Consequence.** A market can migrate, trade, resolve and retire along the
lineage, and its rent credit still cannot be closed afterward. Migration as
designed unbricks the market and leaves the last hop of the money bricked.

**Open, and not decided here.** An account that carries its own execution
binding and provably cannot reach the market state needs either a
read-through-lineage frame — take the lineage record and the successor cache,
walk forward, no state change and no new bytes — or its own migrable field. The
first costs the credit nothing it does not have.

---

## 2. Method

Seeds swept: `selected_release_set` tree-wide; `release_set` within
`dclutch-market-core-codec`; `ACTIVATION_PDA_DOMAIN_V1`,
`authenticate_activated_role`, and `execution_release_set_id` tree-wide; then
every program under `programs/*/src/` that decodes a market.

For each hit the value was traced forward to either a cache derivation or a
non-cache consumer. Rows are grouped by that verdict.

---

## 3. The six predicted sites, read

| line | what it is | verdict |
|---|---|---|
| `crates/dclutch-market-core-codec/src/generated.rs:226` | `market_release_set_id != state.identity.selected_release_set` in `RetirementAdmissions::from_authenticated_batch` | ACTIVE — `market_release_set_id` is the adapter's cache seed |
| `generated.rs:227` | `selected.release_set_id != …selected_release_set` | ACTIVE — `selected` is `release_projection(view)` off the cache (`core-sbf/src/release.rs:147`) |
| `generated.rs:281`, `:282` | the same pair in `require_complete` | ACTIVE |
| `generated.rs:1217`, `:1219` | the same pair in `require_admission` | ACTIVE |
| `crates/dclutch-market-core-codec/src/physical.rs:725` | `references.release_set.release_set_id != …selected_release_set` in `CoreMarketViewV1::authenticate` | ACTIVE — the neighbouring flag is documented at `physical.rs:681-682` as "Exact release-set record **and Registry activation** were authenticated" |

The prediction is correct about these. It is incomplete about everything in §4
and §5.

**One row the design calls FOUNDING that M1 does not.** `generated.rs:804` and
`:807` are structurally identical to `:1217`/`:1219` — same `Admission`, same
cache-derived `selected`, same comparison. The design's reason for holding them
FOUNDING is *"founding authors both"*, which is a semantic liveness argument,
not a clause of M1. The outcome is safe (a market being founded has no lineage
to hop, and `found.rs:249` passes instruction data rather than state), but the
rule as written does not separate them. Recorded so a later reader does not
mistake the gap for a decision.

---

## 4. ACTIVE — the state field is the cache seed

Sixteen sites. Each passes `state.identity.selected_release_set.to_bytes()` into
a derivation or a role authentication.

| file:line | function |
|---|---|
| `programs/dclutch-core-sbf/src/capability.rs:137` | `process` → `authenticate_roles` (`:133`) |
| `programs/dclutch-core-sbf/src/execute_provider_v3.rs:114` | `process` → `authenticate_roles` (`:110`) |
| `programs/dclutch-core-sbf/src/fixed_role.rs:188` | `authenticate_fixed_role` (`:184`) |
| `programs/dclutch-core-sbf/src/open_market.rs:182` | `authenticate_open` → `authenticate_continuation_roles` (`:177`) |
| `programs/dclutch-core-sbf/src/retire_v1.rs:736` | `process_checkpoint_suffix` (`:732`) |
| `programs/dclutch-core-sbf/src/retirement_replay_handoff_v1.rs:131` | `authenticate_releases` (`:127`) |
| `programs/dclutch-core-sbf/src/begin_retiring.rs:41` | `process` → `authenticate_role` (`:35`) |
| `programs/dclutch-custody-sbf/src/retirement_replay_handoff_v1.rs:208` | `authenticate_current_roles`, three roles from `:226` |
| `programs/dclutch-trading-sbf/src/outer.rs:356`, `:366` | `process_activation` → `reauthenticate_role` → `outer.rs:1062` |
| `programs/dclutch-trading-sbf/src/outer.rs:614`, `:624` | `process_close`, same path |
| `programs/dclutch-trading-sbf/src/direct_token_setup_v1.rs:386` | `authenticate_activation`, derivation at `:388` |
| `programs/dclutch-resolution-proof-sbf/src/lib.rs:605` | `authenticate_market_and_resolution_release` → `:704` |
| `programs/dclutch-resolution-proof-sbf/src/relay_transport_v1.rs:280` | `authenticate_market`, derivation at `:296` |

These are the unambiguous ones: switch the argument to `active_release_set` and
the route follows the lineage.

---

## 5. ACTIVE by constraint — the class M1 mis-sorts

Twenty-three sites. The child program takes its cache seed from **instruction
data** and then requires the market state to equal it:

```rust
|| state.identity.selected_release_set.to_bytes() != request.release_set
```

Mechanically the state value does not *flow into* the seed — it *constrains* it
to one value. M1 as written therefore sorts these FOUNDING, and that verdict is
wrong for the lineage question: after a hop the request still carries the
founding set, so the child derives the dead cache and refuses.

**The resolution, where the child already holds the Core market account:** keep
the request pinned to `selected` (it is a founding coordinate and every child
address depends on it) and derive the *cache* from `state.active_release_set`.
Two facts, two authors, per §5.1 of the design.

| file:line | function | cache seed proven at |
|---|---|---|
| `programs/dclutch-custody-sbf/src/lib.rs:344` | `authenticate_market` | `lib.rs:306`, `:416` |
| `programs/dclutch-claims-sbf/src/lib.rs:1196` | `authenticate_core_market` | `:1022`, `:1030`, `:1038` |
| `programs/dclutch-claims-sbf/src/custody_replay_v1.rs:321` | `authenticate_core_rent_refund` | `:273` |
| `programs/dclutch-claims-sbf/src/market_closure_v1.rs:589` | `authenticate_core` | `:476` |
| `programs/dclutch-claims-sbf/src/sparse_native_transfer_v1.rs:519` | `authenticate_product_and_core` | `:418` |
| `programs/dclutch-claims-sbf/src/affine_batch_v2.rs:673`, `:722` | batch, `authenticate_core_market_v3` | `:484` |
| `programs/dclutch-claims-sbf/src/terminal_settlement_v3.rs:259`, `:606` | `authenticate_and_prepare`, `authenticate_core` | `crate::authenticate_activated_role` |
| `programs/dclutch-claims-sbf/src/rational_lifecycle_v2.rs:614` | `authenticate_market` | `:553` |
| `programs/dclutch-claims-sbf/src/rational_product_v3.rs:204` | `authenticate_core` | `authenticate_activated_role` |
| `programs/dclutch-dealer-sbf/src/lib.rs:694` | `authenticate_core_identity` | `:1196` |
| `programs/dclutch-trading-sbf/src/outer.rs:1014` | `authenticate_market_and_caller` | `:1062` |
| `programs/dclutch-trading-sbf/src/hot_v3.rs:10271` | `authenticate_market` | `:1362`, `:10299` |
| `programs/dclutch-trading-sbf/src/direct_begin_retiring_v1.rs:496` | `authenticate_market_bytes` | `:671` |
| `programs/dclutch-trading-sbf/src/dealer/v3_accelerator_accounts.rs:461` | `authenticate_core_market_v4` | accelerator frame |
| `programs/dclutch-resolution-proof-sbf/src/core_effect.rs:870`, `:1063`, `:1700` | three market authenticators | `:917`, `:1108`, `:1796` |
| `programs/dclutch-resolution-proof-sbf/src/provider_transport_v3.rs:377` | `authenticate_current_submission` | `:404`, `:489`, `:1033` |
| `programs/dclutch-resolution-proof-sbf/src/provider_instruction_v3.rs:512` | `authenticate_market_and_infrastructure` | `:591` |
| `programs/dclutch-core-sbf/src/generic_founding_v1.rs:1408` | `authenticate_generic_market` | `:672`, `:706` |
| `programs/dclutch-core-sbf/src/series_open.rs:396` | `authenticate_market_and_roles` | `:314`, `:323`, `:426` |

---

## 6. Persisted state

### 6.1 Five families store a release-set id used as a cache seed

| account | stored field | consumed as a seed at |
|---|---|---|
| **Lifecycle rent credit** (`crates/dclutch-rent-contract/src/lifecycle_v2.rs:167`) | `release_set` | `programs/dclutch-rent-sbf/src/lib.rs:697`, `:735` — **§1.3, cannot reach the market** |
| Claims aggregate `LiabilityBasisMarketViewV2` (`crates/dclutch-claims-svm/src/liability_basis_state_v2.rs:194`) | `release_set` | `programs/dclutch-claims-sbf/src/custody_replay_v1.rs:272` — fixable in place, its frame holds the live market |
| Dealer-scenario checkpoint | `input.release_set` | `programs/dclutch-trading-sbf/src/dealer_scenario_checkpoint_v1.rs:795`, `:1082` |
| Controller-funding checkpoint (`crates/dclutch-capability-contract/src/controller_funding_checkpoint.rs:218`) | `release_set` | `programs/dclutch-trading-sbf/src/projected_custody_bootstrap_v1.rs:662`, `:683`, `:877` |
| Projected-custody lock | `lock.release_set` | `projected_custody_bootstrap_v1.rs:1596`, `:2836` |

A hop mid-checkpoint strands that checkpoint; a hop at all strands the rent
credit's close.

### 6.2 No persisted account stores a cache digest or address

Every activation-cache digest is instruction data or an ephemeral signer seed,
recomputed live each transaction:

- `RoleBatchRequestV2::activation_cache_digest` — `crates/dclutch-registry-svm/src/batch_v2.rs:28`, checked at `programs/dclutch-registry-sbf/src/batch_v2.rs:53-55`
- `RegistryContinuationRequestV1` — `crates/dclutch-registry-svm/src/continuation_v1.rs:30`, checked at `programs/dclutch-registry-sbf/src/continuation_v1.rs:79`, consumed as a zero-lamport zero-data system-owned ephemeral signer (`programs/dclutch-core-sbf/src/release.rs:217-222`)
- `TransparentHotContinuationV2` — `crates/dclutch-registry-svm/src/continuation_v2.rs:28`, checked at `programs/dclutch-registry-sbf/src/hot_continuation_v2.rs:70-85`
- consumer-side re-hashes: `claims-sbf/market_closure_v1.rs:487`, `custody-sbf/lib.rs:426`, `rent-sbf/lib.rs:750`

The design's assumption holds for digests. It does not hold for ids.

---

## 7. FOUNDING — correctly, and these must not move

- **The name.** `crates/dclutch-market-core-codec/src/physical.rs:641` and `:657` — seed component 6 of 9.
- **The field's own encode/decode.** `generated.rs:409`, `:457`; `state_layout.rs:47`.
- **Caller-authority seeds.** `custody-sbf/retirement_replay_handoff_v1.rs:372`; `core-sbf/retirement_replay_handoff_v1.rs:268`, `:395`.
- **Custody vault, authority and replay seeds.** `custody-sbf/retirement_replay_handoff_v1.rs:393`, `:443`, `:450`, `:507`.
- **Rent-credit stored copies used as equality pins.** `core-sbf/retire_v1.rs:1744`; `core-sbf/retirement_replay_handoff_v1.rs:254`; `claims-sbf/market_closure_v1.rs:627`; `trading-sbf/outer.rs:1150`; `custody-sbf/retirement_replay_handoff_v1.rs:354`.
- **Child-request authoring.** `core-sbf/resolution.rs:518`; `trading-sbf/direct_replay_setup_v1.rs:287`; `trading-sbf/direct_token_setup_v1.rs:323`.
- **Receipt, aggregate and permit comparisons with no cache in the frame.** `core-sbf/resolution.rs:339`, `:531`; `core-sbf/open_market.rs:212`, `:638`; `core-sbf/capability.rs:825`; `core-sbf/execute_provider_v3.rs:181`; `core-sbf/fixed_role.rs:169`; `core-sbf/retire_v1.rs:924`, `:1279`, `:1695`; `core-sbf/generic_founding_v1.rs:1742`, `:1792`; `custody-sbf/retirement_replay_handoff_v1.rs:273`.

Off-chain operator and planner crates mirror both tiers. They are not a safety
boundary, but every one of them needs the same edit or preflight breaks.

---

## 8. Capability seals — §7.1's required verification, answered

**(a) Is re-minting permissionless?** Yes. `CapabilitySeal` dispatches at
`programs/dclutch-trading-sbf/src/lib.rs:495` to `hot_v3/seal.rs:72`, and the
only signer its frame requires is the rent payer (`seal.rs:84-90`). The doc at
`seal.rs:66-68` states the reason: the output is a pure function of immutable
public bytes.

**(b) Does absence degrade softly?** **No — it refuses hard.**
`process_hot_execution_v3` consults the seal unconditionally
(`hot_v3.rs:1982-1994`), and `authenticate_capability_seal_v3` refuses a
system-owned zero-length account on both the owner and width checks
(`seal.rs:401-412`). There is no unsealed fallback reachable from that route.

**Why this is a chore and not a brick.** Re-mint at the *same* address is
impossible — the mint requires the seal pristine (`seal.rs:143-151`). But a
Trading upgrade moves the seal to a **new, empty** address, because the key
joins on `trading_semantic_release`
(`crates/dclutch-capability-seal-contract/src/lib.rs:406-425`). Anyone may mint
there. So the cost is a mandatory one-time permissionless re-mint per
(descriptor, action) per Trading upgrade, during which the hot route refuses.

The accelerator route is structurally unsealed — it carries
`frame.capability_seal` but compares it by address only (`hot_v3.rs:1288`) and
re-derives every artifact live (`hot_v3.rs:670`–`:914`), deliberately, per the
comment at `hot_v3.rs:1103-1114`. That is a different instruction, not a
fallback.

---

## 9. What this census asks for

1. **Commit 5 is re-sized.** Not a codec change: about thirty-nine adapter
   sites across seven programs, of which twenty-three need the split applied to
   a request/state pair rather than to a single argument.
2. **A ruling on persisted execution bindings.** Four of the five families can
   read the live market and be fixed in place. The lifecycle rent credit
   provably cannot, and it is where the recovered rent lands.
3. **Seal re-minting becomes a step of the upgrade plan**, alongside the
   lineage declaration, or the first hot transaction after every Trading
   upgrade refuses for a reason no caller can diagnose.
