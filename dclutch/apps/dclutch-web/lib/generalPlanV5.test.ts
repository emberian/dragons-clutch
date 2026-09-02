import {
  AddressLookupTableAccount,
  AddressLookupTableProgram,
  ComputeBudgetProgram,
  PublicKey,
  TransactionInstruction,
  TransactionMessage,
  VersionedTransaction,
} from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import { hex, sha256 } from './bytes';
import { type RpcAccount, type SolanaRpcClient } from './rpc';
import * as Abi from './generated/generalSuccessorV5';
import {
  decodeGeneralCandidateV1,
  decodeGeneralHotReceiptV3,
  decodeGeneralLocalStateV3,
  decodeGeneralVerifiedCandidateV2,
  decodeGeneralVerifierV2,
  decodeGeneralSuccessorPlanDocumentV5,
  generalBatchOccurrenceIdentityV1,
  inspectGeneralSuccessorPlanV5,
  reacquireGeneralSuccessorStatusV5,
  type GeneralSuccessorActionV5,
} from './generalPlanV5';

const ACTIONS = [
  'consider', 'freeze', 'initialize-settlement', 'collect', 'materialize', 'distribute', 'close',
  'open-batch', 'place-order', 'cancel-order', 'close-batch', 'submit-candidate',
  'verify-candidate-row', 'release-order', 'close-candidate',
] as const;
const PLAN_ACTIONS = ACTIONS;
const MAX_U64 = 18_446_744_073_709_551_615n;

function bytes(value: number): Uint8Array { return new Uint8Array(32).fill(value); }
function id(value: number): string { return hex(bytes(value)); }
function key(value: number): PublicKey { const output = bytes((value % 250) + 1); output[0] = value; return new PublicKey(output); }
function base64(value: Uint8Array): string { return Buffer.from(value).toString('base64'); }
function hexBytes(value: string): Uint8Array { return Uint8Array.from(Array.from({ length: value.length / 2 }, (_, index) => Number.parseInt(value.slice(index * 2, index * 2 + 2), 16))); }
function putU16(output: Uint8Array, offset: number, value: number): void { new DataView(output.buffer, output.byteOffset + offset, 2).setUint16(0, value, true); }
function putU32(output: Uint8Array, offset: number, value: number): void { new DataView(output.buffer, output.byteOffset + offset, 4).setUint32(0, value, true); }
function putU64(output: Uint8Array, offset: number, value: bigint): void { new DataView(output.buffer, output.byteOffset + offset, 8).setBigUint64(0, value, true); }

function request(action: GeneralSuccessorActionV5): Uint8Array {
  const tag = ACTIONS.indexOf(action); const v3 = tag >= Abi.ACTION_OPEN_BATCH_V3; const output = new Uint8Array(Abi.GENERAL_REQUEST_BYTES_V3);
  output.set(v3 ? Abi.GENERAL_REQUEST_MAGIC_V3 : Abi.GENERAL_REQUEST_MAGIC_V2); putU16(output, 8, v3 ? 3 : 2); output[10] = tag;
  const row = action === 'collect' || action === 'distribute';
  const revisionless = action === 'place-order' || action === 'cancel-order' || action === 'submit-candidate' || action === 'release-order' || action === 'close-candidate';
  output[11] = row ? 1 : 0; putU64(output, 16, action === 'initialize-settlement' || revisionless ? 0n : 9n);
  if (action !== 'freeze') output.set(bytes(52), 24);
  putU32(output, 56, action === 'consider' || row ? 2 : 0); output[60] = row ? 3 : 0; output[61] = 7; output[62] = action === 'close' ? 8 : 0;
  if (action === 'place-order' || action === 'cancel-order') output[62] = 8;
  if (action === 'verify-candidate-row') { output[62] = 8; output[63] = 9; }
  return output;
}

function childStart(action: GeneralSuccessorActionV5): number {
  const starts: Readonly<Record<GeneralSuccessorActionV5, number>> = Object.freeze({
    consider: 10, freeze: 8, 'initialize-settlement': 11, collect: 10, materialize: 9,
    distribute: 10, close: 10, 'open-batch': 8, 'place-order': 10, 'cancel-order': 9,
    'close-batch': 8, 'submit-candidate': 11, 'verify-candidate-row': 15,
    'release-order': 8, 'close-candidate': Abi.GENERAL_CLOSE_CANDIDATE_CHILD_START_V3,
  });
  return starts[action];
}

function routes(action: GeneralSuccessorActionV5): unknown[] {
  const roles = action === 'initialize-settlement' ? ['claims', 'custody', 'custody']
    : action === 'collect' || action === 'materialize' || action === 'distribute' ? ['claims', 'custody']
      : action === 'close' ? ['custody', 'claims', 'custody', 'custody'] : [];
  let start = childStart(action);
  return roles.map((role, route) => {
    const receiptDependencies = action === 'initialize-settlement' && route === 2
      ? [{ producerRole: 'custody', producerRoute: 1, expectedReceiptBytes: Abi.GENERAL_CUSTODY_RECEIPT_BYTES_V1 }]
      : action === 'close' && route === 3
        ? [{ producerRole: 'custody', producerRoute: 2, expectedReceiptBytes: Abi.GENERAL_CUSTODY_RECEIPT_BYTES_V1 }] : [];
    const output = { route, role, accountStart: start, accountCount: 2, receiptDependencies };
    start += 2; return output;
  });
}

async function fixture(action: GeneralSuccessorActionV5, outcomeCount = 1): Promise<{ text: string; raw: Record<string, unknown>; request: Uint8Array; table: AddressLookupTableAccount; accounts: ReadonlyArray<PublicKey> }> {
  const trading = key(201); const payer = key(202); const lookup = key(203); const market = key(1); const root = key(2); const primary = key(70); const secondary = key(71); const result = key(72);
  const requestBytes = request(action); const envelope = new Uint8Array(Abi.GENERAL_HOT_ENVELOPE_BYTES_V3);
  envelope.set(Abi.GENERAL_HOT_MAGIC_V3); putU16(envelope, 8, Abi.GENERAL_HOT_VERSION_V3); putU16(envelope, 10, Abi.GENERAL_HOT_PROFILE_V3);
  putU32(envelope, Abi.GENERAL_ENVELOPE_REQUEST_BYTES_OFFSET_V3, Abi.GENERAL_REQUEST_BYTES_V3); envelope.set(bytes(41), Abi.GENERAL_ENVELOPE_RELEASE_SET_OFFSET_V3);
  envelope.set(market.toBytes(), Abi.GENERAL_ENVELOPE_MARKET_OFFSET_V3); putU64(envelope, Abi.GENERAL_ENVELOPE_GENERATION_OFFSET_V3, 7n); envelope.set(bytes(42), Abi.GENERAL_ENVELOPE_ROOT_PRESTATE_DIGEST_OFFSET_V3);
  const data = new Uint8Array(envelope.length + requestBytes.length); data.set(envelope); data.set(requestBytes, envelope.length);
  const accounts = Array.from({ length: 52 }, (_, index) => key(index + 3)); accounts[0] = market; accounts[1] = root; accounts[5] = primary; accounts[6] = secondary; accounts[7] = result; accounts[46] = payer;
  if (action === 'close-candidate') { accounts[6] = payer; accounts[7] = key(63); }
  const metas = accounts.map((pubkey, index) => ({ pubkey, isSigner: index === 46 || (action === 'close-candidate' && index === 6), isWritable: index === 46 || index === 5 || index === 6 || index === 7 }));
  const addresses = accounts.filter((_, index) => index !== 46).sort((left, right) => {
    // Byte-lexicographic, without Buffer: node's Buffer.compare static is typed
    // against Buffer under the plain node lib set, and this package must
    // typecheck without the DOM escape hatch.
    const a = left.toBytes();
    const b = right.toBytes();
    for (let index = 0; index < a.length && index < b.length; index += 1) {
      if (a[index] !== b[index]) return (a[index] ?? 0) < (b[index] ?? 0) ? -1 : 1;
    }
    return a.length === b.length ? 0 : a.length < b.length ? -1 : 1;
  });
  const table = new AddressLookupTableAccount({ key: lookup, state: { deactivationSlot: MAX_U64, lastExtendedSlot: 1, lastExtendedSlotStartIndex: 0, authority: undefined, addresses } });
  const instruction = new TransactionInstruction({ programId: trading, keys: metas, data: data as Buffer });
  const heap = ComputeBudgetProgram.requestHeapFrame({ bytes: Abi.GENERAL_HOT_HEAP_FRAME_BYTES_V3 });
  const transaction = new VersionedTransaction(new TransactionMessage({ payerKey: payer, recentBlockhash: key(204).toBase58(), instructions: [heap, instruction] }).compileToV0Message([table]));
  const artifactNames = ['programSet', 'descriptor', 'config', 'accountProfile', 'lifecyclePolicy', 'requestProfile', 'strategy', 'certificate', 'admission', 'transition', 'effect'];
  const raw: Record<string, unknown> = {
    format: 'dclutch/general-successor-plan/v5', action, transactionBase64: base64(transaction.serialize()), observedSlot: '77', outcomeCount, admittedInvocationCount: 1, heapFrameBytes: Abi.GENERAL_HOT_HEAP_FRAME_BYTES_V3,
    tradingProgram: trading.toBase58(), lookupTable: lookup.toBase58(), payer: payer.toBase58(), requiredSigners: [payer.toBase58()], market: market.toBase58(), root: root.toBase58(), generation: '7',
    releaseSet: id(41), rootPrestateDigest: id(42), familyRequestDigest: hex(await sha256(requestBytes)), checkedManifestDigest: id(43), tradingArtifactRelease: id(44), generalArtifactRelease: id(45), productRecord: id(46),
    artifacts: Object.fromEntries(artifactNames.map((name, index) => [name, id(80 + index)])),
    lifecycle: {
      primary: { accountCoordinate: 5, account: primary.toBase58(), bump: 7, isMaterialized: true },
      secondary: action === 'close' || action === 'place-order' || action === 'cancel-order' || action === 'verify-candidate-row'
        ? { accountCoordinate: 6, account: secondary.toBase58(), bump: 8, isMaterialized: action === 'cancel-order' } : null,
      conditionalResult: action === 'verify-candidate-row'
        ? { accountCoordinate: 7, account: result.toBase58(), bump: 9, isMaterialized: false } : null,
      terminalCoordinate: action === 'close' ? '10' : null,
      childAccountStart: childStart(action),
    },
    childRoutes: routes(action),
  };
  return { text: JSON.stringify(raw), raw, request: requestBytes, table, accounts: Object.freeze(accounts) };
}

function localState(kind: 'selection' | 'settlement' | 'batch' | 'order', outcomeCount: number): Uint8Array {
  const body = kind === 'selection' ? new Uint8Array(Abi.GENERAL_SELECTION_BYTES_V2)
    : kind === 'settlement' ? new Uint8Array(Abi.GENERAL_SETTLEMENT_HEADER_BYTES_V2 + outcomeCount * 8)
      : kind === 'batch' ? new Uint8Array(Abi.GENERAL_BATCH_BYTES_V1)
        : new Uint8Array(Abi.GENERAL_ORDER_ROW_BASE_V1 + outcomeCount * Abi.GENERAL_ORDER_ROW_STRIDE_V1);
  if (kind === 'selection') {
    body.set(Abi.GENERAL_SELECTION_MAGIC_V2); putU16(body, 8, 2); body[10] = 1; putU32(body, 12, outcomeCount); putU64(body, 16, 9n); putU32(body, 24, 2); putU32(body, 28, 1); putU64(body, 32, 4n); putU64(body, 40, 100n);
    for (const [offset, value] of [[48, 1], [80, 2], [112, 3], [144, 4], [176, 5]] as const) body.set(bytes(value), offset);
    putU64(body, 208, 12n); putU64(body, 216, 3n);
  } else if (kind === 'settlement') {
    body.set(Abi.GENERAL_SETTLEMENT_MAGIC_V2); putU16(body, 8, 2); body[10] = Abi.GENERAL_PHASE_COLLECTING_V2; putU32(body, 12, outcomeCount); putU32(body, 16, 2); putU32(body, 20, 1); putU64(body, 24, 9n); body.set(bytes(52), 32); putU64(body, 64, 7n);
    for (let index = 0; index < outcomeCount; index += 1) putU64(body, 88 + index * 8, BigInt(index));
  } else if (kind === 'batch') {
    body.set(Abi.GENERAL_BATCH_MAGIC_V1); putU16(body, 8, 1); body[10] = Abi.GENERAL_BATCH_PHASE_V1; putU32(body, Abi.GENERAL_BATCH_OUTCOME_COUNT_OFFSET_V1, outcomeCount);
    putU64(body, Abi.GENERAL_BATCH_SEQUENCE_OFFSET_V1, 2n); putU64(body, Abi.GENERAL_BATCH_GENERATION_OFFSET_V1, 7n); body.set(key(1).toBytes(), Abi.GENERAL_BATCH_MARKET_OFFSET_V1);
    body.set(bytes(2), Abi.GENERAL_BATCH_PRODUCT_ID_OFFSET_V1); body.set(bytes(3), Abi.GENERAL_BATCH_CONFIG_ID_OFFSET_V1); putU64(body, Abi.GENERAL_BATCH_PRICE_SCALE_OFFSET_V1, 100n);
    putU64(body, Abi.GENERAL_BATCH_COLLECTION_CLOSE_SLOT_OFFSET_V1, 80n); putU32(body, Abi.GENERAL_BATCH_MAX_ORDERS_OFFSET_V1, 4); putU64(body, Abi.GENERAL_BATCH_SETTLEMENT_CLOSE_SLOT_OFFSET_V1, 100n);
    body[Abi.GENERAL_BATCH_STATUS_OFFSET_V1] = Abi.GENERAL_BATCH_STATUS_COLLECTING_V1; putU32(body, Abi.GENERAL_BATCH_ORDER_COUNT_OFFSET_V1, 1); putU64(body, Abi.GENERAL_BATCH_OPENED_ROOT_REVISION_OFFSET_V1, 9n);
    putU64(body, Abi.GENERAL_BATCH_COMMITTED_QUOTE_RESERVE_OFFSET_V1, 10n);
  } else {
    body.set(Abi.GENERAL_ORDER_MAGIC_V1); putU16(body, 8, 1); body[10] = Abi.GENERAL_ORDER_PHASE_V1; putU32(body, Abi.GENERAL_ORDER_OUTCOME_COUNT_OFFSET_V1, outcomeCount);
    putU64(body, Abi.GENERAL_ORDER_NONCE_OFFSET_V1, 2n); body.set(key(3).toBytes(), Abi.GENERAL_ORDER_OWNER_ID_OFFSET_V1); body.set(key(1).toBytes(), Abi.GENERAL_ORDER_MARKET_OFFSET_V1);
    body.set(bytes(4), Abi.GENERAL_ORDER_BATCH_ID_OFFSET_V1); putU64(body, Abi.GENERAL_ORDER_GENERATION_OFFSET_V1, 7n); putU64(body, Abi.GENERAL_ORDER_MAX_LOTS_OFFSET_V1, 2n);
    putU64(body, Abi.GENERAL_ORDER_MAX_QUOTE_DEBIT_PER_LOT_OFFSET_V1, 5n); putU64(body, Abi.GENERAL_ORDER_VALID_UNTIL_SLOT_OFFSET_V1, 100n);
    body[Abi.GENERAL_ORDER_STATE_OFFSET_V1] = Abi.GENERAL_ORDER_STATE_PLACED_V1; putU64(body, Abi.GENERAL_ORDER_STATE_OFFSET_V1 + 8, 50n);
    for (let index = 0; index < outcomeCount; index += 1) { putU64(body, Abi.GENERAL_ORDER_ROW_BASE_V1 + index * Abi.GENERAL_ORDER_ROW_STRIDE_V1, BigInt(index + 1)); putU64(body, Abi.GENERAL_ORDER_ROW_BASE_V1 + index * Abi.GENERAL_ORDER_ROW_STRIDE_V1 + 8, BigInt(index)); }
  }
  const output = new Uint8Array(Abi.GENERAL_LOCAL_STATE_HEADER_BYTES_V3 + body.length); output.set(Abi.GENERAL_LOCAL_STATE_MAGIC_V3); putU16(output, 8, Abi.GENERAL_LOCAL_STATE_VERSION_V3);
  output[10] = kind === 'selection' ? Abi.GENERAL_LOCAL_STATE_SELECTION_KIND_V3
    : kind === 'settlement' ? Abi.GENERAL_LOCAL_STATE_SETTLEMENT_KIND_V3
      : kind === 'batch' ? Abi.GENERAL_LOCAL_STATE_BATCH_KIND_V3 : Abi.GENERAL_LOCAL_STATE_ORDER_KIND_V3;
  output[11] = 7; putU64(output, 16, 50n); output.set(key(99).toBytes(), 24); output.set(body, 64); return output;
}

function candidate(status: number = Abi.GENERAL_SUBMISSION_STATUS_SUBMITTED_V1, outcomeCount = 258, candidateId = bytes(61), batchId = bytes(62)): Uint8Array {
  const body = new Uint8Array(Abi.GENERAL_SUBMISSION_BYTES_V1);
  body.set(Abi.GENERAL_SUBMISSION_MAGIC_V1); putU16(body, Abi.GENERAL_SUBMISSION_VERSION_OFFSET_V1, Abi.GENERAL_SUBMISSION_VERSION_V1); body[Abi.GENERAL_SUBMISSION_PHASE_OFFSET_V1] = Abi.GENERAL_SUBMISSION_PHASE_V1;
  putU32(body, Abi.GENERAL_SUBMISSION_OUTCOME_COUNT_OFFSET_V1, outcomeCount); putU32(body, Abi.GENERAL_SUBMISSION_PAGE_COUNT_OFFSET_V1, 2); body[Abi.GENERAL_SUBMISSION_STATUS_OFFSET_V1] = status;
  putU64(body, Abi.GENERAL_SUBMISSION_PAGE_REVISION_OFFSET_V1, 9n); body.set(candidateId, Abi.GENERAL_SUBMISSION_CANDIDATE_ID_OFFSET_V1); body.set(batchId, Abi.GENERAL_SUBMISSION_BATCH_ID_OFFSET_V1); body.set(key(63).toBytes(), Abi.GENERAL_SUBMISSION_SOLVER_ID_OFFSET_V1);
  if (status !== Abi.GENERAL_SUBMISSION_STATUS_SUBMITTED_V1) { body.set(bytes(64), Abi.GENERAL_SUBMISSION_VERIFIED_DIGEST_OFFSET_V1); putU64(body, Abi.GENERAL_SUBMISSION_VERIFIED_REVISION_OFFSET_V1, 8n); }
  putU64(body, Abi.GENERAL_SUBMISSION_SUBMITTED_SLOT_OFFSET_V1, 70n); putU32(body, Abi.GENERAL_SUBMISSION_ROW_COUNT_OFFSET_V1, 4); putU64(body, Abi.GENERAL_SUBMISSION_REWARD_RATE_OFFSET_V1, 10n); putU64(body, Abi.GENERAL_SUBMISSION_VERIFICATION_REMAINING_OFFSET_V1, 40n); putU64(body, Abi.GENERAL_SUBMISSION_CLEANUP_REMAINING_OFFSET_V1, 10n);
  return body;
}

function candidateState(body: Uint8Array): Uint8Array {
  const output = new Uint8Array(Abi.GENERAL_LOCAL_STATE_HEADER_BYTES_V3 + body.length);
  output.set(Abi.GENERAL_LOCAL_STATE_MAGIC_V3); putU16(output, Abi.GENERAL_LOCAL_STATE_VERSION_OFFSET_V3, Abi.GENERAL_LOCAL_STATE_VERSION_V3);
  output[Abi.GENERAL_LOCAL_STATE_KIND_OFFSET_V3] = Abi.GENERAL_LOCAL_STATE_CANDIDATE_KIND_V3; output[Abi.GENERAL_LOCAL_STATE_BUMP_OFFSET_V3] = 7;
  putU64(output, Abi.GENERAL_LOCAL_STATE_RENT_PRINCIPAL_OFFSET_V3, 50n); output.set(key(63).toBytes(), Abi.GENERAL_LOCAL_STATE_BENEFICIARY_OFFSET_V3); output.set(body, Abi.GENERAL_LOCAL_STATE_BODY_OFFSET_V3);
  return output;
}

function lookupTableData(table: AddressLookupTableAccount): Uint8Array {
  const header = new Uint8Array(56); const view = new DataView(header.buffer);
  view.setUint32(0, 1, true); view.setBigUint64(4, table.state.deactivationSlot, true); view.setBigUint64(12, BigInt(table.state.lastExtendedSlot), true); header[20] = table.state.lastExtendedSlotStartIndex;
  if (table.state.authority !== undefined) { header[21] = 1; header.set(table.state.authority.toBytes(), 22); }
  const output = new Uint8Array(header.length + table.state.addresses.length * 32); output.set(header);
  table.state.addresses.forEach((address, index) => output.set(address.toBytes(), header.length + index * 32));
  return output;
}

function rpcAccount(data: Uint8Array, owner: string, executable = false): RpcAccount {
  return Object.freeze({ data, executable, lamports: '1000000', owner, space: data.length });
}

function verifier(outcomeCount: number, current = true): Uint8Array {
  const body = new Uint8Array(Abi.GENERAL_VERIFIER_HEADER_BYTES_V2 + outcomeCount * 5 * Abi.GENERAL_VERIFIER_TAIL_ITEM_STRIDE_V2);
  body.set(Abi.GENERAL_VERIFIER_MAGIC_V2); putU16(body, Abi.GENERAL_VERIFIER_VERSION_OFFSET_V2, Abi.GENERAL_VERIFIER_VERSION_V2); body[Abi.GENERAL_VERIFIER_HAS_CURRENT_ORDER_OFFSET_V2] = current ? 1 : 0;
  putU32(body, Abi.GENERAL_VERIFIER_OUTCOME_COUNT_OFFSET_V2, outcomeCount); putU32(body, Abi.GENERAL_VERIFIER_PAGE_COUNT_OFFSET_V2, 2); putU32(body, Abi.GENERAL_VERIFIER_NEXT_PAGE_INDEX_OFFSET_V2, current ? 1 : 0); putU32(body, Abi.GENERAL_VERIFIER_NEXT_ROW_INDEX_OFFSET_V2, current ? 1 : 0); putU32(body, Abi.GENERAL_VERIFIER_ORDER_COUNT_OFFSET_V2, current ? 1 : 0);
  putU64(body, Abi.GENERAL_VERIFIER_REVISION_OFFSET_V2, current ? 9n : 0n); putU32(body, Abi.GENERAL_VERIFIER_CANDIDATE_COORDINATE_OFFSET_V2, 7); body.set(bytes(65), Abi.GENERAL_VERIFIER_CANDIDATE_ID_OFFSET_V2); body.set(bytes(66), Abi.GENERAL_VERIFIER_PRODUCT_ID_OFFSET_V2); body.set(bytes(67), Abi.GENERAL_VERIFIER_BATCH_ID_OFFSET_V2);
  putU64(body, Abi.GENERAL_VERIFIER_PRICE_SCALE_OFFSET_V2, 100n); putU64(body, Abi.GENERAL_VERIFIER_FILLED_LOTS_OFFSET_V2, current ? 3n : 0n); putU64(body, Abi.GENERAL_VERIFIER_QUOTE_DEBIT_OFFSET_V2, current ? 15n : 0n); putU64(body, Abi.GENERAL_VERIFIER_QUOTE_CREDIT_OFFSET_V2, current ? 9n : 0n);
  putU64(body, Abi.GENERAL_VERIFIER_TAILS_BASE_OFFSET_V2, 100n);
  if (current) {
    body.set(bytes(68), Abi.GENERAL_VERIFIER_CURRENT_ORDER_ID_OFFSET_V2); body.set(key(69).toBytes(), Abi.GENERAL_VERIFIER_CURRENT_OWNER_ID_OFFSET_V2); putU64(body, Abi.GENERAL_VERIFIER_CURRENT_NONCE_OFFSET_V2, 3n); putU64(body, Abi.GENERAL_VERIFIER_CURRENT_MAX_LOTS_OFFSET_V2, 4n); putU64(body, Abi.GENERAL_VERIFIER_CURRENT_MAX_QUOTE_DEBIT_PER_LOT_OFFSET_V2, 5n); putU64(body, Abi.GENERAL_VERIFIER_CURRENT_LOTS_OFFSET_V2, 2n);
    const receive = Abi.GENERAL_VERIFIER_TAILS_BASE_OFFSET_V2 + outcomeCount * Abi.GENERAL_VERIFIER_TAIL_ITEM_STRIDE_V2; const deliver = receive + outcomeCount * Abi.GENERAL_VERIFIER_TAIL_ITEM_STRIDE_V2;
    putU64(body, receive, 2n); putU64(body, deliver, 1n);
  }
  return body;
}

function verifiedCandidate(outcomeCount: number): Uint8Array {
  const body = new Uint8Array(Abi.GENERAL_VERIFIED_CANDIDATE_HEADER_BYTES_V2 + outcomeCount * 2 * Abi.GENERAL_VERIFIED_CANDIDATE_TAIL_ITEM_STRIDE_V2);
  body.set(Abi.GENERAL_VERIFIED_CANDIDATE_MAGIC_V2); putU16(body, Abi.GENERAL_VERIFIED_CANDIDATE_VERSION_OFFSET_V2, Abi.GENERAL_VERIFIED_CANDIDATE_VERSION_V2); body[Abi.GENERAL_VERIFIED_CANDIDATE_PHASE_OFFSET_V2] = Abi.GENERAL_VERIFIED_CANDIDATE_PHASE_V2;
  putU32(body, Abi.GENERAL_VERIFIED_CANDIDATE_OUTCOME_COUNT_OFFSET_V2, outcomeCount); putU32(body, Abi.GENERAL_VERIFIED_CANDIDATE_PAGE_COUNT_OFFSET_V2, 2); putU32(body, Abi.GENERAL_VERIFIED_CANDIDATE_CANDIDATE_COORDINATE_OFFSET_V2, 7); putU64(body, Abi.GENERAL_VERIFIED_CANDIDATE_REVISION_OFFSET_V2, 9n);
  body.set(bytes(65), Abi.GENERAL_VERIFIED_CANDIDATE_CANDIDATE_ID_OFFSET_V2); body.set(bytes(66), Abi.GENERAL_VERIFIED_CANDIDATE_PRODUCT_ID_OFFSET_V2); body.set(bytes(67), Abi.GENERAL_VERIFIED_CANDIDATE_BATCH_ID_OFFSET_V2); putU64(body, Abi.GENERAL_VERIFIED_CANDIDATE_FILLED_LOTS_OFFSET_V2, 3n); putU64(body, Abi.GENERAL_VERIFIED_CANDIDATE_QUOTE_DEBIT_OFFSET_V2, 15n); putU64(body, Abi.GENERAL_VERIFIED_CANDIDATE_QUOTE_CREDIT_OFFSET_V2, 9n); putU64(body, Abi.GENERAL_VERIFIED_CANDIDATE_PRICE_SCALE_OFFSET_V2, 100n);
  putU64(body, Abi.GENERAL_VERIFIED_CANDIDATE_CLAIM_INPUTS_BASE_OFFSET_V2, 4n); putU64(body, Abi.GENERAL_VERIFIED_CANDIDATE_CLAIM_INPUTS_BASE_OFFSET_V2 + outcomeCount * Abi.GENERAL_VERIFIED_CANDIDATE_TAIL_ITEM_STRIDE_V2, 2n);
  return body;
}

describe('General V5 operator-plan browser boundary', () => {
  it('uses the Rust producer byte ceiling before parsing untrusted JSON', () => {
    expect(Abi.GENERAL_SUCCESSOR_PLAN_MAX_BYTES_V5).toBe(65_536);
    expect(() => decodeGeneralSuccessorPlanDocumentV5(' '.repeat(Abi.GENERAL_SUCCESSOR_PLAN_MAX_BYTES_V5 + 1))).toThrow(/byte bound/);
  });

  it('consumes every frozen unsigned operator action and remains runtime-width at N=1/N=258', async () => {
    for (const action of PLAN_ACTIONS) {
      const current = await fixture(action, action === 'distribute' ? 258 : 1); const inspection = await inspectGeneralSuccessorPlanV5(decodeGeneralSuccessorPlanDocumentV5(current.text));
      expect(inspection.request.action).toBe(action); expect(inspection.plan.outcomeCount).toBe(action === 'distribute' ? 258 : 1); expect(inspection.transaction.wireBytes).toBeLessThanOrEqual(1_232);
      expect(inspection.request.wire).toBe(ACTIONS.indexOf(action) >= Abi.ACTION_OPEN_BATCH_V3 ? 'v3' : 'v2');
    }
  });

  it('refuses stale/substituted reports, action geometry, and receipt order', async () => {
    const value = await fixture('initialize-settlement');
    expect(() => decodeGeneralSuccessorPlanDocumentV5(JSON.stringify({ ...value.raw, releaseSet: id(99), extra: true }))).toThrow(/extraneous/);
    const wrongAction = decodeGeneralSuccessorPlanDocumentV5(JSON.stringify({ ...value.raw, action: 'freeze', childRoutes: [] }));
    await expect(inspectGeneralSuccessorPlanV5(wrongAction)).rejects.toThrow(/differs/);
    const childRoutes = structuredClone(value.raw.childRoutes) as Array<Record<string, unknown>>;
    childRoutes[2] = { ...childRoutes[2], receiptDependencies: [{ producerRole: 'custody', producerRoute: 0, expectedReceiptBytes: Abi.GENERAL_CUSTODY_RECEIPT_BYTES_V1 }] };
    expect(() => decodeGeneralSuccessorPlanDocumentV5(JSON.stringify({ ...value.raw, childRoutes }))).toThrow(/dependency/);
    expect(() => decodeGeneralSuccessorPlanDocumentV5(JSON.stringify({ ...value.raw, heapFrameBytes: 32_768 }))).toThrow(/heap frame/);
    const substitutedHeap = VersionedTransaction.deserialize(Buffer.from(String(value.raw.transactionBase64), 'base64'));
    substitutedHeap.message.compiledInstructions[0].data.set(ComputeBudgetProgram.requestHeapFrame({ bytes: 32_768 }).data);
    const substitutedPacket = decodeGeneralSuccessorPlanDocumentV5(JSON.stringify({ ...value.raw, transactionBase64: base64(substitutedHeap.serialize()) }));
    await expect(inspectGeneralSuccessorPlanV5(substitutedPacket)).rejects.toThrow(/heap declaration/);
  });

  it('accepts only the released CloseCandidate topology', async () => {
    const value = await fixture('close-candidate');
    const plan = decodeGeneralSuccessorPlanDocumentV5(value.text);
    const inspection = await inspectGeneralSuccessorPlanV5(plan);
    expect(inspection.request).toMatchObject({ action: 'close-candidate', expectedRevision: 0n, subjectId: id(52) });
    expect(plan.lifecycle).toMatchObject({ childAccountStart: Abi.GENERAL_CLOSE_CANDIDATE_CHILD_START_V3, secondary: null, conditionalResult: null });
    const lifecycle = structuredClone(value.raw.lifecycle) as Record<string, unknown>;
    expect(() => decodeGeneralSuccessorPlanDocumentV5(JSON.stringify({ ...value.raw, lifecycle: { ...lifecycle, childAccountStart: Abi.GENERAL_CLOSE_CANDIDATE_BATCH_ACCOUNT_V3 } }))).toThrow(/lifecycle shape/);
  });

  it('reacquires CloseCandidate with its closed Batch and refuses early censorship', async () => {
    const value = await fixture('close-candidate');
    const inspection = await inspectGeneralSuccessorPlanV5(decodeGeneralSuccessorPlanDocumentV5(value.text));
    const batchState = localState('batch', 1); const body = Abi.GENERAL_LOCAL_STATE_BODY_OFFSET_V3;
    batchState[body + Abi.GENERAL_BATCH_STATUS_OFFSET_V1] = Abi.GENERAL_BATCH_STATUS_CLOSED_V1;
    putU64(batchState, body + Abi.GENERAL_BATCH_CLOSED_ROOT_REVISION_OFFSET_V1, 10n);
    const decodedBatch = decodeGeneralLocalStateV3(batchState);
    // `decodeGeneralLocalStateV3` never yields a vacant status -- only
    // `decodeStateAccount` adds that arm, for a funded System account -- so the
    // vacant disjunct this guard was copied with can never fire.
    if (decodedBatch.status.kind !== 'batch') throw new Error('test Batch did not decode');
    const batchId = hexBytes(await generalBatchOccurrenceIdentityV1(decodedBatch.status));
    const considered = candidateState(candidate(Abi.GENERAL_SUBMISSION_STATUS_CONSIDERED_V1, 1, bytes(52), batchId));
    const submitted = candidateState(candidate(Abi.GENERAL_SUBMISSION_STATUS_SUBMITTED_V1, 1, bytes(52), batchId));
    const candidateAddress = value.accounts[5]?.toBase58(); const batchAddress = value.accounts[8]?.toBase58();
    if (candidateAddress === undefined || batchAddress === undefined) throw new Error('test topology is incomplete');
    const trading = String(value.raw.tradingProgram); const lookup = value.table.key.toBase58();
    const client = (candidateBytes: Uint8Array, slot: string): SolanaRpcClient => ({
      finalizedSlot: async () => '77',
      multipleAccounts: async (addresses: ReadonlyArray<string>) => Object.freeze({
        slot,
        accounts: Object.freeze(addresses.map((address) => Object.freeze({
          address,
          account: address === lookup ? rpcAccount(lookupTableData(value.table), AddressLookupTableProgram.programId.toBase58())
            : address === candidateAddress ? rpcAccount(candidateBytes, trading)
              : address === batchAddress ? rpcAccount(batchState, trading)
                : rpcAccount(new Uint8Array(), key(240).toBase58(), address === trading || address === ComputeBudgetProgram.programId.toBase58()),
        }))),
      }),
    }) as unknown as SolanaRpcClient;

    const current = await reacquireGeneralSuccessorStatusV5(client(considered, '99'), inspection);
    expect(current.candidateClose).toMatchObject({ solver: key(63).toBase58(), closedBatchAccount: batchAddress, closedBatch: { phase: 'closed', settlementCloseSlot: 100n } });
    await expect(reacquireGeneralSuccessorStatusV5(client(candidateState(candidate(Abi.GENERAL_SUBMISSION_STATUS_CONSIDERED_V1, 1, bytes(52), bytes(91))), '99'), inspection)).rejects.toThrow(/does not join/);
    const underfunded = candidate(Abi.GENERAL_SUBMISSION_STATUS_CONSIDERED_V1, 1, bytes(52), batchId);
    putU64(underfunded, Abi.GENERAL_SUBMISSION_CLEANUP_REMAINING_OFFSET_V1, 9n);
    await expect(reacquireGeneralSuccessorStatusV5(client(candidateState(underfunded), '99'), inspection)).rejects.toThrow(/does not join/);
    await expect(reacquireGeneralSuccessorStatusV5(client(submitted, '99'), inspection)).rejects.toThrow(/censor.*before.*deadline/i);
    const expired = await reacquireGeneralSuccessorStatusV5(client(submitted, '100'), inspection);
    expect(expired.candidateClose?.closedBatch.phase).toBe('closed');
  });

  it('keeps manifest ordinal distinct from source page/execution coordinates', async () => {
    const value = await fixture('collect'); const inspection = await inspectGeneralSuccessorPlanV5(decodeGeneralSuccessorPlanDocumentV5(value.text));
    expect(inspection.request).toMatchObject({ manifestOrderIndex: 1, pageIndex: 2, executionIndex: 3 });
    const transaction = VersionedTransaction.deserialize(base64ToBytes(value.raw.transactionBase64 as string));
    transaction.message.compiledInstructions[1].data[11 + Abi.GENERAL_HOT_ENVELOPE_BYTES_V3] = 0;
    const hostile = { ...value.raw, transactionBase64: base64(transaction.serialize()) };
    await expect(inspectGeneralSuccessorPlanV5(decodeGeneralSuccessorPlanDocumentV5(JSON.stringify(hostile)))).rejects.toThrow(/differs/);
  });

  it('hostile-decodes exact local selection and runtime-width settlement state', () => {
    expect(decodeGeneralLocalStateV3(localState('selection', 1)).status).toMatchObject({ kind: 'selection', phase: 'open', revision: 9n, bestCandidateId: id(4) });
    const settlement = decodeGeneralLocalStateV3(localState('settlement', 258)).status;
    expect(settlement).toMatchObject({ kind: 'settlement', phase: 'collecting', outcomeCount: 258, nextOrder: 1, revision: 9n });
    const padded = localState('selection', 1); padded[12] = 1; expect(() => decodeGeneralLocalStateV3(padded)).toThrow(/reserved/);
    const truncated = localState('settlement', 258).slice(0, -8); expect(() => decodeGeneralLocalStateV3(truncated)).toThrow(/runtime width/);
  });

  it('hostile-decodes content-addressed Batch and mutable-window-masked Order state', () => {
    const batch = decodeGeneralLocalStateV3(localState('batch', 258)).status;
    expect(batch).toMatchObject({ kind: 'batch', phase: 'collecting', outcomeCount: 258, generation: 7n, orderCount: 1, committedQuoteReserve: 10n });
    const order = decodeGeneralLocalStateV3(localState('order', 258)).status;
    expect(order).toMatchObject({ kind: 'order', phase: 'placed', outcomeCount: 258, generation: 7n, maxLots: 2n, admittedSlot: 50n });
    if (order.kind !== 'order') throw new Error('fixture did not decode as an order');
    expect(order.receivePerLot[257]).toBe(258n);

    const badBatchPadding = localState('batch', 1); badBatchPadding[Abi.GENERAL_LOCAL_STATE_BODY_OFFSET_V3 + 161] = 1;
    expect(() => decodeGeneralLocalStateV3(badBatchPadding)).toThrow(/batch/);
    const impossibleClosedBatch = localState('batch', 1); impossibleClosedBatch[Abi.GENERAL_LOCAL_STATE_BODY_OFFSET_V3 + Abi.GENERAL_BATCH_STATUS_OFFSET_V1] = Abi.GENERAL_BATCH_STATUS_CLOSED_V1;
    expect(() => decodeGeneralLocalStateV3(impossibleClosedBatch)).toThrow(/lifecycle/);
    const zeroMovement = localState('order', 1); zeroMovement.fill(0, Abi.GENERAL_LOCAL_STATE_BODY_OFFSET_V3 + Abi.GENERAL_ORDER_ROW_BASE_V1);
    expect(() => decodeGeneralLocalStateV3(zeroMovement)).toThrow(/claim movement/);
    const badOrderPadding = localState('order', 1); badOrderPadding[Abi.GENERAL_LOCAL_STATE_BODY_OFFSET_V3 + Abi.GENERAL_ORDER_STATE_OFFSET_V1 + 1] = 1;
    expect(() => decodeGeneralLocalStateV3(badOrderPadding)).toThrow(/order/);
  });

  it('hostile-decodes exact Candidate V1 facts and reserves', () => {
    expect(decodeGeneralCandidateV1(candidate())).toMatchObject({ kind: 'candidate', phase: 'submitted', outcomeCount: 258, pageCount: 2, pageRevision: 9n, rowCount: 4, rewardRateLamports: 10n, verificationRemaining: 40n });
    expect(decodeGeneralCandidateV1(candidate(Abi.GENERAL_SUBMISSION_STATUS_VERIFIED_V1))).toMatchObject({ phase: 'verified', verifiedRevision: 8n, verifiedDigest: id(64) });
    const badMagic = candidate(); badMagic[Abi.GENERAL_SUBMISSION_MAGIC_OFFSET_V1] ^= 1; expect(() => decodeGeneralCandidateV1(badMagic)).toThrow(/exact V1/);
    const badVersion = candidate(); putU16(badVersion, Abi.GENERAL_SUBMISSION_VERSION_OFFSET_V1, 2); expect(() => decodeGeneralCandidateV1(badVersion)).toThrow(/exact V1/);
    const badReserved = candidate(); badReserved[Abi.GENERAL_SUBMISSION_TAIL_RESERVED_OFFSET_V1] = 1; expect(() => decodeGeneralCandidateV1(badReserved)).toThrow(/tail/);
    expect(() => decodeGeneralCandidateV1(candidate().slice(0, -1))).toThrow(/exact V1/);
  });

  it('hostile-decodes runtime-width Verifier V2 cursor, simplex, and current-order states', () => {
    const one = decodeGeneralVerifierV2(verifier(1)); expect(one).toMatchObject({ kind: 'verifier', phase: 'streaming', outcomeCount: 1, nextPageIndex: 1, nextRowIndex: 1, orderCount: 1, revision: 9n, priceScale: 100n }); expect(one.currentOrder).toMatchObject({ maxLots: 4n, lots: 2n, receivePerLot: [2n], deliverPerLot: [1n] });
    const wide = decodeGeneralVerifierV2(verifier(258)); expect(wide.prices).toHaveLength(258); expect(wide.prices[0]).toBe(100n);
    const initial = decodeGeneralVerifierV2(verifier(1, false)); expect(initial).toMatchObject({ phase: 'initial', currentOrder: null });
    const badMagic = verifier(1); badMagic[0] ^= 1; expect(() => decodeGeneralVerifierV2(badMagic)).toThrow(/exact V2/);
    const badVersion = verifier(1); putU16(badVersion, Abi.GENERAL_VERIFIER_VERSION_OFFSET_V2, 1); expect(() => decodeGeneralVerifierV2(badVersion)).toThrow(/exact V2/);
    const badReserved = verifier(1); badReserved[11] = 1; expect(() => decodeGeneralVerifierV2(badReserved)).toThrow(/reserved/);
    expect(() => decodeGeneralVerifierV2(verifier(258).slice(0, -8))).toThrow(/runtime width/);
    const badCursor = verifier(1); putU32(badCursor, Abi.GENERAL_VERIFIER_NEXT_PAGE_INDEX_OFFSET_V2, 2); putU32(badCursor, Abi.GENERAL_VERIFIER_NEXT_ROW_INDEX_OFFSET_V2, 1); expect(() => decodeGeneralVerifierV2(badCursor)).toThrow(/cursor/);
    const badSimplex = verifier(1); putU64(badSimplex, Abi.GENERAL_VERIFIER_TAILS_BASE_OFFSET_V2, 99n); expect(() => decodeGeneralVerifierV2(badSimplex)).toThrow(/cursor/);
    const absentPayload = verifier(1, false); putU64(absentPayload, Abi.GENERAL_VERIFIER_CURRENT_ORDER_ID_OFFSET_V2, 1n); expect(() => decodeGeneralVerifierV2(absentPayload)).toThrow(/absent current order/);
  });

  it('hostile-decodes runtime-width terminal VerifiedCandidate V2 facts', () => {
    const one = decodeGeneralVerifiedCandidateV2(verifiedCandidate(1)); expect(one).toMatchObject({ kind: 'verified-candidate', outcomeCount: 1, pageCount: 2, candidateCoordinate: 7, revision: 9n, filledLots: 3n, priceScale: 100n, claimInputs: [4n], claimOutputs: [2n] });
    const wide = decodeGeneralVerifiedCandidateV2(verifiedCandidate(258)); expect(wide.claimInputs).toHaveLength(258); expect(wide.claimOutputs[0]).toBe(2n);
    const badMagic = verifiedCandidate(1); badMagic[0] ^= 1; expect(() => decodeGeneralVerifiedCandidateV2(badMagic)).toThrow(/exact V2/);
    const badVersion = verifiedCandidate(1); putU16(badVersion, Abi.GENERAL_VERIFIED_CANDIDATE_VERSION_OFFSET_V2, 1); expect(() => decodeGeneralVerifiedCandidateV2(badVersion)).toThrow(/exact V2/);
    const badReserved = verifiedCandidate(1); badReserved[11] = 1; expect(() => decodeGeneralVerifiedCandidateV2(badReserved)).toThrow(/reserved/);
    expect(() => decodeGeneralVerifiedCandidateV2(verifiedCandidate(258).slice(0, -8))).toThrow(/runtime width/);
  });

  it('derives Batch occurrence identity from durable terms, never runtime-owned deadlines', async () => {
    const originalWire = localState('batch', 3);
    const original = decodeGeneralLocalStateV3(originalWire).status;
    if (original.kind !== 'batch') throw new Error('fixture did not decode as a batch');
    const originalIdentity = await generalBatchOccurrenceIdentityV1(original);

    const laterWindowWire = originalWire.slice();
    putU64(laterWindowWire, Abi.GENERAL_LOCAL_STATE_BODY_OFFSET_V3 + Abi.GENERAL_BATCH_COLLECTION_CLOSE_SLOT_OFFSET_V1, 90n);
    putU64(laterWindowWire, Abi.GENERAL_LOCAL_STATE_BODY_OFFSET_V3 + Abi.GENERAL_BATCH_SETTLEMENT_CLOSE_SLOT_OFFSET_V1, 120n);
    const laterWindow = decodeGeneralLocalStateV3(laterWindowWire).status;
    if (laterWindow.kind !== 'batch') throw new Error('fixture did not decode as a batch');
    expect(await generalBatchOccurrenceIdentityV1(laterWindow)).toBe(originalIdentity);

    const nextSequenceWire = originalWire.slice();
    putU64(nextSequenceWire, Abi.GENERAL_LOCAL_STATE_BODY_OFFSET_V3 + Abi.GENERAL_BATCH_SEQUENCE_OFFSET_V1, 3n);
    const nextSequence = decodeGeneralLocalStateV3(nextSequenceWire).status;
    if (nextSequence.kind !== 'batch') throw new Error('fixture did not decode as a batch');
    expect(await generalBatchOccurrenceIdentityV1(nextSequence)).not.toBe(originalIdentity);
  });

  it('reads the envelope bump-hint tail instead of demanding the retired reserved zeros', async () => {
    // `d0306a64` gave the last eight envelope bytes to `HotBumpHintsV1`, and
    // this decoder kept refusing them as required-zero reserved space -- the
    // same shape of defect HINTS-TS removed from the Direct evidence encoder,
    // where a legally mined wire failed its own authenticator.
    expect(Abi.GENERAL_ENVELOPE_BUMP_HINTS_OFFSET_V3 + Abi.GENERAL_ENVELOPE_BUMP_HINT_COUNT_V3).toBe(Abi.GENERAL_HOT_ENVELOPE_BYTES_V3);
    const value = await fixture('freeze');
    const absent = await inspectGeneralSuccessorPlanV5(decodeGeneralSuccessorPlanDocumentV5(value.text));
    expect(absent.envelope.bumpHints).toEqual([0, 0, 0, 0, 0, 0, 0, 0]);
    const transaction = VersionedTransaction.deserialize(base64ToBytes(value.raw.transactionBase64 as string));
    const mined = [254, 253, 252, 0, 251, 0, 250, 0];
    mined.forEach((bump, slot) => { transaction.message.compiledInstructions[1].data[Abi.GENERAL_ENVELOPE_BUMP_HINTS_OFFSET_V3 + slot] = bump; });
    const hinted = await inspectGeneralSuccessorPlanV5(decodeGeneralSuccessorPlanDocumentV5(JSON.stringify({ ...value.raw, transactionBase64: base64(transaction.serialize()) })));
    expect(hinted.envelope.bumpHints).toEqual(mined);
    expect(hinted.plan.familyRequestDigest).toBe(absent.plan.familyRequestDigest);
  });

  it('joins a commit-last Hot receipt to the exact request and selected descriptor', async () => {
    const value = await fixture('close'); const inspection = await inspectGeneralSuccessorPlanV5(decodeGeneralSuccessorPlanDocumentV5(value.text));
    const receipt = new Uint8Array(Abi.GENERAL_HOT_ACK_BYTES_V3); receipt.set(Abi.GENERAL_HOT_ACK_MAGIC_V3); putU16(receipt, 8, 3); putU16(receipt, 10, 1);
    receipt.set(bytes(41), Abi.GENERAL_ACK_RELEASE_SET_OFFSET_V3); receipt.set(key(1).toBytes(), Abi.GENERAL_ACK_MARKET_OFFSET_V3); putU64(receipt, Abi.GENERAL_ACK_GENERATION_OFFSET_V3, 7n); receipt.set(key(2).toBytes(), Abi.GENERAL_ACK_ROOT_OFFSET_V3);
    receipt.set(await sha256(value.request), Abi.GENERAL_ACK_REQUEST_DIGEST_OFFSET_V3); receipt.set(bytes(81), Abi.GENERAL_ACK_SELECTED_PROGRAM_OFFSET_V3); receipt.set(bytes(42), Abi.GENERAL_ACK_ROOT_PRESTATE_DIGEST_OFFSET_V3); receipt.set(bytes(61), Abi.GENERAL_ACK_ROOT_POSTSTATE_DIGEST_OFFSET_V3); receipt.set(bytes(62), Abi.GENERAL_ACK_EXECUTION_DIGEST_OFFSET_V3);
    expect(decodeGeneralHotReceiptV3(base64(receipt), inspection)).toMatchObject({ requestDigest: inspection.plan.familyRequestDigest, selectedProgram: inspection.plan.artifacts.descriptor });
    receipt[Abi.GENERAL_ACK_REQUEST_DIGEST_OFFSET_V3] ^= 1; expect(() => decodeGeneralHotReceiptV3(base64(receipt), inspection)).toThrow(/another request/);
  });
});

function base64ToBytes(value: string): Uint8Array { return Uint8Array.from(Buffer.from(value, 'base64')); }
