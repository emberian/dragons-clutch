import {
  PublicKey,
  SystemProgram,
  TransactionInstruction,
  TransactionMessage,
  VersionedTransaction,
} from '@solana/web3.js';

import { ascii, isZero, requireZero, slice, u16, u64 } from './bytes';
import { decodeCompactIntentV1, encodeCompactIntentV1, type CompactIntentV1 } from './directCodec';
import { CLAIM_PROGRAM_ID, CONTROLLER_SEED, CUSTODY_PROGRAM_ID, PACKET_DATA_SIZE, POSITION_SEED, REPLAY_SEED } from './directTransaction';
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
  REGISTERED_CREATE_ABI_VERSION,
  REGISTERED_CREATE_BYTES_VALUE,
  REGISTERED_CREATE_CONTROLLER_BUMP_OFFSET,
  REGISTERED_CREATE_INTENT_OFFSET,
  REGISTERED_CREATE_MAGIC_BYTES,
  REGISTERED_CREATE_MAGIC_OFFSET,
  REGISTERED_CREATE_REGISTRATION_BUMP_OFFSET,
  REGISTERED_CREATE_REPLAY_BUMP_OFFSET,
  REGISTERED_CREATE_RESERVED_OFFSET,
  REGISTERED_CREATE_VERSION_OFFSET,
  REGISTERED_RETIRE_ABI_VERSION,
  REGISTERED_RETIRE_BYTES_VALUE,
  REGISTERED_RETIRE_CONTROLLER_BUMP_OFFSET,
  REGISTERED_RETIRE_MAGIC_BYTES,
  REGISTERED_RETIRE_MAGIC_OFFSET,
  REGISTERED_RETIRE_REGISTRATION_BUMP_OFFSET,
  REGISTERED_RETIRE_RESERVED_OFFSET,
  REGISTERED_RETIRE_VERSION_OFFSET,
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
export const LEGACY_TOKEN_PROGRAM_ID = new PublicKey('TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA');
const MAX_REGISTERED_STATES = 128;
const TOKEN_ACCOUNT_BYTES = 165;
const PRICE_SCALE = 1_000_000n;
const FEE_SCALE = 10_000n;

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

export type RegisteredCreateRouteV1 = Readonly<{
  realm: string;
  feePolicy: string;
  capabilityManifest: string;
  mint: string;
  collateral: string;
  venue: string;
  tokenProgram: string;
}>;

export type RegisteredCreateAddressesV1 = Readonly<{
  controller: PublicKey;
  controllerBump: number;
  replay: PublicKey;
  replayBump: number;
  registration: PublicKey;
  registrationBump: number;
}>;

export type MakerReplayObservationV1 = Readonly<{
  exists: boolean;
  nextNonce: bigint;
}>;

export type LegacyTokenObservationV1 = Readonly<{
  mint: string;
  owner: string;
  amount: bigint;
  delegate: string | null;
  delegatedAmount: bigint;
  frozen: boolean;
}>;

export type RegisteredRetirementDelegationV1 = 'seller-not-applicable' | 'revoke-registration' | 'already-revoked';

export type RegisteredCreateInputV1 = Readonly<{
  controllerProgram: string;
  payer: string;
  maker: string;
  market: string;
  recentBlockhash: string;
  intent: CompactIntentV1;
  expectedNonce: bigint;
  route: RegisteredCreateRouteV1;
}>;

export type RegisteredCreateTransactionPlanV1 = Readonly<{
  instructions: ReadonlyArray<TransactionInstruction>;
  transaction: VersionedTransaction;
  wireBytes: Uint8Array;
  requiredSignerKeys: ReadonlyArray<string>;
  derived: RegisteredCreateAddressesV1;
  approvalAmount: bigint | null;
}>;

export type RegisteredRetireInputV1 = Readonly<{
  controllerProgram: string;
  payer: string;
  recentBlockhash: string;
  state: RegisteredDirectStateObservation;
}>;

export type RegisteredRetireTransactionPlanV1 = Readonly<{
  instruction: TransactionInstruction;
  transaction: VersionedTransaction;
  wireBytes: Uint8Array;
  requiredSignerKeys: ReadonlyArray<string>;
  rentDestination: string;
  tokenAction: 'none' | 'revoke-or-confirm-absent';
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

function u32At(bytes: Uint8Array, offset: number): number {
  if (offset < 0 || offset + 4 > bytes.length) throw new Error('u32 field is truncated');
  return new DataView(bytes.buffer, bytes.byteOffset + offset, 4).getUint32(0, true);
}

function sameBytes(left: Uint8Array, right: Uint8Array): boolean {
  return left.length === right.length && left.every((value, index) => value === right[index]);
}

/** Project only the legacy SPL-token fields needed to make delegation visible. */
export function decodeLegacyTokenObservationV1(account: RpcAccount): LegacyTokenObservationV1 {
  if (account.owner !== LEGACY_TOKEN_PROGRAM_ID.toBase58() || account.executable || account.data.length !== TOKEN_ACCOUNT_BYTES) {
    throw new Error('collateral must be one exact initialized legacy SPL-token account');
  }
  if (account.data[108] !== 1 && account.data[108] !== 2) throw new Error('collateral token account state is uninitialized or undefined');
  const delegateTag = u32At(account.data, 72);
  if (delegateTag > 1) throw new Error('collateral token delegate option is noncanonical');
  const delegate = delegateTag === 0 ? null : new PublicKey(slice(account.data, 76, 32)).toBase58();
  const delegatedAmount = u64(account.data, 121);
  if (delegate === null && delegatedAmount !== 0n) throw new Error('collateral has a delegated amount without a delegate');
  const nativeTag = u32At(account.data, 109);
  if (nativeTag > 1) throw new Error('collateral native-reserve option is noncanonical');
  if (nativeTag !== 0) throw new Error('native-wrapped collateral is not admitted by registered retirement');
  return Object.freeze({
    mint: new PublicKey(slice(account.data, 0, 32)).toBase58(),
    owner: new PublicKey(slice(account.data, 32, 32)).toBase58(),
    amount: u64(account.data, 64),
    delegate,
    delegatedAmount,
    frozen: account.data[108] === 2,
  });
}

export function registeredBuyerReserve(maximumFill: bigint, limitPrice: bigint, feeBasisPoints: number): bigint {
  if (maximumFill <= 0n || maximumFill > 18_446_744_073_709_551_615n) throw new Error('maximum fill must be a positive u64');
  if (limitPrice < 0n || limitPrice > PRICE_SCALE) throw new Error('limit price exceeds the exact 1e6 scale');
  if (!Number.isInteger(feeBasisPoints) || feeBasisPoints < 0 || feeBasisPoints > 10_000) throw new Error('fee basis points exceed the exact denominator');
  const gross = maximumFill * limitPrice / PRICE_SCALE;
  const reserve = gross + gross * BigInt(feeBasisPoints) / FEE_SCALE;
  if (reserve > 18_446_744_073_709_551_615n) throw new Error('buyer approval reserve exceeds u64');
  if (reserve === 0n) throw new Error('buyer order has zero worst-case collateral reserve');
  return reserve;
}

export function registeredRetirementDelegation(
  state: RegisteredDirectStateObservation,
  token: LegacyTokenObservationV1 | null,
): RegisteredRetirementDelegationV1 {
  if (state.state.intent.side === 0) {
    if (token !== null) throw new Error('seller retirement must not acquire a token-delegation route');
    return 'seller-not-applicable';
  }
  if (state.state.intent.side !== 1 || token === null) throw new Error('buyer retirement requires its exact collateral token state');
  if (token.owner !== state.state.maker) throw new Error('buyer collateral owner is not the persisted maker');
  if (token.delegate === state.address) {
    if (token.frozen) throw new Error('buyer collateral is frozen, so SPL Token would refuse delegation revoke');
    return 'revoke-registration';
  }
  if (token.delegate === null && token.delegatedAmount === 0n) return 'already-revoked';
  throw new Error('buyer collateral delegation names a substituted authority');
}

export function deriveRegisteredCreateAddresses(
  controllerProgramText: string,
  marketText: string,
  generation: bigint,
  makerText: string,
  nonce: bigint,
): RegisteredCreateAddressesV1 {
  const controllerProgram = canonicalKey(controllerProgramText, 'controller program');
  const market = canonicalKey(marketText, 'Market');
  const maker = canonicalKey(makerText, 'maker');
  const generationBytes = u64Bytes(generation, 'generation');
  const nonceBytes = u64Bytes(nonce, 'nonce');
  const [controller, controllerBump] = PublicKey.findProgramAddressSync([CONTROLLER_SEED], controllerProgram);
  const [replay, replayBump] = PublicKey.findProgramAddressSync([REPLAY_SEED, market.toBytes(), generationBytes, maker.toBytes()], controllerProgram);
  const [registration, registrationBump] = PublicKey.findProgramAddressSync([REGISTERED_SEED, market.toBytes(), generationBytes, maker.toBytes(), nonceBytes], controllerProgram);
  return Object.freeze({ controller, controllerBump, replay, replayBump, registration, registrationBump });
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

export function encodeRegisteredCreateInstructionV1(
  intent: CompactIntentV1,
  controllerBump: number,
  replayBump: number,
  registrationBump: number,
): Uint8Array {
  const output = new Uint8Array(REGISTERED_CREATE_BYTES_VALUE);
  output.set(REGISTERED_CREATE_MAGIC_BYTES, REGISTERED_CREATE_MAGIC_OFFSET);
  putU16(output, REGISTERED_CREATE_VERSION_OFFSET, REGISTERED_CREATE_ABI_VERSION);
  output[REGISTERED_CREATE_CONTROLLER_BUMP_OFFSET] = controllerBump;
  output[REGISTERED_CREATE_REPLAY_BUMP_OFFSET] = replayBump;
  output[REGISTERED_CREATE_REGISTRATION_BUMP_OFFSET] = registrationBump;
  requireZero(output, REGISTERED_CREATE_RESERVED_OFFSET, REGISTERED_CREATE_INTENT_OFFSET - REGISTERED_CREATE_RESERVED_OFFSET, 'registered creation header');
  output.set(encodeCompactIntentV1(intent), REGISTERED_CREATE_INTENT_OFFSET);
  return output;
}

export function encodeLegacyApproveInstructionV1(amount: bigint): Uint8Array {
  const output = new Uint8Array(9);
  output[0] = 4;
  putU64(output, 1, amount, 'approval amount');
  return output;
}

export function encodeRegisteredRetireInstructionV1(controllerBump: number, registrationBump: number): Uint8Array {
  const output = new Uint8Array(REGISTERED_RETIRE_BYTES_VALUE);
  output.set(REGISTERED_RETIRE_MAGIC_BYTES, REGISTERED_RETIRE_MAGIC_OFFSET);
  putU16(output, REGISTERED_RETIRE_VERSION_OFFSET, REGISTERED_RETIRE_ABI_VERSION);
  output[REGISTERED_RETIRE_CONTROLLER_BUMP_OFFSET] = controllerBump;
  output[REGISTERED_RETIRE_REGISTRATION_BUMP_OFFSET] = registrationBump;
  requireZero(output, REGISTERED_RETIRE_RESERVED_OFFSET, REGISTERED_RETIRE_BYTES_VALUE - REGISTERED_RETIRE_RESERVED_OFFSET, 'registered retirement tail');
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

export function buildRegisteredCreateTransaction(input: RegisteredCreateInputV1): RegisteredCreateTransactionPlanV1 {
  const controllerProgram = canonicalKey(input.controllerProgram, 'controller program');
  const payer = canonicalKey(input.payer, 'payer');
  const maker = canonicalKey(input.maker, 'maker');
  const market = canonicalKey(input.market, 'Market');
  canonicalKey(input.recentBlockhash, 'recent blockhash');
  const intent = input.intent;
  if (!sameBytes(intent.market, market.toBytes())) throw new Error('registered intent substitutes the selected Market');
  if (intent.lifecycle !== 2) throw new Error('registered creation requires lifecycle 2');
  if (intent.nonce !== input.expectedNonce) throw new Error('registered intent nonce is stale relative to the reacquired replay root');
  if (intent.side > 1) throw new Error('registered intent side is undefined');
  if (intent.validFrom > intent.validThrough || intent.maximumFill === 0n) throw new Error('registered validity or maximum fill is invalid');
  const derived = deriveRegisteredCreateAddresses(input.controllerProgram, input.market, intent.generation, input.maker, intent.nonce);
  const route = input.route;
  const keys = {
    realm: canonicalKey(route.realm, 'Realm'),
    feePolicy: canonicalKey(route.feePolicy, 'fee policy'),
    capabilityManifest: canonicalKey(route.capabilityManifest, 'capability manifest'),
    mint: canonicalKey(route.mint, 'mint'),
    collateral: canonicalKey(route.collateral, 'collateral'),
    venue: canonicalKey(route.venue, 'venue'),
    tokenProgram: canonicalKey(route.tokenProgram, 'token program'),
  };
  if (!keys.tokenProgram.equals(LEGACY_TOKEN_PROGRAM_ID)) throw new Error('registered creation supports only the controller’s exact legacy-token profile');
  const fixed = [
    derived.controller, derived.replay, derived.registration, CLAIM_PROGRAM_ID, SystemProgram.programId,
    market, keys.realm, keys.feePolicy, keys.capabilityManifest, keys.mint, keys.collateral, keys.venue, keys.tokenProgram,
  ];
  if (new Set(fixed.map((key) => key.toBase58())).size !== fixed.length) throw new Error('registered creation aliases two fixed account roles');
  if (fixed.some((key) => key.equals(maker) || key.equals(payer))) throw new Error('maker or payer aliases a fixed registered-creation role');
  const create = new TransactionInstruction({
    programId: controllerProgram,
    keys: [
      { pubkey: derived.controller, isSigner: false, isWritable: false },
      { pubkey: maker, isSigner: true, isWritable: false },
      { pubkey: payer, isSigner: true, isWritable: true },
      { pubkey: derived.replay, isSigner: false, isWritable: true },
      { pubkey: derived.registration, isSigner: false, isWritable: true },
      { pubkey: CLAIM_PROGRAM_ID, isSigner: false, isWritable: false },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
      { pubkey: market, isSigner: false, isWritable: false },
      { pubkey: keys.realm, isSigner: false, isWritable: false },
      { pubkey: keys.feePolicy, isSigner: false, isWritable: false },
      { pubkey: keys.capabilityManifest, isSigner: false, isWritable: false },
      { pubkey: keys.mint, isSigner: false, isWritable: false },
      { pubkey: keys.collateral, isSigner: false, isWritable: false },
      { pubkey: keys.venue, isSigner: false, isWritable: false },
      { pubkey: keys.tokenProgram, isSigner: false, isWritable: false },
    ],
    data: encodeRegisteredCreateInstructionV1(intent, derived.controllerBump, derived.replayBump, derived.registrationBump) as Buffer,
  });
  let approvalAmount: bigint | null = null;
  const instructions: TransactionInstruction[] = [];
  if (intent.side === 1) {
    approvalAmount = registeredBuyerReserve(intent.maximumFill, intent.limitPrice, intent.feeBasisPoints);
    instructions.push(new TransactionInstruction({
      programId: LEGACY_TOKEN_PROGRAM_ID,
      keys: [
        { pubkey: keys.collateral, isSigner: false, isWritable: true },
        { pubkey: derived.registration, isSigner: false, isWritable: false },
        { pubkey: maker, isSigner: true, isWritable: false },
      ],
      data: encodeLegacyApproveInstructionV1(approvalAmount) as Buffer,
    }));
  }
  instructions.push(create);
  const message = new TransactionMessage({ payerKey: payer, recentBlockhash: input.recentBlockhash, instructions }).compileToV0Message();
  const transaction = new VersionedTransaction(message);
  const wireBytes = transaction.serialize();
  if (wireBytes.length > PACKET_DATA_SIZE) throw new Error(`registered creation transaction is ${wireBytes.length} bytes, above the ${PACKET_DATA_SIZE}-byte packet bound`);
  return Object.freeze({
    instructions: Object.freeze(instructions), transaction, wireBytes,
    requiredSignerKeys: Object.freeze(message.staticAccountKeys.slice(0, message.header.numRequiredSignatures).map((key) => key.toBase58())),
    derived, approvalAmount,
  });
}

export function buildRegisteredRetireTransaction(input: RegisteredRetireInputV1): RegisteredRetireTransactionPlanV1 {
  const controllerProgram = canonicalKey(input.controllerProgram, 'controller program');
  const payer = canonicalKey(input.payer, 'payer');
  canonicalKey(input.recentBlockhash, 'recent blockhash');
  const state = input.state.state;
  if (state.phase === 0 || state.phase > 3) throw new Error('registered retirement requires a canonical terminal phase');
  if (state.phase === 1 && state.remaining !== 0n) throw new Error('filled registration retains a nonzero residual');
  if (state.remaining > state.intent.maximumFill || state.intent.maximumFill === 0n || state.intent.validFrom > state.intent.validThrough) {
    throw new Error('registered terminal state has invalid residual, capacity, or validity facts');
  }
  const maker = canonicalKey(state.maker, 'persisted maker');
  const [controller, controllerBump] = PublicKey.findProgramAddressSync([CONTROLLER_SEED], controllerProgram);
  if (state.controller !== controller.toBase58()) throw new Error('terminal registration names a noncanonical controller authority');
  const registration = deriveRegisteredAddress(input.controllerProgram, state);
  if (registration.address.toBase58() !== input.state.address || registration.bump !== input.state.bump) throw new Error('terminal registration address no longer matches exact PDA coordinates');
  const fixed = [controller, registration.address, maker, CLAIM_PROGRAM_ID];
  if (new Set(fixed.map((key) => key.toBase58())).size !== fixed.length) throw new Error('registered retirement aliases two exact account roles');
  if (!payer.equals(maker) && fixed.some((key) => key.equals(payer))) throw new Error('fee payer aliases a non-maker retirement role');
  const keys = [
    { pubkey: controller, isSigner: false, isWritable: false },
    { pubkey: registration.address, isSigner: false, isWritable: true },
    { pubkey: maker, isSigner: state.intent.side === 1, isWritable: true },
    { pubkey: CLAIM_PROGRAM_ID, isSigner: false, isWritable: false },
  ];
  let tokenAction: 'none' | 'revoke-or-confirm-absent' = 'none';
  if (state.intent.side === 1) {
    const collateral = new PublicKey(state.intent.collateralAccount);
    if ([controller, registration.address, maker, CLAIM_PROGRAM_ID, LEGACY_TOKEN_PROGRAM_ID].some((key) => key.equals(collateral))) {
      throw new Error('buyer collateral aliases a retirement account role');
    }
    keys.push({ pubkey: collateral, isSigner: false, isWritable: true });
    keys.push({ pubkey: LEGACY_TOKEN_PROGRAM_ID, isSigner: false, isWritable: false });
    tokenAction = 'revoke-or-confirm-absent';
  } else if (state.intent.side !== 0) {
    throw new Error('registered retirement side is undefined');
  }
  const instruction = new TransactionInstruction({
    programId: controllerProgram,
    keys,
    data: encodeRegisteredRetireInstructionV1(controllerBump, registration.bump) as Buffer,
  });
  const message = new TransactionMessage({ payerKey: payer, recentBlockhash: input.recentBlockhash, instructions: [instruction] }).compileToV0Message();
  const transaction = new VersionedTransaction(message);
  const wireBytes = transaction.serialize();
  if (wireBytes.length > PACKET_DATA_SIZE) throw new Error(`registered retirement transaction is ${wireBytes.length} bytes, above the ${PACKET_DATA_SIZE}-byte packet bound`);
  return Object.freeze({
    instruction, transaction, wireBytes,
    requiredSignerKeys: Object.freeze(message.staticAccountKeys.slice(0, message.header.numRequiredSignatures).map((key) => key.toBase58())),
    rentDestination: maker.toBase58(), tokenAction,
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
