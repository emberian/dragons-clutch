import {
  AddressLookupTableAccount,
  AddressLookupTableProgram,
  PublicKey,
  VersionedTransaction,
} from '@solana/web3.js';

import { type RpcAccount, type SolanaRpcClient } from './rpc';

export const SOLANA_PACKET_BYTES = 1_232;
const MAX_UNSIGNED_TEXT_BYTES = 4_096;

type CompiledInstructionView = Readonly<{ programIdIndex: number; accountKeyIndexes: Uint8Array | number[] }>;
type MessageAccountKeysView = Readonly<{ length: number; get(index: number): PublicKey | undefined }>;
type VersionedMessageView = Readonly<{
  staticAccountKeys: PublicKey[];
  addressTableLookups: ReadonlyArray<Readonly<{ accountKey: PublicKey }>>;
  compiledInstructions: ReadonlyArray<CompiledInstructionView>;
  header: Readonly<{ numRequiredSignatures: number }>;
  getAccountKeys(input?: Readonly<{ addressLookupTableAccounts: AddressLookupTableAccount[] }>): MessageAccountKeysView;
  isAccountWritable(index: number): boolean;
}>;

export type UnsignedTransactionInspectionV1 = Readonly<{
  transaction: VersionedTransaction;
  bytes: Uint8Array;
  digestHex: string;
  wireBytes: number;
  requiredSignatures: number;
  staticAccounts: number;
  lookupTables: ReadonlyArray<string>;
  instructionCount: number;
}>;

export type UnsignedDependencyV1 = Readonly<{
  address: string;
  signer: boolean;
  writable: boolean;
  program: boolean;
  present: boolean;
  executable: boolean | null;
  owner: string | null;
  dataBytes: number | null;
}>;

export type UnsignedTransactionChainReportV1 = Readonly<{
  observedSlots: ReadonlyArray<string>;
  dependencies: ReadonlyArray<UnsignedDependencyV1>;
  missing: ReadonlyArray<string>;
  nonExecutablePrograms: ReadonlyArray<string>;
}>;

function decodeBase64(text: string): Uint8Array {
  if (text.trim() !== text || text.length === 0 || text.length > MAX_UNSIGNED_TEXT_BYTES || text.length % 4 !== 0 || !/^[A-Za-z0-9+/]*={0,2}$/.test(text)) {
    throw new Error('unsigned transaction must be bounded canonical base64 text');
  }
  const binary = atob(text);
  return Uint8Array.from(binary, (character) => character.charCodeAt(0));
}

function hex(bytes: Uint8Array): string {
  return Array.from(bytes, (byte) => byte.toString(16).padStart(2, '0')).join('');
}

function messageView(transaction: VersionedTransaction): VersionedMessageView {
  return transaction.message as unknown as VersionedMessageView;
}

function requireUnsigned(transaction: VersionedTransaction): void {
  if (transaction.signatures.some((signature) => signature.some((byte) => byte !== 0))) {
    throw new Error('transaction already contains at least one signature');
  }
  const required = messageView(transaction).header.numRequiredSignatures;
  if (transaction.signatures.length !== required) throw new Error('transaction signature vector does not match the message header');
}

export async function inspectUnsignedTransactionV1(base64: string): Promise<UnsignedTransactionInspectionV1> {
  const bytes = decodeBase64(base64);
  if (bytes.length > SOLANA_PACKET_BYTES) throw new Error(`transaction is ${bytes.length} bytes, above Solana's ${SOLANA_PACKET_BYTES}-byte packet limit`);
  let transaction: VersionedTransaction;
  try {
    transaction = VersionedTransaction.deserialize(bytes);
  } catch {
    throw new Error('bytes are not one canonical Solana versioned transaction');
  }
  requireUnsigned(transaction);
  const message = messageView(transaction);
  const digest = new Uint8Array(await crypto.subtle.digest('SHA-256', bytes));
  return Object.freeze({
    transaction,
    bytes,
    digestHex: hex(digest),
    wireBytes: bytes.length,
    requiredSignatures: message.header.numRequiredSignatures,
    staticAccounts: message.staticAccountKeys.length,
    lookupTables: Object.freeze(message.addressTableLookups.map((lookup) => lookup.accountKey.toBase58())),
    instructionCount: message.compiledInstructions.length,
  });
}

function requireLookup(account: RpcAccount | null, address: string): AddressLookupTableAccount {
  if (account === null) throw new Error(`lookup table ${address} is absent`);
  if (account.owner !== AddressLookupTableProgram.programId.toBase58() || account.executable) throw new Error(`lookup table ${address} has invalid authority`);
  let state: ReturnType<typeof AddressLookupTableAccount.deserialize>;
  try {
    state = AddressLookupTableAccount.deserialize(account.data);
  } catch {
    throw new Error(`lookup table ${address} has malformed data`);
  }
  const table = new AddressLookupTableAccount({ key: new PublicKey(address), state });
  if (!table.isActive()) throw new Error(`lookup table ${address} is deactivated`);
  return table;
}

function chunks<T>(values: ReadonlyArray<T>, width: number): T[][] {
  const output: T[][] = [];
  for (let index = 0; index < values.length; index += width) output.push(values.slice(index, index + width));
  return output;
}

export async function acquireUnsignedTransactionDependenciesV1(
  client: SolanaRpcClient,
  inspection: UnsignedTransactionInspectionV1,
): Promise<UnsignedTransactionChainReportV1> {
  const floor = await client.finalizedSlot();
  const message = messageView(inspection.transaction);
  const lookupTables: AddressLookupTableAccount[] = [];
  const observedSlots: string[] = [];
  if (inspection.lookupTables.length > 0) {
    const observation = await client.multipleAccounts(inspection.lookupTables, floor);
    observedSlots.push(observation.slot);
    for (const entry of observation.accounts) lookupTables.push(requireLookup(entry.account, entry.address));
  }
  const keys = message.getAccountKeys(lookupTables.length === 0 ? undefined : { addressLookupTableAccounts: lookupTables });
  const addresses = Array.from({ length: keys.length }, (_, index) => {
    const key = keys.get(index);
    if (key === undefined) throw new Error(`message account index ${index} did not resolve`);
    return key.toBase58();
  });
  if (new Set(addresses).size !== addresses.length) throw new Error('transaction message aliases one account index more than once');
  const programIndexes = new Set(message.compiledInstructions.map((instruction) => instruction.programIdIndex));
  const accounts = new Map<string, RpcAccount | null>();
  for (const group of chunks(addresses, 32)) {
    const observation = await client.multipleAccounts(group, floor);
    observedSlots.push(observation.slot);
    for (const entry of observation.accounts) accounts.set(entry.address, entry.account);
  }
  const dependencies = addresses.map((address, index) => {
    const account = accounts.get(address) ?? null;
    return Object.freeze({
      address,
      signer: index < message.header.numRequiredSignatures,
      writable: message.isAccountWritable(index),
      program: programIndexes.has(index),
      present: account !== null,
      executable: account?.executable ?? null,
      owner: account?.owner ?? null,
      dataBytes: account?.data.length ?? null,
    });
  });
  const missing = dependencies.filter((dependency) => !dependency.present && !dependency.signer).map((dependency) => dependency.address);
  const nonExecutablePrograms = dependencies.filter((dependency) => dependency.program && dependency.executable !== true).map((dependency) => dependency.address);
  return Object.freeze({
    observedSlots: Object.freeze(observedSlots),
    dependencies: Object.freeze(dependencies),
    missing: Object.freeze(missing),
    nonExecutablePrograms: Object.freeze(nonExecutablePrograms),
  });
}

export type ReadonlyWalletIdentityV1 = Readonly<{ address: string; label: string }>;

type InjectedWallet = Readonly<{
  publicKey?: Readonly<{ toBase58(): string }>;
  connect(input?: Readonly<{ onlyIfTrusted?: boolean }>): Promise<unknown>;
}>;

export async function requestReadonlyWalletIdentityV1(candidate: unknown): Promise<ReadonlyWalletIdentityV1> {
  if (candidate === null || typeof candidate !== 'object' || !('connect' in candidate) || typeof candidate.connect !== 'function') {
    throw new Error('no compatible injected Solana wallet adapter is available');
  }
  const wallet = candidate as InjectedWallet;
  await wallet.connect();
  const address = wallet.publicKey?.toBase58();
  if (address === undefined || new PublicKey(address).toBase58() !== address) throw new Error('wallet did not expose one canonical public identity');
  return Object.freeze({ address, label: 'connected identity only · no signature requested' });
}
