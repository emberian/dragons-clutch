/* tslint:disable */
/* eslint-disable */

/**
 * One basis-point unit, for the loader's post-load re-check.
 */
export function partition_quality_basis_points_per_unit_v1(): bigint;

/**
 * The ceiling on an author's ceiling, for the loader's post-load re-check.
 */
export function partition_quality_maximum_ceiling_bps_v1(): number;

/**
 * The largest volatility a band may state, for the wizard's own input bound.
 */
export function partition_quality_maximum_volatility_bps_v1(): number;

/**
 * Measure one partition against its own founding belief, through the gate.
 *
 * Returns the compiler's report, or `{"error": "<the compiler's own refusal>"}`.
 * A refusal is never softened into a warning here: `DegenerateOutcomePartition`
 * reaches the browser as that word.
 */
export function require_interesting_partition_v1_wasm(request_json: string): string;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly partition_quality_basis_points_per_unit_v1: () => bigint;
    readonly partition_quality_maximum_ceiling_bps_v1: () => number;
    readonly partition_quality_maximum_volatility_bps_v1: () => number;
    readonly require_interesting_partition_v1_wasm: (a: number, b: number) => [number, number];
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
