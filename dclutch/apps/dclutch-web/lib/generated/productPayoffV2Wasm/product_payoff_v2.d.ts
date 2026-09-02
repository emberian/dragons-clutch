/* tslint:disable */
/* eslint-disable */

/**
 * Evaluate one canonical Product V2 record at every coordinate in the request.
 *
 * One call per curve rather than one per point: the boundary crossing is the
 * only cost that scales with the sample, and the arithmetic inside is the
 * codec's.
 */
export function evaluate_product_payoff_v2_wasm(request_json: string): string;

/**
 * The canonical record width, for the loader's post-load re-check.
 */
export function product_payoff_v2_bytes_v1(): number;

/**
 * The canonical record magic, for the loader's post-load re-check.
 */
export function product_payoff_v2_magic_v1(): string;

/**
 * The canonical record version, for the loader's post-load re-check.
 */
export function product_payoff_v2_version_v1(): number;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly evaluate_product_payoff_v2_wasm: (a: number, b: number) => [number, number];
    readonly product_payoff_v2_bytes_v1: () => number;
    readonly product_payoff_v2_magic_v1: () => [number, number];
    readonly product_payoff_v2_version_v1: () => number;
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
