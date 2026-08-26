import {
  AddressLookupTableAccount,
  PublicKey,
  TransactionInstruction,
  TransactionMessage,
  VersionedTransaction,
} from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import { hex, sha256 } from './bytes';
import * as Abi from './generated/generalSuccessorV5';
import {
  decodeGeneralHotReceiptV3,
  decodeGeneralLocalStateV3,
  decodeGeneralSuccessorPlanDocumentV5,
  inspectGeneralSuccessorPlanV5,
  type GeneralSuccessorActionV5,
} from './generalPlanV5';

const ACTIONS = ['consider', 'freeze', 'initialize-settlement', 'collect', 'materialize', 'distribute', 'close'] as const;
const MAX_U64 = 18_446_744_073_709_551_615n;

function bytes(value: number): Uint8Array { return new Uint8Array(32).fill(value); }
function id(value: number): string { return hex(bytes(value)); }
function key(value: number): PublicKey { const output = bytes((value % 250) + 1); output[0] = value; return new PublicKey(output); }
function base64(value: Uint8Array): string { return Buffer.from(value).toString('base64'); }
function putU16(output: Uint8Array, offset: number, value: number): void { new DataView(output.buffer, output.byteOffset + offset, 2).setUint16(0, value, true); }
function putU32(output: Uint8Array, offset: number, value: number): void { new DataView(output.buffer, output.byteOffset + offset, 4).setUint32(0, value, true); }
function putU64(output: Uint8Array, offset: number, value: bigint): void { new DataView(output.buffer, output.byteOffset + offset, 8).setBigUint64(0, value, true); }

function request(action: GeneralSuccessorActionV5): Uint8Array {
  const tag = ACTIONS.indexOf(action); const output = new Uint8Array(Abi.GENERAL_REQUEST_BYTES_V2);
  output.set(Abi.GENERAL_REQUEST_MAGIC_V2); putU16(output, 8, 2); output[10] = tag;
  const row = action === 'collect' || action === 'distribute';
  output[11] = row ? 1 : 0; putU64(output, 16, action === 'initialize-settlement' ? 0n : 9n);
  if (action !== 'freeze') output.set(bytes(52), 24);
  putU32(output, 56, action === 'consider' || row ? 2 : 0); output[60] = row ? 3 : 0; output[61] = 7; output[62] = action === 'close' ? 8 : 0;
  return output;
}

function routes(action: GeneralSuccessorActionV5): unknown[] {
  const roles = action === 'initialize-settlement' ? ['claims', 'custody', 'custody']
    : action === 'collect' || action === 'materialize' || action === 'distribute' ? ['claims', 'custody']
      : action === 'close' ? ['custody', 'claims', 'custody', 'custody'] : [];
  let start = action === 'close' ? 9 : 8;
  return roles.map((role, route) => {
    const receiptDependencies = action === 'initialize-settlement' && route === 2
      ? [{ producerRole: 'custody', producerRoute: 1, expectedReceiptBytes: Abi.GENERAL_CUSTODY_RECEIPT_BYTES_V1 }]
      : action === 'close' && route === 3
        ? [{ producerRole: 'custody', producerRoute: 2, expectedReceiptBytes: Abi.GENERAL_CUSTODY_RECEIPT_BYTES_V1 }] : [];
    const output = { route, role, accountStart: start, accountCount: 2, receiptDependencies };
    start += 2; return output;
  });
}

async function fixture(action: GeneralSuccessorActionV5, outcomeCount = 1): Promise<{ text: string; raw: Record<string, unknown>; request: Uint8Array }> {
  const trading = key(201); const payer = key(202); const lookup = key(203); const market = key(1); const root = key(2); const primary = key(70); const terminal = key(71);
  const requestBytes = request(action); const envelope = new Uint8Array(Abi.GENERAL_HOT_ENVELOPE_BYTES_V3);
  envelope.set(Abi.GENERAL_HOT_MAGIC_V3); putU16(envelope, 8, Abi.GENERAL_HOT_VERSION_V3); putU16(envelope, 10, Abi.GENERAL_HOT_PROFILE_V3);
  putU32(envelope, Abi.GENERAL_ENVELOPE_REQUEST_BYTES_OFFSET_V3, Abi.GENERAL_REQUEST_BYTES_V2); envelope.set(bytes(41), Abi.GENERAL_ENVELOPE_RELEASE_SET_OFFSET_V3);
  envelope.set(market.toBytes(), Abi.GENERAL_ENVELOPE_MARKET_OFFSET_V3); putU64(envelope, Abi.GENERAL_ENVELOPE_GENERATION_OFFSET_V3, 7n); envelope.set(bytes(42), Abi.GENERAL_ENVELOPE_ROOT_PRESTATE_DIGEST_OFFSET_V3);
  const data = new Uint8Array(envelope.length + requestBytes.length); data.set(envelope); data.set(requestBytes, envelope.length);
  const accounts = Array.from({ length: 52 }, (_, index) => key(index + 3)); accounts[0] = market; accounts[1] = root; accounts[46] = payer; accounts[47] = primary; accounts[48] = terminal;
  const metas = accounts.map((pubkey, index) => ({ pubkey, isSigner: index === 46, isWritable: index === 46 || index === 47 || index === 48 }));
  const addresses = accounts.filter((_, index) => index !== 46).sort((left, right) => Buffer.compare(left.toBytes(), right.toBytes()));
  const table = new AddressLookupTableAccount({ key: lookup, state: { deactivationSlot: MAX_U64, lastExtendedSlot: 1, lastExtendedSlotStartIndex: 0, authority: undefined, addresses } });
  const instruction = new TransactionInstruction({ programId: trading, keys: metas, data });
  const transaction = new VersionedTransaction(new TransactionMessage({ payerKey: payer, recentBlockhash: key(204).toBase58(), instructions: [instruction] }).compileToV0Message([table]));
  const artifactNames = ['programSet', 'descriptor', 'config', 'accountProfile', 'lifecyclePolicy', 'requestProfile', 'strategy', 'certificate', 'admission', 'transition', 'effect'];
  const raw: Record<string, unknown> = {
    format: 'dclutch/general-successor-plan/v5', action, transactionBase64: base64(transaction.serialize()), observedSlot: '77', outcomeCount, scratchPageCount: 1,
    tradingProgram: trading.toBase58(), lookupTable: lookup.toBase58(), payer: payer.toBase58(), requiredSigners: [payer.toBase58()], market: market.toBase58(), root: root.toBase58(), generation: '7',
    releaseSet: id(41), rootPrestateDigest: id(42), familyRequestDigest: hex(await sha256(requestBytes)), checkedManifestDigest: id(43), tradingArtifactRelease: id(44), generalArtifactRelease: id(45), productRecord: id(46),
    artifacts: Object.fromEntries(artifactNames.map((name, index) => [name, id(80 + index)])),
    lifecycle: { primaryState: primary.toBase58(), primaryStateBump: 7, terminalState: action === 'close' ? terminal.toBase58() : null, terminalStateBump: action === 'close' ? 8 : null, terminalCoordinate: action === 'close' ? '10' : null, childAccountStart: action === 'close' ? 9 : 8 },
    childRoutes: routes(action),
  };
  return { text: JSON.stringify(raw), raw, request: requestBytes };
}

function localState(kind: 'selection' | 'settlement', outcomeCount: number): Uint8Array {
  const body = kind === 'selection' ? new Uint8Array(Abi.GENERAL_SELECTION_BYTES_V2) : new Uint8Array(Abi.GENERAL_SETTLEMENT_HEADER_BYTES_V2 + outcomeCount * 8);
  if (kind === 'selection') {
    body.set(Abi.GENERAL_SELECTION_MAGIC_V2); putU16(body, 8, 2); body[10] = 1; putU32(body, 12, outcomeCount); putU64(body, 16, 9n); putU32(body, 24, 2); putU32(body, 28, 1); putU64(body, 32, 4n); putU64(body, 40, 100n);
    for (const [offset, value] of [[48, 1], [80, 2], [112, 3], [144, 4], [176, 5]] as const) body.set(bytes(value), offset);
    putU64(body, 208, 12n); putU64(body, 216, 3n);
  } else {
    body.set(Abi.GENERAL_SETTLEMENT_MAGIC_V2); putU16(body, 8, 2); body[10] = Abi.GENERAL_PHASE_COLLECTING_V2; putU32(body, 12, outcomeCount); putU32(body, 16, 2); putU32(body, 20, 1); putU64(body, 24, 9n); body.set(bytes(52), 32); putU64(body, 64, 7n);
    for (let index = 0; index < outcomeCount; index += 1) putU64(body, 88 + index * 8, BigInt(index));
  }
  const output = new Uint8Array(Abi.GENERAL_LOCAL_STATE_HEADER_BYTES_V3 + body.length); output.set(Abi.GENERAL_LOCAL_STATE_MAGIC_V3); putU16(output, 8, Abi.GENERAL_LOCAL_STATE_VERSION_V3);
  output[10] = kind === 'selection' ? Abi.GENERAL_LOCAL_STATE_SELECTION_KIND_V3 : Abi.GENERAL_LOCAL_STATE_SETTLEMENT_KIND_V3; output[11] = 7; putU64(output, 16, 50n); output.set(key(99).toBytes(), 24); output.set(body, 64); return output;
}

describe('General V5 operator-plan browser boundary', () => {
  it('consumes every frozen unsigned operator action and remains runtime-width at N=1/N=258', async () => {
    for (const action of ACTIONS) {
      const current = await fixture(action, action === 'distribute' ? 258 : 1); const inspection = await inspectGeneralSuccessorPlanV5(decodeGeneralSuccessorPlanDocumentV5(current.text));
      expect(inspection.request.action).toBe(action); expect(inspection.plan.outcomeCount).toBe(action === 'distribute' ? 258 : 1); expect(inspection.transaction.wireBytes).toBeLessThanOrEqual(1_232);
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
  });

  it('keeps manifest ordinal distinct from source page/execution coordinates', async () => {
    const value = await fixture('collect'); const inspection = await inspectGeneralSuccessorPlanV5(decodeGeneralSuccessorPlanDocumentV5(value.text));
    expect(inspection.request).toMatchObject({ manifestOrderIndex: 1, pageIndex: 2, executionIndex: 3 });
    const transaction = VersionedTransaction.deserialize(base64ToBytes(value.raw.transactionBase64 as string));
    transaction.message.compiledInstructions[0].data[11 + Abi.GENERAL_HOT_ENVELOPE_BYTES_V3] = 0;
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
