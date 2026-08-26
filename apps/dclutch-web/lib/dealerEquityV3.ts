import {
  AddressLookupTableAccount,
  PublicKey,
  SYSVAR_INSTRUCTIONS_PUBKEY,
  SYSVAR_RENT_PUBKEY,
  TransactionInstruction,
  TransactionMessage,
  VersionedTransaction,
} from '@solana/web3.js';

import { hex, isZero, requireZero, sha256, slice, u16, u64 } from './bytes';
import { PACKET_DATA_SIZE } from './directTransaction';
import {
  HOT_EXECUTION_ENVELOPE_BYTES_V3,
  HOT_EXECUTION_MAGIC_V3,
  HOT_EXECUTION_PROFILE_V3,
  HOT_EXECUTION_VERSION_V3,
  HOT_FIXED_ACCOUNT_COUNT_V3,
  HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3,
  HOT_MARKET_ACCOUNT_V3,
  HOT_RENT_SYSVAR_ACCOUNT_V3,
  HOT_ROOT_ACCOUNT_V3,
  HOT_TRADING_PROGRAM_ACCOUNT_V3,
} from './generated/directInlineV3';
import {
  DEALER_EQUITY_CLAIMS_PACKET_BYTES_OFFSET_V3,
  DEALER_EQUITY_CONTRIBUTE_P0_SELECTOR_V3,
  DEALER_EQUITY_CONTRIBUTE_P1_SELECTOR_V3,
  DEALER_EQUITY_CONTRIBUTE_P2_SELECTOR_V3,
  DEALER_EQUITY_HEADER_BYTES_V3,
  DEALER_EQUITY_REDEEM_P0_SELECTOR_V3,
  DEALER_EQUITY_REDEEM_P1_SELECTOR_V3,
  DEALER_EQUITY_REDEEM_P2_SELECTOR_V3,
  DEALER_EQUITY_REQUEST_MAGIC_V3,
  DEALER_EQUITY_REQUEST_VERSION_V3,
  DEALER_LP_POSITION_PDA_DOMAIN_V3,
  DEALER_OBLIGATION_PDA_DOMAIN_V3,
  SIGNED_DELTA_BYTES_V3,
  SIGNED_DELTA_PLAN_HEADER_BYTES_V3,
  SIGNED_DELTA_PLAN_MAGIC_V3,
  SIGNED_DELTA_POSITION_BYTES_V3,
  SIGNED_DELTA_ROW_BYTES_V3,
  SIGNED_DELTA_WIRE_VERSION_V3,
} from './generated/dealerEquityV3';
import { type CheckedHotOuterEvidenceV3, type DirectHotAccountMetaV3 } from './directInlineV3';

const MAX_U64 = 18_446_744_073_709_551_615n;
const SIGNED_DELTA_ROLE_OFFSET = 10;
const SIGNED_DELTA_RELEASE_SET_OFFSET = 16;
const SIGNED_DELTA_MARKET_OFFSET = 48;
const SIGNED_DELTA_REQUEST_OFFSET = 80;
const SIGNED_DELTA_CLAIM_COUNT_OFFSET = 216;
const SIGNED_DELTA_POSITION_COUNT_OFFSET = 220;
const SIGNED_DELTA_ROW_COUNT_OFFSET = 224;

export type DealerEquityActionV3 = 'contribute' | 'redeem';

export type DealerEquityRequestV3 = Readonly<{
  bytes: Uint8Array;
  selector: 1 | 2 | 3 | 4 | 5 | 6;
  action: DealerEquityActionV3;
  signedPositionCount: 0 | 1 | 2;
  width: number;
  releaseSet: Uint8Array;
  market: string;
  childRoot: string;
  lpPosition: string;
  lpOwner: string;
  obligation: string;
  obligationDigest: Uint8Array;
  lpDigest: Uint8Array;
  dealerPositionOwner: string;
  dealerClaimsDigest: Uint8Array;
  lpClaimsDigest: Uint8Array;
  collateralDigest: Uint8Array;
  obligationRevision: bigint;
  lpRevision: bigint;
  dealerClaimsRevision: bigint;
  lpClaimsRevision: bigint;
  generation: bigint;
  expiresAt: bigint;
  lockedCapitalFloor: bigint;
  collateral: bigint;
  shares: bigint;
  claimsPacketBytes: number;
}>;

export type DealerEquityHotRouteV3 = Readonly<{
  payer: string;
  tradingProgram: string;
  market: string;
  releaseSet: Uint8Array;
  generation: bigint;
  rootPrestateDigest: Uint8Array;
  observedSlot: bigint;
  fixedAccounts: ReadonlyArray<DirectHotAccountMetaV3>;
  strategyAccounts: ReadonlyArray<DirectHotAccountMetaV3>;
  runtimeAccounts: ReadonlyArray<DirectHotAccountMetaV3>;
  recentBlockhash: string;
  lookupTables: ReadonlyArray<AddressLookupTableAccount>;
  outerEvidence: CheckedHotOuterEvidenceV3;
}>;

export type DealerEquityTransactionPlanV3 = Readonly<{
  request: DealerEquityRequestV3;
  hotInstructionBytes: Uint8Array;
  transaction: VersionedTransaction;
  wireBytes: Uint8Array;
  requiredSigners: ReadonlyArray<string>;
  loadedAddresses: number;
  accountCount: number;
}>;

function same(left: Uint8Array, right: Uint8Array): boolean {
  return left.length === right.length && left.every((value, index) => value === right[index]);
}

function u32(bytes: Uint8Array, offset: number): number {
  const value = slice(bytes, offset, 4);
  return new DataView(value.buffer, value.byteOffset, value.byteLength).getUint32(0, true);
}

function key(bytes: Uint8Array, field: string): string {
  if (bytes.length !== 32 || isZero(bytes)) throw new Error(`${field} is zero or truncated`);
  return new PublicKey(bytes).toBase58();
}

function exactKey(text: string, field: string): PublicKey {
  const value = new PublicKey(text);
  if (value.toBase58() !== text) throw new Error(`${field} must be canonical base58 text`);
  return value;
}

function putU16(bytes: Uint8Array, offset: number, value: number): void {
  new DataView(bytes.buffer, bytes.byteOffset + offset, 2).setUint16(0, value, true);
}

function putU32(bytes: Uint8Array, offset: number, value: number): void {
  new DataView(bytes.buffer, bytes.byteOffset + offset, 4).setUint32(0, value, true);
}

function putU64(bytes: Uint8Array, offset: number, value: bigint): void {
  new DataView(bytes.buffer, bytes.byteOffset + offset, 8).setBigUint64(0, value, true);
}

function shape(selector: number): Readonly<{ selector: 1 | 2 | 3 | 4 | 5 | 6; action: DealerEquityActionV3; positions: 0 | 1 | 2 }> {
  switch (selector) {
    case DEALER_EQUITY_CONTRIBUTE_P0_SELECTOR_V3: return Object.freeze({ selector: 1, action: 'contribute', positions: 0 });
    case DEALER_EQUITY_CONTRIBUTE_P1_SELECTOR_V3: return Object.freeze({ selector: 2, action: 'contribute', positions: 1 });
    case DEALER_EQUITY_CONTRIBUTE_P2_SELECTOR_V3: return Object.freeze({ selector: 3, action: 'contribute', positions: 2 });
    case DEALER_EQUITY_REDEEM_P0_SELECTOR_V3: return Object.freeze({ selector: 4, action: 'redeem', positions: 0 });
    case DEALER_EQUITY_REDEEM_P1_SELECTOR_V3: return Object.freeze({ selector: 5, action: 'redeem', positions: 1 });
    case DEALER_EQUITY_REDEEM_P2_SELECTOR_V3: return Object.freeze({ selector: 6, action: 'redeem', positions: 2 });
    default: throw new Error('Dealer equity selector is outside the executable 1..6 successor range');
  }
}

function delta(bytes: Uint8Array, offset: number, allowNeutral: boolean): Readonly<{ direction: number; magnitude: bigint }> {
  const direction = bytes[offset];
  requireZero(bytes, offset + 1, 7, 'SignedDelta signed magnitude');
  const magnitude = u64(bytes, offset + 8);
  if (direction === undefined || direction > 2 || (magnitude === 0n) !== (direction === 0) || (!allowNeutral && direction === 0)) {
    throw new Error('SignedDelta has a noncanonical signed magnitude');
  }
  return Object.freeze({ direction, magnitude });
}

async function validateSignedDelta(
  packet: Uint8Array,
  header: Uint8Array,
  expectedPositions: number,
  width: number,
  releaseSet: Uint8Array,
  market: Uint8Array,
): Promise<void> {
  if (expectedPositions === 0) {
    if (packet.length !== 0) throw new Error('Dealer P0 request carries an unexpected Claims packet');
    return;
  }
  if (packet.length < SIGNED_DELTA_PLAN_HEADER_BYTES_V3 || !same(slice(packet, 0, 8), SIGNED_DELTA_PLAN_MAGIC_V3)
      || u16(packet, 8) !== SIGNED_DELTA_WIRE_VERSION_V3 || packet[SIGNED_DELTA_ROLE_OFFSET] !== 2) {
    throw new Error('Dealer Claims suffix is not one Trading-role SignedDelta V3 packet');
  }
  requireZero(packet, 11, 5, 'SignedDelta header');
  requireZero(packet, 228, 12, 'SignedDelta header tail');
  const claimCount = u32(packet, SIGNED_DELTA_CLAIM_COUNT_OFFSET);
  const positionCount = u32(packet, SIGNED_DELTA_POSITION_COUNT_OFFSET);
  const rowCount = u32(packet, SIGNED_DELTA_ROW_COUNT_OFFSET);
  const expected = SIGNED_DELTA_PLAN_HEADER_BYTES_V3 + positionCount * SIGNED_DELTA_POSITION_BYTES_V3
    + claimCount * SIGNED_DELTA_BYTES_V3 + rowCount * SIGNED_DELTA_ROW_BYTES_V3;
  if (claimCount !== width || positionCount !== expectedPositions || rowCount === 0 || packet.length !== expected
      || !same(slice(packet, SIGNED_DELTA_RELEASE_SET_OFFSET, 32), releaseSet)
      || !same(slice(packet, SIGNED_DELTA_MARKET_OFFSET, 32), market)
      || !same(slice(packet, SIGNED_DELTA_REQUEST_OFFSET, 32), await sha256(header))) {
    throw new Error('Dealer SignedDelta width, identity, request digest, or exact table length differs');
  }
  const positionsStart = SIGNED_DELTA_PLAN_HEADER_BYTES_V3;
  const aggregateStart = positionsStart + positionCount * SIGNED_DELTA_POSITION_BYTES_V3;
  const rowsStart = aggregateStart + claimCount * SIGNED_DELTA_BYTES_V3;
  let priorOwner: Uint8Array | null = null;
  const used = new Set<number>();
  for (let index = 0; index < positionCount; index += 1) {
    const offset = positionsStart + index * SIGNED_DELTA_POSITION_BYTES_V3;
    const owner = slice(packet, offset, 32);
    if (isZero(owner) || u64(packet, offset + 32) === MAX_U64
        || (priorOwner !== null && hex(priorOwner) >= hex(owner))) throw new Error('SignedDelta Position table is not canonical');
    priorOwner = owner;
  }
  let priorCoordinate = -1;
  const credits = Array<bigint>(claimCount).fill(0n);
  const debits = Array<bigint>(claimCount).fill(0n);
  for (let index = 0; index < rowCount; index += 1) {
    const offset = rowsStart + index * SIGNED_DELTA_ROW_BYTES_V3;
    const position = u32(packet, offset);
    const outcome = u32(packet, offset + 4);
    if (position >= positionCount || outcome >= claimCount) throw new Error('SignedDelta row coordinate exceeds its runtime table');
    const coordinate = position * claimCount + outcome;
    if (coordinate <= priorCoordinate) throw new Error('SignedDelta rows are duplicated or not strictly ordered');
    priorCoordinate = coordinate;
    used.add(position);
    const value = delta(packet, offset + 8, false);
    if (value.direction === 1) credits[outcome] = (credits[outcome] ?? 0n) + value.magnitude;
    else debits[outcome] = (debits[outcome] ?? 0n) + value.magnitude;
  }
  if (used.size !== positionCount) throw new Error('SignedDelta Position table contains an unused entry');
  for (let outcome = 0; outcome < claimCount; outcome += 1) {
    const aggregate = delta(packet, aggregateStart + outcome * SIGNED_DELTA_BYTES_V3, true);
    const credit = credits[outcome] ?? 0n;
    const debit = debits[outcome] ?? 0n;
    const direction = credit === debit ? 0 : credit > debit ? 1 : 2;
    const magnitude = credit >= debit ? credit - debit : debit - credit;
    if (aggregate.direction !== direction || aggregate.magnitude !== magnitude) throw new Error('SignedDelta rows do not conserve to the aggregate vector');
  }
}

export async function decodeDealerEquityRequestV3(bytes: Uint8Array): Promise<DealerEquityRequestV3> {
  if (bytes.length < DEALER_EQUITY_HEADER_BYTES_V3 || !same(slice(bytes, 0, 8), DEALER_EQUITY_REQUEST_MAGIC_V3)
      || u16(bytes, 8) !== DEALER_EQUITY_REQUEST_VERSION_V3) throw new Error('Dealer request has the wrong exact header');
  requireZero(bytes, 476, 4, 'Dealer request tail');
  const selected = shape(u16(bytes, 10));
  const width = u32(bytes, 12);
  const claimsPacketBytes = u32(bytes, DEALER_EQUITY_CLAIMS_PACKET_BYTES_OFFSET_V3);
  if (width === 0 || bytes.length !== DEALER_EQUITY_HEADER_BYTES_V3 + claimsPacketBytes) throw new Error('Dealer request has a zero width or inconsistent Claims suffix');
  const releaseSet = slice(bytes, 16, 32);
  const marketBytes = slice(bytes, 48, 32);
  for (const [offset, field] of [[16, 'release set'], [48, 'Market'], [80, 'child root'], [112, 'LP Position'], [144, 'LP owner'], [176, 'obligation'], [208, 'obligation digest'], [240, 'LP digest'], [272, 'Dealer Position owner'], [304, 'Dealer Claims digest'], [336, 'LP Claims digest'], [368, 'collateral digest']] as const) {
    if (isZero(slice(bytes, offset, 32))) throw new Error(`Dealer request ${field} is zero`);
  }
  const revisions = [u64(bytes, 400), u64(bytes, 408), u64(bytes, 416), u64(bytes, 424)];
  const generation = u64(bytes, 432);
  const shares = u64(bytes, 464);
  const collateral = u64(bytes, 456);
  if (revisions.some((value) => value === 0n) || generation === 0n || shares === 0n
      || (selected.action === 'redeem' && collateral !== 0n)) throw new Error('Dealer request revisions, generation, shares, or redemption collateral are noncanonical');
  await validateSignedDelta(bytes.slice(DEALER_EQUITY_HEADER_BYTES_V3), bytes.slice(0, DEALER_EQUITY_HEADER_BYTES_V3), selected.positions, width, releaseSet, marketBytes);
  return Object.freeze({
    bytes: new Uint8Array(bytes), selector: selected.selector, action: selected.action, signedPositionCount: selected.positions,
    width, releaseSet, market: key(marketBytes, 'Dealer Market'), childRoot: key(slice(bytes, 80, 32), 'Dealer child root'),
    lpPosition: key(slice(bytes, 112, 32), 'Dealer LP Position'), lpOwner: key(slice(bytes, 144, 32), 'Dealer LP owner'),
    obligation: key(slice(bytes, 176, 32), 'Dealer obligation'), obligationDigest: slice(bytes, 208, 32), lpDigest: slice(bytes, 240, 32),
    dealerPositionOwner: key(slice(bytes, 272, 32), 'Dealer Position owner'), dealerClaimsDigest: slice(bytes, 304, 32),
    lpClaimsDigest: slice(bytes, 336, 32), collateralDigest: slice(bytes, 368, 32), obligationRevision: revisions[0] ?? 0n,
    lpRevision: revisions[1] ?? 0n, dealerClaimsRevision: revisions[2] ?? 0n, lpClaimsRevision: revisions[3] ?? 0n,
    generation, expiresAt: u64(bytes, 440), lockedCapitalFloor: u64(bytes, 448), collateral, shares, claimsPacketBytes,
  });
}

function validateRoute(route: DealerEquityHotRouteV3, request: DealerEquityRequestV3): void {
  if (route.outerEvidence.status !== 'checked') throw new Error(`Dealer V3 hot execution unavailable: ${route.outerEvidence.reason}`);
  if (route.fixedAccounts.length !== HOT_FIXED_ACCOUNT_COUNT_V3 || route.releaseSet.length !== 32 || isZero(route.releaseSet)
      || route.rootPrestateDigest.length !== 32 || isZero(route.rootPrestateDigest)) throw new Error('Dealer Hot route has invalid fixed geometry or identities');
  const market = exactKey(route.market, 'Dealer Market');
  const trading = exactKey(route.tradingProgram, 'Trading program');
  const root = route.fixedAccounts[HOT_ROOT_ACCOUNT_V3];
  if (route.fixedAccounts[HOT_MARKET_ACCOUNT_V3]?.address !== market.toBase58()
      || route.fixedAccounts[HOT_TRADING_PROGRAM_ACCOUNT_V3]?.address !== trading.toBase58()
      || route.fixedAccounts[HOT_RENT_SYSVAR_ACCOUNT_V3]?.address !== SYSVAR_RENT_PUBKEY.toBase58()
      || route.fixedAccounts[HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3]?.address !== SYSVAR_INSTRUCTIONS_PUBKEY.toBase58()
      || root === undefined || root.isSigner || !root.isWritable) throw new Error('Dealer Hot fixed account roles differ from the canonical ABI');
  route.fixedAccounts.forEach((account, index) => {
    if (account.isSigner || account.isWritable !== (index === HOT_ROOT_ACCOUNT_V3)) throw new Error(`Dealer Hot fixed account ${index} has substituted privileges`);
  });
  if (!same(route.releaseSet, request.releaseSet) || route.market !== request.market || route.generation !== request.generation
      || root.address !== request.childRoot || route.observedSlot > request.expiresAt) throw new Error('Dealer request differs from the finalized Market/root/release/generation/expiry');
  const expectedObligation = PublicKey.findProgramAddressSync([DEALER_OBLIGATION_PDA_DOMAIN_V3, exactKey(request.childRoot, 'child root').toBytes()], trading)[0].toBase58();
  const expectedLp = PublicKey.findProgramAddressSync([DEALER_LP_POSITION_PDA_DOMAIN_V3, exactKey(request.childRoot, 'child root').toBytes(), exactKey(request.lpOwner, 'LP owner').toBytes()], trading)[0].toBase58();
  if (request.obligation !== expectedObligation || request.lpPosition !== expectedLp) throw new Error('Dealer request LP or obligation account is not the canonical Trading PDA');
  const add = request.action === 'contribute';
  const expectedRuntime = (add ? 50 : 64) + request.signedPositionCount;
  const scalarCount = add ? 26 : 35;
  const identityCount = add ? 36 : 52;
  const callerPages = Math.ceil((scalarCount * 8 + identityCount * 32) / 880);
  if (route.runtimeAccounts.length !== expectedRuntime || route.strategyAccounts.length !== 8 + callerPages
      || route.strategyAccounts.some((account, index) => account.isSigner || account.isWritable || (index === 6 && !account.executable))) {
    throw new Error('Dealer runtime or admitted-AOT account geometry differs from the selected action/P shape');
  }
}

export async function compileDealerEquityTransactionV3(
  route: DealerEquityHotRouteV3,
  requestBytes: Uint8Array,
): Promise<DealerEquityTransactionPlanV3> {
  const request = await decodeDealerEquityRequestV3(requestBytes);
  validateRoute(route, request);
  const hotInstructionBytes = new Uint8Array(HOT_EXECUTION_ENVELOPE_BYTES_V3 + request.bytes.length);
  hotInstructionBytes.set(HOT_EXECUTION_MAGIC_V3, 0);
  putU16(hotInstructionBytes, 8, HOT_EXECUTION_VERSION_V3);
  putU16(hotInstructionBytes, 10, HOT_EXECUTION_PROFILE_V3);
  putU32(hotInstructionBytes, 12, request.bytes.length);
  hotInstructionBytes.set(route.releaseSet, 16);
  hotInstructionBytes.set(exactKey(route.market, 'Dealer Market').toBytes(), 48);
  putU64(hotInstructionBytes, 80, route.generation);
  hotInstructionBytes.set(route.rootPrestateDigest, 88);
  hotInstructionBytes.set(request.bytes, HOT_EXECUTION_ENVELOPE_BYTES_V3);
  const toMeta = (account: DirectHotAccountMetaV3) => ({ pubkey: exactKey(account.address, 'Dealer route account'), isSigner: account.isSigner, isWritable: account.isWritable });
  const instruction = new TransactionInstruction({
    programId: exactKey(route.tradingProgram, 'Trading program'),
    keys: [...route.fixedAccounts, ...route.strategyAccounts, ...route.runtimeAccounts].map(toMeta),
    data: hotInstructionBytes as Buffer,
  });
  exactKey(route.recentBlockhash, 'recent blockhash');
  const transaction = new VersionedTransaction(new TransactionMessage({
    payerKey: exactKey(route.payer, 'payer'), recentBlockhash: route.recentBlockhash, instructions: [instruction],
  }).compileToV0Message([...route.lookupTables]));
  const wireBytes = transaction.serialize();
  if (wireBytes.length > PACKET_DATA_SIZE) throw new Error(`Dealer V3 transaction is ${wireBytes.length} bytes, above the ${PACKET_DATA_SIZE}-byte packet bound`);
  const requiredSigners = Object.freeze(transaction.message.staticAccountKeys
    .slice(0, transaction.message.header.numRequiredSignatures).map((value) => value.toBase58()));
  if (requiredSigners.length !== 1 || requiredSigners[0] !== route.payer) throw new Error('Dealer V3 message requires an unexpected transaction signer');
  return Object.freeze({
    request, hotInstructionBytes, transaction, wireBytes, requiredSigners,
    loadedAddresses: transaction.message.addressTableLookups.reduce((sum, value) => sum + value.readonlyIndexes.length + value.writableIndexes.length, 0),
    accountCount: route.fixedAccounts.length + route.strategyAccounts.length + route.runtimeAccounts.length,
  });
}
