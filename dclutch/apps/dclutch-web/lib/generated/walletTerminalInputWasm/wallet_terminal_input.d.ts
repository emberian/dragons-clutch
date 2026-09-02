/* tslint:disable */
/* eslint-disable */

/**
 * Build the exact payout input. Browser entry point.
 */
export function build_wallet_terminal_payout_input_v1(request_json: string, round_one_json: string, round_two_json: string): string;

/**
 * The Core Market state width, read from its codec for the client to check.
 */
export function core_state_bytes_v1(): number;

/**
 * The request with its address book derived. Browser entry point.
 */
export function derive_wallet_terminal_input_request_v1(request_json: string, round_one_json: string, round_two_json: string, round_three_json: string): string;

/**
 * The Claims aggregate header width, read from Claims rather than written down.
 */
export function liability_basis_market_header_bytes_v2(): number;

/**
 * The Claims Position header width, read from Claims rather than written down.
 */
export function liability_basis_position_header_bytes_v2(): number;

/**
 * Every address phase zero's round three observes. Browser entry point.
 */
export function wallet_terminal_input_book_round_three_addresses_v1(request_json: string, round_one_json: string, round_two_json: string): string;

/**
 * Every address phase zero's round two observes. Browser entry point.
 */
export function wallet_terminal_input_book_round_two_addresses_v1(request_json: string, round_one_json: string): string;

/**
 * Every address round two observes. Browser entry point.
 */
export function wallet_terminal_input_frame_addresses_v1(request_json: string, round_one_json: string): string;

/**
 * Every address round one observes. Browser entry point.
 */
export function wallet_terminal_input_round_one_addresses_v1(request_json: string): string;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly build_wallet_terminal_payout_input_v1: (a: number, b: number, c: number, d: number, e: number, f: number) => [number, number, number, number];
    readonly core_state_bytes_v1: () => number;
    readonly derive_wallet_terminal_input_request_v1: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => [number, number, number, number];
    readonly liability_basis_market_header_bytes_v2: () => number;
    readonly liability_basis_position_header_bytes_v2: () => number;
    readonly wallet_terminal_input_book_round_three_addresses_v1: (a: number, b: number, c: number, d: number, e: number, f: number) => [number, number, number, number];
    readonly wallet_terminal_input_book_round_two_addresses_v1: (a: number, b: number, c: number, d: number) => [number, number, number, number];
    readonly wallet_terminal_input_frame_addresses_v1: (a: number, b: number, c: number, d: number) => [number, number, number, number];
    readonly wallet_terminal_input_round_one_addresses_v1: (a: number, b: number) => [number, number, number, number];
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
