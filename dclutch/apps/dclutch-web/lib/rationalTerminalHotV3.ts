import { PublicKey } from '@solana/web3.js';

import { isZero, requireNonzero, requireZero, sha256, slice } from './bytes';
import * as Abi from './generated/rationalTerminalHotV3';

const MAX_U64 = 18_446_744_073_709_551_615n;

/**
 * One asset row of a terminal request.
 *
 * Physical ABI v3 cut this row from 160 bytes to 64. The shard Mint, the
 * Structured custody Account and the Claims custody owner were all RE-DERIVED
 * by the Claims adapter from coordinates the request already carries, so
 * sending them was asking the wallet to restate three PDAs the program
 * recomputes anyway -- and a wallet that got one wrong was writing a key the
 * program would then disagree with. They are gone from the type as well as
 * from the wire, so a caller that still has them cannot quietly pass them.
 */
export type RationalTerminalAssetV3 = Readonly<{
  actorShardAccount: string;
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

// `distinct` lived here and checked that the shard Mint, the actor shard
// Account and the Structured custody Account named three different roles. Its
// three operands became one when physical ABI v3 stopped carrying the two
// derived keys, and a one-element uniqueness check is a check of nothing, so
// the helper is not restored HERE -- a request encoder has one operand and
// always will.
//
// It came back where its operands are: `rationalOpenClaimsMetasV4` in
// rationalOpenChainV4.ts, which assembles the frame and therefore holds the
// receipt pair and every coordinate role at once. Its chain-side owner is
// `ClaimsSbfError::ReceiptAlias`, raised before the adapter derives anything,
// and its grammar-side owner is `ResolvedRequestV2::join`.

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
  // Every coordinate below is a RATIONAL_TERMINAL_HOT_* name, which is the
  // terminal class of the action-conditional v3 header already resolved. The
  // class-free REQUEST_*_OFFSET names this used to spell no longer exist for
  // the fields that moved: `realm` and `collateralRecipient` sit at 348 and 380
  // in a terminal request and are absent from the other two classes entirely,
  // so a class-free name for them could only have been read against the wrong
  // action.
  const identities: ReadonlyArray<readonly [number, Uint8Array]> = [
    [Abi.RATIONAL_TERMINAL_HOT_RELEASE_SET_OFFSET_V3, identity(input.releaseSet, 'release set')],
    [Abi.RATIONAL_TERMINAL_HOT_MARKET_OFFSET_V3, key(input.market, 'Market')],
    [Abi.RATIONAL_TERMINAL_HOT_GRAPH_ID_OFFSET_V3, identity(input.graphId, 'representation graph')],
    [Abi.RATIONAL_TERMINAL_HOT_DESCRIPTOR_ID_OFFSET_V3, identity(input.descriptorId, 'representation descriptor')],
    [Abi.RATIONAL_TERMINAL_HOT_ACTOR_OFFSET_V3, key(input.actor, 'actor')],
    [Abi.RATIONAL_TERMINAL_HOT_RECEIPT_MINT_OFFSET_V3, key(input.receiptMint, 'receipt Mint')],
    [Abi.RATIONAL_TERMINAL_HOT_REPRESENTATION_AUTHORITY_OFFSET_V3, key(input.representationAuthority, 'representation authority')],
    [Abi.RATIONAL_TERMINAL_HOT_TOKEN_PROGRAM_OFFSET_V3, key(input.tokenProgram, 'Token program')],
    [Abi.RATIONAL_TERMINAL_HOT_REALM_OFFSET_V3, key(input.realm, 'Realm')],
    [Abi.RATIONAL_TERMINAL_HOT_COLLATERAL_RECIPIENT_OFFSET_V3, key(input.collateralRecipient, 'collateral recipient')],
  ];
  for (const [offset, value] of identities) output.set(value, offset);
  // Receipt Account and parent context are canonically absent in this terminal
  // family request; the authenticated Hot adapter owns the latter digest.
  // The actor-Position revision is NOT written. A terminal redemption has no
  // actor Position, so its revision was always the absent sentinel; v3 stopped
  // carrying a field whose value the action already determines, and the decoder
  // supplies it. Writing MAX_U64 here would now land on the request magic.
  const scalars: ReadonlyArray<readonly [number, bigint]> = [
    [Abi.RATIONAL_TERMINAL_HOT_EXPECTED_REPRESENTATION_REVISION_OFFSET_V3, input.expectedRepresentationRevision],
    [Abi.RATIONAL_TERMINAL_HOT_EXPECTED_CLAIMS_MARKET_REVISION_OFFSET_V3, input.expectedClaimsMarketRevision],
    [Abi.RATIONAL_TERMINAL_HOT_EXPECTED_CUSTODY_POSITION_REVISION_OFFSET_V3, input.expectedCustodyPositionRevision],
    [Abi.RATIONAL_TERMINAL_HOT_EXPECTED_CUSTODY_REPLAY_REVISION_OFFSET_V3, input.expectedCustodyReplayRevision],
    [Abi.RATIONAL_TERMINAL_HOT_GENERATION_OFFSET_V3, input.generation],
    [Abi.RATIONAL_TERMINAL_HOT_QUANTITY_OFFSET_V3, input.quantity],
    [Abi.RATIONAL_TERMINAL_HOT_DENOMINATOR_OFFSET_V3, input.denominator],
    [Abi.RATIONAL_TERMINAL_HOT_EXPECTED_RECEIPT_SUPPLY_OFFSET_V3, input.expectedReceiptSupply],
  ];
  for (const [offset, value] of scalars) putU64(output, offset, value);
  putU32(output, Abi.RATIONAL_TERMINAL_HOT_OUTCOME_COUNT_OFFSET_V3, input.outcomeCount);
  putU32(output, Abi.RATIONAL_TERMINAL_HOT_SELECTED_OUTCOME_OFFSET_V3, input.selectedOutcome);
  // The asset count is not written either: v3 derives it from the action, which
  // for a terminal request is exactly RATIONAL_TERMINAL_HOT_FIXED_ASSET_COUNT_V3.
  // The asset coordinates below are already absolute within the family request.
  output.set(
    key(input.asset.actorShardAccount, 'actor shard account'),
    Abi.RATIONAL_TERMINAL_HOT_ASSET_ACTOR_SHARD_ACCOUNT_OFFSET_V3,
  );
  for (const [offset, value] of [
    [Abi.RATIONAL_TERMINAL_HOT_ASSET_COEFFICIENT_OFFSET_V3, input.asset.coefficient],
    [Abi.RATIONAL_TERMINAL_HOT_ASSET_EXPECTED_SHARD_SUPPLY_OFFSET_V3, input.asset.expectedShardSupply],
    [Abi.RATIONAL_TERMINAL_HOT_ASSET_EXPECTED_ACTOR_SHARDS_OFFSET_V3, input.asset.expectedActorShards],
    [Abi.RATIONAL_TERMINAL_HOT_ASSET_EXPECTED_STRUCTURED_SHARDS_OFFSET_V3, input.asset.expectedStructuredShards],
  ] as const) putU64(output, offset, value);
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
      || !family
        .slice(
          Abi.RATIONAL_TERMINAL_HOT_PARENT_CONTEXT_OFFSET_V3,
          Abi.RATIONAL_TERMINAL_HOT_PARENT_CONTEXT_OFFSET_V3 + 32,
        )
        .every((value) => value === 0)) {
    throw new Error('Rational terminal family bytes are not canonical Hot V3');
  }
  const familyDigest = await sha256(family);
  const childRequest = family.slice();
  // Magic, version and parent context are all in the COMMON PREFIX, placed
  // identically in all three classes, so the class-free names are the honest
  // ones here: this rewrite is the same edit whatever the action.
  childRequest.set(Abi.REQUEST_MAGIC_V2, Abi.REQUEST_MAGIC_OFFSET_V3);
  putU16(childRequest, Abi.REQUEST_VERSION_OFFSET_V3, Abi.PHYSICAL_ABI_VERSION_V3);
  childRequest.set(familyDigest, Abi.REQUEST_PARENT_CONTEXT_OFFSET_V3);
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
