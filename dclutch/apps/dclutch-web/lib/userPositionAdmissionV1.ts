import { hex, sha256 } from './bytes';
import {
  USER_POSITION_ADMISSION_ACCOUNT_COUNT_V1,
  USER_POSITION_ADMISSION_OWNER_ACCOUNT_V1,
  USER_POSITION_ADMISSION_PLAN_FORMAT_V1,
  USER_POSITION_ADMISSION_WASM_BYTES_V1,
  USER_POSITION_ADMISSION_WASM_SHA256_V1,
} from './generated/userPositionAdmissionWasmV1';

/**
 * The compiled admission planner, and the browser's half of the seam.
 *
 * THE DEFECT THIS CLOSES. `JoinPanel` said, in its own words, that admission
 * "needs the position owner's signature over a frame the browser cannot yet
 * assemble byte-exactly", and handed the reader a CLI command. You cannot
 * trade in a market you cannot join, so that sentence is the whole reason
 * maker/taker trade was present-but-unreachable for a stranger holding only a
 * wallet.
 *
 * WHAT THIS FILE DELIBERATELY DOES NOT DO is build the frame. Twenty-seven
 * accounts with per-coordinate privileges, two rent deficits and a predicted
 * Claims receipt, reimplemented in TypeScript, is the mirror this application
 * keeps convicting — `evaluateProductV2` is already one, and the answer to a
 * mirror is never a second mirror. `plan_user_position_admission_v1` is a pure
 * deterministic planner, so it is COMPILED to wasm32 and called. Every
 * coordinate here is the Rust owner's.
 *
 * The web shell keeps what the planner must never have: finalized RPC, Wallet
 * Standard, durable storage, and submission.
 */

/** One account coordinate in the planner's own instruction frame. */
export type AdmissionAccountMetaV1 = Readonly<{
  pubkey: string;
  isSigner: boolean;
  isWritable: boolean;
}>;

/** One unsigned instruction, exactly as the planner ordered it. */
export type AdmissionInstructionV1 = Readonly<{
  programId: string;
  accounts: ReadonlyArray<AdmissionAccountMetaV1>;
  dataBase64: string;
}>;

/** The planner's answer: zero to two rent transfers, then the Trading outer. */
export type UserPositionAdmissionPlanV1 = Readonly<{
  instructions: ReadonlyArray<AdmissionInstructionV1>;
  requiredSigner: string;
  observedSlot: string;
  position: string;
  admission: string;
  positionTopUpLamports: string;
  admissionTopUpLamports: string;
}>;

/** The three functions the compiled planner exposes. */
export type UserPositionAdmissionWasmV1 = Readonly<{
  /** The linked-basis record digest an owner's admission record names. */
  linked_basis_record_digest_v1(admissionBase64: string): string;
  plan_user_position_admission_v1_wasm(snapshotJson: string): string;
  user_position_admission_account_count_v1(): number;
  user_position_admission_magic_v1(): string;
}>;

function text(value: unknown, field: string): string {
  if (typeof value !== 'string' || value === '') throw new Error(`admission plan ${field} is absent`);
  return value;
}

/**
 * Hostile-decode the planner's own answer.
 *
 * The width check is not defensive noise. `USER_POSITION_ADMISSION_ACCOUNT_COUNT_V1`
 * is emitted from the contract and pinned again inside the WASM crate by a
 * `const _: () = assert!(...)`, so the planner cannot emit another width — but
 * a substituted or truncated transport can, and this is the transaction a
 * wallet is about to sign. The client checks the number it was told against
 * the number the contract states, and never writes either down.
 */
export function parseUserPositionAdmissionPlanV1(source: string): UserPositionAdmissionPlanV1 {
  let parsed: unknown;
  try { parsed = JSON.parse(source); } catch { throw new Error('admission plan is not JSON'); }
  if (parsed === null || typeof parsed !== 'object') throw new Error('admission plan is not an object');
  const plan = parsed as Record<string, unknown>;
  if (plan.format !== USER_POSITION_ADMISSION_PLAN_FORMAT_V1) {
    throw new Error('admission plan is not the exact accepted format');
  }
  if (plan.accountCount !== USER_POSITION_ADMISSION_ACCOUNT_COUNT_V1) {
    throw new Error(`admission plan states a ${String(plan.accountCount)}-account frame where the contract has ${USER_POSITION_ADMISSION_ACCOUNT_COUNT_V1}`);
  }
  if (plan.ownerAccountIndex !== USER_POSITION_ADMISSION_OWNER_ACCOUNT_V1) {
    throw new Error('admission plan places the owner at another frame coordinate than the contract');
  }
  const rawInstructions = plan.instructions;
  if (!Array.isArray(rawInstructions)) throw new Error('admission plan carries no instruction list');
  const instructions = rawInstructions.map((entry, index) => {
    const one = entry as Record<string, unknown>;
    const metas = one.accounts;
    if (!Array.isArray(metas)) throw new Error(`admission instruction ${index} carries no account list`);
    return Object.freeze({
      programId: text(one.programId, `instruction ${index} program`),
      dataBase64: text(one.dataBase64, `instruction ${index} data`),
      accounts: Object.freeze(metas.map((meta, at) => {
        const account = meta as Record<string, unknown>;
        return Object.freeze({
          pubkey: text(account.pubkey, `instruction ${index} account ${at}`),
          isSigner: account.isSigner === true,
          isWritable: account.isWritable === true,
        });
      })),
    });
  });
  return Object.freeze({
    instructions: Object.freeze(instructions),
    requiredSigner: text(plan.requiredSigner, 'required signer'),
    observedSlot: text(plan.observedSlot, 'observed slot'),
    position: text(plan.position, 'Position'),
    admission: text(plan.admission, 'admission record'),
    positionTopUpLamports: text(plan.positionTopUpLamports, 'Position top-up'),
    admissionTopUpLamports: text(plan.admissionTopUpLamports, 'admission top-up'),
  });
}

/** Load the checked Rust planner blob; unverified fetched bytes never execute. */
export async function loadUserPositionAdmissionWasmV1(
  fetcher: typeof fetch = (input, init) => globalThis.fetch(input, init),
): Promise<UserPositionAdmissionWasmV1> {
  const url = new URL('./generated/userPositionAdmissionWasm/user_position_admission_bg.wasm', import.meta.url);
  const response = await fetcher(url);
  if (!response.ok) throw new Error(`admission planner WASM fetch failed with HTTP ${response.status}`);
  const bytes = new Uint8Array(await response.arrayBuffer());
  if (bytes.length !== USER_POSITION_ADMISSION_WASM_BYTES_V1
      || hex(await sha256(bytes)) !== USER_POSITION_ADMISSION_WASM_SHA256_V1) {
    throw new Error('admission planner WASM bytes do not match the generated Rust artifact identity');
  }
  const wasmModule = await import('./generated/userPositionAdmissionWasm/user_position_admission.js');
  await wasmModule.default({ module_or_path: bytes });
  const width = wasmModule.user_position_admission_account_count_v1();
  if (width !== USER_POSITION_ADMISSION_ACCOUNT_COUNT_V1) {
    // The blob agreed with its digest and still disagrees with the contract.
    // That can only mean the emitted facts and the artifact came from
    // different trees, which is exactly the drift the canary exists for.
    throw new Error(`admission planner reports a ${width}-account frame where the contract has ${USER_POSITION_ADMISSION_ACCOUNT_COUNT_V1}`);
  }
  return Object.freeze({
    plan_user_position_admission_v1_wasm: wasmModule.plan_user_position_admission_v1_wasm,
    linked_basis_record_digest_v1: wasmModule.linked_basis_record_digest_v1,
    user_position_admission_account_count_v1: wasmModule.user_position_admission_account_count_v1,
    user_position_admission_magic_v1: wasmModule.user_position_admission_magic_v1,
  });
}
