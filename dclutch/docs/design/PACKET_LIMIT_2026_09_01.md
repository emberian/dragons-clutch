# The legacy packet, protocol-wide — 2026-09-01

Tree `/Users/ember/dev/dclutch`, HEAD `10e44feae5751b1e37ed1605501d3f57a785ddd6`.
Reading lane; no source changed. Every path:line below is HEAD's version.

**Verdict.** The protocol needs two fix classes, not one, and they do not
overlap. Every route that is over 1,232 bytes *as a legacy message* — the six
C-09 routes behind GOAL.md's "thirteen", the four Custody operations, the two
relay records, the retirement checkpoint, the Dealer commit, Found31, the
founding chain, Series consume, Provider submit/execute, and General at
N=258 — is closed by **v0 over an Address Lookup Table**, which the tree's own
doctrine and code make a *client* change (`crates/dclutch-versioned-message-operator/src/lib.rs:3-5`:
tables are routing data, never protocol authority), already applied on most of
them. Exactly one family is over *with the table already applied*: Structured
full-width `IssueStructured`/`UnwrapStructured` (1,397 on the Claims route with
41 of 45 keys looked up; 1,589 on the Trading Hot route) and Hot-route
`Denominate` (1,253). For those the only remaining legal lever is
**commit-don't-inline as a Lean-emitted ABI revision** — drop the three
per-coordinate PDAs the program re-derives anyway (−288 bytes → 1,109; K rises
to 5 with the two mandatory-zero terminal fields) — which keeps the route's
authentication (the derivation *is* the authentication) and its atomicity (one
instruction). Splitting issuance is *not* legal as the code stands:
`request.rs:477` forces a complete set per instruction, which is the
"exhaustive before it mints" rule in AGENTS.md. The width-2/partition-gate
contradiction is **not resolved by any packet fix and no packet fix needs it
resolved**: Structured K=2 is dissolved twice (packet + priority fee = 1,241;
and a degree-≥2 spline cannot decode at width 2), and the gate is now a belief
family under which a width-2 *proposition* founds and a width-2 *spot band*
still refuses by arithmetic. What remains is a product ruling for ember —
whether a binary price market is a `StatedProposition` or a spot band with an
exemption — with the options costed in §7. One ALT builder was written and
abandoned (`compile_direct_hot_v0`, zero callers); it is not revivable as it
stands, but its action-neutral report is the seam the registered Direct
family's operator builder — which does not exist at HEAD — will need (§5).

## 1. What "measured" means in this document

Three instruments exist, and they are not interchangeable:

| instrument | what it proves | where |
| --- | --- | --- |
| campaign serializer, `wire_bytes` in the evidence fold | the exact extent of a transaction ProgramTest *executed*; ProgramTest submits no packet, so the number is measured, never enforced | `tools/gauntlet/program-test-evidence/src/lib.rs:98-105`; per-campaign witnesses in `tools/gauntlet/*/witnesses.json` |
| local validator (tier 1, journey, successor bootstrap) | the packet was accepted, so it is ≤ 1,232 by enforcement; the extent is recorded only where a journal keeps it | `tools/local-validator/bootstrap/successor/src/founding_submission_journal.rs:384-390`, `terminal_sequence.rs:2744`, `series_consume_campaign.rs:536` |
| derived arithmetic | a byte model, not a built packet | flagged "derived" below; never presented as measured |

Legacy vs v0 matters: the same instruction measures 2,634 legacy and 1,397 v0
over a live table (`tools/gauntlet/claims-rational-representation-v2/witnesses.json:23`).
Every row says which transport it measured. Every campaign figure carries
`set_compute_unit_limit` (+40 B since `7b80869d`); **none carries the
`set_compute_unit_price` the house builder pushes unconditionally**
(`crates/dclutch-representation-composition-v3-operator/src/lib.rs:861-893`,
+12 B), so a route built through that builder needs 12 more bytes than any
number below.

The only fold on disk at HEAD is the Dealer lane's
(`/private/tmp/dclutch-gauntlet/out/ledger.json`: 877 observations, all
`dealer-checkpoint-programtest`, 284 transactions, 18 unmeasured, all of them
the v0 commit rows). Every other number is read from the witness that pinned it
or the commit that recorded it, with the HEAD it was measured at where the
source states one. Re-deriving the protocol-wide set at *this* HEAD means
running every `tools/gauntlet/*/run-*.sh` and folding — hours of SBF builds in
a ten-lane dirty tree, and not done here.

## 2. Route table

Owner column: lane map `GOAL.md:2267-2272,2444,2457-2471,2537`. S3 Direct owns
`hot_v3.rs` (469 lines in flight, `WAVE.md:6775`) and the `direct_*` tests;
S7 Structured owns `bearer-v2-operator` and `rational_representation_v2_program_test.rs`;
S4 General; S5 Dealer; C-10 Witness owns the retirement chain and the
Claims/Custody/claim-check gauntlets; COHORT-10 owns `market.rs`/`plan.rs` and
the founding chain; REDEMPTION owns the wallet-terminal path. C-09 and NON-PRICE
are **closed** (`GOAL.md:2457,2531`); the resolution routes have no live lane,
and `programs/dclutch-resolution-proof-sbf/src/relay_transport_v1.rs` is dirty
in the working tree under a lane the map does not name.

"Over" is against 1,232 for the transport named. Fix classes: **ALT** = v0 over
a frozen lookup table; **CDI** = commit-don't-inline; **ABI** = drop re-derived
keys from the frame; **split** = two instructions; **none** = fits.

### 2a. Measured extents

| route | program | bytes (transport; measured at) | accounts | over | cheapest legal fix | owner |
| --- | --- | --- | --- | --- | --- | --- |
| `resolution/pre_market_funding_v1::process_pre_market_funding_v2` | resolution | **1,797** legacy; hostiles 1,765 (`acf890e5`, HEAD `4be7d485`) | 43 (`crates/dclutch-svm-harness/tests/pre_market_resolution_funding.rs:1056-1058`); request 272 B (`crates/dclutch-resolution-codec/src/pre_market_funding_v1.rs:15`) | **+565** | ALT — 43 keys inline is the whole overrun; the 272-B request is not | none (C-09 closed) |
| `resolution/pre_market_funding_abort_v1::…` | resolution | 1,002 legacy | 16 (`programs/dclutch-resolution-proof-sbf/src/pre_market_funding_abort_v1.rs:28`); request 368 B | fits by 230 | none — and must stay legacy: the stranger's unwind (`tools/gauntlet/resolution-pre-market-funding/README.md:55-58`) | — |
| `core/resolution::process#Retire` (terminal admit, no child) | core | **1,456** legacy | outer frame constant 22 (`programs/dclutch-core-sbf/src/resolution.rs:68`); fold records no count | **+224** | ALT | none |
| `core/resolution::process#CreateFund` → `resolution/process_create#CreateFund` | core→resolution | **1,275** legacy (core-v3 fold); journey: 2,016 legacy refused by RPC, rides v0 (`tools/gauntlet/journey/README.md:136-141`) | outer 18 (`resolution.rs:60`); the operator frame (`dclutch-resolution-core-v3-operator`) is wider and unrecorded | **+43** / +784 | ALT (journey already does) | none / journey |
| `resolution/core_effect::process_direct_funding_close_v1` (CloseFund) | resolution | **1,237** legacy | 21 (`programs/dclutch-resolution-proof-sbf/src/core_effect.rs:78`); request 472 B | **+5** | ALT | none |
| `resolution/core_effect::process_direct_funding_activation_v1` | resolution | 1,189; replay 1,172 legacy | 20 (`core_effect.rs:76`); request 440 B | fits by 43 | none; 43 B is one more key | none |
| `resolution/process_abandon#magic` | resolution | 1,052 legacy | 18 (`provider_transport_v3.rs:68`); 256 B | fits | none | none |
| `resolution/process_settle#Settle` | resolution | **1,321** legacy ×4 (`acf890e5`); 1,277 legacy on 2026-08-29 (`docs/evidence/PYTH_CREDENTIAL_FREE_DEVNET_2026_08_29.md:205`), 352 over the frozen ALT | 32 metas (same doc); request 32 B (`sponsored_push_v1.rs:36`) | **+89** | ALT (the 2026-08-29 caller already routes it; the two legacy figures differ by 44 B and are not reconciled in the tree) | none |
| `resolution/process_capture#Capture` | resolution | **1,255** legacy ×4; 348 over ALT (08-29) | 30 metas | **+23** | ALT | none |
| `resolution/process_commit_failure#CommitFailure` | resolution | 1,222 legacy ×4 | 29 metas | fits by 10 — **no room for the 12-B price instruction** | none today; the first added key or a priority fee puts it over. A *failure* route should not depend on a table; if it grows, CDI on its frame, not ALT | none |
| `resolution/process_close_candidate`, `process_close_head` | resolution | 333 legacy | 4 | fits | none | none |
| `resolution/process_append#AppendObservation` | resolution | **1,377** legacy (`tools/gauntlet/resolution-relayed/witnesses.json:23`) | carries the 424-B VirtualPool chunk inline | **+145** | ALT, or a smaller chunk: the append *is* the CDI chunk (`APPEND_OBSERVATION_PREFIX_BYTES = 40`, `crates/dclutch-relay-contract/src/instruction.rs:26`); both keep the sealed digest | none |
| `resolution/process_consume#ConsumeRecord` | resolution | **1,600** legacy (1,534 until 2026-09-03; `crates/dclutch-svm-harness/tests/relayed_mainnet_state.rs` `CONSUME_EXTENT`) | 30 (28 until the `StatisticSpecV1` pair joined the frame) | **+368** | ALT (key-heavy) | none |
| `resolution/process_commit_deadline_failure` (liveness walk) | resolution | 991 legacy | 22 | fits by 241 | none — **must never need a table** (`witnesses.json:30`; `packages/dclutch-sdk/lib/failureWalk.ts:77-81`) | none |
| `resolution/process_create_record`, `seal`, `retire` | resolution | measured, fit (witness asserts exactly the two above are over) | — | fits | none | none |
| `resolution/provider_transport_v3::…` (submit) | resolution | validator (journey), v0; legacy impossible: 38 keys + 416-B request ≥ 1,632 | 38 (`provider_transport_v3.rs:58`) | n/a legacy | ALT (in place) | journey |
| `core/execute_provider_v3::process#ExecuteProvider` | core | validator (journey), v0; 47 keys + 608-B request | 47 (`crates/dclutch-resolution-core-v3-operator/src/provider_finalized_projection_v3.rs:38`) | n/a legacy | ALT (in place) | journey |
| `custody/open_vault#OpenVault` | custody | **1,340** legacy; 1,043 is the v0 maximum of the family (`tools/gauntlet/claims-custody/README.md:84-92`) | — | **+108** | ALT (campaign already routes) | C-10 Witness |
| `custody/execute_transfer#Transfer`, `close_vault#CloseVault` | custody | **1,306** legacy each | — | **+74** | ALT | C-10 |
| `custody/delegated::process` (DCLCUDQ2) | custody | **1,410** legacy; 776-B request | — | **+178** | ALT; the 776-B body is the largest inline request in the protocol and is the one Custody frame where CDI (a committed delegation record) would beat ALT if the table is ever unwanted | C-10 |
| `custody/initialize_replay`, `close_replay` | custody | 1,208 / 1,174 legacy | — | fits by 24 / 58 | none | C-10 |
| `core/process_instruction#Retire` (prepare), `commit_checkpoint#CLOSE_VAULT`, `#CLOSE_REPLAY`, `finish_checkpoint_retirement` | core | **2,005–2,157** legacy (12 tx, `tools/gauntlet/retirement-checkpoint/witnesses.json:23`); v0 over a dedicated table 1,135 / 1,191 / 1,191 / 1,071 (`docs/evidence/AGGREGATE_RETIREMENT_CHECKPOINT_SPLIT_2026_08_28.md:65-80`) | 35 metas, 36 keys; data 808/864/864/744 | +773..+925 legacy; fits v0 | ALT (designed in). Note the v0 frames are *data*-bound: 864 of 1,191 | C-10 |
| `core/retirement_replay_handoff_v1`, `custody/retirement_replay_handoff_v1` | core, custody | 1,209 legacy ×8 (`tools/gauntlet/retirement-replay-handoff/witnesses.json:16`) | 23 | fits by 23 | none — and stays legacy on purpose | C-10 |
| `core/series_consume::process` | core | routed v0 **1,037** on the local validator (`tools/local-validator/bootstrap/evidence/series-consume-replay-2026-08-31.json:24`); legacy impossible — 61 unique keys = 1,952 B of addresses (`docs/evidence/GENERAL_AND_SERIES_EXECUTED_CAMPAIGNS_2026_08_29.md:154-155`) | 61 unique | n/a | ALT (in place) | none (C-07) |
| `core/found::process#Found` (Found31) | core | **1,242** legacy (`tools/gauntlet/tier1/witnesses.json:59`); executes v0 on tier 1 | 31 | **+10** | ALT (in place since `4e1c4db`) | COHORT-10 |
| `trading/projected_custody_bootstrap_v1::process_projected_custody_bootstrap_v2` (DCLTPCB2) | trading | 517 B signed v0 (`docs/evidence/FOUND_COMPACT_2026_08_28.md:62-66`) | 90 refs → 62 keys | fits; **2 keys from the 64-lock wall** | none for bytes; the wall is locks, which no packet fix moves | COHORT-10 |
| `registry/continuation_v1::process` | registry | 1,206 legacy (`docs/evidence/CONTINUATION_ROUTE_FIX_OR_RETIRE_2026_08_30.md:95`) | — | fits by 26 | none; harness-only (`GOAL.md:114`) | — |
| `direct-aot/process_instruction` | direct-aot | 169–755 legacy (`docs/evidence/DIRECT_FAMILY_CAMPAIGN_2026_08_27.md:139`) | 0; 584-B request | fits | none | S3 Direct |
| `general-accelerator/process_instruction`, N=1 | general-acc | 745–868 legacy (`docs/evidence/GENERAL_ACCELERATOR_CAMPAIGN_2026_08_27.md:380-388`, `bb4e83ca`) | 31–90 | fits | none | S4 General |
| same, N=258 | general-acc | Consider 1,273 · Freeze 1,207 · InitializeSettlement 1,330 · Collect 1,310 · Materialize 1,276 · Distribute 1,310 · Close 1,295 legacy (`:392-400`) | 45–104 | +41 / fits / +98 / +78 / +44 / +78 / +63 | ALT — the Trading-Hot form of the same seven compiles to 660–918 over one table per action (`docs/evidence/GENERAL_ALT_PACKET_WITNESS_2026_08_27.md:80-98`) | S4 |
| `trading/hot_v3::process_hot_execution_v3` via `registry/hot_continuation_v2` — **Structured IssueStructured** | trading | **1,589** v0 + ALT + heap grant + CU limit (`9adfaa9e`; `programs/dclutch-claims-sbf/tests/rational_representation_v2_program_test.rs:5370`) | 79 | **+357 with the table applied** | **ABI (−288 per the scholar's lever table) or CDI**; ALT exhausted | S7 Structured |
| same — **Structured Denominate** | trading | **1,253** v0 + ALT (`:5341`; the earlier "fits" was false by 21) | 71 | **+21 with the table applied** | ABI: the single asset row's three re-derived PDAs are 96 B | S7 |
| `claims/rational_representation_v2::process` — IssueStructured / UnwrapStructured, K=3 | claims | **1,397** v0 over a live table; 2,634 legacy (`tools/gauntlet/claims-rational-representation-v2/witnesses.json:23`, `7b80869d`) | 45 outer metas, 41 looked up; 768 B are inline pubkeys | **+165 with the table applied**; ALT-free levers sum to ≤ 96 | **ABI**: drop 3 re-derived PDAs/coordinate → 1,109 (K=3 fits, 123 spare); + two zero terminal fields → K=5 (`docs/evidence/ARCHITECT_SCHOLAR_2026_09_01.md:1244-1300`) | S7 |
| same — Denominate / Reconstitute / RedeemTerminal | claims | 1,061 v0 (selected) | one coordinate quadruple | fits | none | S7 |
| `trading/hot_v3…` — **Direct inline fill** | trading | **1,167** v0 on the local validator (`docs/evidence/FIRST_LOCAL_DIRECT_FILL_2026_08_31.md:209`) | 61 unique (4 static + 57 loaded) | fits by 65; **3 keys from the lock wall** | none | S3 |
| `trading/hot_v3…` — **Dealer Hot rows** (LP open/close, equity Add, selector 1) | trading + dealer-accelerator | **2,342 / 2,375 / 3,084** legacy (`programs/dclutch-dealer-accelerator-sbf/program-test/tests/accepted.rs:1244-1258`; fold labels `lp_lifecycle::*`), recorded with **no route claim** | not recorded | **+1,110 .. +1,852** | ALT *if* ≥ 61 lookup-eligible keys are in the frame (each saves 31 B; derived); the frame is not measured for that. Instrument: compile the same instruction through `compile_unsigned_packet_v0` with a live table in `accepted.rs`, the way `rational_representation_v2_program_test.rs:4035-4054` does | S5 Dealer |
| `trading/dealer_scenario_checkpoint_v1::…commit_v1` | trading | **1,366** legacy (one legacy submission; 18 v0 commits unmeasured) | not recorded | **+134** | ALT (in place for every other commit) | S5 |
| `…checkpoint_create/page/evaluate/reserve/rollback/cleanup_v1` | trading | 541 · 806–871 · 409 · 508–1,026 · 1,026 · 276 legacy (dealer fold) | not recorded | fits | none | S5 |
| Claims composed chain (`protocol_position_v2` + `sparse_native_transfer_v1` + Close) | claims | 1,261 legacy before deriving the Close request; 973 after (`tools/gauntlet/claims-custody/README.md:74-80`) | — | was +29 | CDI in the client (done) | C-10 |
| `claims/fractional_claim_check_v1::*`, `claim_check_*` | claims | widest 1,050 legacy (wallet-payout control), every claim-check open/crank/redeem/close smaller (`tools/gauntlet/claims-claim-check/witnesses.json:23`) | — | fits | none; stranger route, stays legacy | C-10 / REDEMPTION |
| `claims/fractional_atomic_v3` Wrap/WholeUnwrap · Terminal · Terminalize; `fractional_retirement_v3` Begin/Coordinate/Finish | claims | operator plan v0 at maximum width 682 · 708 · 656 · 508 · 536 · 512 (`crates/dclutch-fractional-claim-operator/tests/topology_v3.rs:72-80`; not an executed packet) | 29 / 42 / 16 / 6 / 20 / 8 loaded | fits | none | none (C-08) |

### 2b. Executed, extent asserted but not recorded

| route | program | transport / assertion | instrument that would record it | owner |
| --- | --- | --- | --- | --- |
| `trading/hot_v3…` — **Direct registered Sell / Buy** (Buy 1,144,079 CU, three Custody children, `WAVE.md:6783-6784`) | trading | v0 over the waist's table; `assert!(wire <= 1_232)` on every submission (`programs/dclutch-trading-sbf/program-test/direct-hot/src/waist.rs:1237-1250`) — the value prints only on overflow | print/record `wire` at `waist.rs:1237` through `program-test-evidence` | S3 |
| `trading/hot_v3…` — **General OpenBatch** N=2 (heap wall at 708,284 CU, `WAVE.md:6734`) | trading | v0 over the waist's table (`programs/dclutch-trading-sbf/program-test/general-hot/tests/open_batch.rs:697-698`); `accounts.len() <= 100` (`:662`); no wire measurement | same as above | S4 |
| `claims/affine_batch_v2`, `signed_delta_v3`, `rational_lifecycle_v2`, `protocol_position_v2`, `sparse_native_transfer_v1`, `custody_replay_v1`, `terminal_settlement_v3` | claims | v0 over a live table; witness asserts max ≤ 1,232 and zero unmeasured | the maxima are in each fold; no tracked doc states them | C-10 / S7 |
| `trading/user_position_admission_v1` (+ Admit/Close) | trading | v0 over a table, `wire_extent <= PACKET_DATA_BYTES` (`…/user-position-admission/tests/lifecycle.rs:580`); successor journals `wire_bytes` (`…/successor/src/user_position_admission.rs:241,1226`) | the journal, once a run is kept | — |
| `dealer/process_dealer_family_instruction` | dealer | legacy family campaign, ≤ 35 accounts, records `wire_bytes` (`programs/dclutch-dealer-sbf/program-test/tests/family.rs`); "submits no packet" (`tools/gauntlet/dealer/README.md:93-100`) | the fold | S5 |
| tier 1: `core/found::project`, `generic_founding_v1` (+ FoundAndPermit/Open), `infrastructure::process_initialize`, `claims/founding_v5`, `custody/projected::process` (+ 9 projected arms), `registry/*` (ActivateRole, Reauthenticate, record_v1 + arms), `rent/process_create_v2`, `trading/generic_market_founding_v3`, `projected_custody_abort_v1` | core, claims, custody, registry, rent, trading | validator-enforced ≤ 1,232; the accepted caller chain is 51/60/60 compiled keys (`docs/evidence/CU_ARCHITECTURE_CHANGE_MATRIX_2026_08_28.md:80`), i.e. legacy-impossible and v0 by construction | `founding_submission_journal.rs:384-390` already computes `expected_wire_bytes`; nothing publishes it | COHORT-10 |
| journey: `core/begin_retiring`, `resolution/core_effect::process_core_effect`, `provider_instruction_v3`, `process_submit#magic`, `rent/process_sweep_v2` | core, resolution, rent | validator-enforced; the four provider/fund frames ride v0 (`tools/gauntlet/journey/README.md:138-141`) | journey producer recording `wire_bytes` | journey |
| `core/series_permit_expiry::process`; Series permit-expiry Hot | core, trading | local validator; `docs/evidence/SERIES_PERMIT_EXPIRY_HOT_WALL_2026_08_31.json` records request bytes (160/32/640) and no wire | `series_permit_expiry_campaign.rs` journal | none (C-07) |
| `registry/lineage_v1::process` | registry | driven on a real chain via v0 (`70aaec46`) | the lineage caller's journal | COHORT-10 |

### 2c. Not executed at HEAD (no extent exists)

Unwitnessed or blocked; the instrument is the campaign that would drive them
plus `wire_bytes` at submission. Owner per `docs/evidence/UNWITNESSED_ROUTES_BY_ROW_2026_09_01.md`.

- **C-01**: `core/infrastructure_v2`, `rent/process_close_v2`, `registry/process_abort#4`, `trading/outer::process_capability_lifecycle#else`.
- **C-02**: `core/open_market`, capability activate/close arms, `trading/hot_v3::process_capability_seal_v1`, `…seal_close_v1`.
- **C-04** (S3): `trading/direct_begin_retiring_v1`, `direct_close_maker_v1`, `direct_fee_settlement_v1`, `direct_replay_setup_v1`, `direct_token_setup_v1` — program-tests exist and are dirty in the tree (`direct_begin_retiring_on_chain.rs`, `direct_close_maker_on_chain.rs`, `direct_hot_fee_pair.rs`); none records `wire_bytes`.
- **C-05/founding** (COHORT-10): `trading/generic_founding_stages_v1::*`, `projected_custody_bootstrap_v1::process_controller_funding_{prepare,cleanup_step1,cleanup_step2}_v1` (DCLTCFQ1, 51 keys — legacy-impossible by count).
- **C-07**: `claims/series_founding_transport_v1`, `core/series_open`, `core/series_permit_expiry_precommit_v1`.
- **C-08** (S7): `claims/rational_representation_v2::process_replay_close` — `asset_count == 1`, derived to fit, unmeasured.
- **C-09**: `core/resolution::authenticate_recovery_policy` (ember's open ruling), `core/resolution::process#AdmitTerminal/#VerifyFundReady/#CloseFund` (dead arms, `programs/dclutch-core-sbf/src/resolution.rs:263-272`), `resolution/process_reclaim#magic` (18 accounts, 288-B request — fits by the abandon frame's arithmetic, derived).
- **C-10** (Witness): `claims/market_closure_v1` (+ `process_checkpoint_handoff`), `claims/process_core_effect` (DCLTCEF1), `core/retire_v1::process#Retire`, `core/retire_v1::process_checkpoint_prepare#Retire`, `process_checkpoint_suffix`, `core/commit_checkpoint#*` as direct entries.
- Not in the release set: `product-runtime-v2/*`, `series-shadow/*`; `dealer-accelerator/*` is measured above through the Dealer Hot rows.

**Register staleness.** `docs/reference/routes.md` (regenerated `5c2ffc62`)
still shows the sponsored and pre-market routes as NEVER-EXECUTED; the C-09
bindings (`acf890e5`, 21:38) and the claim-check bindings (`4b259958`) landed
after it, and genref refuses the dirty tree. The convergence owner regenerates.

## 3. The "thirteen"

By **route** the C-09 finding is **six**: `pre_market_funding_v2`, terminal
admit, `CreateFund`, `direct_funding_close_v1`, `Settle`, `Capture`. By
**transaction** the tracked witnesses sum to **14** — sponsored 8
(`tools/gauntlet/resolution-sponsored/witnesses.json:28`, expect `8`), core-v3
3 (`resolution-core-v3/witnesses.json:21`, three labels), pre-market 3
(initializer + two hostiles, `resolution-pre-market-funding/README.md:48-52`).
`acf890e5` says thirteen and `GOAL.md:2532` says "thirteen routes"; neither
number reproduces from what is tracked, and the on-disk ledger holds only the
Dealer fold. Re-run the three runners and take
`[.transactions[] | select(.wire_bytes > 1232)] | length` over the folds to
settle it. The conclusion does not move: six routes, all key-heavy, all ALT.

## 4. One fix class or two

**Two, and they partition cleanly by one test: is the table already applied?**

- **Table not applied → v0+ALT closes it.** Every legacy-only overrun in §2a:
  the six C-09 routes (largest 1,797; 43 keys), Custody's four (largest 1,410),
  the two relay records, the retirement checkpoint (2,157 → 1,191), the Dealer
  commit (1,366), Found31, DCLTPCB2/GMF, Series consume, Provider submit and
  execute, General at N=258 (1,330 → 918), and very probably the Dealer Hot
  rows. No program, ABI, or Lean artifact changes. The tree has done it eleven
  times already; the C-09 routes are the last family that never measured.
- **Table applied and still over → only the frame can move.** Structured
  full-width Issue/Unwrap (1,397 Claims-direct; 1,589 Hot) and Hot Denominate
  (1,253). The scholar's lever table (`ARCHITECT_SCHOLAR_2026_09_01.md:1266-1279`)
  is exhaustive: ALT exhausted (41/45; the other 4 are signers or invoked
  programs, structurally ineligible), a second table is −34 net, merging the
  signers leaves 69 over. The only lever that closes 165 is the request ABI:
  −288 by dropping the three per-coordinate PDAs that
  `programs/dclutch-claims-sbf/src/rational_representation_v2.rs:1155-1204`
  derives from `(program_id, descriptor, outcome)` and then compares against
  the inlined copies. Same shape as the K lift `open_structured_v3.rs:116`
  names ("commit-don't-inline or staged issuance, not a wider record",
  `crates/dclutch-bearer-v2-operator/src/open_structured_v3.rs:116-118`).
- **A third wall that is not bytes.** The 64-lock limit: Dealer's unsplit
  selector-9 scenario (122 metas), Direct inline at 61, DCLTPCB2 at 62. ALT
  moves none of it (`tools/gauntlet/dealer/README.md:96-100`); only the
  checkpoint split does, and it has landed. A packet document that reports
  "fits" on a 63-key frame is reporting the wrong wall.

**Legality of each class**, against what the code and decisions state:

| class | authentication | atomicity | verdict |
| --- | --- | --- | --- |
| ALT | untouched: the instruction bytes and the account set are identical; the table is "transaction-routing data, never protocol authority" (`versioned-message-operator/src/lib.rs:3-5`), validated as finalized bytes before use | untouched: one instruction | **legal everywhere**, with three obligations: a frozen table per market is an operator lifecycle act the client must precommit (`terminal_sequence.rs:5919-5937` plans it; the Pyth caller refuses a mutable or substituted table, `PYTH_CREDENTIAL_FREE_DEVNET_2026_08_29.md:205-207`); routes that must run when nobody cooperated stay table-free (liveness walk 991, abort 1,002, claim-check ≤ 1,050, handoff 1,209 — all asserted by witnesses); and the twins must build the same message (§5) |
| ABI (drop re-derived keys) | preserved: the program already derives and requires equality; removing the wire copy removes a redundant check. The request is the digest preimage for the Claims caller-authority PDA (`programs/dclutch-trading-sbf/src/claims_composition_v3.rs:551-558`), so the preimage changes — a Lean-emitted schema revision (`generated.rs` is emitted by `EmitRationalRepresentationV2PhysicalAbiRust.lean`), not a semantic change | preserved | **legal**; cost is a regeneration and every twin that states the width |
| CDI (digest in the instruction, body in an account) | preserved iff the account's bytes are authenticated by the digest at execution — the pattern Series (root + height-32 proofs), Dealer pages, and the relay's chunked records already use | preserved for the executing instruction; the *staging* transactions move no liability | **legal** where a staging record exists; it is a new record type per route |
| split | — | **reopens it**: Issue/Unwrap require `asset_count == outcome_count` (`crates/dclutch-rational-representation-v2-request-contract/src/request.rs:477`), which is AGENTS.md's "exhaustive, disjoint … before it can mint liabilities" | **not legal** without a staged escrow that holds the full set before the mint — General's `Prepared → CustodyStaged → Open-or-abort` shape, weeks-class (`GOAL.md:802`) |

## 5. v0+ALT: protocol change or client change

**Client**, by the tree's own doctrine and by inspection — with the caveat
that "client" here is four things, not one:

1. **Rust operators** already compile v0: `dclutch-operator` (10 sites),
   `successor` bootstrap (19), `relayer` (6), the composition operators
   (`compile_unsigned_packet_v0`, `representation-composition-v3-operator/src/lib.rs:861-893`).
2. **The TypeScript twin** already compiles v0 for Direct, Found and
   registered creation (`apps/dclutch-web/lib/directTransaction.ts`,
   `coreFound.ts`, `registeredDirect.ts`; CLI `packages/dclutch-cli/src/internal/rpcSubmission.ts`).
   Two SDK builders are **deliberately legacy**: `failureWalk.ts:77-81` (must
   never need a table) and `claimsCustodyReplay.ts:466` (refuses its own
   packet above the bound). Any route moved to ALT gains a TS builder that
   observes the table; that is per-route work in `apps/dclutch-web/lib` and
   `packages/dclutch-sdk/lib`, verified by the `abi:*:verify` scripts
   (`apps/dclutch-web/package.json:14-53`).
3. **The WASM twins** (`crates/dclutch-*-wasm`, eight crates) emit instruction
   bytes and plans, not messages — no `v0`/lookup reference in their sources —
   so ALT routing does not enter them. The ABI revision in §4 does: every
   generated width they state.
4. **The market lifecycle**: a frozen table per market is precommitted and
   observed finalized before use. This is where "client" touches protocol
   posture: a route whose *only* submittable form needs a table has made table
   publication a liveness precondition. The tree already draws that line
   (walk/abort/claim-check/handoff stay legacy). The six C-09 routes do not
   cross it — Capture/Settle are the sponsor's, not the stranger's — but
   `CommitFailure` at 1,222 is 10 bytes from crossing it, and that one is a
   failure path.

No program, no ABI, no Lean, no refusal code changes for ALT. It is not a
protocol change here.

**The abandoned Direct ALT builder.** `crates/dclutch-operator/src/direct_inline_v3.rs:1357`
`compile_direct_hot_v0` is a v0-over-lookup-table builder, and it has zero
callers at HEAD. What it is: the compiler half of a two-function island from
`a865216c` (2026-08-26, "operator: select generic Direct Hot actions") —
`build_direct_hot_request_v4` (`:671`, also zero external references) emits an
*action-neutral* `DirectHotReportV4` (`:375`; `report.action` is any
`DirectExecutionActionV3`), and `compile_direct_hot_v0` compiles it through
"the sole canonical LUT", refusing unless the table's address list equals
`canonical_direct_hot_lookup_addresses_v4` (one sorted union table for the
whole frame). Why it was never wired: two days later `d4711c11` ("direct:
freeze authenticated ordinary route") landed
`compile_direct_inline_routed_v0_v3` (`direct_inline_route_v3.rs:3152`) —
a *request-specific* frozen table, route classes assigned before alias
packing, the route itself authenticated
(`assemble_authenticated_direct_inline_ordinary_route_v3`, `:1196`), and a
wire pin (`:6232-6234`: 1,167 bytes, 57 loaded, 4 static — the number the
local validator reproduced). That is what the successor
(`tools/local-validator/bootstrap/successor/src/direct_trade.rs:49-53,1833`)
and the browser twin (`apps/dclutch-web/lib/directInlineV3.ts:866`) build
with. The island's *validator* survived — the live route calls
`validate_direct_hot_instruction_sequence_v4` (`direct_inline_route_v3.rs:3349`)
— and its *compiler* did not. Is it the right shape to revive? **Not for the
inline route**: it authenticates nothing about the route, pins no extent, and
one canonical table is the shape the successor rejected. **But it is the only
operator-side v0 builder shaped for any Direct action other than
`InlineOrdinary`**: the live V3 builder hardcodes `InlineOrdinary`
(`:3349-3354`), so the registered family — Sell 369,305 CU, Buy 1,144,079 CU —
has no operator v0 path at HEAD; the program-test waist compiles the message
itself (`waist.rs:1237`). So the registered route's builder, when Direct (S3)
writes it, takes the action-neutral *report* seam from V4 and the
authenticated, request-specific, extent-pinned *compile* from V3. Until then
the island is a superseded authority path beside its successor, which AGENTS.md
says to delete in the same convergence cycle; Direct owns the file.

## 6. What commit-don't-inline costs, from the routes that already do it

| worked example | bytes | accounts | CU | source |
| --- | --- | --- | --- | --- |
| Structured selected vs full width, K=3 (the descriptor is committed on chain; a selected action names one coordinate) | 1,061 vs 1,397 v0: **−336 = 2 × 168/coordinate** (`ASSET_BYTES_V2 + 2 × RATIONAL_ASSET_ACCOUNT_COUNT_V2`) | one quadruple vs K | Hot route: Denominate 749,161 vs Issue 830,476 (different actions; not a controlled pair) | `claims-rational-representation-v2/witnesses.json:23,30`; `9adfaa9e` |
| Structured ABI lift (the lever, not yet built) | −288 → 1,109; K=5 at −352 | 0 (the accounts stay; only inline copies go) | ≤ 0: the derivation already runs, three equality checks go | `ARCHITECT_SCHOLAR_2026_09_01.md:1266-1279` |
| Dealer checkpoint: six page receipts, then commit | pages 806–871 legacy each; commit 1,366 legacy / v0 unmeasured | not recorded | pages 23,961–37,974 each (×6 ≈ 144k–228k); commit 461,933–476,876 (dealer fold, `ledger.json`) | vs the unsplit 122-meta scenario that no cluster can lock |
| General register bank in scratch pages (N=1 → N=258) | v0 +56 B (608 → 664 Consider); legacy +462 | +2 per page (+28 at N=258) | Consider 36,113 → 74,877; InitializeSettlement 61,753 → 164,970 | `GENERAL_ALT_PACKET_WITNESS:80-98`, `GENERAL_ACCELERATOR_CAMPAIGN:380-400` |
| Relay chunked record (append chunks, seal, consume) | append 1,377 with a 424-B chunk; consume 1,600 (1,534 until 2026-09-03) | consume 30 | — | `resolution-relayed/witnesses.json:23` |
| Claims composed chain (derive the third request from the two it binds) | 1,261 → 973 (−288) | 0 | unmeasured | `claims-custody/README.md:74-80` |
| Aggregate retirement (one 2,152-B payload → four transactions) | v0 1,071–1,191 each, data 744–864 | 35 metas each | — | `AGGREGATE_RETIREMENT_CHECKPOINT_SPLIT:65-80`, `CU_ARCHITECTURE_CHANGE_MATRIX:82` |

The pattern: CDI costs **+2 accounts and roughly +25–40k CU per committed page**
where the body is paged (General, Dealer), and **0 accounts, ≤ 0 CU** where it
is a derivation the program already performs (Structured ABI, the Claims
chain). The Structured lift is the cheap kind.

## 7. Width-2 markets versus the partition gate

**Not resolved by any packet fix, and no packet fix needs it resolved.** Three
facts at HEAD, each independent:

1. **K=2 is not a harbour, twice.** Packet: `1,061 + 168 = 1,229`, +12 for the
   price instruction the house builder always pushes = **1,241** (derived;
   `ARCHITECT_SCHOLAR:1285-1294`). Curve family: `spline_eval_v3.rs:226,265-267`
   iterates `degree..width`, empty at width 2, `SplineDegenerateSpan`, reached
   unconditionally from `ProductBasisV3::validate` — a K=2 spline record cannot
   decode. So nothing on the Structured side can want width 2, and the −288
   ABI lift removes the packet reason to look for it (K=3 fits, K=5 with the
   terminal fields).
2. **The gate is a family now.** `FoundingBeliefV1::{SpotBand, StatedProposition}`
   (`crates/dclutch-product-compiler/src/partition_quality.rs:195-206`); the
   founding path matches on the kind
   (`tools/local-validator/bootstrap/successor/src/market.rs:3119-3160`). A
   width-2 market founded on a **stated prior** passes when the prior is under
   the ceiling — the non-price lane's closure (`GOAL.md:2457`,
   `NON_PRICE_RESOLUTION_DESIGN_2026_09_01.md:353-355`). A width-2 **spot
   band** still refuses by arithmetic: one ordinary cell is 10,000 bps against
   a ceiling ≤ `MAX_CELL_EX_ANTE_SHARE_BPS_V1`, and `centred_cuts_v1` refuses
   `ordinary_cells < 2` outright (`partition_quality.rs:521-523`).
3. **So the residue is a product question, not a design contradiction:** is
   "SOL above 150 at expiry" a spot band (refused at width 2) or a proposition
   (admitted with a stated prior)? That is ember's ruling. Options:

| option | what changes | cost | what it gives up |
| --- | --- | --- | --- |
| **A. status quo** — width-2 spot bands refuse; propositions found at width 2 | one sentence at `market.rs:12283` ("protocol floor") saying the floor is for propositions | a doc line | a binary *price* market must be authored as a proposition |
| **B. route binary price questions through `StatedProposition`** | wizard/simulator ask for P(above X) instead of a band; the observation stays price-shaped | zero protocol code; UX + simulator inputs | the belief is a prior, not a walk — the gate measures what the author states, not where spot's mass lands |
| **C. width term / exemption in the SpotBand model** (measure a one-cell partition by tail mass instead of cell share) | `assess_partition_quality_v1` + the WASM twin + tests | model change to the gate's meaning; a new degenerate case to name | weakens the gate at exactly the width it was built to catch |

A and B together cost nothing and keep the gate's meaning; C is the one that
needs an argument. None of the three is on the packet's critical path.

## 8. The bound has no owner

`1232`/`1_232` is restated 115 times across `.rs/.ts/.tsx`
(`ARCHITECT_SCHOLAR:1307-1316`), 34 of them as named constants at HEAD, none
labeled chain-derived as AGENTS.md requires.
`dclutch_versioned_message_operator::PACKET_DATA_BYTES`
(`crates/dclutch-versioned-message-operator/src/lib.rs:24`) is the natural
Rust owner and `apps/dclutch-web/lib/solanaLimits.ts:2` the browser one. Not a
packet fix; a precondition for any witness that pins a margin (CommitFailure's
10, CloseFund's 5) meaning the same thing in every crate.

## 9. What was not measured here, and the instrument for each

- Dealer Hot rows' account counts (→ whether ALT alone fits 3,084):
  `compile_unsigned_packet_v0` over a live table in `accepted.rs`.
- Direct registered Buy/Sell and General OpenBatch extents: record `wire` at
  `waist.rs:1237` through `program-test-evidence`.
- Tier-1 and journey extents: `founding_submission_journal.rs:384` already
  computes them; publish the journal.
- The seven claims campaigns' maxima: in their folds; not tracked.
- The 1,277 vs 1,321 Settle discrepancy: rerun the 08-29 caller test beside
  the C-09 sponsored runner at one HEAD.
- The thirteen/fourteen count: §3.

## 10. Addendum — 2026-09-04, lane SEVEN

The body above is a dated reading. Nothing in it is edited; this section says
what three days of campaigns changed about four of its rows, and records one
measurement that has nowhere else to live.

### 10a. The expired-source abort misses the legacy packet by five

`DCLTPCA1` — `trading/projected_custody_bootstrap_v1::process_projected_custody_abort_v1`,
the only route out of a funded projection whose founding expired — serialises
to **1,237 bytes with its keys inline, against `PACKET_DATA_SIZE` = 1,232. It
misses by five.** Measured 2026-09-03 by lane ABORT-WITNESS on the campaign
serializer (`tools/gauntlet/source-abort/witnesses.json`, witness
`the-abort-frame-overruns-the-legacy-packet-maximum`); the two cleanup
transactions in the same suffix fit at 706.

This is a §2a row that did not exist when §2a was written, and it belongs in
this document rather than only in a campaign's witness file because it is a
*route property*, not a campaign property: the abort cannot be submitted as a
legacy transaction by anyone, ever, and five bytes is close enough that a
reader who has not seen the number will assume it fits.

**The measurement is not an enforcement.** ProgramTest submits no packet and
cannot refuse for size, which is the fast-lane hole §1 already names and the
one Found31 fell through. So the campaign measures against the limit; nothing
has yet asked a runtime to enforce it on this frame.

**THE QUESTION, and it is not this lane's to answer.** The successor bootstrap
already compiles the same 36-account frame as a v0 message over a finalized
address lookup table (`tools/local-validator/bootstrap/successor/src/market.rs`,
`build_projected_custody_abort_v1`), which is the §5 answer applied — a client
change, tables being routing data and never protocol authority. What nobody has
decided is whether the abort suffix gets a **lane that submits it to a
validator**, where the table is real and the packet is enforced. Until it does,
the route's only witness is a bank that warps past the expiry slot in one call
where a validator waits out hundreds of real ones, and the ALT is asserted
rather than exercised on this route. Owner: whoever owns tier 1's shape.

### 10b. §2b's tier-1 row overstated two Registry routes, and no longer does

§2b lists, among the routes tier 1 drives, `registry/*` — naming
`Reauthenticate` and `record_v1 + arms` explicitly. Both claims were false when
written, in different ways:

- **`registry/process_reauthenticate#Reauthenticate` had never been submitted
  anywhere**, by any campaign, on any substrate. The shipped builder
  `build_registry_reauthentication_v1` produced the exact three-account
  read-only frame the whole time and nothing called it.
- **`registry/process_abort#4` had no host builder at all.** `AbortRecordV1` is
  encoded by `dclutch-record-contract` and by nothing else, so the arm was
  unreachable from outside the program by construction — not merely unscheduled.

Both execute on tier 1 as of `c42da8fef` (2026-09-04): five reauthentications,
one per `ExecutionRoleV1`, read back at a slot no earlier than their
activation's; and a Begin / hostile / reclaim triple over a record published in
order to be abandoned. **Their extent is still not recorded** — the instrument
column's point stands unchanged, `founding_submission_journal.rs:384-390`
computes `expected_wire_bytes` and nothing publishes it.

### 10c. §2c: four of its routes now have a stated reason instead of silence

`tools/gauntlet/blocked.json` gained rows for four routes §2c lists as
unwitnessed, so they stop rendering as "no campaign and no reason recorded":

- **C-04's `direct_replay_setup_v1` / `direct_token_setup_v1`** are blocked on
  their producer, not on a packet. `local-private-validator-direct-trade-v1` is
  their only driver in the tree and its finalized evidence records no `logs` and
  no `error` — `census observe` cannot corroborate a claim without the chain's
  own invoke lines, and the binary hard-refuses any transaction whose RPC meta
  `err` is non-null, so it could only ever produce `executed` rows. §2c's note
  that the program-tests "exist and are dirty in the tree" is worth narrowing:
  no program-test drives either setup route. The blocked row states the cost.
- **C-05's `generic_founding_stages_v1::*`** are blocked on a campaign, not on
  an executor: `execute_split_market_founding` is complete with four hostiles,
  and as of 2026-09-04 it is selected by `MarketRunInput::founding_route` rather
  than by an environment variable nothing set. The blocked row explains why a
  split run cannot ride tier 1 — it would make tier 1's bindings, its CU budgets
  and its witnesses wrong at once — and therefore needs a tier of its own.
