import {
  PublicKey,
  TransactionInstruction,
  TransactionMessage,
  VersionedTransaction,
} from '@solana/web3.js';

import { ascii, isZero, requireZero, slice, u16, u64 } from './bytes';
import { decodeCompactIntentV1, encodeCompactIntentV1, type CompactIntentV1 } from './directCodec';
import { CLAIM_PROGRAM_ID, CONTROLLER_SEED, CUSTODY_PROGRAM_ID, PACKET_DATA_SIZE, POSITION_SEED } from './directTransaction';
import {
  REGISTERED_BUYER_POSITION_BUMP_OFFSET,
  REGISTERED_BUYER_REGISTRATION_BUMP_OFFSET,
  REGISTERED_CONTROLLER_ABI_VERSION,
  REGISTERED_CONTROLLER_BUMP_OFFSET,
  REGISTERED_CONTROLLER_BYTES_VALUE,
  REGISTERED_CONTROLLER_EXECUTION_PRICE_OFFSET,
  REGISTERED_CONTROLLER_FILL_OFFSET,
  REGISTERED_CONTROLLER_MAGIC_BYTES,
  REGISTERED_CONTROLLER_MAGIC_OFFSET,
  REGISTERED_CONTROLLER_RESERVED_OFFSET,
  REGISTERED_CONTROLLER_VERSION_OFFSET,
  REGISTERED_SELLER_POSITION_BUMP_OFFSET,
  REGISTERED_SELLER_REGISTRATION_BUMP_OFFSET,
  REGISTERED_STATE_ABI_VERSION,
  REGISTERED_STATE_BYTES_VALUE,
  REGISTERED_STATE_CONTROLLER_OFFSET,
  REGISTERED_STATE_INTENT_OFFSET,
  REGISTERED_STATE_MAGIC_BYTES,
  REGISTERED_STATE_MAGIC_OFFSET,
  REGISTERED_STATE_MAKER_OFFSET,
  REGISTERED_STATE_PHASE_OFFSET,
  REGISTERED_STATE_REMAINING_OFFSET,
  REGISTERED_STATE_RESERVED_OFFSET,
  REGISTERED_STATE_SEQUENCE_OFFSET,
  REGISTERED_STATE_VERSION_OFFSET,
  REGISTERED_TERMINAL_ABI_VERSION,
  REGISTERED_TERMINAL_ACTION_OFFSET,
  REGISTERED_TERMINAL_BYTES_VALUE,
  REGISTERED_TERMINAL_CANCEL,
  REGISTERED_TERMINAL_CONTROLLER_BUMP_OFFSET,
  REGISTERED_TERMINAL_EXPECTED_SEQUENCE_OFFSET,
  REGISTERED_TERMINAL_EXPIRE,
  REGISTERED_TERMINAL_MAGIC_BYTES,
  REGISTERED_TERMINAL_MAGIC_OFFSET,
  REGISTERED_TERMINAL_REGISTRATION_BUMP_OFFSET,
  REGISTERED_TERMINAL_RESERVED_OFFSET,
  REGISTERED_TERMINAL_VERSION_OFFSET,
} from './generated/registeredDirect';
import { type RpcAccount, type SolanaRpcClient } from './rpc';

export const REGISTERED_SEED = new TextEncoder().encode('dclutch/direct-registered/v1');
const MAX_REGISTERED_STATES = 128;

export type RegisteredPhase = 'open' | 'filled' | 'cancelled' | 'expired';

export type RegisteredIntentStateV1 = Readonly<{
  phase: number;
  controller: string;
  maker: string;
  intent: CompactIntentV1;
  remaining: bigint;
  sequence: bigint;
}>;

export type RegisteredDirectStateObservation = Readonly<{
  status: 'accepted';
  address: string;
  observedSlot: string;
  lamports: string;
  bump: number;
  state: RegisteredIntentStateV1;
}>;

export type RefusedRegisteredDirectState = Readonly<{
  status: 'refused';
  address: string;
  observedSlot: string;
  reason: string;
}>;

export type RegisteredDirectSnapshot = Readonly<{
  scanSlot: string;
  states: ReadonlyArray<RegisteredDirectStateObservation>;
  refused: ReadonlyArray<RefusedRegisteredDirectState>;
}>;

export type RegisteredFillRouteV1 = Readonly<{
  journal: string;
  realm: string;
  feePolicy: string;
  capabilityManifest: string;
  mint: string;
  source: string;
  sellerDestination: string;
  feeDestination: string;
  tokenProgram: string;
}>;

export type RegisteredFillInputV1 = Readonly<{
  controllerProgram: string;
  payer: string;
  recentBlockhash: string;
  seller: RegisteredDirectStateObservation;
  buyer: RegisteredDirectStateObservation;
  fill: bigint;
  executionPrice: bigint;
  route: RegisteredFillRouteV1;
}>;

export type RegisteredTerminalInputV1 = Readonly<{
  controllerProgram: string;
  payer: string;
  recentBlockhash: string;
  state: RegisteredDirectStateObservation;
  action: 'cancel' | 'expire';
  finalizedSlot: bigint;
}>;

export type RegisteredTransactionPlanV1 = Readonly<{
  instruction: TransactionInstruction;
  transaction: VersionedTransaction;
  wireBytes: Uint8Array;
  requiredSignerKeys: ReadonlyArray<string>;
}>;

function canonicalKey(value: string, field: string): PublicKey {
  const key = new PublicKey(value);
  if (key.toBase58() !== value) throw new Error(`${field} must be canonical base58 text`);
  return key;
}

function exactMagic(bytes: Uint8Array, offset: number, magic: Uint8Array, field: string): void {
  if (!slice(bytes, offset, magic.length).every((value, index) => value === magic[index])) throw new Error(`${field} magic is not canonical`);
}

function putU16(bytes: Uint8Array, offset: number, value: number): void {
  new DataView(bytes.buffer, bytes.byteOffset + offset, 2).setUint16(0, value, true);
}

function putU64(bytes: Uint8Array, offset: number, value: bigint, field: string): void {
  if (value < 0n || value > 18_446_744_073_709_551_615n) throw new Error(`${field} is not a u64`);
  new DataView(bytes.buffer, bytes.byteOffset + offset, 8).setBigUint64(0, value, true);
}

function u64Bytes(value: bigint, field: string): Uint8Array {
  const output = new Uint8Array(8);
  putU64(output, 0, value, field);
  return output;
}

export function registeredPhase(phase: number): RegisteredPhase {
  const labels: RegisteredPhase[] = ['open', 'filled', 'cancelled', 'expired'];
  const label = labels[phase];
  if (label === undefined) throw new Error('registered phase is undefined');
  return label;
}

export function decodeRegisteredIntentStateV1(bytes: Uint8Array): RegisteredIntentStateV1 {
  if (bytes.length !== REGISTERED_STATE_BYTES_VALUE) throw new Error(`registered state must be exactly ${REGISTERED_STATE_BYTES_VALUE} bytes`);
  exactMagic(bytes, REGISTERED_STATE_MAGIC_OFFSET, REGISTERED_STATE_MAGIC_BYTES, 'registered state');
  if (u16(bytes, REGISTERED_STATE_VERSION_OFFSET) !== REGISTERED_STATE_ABI_VERSION) throw new Error('registered state version is unsupported');
  requireZero(bytes, REGISTERED_STATE_RESERVED_OFFSET, REGISTERED_STATE_CONTROLLER_OFFSET - REGISTERED_STATE_RESERVED_OFFSET, 'registered state header');
  const phase = bytes[REGISTERED_STATE_PHASE_OFFSET];
  registeredPhase(phase);
  const controllerBytes = slice(bytes, REGISTERED_STATE_CONTROLLER_OFFSET, 32);
  const makerBytes = slice(bytes, REGISTERED_STATE_MAKER_OFFSET, 32);
  if (isZero(controllerBytes) || isZero(makerBytes)) throw new Error('registered controller and maker must be nonzero');
  const intent = decodeCompactIntentV1(slice(bytes, REGISTERED_STATE_INTENT_OFFSET, 136));
  const remaining = u64(bytes, REGISTERED_STATE_REMAINING_OFFSET);
  const sequence = u64(bytes, REGISTERED_STATE_SEQUENCE_OFFSET);
  if (intent.lifecycle > 2) throw new Error('persisted Direct state has an undefined lifecycle policy');
  if (intent.side > 1) throw new Error('persisted Direct state has an undefined maker side');
  if (remaining > intent.maximumFill) throw new Error('registered remaining quantity exceeds signed capacity');
  if (phase === 0 && remaining === 0n) throw new Error('open registered state has no remaining quantity');
  if (phase === 1 && remaining !== 0n) throw new Error('filled registered state retains remaining quantity');
  return Object.freeze({
    phase,
    controller: new PublicKey(controllerBytes).toBase58(),
    maker: new PublicKey(makerBytes).toBase58(),
    intent,
    remaining,
    sequence,
  });
}

export function encodeRegisteredIntentStateV1(state: RegisteredIntentStateV1): Uint8Array {
  registeredPhase(state.phase);
  const output = new Uint8Array(REGISTERED_STATE_BYTES_VALUE);
  output.set(REGISTERED_STATE_MAGIC_BYTES, REGISTERED_STATE_MAGIC_OFFSET);
  putU16(output, REGISTERED_STATE_VERSION_OFFSET, REGISTERED_STATE_ABI_VERSION);
  output[REGISTERED_STATE_PHASE_OFFSET] = state.phase;
  output.set(canonicalKey(state.controller, 'registered controller').toBytes(), REGISTERED_STATE_CONTROLLER_OFFSET);
  output.set(canonicalKey(state.maker, 'registered maker').toBytes(), REGISTERED_STATE_MAKER_OFFSET);
  output.set(encodeCompactIntentV1(state.intent), REGISTERED_STATE_INTENT_OFFSET);
  putU64(output, REGISTERED_STATE_REMAINING_OFFSET, state.remaining, 'remaining quantity');
  putU64(output, REGISTERED_STATE_SEQUENCE_OFFSET, state.sequence, 'registered sequence');
  return output;
}

export function deriveRegisteredAddress(
  controllerProgramText: string,
  state: RegisteredIntentStateV1,
): Readonly<{ address: PublicKey; bump: number }> {
  const controllerProgram = canonicalKey(controllerProgramText, 'controller program');
  const [controller] = PublicKey.findProgramAddressSync([CONTROLLER_SEED], controllerProgram);
  if (state.controller !== controller.toBase58()) throw new Error('registered state names a noncanonical controller authority');
  const [address, bump] = PublicKey.findProgramAddressSync([
    REGISTERED_SEED,
    state.intent.market,
    u64Bytes(state.intent.generation, 'generation'),
    canonicalKey(state.maker, 'maker').toBytes(),
    u64Bytes(state.intent.nonce, 'nonce'),
  ], controllerProgram);
  return Object.freeze({ address, bump });
}

export async function observeRegisteredDirectState(
  client: SolanaRpcClient,
  controllerProgram: string,
  address: string,
  minimumContextSlot: string,
): Promise<RegisteredDirectStateObservation> {
  const observation = await client.accountInfo(address, minimumContextSlot);
  if (observation.account === null) throw new Error('registered state disappeared from finalized chain state');
  return projectRegisteredDirectState(controllerProgram, address, observation.slot, observation.account);
}

export function projectRegisteredDirectState(
  controllerProgram: string,
  address: string,
  observedSlot: string,
  account: RpcAccount,
): RegisteredDirectStateObservation {
  if (account.owner !== CLAIM_PROGRAM_ID.toBase58() || account.executable) throw new Error('registered state has the wrong physical owner or is executable');
  const state = decodeRegisteredIntentStateV1(account.data);
  const derived = deriveRegisteredAddress(controllerProgram, state);
  if (derived.address.toBase58() !== address) throw new Error('registered state address does not match its exact intent coordinates');
  return Object.freeze({ status: 'accepted', address, observedSlot, lamports: account.lamports, bump: derived.bump, state });
}

async function concurrentMap<T, U>(values: ReadonlyArray<T>, mapper: (value: T) => Promise<U>): Promise<U[]> {
  const output = new Array<U>(values.length);
  let next = 0;
  async function worker(): Promise<void> {
    for (;;) {
      const index = next++;
      if (index >= values.length) return;
      output[index] = await mapper(values[index]);
    }
  }
  await Promise.all(Array.from({ length: Math.min(4, values.length) }, () => worker()));
  return output;
}

export async function scanRegisteredDirectStates(
  client: SolanaRpcClient,
  controllerProgram: string,
): Promise<RegisteredDirectSnapshot> {
  canonicalKey(controllerProgram, 'controller program');
  const scan = await client.programHeaders(CLAIM_PROGRAM_ID.toBase58());
  const candidates = scan.accounts.filter((entry) => {
    try { return entry.account.space === REGISTERED_STATE_BYTES_VALUE && ascii(entry.account.data, 0, 8) === 'DCLTRGI1'; } catch { return false; }
  });
  if (candidates.length > MAX_REGISTERED_STATES) throw new Error(`registered scan exceeds the explicit ${MAX_REGISTERED_STATES}-state browser bound`);
  const projected = await concurrentMap(candidates, async (entry): Promise<RegisteredDirectStateObservation | RefusedRegisteredDirectState> => {
    try {
      return await observeRegisteredDirectState(client, controllerProgram, entry.address, scan.slot);
    } catch (error) {
      return Object.freeze({ status: 'refused', address: entry.address, observedSlot: scan.slot, reason: error instanceof Error ? error.message : 'registered decoder refused the account' });
    }
  });
  return Object.freeze({
    scanSlot: scan.slot,
    states: Object.freeze(projected.filter((entry): entry is RegisteredDirectStateObservation => entry.status === 'accepted')),
    refused: Object.freeze(projected.filter((entry): entry is RefusedRegisteredDirectState => entry.status === 'refused')),
  });
}

export function encodeRegisteredFillInstructionV1(fill: bigint, executionPrice: bigint, bumps: readonly [number, number, number, number, number]): Uint8Array {
  const output = new Uint8Array(REGISTERED_CONTROLLER_BYTES_VALUE);
  output.set(REGISTERED_CONTROLLER_MAGIC_BYTES, REGISTERED_CONTROLLER_MAGIC_OFFSET);
  putU16(output, REGISTERED_CONTROLLER_VERSION_OFFSET, REGISTERED_CONTROLLER_ABI_VERSION);
  output[REGISTERED_CONTROLLER_BUMP_OFFSET] = bumps[0];
  output[REGISTERED_SELLER_REGISTRATION_BUMP_OFFSET] = bumps[1];
  output[REGISTERED_BUYER_REGISTRATION_BUMP_OFFSET] = bumps[2];
  output[REGISTERED_SELLER_POSITION_BUMP_OFFSET] = bumps[3];
  output[REGISTERED_BUYER_POSITION_BUMP_OFFSET] = bumps[4];
  output[REGISTERED_CONTROLLER_RESERVED_OFFSET] = 0;
  putU64(output, REGISTERED_CONTROLLER_FILL_OFFSET, fill, 'fill');
  putU64(output, REGISTERED_CONTROLLER_EXECUTION_PRICE_OFFSET, executionPrice, 'execution price');
  return output;
}

export function encodeRegisteredTerminal(action: 'cancel' | 'expire', controllerBump: number, registrationBump: number, sequence: bigint): Uint8Array {
  const output = new Uint8Array(REGISTERED_TERMINAL_BYTES_VALUE);
  output.set(REGISTERED_TERMINAL_MAGIC_BYTES, REGISTERED_TERMINAL_MAGIC_OFFSET);
  putU16(output, REGISTERED_TERMINAL_VERSION_OFFSET, REGISTERED_TERMINAL_ABI_VERSION);
  output[REGISTERED_TERMINAL_ACTION_OFFSET] = action === 'cancel' ? REGISTERED_TERMINAL_CANCEL : REGISTERED_TERMINAL_EXPIRE;
  output[REGISTERED_TERMINAL_CONTROLLER_BUMP_OFFSET] = controllerBump;
  output[REGISTERED_TERMINAL_REGISTRATION_BUMP_OFFSET] = registrationBump;
  requireZero(output, REGISTERED_TERMINAL_RESERVED_OFFSET, REGISTERED_TERMINAL_EXPECTED_SEQUENCE_OFFSET - REGISTERED_TERMINAL_RESERVED_OFFSET, 'registered terminal header');
  putU64(output, REGISTERED_TERMINAL_EXPECTED_SEQUENCE_OFFSET, sequence, 'expected sequence');
  return output;
}

function transactionPlan(instruction: TransactionInstruction, payer: PublicKey, recentBlockhash: string): RegisteredTransactionPlanV1 {
  canonicalKey(recentBlockhash, 'recent blockhash');
  const message = new TransactionMessage({ payerKey: payer, recentBlockhash, instructions: [instruction] }).compileToV0Message();
  const transaction = new VersionedTransaction(message);
  const wireBytes = transaction.serialize();
  if (wireBytes.length > PACKET_DATA_SIZE) throw new Error(`registered transaction is ${wireBytes.length} bytes, above the ${PACKET_DATA_SIZE}-byte packet bound`);
  return Object.freeze({
    instruction,
    transaction,
    wireBytes,
    requiredSignerKeys: Object.freeze(message.staticAccountKeys.slice(0, message.header.numRequiredSignatures).map((key) => key.toBase58())),
  });
}

export function buildRegisteredFillTransaction(input: RegisteredFillInputV1): RegisteredTransactionPlanV1 {
  const controllerProgram = canonicalKey(input.controllerProgram, 'controller program');
  const payer = canonicalKey(input.payer, 'payer');
  const seller = input.seller.state;
  const buyer = input.buyer.state;
  if (seller.phase !== 0 || buyer.phase !== 0 || seller.remaining === 0n || buyer.remaining === 0n) throw new Error('registered fill requires two open positive-residual states');
  if (seller.intent.side !== 0 || buyer.intent.side !== 1 || seller.intent.lifecycle > 2 || buyer.intent.lifecycle > 2) throw new Error('registered fill requires canonical seller/buyer lifecycle roles');
  if (seller.maker === buyer.maker || seller.intent.market.some((byte, index) => byte !== buyer.intent.market[index])
      || seller.intent.generation !== buyer.intent.generation || seller.intent.outcome !== buyer.intent.outcome
      || seller.intent.feeBasisPoints !== buyer.intent.feeBasisPoints) throw new Error('registered fill states do not share exact Market/outcome/generation/fee coordinates');
  if (input.fill === 0n || input.fill > seller.remaining || input.fill > buyer.remaining) throw new Error('registered fill exceeds positive residual capacity');
  if ((seller.intent.lifecycle === 0 && input.fill !== seller.remaining)
      || (buyer.intent.lifecycle === 0 && input.fill !== buyer.remaining)) throw new Error('registered FOK residual requires exact full consumption');
  if (input.executionPrice < seller.intent.limitPrice || input.executionPrice > buyer.intent.limitPrice) throw new Error('execution price is outside the signed spread');
  if (new PublicKey(seller.intent.collateralAccount).toBase58() !== input.route.sellerDestination
      || new PublicKey(buyer.intent.collateralAccount).toBase58() !== input.route.source) throw new Error('registered route substitutes a signed collateral account');
  const [controller, controllerBump] = PublicKey.findProgramAddressSync([CONTROLLER_SEED], controllerProgram);
  const sellerRegistration = deriveRegisteredAddress(input.controllerProgram, seller);
  const buyerRegistration = deriveRegisteredAddress(input.controllerProgram, buyer);
  if (sellerRegistration.address.toBase58() !== input.seller.address || buyerRegistration.address.toBase58() !== input.buyer.address) throw new Error('registered observation address no longer matches state coordinates');
  const market = new PublicKey(seller.intent.market);
  const [sellerPosition, sellerPositionBump] = PublicKey.findProgramAddressSync([POSITION_SEED, market.toBytes(), new PublicKey(seller.maker).toBytes(), Uint8Array.of(seller.intent.outcome)], controllerProgram);
  const [buyerPosition, buyerPositionBump] = PublicKey.findProgramAddressSync([POSITION_SEED, market.toBytes(), new PublicKey(buyer.maker).toBytes(), Uint8Array.of(buyer.intent.outcome)], controllerProgram);
  const data = encodeRegisteredFillInstructionV1(input.fill, input.executionPrice, [controllerBump, sellerRegistration.bump, buyerRegistration.bump, sellerPositionBump, buyerPositionBump]);
  const key = (text: string, field: string) => canonicalKey(text, field);
  const instruction = new TransactionInstruction({
    programId: controllerProgram,
    keys: [
      { pubkey: controller, isSigner: false, isWritable: false },
      { pubkey: sellerRegistration.address, isSigner: false, isWritable: true },
      { pubkey: buyerRegistration.address, isSigner: false, isWritable: true },
      { pubkey: key(input.route.journal, 'journal'), isSigner: false, isWritable: true },
      { pubkey: sellerPosition, isSigner: false, isWritable: true },
      { pubkey: buyerPosition, isSigner: false, isWritable: true },
      { pubkey: CLAIM_PROGRAM_ID, isSigner: false, isWritable: false },
      { pubkey: CUSTODY_PROGRAM_ID, isSigner: false, isWritable: false },
      { pubkey: market, isSigner: false, isWritable: false },
      { pubkey: key(input.route.realm, 'Realm'), isSigner: false, isWritable: false },
      { pubkey: key(input.route.feePolicy, 'fee policy'), isSigner: false, isWritable: false },
      { pubkey: key(input.route.capabilityManifest, 'capability manifest'), isSigner: false, isWritable: false },
      { pubkey: key(input.route.mint, 'mint'), isSigner: false, isWritable: false },
      { pubkey: key(input.route.source, 'source'), isSigner: false, isWritable: true },
      { pubkey: key(input.route.sellerDestination, 'seller destination'), isSigner: false, isWritable: true },
      { pubkey: key(input.route.feeDestination, 'fee destination'), isSigner: false, isWritable: true },
      { pubkey: key(input.route.tokenProgram, 'token program'), isSigner: false, isWritable: false },
    ],
    data: data as Buffer,
  });
  return transactionPlan(instruction, payer, input.recentBlockhash);
}

export function buildRegisteredTerminalTransaction(input: RegisteredTerminalInputV1): RegisteredTransactionPlanV1 {
  const controllerProgram = canonicalKey(input.controllerProgram, 'controller program');
  const payer = canonicalKey(input.payer, 'payer');
  const state = input.state.state;
  if (state.phase !== 0 || state.remaining === 0n) throw new Error('registered terminal action requires one open positive-residual state');
  if (input.action === 'expire' && input.finalizedSlot <= state.intent.validThrough) throw new Error('permissionless expiry is not yet admitted by finalized Clock state');
  const [controller, controllerBump] = PublicKey.findProgramAddressSync([CONTROLLER_SEED], controllerProgram);
  const registration = deriveRegisteredAddress(input.controllerProgram, state);
  if (registration.address.toBase58() !== input.state.address || registration.bump !== input.state.bump) throw new Error('registered observation no longer matches exact PDA coordinates');
  const keys = [
    { pubkey: controller, isSigner: false, isWritable: false },
    { pubkey: registration.address, isSigner: false, isWritable: true },
  ];
  if (input.action === 'cancel') keys.push({ pubkey: canonicalKey(state.maker, 'maker'), isSigner: true, isWritable: false });
  keys.push({ pubkey: CLAIM_PROGRAM_ID, isSigner: false, isWritable: false });
  const instruction = new TransactionInstruction({
    programId: controllerProgram,
    keys,
    data: encodeRegisteredTerminal(input.action, controllerBump, registration.bump, state.sequence) as Buffer,
  });
  return transactionPlan(instruction, payer, input.recentBlockhash);
}
