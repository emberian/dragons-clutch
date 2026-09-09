import { PublicKey, TransactionInstruction, TransactionMessage, VersionedTransaction, type AddressLookupTableAccount } from '@solana/web3.js';
import { hex } from '@dclutch/sdk/bytes';
import { type RpcAccount, type SolanaRpcClient } from '@dclutch/sdk/rpc';
import { readStructuredLifecycleSnapshotV1, type StructuredAccountRequestV1 } from './structuredLifecycleSnapshotV1';
import { type StructuredLifecycleIntentV1 } from './structuredLifecycleModelV1';

export type StructuredLifecycleNativeV1 = Readonly<{
  plan_structured_lifecycle_v1(source: string): string;
  verify_structured_lifecycle_poststates_v1(source: string): string;
}>;
export type StructuredNativeAccountV1 = Readonly<{ owner: string; lamports: string; executable: boolean; space: string; dataBase64: string }>;
export type StructuredNativeScanV1 = Readonly<{ program: string; dataSize: number | null; memcmp: ReadonlyArray<{ offset: number; bytesBase64: string }>; dataSlice: Readonly<{ offset: number; length: number }> | null }>;
export type StructuredNativeSnapshotV1 = Readonly<{
  slot: string;
  accounts: ReadonlyArray<{ request: StructuredAccountRequestV1; value: StructuredNativeAccountV1 | null }>;
  scans: ReadonlyArray<{ request: StructuredNativeScanV1; slot: string; addresses: ReadonlyArray<string> }>;
}>;
export type StructuredNativeInputV1 = Readonly<{ format: 'dclutch-structured-lifecycle-input-v1'; intent: StructuredLifecycleIntentV1; checkedInfrastructureBase64: string; snapshot: StructuredNativeSnapshotV1 }>;
export type StructuredNativePreviewV1 = Readonly<{ receiptMint: string; coordinate: number | null; position: string | null; preparationLamports: string; returnedRentLamports: string; rentRecipient: string | null; receiptSupplyBefore: string; receiptSupplyAfter: string }>;
export type StructuredNativePlanV1 = Readonly<{
  intent: StructuredLifecycleIntentV1; stepId: string; stepKind: 'seal-artifact' | 'fund-rent' | 'execute-lifecycle'; selectedCapability: string; finalizedSlot: string;
  instructions: ReadonlyArray<{ programId: string; accounts: ReadonlyArray<{ address: string; isSigner: boolean; isWritable: boolean }>; dataBase64: string }>;
  requiredWalletSigners: ReadonlyArray<string>; preview: StructuredNativePreviewV1;
  expectedPoststates: ReadonlyArray<{ address: string; value: StructuredNativeAccountV1 | null; deductTransactionFee: boolean }>;
}>;
export type StructuredNativeResultV1 =
  | Readonly<{ kind: 'discover'; requests: ReadonlyArray<StructuredAccountRequestV1>; scans: ReadonlyArray<StructuredNativeScanV1> }>
  | Readonly<{ kind: 'select-capability'; capabilities: ReadonlyArray<{ capability: string; programSet: string; entryIndex: number }> }>
  | Readonly<{ kind: 'select'; capabilities: ReadonlyArray<{ capability: string; descriptor: string; receiptMint: string; denominator: string; support: ReadonlyArray<{ coordinate: number; coefficient: string }> }> }>
  | Readonly<{ kind: 'ready'; plan: StructuredNativePlanV1 }>
  | Readonly<{ kind: 'complete'; selectedCapability: string; preview: StructuredNativePreviewV1 }>;

export function structuredBase64V1(bytes: Uint8Array): string {
  let value = ''; for (let start = 0; start < bytes.length; start += 8192) value += String.fromCharCode(...bytes.subarray(start, start + 8192));
  return btoa(value);
}
export function structuredBytesV1(source: string): Uint8Array {
  const bytes = Uint8Array.from(atob(source), (character) => character.charCodeAt(0));
  if (structuredBase64V1(bytes) !== source) throw new Error('Structured bytes are not canonical base64');
  return bytes;
}
function account(value: RpcAccount | null): StructuredNativeAccountV1 | null {
  return value === null ? null : { owner: value.owner, lamports: value.lamports, executable: value.executable, space: String(value.space), dataBase64: structuredBase64V1(value.data) };
}
export function structuredNativeResultV1(native: StructuredLifecycleNativeV1, input: StructuredNativeInputV1): StructuredNativeResultV1 {
  const output = JSON.parse(native.plan_structured_lifecycle_v1(JSON.stringify(input))) as { format?: unknown; result?: StructuredNativeResultV1 };
  if (output.format !== 'dclutch-structured-lifecycle-planning-v1' || !output.result || !['discover', 'select-capability', 'select', 'ready', 'complete'].includes(output.result.kind)) throw new Error('Structured native planner returned an invalid result');
  return output.result;
}
function mergeRequests(old: ReadonlyArray<StructuredAccountRequestV1>, added: ReadonlyArray<StructuredAccountRequestV1>): StructuredAccountRequestV1[] {
  const requests = new Map(old.map((row) => [row.address, row]));
  for (const row of added) {
    const prior = requests.get(row.address);
    if (prior && prior.dataSlice !== null && row.dataSlice !== null && JSON.stringify(prior.dataSlice) !== JSON.stringify(row.dataSlice)) throw new Error('Structured native discovery requested conflicting account windows');
    requests.set(row.address, prior?.dataSlice === null ? prior : row);
  }
  return [...requests.values()];
}

/** Native code selects every account and scan; every iteration reacquires the complete point corpus. */
export async function discoverStructuredLifecycleV1(
  client: Pick<SolanaRpcClient, 'assertMutationCluster' | 'finalizedSlot' | 'multipleAccounts' | 'multipleAccountDataSlices' | 'programAccountsFiltered'>,
  native: StructuredLifecycleNativeV1, intent: StructuredLifecycleIntentV1, checkedInfrastructure: Uint8Array,
  requireCurrent: () => void = () => {},
): Promise<Readonly<{ clusterGenesis: string; input: StructuredNativeInputV1; result: Exclude<StructuredNativeResultV1, { kind: 'discover' }> }>> {
  const cluster = await client.assertMutationCluster(); requireCurrent();
  let floor = await client.finalizedSlot(); requireCurrent();
  let snapshot: StructuredNativeSnapshotV1 = { slot: floor, accounts: [], scans: [] };
  let requests: StructuredAccountRequestV1[] = [];
  const scans = new Map<string, StructuredNativeSnapshotV1['scans'][number]>();
  // Provisional discovery profile: expand only with a measured supported native route.
  for (let round = 0; round < 24; round += 1) {
    requireCurrent();
    const input: StructuredNativeInputV1 = { format: 'dclutch-structured-lifecycle-input-v1', intent, checkedInfrastructureBase64: structuredBase64V1(checkedInfrastructure), snapshot };
    const result = structuredNativeResultV1(native, input); requireCurrent();
    if (result.kind !== 'discover') return { clusterGenesis: cluster.genesisHash, input, result };
    if (result.requests.length === 0 && result.scans.length === 0) throw new Error('Structured native discovery made no progress');
    requests = mergeRequests(requests, result.requests);
    if (result.scans.length > 4) throw new Error('Structured discovery exceeds its four-scan transport profile');
    for (const query of result.scans) {
      const observed = await client.programAccountsFiltered(query.program, {
        ...(query.dataSize === null ? {} : { dataSize: query.dataSize }),
        memcmp: query.memcmp.map((filter) => ({ offset: filter.offset, bytes: structuredBytesV1(filter.bytesBase64) })),
        ...(query.dataSlice === null ? {} : { dataSlice: query.dataSlice }),
      }, floor); requireCurrent();
      scans.set(JSON.stringify(query), { request: query, slot: observed.slot, addresses: observed.accounts.map((row) => row.address) });
      if (BigInt(observed.slot) > BigInt(floor)) floor = observed.slot;
    }
    if (requests.length === 0) throw new Error('Structured native discovery omitted its point-account corpus');
    const observed = await readStructuredLifecycleSnapshotV1(client, requests, floor, requireCurrent); requireCurrent();
    floor = observed.slot;
    snapshot = { slot: observed.slot, accounts: observed.accounts.map((row) => ({ request: { address: row.address, dataSlice: row.dataSlice }, value: account(row.account) })), scans: [...scans.values()] };
  }
  throw new Error('Structured discovery exceeds its 24-round transport profile');
}

/** Compile only native-authored instructions through the official Solana transaction library. */
export function compileStructuredLifecycleStepV1(plan: StructuredNativePlanV1, blockhash: string, tables: ReadonlyArray<AddressLookupTableAccount>): VersionedTransaction {
  if (plan.requiredWalletSigners.length !== 1 || plan.requiredWalletSigners[0] !== plan.intent.payer) throw new Error('Structured step requires a signer other than the selected payer');
  const instructions = plan.instructions.map((ix) => new TransactionInstruction({ programId: new PublicKey(ix.programId), keys: ix.accounts.map((meta) => ({ pubkey: new PublicKey(meta.address), isSigner: meta.isSigner, isWritable: meta.isWritable })), data: Buffer.from(structuredBytesV1(ix.dataBase64)) }));
  const transaction = new VersionedTransaction(new TransactionMessage({ payerKey: new PublicKey(plan.intent.payer), recentBlockhash: blockhash, instructions }).compileToV0Message([...tables]));
  if (transaction.message.header.numRequiredSignatures !== 1 || transaction.serialize().length > 1232) throw new Error('Structured step exceeds the one-payer Solana packet profile');
  return transaction;
}

/** A saved review must reproduce the native plan and the same exact instruction message. */
export function reconstructStructuredLifecycleReviewV1(native: StructuredLifecycleNativeV1, input: StructuredNativeInputV1, plan: StructuredNativePlanV1, transaction: VersionedTransaction, tables: ReadonlyArray<AddressLookupTableAccount>): void {
  const result = structuredNativeResultV1(native, input);
  if (result.kind !== 'ready' || JSON.stringify(result.plan) !== JSON.stringify(plan)) throw new Error('Structured saved plan differs from native reconstruction');
  const reconstructed = compileStructuredLifecycleStepV1(result.plan, transaction.message.recentBlockhash, tables);
  if (hex(reconstructed.message.serialize()) !== hex(transaction.message.serialize())) throw new Error('Structured saved packet differs from native instructions');
}
