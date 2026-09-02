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
export const ROUTE_COUNT_V1 = 169 as const;

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
