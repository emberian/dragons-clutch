/* tslint:disable */
/* eslint-disable */

/**
 * Decode one exact Upgradeable Loader Program-to-ProgramData link.
 */
export function derive_source_provider_programdata_v1(source: string): string;

/**
 * Derive the complete reclaim routing hints from one lifecycle account.
 */
export function derive_source_provider_reclaim_coordinates_v1(source: string): string;

/**
 * Derive the first provider-submit coordinates from one exact Market.
 */
export function derive_source_provider_submit_base_v1(source: string): string;

/**
 * Derive the lifecycle and Receiver authority for one fresh update signer.
 */
export function derive_source_provider_submit_fresh_v1(source: string): string;

/**
 * Continue submit discovery through SourceMaterial and infrastructure.
 */
export function derive_source_provider_submit_material_v1(source: string): string;

/**
 * Derive the ProviderRelease pair selected by one SourceSpec.
 */
export function derive_source_provider_submit_provider_release_v1(source: string): string;

/**
 * Derive the Pyth release pair selected by one ProviderRelease.
 */
export function derive_source_provider_submit_pyth_release_v1(source: string): string;

/**
 * Derive the exact Receiver and Router frame from Pyth and verified VAA.
 */
export function derive_source_provider_submit_pyth_v1(source: string): string;

/**
 * Plan one exact permissionless provider reclaim from finalized chain state.
 */
export function plan_source_provider_reclaim_v1(source: string): string;

/**
 * Plan one exact provider submission from one complete finalized frame.
 */
export function plan_source_provider_submit_v1(source: string): string;

/**
 * Read one sponsored `PriceUpdateV2` account through the Source family's own decoder.
 */
export function read_source_provider_price_update_v1(source: string): string;

/**
 * Reauthenticate the lifecycle and Receiver update created by a submission.
 */
export function verify_source_provider_submit_poststate_v1(source: string): string;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly derive_source_provider_programdata_v1: (a: number, b: number) => [number, number, number, number];
    readonly derive_source_provider_reclaim_coordinates_v1: (a: number, b: number) => [number, number, number, number];
    readonly derive_source_provider_submit_base_v1: (a: number, b: number) => [number, number, number, number];
    readonly derive_source_provider_submit_fresh_v1: (a: number, b: number) => [number, number, number, number];
    readonly derive_source_provider_submit_material_v1: (a: number, b: number) => [number, number, number, number];
    readonly derive_source_provider_submit_provider_release_v1: (a: number, b: number) => [number, number, number, number];
    readonly derive_source_provider_submit_pyth_release_v1: (a: number, b: number) => [number, number, number, number];
    readonly derive_source_provider_submit_pyth_v1: (a: number, b: number) => [number, number, number, number];
    readonly plan_source_provider_reclaim_v1: (a: number, b: number) => [number, number, number, number];
    readonly plan_source_provider_submit_v1: (a: number, b: number) => [number, number, number, number];
    readonly read_source_provider_price_update_v1: (a: number, b: number) => [number, number, number, number];
    readonly verify_source_provider_submit_poststate_v1: (a: number, b: number) => [number, number, number, number];
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
