# Unwitnessed routes, by contract row — 2026-09-01

Status: work queue. Owner: whichever lane owns the row.
Generated from the route register at `3466740e`; regenerate by re-running
`dclutch-route-census inventory` and re-deriving status the way
`tools/genref/generate.mjs` does (bindings, then `blocked.json`).

**55 of 161 routes are `NEVER-EXECUTED, no stated reason`.** That number is the
largest single thing C-00 and C-16 close against, and it is protocol work rather
than bookkeeping: the instrument gap behind it was closed separately (10 rows
repaired at `96ddf38f`, 2 more witnessed at `939d0806`). Every route below has
**no campaign in the tree at all**.

A route sits in the row whose *capability* it serves, not the program that hosts
it, because the lane that would drive it is the capability's lane. The mapping is
hand-authored and reviewable — if your row lists a route you do not own, say so
and it moves.

Two things this list does NOT say. It does not say these routes are broken: an
unwitnessed route is one no campaign drives, which is a statement about coverage
and not about correctness. And it does not say they are all equally hard —
several are one campaign apart, and `blocked.json` is the right home for any that
turn out to be structurally undrivable, **with a reason and an owner**, rather
than being left in this list to look like unstarted work.

| row | capability | unwitnessed |
| --- | --- | --- |
| **C-01** | Infrastructure, Registry, release lineage, migration and Rent | **4** |
| **C-02** | Compiler-shaped product entrance | **5** |
| **C-04** | Direct | **4** |
| **C-06** | Dealer | **8** |
| **C-07** | Series | **2** |
| **C-08** | Structured/Fractional representation | **5** |
| **C-09** | Objective resolution | **14** |
| **C-10** | Claims, Custody and terminal lifecycle | **13** |
| | **total** | **55** |

---

## C-01 — Infrastructure, Registry, release lineage, migration and Rent  (4)

| route | program | declared at |
| --- | --- | --- |
| `core/infrastructure_v2::process_initialize_v2` | core | `programs/dclutch-core-sbf/src/lib.rs:411` |
| `registry/lineage_v1::process` | registry | `programs/dclutch-registry-sbf/src/lib.rs:334` |
| `registry/process_abort#4` | registry | `programs/dclutch-registry-sbf/src/record_v1.rs:70` |
| `trading/outer::process_capability_lifecycle#else` | trading | `programs/dclutch-trading-sbf/src/lib.rs:772` |

## C-02 — Compiler-shaped product entrance  (5)

| route | program | declared at |
| --- | --- | --- |
| `trading/generic_founding_stages_v1::process_generic_found_and_permit_v1` | trading | `programs/dclutch-trading-sbf/src/lib.rs:670` |
| `trading/generic_founding_stages_v1::process_generic_market_open_v1` | trading | `programs/dclutch-trading-sbf/src/lib.rs:682` |
| `trading/projected_custody_bootstrap_v1::process_controller_funding_cleanup_step1_v1` | trading | `programs/dclutch-trading-sbf/src/lib.rs:735` |
| `trading/projected_custody_bootstrap_v1::process_controller_funding_cleanup_step2_v1` | trading | `programs/dclutch-trading-sbf/src/lib.rs:747` |
| `trading/projected_custody_bootstrap_v1::process_controller_funding_prepare_v1` | trading | `programs/dclutch-trading-sbf/src/lib.rs:706` |

## C-04 — Direct  (4)

| route | program | declared at |
| --- | --- | --- |
| `trading/direct_begin_retiring_v1::process_direct_begin_retiring_v1` | trading | `programs/dclutch-trading-sbf/src/lib.rs:612` |
| `trading/direct_fee_settlement_v1::process_direct_fee_settlement_v1` | trading | `programs/dclutch-trading-sbf/src/lib.rs:646` |
| `trading/direct_replay_setup_v1::process_direct_replay_setup_v1` | trading | `programs/dclutch-trading-sbf/src/lib.rs:628` |
| `trading/direct_token_setup_v1::process_direct_token_setup_v1` | trading | `programs/dclutch-trading-sbf/src/lib.rs:636` |

## C-06 — Dealer  (8)

| route | program | declared at |
| --- | --- | --- |
| `custody/dealer_reservation_v1::process` | custody | `programs/dclutch-custody-sbf/src/lib.rs:255` |
| `trading/dealer_scenario_checkpoint_v1::process_dealer_scenario_checkpoint_cleanup_v1` | trading | `programs/dclutch-trading-sbf/src/lib.rs:597` |
| `trading/dealer_scenario_checkpoint_v1::process_dealer_scenario_checkpoint_commit_v1` | trading | `programs/dclutch-trading-sbf/src/lib.rs:589` |
| `trading/dealer_scenario_checkpoint_v1::process_dealer_scenario_checkpoint_create_v1` | trading | `programs/dclutch-trading-sbf/src/lib.rs:549` |
| `trading/dealer_scenario_checkpoint_v1::process_dealer_scenario_checkpoint_evaluate_v1` | trading | `programs/dclutch-trading-sbf/src/lib.rs:565` |
| `trading/dealer_scenario_checkpoint_v1::process_dealer_scenario_checkpoint_page_v1` | trading | `programs/dclutch-trading-sbf/src/lib.rs:557` |
| `trading/dealer_scenario_checkpoint_v1::process_dealer_scenario_checkpoint_reserve_v1` | trading | `programs/dclutch-trading-sbf/src/lib.rs:573` |
| `trading/dealer_scenario_checkpoint_v1::process_dealer_scenario_checkpoint_rollback_v1` | trading | `programs/dclutch-trading-sbf/src/lib.rs:581` |

## C-07 — Series  (2)

| route | program | declared at |
| --- | --- | --- |
| `claims/series_founding_transport_v1::process` | claims | `programs/dclutch-claims-sbf/src/lib.rs:471` |
| `core/series_permit_expiry_precommit_v1::process` | core | `programs/dclutch-core-sbf/src/lib.rs:436` |

## C-08 — Structured/Fractional representation  (5)

| route | program | declared at |
| --- | --- | --- |
| `claims/fractional_retirement_v3::process` | claims | `programs/dclutch-claims-sbf/src/lib.rs:396` |
| `claims/process_begin#Begin` | claims | `programs/dclutch-claims-sbf/src/fractional_retirement_v3.rs:149` |
| `claims/process_coordinate#RetireCoordinate` | claims | `programs/dclutch-claims-sbf/src/fractional_retirement_v3.rs:152` |
| `claims/process_finish#Finish` | claims | `programs/dclutch-claims-sbf/src/fractional_retirement_v3.rs:155` |
| `claims/rational_representation_v2::process_replay_close` | claims | `programs/dclutch-claims-sbf/src/lib.rs:504` |

## C-09 — Objective resolution  (14)

| route | program | declared at |
| --- | --- | --- |
| `core/authenticate_no_recovery_entries#None` | core | `programs/dclutch-core-sbf/src/resolution.rs:801` |
| `core/resolution::authenticate_recovery_policy#(recovery_id,policy)` | core | `programs/dclutch-core-sbf/src/resolution.rs:780` |
| `core/resolution::process#AdmitTerminal` | core | `programs/dclutch-core-sbf/src/resolution.rs:266` |
| `resolution/core_effect::process_direct_funding_activation_v1` | resolution | `programs/dclutch-resolution-proof-sbf/src/lib.rs:306` |
| `resolution/core_effect::process_direct_funding_close_v1` | resolution | `programs/dclutch-resolution-proof-sbf/src/lib.rs:313` |
| `resolution/pre_market_funding_abort_v1::process_pre_market_funding_abort_v1` | resolution | `programs/dclutch-resolution-proof-sbf/src/lib.rs:292` |
| `resolution/pre_market_funding_v1::process_pre_market_funding_v2` | resolution | `programs/dclutch-resolution-proof-sbf/src/lib.rs:299` |
| `resolution/process_abandon#magic` | resolution | `programs/dclutch-resolution-proof-sbf/src/provider_transport_v3.rs:95` |
| `resolution/process_capture#Capture` | resolution | `programs/dclutch-resolution-proof-sbf/src/sponsored_push_v1.rs:78` |
| `resolution/process_close_candidate#CloseCandidate` | resolution | `programs/dclutch-resolution-proof-sbf/src/sponsored_push_v1.rs:81` |
| `resolution/process_close_head#CloseHead` | resolution | `programs/dclutch-resolution-proof-sbf/src/sponsored_push_v1.rs:83` |
| `resolution/process_commit_failure#CommitFailure` | resolution | `programs/dclutch-resolution-proof-sbf/src/sponsored_push_v1.rs:85` |
| `resolution/process_settle#Settle` | resolution | `programs/dclutch-resolution-proof-sbf/src/sponsored_push_v1.rs:79` |
| `resolution/sponsored_push_v1::process_sponsored_push_v1` | resolution | `programs/dclutch-resolution-proof-sbf/src/lib.rs:344` |

## C-10 — Claims, Custody and terminal lifecycle  (13)

| route | program | declared at |
| --- | --- | --- |
| `claims/claim_check_compaction_v1::process_compaction` | claims | `programs/dclutch-claims-sbf/src/lib.rs:534` |
| `claims/claim_check_redemption_v1::process_redemption#else` | claims | `programs/dclutch-claims-sbf/src/lib.rs:589` |
| `claims/market_closure_v1::process_checkpoint_handoff` | claims | `programs/dclutch-claims-sbf/src/lib.rs:384` |
| `core/commit_checkpoint#AGGREGATE_RETIREMENT_CLOSE_REPLAY_MAGIC_V1` | core | `programs/dclutch-core-sbf/src/retire_v1.rs:796` |
| `core/commit_checkpoint#AGGREGATE_RETIREMENT_CLOSE_VAULT_MAGIC_V1` | core | `programs/dclutch-core-sbf/src/retire_v1.rs:776` |
| `core/finish_checkpoint_retirement#AGGREGATE_RETIREMENT_FINISH_MAGIC_V1` | core | `programs/dclutch-core-sbf/src/retire_v1.rs:809` |
| `core/process_instruction#Retire` | core | `programs/dclutch-core-sbf/src/lib.rs:622` |
| `core/retire_v1::process_checkpoint_prepare#Retire` | core | `programs/dclutch-core-sbf/src/lib.rs:629` |
| `core/retire_v1::process_checkpoint_suffix` | core | `programs/dclutch-core-sbf/src/lib.rs:384` |
| `core/retirement_replay_handoff_v1::process` | core | `programs/dclutch-core-sbf/src/lib.rs:395` |
| `custody/reserve#Reserve` | custody | `programs/dclutch-custody-sbf/src/dealer_reservation_v1.rs:173` |
| `custody/retirement_replay_handoff_v1::process` | custody | `programs/dclutch-custody-sbf/src/lib.rs:267` |
| `custody/rollback#Rollback` | custody | `programs/dclutch-custody-sbf/src/dealer_reservation_v1.rs:176` |

