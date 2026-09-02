import { PublicKey, TransactionInstruction, TransactionMessage, VersionedTransaction } from '@solana/web3.js';

import { SOLANA_PACKET_BYTES_V1 } from './solanaLimits';
import {
  acquireUserPositionAdmissionSnapshotV1,
  type UserPositionAdmissionDerivedV1,
  type UserPositionAdmissionRequestV1,
} from './userPositionAdmissionSnapshot';
import {
  loadUserPositionAdmissionWasmV1,
  parseUserPositionAdmissionPlanV1,
  type UserPositionAdmissionPlanV1,
} from './userPositionAdmissionV1';
import { type SolanaRpcClient } from './rpc';

/**
 * The step between the planner's answer and a wallet signature.
 *
 * The compiled Rust planner returns ORDERED INSTRUCTIONS — zero to two rent
 * transfers, then the Trading outer — and says why they belong together: any
 * Claims refusal must roll the whole admission back, so they are one
 * transaction or they are wrong. A wallet signs a transaction, so exactly one
 * thing has to happen here, and this file does only that.
 *
 * The single judgement it makes is refusing a packet that cannot fly. The
 * outer frame is twenty-seven accounts before the two transfers are added, so
 * an oversize is a real outcome and not a theoretical one. Saying so by name,
 * with both numbers, beats handing a wallet bytes that fail opaquely at
 * submission — and it is the honest place to notice, because nothing
 * downstream can tell an oversize packet from a rejected one.
 */

export type CompiledAdmissionTransactionV1 = Readonly<{
  transaction: VersionedTransaction;
  wireBytes: Uint8Array;
  requiredSigners: ReadonlyArray<string>;
  plan: UserPositionAdmissionPlanV1;
}>;

function key(value: string, field: string): PublicKey {
  try { return new PublicKey(value); } catch { throw new Error(`${field} is not a base58 public key`); }
}

/** Compile the planner's instructions into one v0 transaction. */
export function compileUserPositionAdmissionTransactionV1(
  plan: UserPositionAdmissionPlanV1,
  input: Readonly<{ payer: string; recentBlockhash: string }>,
): CompiledAdmissionTransactionV1 {
  const payer = key(input.payer, 'admission payer');
  if (payer.toBase58() !== plan.requiredSigner) {
    // The planner authenticated exactly one signer against finalized state. A
    // transaction paid by anybody else is a different transaction.
    throw new Error('admission payer is not the planner’s required signer');
  }
  if (plan.instructions.length === 0) throw new Error('the admission plan carries no instructions');
  const instructions = plan.instructions.map((one) => new TransactionInstruction({
    programId: key(one.programId, 'admission instruction program'),
    keys: one.accounts.map((meta) => ({
      pubkey: key(meta.pubkey, 'admission account'),
      isSigner: meta.isSigner,
      isWritable: meta.isWritable,
    })),
    data: Buffer.from(Uint8Array.from(atob(one.dataBase64), (one2) => one2.charCodeAt(0))),
  }));
  const transaction = new VersionedTransaction(new TransactionMessage({
    payerKey: payer,
    recentBlockhash: key(input.recentBlockhash, 'recent blockhash').toBase58(),
    instructions,
  }).compileToV0Message());
  // Past the packet bound web3.js throws its own `encoding overruns
  // Uint8Array` from inside the serializer, and it will not produce the byte
  // count on the way out. So the refusal reports what IS knowable and
  // actionable — how many distinct accounts the frame reached, against the
  // bound it broke. "Encoding overruns" is not a fact about anybody's market.
  let wireBytes: Uint8Array;
  try {
    wireBytes = transaction.serialize();
  } catch {
    const distinct = new Set(instructions.flatMap((one) => [one.programId.toBase58(), ...one.keys.map((meta) => meta.pubkey.toBase58())])).size;
    throw new Error(`admission transaction does not fit Solana’s ${SOLANA_PACKET_BYTES_V1.toLocaleString('en-US')}-byte packet bound: the frame reached ${distinct} distinct accounts`);
  }
  if (wireBytes.length > SOLANA_PACKET_BYTES_V1) {
    throw new Error(`admission transaction is ${wireBytes.length} bytes, above Solana’s ${SOLANA_PACKET_BYTES_V1.toLocaleString('en-US')}-byte packet bound`);
  }
  const requiredSigners = Object.freeze(transaction.message.staticAccountKeys
    .slice(0, transaction.message.header.numRequiredSignatures)
    .map((address) => address.toBase58()));
  if (requiredSigners.length !== 1 || requiredSigners[0] !== plan.requiredSigner) {
    throw new Error('the compiled admission message has another signer set than the planner named');
  }
  return Object.freeze({ transaction, wireBytes, requiredSigners, plan });
}

export type PreparedAdmissionV1 = CompiledAdmissionTransactionV1 & Readonly<{
  derived: UserPositionAdmissionDerivedV1;
  observedSlot: string;
}>;

/**
 * Acquire, plan, and compile — the whole browser path to an unsigned admission.
 *
 * Nothing here is authored: the snapshot is derived and read at one finalized
 * floor, the plan is the compiled Rust owner's, and the only local step is
 * putting its instructions into one message for the wallet.
 */
export async function prepareUserPositionAdmissionV1(
  client: Pick<SolanaRpcClient, 'finalizedSlot' | 'probe' | 'blockTime' | 'multipleAccounts' | 'multipleAccountDataSlices' | 'latestBlockhash'>,
  request: UserPositionAdmissionRequestV1,
  loadPlanner: typeof loadUserPositionAdmissionWasmV1 = loadUserPositionAdmissionWasmV1,
): Promise<PreparedAdmissionV1> {
  // The planner is loaded FIRST now: the acquisition needs it to decode the
  // admission record, which is the only account on chain that names the
  // linked-basis record digest.
  const planner = await loadPlanner();
  const acquired = await acquireUserPositionAdmissionSnapshotV1(client, request, planner);
  let planJson: string;
  try {
    planJson = planner.plan_user_position_admission_v1_wasm(acquired.snapshotJson);
  } catch (error) {
    // The planner's own refusal, unchanged. This boundary invents no reason.
    throw new Error(typeof error === 'string' ? error : error instanceof Error ? error.message : 'the admission planner refused without a usable reason');
  }
  const plan = parseUserPositionAdmissionPlanV1(planJson);
  const blockhash = await client.latestBlockhash();
  const compiled = compileUserPositionAdmissionTransactionV1(plan, {
    payer: plan.requiredSigner,
    recentBlockhash: blockhash.blockhash,
  });
  return Object.freeze({ ...compiled, derived: acquired.derived, observedSlot: acquired.observedSlot });
}
