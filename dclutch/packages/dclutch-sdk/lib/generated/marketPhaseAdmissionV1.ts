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
  { route: "core/execute_provider_v3::process#ExecuteProvider", phases: ["Open"], prestates: [["Open", "Consumed"]] },
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
];

/** The gate for one route, or `null` when the census read none for it. */
export function routePhaseGateV1(route: string): RoutePhaseGateV1 | null {
  return ROUTE_PHASE_GATES_V1.find((gate) => gate.route === route) ?? null;
}
