import {
  PublicKey,
  TransactionInstruction,
  TransactionMessage,
  VersionedTransaction,
} from '@solana/web3.js';

import { ascii, hex, isZero, requireZero, sha256, slice, u16, u64 } from './bytes';
import { decodeCoreAccount, verifyLocalBindings, type FullAccountObservation } from './decoders';
import { LEGACY_TOKEN_PROGRAM_ID, decodeLegacyTokenObservationV1 } from './registeredDirect';
import { PACKET_DATA_SIZE } from './directTransaction';
import { type RpcAccount, type SolanaRpcClient } from './rpc';

export const ECONOMIC_PROJECTION_BYTES = 1_136;
export const ECONOMIC_FOUNDING_BYTES = 208;
export const ECONOMIC_OPERATION_BYTES = 32;
export const EXECUTION_RELEASE_SET_BYTES = 336;
export const ECONOMIC_HOARD_SEED = new TextEncoder().encode('dclutch-economic-hoard-v1');

const MAX_U64 = 18_446_744_073_709_551_615n;
const MARKET_IDENTITY_OFFSET = 32;
const MARKET_IDENTITY_BYTES = 168;

export type EconomicPhase = 'open' | 'terminal' | 'retiring' | 'retired';
export type EconomicHolder = 'source' | 'destination';
export type EconomicRepresentation = 'native' | 'materialized';
export type EconomicAction = 'split' | 'merge' | 'materialize' | 'dematerialize' | 'redeem';
export type EconomicRole = 'core' | 'claims' | 'trading' | 'resolution' | 'custody';

export type EconomicStateV1 = Readonly<{
  outcomeCount: number;
  phase: EconomicPhase;
  winner: number | null;
  hoard: bigint;
  supply: ReadonlyArray<bigint>;
  nativeSupply: ReadonlyArray<bigint>;
  materializedSupply: ReadonlyArray<bigint>;
  sourceNative: ReadonlyArray<bigint>;
  sourceMaterialized: ReadonlyArray<bigint>;
  destinationNative: ReadonlyArray<bigint>;
  destinationMaterialized: ReadonlyArray<bigint>;
}>;

export type EconomicProjectionV1 = Readonly<{
  marketId: Uint8Array;
  releaseSetId: Uint8Array;
  sourceHolder: string;
  destinationHolder: string;
  collateralMint: string;
  hoardAccount: string;
  revision: bigint;
  state: EconomicStateV1;
}>;

export type EconomicProjectionObservationV1 = Readonly<{
  status: 'founded';
  address: string;
  observedSlot: string;
  lamports: string;
  projection: EconomicProjectionV1;
}>;

export type EconomicVacancyObservationV1 = Readonly<{
  status: 'vacant';
  address: string;
  observedSlot: string;
  lamports: string;
}>;

export type RefusedEconomicProjectionV1 = Readonly<{
  status: 'refused';
  address: string;
  observedSlot: string;
  reason: string;
}>;

export type EconomicSnapshotV1 = Readonly<{
  scanSlot: string;
  founded: ReadonlyArray<EconomicProjectionObservationV1>;
  vacant: ReadonlyArray<EconomicVacancyObservationV1>;
  refused: ReadonlyArray<RefusedEconomicProjectionV1>;
}>;

export type ReleaseBindingV1 = Readonly<{ program: string; artifactRelease: string }>;
export type ExecutionReleaseSetV1 = Readonly<{
  digest: Uint8Array;
  owner: string;
  roles: Readonly<Record<EconomicRole, ReleaseBindingV1>>;
}>;

export type EconomicOperationV1 = Readonly<{
  action: EconomicAction;
  holder: EconomicHolder;
  representation: EconomicRepresentation;
  outcome: number;
  quantity: bigint;
  expectedRevision: bigint;
}>;

export type EconomicClaimEffectV1 = Readonly<{
  operation: 'debit' | 'credit';
  holder: EconomicHolder;
  outcome: number;
  amount: bigint;
}>;

export type EconomicCustodyEffectV1 = Readonly<{
  source: EconomicHolder | 'hoard';
  destination: EconomicHolder | 'hoard';
  amount: bigint;
}>;

export type EconomicSimulationV1 = Readonly<{
  nextState: EconomicStateV1;
  claims: ReadonlyArray<EconomicClaimEffectV1>;
  custody: EconomicCustodyEffectV1 | null;
  admissionRole: 'trading' | 'resolution';
}>;

export type EconomicTransactionPlanV1 = Readonly<{
  instruction: TransactionInstruction;
  transaction: VersionedTransaction;
  wireBytes: Uint8Array;
  requiredSignerKeys: ReadonlyArray<string>;
  simulation: EconomicSimulationV1;
}>;

function key(text: string, field: string): PublicKey {
  const parsed = new PublicKey(text);
  if (parsed.toBase58() !== text) throw new Error(`${field} must be canonical base58 text`);
  return parsed;
}

function putU64(output: Uint8Array, offset: number, value: bigint, field: string): void {
  if (value < 0n || value > MAX_U64) throw new Error(`${field} is not a u64`);
  new DataView(output.buffer, output.byteOffset + offset, 8).setBigUint64(0, value, true);
}

function same(left: Uint8Array, right: Uint8Array): boolean {
  return left.length === right.length && left.every((value, index) => value === right[index]);
}

function allZero(bytes: Uint8Array): boolean {
  return bytes.every((byte) => byte === 0);
}

function vector(bytes: Uint8Array, offset: number, width: number): ReadonlyArray<bigint> {
  return Object.freeze(Array.from({ length: width }, (_, index) => u64(bytes, offset + index * 8)));
}

function validateEconomicState(state: EconomicStateV1): void {
  const width = state.outcomeCount;
  if (!Number.isInteger(width) || width < 1 || width > 16) throw new Error('economic outcome count is outside 1..16');
  const vectors = [state.supply, state.nativeSupply, state.materializedSupply, state.sourceNative, state.sourceMaterialized, state.destinationNative, state.destinationMaterialized];
  if (vectors.some((values) => values.length !== width)) throw new Error('economic vector width is noncanonical');
  for (let outcome = 0; outcome < width; outcome += 1) {
    if (state.nativeSupply[outcome] + state.materializedSupply[outcome] !== state.supply[outcome]) throw new Error('representation supply does not partition conservative supply');
    if (state.sourceNative[outcome] + state.destinationNative[outcome] > state.nativeSupply[outcome]
        || state.sourceMaterialized[outcome] + state.destinationMaterialized[outcome] > state.materializedSupply[outcome]) throw new Error('holder projection exceeds representation supply');
    if (state.phase === 'open' && state.supply[outcome] > state.hoard) throw new Error('open economic state is undercollateralized');
  }
  if ((state.phase === 'terminal' || state.phase === 'retiring')) {
    if (state.winner === null || state.winner < 0 || state.winner >= width) throw new Error('terminal winner is outside the active width');
    if (state.supply[state.winner] > state.hoard) throw new Error('terminal winning liabilities exceed Hoard principal');
  } else if (state.winner !== null) throw new Error('open or retired state carries a winner');
  if (state.phase === 'retired' && (state.hoard !== 0n || state.supply.some((amount) => amount !== 0n))) throw new Error('retired economic state retains liabilities');
}

export function decodeEconomicStateV1(bytes: Uint8Array): EconomicStateV1 {
  if (bytes.length < 16 || ascii(bytes, 0, 4) !== 'DCES' || bytes[4] !== 1) throw new Error('economic state header is unsupported');
  const width = bytes[6];
  if (width < 1 || width > 16 || bytes.length !== 16 + width * 7 * 8) throw new Error('economic state has a noncanonical active width');
  const phaseNames: EconomicPhase[] = ['open', 'terminal', 'retiring', 'retired'];
  const phase = phaseNames[bytes[5]];
  if (phase === undefined) throw new Error('economic phase is undefined');
  const winner = phase === 'terminal' || phase === 'retiring' ? bytes[7] : null;
  if (winner === null && bytes[7] !== 0) throw new Error('nonterminal economic state carries winner bytes');
  let offset = 16;
  const values = Array.from({ length: 7 }, () => {
    const current = vector(bytes, offset, width);
    offset += width * 8;
    return current;
  });
  const state = Object.freeze({
    outcomeCount: width, phase, winner, hoard: u64(bytes, 8), supply: values[0], nativeSupply: values[1],
    materializedSupply: values[2], sourceNative: values[3], sourceMaterialized: values[4],
    destinationNative: values[5], destinationMaterialized: values[6],
  });
  validateEconomicState(state);
  return state;
}

export function decodeEconomicProjectionV1(bytes: Uint8Array): EconomicProjectionV1 {
  if (bytes.length !== ECONOMIC_PROJECTION_BYTES || ascii(bytes, 0, 8) !== 'DCLTECO1' || bytes[8] !== 1) throw new Error('economic projection has the wrong exact width, magic, or version');
  requireZero(bytes, 9, 7, 'economic projection header');
  requireZero(bytes, 218, 6, 'economic projection state header');
  const identities = [slice(bytes, 16, 32), slice(bytes, 48, 32), slice(bytes, 80, 32), slice(bytes, 112, 32), slice(bytes, 144, 32), slice(bytes, 176, 32)];
  if (identities.some(isZero) || same(identities[2], identities[3]) || same(identities[4], identities[5])) throw new Error('economic projection has zero or aliased immutable identities');
  const stateLength = u16(bytes, 216);
  if (stateLength > 912) throw new Error('economic projection state length exceeds the measured profile');
  const end = 224 + stateLength;
  if (!allZero(bytes.slice(end))) throw new Error('economic projection inactive capacity is nonzero');
  return Object.freeze({
    marketId: identities[0], releaseSetId: identities[1], sourceHolder: new PublicKey(identities[2]).toBase58(),
    destinationHolder: new PublicKey(identities[3]).toBase58(), collateralMint: new PublicKey(identities[4]).toBase58(),
    hoardAccount: new PublicKey(identities[5]).toBase58(), revision: u64(bytes, 208),
    state: decodeEconomicStateV1(bytes.slice(224, end)),
  });
}

export async function decodeExecutionReleaseSetV1(account: RpcAccount): Promise<ExecutionReleaseSetV1> {
  if (account.executable || account.data.length !== EXECUTION_RELEASE_SET_BYTES || ascii(account.data, 0, 8) !== 'DCLTRLS1'
      || u16(account.data, 8) !== 1 || u16(account.data, 10) !== 1) throw new Error('execution release set has the wrong exact layout or executable flag');
  requireZero(account.data, 12, 4, 'execution release-set header');
  const names: EconomicRole[] = ['core', 'claims', 'trading', 'resolution', 'custody'];
  const roles = Object.fromEntries(names.map((name, index) => {
    const offset = 16 + index * 64;
    const programBytes = slice(account.data, offset, 32);
    const release = slice(account.data, offset + 32, 32);
    if (isZero(programBytes) || isZero(release)) throw new Error(`execution ${name} binding is zero`);
    return [name, Object.freeze({ program: new PublicKey(programBytes).toBase58(), artifactRelease: hex(release) })];
  })) as Record<EconomicRole, ReleaseBindingV1>;
  const pairs = names.map((name) => `${roles[name].program}:${roles[name].artifactRelease}`);
  for (let left = 0; left < names.length; left += 1) for (let right = left + 1; right < names.length; right += 1) {
    const sameProgram = roles[names[left]].program === roles[names[right]].program;
    const sameRelease = roles[names[left]].artifactRelease === roles[names[right]].artifactRelease;
    if ((sameProgram || sameRelease) && pairs[left] !== pairs[right]) throw new Error('release set aliases a program or artifact across inconsistent role pairs');
  }
  if (account.owner !== roles.core.program) throw new Error('release-set account owner is not its selected Core program');
  return Object.freeze({ digest: await sha256(account.data), owner: account.owner, roles: Object.freeze(roles) });
}

export function authenticateEconomicRelease(release: ExecutionReleaseSetV1, economicProgram: string): void {
  key(economicProgram, 'economic program');
  if (release.roles.claims.program !== economicProgram || release.roles.custody.program !== economicProgram
      || release.roles.claims.artifactRelease !== release.roles.custody.artifactRelease) {
    throw new Error('economic program is not the release set’s identical Claims and Custody binding');
  }
}

export async function scanEconomicProjections(client: SolanaRpcClient, economicProgram: string): Promise<EconomicSnapshotV1> {
  key(economicProgram, 'economic program');
  const scan = await client.programHeaders(economicProgram);
  const candidates = scan.accounts.filter((entry) => entry.account.space === ECONOMIC_PROJECTION_BYTES);
  if (candidates.length > 128) throw new Error('economic projection scan exceeds the explicit 128-account browser bound');
  const projected = await Promise.all(candidates.map(async (entry): Promise<EconomicProjectionObservationV1 | EconomicVacancyObservationV1 | RefusedEconomicProjectionV1> => {
    try {
      const read = await client.accountInfo(entry.address, scan.slot);
      if (read.account === null) throw new Error('economic projection disappeared during finalized reacquisition');
      if (read.account.owner !== economicProgram || read.account.executable || read.account.data.length !== ECONOMIC_PROJECTION_BYTES) throw new Error('economic projection has the wrong owner, executable flag, or width');
      if (allZero(read.account.data)) return Object.freeze({ status: 'vacant', address: entry.address, observedSlot: read.slot, lamports: read.account.lamports });
      return Object.freeze({ status: 'founded', address: entry.address, observedSlot: read.slot, lamports: read.account.lamports, projection: decodeEconomicProjectionV1(read.account.data) });
    } catch (error) {
      return Object.freeze({ status: 'refused', address: entry.address, observedSlot: scan.slot, reason: error instanceof Error ? error.message : 'economic projection refused' });
    }
  }));
  return Object.freeze({
    scanSlot: scan.slot,
    founded: Object.freeze(projected.filter((entry): entry is EconomicProjectionObservationV1 => entry.status === 'founded')),
    vacant: Object.freeze(projected.filter((entry): entry is EconomicVacancyObservationV1 => entry.status === 'vacant')),
    refused: Object.freeze(projected.filter((entry): entry is RefusedEconomicProjectionV1 => entry.status === 'refused')),
  });
}

export type EconomicFoundingCoordinatesV1 = Readonly<{
  marketId: Uint8Array;
  outcomeCount: number;
  collateralMint: string;
  tokenProgram: string;
}>;

function fullObservation(address: string, slot: string, account: RpcAccount): FullAccountObservation {
  return Object.freeze({ address, owner: account.owner, executable: account.executable, lamports: account.lamports, observedSlot: slot, data: account.data });
}

export async function deriveEconomicFoundingCoordinates(
  coreProgram: string,
  marketAddress: string,
  marketAccount: RpcAccount,
  realmAddress: string,
  realmAccount: RpcAccount,
  observedSlot: string,
): Promise<EconomicFoundingCoordinatesV1> {
  const marketDecoded = await verifyLocalBindings(decodeCoreAccount(fullObservation(marketAddress, observedSlot, marketAccount), coreProgram), coreProgram);
  const realmDecoded = await verifyLocalBindings(decodeCoreAccount(fullObservation(realmAddress, observedSlot, realmAccount), coreProgram), coreProgram);
  if (marketDecoded.status !== 'decoded' || marketDecoded.semantics.kind !== 'Market' || marketDecoded.semantics.phase !== 'Open'
      || !marketDecoded.bindings.every((check) => check.ok)) throw new Error('founding Market is not a binding-clean Open Market');
  if (realmDecoded.status !== 'decoded' || realmDecoded.semantics.kind !== 'Realm' || realmDecoded.semantics.contentDigest !== marketDecoded.semantics.realmId
      || !realmDecoded.bindings.every((check) => check.ok)) throw new Error('founding Realm is not the Market’s canonical content-bound Realm');
  const marketId = await sha256(marketDecoded.semantics.identityBytes);
  if (!same(marketDecoded.semantics.identityBytes, marketAccount.data.slice(MARKET_IDENTITY_OFFSET, MARKET_IDENTITY_OFFSET + MARKET_IDENTITY_BYTES))) throw new Error('Market identity projection changed during founding derivation');
  return Object.freeze({
    marketId, outcomeCount: marketDecoded.semantics.outcomeCount,
    tokenProgram: new PublicKey(realmDecoded.semantics.canonicalBytes.slice(16, 48)).toBase58(),
    collateralMint: new PublicKey(realmDecoded.semantics.canonicalBytes.slice(48, 80)).toBase58(),
  });
}

export function encodeEconomicFoundingV1(input: Readonly<{
  marketId: Uint8Array;
  releaseSetId: Uint8Array;
  sourceHolder: string;
  destinationHolder: string;
  collateralMint: string;
  hoardAccount: string;
  outcomeCount: number;
}>): Uint8Array {
  const identities = [input.marketId, input.releaseSetId, key(input.sourceHolder, 'source holder').toBytes(), key(input.destinationHolder, 'destination holder').toBytes(), key(input.collateralMint, 'collateral mint').toBytes(), key(input.hoardAccount, 'Hoard account').toBytes()];
  if (identities.some((identity) => identity.length !== 32 || isZero(identity))) throw new Error('economic founding identities must be nonzero 32-byte values');
  if (same(identities[2], identities[3]) || same(identities[4], identities[5])) throw new Error('economic founding aliases immutable identities');
  if (!Number.isInteger(input.outcomeCount) || input.outcomeCount < 1 || input.outcomeCount > 16) throw new Error('economic founding outcome count is outside 1..16');
  const output = new Uint8Array(ECONOMIC_FOUNDING_BYTES);
  output.set(new TextEncoder().encode('DCLTECI1'));
  output[8] = 1;
  output[13] = input.outcomeCount;
  for (let index = 0; index < identities.length; index += 1) output.set(identities[index], 16 + index * 32);
  return output;
}

export function encodeEconomicOperationV1(operation: EconomicOperationV1): Uint8Array {
  const tags: Record<EconomicAction, number> = { split: 1, merge: 2, materialize: 4, dematerialize: 5, redeem: 6 };
  if (!Number.isInteger(operation.outcome) || operation.outcome < 0 || operation.outcome > 255) throw new Error('economic outcome is not a byte');
  const output = new Uint8Array(ECONOMIC_OPERATION_BYTES);
  output.set(new TextEncoder().encode('DCLTECI1'));
  output[8] = 1;
  output[9] = tags[operation.action];
  output[10] = operation.action === 'materialize' || operation.action === 'dematerialize' ? 0 : operation.holder === 'source' ? 0 : 1;
  output[11] = operation.action === 'materialize' ? 0 : operation.action === 'dematerialize' ? 1 : operation.representation === 'native' ? 0 : 1;
  output[12] = operation.action === 'split' || operation.action === 'merge' ? 0 : operation.outcome;
  putU64(output, 16, operation.quantity, 'economic quantity');
  putU64(output, 24, operation.expectedRevision, 'economic revision');
  return output;
}

function mutableState(state: EconomicStateV1) {
  return {
    outcomeCount: state.outcomeCount, phase: state.phase, winner: state.winner, hoard: state.hoard,
    supply: [...state.supply], nativeSupply: [...state.nativeSupply], materializedSupply: [...state.materializedSupply],
    sourceNative: [...state.sourceNative], sourceMaterialized: [...state.sourceMaterialized],
    destinationNative: [...state.destinationNative], destinationMaterialized: [...state.destinationMaterialized],
  };
}

function freezeState(state: ReturnType<typeof mutableState>): EconomicStateV1 {
  const frozen = Object.freeze({
    ...state, supply: Object.freeze(state.supply), nativeSupply: Object.freeze(state.nativeSupply),
    materializedSupply: Object.freeze(state.materializedSupply), sourceNative: Object.freeze(state.sourceNative),
    sourceMaterialized: Object.freeze(state.sourceMaterialized), destinationNative: Object.freeze(state.destinationNative),
    destinationMaterialized: Object.freeze(state.destinationMaterialized),
  });
  validateEconomicState(frozen);
  return frozen;
}

function checkedAdd(value: bigint, amount: bigint, field: string): bigint {
  const next = value + amount;
  if (next > MAX_U64) throw new Error(`${field} overflows u64`);
  return next;
}

function checkedSub(value: bigint, amount: bigint, field: string): bigint {
  if (amount > value) throw new Error(`${field} has insufficient balance`);
  return value - amount;
}

function claimsFor(state: ReturnType<typeof mutableState>, holder: EconomicHolder, representation: EconomicRepresentation): bigint[] {
  if (holder === 'source') return representation === 'native' ? state.sourceNative : state.sourceMaterialized;
  return representation === 'native' ? state.destinationNative : state.destinationMaterialized;
}

function supplyFor(state: ReturnType<typeof mutableState>, representation: EconomicRepresentation): bigint[] {
  return representation === 'native' ? state.nativeSupply : state.materializedSupply;
}

export function simulateEconomicOperationV1(projection: EconomicProjectionV1, operation: EconomicOperationV1): EconomicSimulationV1 {
  if (operation.expectedRevision !== projection.revision) throw new Error('economic operation revision is stale');
  if (operation.quantity <= 0n || operation.quantity > MAX_U64) throw new Error('economic operation requires a positive u64 quantity');
  const state = mutableState(projection.state);
  const claims: EconomicClaimEffectV1[] = [];
  let custody: EconomicCustodyEffectV1 | null = null;
  const holderClaims = claimsFor(state, operation.holder, operation.representation);
  const representationSupply = supplyFor(state, operation.representation);
  const requireOutcome = () => {
    if (!Number.isInteger(operation.outcome) || operation.outcome < 0 || operation.outcome >= state.outcomeCount) throw new Error('economic outcome is outside the active width');
    return operation.outcome;
  };
  if (operation.action === 'split') {
    if (state.phase !== 'open') throw new Error('complete-set split requires Open phase');
    state.hoard = checkedAdd(state.hoard, operation.quantity, 'Hoard');
    for (let outcome = 0; outcome < state.outcomeCount; outcome += 1) {
      state.supply[outcome] = checkedAdd(state.supply[outcome], operation.quantity, 'conservative supply');
      representationSupply[outcome] = checkedAdd(representationSupply[outcome], operation.quantity, 'representation supply');
      holderClaims[outcome] = checkedAdd(holderClaims[outcome], operation.quantity, 'holder claims');
      claims.push(Object.freeze({ operation: 'credit', holder: operation.holder, outcome, amount: operation.quantity }));
    }
    custody = Object.freeze({ source: operation.holder, destination: 'hoard', amount: operation.quantity });
  } else if (operation.action === 'merge') {
    if (state.phase !== 'open') throw new Error('complete-set merge requires Open phase');
    state.hoard = checkedSub(state.hoard, operation.quantity, 'Hoard');
    for (let outcome = 0; outcome < state.outcomeCount; outcome += 1) {
      state.supply[outcome] = checkedSub(state.supply[outcome], operation.quantity, 'conservative supply');
      representationSupply[outcome] = checkedSub(representationSupply[outcome], operation.quantity, 'representation supply');
      holderClaims[outcome] = checkedSub(holderClaims[outcome], operation.quantity, 'holder claims');
      claims.push(Object.freeze({ operation: 'debit', holder: operation.holder, outcome, amount: operation.quantity }));
    }
    custody = Object.freeze({ source: 'hoard', destination: operation.holder, amount: operation.quantity });
  } else if (operation.action === 'materialize') {
    if (state.phase !== 'open') throw new Error('materialization requires Open phase');
    const outcome = requireOutcome();
    state.nativeSupply[outcome] = checkedSub(state.nativeSupply[outcome], operation.quantity, 'native supply');
    state.sourceNative[outcome] = checkedSub(state.sourceNative[outcome], operation.quantity, 'source native claims');
    state.materializedSupply[outcome] = checkedAdd(state.materializedSupply[outcome], operation.quantity, 'materialized supply');
    state.destinationMaterialized[outcome] = checkedAdd(state.destinationMaterialized[outcome], operation.quantity, 'destination materialized claims');
    claims.push(Object.freeze({ operation: 'debit', holder: 'source', outcome, amount: operation.quantity }), Object.freeze({ operation: 'credit', holder: 'destination', outcome, amount: operation.quantity }));
  } else if (operation.action === 'dematerialize') {
    if (state.phase === 'retired') throw new Error('dematerialization requires a live phase');
    const outcome = requireOutcome();
    state.materializedSupply[outcome] = checkedSub(state.materializedSupply[outcome], operation.quantity, 'materialized supply');
    state.sourceMaterialized[outcome] = checkedSub(state.sourceMaterialized[outcome], operation.quantity, 'source materialized claims');
    state.nativeSupply[outcome] = checkedAdd(state.nativeSupply[outcome], operation.quantity, 'native supply');
    state.destinationNative[outcome] = checkedAdd(state.destinationNative[outcome], operation.quantity, 'destination native claims');
    claims.push(Object.freeze({ operation: 'debit', holder: 'source', outcome, amount: operation.quantity }), Object.freeze({ operation: 'credit', holder: 'destination', outcome, amount: operation.quantity }));
  } else {
    if ((state.phase !== 'terminal' && state.phase !== 'retiring') || state.winner === null) throw new Error('redemption requires Terminal or Retiring phase');
    const outcome = requireOutcome();
    state.supply[outcome] = checkedSub(state.supply[outcome], operation.quantity, 'conservative supply');
    representationSupply[outcome] = checkedSub(representationSupply[outcome], operation.quantity, 'representation supply');
    holderClaims[outcome] = checkedSub(holderClaims[outcome], operation.quantity, 'holder claims');
    claims.push(Object.freeze({ operation: 'debit', holder: operation.holder, outcome, amount: operation.quantity }));
    if (state.winner === outcome) {
      state.hoard = checkedSub(state.hoard, operation.quantity, 'Hoard');
      custody = Object.freeze({ source: 'hoard', destination: operation.holder, amount: operation.quantity });
    }
  }
  return Object.freeze({
    nextState: freezeState(state), claims: Object.freeze(claims), custody,
    admissionRole: operation.action === 'redeem' ? 'resolution' : 'trading',
  });
}

function transactionPlan(instruction: TransactionInstruction, payer: PublicKey, recentBlockhash: string, simulation: EconomicSimulationV1): EconomicTransactionPlanV1 {
  key(recentBlockhash, 'recent blockhash');
  const message = new TransactionMessage({ payerKey: payer, recentBlockhash, instructions: [instruction] }).compileToV0Message();
  const transaction = new VersionedTransaction(message);
  const wireBytes = transaction.serialize();
  if (wireBytes.length > PACKET_DATA_SIZE) throw new Error(`economic transaction is ${wireBytes.length} bytes, above the ${PACKET_DATA_SIZE}-byte packet bound`);
  return Object.freeze({ instruction, transaction, wireBytes, simulation, requiredSignerKeys: Object.freeze(message.staticAccountKeys.slice(0, message.header.numRequiredSignatures).map((entry) => entry.toBase58())) });
}

export function buildEconomicOperationTransaction(input: Readonly<{
  economicProgram: string;
  payer: string;
  recentBlockhash: string;
  authority: string;
  projection: EconomicProjectionObservationV1;
  releaseSet: string;
  operation: EconomicOperationV1;
  holderToken?: string;
}>): EconomicTransactionPlanV1 {
  const program = key(input.economicProgram, 'economic program');
  const payer = key(input.payer, 'payer');
  const authority = key(input.authority, 'semantic authority');
  const projection = key(input.projection.address, 'economic projection');
  const releaseSet = key(input.releaseSet, 'release set');
  const simulation = simulateEconomicOperationV1(input.projection.projection, input.operation);
  const accounts = [
    { pubkey: authority, isSigner: true, isWritable: false },
    { pubkey: projection, isSigner: false, isWritable: true },
    { pubkey: releaseSet, isSigner: false, isWritable: false },
  ];
  if (simulation.custody !== null) {
    if (input.holderToken === undefined) throw new Error('custody operation requires the holder’s exact token account');
    const holder = simulation.custody.source === 'hoard' ? simulation.custody.destination : simulation.custody.source;
    if (holder === 'hoard') throw new Error('custody plan does not identify one holder');
    const holderAuthority = key(holder === 'source' ? input.projection.projection.sourceHolder : input.projection.projection.destinationHolder, 'holder authority');
    const holderToken = key(input.holderToken, 'holder token account');
    const mint = key(input.projection.projection.collateralMint, 'collateral mint');
    const hoard = key(input.projection.projection.hoardAccount, 'Hoard token account');
    const [hoardAuthority] = PublicKey.findProgramAddressSync([ECONOMIC_HOARD_SEED, projection.toBytes()], program);
    const inbound = simulation.custody.destination === 'hoard';
    accounts.push(
      { pubkey: holderAuthority, isSigner: inbound, isWritable: false },
      { pubkey: mint, isSigner: false, isWritable: false },
      { pubkey: holderToken, isSigner: false, isWritable: true },
      { pubkey: hoard, isSigner: false, isWritable: true },
      { pubkey: hoardAuthority, isSigner: false, isWritable: false },
      { pubkey: LEGACY_TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
    );
  }
  if (new Set(accounts.map((account) => account.pubkey.toBase58())).size !== accounts.length) throw new Error('economic transaction aliases two exact account roles');
  if (accounts.some((account) => account.pubkey.equals(payer))) throw new Error('fee payer must be distinct because the economic adapter requires every semantic account read-only or nonsigner exactly as framed');
  return transactionPlan(new TransactionInstruction({ programId: program, keys: accounts, data: encodeEconomicOperationV1(input.operation) as Buffer }), payer, input.recentBlockhash, simulation);
}

export function buildEconomicFoundingTransaction(input: Readonly<{
  economicProgram: string;
  payer: string;
  recentBlockhash: string;
  authority: string;
  projection: EconomicVacancyObservationV1;
  releaseSet: string;
  coordinates: EconomicFoundingCoordinatesV1;
  releaseSetId: Uint8Array;
  sourceHolder: string;
  destinationHolder: string;
  hoardAccount: string;
}>): Readonly<Omit<EconomicTransactionPlanV1, 'simulation'> & { foundingBytes: Uint8Array }> {
  const program = key(input.economicProgram, 'economic program');
  const payer = key(input.payer, 'payer');
  const authority = key(input.authority, 'Core authority');
  const projection = key(input.projection.address, 'vacant projection');
  const releaseSet = key(input.releaseSet, 'release set');
  const accounts = [
    { pubkey: authority, isSigner: true, isWritable: false },
    { pubkey: projection, isSigner: false, isWritable: true },
    { pubkey: releaseSet, isSigner: false, isWritable: false },
  ];
  if (new Set(accounts.map((account) => account.pubkey.toBase58())).size !== accounts.length || accounts.some((account) => account.pubkey.equals(payer))) throw new Error('founding aliases authority, projection, release, or fee payer roles');
  const foundingBytes = encodeEconomicFoundingV1({
    marketId: input.coordinates.marketId, releaseSetId: input.releaseSetId, sourceHolder: input.sourceHolder,
    destinationHolder: input.destinationHolder, collateralMint: input.coordinates.collateralMint,
    hoardAccount: input.hoardAccount, outcomeCount: input.coordinates.outcomeCount,
  });
  const instruction = new TransactionInstruction({ programId: program, keys: accounts, data: foundingBytes as Buffer });
  key(input.recentBlockhash, 'recent blockhash');
  const message = new TransactionMessage({ payerKey: payer, recentBlockhash: input.recentBlockhash, instructions: [instruction] }).compileToV0Message();
  const transaction = new VersionedTransaction(message);
  const wireBytes = transaction.serialize();
  if (wireBytes.length > PACKET_DATA_SIZE) throw new Error('economic founding exceeds the transaction packet bound');
  return Object.freeze({ instruction, transaction, wireBytes, foundingBytes, requiredSignerKeys: Object.freeze(message.staticAccountKeys.slice(0, message.header.numRequiredSignatures).map((entry) => entry.toBase58())) });
}

export function inspectEconomicTokenRoute(input: Readonly<{
  projectionAddress: string;
  economicProgram: string;
  projection: EconomicProjectionV1;
  holder: EconomicHolder;
  holderToken: RpcAccount;
  hoardToken: RpcAccount;
  mint: RpcAccount;
  custody: EconomicCustodyEffectV1;
}>): Readonly<{ holderBefore: bigint; holderAfter: bigint; hoardBefore: bigint; hoardAfter: bigint; decimals: number }> {
  if (input.mint.owner !== LEGACY_TOKEN_PROGRAM_ID.toBase58() || input.mint.executable || input.mint.data.length !== 82 || input.mint.data[45] !== 1) throw new Error('collateral Mint is not one initialized exact legacy Mint');
  const holder = decodeLegacyTokenObservationV1(input.holderToken);
  const hoard = decodeLegacyTokenObservationV1(input.hoardToken);
  const holderAuthority = input.holder === 'source' ? input.projection.sourceHolder : input.projection.destinationHolder;
  const [hoardAuthority] = PublicKey.findProgramAddressSync([ECONOMIC_HOARD_SEED, key(input.projectionAddress, 'projection').toBytes()], key(input.economicProgram, 'economic program'));
  if (holder.mint !== input.projection.collateralMint || hoard.mint !== input.projection.collateralMint || holder.owner !== holderAuthority || hoard.owner !== hoardAuthority.toBase58()) throw new Error('token accounts do not match immutable holder/Mint/Hoard authority bindings');
  const inbound = input.custody.destination === 'hoard';
  const holderAfter = inbound ? checkedSub(holder.amount, input.custody.amount, 'holder token balance') : checkedAdd(holder.amount, input.custody.amount, 'holder token balance');
  const hoardAfter = inbound ? checkedAdd(hoard.amount, input.custody.amount, 'Hoard token balance') : checkedSub(hoard.amount, input.custody.amount, 'Hoard token balance');
  return Object.freeze({ holderBefore: holder.amount, holderAfter, hoardBefore: hoard.amount, hoardAfter, decimals: input.mint.data[44] });
}
