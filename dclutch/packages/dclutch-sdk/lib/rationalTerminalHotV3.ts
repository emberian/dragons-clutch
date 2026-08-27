import { PublicKey } from '@solana/web3.js';

import { isZero, requireNonzero, requireZero, sha256, slice } from './bytes';
import * as Abi from './generated/rationalTerminalHotV3';

const MAX_U64 = 18_446_744_073_709_551_615n;

export type RationalTerminalAssetV3 = Readonly<{
  shardMint: string;
  actorShardAccount: string;
  structuredCustodyAccount: string;
  claimsCustodyOwner: string;
  coefficient: bigint;
  expectedShardSupply: bigint;
  expectedActorShards: bigint;
  expectedStructuredShards: bigint;
}>;

export type RationalTerminalHotInputV3 = Readonly<{
  releaseSet: Uint8Array;
  market: string;
  graphId: Uint8Array;
  descriptorId: Uint8Array;
  actor: string;
  receiptMint: string;
  representationAuthority: string;
  tokenProgram: string;
  realm: string;
  collateralRecipient: string;
  expectedRepresentationRevision: bigint;
  expectedClaimsMarketRevision: bigint;
  expectedCustodyPositionRevision: bigint;
  expectedCustodyReplayRevision: bigint;
  generation: bigint;
  quantity: bigint;
  denominator: bigint;
  expectedReceiptSupply: bigint;
  outcomeCount: number;
  selectedOutcome: number;
  asset: RationalTerminalAssetV3;
}>;

export type RationalTerminalCompiledV3 = Readonly<{
  familyBytes: Uint8Array;
  familyDigest: Uint8Array;
  childRequest: Uint8Array;
  childDigest: Uint8Array;
  claimsAccountCount: 49;
  rawQuantity: bigint;
  rawShardBurn: bigint;
  payoutPolicy: 'product-derived-including-zero';
}>;

export type RationalTerminalPayoutV3 = Readonly<{
  scenario: 'categorical' | 'graded-rational' | 'graded-failure';
  payoutPerShard: bigint;
  rawPayout: bigint;
  losing: boolean;
}>;

type RationalBasisTermV3 = Readonly<{
  claim: number;
  tag: 0 | 1 | 2 | 3;
  left: number;
  peak: number;
  right: number;
  amplitude: bigint;
}>;

export type RationalProductBasisViewV3 = Readonly<{
  bytes: Uint8Array;
  kind: 'categorical-q1' | 'graded-exact-complement';
  productId: Uint8Array;
  resultDomainId: Uint8Array;
  coordinateDomainId: Uint8Array;
  resultUnitId: Uint8Array;
  evaluatorReleaseId: Uint8Array;
  width: number;
  scale: bigint;
  knotDenominator: bigint;
  failurePayouts: ReadonlyArray<bigint>;
  knots: ReadonlyArray<bigint>;
  terms: ReadonlyArray<RationalBasisTermV3>;
}>;

const BASIS_HEADER_BYTES_V3 = 256;
const BASIS_KNOT_BYTES_V3 = 16;
const BASIS_TERM_BYTES_V3 = 32;

function readU32(bytes: Uint8Array, offset: number): number {
  const value = bytes.slice(offset, offset + 4);
  return new DataView(value.buffer, value.byteOffset, value.byteLength).getUint32(0, true);
}

function readU64(bytes: Uint8Array, offset: number): bigint {
  const value = bytes.slice(offset, offset + 8);
  return new DataView(value.buffer, value.byteOffset, value.byteLength).getBigUint64(0, true);
}

function readI128(bytes: Uint8Array, offset: number): bigint {
  const value = bytes.slice(offset, offset + 16);
  const view = new DataView(value.buffer, value.byteOffset, value.byteLength);
  return (view.getBigInt64(8, true) << 64n) | view.getBigUint64(0, true);
}

function rationalCompare(left: bigint, leftDenominator: bigint, right: bigint, rightDenominator: bigint): number {
  const a = left * rightDenominator; const b = right * leftDenominator;
  return a < b ? -1 : a > b ? 1 : 0;
}

function terminalRamp(amplitude: bigint, left: bigint, right: bigint, knotDenominator: bigint,
  numerator: bigint, denominator: bigint, rising: boolean): bigint {
  const leftCmp = rationalCompare(numerator, denominator, left, knotDenominator);
  const rightCmp = rationalCompare(numerator, denominator, right, knotDenominator);
  if (rising && leftCmp <= 0) return 0n;
  if (rising && rightCmp >= 0) return amplitude;
  if (!rising && leftCmp <= 0) return amplitude;
  if (!rising && rightCmp >= 0) return 0n;
  const coordinate = numerator * knotDenominator;
  const low = left * denominator; const high = right * denominator;
  const elapsed = rising ? coordinate - low : high - coordinate;
  const width = high - low;
  if (elapsed <= 0n || elapsed >= width || width <= 0n) throw new Error('graded terminal interpolation left its checked interior');
  return amplitude * elapsed / width;
}

function compareTerm(left: RationalBasisTermV3, right: RationalBasisTermV3): number {
  for (const [a, b] of [[left.claim, right.claim], [left.tag, right.tag], [left.left, right.left], [left.peak, right.peak], [left.right, right.right]]) {
    if (a !== b) return a < b ? -1 : 1;
  }
  return 0;
}

function evaluateBasisTerm(term: RationalBasisTermV3, knots: ReadonlyArray<bigint>, knotDenominator: bigint,
  numerator: bigint, denominator: bigint): bigint {
  const knot = (index: number): bigint => {
    const value = knots[index];
    if (value === undefined) throw new Error('graded ProductBasisV3 term selects an absent knot');
    return value;
  };
  if (term.tag === 0) return term.amplitude;
  if (term.tag === 1) return terminalRamp(term.amplitude, knot(term.left), knot(term.right), knotDenominator, numerator, denominator, true);
  if (term.tag === 2) return terminalRamp(term.amplitude, knot(term.left), knot(term.right), knotDenominator, numerator, denominator, false);
  return [
    terminalRamp(term.amplitude, knot(term.left), knot(term.peak), knotDenominator, numerator, denominator, true),
    terminalRamp(term.amplitude, knot(term.peak), knot(term.right), knotDenominator, numerator, denominator, false),
  ].reduce((left, right) => left < right ? left : right);
}

/** Hostile-decode the exact ProductBasisV3 body used by the Rust operator. */
export function decodeRationalProductBasisV3(bytes: Uint8Array): RationalProductBasisViewV3 {
  if (bytes.length < BASIS_HEADER_BYTES_V3 || new TextDecoder().decode(bytes.slice(0, 8)) !== 'DCLTPAY3'
      || new DataView(bytes.buffer, bytes.byteOffset + 8, 2).getUint16(0, true) !== 3
      || new DataView(bytes.buffer, bytes.byteOffset + 10, 2).getUint16(0, true) !== BASIS_HEADER_BYTES_V3
      || readU32(bytes, 12) !== bytes.length) throw new Error('ProductBasisV3 has the wrong exact header or width');
  requireZero(bytes, 18, 2, 'ProductBasisV3 header'); requireZero(bytes, 208, 48, 'ProductBasisV3 tail');
  const tag = bytes[16]; const rounding = bytes[17]; const width = readU32(bytes, 20);
  const knotCount = readU32(bytes, 24); const termCount = readU32(bytes, 28);
  const productId = slice(bytes, 32, 32); const resultDomainId = slice(bytes, 64, 32);
  const coordinateDomainId = slice(bytes, 96, 32); const resultUnitId = slice(bytes, 128, 32);
  const evaluatorReleaseId = slice(bytes, 176, 32);
  [productId, resultDomainId, coordinateDomainId, resultUnitId, evaluatorReleaseId]
    .forEach((value, index) => requireNonzero(value, `ProductBasisV3 identity ${index}`));
  const scale = readU64(bytes, 160); const knotDenominator = readU64(bytes, 168);
  if (width === 0 || scale === 0n) throw new Error('ProductBasisV3 has a zero basis width or payout scale');
  const graded = tag === 2;
  if (tag !== 1 && tag !== 2) throw new Error('ProductBasisV3 has an undefined basis kind');
  const exact = BASIS_HEADER_BYTES_V3 + (graded ? width * 8 : 0) + knotCount * BASIS_KNOT_BYTES_V3 + termCount * BASIS_TERM_BYTES_V3;
  if (!Number.isSafeInteger(exact) || bytes.length !== exact) throw new Error('ProductBasisV3 has the wrong exact runtime tail');
  if (!graded && (rounding !== 0 || width === 0 || scale !== 1n || knotDenominator !== 1n || knotCount !== 0 || termCount !== 0)) {
    throw new Error('categorical ProductBasisV3 is noncanonical');
  }
  if (graded && (rounding !== 1 || width < 2 || knotDenominator === 0n || termCount === 0)) {
    throw new Error('graded ProductBasisV3 is noncanonical');
  }
  const failurePayouts: bigint[] = [];
  let offset = BASIS_HEADER_BYTES_V3;
  if (graded) {
    let total = 0n;
    for (let index = 0; index < width; index += 1) {
      const payout = readU64(bytes, offset); offset += 8; total += payout;
      if (total > MAX_U64) throw new Error('graded ProductBasisV3 failure partition overflows u64');
      failurePayouts.push(payout);
    }
    if (total !== scale) throw new Error('graded ProductBasisV3 failure payouts do not partition Q');
  }
  const knots: bigint[] = [];
  for (let index = 0; index < knotCount; index += 1) {
    const value = readI128(bytes, offset); offset += BASIS_KNOT_BYTES_V3;
    if (knots.length > 0 && value <= (knots[knots.length - 1] as bigint)) throw new Error('graded ProductBasisV3 knots are not strictly increasing');
    knots.push(value);
  }
  const terms: RationalBasisTermV3[] = [];
  for (let index = 0; index < termCount; index += 1) {
    requireZero(bytes, offset + 5, 3, 'ProductBasisV3 term'); requireZero(bytes, offset + 20, 4, 'ProductBasisV3 term');
    const claim = readU32(bytes, offset); const termTag = bytes[offset + 4];
    const left = readU32(bytes, offset + 8); const peak = readU32(bytes, offset + 12); const right = readU32(bytes, offset + 16);
    const amplitude = readU64(bytes, offset + 24); offset += BASIS_TERM_BYTES_V3;
    if (termTag === undefined || termTag > 3 || amplitude === 0n || claim >= width - 1
        || (termTag === 0 && (left !== 0 || peak !== 0 || right !== 0))
        || ((termTag === 1 || termTag === 2) && (peak !== 0 || left >= right || right >= knotCount))
        || (termTag === 3 && (left >= peak || peak >= right || right >= knotCount))) {
      throw new Error('graded ProductBasisV3 has an invalid term');
    }
    const term = Object.freeze({ claim, tag: termTag as 0 | 1 | 2 | 3, left, peak, right, amplitude });
    const prior = terms[terms.length - 1];
    if ((prior !== undefined && compareTerm(prior, term) >= 0)
        || (prior === undefined && claim !== 0)
        || (prior !== undefined && claim !== prior.claim && claim !== prior.claim + 1)) {
      throw new Error('graded ProductBasisV3 terms are not canonical and gap-free');
    }
    terms.push(term);
  }
  if (graded && terms[terms.length - 1]?.claim !== width - 2) throw new Error('graded ProductBasisV3 omits a primary claim');
  if (graded) {
    const cells: ReadonlyArray<readonly [bigint, bigint]> = knots.length >= 2
      ? knots.slice(0, -1).map((left, index) => [left, knots[index + 1] as bigint] as const)
      : [[knots[0] ?? 0n, knots[0] ?? 0n] as const];
    for (const [left, right] of cells) {
      let bound = 0n;
      for (const term of terms) {
        const a = evaluateBasisTerm(term, knots, knotDenominator, left, knotDenominator);
        const b = evaluateBasisTerm(term, knots, knotDenominator, right, knotDenominator);
        bound += a > b ? a : b;
      }
      if (bound > scale) throw new Error('graded ProductBasisV3 exceeds its simultaneous payout envelope');
    }
  }
  return Object.freeze({ bytes: new Uint8Array(bytes), kind: graded ? 'graded-exact-complement' : 'categorical-q1',
    productId, resultDomainId, coordinateDomainId, resultUnitId, evaluatorReleaseId, width, scale, knotDenominator,
    failurePayouts: Object.freeze(failurePayouts), knots: Object.freeze(knots), terms: Object.freeze(terms) });
}

/** Evaluate the exact ProductBasisV3 payout; one final floor occurs per graded term. */
export function evaluateRationalTerminalPayoutV3(input: Readonly<{
  basis: Uint8Array;
  resultOutcomeCount: number;
  terminalWinner: number;
  selectedOutcome: number;
  rawQuantity: bigint;
  terminalCoordinate: Readonly<{ numerator: bigint; denominator: bigint }> | null;
}>): RationalTerminalPayoutV3 {
  const basis = decodeRationalProductBasisV3(input.basis);
  if (basis.width === 0 || input.resultOutcomeCount === 0 || input.terminalWinner >= input.resultOutcomeCount
      || input.selectedOutcome >= basis.width || input.rawQuantity <= 0n || input.rawQuantity > MAX_U64) {
    throw new Error('terminal Product/result selector or raw quantity is outside its exact domain');
  }
  let payout: bigint; let scenario: RationalTerminalPayoutV3['scenario'];
  if (basis.kind === 'categorical-q1') {
    if (input.terminalCoordinate !== null || basis.width !== input.resultOutcomeCount) {
      throw new Error('categorical ProductBasisV3 or terminal scenario is noncanonical');
    }
    payout = input.selectedOutcome === input.terminalWinner ? 1n : 0n;
    scenario = 'categorical';
  } else {
    if (basis.width < 2) throw new Error('graded ProductBasisV3 has the wrong exact runtime tail');
    const failure = input.terminalWinner === input.resultOutcomeCount - 1;
    if (failure) {
      if (input.terminalCoordinate !== null) throw new Error('failure terminal cannot carry a rational coordinate');
      payout = basis.failurePayouts[input.selectedOutcome] as bigint;
      scenario = 'graded-failure';
    } else {
      const coordinate = input.terminalCoordinate;
      if (coordinate === null || coordinate.denominator <= 0n || coordinate.denominator > 0xffff_ffffn) {
        throw new Error('ordinary graded terminal requires one exact i64/u32 coordinate');
      }
      let primary = 0n; let total = 0n;
      for (const term of basis.terms) {
        const value = evaluateBasisTerm(term, basis.knots, basis.knotDenominator, coordinate.numerator, coordinate.denominator);
        total += value;
        if (term.claim === input.selectedOutcome) primary += value;
      }
      if (total > basis.scale) throw new Error('graded terminal payouts exceed their exact complement scale');
      payout = input.selectedOutcome === basis.width - 1 ? basis.scale - total : primary;
      scenario = 'graded-rational';
    }
  }
  const rawPayout = payout * input.rawQuantity;
  if (rawPayout > MAX_U64) throw new Error('terminal raw collateral payout exceeds u64::MAX');
  return Object.freeze({ scenario, payoutPerShard: payout, rawPayout, losing: rawPayout === 0n });
}

function identity(value: Uint8Array, field: string): Uint8Array {
  if (value.length !== 32 || isZero(value)) throw new Error(`${field} must be one nonzero 32-byte identity`);
  return value;
}

function key(value: string, field: string): Uint8Array {
  const parsed = new PublicKey(value);
  if (parsed.toBase58() !== value) throw new Error(`${field} must be canonical base58 text`);
  return identity(parsed.toBytes(), field);
}

function unsigned(value: bigint, field: string): bigint {
  if (value < 0n || value > MAX_U64) throw new Error(`${field} is outside canonical u64`);
  return value;
}

function putU16(output: Uint8Array, offset: number, value: number): void {
  new DataView(output.buffer, output.byteOffset + offset, 2).setUint16(0, value, true);
}

function putU32(output: Uint8Array, offset: number, value: number): void {
  new DataView(output.buffer, output.byteOffset + offset, 4).setUint32(0, value, true);
}

function putU64(output: Uint8Array, offset: number, value: bigint): void {
  new DataView(output.buffer, output.byteOffset + offset, 8).setBigUint64(0, unsigned(value, `u64 at ${offset}`), true);
}

function distinct(values: ReadonlyArray<Uint8Array>, field: string): void {
  const keys = values.map((value) => new PublicKey(value).toBase58());
  if (new Set(keys).size !== keys.length) throw new Error(`${field} aliases two distinct roles`);
}

export function encodeRationalTerminalHotRequestV3(input: RationalTerminalHotInputV3): Uint8Array {
  if (!Number.isInteger(input.outcomeCount) || input.outcomeCount <= 0 || input.outcomeCount > 0xffff_ffff
      || !Number.isInteger(input.selectedOutcome) || input.selectedOutcome < 0
      || input.selectedOutcome >= input.outcomeCount) {
    throw new Error('Product outcome width or selected outcome is outside runtime u32');
  }
  if (input.quantity === 0n || input.denominator === 0n || input.generation === 0n
      || input.expectedRepresentationRevision === MAX_U64) {
    throw new Error('terminal quantity, denominator, and next replay revision must be executable');
  }
  const rawShardBurn = unsigned(input.denominator, 'terminal denominator') * unsigned(input.quantity, 'terminal quantity');
  unsigned(rawShardBurn, 'terminal raw shard burn');
  const output = new Uint8Array(Abi.RATIONAL_TERMINAL_HOT_REQUEST_BYTES_V3);
  output.set(Abi.RATIONAL_TERMINAL_HOT_MAGIC_V3, Abi.RATIONAL_TERMINAL_HOT_MAGIC_OFFSET_V3);
  putU16(output, Abi.RATIONAL_TERMINAL_HOT_VERSION_OFFSET_V3, Abi.RATIONAL_TERMINAL_HOT_VERSION_V3);
  output[Abi.RATIONAL_TERMINAL_HOT_ACTION_OFFSET_V3] = Abi.ACTION_REDEEM_TERMINAL;
  output[Abi.RATIONAL_TERMINAL_HOT_CALLER_ROLE_OFFSET_V3] = Abi.CALLER_ROLE_TRADING;
  const identities: ReadonlyArray<readonly [number, Uint8Array]> = [
    [Abi.REQUEST_RELEASE_SET_OFFSET, identity(input.releaseSet, 'release set')],
    [Abi.REQUEST_MARKET_OFFSET, key(input.market, 'Market')],
    [Abi.REQUEST_GRAPH_ID_OFFSET, identity(input.graphId, 'representation graph')],
    [Abi.REQUEST_DESCRIPTOR_ID_OFFSET, identity(input.descriptorId, 'representation descriptor')],
    [Abi.REQUEST_ACTOR_OFFSET, key(input.actor, 'actor')],
    [Abi.REQUEST_RECEIPT_MINT_OFFSET, key(input.receiptMint, 'receipt Mint')],
    [Abi.REQUEST_REPRESENTATION_AUTHORITY_OFFSET, key(input.representationAuthority, 'representation authority')],
    [Abi.REQUEST_TOKEN_PROGRAM_OFFSET, key(input.tokenProgram, 'Token program')],
    [Abi.REQUEST_REALM_OFFSET, key(input.realm, 'Realm')],
    [Abi.REQUEST_COLLATERAL_RECIPIENT_OFFSET, key(input.collateralRecipient, 'collateral recipient')],
  ];
  for (const [offset, value] of identities) output.set(value, offset);
  // Receipt Account and parent context are canonically absent in this terminal
  // family request; the authenticated Hot adapter owns the latter digest.
  const scalars: ReadonlyArray<readonly [number, bigint]> = [
    [Abi.REQUEST_EXPECTED_REPRESENTATION_REVISION_OFFSET, input.expectedRepresentationRevision],
    [Abi.REQUEST_EXPECTED_CLAIMS_MARKET_REVISION_OFFSET, input.expectedClaimsMarketRevision],
    [Abi.REQUEST_EXPECTED_ACTOR_POSITION_REVISION_OFFSET, MAX_U64],
    [Abi.REQUEST_EXPECTED_CUSTODY_POSITION_REVISION_OFFSET, input.expectedCustodyPositionRevision],
    [Abi.REQUEST_EXPECTED_CUSTODY_REPLAY_REVISION_OFFSET, input.expectedCustodyReplayRevision],
    [Abi.REQUEST_GENERATION_OFFSET, input.generation], [Abi.REQUEST_QUANTITY_OFFSET, input.quantity],
    [Abi.REQUEST_DENOMINATOR_OFFSET, input.denominator], [Abi.REQUEST_EXPECTED_RECEIPT_SUPPLY_OFFSET, input.expectedReceiptSupply],
  ];
  for (const [offset, value] of scalars) putU64(output, offset, value);
  putU32(output, Abi.REQUEST_OUTCOME_COUNT_OFFSET, input.outcomeCount);
  putU32(output, Abi.REQUEST_SELECTED_OUTCOME_OFFSET, input.selectedOutcome);
  putU32(output, Abi.REQUEST_ASSET_COUNT_OFFSET, Abi.RATIONAL_TERMINAL_HOT_FIXED_ASSET_COUNT_V3);
  const assetOffset = Abi.REQUEST_HEADER_BYTES_V2;
  const assetIdentities = [
    key(input.asset.shardMint, 'shard Mint'), key(input.asset.actorShardAccount, 'actor shard account'),
    key(input.asset.structuredCustodyAccount, 'Structured custody account'), key(input.asset.claimsCustodyOwner, 'Claims custody owner'),
  ];
  distinct(assetIdentities.slice(0, 3), 'terminal asset');
  output.set(assetIdentities[0], assetOffset + Abi.ASSET_SHARD_MINT_OFFSET);
  output.set(assetIdentities[1], assetOffset + Abi.ASSET_ACTOR_SHARD_ACCOUNT_OFFSET);
  output.set(assetIdentities[2], assetOffset + Abi.ASSET_STRUCTURED_CUSTODY_ACCOUNT_OFFSET);
  output.set(assetIdentities[3], assetOffset + Abi.ASSET_CLAIMS_CUSTODY_OWNER_OFFSET);
  for (const [offset, value] of [
    [Abi.ASSET_COEFFICIENT_OFFSET, input.asset.coefficient],
    [Abi.ASSET_EXPECTED_SHARD_SUPPLY_OFFSET, input.asset.expectedShardSupply],
    [Abi.ASSET_EXPECTED_ACTOR_SHARDS_OFFSET, input.asset.expectedActorShards],
    [Abi.ASSET_EXPECTED_STRUCTURED_SHARDS_OFFSET, input.asset.expectedStructuredShards],
  ] as const) putU64(output, assetOffset + offset, value);
  if (input.asset.expectedActorShards < rawShardBurn) {
    throw new Error('actor shard balance cannot fund exact terminal burn');
  }
  return output;
}

export async function specializeRationalTerminalChildV2(family: Uint8Array): Promise<Readonly<{
  familyDigest: Uint8Array;
  childRequest: Uint8Array;
}>> {
  if (family.length !== Abi.RATIONAL_TERMINAL_HOT_REQUEST_BYTES_V3
      || !family.slice(0, 8).every((value, index) => value === Abi.RATIONAL_TERMINAL_HOT_MAGIC_V3[index])
      || !family.slice(Abi.REQUEST_PARENT_CONTEXT_OFFSET, Abi.REQUEST_PARENT_CONTEXT_OFFSET + 32).every((value) => value === 0)) {
    throw new Error('Rational terminal family bytes are not canonical Hot V3');
  }
  const familyDigest = await sha256(family);
  const childRequest = family.slice();
  childRequest.set(Abi.REQUEST_MAGIC_V2, Abi.REQUEST_MAGIC_OFFSET);
  putU16(childRequest, Abi.REQUEST_VERSION_OFFSET, Abi.PHYSICAL_ABI_VERSION_V2);
  childRequest.set(familyDigest, Abi.REQUEST_PARENT_CONTEXT_OFFSET);
  return Object.freeze({ familyDigest, childRequest });
}

/**
 * Compile the fixed terminal family and exact Claims child without guessing a
 * payout. The ProductV3 evaluator owns payout and explicitly admits zero.
 */
export async function compileRationalTerminalHotV3(input: RationalTerminalHotInputV3): Promise<RationalTerminalCompiledV3> {
  const familyBytes = encodeRationalTerminalHotRequestV3(input);
  const specialized = await specializeRationalTerminalChildV2(familyBytes);
  const childDigest = await sha256(specialized.childRequest);
  return Object.freeze({
    familyBytes,
    familyDigest: specialized.familyDigest,
    childRequest: specialized.childRequest,
    childDigest,
    claimsAccountCount: 49,
    rawQuantity: input.quantity,
    rawShardBurn: input.denominator * input.quantity,
    payoutPolicy: 'product-derived-including-zero',
  });
}
