/* tslint:disable */
/* eslint-disable */

/**
 * The finalized linked-basis RECORD digest this owner was admitted against.
 *
 * THE BUG THIS CLOSES. The browser derived the linked-basis record address
 * from the Claims aggregate's `basis_id`. That is the SEMANTIC LiabilityBasisV2
 * identity: it authenticates a basis body and cannot address one, because the
 * semantic preimage ignores bytes the record digest covers. Measured on devnet
 * cohort-11, the raw-record PDA it derives is VACANT while the record the
 * campaign published sits at the PDA of a digest the aggregate does not carry
 * -- so the frame named an account nothing lives at, and the planner failed
 * decoding empty bytes instead of saying which coordinate was wrong.
 *
 * `ProtocolPositionAdmissionEvidenceV2` is the only place on chain that names
 * the record digest, and it is decoded HERE rather than sliced in TypeScript,
 * because an offset written down in a client is the same defect one level up.
 */
export function linked_basis_record_digest_v1(admission_base64: string): string;

/**
 * Plan one wallet-authorized Position admission. Browser entry point.
 */
export function plan_user_position_admission_v1_wasm(snapshot_json: string): string;

/**
 * The outer frame width, read from the contract for the client to check against.
 */
export function user_position_admission_account_count_v1(): number;

/**
 * The outer selector, read from the contract rather than written down.
 */
export function user_position_admission_magic_v1(): string;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly linked_basis_record_digest_v1: (a: number, b: number) => [number, number, number, number];
    readonly plan_user_position_admission_v1_wasm: (a: number, b: number) => [number, number, number, number];
    readonly user_position_admission_account_count_v1: () => number;
    readonly user_position_admission_magic_v1: () => [number, number];
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
