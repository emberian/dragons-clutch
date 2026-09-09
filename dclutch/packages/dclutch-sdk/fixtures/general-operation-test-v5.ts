/** Test-only packet derived from the archived native CLI output; identities/heap/root digest are replaced explicitly. */
import { ComputeBudgetProgram, Keypair, PublicKey, VersionedTransaction } from '@solana/web3.js';
import archived from './general-successor-plan-v5.devnet.json';
import { fromHex, hex, sha256 } from '../lib/bytes';
import * as Abi from '../lib/generated/generalSuccessorV5';
import { decodeGeneralSuccessorPlanDocumentV5, inspectGeneralSuccessorPlanV5 } from '../lib/generalPlanV5';
import { generalAccountCommitmentV5 } from '../lib/generalExecutionV5';
import { type RpcAccount } from '../lib/rpc';

export async function generalOperationTestFixtureV5() {
  const payer = Keypair.fromSeed(new Uint8Array(32).fill(17));
  const transaction = VersionedTransaction.deserialize(Buffer.from(archived.transactionBase64, 'base64'));
  transaction.message.staticAccountKeys[0] = payer.publicKey;
  transaction.message.compiledInstructions[1]!.data = ComputeBudgetProgram.requestHeapFrame({ bytes: Abi.GENERAL_HOT_HEAP_FRAME_BYTES_V3 }).data;
  const root: RpcAccount = { data: new Uint8Array([1, 2, 3]), owner: archived.tradingProgram, executable: false, lamports: '100', space: 3 };
  const rootDigest = await sha256(root.data);
  transaction.message.compiledInstructions[2]!.data.set(rootDigest, Abi.GENERAL_ENVELOPE_ROOT_PRESTATE_DIGEST_OFFSET_V3);
  const nativePlan = JSON.stringify({ ...archived, payer: payer.publicKey.toBase58(), requiredSigners: [payer.publicKey.toBase58()], heapFrameBytes: Abi.GENERAL_HOT_HEAP_FRAME_BYTES_V3, rootPrestateDigest: hex(rootDigest), transactionBase64: Buffer.from(transaction.serialize()).toString('base64') });
  const inspection = await inspectGeneralSuccessorPlanV5(decodeGeneralSuccessorPlanDocumentV5(nativePlan));
  const preview = { messageDigest: hex(await sha256(transaction.message.serialize())), observedSlot: archived.observedSlot, computeUnits: '42', rootPoststateDigest: 'ab'.repeat(32), accounts: [{ address: archived.root, beforeCommitment: await generalAccountCommitmentV5(root), beforeLamports: '100', afterLamports: '100', beforeToken: null, afterToken: null, beforeClaims: null, afterClaims: null }] };
  return { payer, nativePlan, inspection, preview, root, transaction };
}

export async function generalOperationReceiptTestFixtureV5(fixture: Awaited<ReturnType<typeof generalOperationTestFixtureV5>>) {
  const { inspection } = fixture; const plan = inspection.plan;
  const ack = new Uint8Array(Abi.GENERAL_HOT_ACK_BYTES_V3); ack.set(Abi.GENERAL_HOT_ACK_MAGIC_V3);
  const view = new DataView(ack.buffer); view.setUint16(8, 3, true); view.setUint16(10, Abi.GENERAL_HOT_PROFILE_V3, true); view.setBigUint64(Abi.GENERAL_ACK_GENERATION_OFFSET_V3, plan.generation, true);
  for (const [offset, value] of [
    [Abi.GENERAL_ACK_RELEASE_SET_OFFSET_V3, fromHex(plan.releaseSet, 'release')],
    [Abi.GENERAL_ACK_MARKET_OFFSET_V3, new PublicKey(plan.market).toBytes()],
    [Abi.GENERAL_ACK_ROOT_OFFSET_V3, new PublicKey(plan.root).toBytes()],
    [Abi.GENERAL_ACK_REQUEST_DIGEST_OFFSET_V3, fromHex(inspection.ackRequestDigest, 'request')],
    [Abi.GENERAL_ACK_SELECTED_PROGRAM_OFFSET_V3, fromHex(plan.artifacts.descriptor, 'descriptor')],
    [Abi.GENERAL_ACK_ROOT_PRESTATE_DIGEST_OFFSET_V3, fromHex(plan.rootPrestateDigest, 'prestate')],
    [Abi.GENERAL_ACK_ROOT_POSTSTATE_DIGEST_OFFSET_V3, await sha256(fixture.root.data)],
    [Abi.GENERAL_ACK_EXECUTION_DIGEST_OFFSET_V3, new Uint8Array(32).fill(21)],
  ] as const) ack.set(value, offset);
  return ack;
}
