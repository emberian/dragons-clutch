# The cliff doctrine, v1

**Head current at `330bbfaba` (2026-09-04), tree root `/Users/ember/dev/dclutch`.** The body below `## History` is the doctrine as measured at `7e3cac9e` (2026-08-31) with the LIFT-1312 corrections, verbatim; this head states the doctrine and the corrected claims.

## What is true now

Every fixed bound is exactly one of **PHYSICS** (derived from a chain
constant, which must be named), **PURCHASABLE** (buyable at a written price:
a page, a second transaction, an ALT, a wider record plus rent, a schema bump
plus the lineage migration, a wider Lean proof plus corpus), or
**SESSION-SPLITTABLE** (an artifact of doing something in one shot; a staged
design dissolves it, and ember has pre-ruled multi-transaction lifecycles
acceptable). Almost nothing in the tree is physics: three genuine walls
(packet, CU, account locks), a small set of bounds derived from them, and two
habits — one Lean literal (`1312`) and a house habit of writing `16`.

Corrected by the chartered lift that was investigated and deliberately not
taken (`3be5072c`):

- **The binding cliff is the 1,232-byte packet, not the 1312 record bound.**
  Structured full-width issuance at K=3 is over the packet with the ALT already
  spent; raising 1312 admits descriptors that publish but can never issue. The
  real lift is session-splitting issuance, queued.
- The 42-instruction cap divides by 24 (the `TransitionVMV2` stride), not 16.
- `CAPABILITY_PROGRAM_SET_MAX_BYTES_V2` is 2336, not 1312.
- `1312` has four independent Lean authors, each now naming the others, with
  the derivation beside them; nothing physical selects it.
- `RATIONAL_OPEN_STRUCTURED_MAXIMUM_COORDINATES_V3` is solved from the formula,
  and the wall ordering is a checked assertion in the packet test.

**Maintenance rule.** A new fixed bound enters the tree only with its class
and, for PURCHASABLE, its price in the defining comment.
`docs/OMISSION_INDEX.md`'s P-rows cite their class here.

## History

# The cliff doctrine, v1

Status: reference, measured at HEAD 7e3cac9e (2026-08-31). Answers ember's
twice-dropped question — "why do we have all these shitty bounds?" — by
classifying every fixed bound in the protocol, and closes with the
commit-don't-inline shortlist and the lift candidates the next charter should
draw from. No implementation and no wire change is proposed here.

## The doctrine

Every fixed bound is exactly one of:

- **PHYSICS** — derived from a chain constant. Movable only if the chain
  moves. Naming the chain constant is mandatory; "it felt big enough" is not
  physics.
- **PURCHASABLE** — buyable with a **named price**: an account page, a second
  transaction, an ALT, a wider record (plus its rent), a schema bump plus the
  priced lineage migration, or a wider Lean proof plus its corpus. A bound in
  this class is only healthy while its price is written down.
- **SESSION-SPLITTABLE** — an artifact of doing something in one shot. A
  staged / multi-transaction design dissolves it. Ember has pre-ruled
  multi-tx lifecycles acceptable, so nothing in this class is load-bearing
  ontology; it is all sequencing convenience.

The honest one-line answer to the question: **almost nothing here is
physics.** The tree has three genuine walls (packet, CU, account locks), a
small set of bounds correctly derived from them, and then two habits that
generated everything else: one unexplained Lean literal (`1312`) whose
consequences cascade into the sharpest user-visible cliffs, and a house habit
of writing `16` wherever a width was needed.

### Reading traps (learned while measuring)

1. **`MAX`/`LIMIT` in a name does not mean bound.** The scalar-index
   vocabulary uses the same words for register *positions*:
   `DEALER_SCENARIO_MAX_POSITION_COUNT_SCALAR_V4 = 5` is register slot 5 (the
   real bound, P = 2, lives as a constant inside the transition program —
   `programs/dclutch-trading-sbf/src/dealer/v3_trade_artifacts.rs:116`), and
   the `FILL_SCALAR_*_MAXIMUM/LIMIT` and `SCALAR_BUYER/SELLER_*` families are
   all bank indices. Any future census must exclude the `_SCALAR_` namespace.
2. **Some `MAX` constants are measurements, not gates.**
   `STRUCTURED_HOT_MAX_TOKEN_EFFECTS_V2 = 257` says of itself: "a
   capacity-profile measurement and has no executable meaning; do not size
   against it" (`crates/dclutch-structured-v2-contract/src/hot_v2.rs:43`).
3. **Sentinels:** `MARKET_PRINCIPAL_CAP_*_UNBOUNDED_V1` are u64/u128 max
   values meaning "no cap", not caps.

## 0. The three walls (chain physics, with in-tree witnesses)

| Chain constant | Value | In-tree echo | Witness of proximity |
|---|---|---|---|
| Transaction packet | 1,232 bytes | measured wire asserts | Direct inline routed v0 is **1,167 / 1,232 bytes** (`crates/dclutch-operator/src/direct_inline_route_v3.rs:6085`) — 65 bytes of headroom on the flagship route |
| Per-tx compute | 1,400,000 CU | `TRANSACTION_COMPUTE_UNIT_LIMIT_V1` (`crates/dclutch-operator/src/registry.rs:63`), `DIRECT_HOT_COMPUTE_UNIT_LIMIT_V1` (`crates/dclutch-operator/src/direct_inline_v3.rs:206`) | hot routes request the full 1.4M |
| Per-tx account locks | 64 | `SOLANA_DEVNET_ACCOUNT_LOCK_LIMIT_V1` (`crates/dclutch-operator/src/dealer_scenario_hot_v4.rs:102`), `GENERAL_INVOCATION_MAX_UNIQUE_LOCKS_V1 = 64` (`crates/dclutch-general-adapter-contract/src/invocation_v1.rs:34`) | Direct inline locks **61 / 64** (4 static + 57 ALT-loaded, same test) |
| Program heap | 32 KiB default, 256 KiB max | `ADAPTER_DEFAULT_HEAP_BYTES` / `ADAPTER_MAX_HEAP_BYTES = 256*1024` (`programs/dclutch-trading-sbf/src/entrypoint_adapter.rs:256,262`), gated per-route by `declares_extended_heap_profile_v1` | the P-006 defect (a route missing from the heap-profile list refusing `0x4008` unconditionally) shows the gate is live |
| PDA seeds | ≤ 16 seeds × ≤ 32 bytes | `MAX_SEEDS = 16`, `MAX_SEED_BYTES = 32` | `GENERAL_MAX_STATE_SEEDS_V3 = 5` sits well inside it |
| Account data | 10 MiB max; +10,240 bytes realloc per ix; rent per byte | `SOLANA_MAX_PERMITTED_ACCOUNT_DATA_BYTES` | no record class is within two orders of magnitude of the ceiling — **account width is never the wall here; rent and schema discipline are** |

ALT is **already purchased** on the two routes that needed it: Direct inline
(57 loaded addresses, above) and General at N=258
(`docs/evidence/GENERAL_ALT_PACKET_WITNESS_2026_08_27.md` — all seven actions'
account sets derived and packet-checked as v0 transactions).

## 1. Bounds by domain

Columns: value; defined at; enforced at (the refusal a user actually hits);
what breaks; class; price or split. "Lean" marks a Lean-emitted definition
(single author, regenerate to move).

### 1.1 Trading hot path

| Bound | Value | Defined | Enforced / breaks | Class | Price / split |
|---|---|---|---|---|---|
| `HOT_FIXED_ACCOUNT_COUNT_V3` | 39 | `crates/dclutch-capability-program-contract/src/hot_v3.rs:136` | fixed frame of every hot execution; runtime accounts start at 39, so family breadth competes for the remaining ~22 locks | PHYSICS-derived profile | the ceiling (64 locks) is physics; the frame itself moves only with a v4 frame schema. An action too wide for the residue is SESSION-SPLITTABLE (stage the action) |
| `HOT_EXECUTION_ENVELOPE_BYTES_V3` | 128 | `hot_v3.rs:39` | family request data begins at offset 128; envelope fields beyond 128 need a schema bump | PURCHASABLE | packet bytes — every envelope byte is a wire byte on a route already at 1,167/1,232 |
| `TRADING_MAX_HOT_BANK_PAGES_V3` | 10 | `programs/dclutch-trading-sbf/src/lib.rs:275` | register-bank transport pages per execution | PURCHASABLE | more pages = more account locks; priced in locks, so effectively capped by the 64 wall |
| `TRADING_MAX_HOT_PHYSICAL_REPRESENTATIVES_V3` | 251 | `lib.rs:283` | physical account coordinates addressable by one execution (u8 coordinate space minus the 5 frame-carried) | PHYSICS-derived (u8 addressing) | a u16 coordinate is a schema bump, but pointless below the 64-lock wall — locks bind first |
| General common register bank | 90 scalars | `GENERAL_TRANSITION_COMMON_SCALARS_V3` / `GENERAL_HOT_COMMON_SCALARS_V3` (Lean; `crates/dclutch-general-adapter-contract/src/generated_transition_programs_v3.rs:3`, `hot_candidate_v3.rs:26`) | width of General's common bank; feeds scratch-page geometry (90×8 + identities×32) | PURCHASABLE | wider bank = more scratch pages = more locks; regenerate from Lean |
| Compute limits | 1.4M CU | §0 | route refuses past budget | PHYSICS | — |

### 1.2 Transition VM and capability programs

| Bound | Value | Defined | Enforced / breaks | Class | Price / split |
|---|---|---|---|---|---|
| `MAX_INSTRUCTIONS` (VM profile) | 64 | `crates/dclutch-transition-vm/src/lib.rs:21` ("first measured VM profile") | `Error::InvalidCount` on parse; a transition program longer than 64 instructions cannot exist | SESSION-SPLITTABLE (P-003's own path: staged computation certificates, AOT), PURCHASABLE interim (wider measured profile, price = CU re-measurement) | the floor that survives every split: the **final economic commit stays bounded and atomic** (P-003) |
| `CAPABILITY_PROGRAM_TRANSITION_MAX_INSTRUCTIONS_V2` | 42 | Lean-derived: `(1312 − headers)/16` (`formal/dclutch-semantics/DClutchSemantics/CapabilityProgramAbi.lean:53-56`; emitted `crates/dclutch-capability-program-contract/src/generated.rs:4`) | a capability descriptor's transition program caps at 42 instructions — **tighter than the VM's 64, purely because of the 1312 record literal** | PURCHASABLE | raise `finalizedRecordMaxBytes` and regenerate — see §5.1 |
| `MAX_SCALARS` / `MAX_IDENTITIES` (VM) | 64 / 16 | `transition-vm/src/lib.rs:23,25` | register bank width per program | PURCHASABLE | wider profile + CU re-measurement |
| `MAX_EFFECTS` | 7 | `crates/dclutch-effect-kernel/src/lib.rs:26` | effects per transition | PURCHASABLE / SPL | wider profile, or split the action; `EFFECT_V4_MAX_EXTENSION_LEAN = 63` (`generated_v4_abi.rs:10`) already stretches v4 |
| `CAPABILITY_PROGRAM_MAX_BYTES_V1` / `ROOT_ACCOUNT` / `ROOT_STATE` | 1304 / 4328 / 4096 | Lean (`capability-program-contract/src/generated.rs:3,5,6`) | descriptor record and root account budgets | PURCHASABLE | wider record class + rent; root growth is realloc-priced |
| `ACTIVATION_MAX_ROLE_REQUEST_BYTES_V2` / `RUNTIME_IDENTITIES` / `RUNTIME_SCALARS` | 2048 / 32 / 96 | `capability-program-contract/src/lib.rs:159,164` | activation request geometry | PURCHASABLE | wider profile; activation is already its own transaction (split is paid) |

### 1.3 Capability manifest and graph (P-002)

| Bound | Value | Defined | Enforced / breaks | Class | Price / split |
|---|---|---|---|---|---|
| `MAX_CAPABILITIES_V1` | 16 | Lean (`crates/dclutch-capability-contract/src/generated_abi.rs:8`) | `lib.rs:547,561,705`; a Market may select at most 16 capabilities | SESSION-SPLITTABLE / commit-don't-inline | P-002's own lifting path: **ordered paged graph with one aggregate commitment** — §4.1 |
| `MAX_DEPENDENCIES_PER_CAPABILITY_V1` | 16 | Lean (`generated_abi.rs:9`) | `lib.rs:325` | same | same |
| `CAPABILITY_MANIFEST_MAX_BYTES_V1` | 8464 | Lean (`generated_abi.rs:7`) | manifest record width — derived from 16×16 | DERIVED | falls with the paged graph |
| `CAPABILITY_PROGRAM_SET_MAX_ENTRIES_V2` | 32 | Lean (`generated_set_v2.rs:5`; bytes 2336 `:4`) | `set_v2.rs:206,345` — a release set caps at 32 entries (General already spends 8 of them) | PURCHASABLE / commit-don't-inline | wider record now; same paged-graph shape later |
| `CAPABILITY_FUNDING_MAX_ENTRIES_V1` | 16 | `crates/dclutch-market-core-codec/src/generated_physical.rs:11` | funding entries at founding | SESSION-SPLITTABLE | staged founding — §4.4 |
| `GENERIC_FOUNDING_MAX_FUNDING_STATES_V1` | 16 | `generic_founding_v1.rs:44` | funding states created by one atomic founding | SESSION-SPLITTABLE | the atomic-set choice is O-001's ("eager atomic creation of the exact selected subset"); a staged founding with an escrowed commitment dissolves the 16, at the price of a founding lifecycle instead of a founding instruction |
| Capability seal rows | 6 × 136 = 968 B | `crates/dclutch-capability-seal-contract/src/lib.rs:100,103,106` | `lib.rs:497` refuses `row_count != 6`; rows are exactly the artifact roles (Descriptor, LifecyclePolicy, AccountProfile, RequestProfile, TransitionProgram, EffectProgram — `canonical_order`, `lib.rs:159`) | PURCHASABLE | a 7th row = a 7th artifact role = new seal profile byte + its own close route (the P-006 ruling's profile gate already anticipates exactly this) |
| Activation cache | 1288 B = 48 + 5×248 | `crates/dclutch-registry-contract/src/activation.rs:37-42`; hard `data_len() != 1288` at `registry-activation-auth-v1/src/lib.rs:201-210` | five role slots — grows only if a sixth state-owning role exists | DERIVED (from O-003's five-role ruling) | a sixth role already requires a new profile per O-003; the cache widens in the same decision |

### 1.4 Product and outcomes (P-001)

| Bound | Value | Defined | Enforced / breaks | Class | Price / split |
|---|---|---|---|---|---|
| `MAX_OUTCOMES` | 16 | **four authors**: `crates/dclutch-economic-kernel/src/lib.rs:15` (hand-written), `crates/dclutch-realm-contract/src/generated_abi.rs:22` (Lean, `MAX_OUTCOMES_V1`), `crates/dclutch-dealer-codec/src/lib.rs:46` and `crates/dclutch-general-codec/src/lib.rs:23` (both re-exporting generated twins) | e.g. `programs/dclutch-trading-sbf/src/dealer/v3_trade_artifacts.rs:1225`; a Product V1 with 17 outcomes cannot found | PURCHASABLE — **not physics**: the composition kernel admits 256 (`MAX_COMPOSITION_OUTCOMES_V3`) and General executes at N=258 under ALT with a packet witness | P-001's measured path: erase width dispatch, contiguous runtime views, page only where packet/account/CU evidence requires. See §3.1 for the argument and §5.3 for the author-unification lift |
| `MAX_RESULT_OUTCOMES` / `MAX_RESULT_REGIONS` | 16 / 15 | `crates/dclutch-product-contract/src/result_domain.rs:15,17` | result domain width; regions = outcomes − 1 | DERIVED (from MAX_OUTCOMES) | moves with it |
| `MAX_PRICE_CELLS` | 15 | `crates/dclutch-resolution-policy-kernel/src/categorical_pyth_v1.rs:8` | price-cell boundaries per categorical resolution | DERIVED (= regions) | moves with it |
| `MAX_TERMS` / `MAX_KNOTS` (V1, V2) | 16 / 16 | Lean (`product-payoff-codec/src/generated.rs:8,10`; `product-payoff-v2-codec/src/lib.rs:29,31`) | payoff terms and knots per product record | PURCHASABLE / commit-don't-inline | wider record (`ProductBasisV3` still holds 2+48 zero-enforced reserved bytes, `runtime_v3.rs:300-301`); the digest root already exists — §4.3 |
| `MAX_PORTFOLIO_CLAIMS` | 16 | `crates/dclutch-product-contract/src/portfolio.rs:18` | claims per portfolio | PURCHASABLE | wider record |
| Spline family | degree ≤ 3, knots ≤ 12, width ≤ 10, support ≤ 4 | Lean (`liability-basis-v2-kernel/src/generated_spline.rs:6,8-10`; `BASIS_SPLINE_MAXIMUM_DEGREE_V3` `product-payoff-v2-codec/src/generated_runtime_v3.rs:14`) | the certified basis family's scope; degree-4 refuses | PURCHASABLE (price = proof) | the price is a wider Lean theorem + emitted hostile corpus — the U-013 pattern, already walked once |
| Price gate | atoms ≤ 10, width ≤ 10 | Lean (`generated_price_gate.rs:7`) | degree-≥2 certificate scope | PURCHASABLE (proof) | same |
| `TRANSITION_MAX_WIDTH_V2` | 4 | Lean (`liability-basis-v2-kernel/src/generated.rs:162`) | claim-width per basis transition | PURCHASABLE (proof) | same |
| Composition V3 | nodes 32, edges 96, outcomes 256, terms 2048, product width 512, repr width 256, exposure terms 65,536 | `crates/dclutch-representation-composition-v3-kernel/src/abi.rs:12…` (Lean twins beside each) | `exposure.rs:539-541,623`, `translation.rs:206`, `abi.rs:405`, `graph.rs:361,802` | PURCHASABLE | wider measured profile + CU evidence; these are the *generous* bounds — the pain lives in the 16s below them |

### 1.5 Structured / Fractional / Rational

| Bound | Value | Defined | Enforced / breaks | Class | Price / split |
|---|---|---|---|---|---|
| `STRUCTURED_CHILD_MAXIMUM_OUTCOMES_V2` | **3** | `crates/dclutch-structured-v2-operator/src/child_request.rs:68` | `descriptor.rs:121` refuses outcome_count 0 or > 3 — **a Structured product over a 4-outcome Market cannot exist** | DERIVED — from `REQUEST_PROFILE_MAX_BYTES_V1 = 1312`: (1312−32)/24 = 53 ops, projection costs 29 + 8K, so K = 3; the in-tree comment says the account ceiling would permit more | lift the 1312 — §5.1 |
| `RATIONAL_OPEN_STRUCTURED_MAXIMUM_COORDINATES_V3` | **3** | `crates/dclutch-bearer-v2-operator/src/open_structured_v3.rs:97` | same formula, same refusal at encode for K = 4 | DERIVED (same generator) | same |
| `FRACTIONAL_REPRESENTATION_WIDTH_MAX_V1` | 256 | `crates/dclutch-claims-svm/src/fractional_claim_check_v1.rs:148` | fractional representation width | PURCHASABLE | wider check profile |
| `FRACTIONAL_MAX_SETTLEABLE_WIDTH_V4` | 64 | `crates/dclutch-fractional-claim-operator/src/selected_release_v4.rs:132` | settleable width per action | SESSION-SPLITTABLE | settle in tranches across transactions |
| `FRACTIONAL_HOT_MAX_TOKEN_EFFECTS_V2` | 256 | `crates/dclutch-fractional-claim-contract/src/hot_v2.rs:24` | token effects per hot execution | SESSION-SPLITTABLE | batch across transactions; per-tx CU binds first |
| `STRUCTURED_MAX_COORDINATES_V2` | 256 | Lean (`structured-v2-kernel/src/generated_abi.rs:5`) | kernel-side coordinate scope | PURCHASABLE (proof) | wider kernel profile — irrelevant while the wire caps at 3 |

### 1.6 Dealer

| Bound | Value | Defined | Enforced / breaks | Class | Price / split |
|---|---|---|---|---|---|
| Scenario positions | **P = 2** | constant inside the transition program (register slot named at `programs/dclutch-trading-sbf/src/dealer/v3_trade_artifacts.rs:116`) | a scenario carrying 3+ positions refuses in-transition | PURCHASABLE (proof + profile) | the scenario kernel's solvency argument is proved at P = 2; widening is a kernel theorem plus a wider witness record |
| `DEALER_SCENARIO_MAX_RESERVATIONS_V1` | 4 | `crates/dclutch-dealer-codec/src/scenario_reservation_receipt_v1.rs:12` | reservations per receipt | PURCHASABLE | wider receipt record |
| Checkpoint / membership pages | 48 / page | `dealer_scenario_checkpoint_v1.rs:114`, `scenario_membership_manifest_v1.rs:15` | page granularity only — **already paged**, so not a cliff | — | model citizen: paging live |
| `DEALER_SCENARIO_MAX_EVIDENCE_SPAN_COUNT_SCALAR_V4` | 100 | `v3_trade_artifacts.rs:132` | evidence spans per scenario | PURCHASABLE | wider record |
| `MAX_MULTI_LP_CUSTODY_EFFECTS_V3` / `MAX_DEALER_SCENARIO_CUSTODY_EFFECTS_V3` / `MAX_CUSTODY_TRANSFERS` | 3 / 4 / 1 | `v3_multi_lp.rs:73`; dealer codec; `economic-kernel/src/lib.rs:19` | custody legs per action | SESSION-SPLITTABLE | stage custody legs across transactions |
| `DEALER_GLOBAL_SELECTOR_MAX_V3` | 9 | `v3_release.rs:81` | selector vocabulary width | DERIVED (enum) | grows with the release, not a user cliff |

### 1.7 Series — the model citizen

| Bound | Value | Defined | Enforced / breaks | Class | Price / split |
|---|---|---|---|---|---|
| `SERIES_MAXIMUM_MERKLE_HEIGHT_V3` / `SERIES_ACTION_MAXIMUM_PROOF_HEIGHT_V3` | 32 / 32 | Lean (`series-v3-kernel/src/generated.rs:7`) | proof height — 2³² leaves; effectively unbounded content behind a committed root | **the commit-don't-inline pattern, already live** | the only real cost is proof bytes per transaction (packet physics) |
| `SERIES_MAX_FUNDING_STATES_V3` | 64 total | `series-v3-kernel/src/lib.rs:1024` | funding states per series | PURCHASABLE | wider profile |
| `SERIES_CONSUME_MAXIMUM_FUNDING_STATES_V3` | 16 per action | `programs/dclutch-trading-sbf/src/series/artifacts_v3.rs:117` | consumed per invocation | **already session-split** (64 total vs 16 per action — the split exists) | — |

Series proves both doctrines at once: unbounded membership behind a merkle
root, and a per-action cap that is a page size rather than a ceiling.

### 1.8 Source / Resolution

| Bound | Value | Defined | Enforced / breaks | Class | Price / split |
|---|---|---|---|---|---|
| `MAX_SHARED_OBSERVATIONS` | 16 | `crates/dclutch-source-contract/src/lib.rs:144` | observations per source | PURCHASABLE | wider record |
| `MAX_RECOVERY_ATTEMPTS` / `RECOVERY_POLICY_MAX_ATTEMPTS_V2` | 4 / 4 | `lib.rs:129`; Lean (`generated_source_recovery_policy_v2.rs:4`) | recovery attempts before terminal admission path | POLICY (deliberate, not a width) | changing it is a policy decision, not a purchase |
| `SCHEDULED_MEDIAN_CORPUS_MAX_SAMPLES_V1` | 7 | Lean (`generated_scheduled_median_v1.rs:15`) | median corpus width | PURCHASABLE (proof) | wider proved corpus |

### 1.9 Relay (mainnet-state family)

| Bound | Value | Defined | Enforced / breaks | Class | Price / split |
|---|---|---|---|---|---|
| `MAX_RELAYED_ACCOUNTS_V1` / `MAX_RELAYED_INLINE_BYTES_V1` | 8 / 448 | Lean (`relay-contract/src/generated_relayed_abi.rs:3,4`) | one attestation must fit its ed25519 verification in one transaction (`MAINNET_STATE_RELAY.md` §4.4 derives the geometry from the packet) | PHYSICS-derived | — |
| `MAX_RELAYER_KEYS_V1` | 5 | `generated_relayed_abi.rs:5` | relayer key-set width (the key set *is* the provider release, §4.5) | POLICY | a release decision |
| `RELAYED_RECORD_MAX_BYTES` | 4792 | `generated_relayed_abi.rs:150` | relayed record body — **already chunked** (ack-chunk machinery), i.e. already session-split | PURCHASABLE | more chunks |

### 1.10 Registry, records, core state

| Bound | Value | Defined | Enforced / breaks | Class | Price / split |
|---|---|---|---|---|---|
| `CoreState` width | 368 B, gapless, zero reserved | Lean (`crates/dclutch-market-core-codec/src/generated.rs:3`) | 38 `!= STATE_BYTES` refusal sites at HEAD | PURCHASABLE — **and the price is already written**: `RELEASE_LINEAGE_MIGRATION_V1.md` (new record class, `Registry::DeclareSuccessor`, per-market `Core::MigrateMarket`, `MigrationBountyV1` 88 B funded by the upgrader) | the zero-reserved packing is deliberate: widening was priced as migration, not as slack |
| `ExecutionReleaseSetV1` | 336 B exact (4 reserved) | `release-set-contract/src/lib.rs:54-61`, `require_zero` `:637` | any new fact = new account class (widening re-hashes every release-set id — `RELEASE_LINEAGE_MIGRATION_V1.md` §3.5) | PURCHASABLE (same priced path) | — |
| `ArtifactReleaseV1` / `ReleaseLineageV1` | 216 / 248 B | `registry-contract/src/artifact.rs:12`, `lineage.rs:55` | packed records | PURCHASABLE (same) | — |
| Registry action namespace | actions 0 and 1 only | `registry-svm/src/lib.rs:117-134` | a new Registry verb costs a new 8-byte magic + a sub-dispatcher branch (§3.5) | PURCHASABLE | price named in the migration doc |
| `MAXIMUM_TICKET_BYTES_V1` | 4096 | `direct-ticket/src/envelope.rs:16`, enforced `:119,133` | ticket text width | PURCHASABLE | wider envelope class + rent |
| `REQUEST_PROFILE_V4_MAX_ROWS` / `ROW_BYTES` | 256 / 4096 | Lean (`request-profile-contract/src/generated_v4.rs:5`) | v4 row machinery — the *wide* profile vocabulary already exists | — | evidence the K=3 lift has somewhere to land |
| `MAX_CURRENT_RENT_QUOTES_V5` | 16 | `account-profile-contract/src/lifecycle_v3.rs:73` | rent quotes per lifecycle record | PURCHASABLE | wider record |
| `PROJECTED_MARKET_MAX_AFFINE_COUNT_V2` | 16 | `programs/dclutch-trading-sbf/src/projected_market_v2.rs:28` | affine projections per market | PURCHASABLE | wider record |
| `MAX_RECEIPT_DEPENDENCIES_V4` | 32 | `execution-strategy-contract/src/shadow_digest_v3.rs:225` | receipt dependency fan-in | PURCHASABLE | wider digest record |
| `LIFECYCLE_RENT_CREDIT_BYTES_V2` | 128 | `rent-contract/src/lifecycle_v2.rs:16` | rent-credit record width | PURCHASABLE (new class, same pattern) | — |
| `MAX_INERT_METADATA_VALUE_BYTES_V2` | 65,535 | `token-svm/src/behavior_profile_v2.rs:52` | Token-2022 metadata value (u16 length in the token program's own format) | PHYSICS (foreign program's wire) | — |
| `CANONICAL_RECORD_MAX_STAGING_LIFETIME_SLOTS_V1` | 216,000 (~24 h) | `record-contract/src/lib.rs:42` | staging expiry — a **time** bound, different genus; griefing guard, not a width | POLICY | — |

## 2. Arguable classifications (both sides, and a pick)

**`MAX_OUTCOMES = 16` (P-001).** The physics case: every outcome widens the
register bank, payoff table, claims width and packet/CU footprint at once, and
the hot route has 65 wire bytes and 3 locks of headroom. The profile case: the
composition kernel already admits 256, General's N=258 account sets are
packet-witnessed under ALT, and P-001 itself orders width-dispatch erasure and
*measurement* before paging. Pick: **PURCHASABLE** — the walls are real but
nobody has shown 16 is where they sit; the price is P-001's measured
contiguous-views campaign, then paging exactly where evidence demands.

**`MAX_INSTRUCTIONS = 64` (P-003).** The physics case: instructions cost CU
and the commit must land in one transaction. The split case: P-003 pre-blesses
staged computation certificates and AOT, with only the final economic commit
required to stay bounded and atomic. Pick: **SESSION-SPLITTABLE**, with the
atomic-commit floor named as the part that never dissolves.

**Founding at 16 funding states.** One-shot founding is O-001's "eager atomic
creation of the exact selected subset" — atomicity is a chosen virtue, and 16
states already brushes the 64-lock wall through created accounts plus roles.
A staged founding (commit the funding-set root, then activate pages) is
strictly more capable and ember has pre-ruled multi-tx lifecycles. Pick:
**SESSION-SPLITTABLE**; the founding *commitment* stays atomic, the
materialization pages.

**`HOT_FIXED_ACCOUNT_COUNT_V3 = 39`.** It looks like a design constant, but
its ceiling is the lock wall and its floor is the role architecture — every
one of the 39 is an authenticated party. Pick: **PHYSICS-derived profile**;
the movable part is whatever a staged action can evict from the single-shot
frame.

## 3. The generative constants

Most of §1 is produced by four numbers:

1. **`finalizedRecordMaxBytes = 1312`** (`CapabilityProgramAbi.lean:53`) — a
   bare literal with no derivation and no comment, below the account ceiling
   its own consumers acknowledge. Generates: the 42-instruction capability
   transition cap, `REQUEST_PROFILE_MAX_BYTES_V1` / `ACCOUNT_PROFILE_MAX_BYTES_V1`
   / `CAPABILITY_PROGRAM_SET_MAX_BYTES_V1` (all 1312), and through the K
   formula the Structured child width 3 and Rational coordinate width 3.
2. **The house 16.** Outcomes, capabilities, dependencies, funding entries,
   funding states, identities, knots, terms, portfolio claims, shared
   observations, rent quotes, affine count, series-consume page. Some are
   pages (fine), some are ceilings (P-001/P-002 own the two that matter);
   none of them is physics.
3. **The 64-lock wall**, which correctly parameterizes the devnet lock
   constants, General invocation locks, and (through frame arithmetic) the
   residual runtime-account budget of every hot route.
4. **The 1232-byte packet**, which correctly parameterizes the relay
   attestation geometry and is the measured wall (1,167) of the Direct inline
   route.

## 4. Commit-don't-inline shortlist

Which fixed inline lists should become a committed root on chain with
unbounded content proven against it. Ranked by (user pain when hit) ×
(migration cheapness). Series (§1.7) is the live model: root + height-32
proofs; Dealer's checkpoint/membership pages and the relay's chunked records
are partial precedents.

| Rank | Inline list today | Migration shape | Why this rank |
|---|---|---|---|
| 1 | **Request/AccountProfile operation lists** (53 ops in 1312 B → K=3) | profile root committed in the descriptor; operation pages proven against it at execution — or, nearer-term, the existing V4 row vocabulary (256 rows × 4096 B) | sharpest live user pain (Structured width 3); the wide vocabulary already exists in-tree; no new cryptographic machinery |
| 2 | **Capability manifest 16×16** (P-002) | P-002's own lifting path verbatim: finalized ordered paged graph, one aggregate commitment, unique kinds, checked acyclicity | design already blessed in the omission index; caps every Market's capability breadth; cost is a schema generation, not research |
| 3 | **Product payoff tables** (`MAX_TERMS`/`MAX_KNOTS` 16, outcome-indexed tables) | the root already exists — `CoreState` pins `identity.product_record` as a digest; content pages (`ProductBasisV3` siblings) proven against that digest | root is free (already committed); pain is real but P-001 orders measurement first |
| 4 | **Founding funding-state set** (16) | commit a funding-set root at founding; activate pages in staged transactions (pre-ruled acceptable) | dissolves a founding ceiling and shrinks the founding transaction's lock pressure at once |
| 5 | **Capability program set entries** (32) | same paged-graph treatment as rank 2, same generation | not yet painful (General uses 8) — do it when rank 2 lands, for free |

## 5. Lift candidates

Bounds that are UNJUSTIFIABLE as they stand: pure convenience, cheap to lift,
real user pain. The actionable list.

1. **`finalizedRecordMaxBytes = 1312`**
   (`formal/dclutch-semantics/DClutchSemantics/CapabilityProgramAbi.lean:53`).
   A bare literal, no rationale anywhere in the tree, sitting below the
   account ceiling its own consumers document. It single-handedly produces
   the two worst user cliffs (Structured child outcomes ≤ 3, Rational
   coordinates ≤ 3) and the 42-instruction capability transition cap. Lift:
   raise the literal (or derive it from a stated budget), regenerate the four
   emitted crates, re-measure CU/packet on the affected routes. Until it is
   lifted, its value deserves at minimum a comment stating what it buys.
2. **`STRUCTURED_CHILD_MAXIMUM_OUTCOMES_V2 = 3`** and
   **`RATIONAL_OPEN_STRUCTURED_MAXIMUM_COORDINATES_V3 = 3`** — the
   user-facing faces of (1). A prediction-market protocol whose structured
   products cap at *three outcomes* is wearing its one unexplained literal in
   public. Falls automatically with (1); listed separately because the pain
   is theirs, not the literal's.
3. **`MAX_OUTCOMES = 16` has four authors** — hand-written in
   `dclutch-economic-kernel/src/lib.rs:15`, Lean-emitted in the realm ABI,
   re-exported through dealer and general codecs, plus the derived
   `MAX_RESULT_OUTCOMES` twin. Before any widening, unify to one Lean author
   (the P-007 treatment, same shape as the seal-layout migration). Cheap,
   removes drift risk, and is a prerequisite for lifting P-001 safely.
4. **`MAX_OUTCOMES_V1` in the realm ABI is defined but unconsumed in its own
   crate** (`crates/dclutch-realm-contract/src/generated_abi.rs:22`) — either
   dead or mis-homed; fold into (3).

## 6. Bounds documented but not enforced (or not bounds at all)

Findings from the sweep, kept honest:

- `MAX_OUTCOMES_V1` (`realm-contract/src/generated_abi.rs:22`): no consumer
  in its defining crate; enforcement happens through the other three authors.
- `FRACTIONAL_MAX_REPRESENTATION_WIDTH_V2 = 256` exists only in a
  program-test fixture
  (`programs/dclutch-claims-sbf/program-test/fractional-atomic/src/narrow_fixture.rs:51`);
  the live bound is `FRACTIONAL_REPRESENTATION_WIDTH_MAX_V1`
  (`crates/dclutch-claims-svm/src/fractional_claim_check_v1.rs:148`).
- `STRUCTURED_HOT_MAX_TOKEN_EFFECTS_V2 = 257`: self-described capacity
  measurement, no executable meaning (`structured-v2-contract/src/hot_v2.rs:43`).
- The `_SCALAR_` register-index namespace (`DEALER_SCENARIO_MAX_POSITION_COUNT_SCALAR_V4`,
  `FILL_SCALAR_*`, `SCALAR_BUYER/SELLER_*`, `REGISTERED_SCALAR_*`): positions,
  not bounds — see the reading traps.
- `MARKET_PRINCIPAL_CAP_*_UNBOUNDED_V1`: sentinels.

## Maintenance rule

A new fixed bound enters the tree only with its class and, for PURCHASABLE,
its price in the defining comment. A bound found without either is a defect of
this doctrine's kind and belongs in §5 of its next revision.

## Corrections (2026-08-31, LIFT-1312, landed `3be5072c`)

The chartered lift of `finalizedRecordMaxBytes = 1312` was investigated and
deliberately NOT taken. Corrections to this document's claims:

- **The binding cliff is the 1,232-byte packet, not the 1312 record bound.**
  Structured full-width issuance at K=3 is 1,357 bytes v0 with the ALT
  already spent — K=3 is ALREADY unissuable; the packet caps issuance at
  K=2. Raising 1312 admits descriptors that publish but can never issue.
  The real lift is session-splitting issuance (staged/two-tx), queued.
- **The `/16` arithmetic is wrong**: the 42-instruction cap divides by 24
  (TransitionVMV2 stride); 16 is the V1 VM's.
- **`CAPABILITY_PROGRAM_SET_MAX_BYTES_V2` is 2336, not 1312** — the
  allocator class was already exceeded 6.45x by its own siblings.
- **1312 has FOUR independent Lean authors**, not one; each now names the
  others, and the derivation lives beside them (nothing physical selects
  1312 — account/rent/stack impose nothing at this scale).
- `RATIONAL_OPEN_STRUCTURED_MAXIMUM_COORDINATES_V3` is now solved from the
  formula rather than hand-written; the wall ordering is a checked
  assertion in the packet test.
