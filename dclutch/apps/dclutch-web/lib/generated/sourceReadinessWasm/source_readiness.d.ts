/* tslint:disable */
/* eslint-disable */

/**
 * Derive the admitted terminal certificate and Source closure receipt.
 */
export function derive_source_close_detail_v1(source_json: string): string;

/**
 * Derive the first account frame from one exact Core Market.
 */
export function derive_source_readiness_base_v1(market_json: string): string;

/**
 * Derive the recovery pair and Resolution subset ledger from exact records.
 */
export function derive_source_readiness_detail_v1(records_json: string): string;

/**
 * Derive the optional recovery-policy pair after reading SourceMaterialV3.
 */
export function derive_source_readiness_recovery_v1(source_json: string): string;

/**
 * Derive the initial terminal-admission Source and Product-root coordinates.
 */
export function derive_source_terminal_base_v1(source_json: string): string;

/**
 * Derive the certificate from exact Source and ResultDomain bytes.
 */
export function derive_source_terminal_detail_v1(source_json: string): string;

/**
 * Derive Product child coordinates from the selected Product root.
 */
export function derive_source_terminal_product_v1(source_json: string): string;

/**
 * Plan exact receipt prepayment or the signer-free V7 direct close.
 */
export function plan_source_close_fund_v1(source_json: string): string;

/**
 * Plan one adjacent Source-readiness action from one finalized observation.
 */
export function plan_source_readiness_v1(snapshot_json: string): string;

/**
 * Plan terminal admission or prove exact already-admitted completion.
 */
export function plan_source_terminal_v1(source_json: string): string;

/**
 * Authenticate one finalized Source closure receipt against persisted plan facts.
 */
export function verify_source_close_receipt_v1(source_json: string): string;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly derive_source_close_detail_v1: (a: number, b: number) => [number, number, number, number];
    readonly derive_source_readiness_base_v1: (a: number, b: number) => [number, number, number, number];
    readonly derive_source_readiness_detail_v1: (a: number, b: number) => [number, number, number, number];
    readonly derive_source_readiness_recovery_v1: (a: number, b: number) => [number, number, number, number];
    readonly derive_source_terminal_base_v1: (a: number, b: number) => [number, number, number, number];
    readonly derive_source_terminal_detail_v1: (a: number, b: number) => [number, number, number, number];
    readonly derive_source_terminal_product_v1: (a: number, b: number) => [number, number, number, number];
    readonly plan_source_close_fund_v1: (a: number, b: number) => [number, number, number, number];
    readonly plan_source_readiness_v1: (a: number, b: number) => [number, number, number, number];
    readonly plan_source_terminal_v1: (a: number, b: number) => [number, number, number, number];
    readonly verify_source_close_receipt_v1: (a: number, b: number) => [number, number, number, number];
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __externref_table_dealloc: (a: number) => void;
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
