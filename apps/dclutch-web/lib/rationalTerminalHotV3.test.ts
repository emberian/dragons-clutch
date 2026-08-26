import { PublicKey } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import * as Abi from './generated/rationalTerminalHotV3';
import { compileRationalTerminalHotV3, encodeRationalTerminalHotRequestV3, evaluateRationalTerminalPayoutV3, specializeRationalTerminalChildV2 } from './rationalTerminalHotV3';

function key(seed: number): string { return new PublicKey(new Uint8Array(32).fill(seed)).toBase58(); }
function id(seed: number): Uint8Array { return new Uint8Array(32).fill(seed); }

function input() {
  return {
    releaseSet: id(1), market: key(2), graphId: id(3), descriptorId: id(4), actor: key(5),
    receiptMint: key(6), representationAuthority: key(7), tokenProgram: key(8), realm: key(9),
    collateralRecipient: key(10), expectedRepresentationRevision: 4n, expectedClaimsMarketRevision: 5n,
    expectedCustodyPositionRevision: 6n, expectedCustodyReplayRevision: 7n, generation: 8n,
    quantity: 2n, denominator: 10n, expectedReceiptSupply: 0n, outcomeCount: 258, selectedOutcome: 257,
    asset: { shardMint: key(11), actorShardAccount: key(12), structuredCustodyAccount: key(13),
      claimsCustodyOwner: key(14), coefficient: 1n, expectedShardSupply: 100n,
      expectedActorShards: 20n, expectedStructuredShards: 0n },
  } as const;
}

function putU16(output: Uint8Array, offset: number, value: number): void { new DataView(output.buffer).setUint16(offset, value, true); }
function putU32(output: Uint8Array, offset: number, value: number): void { new DataView(output.buffer).setUint32(offset, value, true); }
function putU64(output: Uint8Array, offset: number, value: bigint): void { new DataView(output.buffer).setBigUint64(offset, value, true); }

function categoricalBasis(): Uint8Array {
  const output = new Uint8Array(256); output.set(new TextEncoder().encode('DCLTPAY3'));
  putU16(output, 8, 3); putU16(output, 10, 256); putU32(output, 12, output.length); output[16] = 1;
  putU32(output, 20, 3); putU64(output, 160, 1n); putU64(output, 168, 1n); return output;
}

function gradedBasis(): Uint8Array {
  // Two primary constant terms of 3 and 5, then exact complement 2 at Q=10.
  const output = new Uint8Array(256 + 3 * 8 + 2 * 32); output.set(new TextEncoder().encode('DCLTPAY3'));
  putU16(output, 8, 3); putU16(output, 10, 256); putU32(output, 12, output.length); output[16] = 2; output[17] = 1;
  putU32(output, 20, 3); putU32(output, 24, 0); putU32(output, 28, 2); putU64(output, 160, 10n); putU64(output, 168, 1n);
  [4n, 0n, 6n].forEach((value, index) => putU64(output, 256 + index * 8, value));
  for (const [index, amplitude] of [3n, 5n].entries()) {
    const offset = 280 + index * 32; putU32(output, offset, index); output[offset + 4] = 0; putU64(output, offset + 24, amplitude);
  }
  return output;
}

describe('Rational terminal Hot V3 codec', () => {
  it('encodes runtime-u32 terminal intent and specializes the exact child digest', async () => {
    const family = encodeRationalTerminalHotRequestV3(input());
    expect(family).toHaveLength(648);
    expect(family.slice(0, 8)).toEqual(Abi.RATIONAL_TERMINAL_HOT_MAGIC_V3);
    expect(family.slice(144, 176)).toEqual(new Uint8Array(32));
    expect(new DataView(family.buffer).getUint32(472, true)).toBe(258);
    expect(new DataView(family.buffer).getUint32(476, true)).toBe(257);
    const specialized = await specializeRationalTerminalChildV2(family);
    expect(specialized.childRequest.slice(0, 8)).toEqual(Abi.REQUEST_MAGIC_V2);
    expect(specialized.childRequest.slice(144, 176)).toEqual(specialized.familyDigest);
  });

  it('refuses unfunded burns, selected-outcome overflow, and asset aliasing', () => {
    expect(() => encodeRationalTerminalHotRequestV3({ ...input(), asset: { ...input().asset, expectedActorShards: 19n } })).toThrow(/cannot fund/);
    expect(() => encodeRationalTerminalHotRequestV3({ ...input(), selectedOutcome: 258 })).toThrow(/runtime u32/);
    expect(() => encodeRationalTerminalHotRequestV3({ ...input(), asset: { ...input().asset, actorShardAccount: input().asset.shardMint } })).toThrow(/aliases/);
    expect(() => encodeRationalTerminalHotRequestV3({ ...input(), quantity: 18_446_744_073_709_551_615n })).toThrow(/outside canonical u64/);
  });

  it('keeps payout Product-derived and explicitly permits the losing zero-payout route', async () => {
    const compiled = await compileRationalTerminalHotV3({
      ...input(), expectedCustodyReplayRevision: 18_446_744_073_709_551_615n,
    });
    expect(compiled.claimsAccountCount).toBe(49);
    expect(compiled.rawShardBurn).toBe(20n);
    expect(compiled.payoutPolicy).toBe('product-derived-including-zero');
    expect(compiled.childRequest.slice(144, 176)).toEqual(compiled.familyDigest);
  });

  it('evaluates categorical wins and losses without inventing a positive Custody transfer', () => {
    expect(evaluateRationalTerminalPayoutV3({ basis: categoricalBasis(), resultOutcomeCount: 3, terminalWinner: 1,
      selectedOutcome: 1, rawQuantity: 7n, terminalCoordinate: null })).toMatchObject({ rawPayout: 7n, losing: false, scenario: 'categorical' });
    expect(evaluateRationalTerminalPayoutV3({ basis: categoricalBasis(), resultOutcomeCount: 3, terminalWinner: 1,
      selectedOutcome: 2, rawQuantity: 7n, terminalCoordinate: null })).toMatchObject({ rawPayout: 0n, losing: true, scenario: 'categorical' });
  });

  it('evaluates graded ordinary and explicit failure partitions with the same exact raw quantity', () => {
    expect(evaluateRationalTerminalPayoutV3({ basis: gradedBasis(), resultOutcomeCount: 4, terminalWinner: 1,
      selectedOutcome: 2, rawQuantity: 2n, terminalCoordinate: { numerator: 7n, denominator: 3n } })).toMatchObject({ payoutPerShard: 2n, rawPayout: 4n, scenario: 'graded-rational' });
    expect(evaluateRationalTerminalPayoutV3({ basis: gradedBasis(), resultOutcomeCount: 4, terminalWinner: 3,
      selectedOutcome: 0, rawQuantity: 2n, terminalCoordinate: null })).toMatchObject({ payoutPerShard: 4n, rawPayout: 8n, scenario: 'graded-failure' });
  });
});
