/* tslint:disable */
/* eslint-disable */

/**
 * Build the authenticated payout manifest. Browser entry point.
 */
export function build_wallet_terminal_payout_manifest_v1(input_json: string, snapshot_json: string): string;

/**
 * The settlement frame width, read from Claims for the client to check against.
 */
export function terminal_settlement_account_count_v3(): number;

/**
 * The candidate domain, read from Claims rather than written down.
 */
export function terminal_settlement_candidate_domain_v3(): string;

/**
 * The settlement request width, read from Claims rather than written down.
 */
export function terminal_settlement_request_bytes_v3(): number;

/**
 * Every address the derivation authenticates. Browser entry point.
 */
export function wallet_terminal_payout_addresses_v1(input_json: string): string;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly build_wallet_terminal_payout_manifest_v1: (a: number, b: number, c: number, d: number) => [number, number, number, number];
    readonly terminal_settlement_account_count_v3: () => number;
    readonly terminal_settlement_candidate_domain_v3: () => [number, number];
    readonly terminal_settlement_request_bytes_v3: () => number;
    readonly wallet_terminal_payout_addresses_v1: (a: number, b: number) => [number, number, number, number];
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
