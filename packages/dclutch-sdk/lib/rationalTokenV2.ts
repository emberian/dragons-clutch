import {
  AddressLookupTableAccount,
  AddressLookupTableProgram,
  PublicKey,
  TransactionInstruction,
  TransactionMessage,
  VersionedTransaction,
} from '@solana/web3.js';

import { ascii, hex, isZero, requireNonzero, requireZero, sha256, slice, u16, u64 } from './bytes';
import { PACKET_DATA_SIZE } from './directTransaction';
import { deriveFinalizedRecordAddressesV1, SYSTEM_PROGRAM_ID } from './releaseRegistry';
import { type RpcAccount, type SolanaRpcClient } from './rpc';

export const TOKEN_2022_PROGRAM_ID = 'TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb';
export const TOKEN_BEHAVIOR_SELECTION_BYTES_V2 = 144;
export const TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2 = Uint8Array.from([
  0xb4, 0x92, 0xdc, 0x12, 0x85, 0x7a, 0x10, 0xe9, 0xad, 0x28, 0xc5, 0x85, 0x5c, 0x69, 0x55, 0xa0,
  0x10, 0x7f, 0x58, 0x17, 0x35, 0x72, 0x71, 0x34, 0xc0, 0x21, 0xd4, 0xdf, 0xff, 0x9e, 0xa0, 0xe8,
]);
export const TOKEN_2022_BEHAVIOR_PROFILE_ID_V2 = Uint8Array.from([
  0x12, 0x39, 0x3c, 0xc7, 0x3a, 0xb2, 0x58, 0xc7, 0x46, 0xa4, 0xa1, 0x85, 0xa4, 0x76, 0x06, 0x95,
  0x68, 0x41, 0xb4, 0xce, 0x0d, 0x53, 0xb3, 0xaa, 0x04, 0xc7, 0xe6, 0x14, 0xd4, 0x38, 0x14, 0x62,
]);

const MAX_U64 = 18_446_744_073_709_551_615n;
const CORE_STATE_BYTES_V2 = 352;
const TOKEN_ACCOUNT_BYTES = 165;
const TOKEN_MINT_BASE_BYTES = 82;
const TOKEN_MINT_TLV_OFFSET = 166;
const TOKEN_MINT_ACCOUNT_TYPE_OFFSET = 165;
const TOKEN_MINT_ACCOUNT_TYPE = 1;
const TOKEN_BEHAVIOR_MAGIC_V2 = new TextEncoder().encode('DCLTTBS2');
const TOKEN_BEHAVIOR_SCHEMA_V2 = 2;
const TOKEN_TRANSFER_CHECKED_TAG = 12;
const MINT_CLOSE_AUTHORITY_EXTENSION = 3;
const METADATA_POINTER_EXTENSION = 18;
const TOKEN_METADATA_EXTENSION = 19;
const PERMISSIONED_BURN_EXTENSION = 28;

export type TokenBehaviorSelectionViewV2 = Readonly<{
  bytes: Uint8Array;
  realmId: Uint8Array;
  releaseSet: Uint8Array;
  profileId: Uint8Array;
  tokenProgram: string;
}>;

export type TokenBehaviorMintViewV2 = Readonly<{
  mint: string;
  controller: string;
  rawSupply: bigint;
  displayDecimals: number;
  metadata: 'absent' | 'immutable-self-hosted';
}>;

export type TokenBehaviorAccountViewV2 = Readonly<{
  address: string;
  mint: string;
  owner: string;
  rawAmount: bigint;
}>;

export type BearerTransferInspectionV2 = Readonly<{
  observedSlot: string;
  payer: string;
  authority: string;
  coreProgram: string;
  market: string;
  marketPhase: 'Founding' | 'Open' | 'Terminal' | 'Retiring' | 'Retired';
  generation: bigint;
  registryProgram: string;
  selectionRecord: string;
  selectionDigest: Uint8Array;
  selection: TokenBehaviorSelectionViewV2;
  mint: TokenBehaviorMintViewV2;
  source: TokenBehaviorAccountViewV2;
  destination: TokenBehaviorAccountViewV2;
  lookupTable: AddressLookupTableAccount;
}>;

export type BearerTransferPlanV2 = Readonly<{
  transaction: VersionedTransaction;
  instruction: TransactionInstruction;
  instructionBytes: Uint8Array;
  wireBytes: Uint8Array;
  requiredSigners: ReadonlyArray<string>;
  loadedAddresses: number;
  rawAmount: bigint;
  displayDecimals: number;
}>;

type TransferInspectionClient = Pick<
  SolanaRpcClient,
  'finalizedSlot' | 'accountInfo' | 'multipleAccounts' | 'minimumBalanceForRentExemption'
>;

function same(left: Uint8Array, right: Uint8Array): boolean {
  return left.length === right.length && left.every((value, index) => value === right[index]);
}

function key(value: string, field: string): PublicKey {
  const parsed = new PublicKey(value);
  if (parsed.toBase58() !== value) throw new Error(`${field} must be canonical base58 text`);
  return parsed;
}

function required(account: RpcAccount | null, field: string): RpcAccount {
  if (account === null) throw new Error(`${field} is absent at finalized commitment`);
  return account;
}

function putU16(bytes: Uint8Array, offset: number, value: number): void {
  new DataView(bytes.buffer, bytes.byteOffset + offset, 2).setUint16(0, value, true);
}

function putU64(bytes: Uint8Array, offset: number, value: bigint): void {
  if (value < 0n || value > MAX_U64) throw new Error('raw token quantity is outside canonical u64');
  new DataView(bytes.buffer, bytes.byteOffset + offset, 8).setBigUint64(0, value, true);
}

function coptionAddress(bytes: Uint8Array, offset: number, field: string): string | null {
  const tag = new DataView(bytes.buffer, bytes.byteOffset + offset, 4).getUint32(0, true);
  if (tag === 0) return null;
  if (tag !== 1) throw new Error(`${field} has an undefined COption tag`);
  const value = slice(bytes, offset + 4, 32);
  requireNonzero(value, field);
  return new PublicKey(value).toBase58();
}

function coptionU64Absent(bytes: Uint8Array, offset: number, field: string): void {
  const tag = new DataView(bytes.buffer, bytes.byteOffset + offset, 4).getUint32(0, true);
  if (tag !== 0) {
    if (tag !== 1) throw new Error(`${field} has an undefined COption tag`);
    throw new Error(`${field} must be absent under TokenBehaviorProfileV2`);
  }
}

function utf8StringEnd(bytes: Uint8Array, offset: number): number {
  if (offset + 4 > bytes.length) throw new Error('TokenMetadata string length is truncated');
  const width = new DataView(bytes.buffer, bytes.byteOffset + offset, 4).getUint32(0, true);
  const end = offset + 4 + width;
  if (end > bytes.length) throw new Error('TokenMetadata string is truncated');
  new TextDecoder('utf-8', { fatal: true }).decode(bytes.slice(offset + 4, end));
  return end;
}

function validateMetadata(bytes: Uint8Array, mint: Uint8Array): void {
  if (bytes.length < 80 || !isZero(slice(bytes, 0, 32)) || !same(slice(bytes, 32, 32), mint)) {
    throw new Error('TokenMetadata is mutable or selects another Mint');
  }
  let offset = 64;
  for (let index = 0; index < 3; index += 1) offset = utf8StringEnd(bytes, offset);
  if (offset + 4 > bytes.length) throw new Error('TokenMetadata pair count is truncated');
  const count = new DataView(bytes.buffer, bytes.byteOffset + offset, 4).getUint32(0, true);
  offset += 4;
  if (count > Math.floor((bytes.length - offset) / 8)) throw new Error('TokenMetadata pair count exceeds its exact byte bound');
  for (let index = 0; index < count; index += 1) {
    offset = utf8StringEnd(bytes, offset);
    offset = utf8StringEnd(bytes, offset);
  }
  if (offset !== bytes.length) throw new Error('TokenMetadata has a noncanonical trailing suffix');
}

export function encodeTokenBehaviorSelectionV2(realmId: Uint8Array, releaseSet: Uint8Array): Uint8Array {
  if (realmId.length !== 32 || releaseSet.length !== 32) throw new Error('Token behavior Realm and release set must be 32 bytes');
  requireNonzero(realmId, 'Token behavior Realm');
  requireNonzero(releaseSet, 'Token behavior release set');
  if (same(realmId, releaseSet)) throw new Error('Token behavior Realm and release set alias');
  const output = new Uint8Array(TOKEN_BEHAVIOR_SELECTION_BYTES_V2);
  output.set(TOKEN_BEHAVIOR_MAGIC_V2, 0);
  putU16(output, 8, TOKEN_BEHAVIOR_SCHEMA_V2);
  output.set(realmId, 16);
  output.set(releaseSet, 48);
  output.set(TOKEN_2022_BEHAVIOR_PROFILE_ID_V2, 80);
  output.set(key(TOKEN_2022_PROGRAM_ID, 'Token-2022 program').toBytes(), 112);
  return output;
}

export function decodeTokenBehaviorSelectionV2(
  bytes: Uint8Array,
  authenticatedRealm: Uint8Array,
  authenticatedReleaseSet: Uint8Array,
): TokenBehaviorSelectionViewV2 {
  if (bytes.length !== TOKEN_BEHAVIOR_SELECTION_BYTES_V2 || !same(slice(bytes, 0, 8), TOKEN_BEHAVIOR_MAGIC_V2)
      || u16(bytes, 8) !== TOKEN_BEHAVIOR_SCHEMA_V2) throw new Error('TokenBehaviorSelectionV2 has the wrong exact ABI');
  requireZero(bytes, 10, 6, 'TokenBehaviorSelectionV2 header');
  if (!same(slice(bytes, 16, 32), authenticatedRealm) || !same(slice(bytes, 48, 32), authenticatedReleaseSet)) {
    throw new Error('TokenBehaviorSelectionV2 differs from the authenticated Market Realm or release set');
  }
  if (!same(slice(bytes, 80, 32), TOKEN_2022_BEHAVIOR_PROFILE_ID_V2)
      || new PublicKey(slice(bytes, 112, 32)).toBase58() !== TOKEN_2022_PROGRAM_ID) {
    throw new Error('TokenBehaviorSelectionV2 substitutes the behavior profile or Token program');
  }
  return Object.freeze({
    bytes: new Uint8Array(bytes), realmId: slice(bytes, 16, 32), releaseSet: slice(bytes, 48, 32),
    profileId: slice(bytes, 80, 32), tokenProgram: TOKEN_2022_PROGRAM_ID,
  });
}

export function decodeToken2022BehaviorMintV2(address: string, account: RpcAccount): TokenBehaviorMintViewV2 {
  const mintKey = key(address, 'claim Mint');
  if (account.owner !== TOKEN_2022_PROGRAM_ID || account.executable || account.data.length < TOKEN_MINT_TLV_OFFSET) {
    throw new Error('claim Mint is not exact nonexecutable Token-2022 Mint data');
  }
  const bytes = account.data;
  const controller = coptionAddress(bytes, 0, 'Mint authority');
  if (controller === null) throw new Error('claim Mint has no protocol controller');
  const rawSupply = u64(bytes, 36);
  const displayDecimals = bytes[44] ?? 0;
  if (bytes[45] !== 1) throw new Error('claim Mint is uninitialized');
  if (coptionAddress(bytes, 46, 'freeze authority') !== null) throw new Error('claim Mint has a freeze authority');
  requireZero(bytes, TOKEN_MINT_BASE_BYTES, TOKEN_ACCOUNT_BYTES - TOKEN_MINT_BASE_BYTES, 'Mint base padding');
  if (bytes[TOKEN_MINT_ACCOUNT_TYPE_OFFSET] !== TOKEN_MINT_ACCOUNT_TYPE) throw new Error('claim Mint has the wrong Token-2022 account type');

  let offset = TOKEN_MINT_TLV_OFFSET;
  let closeSeen = false;
  let burnSeen = false;
  let pointerSeen = false;
  let metadataSeen = false;
  while (offset < bytes.length) {
    if (offset + 4 > bytes.length) throw new Error('claim Mint TLV header is truncated');
    const extension = u16(bytes, offset);
    const width = u16(bytes, offset + 2);
    const start = offset + 4;
    const end = start + width;
    if (extension === 0 || end > bytes.length) throw new Error('claim Mint has an invalid TLV entry');
    const value = bytes.slice(start, end);
    if (extension === MINT_CLOSE_AUTHORITY_EXTENSION && !closeSeen && width === 32) {
      if (new PublicKey(value).toBase58() !== controller) throw new Error('Mint close authority differs from the protocol controller');
      closeSeen = true;
    } else if (extension === PERMISSIONED_BURN_EXTENSION && !burnSeen && width === 32) {
      if (new PublicKey(value).toBase58() !== controller) throw new Error('permissioned burn authority differs from the protocol controller');
      burnSeen = true;
    } else if (extension === METADATA_POINTER_EXTENSION && !pointerSeen && width === 64) {
      if (!isZero(slice(value, 0, 32)) || !same(slice(value, 32, 32), mintKey.toBytes())) throw new Error('MetadataPointer is mutable or not self-hosted');
      pointerSeen = true;
    } else if (extension === TOKEN_METADATA_EXTENSION && !metadataSeen) {
      validateMetadata(value, mintKey.toBytes());
      metadataSeen = true;
    } else {
      throw new Error(`claim Mint extension ${extension} is duplicate, unknown, or has the wrong exact width`);
    }
    offset = end;
  }
  if (!closeSeen || !burnSeen || pointerSeen !== metadataSeen) throw new Error('claim Mint lacks the exact required lifecycle extensions');
  return Object.freeze({
    mint: address, controller, rawSupply, displayDecimals,
    metadata: metadataSeen ? 'immutable-self-hosted' : 'absent',
  });
}

export function decodeToken2022BehaviorAccountV2(address: string, account: RpcAccount): TokenBehaviorAccountViewV2 {
  key(address, 'claim Token Account');
  if (account.owner !== TOKEN_2022_PROGRAM_ID || account.executable || account.data.length !== TOKEN_ACCOUNT_BYTES) {
    throw new Error('claim Token Account is not exact extension-free Token-2022 data');
  }
  const bytes = account.data;
  const mint = new PublicKey(slice(bytes, 0, 32)).toBase58();
  const owner = new PublicKey(slice(bytes, 32, 32)).toBase58();
  requireNonzero(slice(bytes, 0, 32), 'claim Token Account Mint');
  requireNonzero(slice(bytes, 32, 32), 'claim Token Account owner');
  if (coptionAddress(bytes, 72, 'transfer delegate') !== null || u64(bytes, 121) !== 0n) throw new Error('claim Token Account has delegated authority');
  if (bytes[108] !== 1) throw new Error(bytes[108] === 2 ? 'claim Token Account is frozen' : 'claim Token Account is not initialized');
  coptionU64Absent(bytes, 109, 'native reserve');
  if (coptionAddress(bytes, 129, 'close authority') !== null) throw new Error('claim Token Account has a separate close authority');
  return Object.freeze({ address, mint, owner, rawAmount: u64(bytes, 64) });
}

function decodeCoreMarket(bytes: Uint8Array): Readonly<{
  phase: BearerTransferInspectionV2['marketPhase'];
  realmId: Uint8Array;
  releaseSet: Uint8Array;
  registryProgram: string;
  generation: bigint;
}> {
  if (bytes.length !== CORE_STATE_BYTES_V2 || ascii(bytes, 0, 8) !== 'DCLTCOR2' || u16(bytes, 8) !== 2) {
    throw new Error('Market is not the exact CoreStateV2 ABI');
  }
  const phases = ['Founding', 'Open', 'Terminal', 'Retiring', 'Retired'] as const;
  const phase = phases[bytes[10] ?? 255];
  const readiness = bytes[11] ?? 255;
  if (phase === undefined || readiness > 2) throw new Error('Market has an undefined lifecycle tag');
  const realmId = slice(bytes, 48, 32);
  const releaseSet = slice(bytes, 208, 32);
  const registryBytes = slice(bytes, 240, 32);
  for (const [field, value] of [
    ['Market identity', slice(bytes, 16, 32)], ['Realm identity', realmId], ['Product record', slice(bytes, 80, 32)],
    ['Product identity', slice(bytes, 112, 32)], ['resolution policy', slice(bytes, 144, 32)],
    ['capability manifest', slice(bytes, 176, 32)], ['release set', releaseSet], ['Registry program', registryBytes],
    ['rent beneficiary', slice(bytes, 288, 32)],
  ] as const) requireNonzero(value, field);
  const terminalReceipt = slice(bytes, 320, 32);
  const terminalWinner = new DataView(bytes.buffer, bytes.byteOffset + 12, 4).getUint32(0, true);
  const outstanding = u64(bytes, 280);
  const foundingValid = phase === 'Founding' && readiness !== 2 && terminalWinner === 0 && isZero(terminalReceipt);
  const openValid = phase === 'Open' && readiness === 2 && terminalWinner === 0 && isZero(terminalReceipt);
  const terminalValid = (phase === 'Terminal' || phase === 'Retiring') && readiness === 2 && !isZero(terminalReceipt);
  const retiredValid = phase === 'Retired' && readiness === 2 && !isZero(terminalReceipt) && outstanding === 0n;
  if (!foundingValid && !openValid && !terminalValid && !retiredValid) throw new Error('Market CoreStateV2 lifecycle invariants do not hold');
  return Object.freeze({ phase, realmId, releaseSet, registryProgram: new PublicKey(registryBytes).toBase58(), generation: u64(bytes, 272) });
}

function decodeLookupTable(address: string, account: RpcAccount): AddressLookupTableAccount {
  if (account.owner !== AddressLookupTableProgram.programId.toBase58() || account.executable) {
    throw new Error('address lookup table has the wrong owner or executable bit');
  }
  let state: ReturnType<typeof AddressLookupTableAccount.deserialize>;
  try { state = AddressLookupTableAccount.deserialize(account.data); } catch { throw new Error('address lookup table has malformed data'); }
  const table = new AddressLookupTableAccount({ key: key(address, 'address lookup table'), state });
  if (!table.isActive()) throw new Error('address lookup table is deactivated');
  return table;
}

export async function inspectBearerTransferV2(
  client: TransferInspectionClient,
  input: Readonly<{
    payer: string;
    authority: string;
    coreProgram: string;
    market: string;
    mint: string;
    source: string;
    destination: string;
    lookupTable: string;
  }>,
): Promise<BearerTransferInspectionV2> {
  for (const [field, value] of Object.entries(input)) key(value, field);
  if (input.source === input.destination || input.mint === input.source || input.mint === input.destination) throw new Error('Mint, source, and destination roles alias');
  const floor = await client.finalizedSlot();
  const marketObservation = await client.accountInfo(input.market, floor);
  if (BigInt(marketObservation.slot) < BigInt(floor)) throw new Error('Market observation regressed below the finalized floor');
  const marketAccount = required(marketObservation.account, 'Market');
  if (marketAccount.owner !== input.coreProgram || marketAccount.executable) throw new Error('Market is not nonexecutable state owned by the selected Core program');
  const market = decodeCoreMarket(marketAccount.data);
  const selectionBytes = encodeTokenBehaviorSelectionV2(market.realmId, market.releaseSet);
  const selectionDigest = await sha256(selectionBytes);
  const selectionAddresses = deriveFinalizedRecordAddressesV1(market.registryProgram, TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2, selectionDigest);
  const observation = await client.multipleAccounts([
    selectionAddresses.record, selectionAddresses.staging, input.mint, input.source, input.destination, input.lookupTable,
  ], floor);
  if (BigInt(observation.slot) < BigInt(floor)) throw new Error('token route observation regressed below the finalized floor');
  const accounts = new Map(observation.accounts.map((entry) => [entry.address, entry.account]));
  const record = required(accounts.get(selectionAddresses.record) ?? null, 'TokenBehaviorSelectionV2 record');
  const staging = accounts.get(selectionAddresses.staging);
  if (record.owner !== market.registryProgram || record.executable || !same(record.data, selectionBytes)) {
    throw new Error('TokenBehaviorSelectionV2 is not exact Registry-owned finalized content');
  }
  const rent = await client.minimumBalanceForRentExemption(record.data.length);
  if (BigInt(record.lamports) < BigInt(rent.lamports)) throw new Error('TokenBehaviorSelectionV2 record is below its exact current rent minimum');
  if (staging !== null && staging !== undefined
      && (staging.owner !== SYSTEM_PROGRAM_ID || staging.executable || staging.data.length !== 0)) {
    throw new Error('TokenBehaviorSelectionV2 staging cursor is not vacant System-owned data');
  }
  const selection = decodeTokenBehaviorSelectionV2(record.data, market.realmId, market.releaseSet);
  const mint = decodeToken2022BehaviorMintV2(input.mint, required(accounts.get(input.mint) ?? null, 'claim Mint'));
  const source = decodeToken2022BehaviorAccountV2(input.source, required(accounts.get(input.source) ?? null, 'source Token Account'));
  const destination = decodeToken2022BehaviorAccountV2(input.destination, required(accounts.get(input.destination) ?? null, 'destination Token Account'));
  if (source.mint !== input.mint || destination.mint !== input.mint) throw new Error('source and destination do not share the authenticated claim Mint');
  if (source.owner !== input.authority) throw new Error('transfer authority does not own the source Token Account');
  const lookupTable = decodeLookupTable(input.lookupTable, required(accounts.get(input.lookupTable) ?? null, 'address lookup table'));
  return Object.freeze({
    observedSlot: observation.slot, payer: input.payer, authority: input.authority, coreProgram: input.coreProgram,
    market: input.market, marketPhase: market.phase, generation: market.generation, registryProgram: market.registryProgram,
    selectionRecord: selectionAddresses.record, selectionDigest, selection, mint, source, destination, lookupTable,
  });
}

export function buildUnsignedBearerTransferV2(
  inspection: BearerTransferInspectionV2,
  recentBlockhash: string,
  rawAmount: bigint,
): BearerTransferPlanV2 {
  if (rawAmount <= 0n || rawAmount > MAX_U64) throw new Error('transfer raw quantity must be 1..u64::MAX atoms');
  if (rawAmount > inspection.source.rawAmount) throw new Error('source raw balance cannot fund the exact transfer');
  if (inspection.source.mint !== inspection.mint.mint || inspection.destination.mint !== inspection.mint.mint) {
    throw new Error('inspected token state no longer shares one Mint');
  }
  key(recentBlockhash, 'recent blockhash');
  const instructionBytes = new Uint8Array(10);
  instructionBytes[0] = TOKEN_TRANSFER_CHECKED_TAG;
  putU64(instructionBytes, 1, rawAmount);
  instructionBytes[9] = inspection.mint.displayDecimals;
  const instruction = new TransactionInstruction({
    programId: key(inspection.selection.tokenProgram, 'selected Token program'),
    keys: [
      { pubkey: key(inspection.source.address, 'source Token Account'), isSigner: false, isWritable: true },
      { pubkey: key(inspection.mint.mint, 'claim Mint'), isSigner: false, isWritable: false },
      { pubkey: key(inspection.destination.address, 'destination Token Account'), isSigner: false, isWritable: true },
      { pubkey: key(inspection.authority, 'transfer authority'), isSigner: true, isWritable: false },
    ],
    data: instructionBytes as Buffer,
  });
  const transaction = new VersionedTransaction(new TransactionMessage({
    payerKey: key(inspection.payer, 'transaction payer'), recentBlockhash, instructions: [instruction],
  }).compileToV0Message([inspection.lookupTable]));
  const wireBytes = transaction.serialize();
  if (wireBytes.length > PACKET_DATA_SIZE) throw new Error(`Bearer transfer is ${wireBytes.length} bytes, above the ${PACKET_DATA_SIZE}-byte packet bound`);
  const requiredSigners = Object.freeze(transaction.message.staticAccountKeys
    .slice(0, transaction.message.header.numRequiredSignatures).map((value) => value.toBase58()));
  const expected = new Set([inspection.payer, inspection.authority]);
  if (requiredSigners.length !== expected.size || requiredSigners.some((signer) => !expected.has(signer))) {
    throw new Error('Bearer transfer message has an unexpected signer set');
  }
  const loadedAddresses = transaction.message.addressTableLookups
    .reduce((total, lookup) => total + lookup.readonlyIndexes.length + lookup.writableIndexes.length, 0);
  if (loadedAddresses === 0) throw new Error('selected address lookup table did not contribute to the v0 packet');
  return Object.freeze({
    transaction, instruction, instructionBytes, wireBytes, requiredSigners, loadedAddresses,
    rawAmount, displayDecimals: inspection.mint.displayDecimals,
  });
}

export function tokenBehaviorSummaryV2(inspection: BearerTransferInspectionV2): Readonly<{
  realmId: string;
  releaseSet: string;
  selectionDigest: string;
  profileId: string;
}> {
  return Object.freeze({
    realmId: hex(inspection.selection.realmId), releaseSet: hex(inspection.selection.releaseSet),
    selectionDigest: hex(inspection.selectionDigest), profileId: hex(inspection.selection.profileId),
  });
}
