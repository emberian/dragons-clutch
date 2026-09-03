// @generated from docs/reference/routes.md; do not edit.
// Regenerate with: npm run abi:phase-admission

/** Every phase a Core Market can be in. */
export type MarketPhaseV1 = "Founding" | "Open" | "Terminal" | "Retiring" | "Retired";

/** Every Resolution Fund readiness a Core Market can be in. */
export type MarketReadinessV1 = "Prepaid" | "Ready" | "Consumed";

/**
 * One route's admissible Market prestates, as its own guard declares them.
 *
 * `prestates` is exact and `phases` is its projection. A guard that names no
 * readiness leaves `prestates` empty and admits every readiness in `phases`.
 */
export interface RoutePhaseGateV1 {
  readonly route: string;
  readonly phases: ReadonlyArray<MarketPhaseV1>;
  readonly prestates: ReadonlyArray<readonly [MarketPhaseV1, MarketReadinessV1]>;
}

/** Routes enumerated by the census, gated or not. */
export const ROUTE_COUNT_V1 = 162 as const;

export const ROUTE_PHASE_GATES_V1: ReadonlyArray<RoutePhaseGateV1> = [
  { route: "claims/affine_batch_v2::process", phases: ["Open"], prestates: [] },
  { route: "claims/claim_check_compaction_v1::process_compaction", phases: ["Terminal", "Retiring"], prestates: [] },
  { route: "claims/claim_check_compaction_v1::process_open_escrow", phases: ["Terminal", "Retiring"], prestates: [] },
  { route: "claims/founding_v5::process", phases: ["Founding"], prestates: [] },
  { route: "claims/fractional_claim_check_v1::process_fractional_compaction", phases: ["Terminal", "Retiring"], prestates: [] },
  { route: "claims/market_closure_v1::process", phases: ["Retiring"], prestates: [] },
  { route: "claims/market_closure_v1::process_checkpoint_handoff", phases: ["Retiring"], prestates: [] },
  { route: "claims/process_begin#Begin", phases: ["Terminal", "Retiring"], prestates: [] },
  { route: "claims/process_open#WholeUnwrap", phases: ["Open"], prestates: [] },
  { route: "claims/rational_lifecycle_v2::process", phases: ["Open", "Retiring"], prestates: [] },
  { route: "claims/rational_representation_v2::process", phases: ["Open", "Terminal", "Retiring"], prestates: [] },
  { route: "claims/series_founding_transport_v1::process", phases: ["Founding"], prestates: [] },
  { route: "claims/signed_delta_v3::process", phases: ["Open"], prestates: [] },
  { route: "claims/sparse_native_transfer_v1::process", phases: ["Open"], prestates: [] },
  { route: "claims/terminal_settlement_v3::process", phases: ["Terminal", "Retiring"], prestates: [] },
  { route: "core/commit_checkpoint#AGGREGATE_RETIREMENT_CLOSE_REPLAY_MAGIC_V1", phases: ["Retiring"], prestates: [] },
  { route: "core/commit_checkpoint#AGGREGATE_RETIREMENT_CLOSE_VAULT_MAGIC_V1", phases: ["Retiring"], prestates: [] },
  { route: "core/execute_provider_v3::process#ExecuteProvider", phases: ["Open"], prestates: [["Open", "Consumed"]] },
  { route: "core/finish_checkpoint_retirement#AGGREGATE_RETIREMENT_FINISH_MAGIC_V1", phases: ["Retiring"], prestates: [] },
  { route: "core/open_market::process#OpenMarket", phases: ["Founding"], prestates: [["Founding", "Ready"]] },
  { route: "core/process_open#Open", phases: ["Founding"], prestates: [["Founding", "Prepaid"]] },
  { route: "core/resolution::process#AdmitTerminal", phases: ["Open", "Terminal"], prestates: [["Open", "Consumed"], ["Terminal", "Consumed"]] },
  { route: "core/resolution::process#CreateFund", phases: ["Founding", "Open"], prestates: [["Founding", "Prepaid"], ["Open", "Consumed"]] },
  { route: "core/resolution::process#VerifyFundReady", phases: ["Founding", "Open"], prestates: [["Founding", "Prepaid"], ["Founding", "Ready"], ["Open", "Consumed"]] },
  { route: "core/retire_v1::process#Retire", phases: ["Retiring"], prestates: [] },
  { route: "core/retire_v1::process_checkpoint_prepare#Retire", phases: ["Retiring"], prestates: [] },
  { route: "core/retire_v1::process_checkpoint_suffix", phases: ["Retiring"], prestates: [] },
  { route: "core/retirement_replay_handoff_v1::process", phases: ["Retiring"], prestates: [] },
  { route: "core/series_open::process", phases: ["Founding"], prestates: [["Founding", "Prepaid"]] },
  { route: "custody/retirement_replay_handoff_v1::process", phases: ["Retiring"], prestates: [] },
  { route: "resolution/core_effect::process_direct_funding_activation_v1", phases: ["Founding", "Open"], prestates: [["Founding", "Prepaid"], ["Open", "Consumed"]] },
  { route: "resolution/core_effect::process_direct_funding_close_v1", phases: ["Retiring"], prestates: [["Retiring", "Consumed"]] },
  { route: "resolution/process_admit#AdmitTerminal", phases: ["Open"], prestates: [["Open", "Consumed"]] },
  { route: "resolution/process_capture#Capture", phases: ["Open"], prestates: [["Open", "Consumed"]] },
  { route: "resolution/process_close#CloseFund", phases: ["Retiring"], prestates: [["Retiring", "Consumed"]] },
  { route: "resolution/process_commit_deadline_failure#CommitDeadlineFailure", phases: ["Open"], prestates: [["Open", "Consumed"]] },
  { route: "resolution/process_commit_failure#CommitFailure", phases: ["Open"], prestates: [["Open", "Consumed"]] },
  { route: "resolution/process_consume#ConsumeRecord", phases: ["Open"], prestates: [["Open", "Consumed"]] },
  { route: "resolution/process_create#CreateFund", phases: ["Founding", "Open"], prestates: [["Founding", "Prepaid"], ["Open", "Consumed"]] },
  { route: "resolution/process_create_record#CreateRecord", phases: ["Open"], prestates: [["Open", "Consumed"]] },
  { route: "resolution/process_settle#Settle", phases: ["Open"], prestates: [["Open", "Consumed"]] },
  { route: "resolution/process_submit#magic", phases: ["Open"], prestates: [["Open", "Consumed"]] },
  { route: "resolution/process_verify#VerifyFundReady", phases: ["Founding", "Open"], prestates: [["Founding", "Prepaid"], ["Open", "Consumed"]] },
  { route: "resolution/provider_instruction_v3::process_provider_resolution_v3", phases: ["Open"], prestates: [["Open", "Consumed"]] },
  { route: "trading/direct_begin_retiring_v1::process_direct_begin_retiring_v1", phases: ["Retiring"], prestates: [] },
  { route: "trading/direct_close_maker_v1::process_direct_close_maker_v1", phases: ["Retiring"], prestates: [] },
  { route: "trading/direct_replay_setup_v1::process_direct_replay_setup_v1", phases: ["Open"], prestates: [] },
  { route: "trading/direct_token_setup_v1::process_direct_token_setup_v1", phases: ["Open"], prestates: [] },
];

/** The gate for one route, or `null` when the census read none for it. */
export function routePhaseGateV1(route: string): RoutePhaseGateV1 | null {
  return ROUTE_PHASE_GATES_V1.find((gate) => gate.route === route) ?? null;
}

/**
 * Routes whose admissibility is over a state machine this table cannot state.
 *
 * A Source resolution state, a Dealer root's lifecycle, a Series ticket: none
 * of them is the Core Market's phase, and a Market is `Open` for the whole
 * span in which its Source moves `Primary` to `Resolved`. So these routes are
 * NOT ungated, and a consumer that treated them as ungated would report an
 * admission the chain refuses. A consumer that cannot observe the named
 * machine must say `needs-chain` and not `no-phase-gate`.
 */
export interface RouteOtherMachineGateV1 {
  readonly route: string;
  readonly machines: ReadonlyArray<string>;
}

export const ROUTES_GATED_ON_ANOTHER_MACHINE_V1: ReadonlyArray<RouteOtherMachineGateV1> = [
  { route: "core/activate_capability_child#ActivateCapability", machines: ["funding-ledger"] },
  { route: "core/capability::process#ActivateCapability", machines: ["funding-ledger"] },
  { route: "core/capability::process#CloseCapability", machines: ["funding-ledger"] },
  { route: "core/close_capability_child#CloseCapability", machines: ["funding-ledger"] },
  { route: "core/process_found#FoundAndPermit", machines: ["projected-custody"] },
  { route: "core/series_consume::process", machines: ["projected-custody", "series-ticket"] },
  { route: "core/series_open::process", machines: ["series-ticket"] },
  { route: "core/series_permit_expiry::process", machines: ["series-ticket"] },
  { route: "core/series_permit_expiry_precommit_v1::process", machines: ["series-ticket"] },
  { route: "custody/abort_open_and_close#AbortOpenAndClose", machines: ["projected-custody"] },
  { route: "custody/abort_source_and_close#AbortSourceAndClose", machines: ["projected-custody"] },
  { route: "custody/dealer_reservation_v1::process", machines: ["dealer-checkpoint"] },
  { route: "custody/lock_hoard#LockHoard", machines: ["projected-custody"] },
  { route: "custody/lock_hoard_and_close_source#LockHoardAndCloseSource", machines: ["projected-custody"] },
  { route: "custody/open_hoard#OpenHoard", machines: ["projected-custody"] },
  { route: "custody/open_source_compartment#OpenSourceCompartment", machines: ["projected-custody"] },
  { route: "custody/realize_and_close#RealizeAndClose", machines: ["projected-custody"] },
  { route: "custody/refund_and_close#RefundAndClose", machines: ["projected-custody"] },
  { route: "resolution/process_abandon#magic", machines: ["source"] },
  { route: "resolution/process_capture#Capture", machines: ["source"] },
  { route: "resolution/process_commit_failure#CommitFailure", machines: ["source"] },
  { route: "resolution/process_settle#Settle", machines: ["source"] },
  { route: "trading/dealer_scenario_checkpoint_v1::process_dealer_scenario_checkpoint_cleanup_v1", machines: ["dealer-checkpoint"] },
  { route: "trading/dealer_scenario_checkpoint_v1::process_dealer_scenario_checkpoint_commit_v1", machines: ["dealer-checkpoint"] },
  { route: "trading/dealer_scenario_checkpoint_v1::process_dealer_scenario_checkpoint_evaluate_v1", machines: ["dealer-checkpoint"] },
  { route: "trading/dealer_scenario_checkpoint_v1::process_dealer_scenario_checkpoint_page_v1", machines: ["dealer-checkpoint"] },
  { route: "trading/dealer_scenario_checkpoint_v1::process_dealer_scenario_checkpoint_reserve_v1", machines: ["dealer-checkpoint"] },
  { route: "trading/dealer_scenario_checkpoint_v1::process_dealer_scenario_checkpoint_rollback_v1", machines: ["dealer-checkpoint"] },
  { route: "trading/direct_begin_retiring_v1::process_direct_begin_retiring_v1", machines: ["direct-root"] },
  { route: "trading/direct_close_maker_v1::process_direct_close_maker_v1", machines: ["direct-root"] },
  { route: "trading/direct_token_setup_v1::process_direct_token_setup_v1", machines: ["direct-root"] },
];

/** The machines gating one route that this table cannot state, if any. */
export function routeOtherMachineGateV1(route: string): RouteOtherMachineGateV1 | null {
  return ROUTES_GATED_ON_ANOTHER_MACHINE_V1.find((entry) => entry.route === route) ?? null;
}

/**
 * Routes whose program persists NO lifecycle discriminant for them to consult.
 *
 * Absent from `ROUTE_PHASE_GATES_V1` for a reason no further naming will
 * change: the Registry authenticates ownership, PDA derivation, account
 * vacancy and digest identity, and not one of those is a state byte. A client
 * told only "no gate was read" waits forever for an answer that does not
 * exist; a client told this can say so and move on. Still NOT an admission --
 * every account, release and request check is ahead of the act regardless.
 */
export const ROUTES_WITHOUT_A_STATE_MACHINE_V1: ReadonlyArray<string> = [
  "registry/continuation_v1::process",
  "registry/hot_continuation_v2::process",
  "registry/lineage_v1::process",
  "registry/process_abort#4",
  "registry/process_activate_role#ActivateRole",
  "registry/process_append#2",
  "registry/process_begin#5",
  "registry/process_finalize#3",
  "registry/process_instruction",
  "registry/process_reauthenticate#Reauthenticate",
  "registry/record_v1::dispatch",
];

/** Whether this route's program has no lifecycle discriminant at all. */
export function routeHasNoStateMachineV1(route: string): boolean {
  return ROUTES_WITHOUT_A_STATE_MACHINE_V1.includes(route);
}
