import { PublicKey } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import * as Abi from './generated/rationalTerminalHotV3';
import {
  RATIONAL_OPEN_ABSENT_REVISION_V3,
  compileRationalOpenHotV3,
  encodeRationalOpenHotRequestV3,
  type RationalOpenHotInputV3,
} from './rationalOpenHotV3';

function key(seed: number): string { return new PublicKey(new Uint8Array(32).fill(seed)).toBase58(); }
function id(seed: number): Uint8Array { return new Uint8Array(32).fill(seed); }

function asset(seed: number, actor = 80n) {
  return {
    shardMint: key(seed), actorShardAccount: key(seed + 20), structuredCustodyAccount: key(seed + 40),
    claimsCustodyOwner: key(seed + 60), coefficient: BigInt(seed), expectedShardSupply: 1000n,
    expectedActorShards: actor, expectedStructuredShards: 30n,
  } as const;
}

function base(): Omit<RationalOpenHotInputV3, 'action' | 'receiptAccount' | 'expectedClaimsMarketRevision' | 'expectedActorPositionRevision' | 'expectedCustodyPositionRevision' | 'selectedOutcome' | 'assets'> {
  return {
    releaseSet: id(1), market: key(2), graphId: id(3), descriptorId: id(4), actor: key(5),
    receiptMint: key(6), representationAuthority: key(7), tokenProgram: key(8),
    expectedRepresentationRevision: 9n, generation: 10n, quantity: 2n, denominator: 10n,
    expectedReceiptSupply: 20n, outcomeCount: 3,
  };
}

describe('Rational open Hot V3 / CapabilityV4 compiler', () => {
  it('compiles one selected raw-atom action into its exact family and child', async () => {
    const input: RationalOpenHotInputV3 = {
      ...base(), action: 'reconstitute', receiptAccount: null, selectedOutcome: 2,
      expectedClaimsMarketRevision: 11n, expectedActorPositionRevision: 12n,
      expectedCustodyPositionRevision: 13n, assets: [asset(14)],
    };
    const compiled = await compileRationalOpenHotV3(input);
    expect(compiled.familyBytes).toHaveLength(648);
    expect(new TextDecoder().decode(compiled.familyBytes.slice(0, 8))).toBe('DCRROH03');
    expect(compiled.childRequest.slice(0, 8)).toEqual(Abi.REQUEST_MAGIC_V2);
    expect(compiled.childRequest.slice(Abi.REQUEST_PARENT_CONTEXT_OFFSET, Abi.REQUEST_PARENT_CONTEXT_OFFSET + 32)).toEqual(compiled.familyDigest);
    expect(compiled.claimsAccountCount).toBe(36);
    expect(compiled.rawShardDeltas).toEqual([28n]);
    expect(compiled.rawReceiptDelta).toBe(0n);
  });

  it('keeps Structured width runtime-polymorphic and uses no decimal conversion', async () => {
    const compiled = await compileRationalOpenHotV3({
      ...base(), action: 'issue-structured', receiptAccount: key(9), selectedOutcome: null,
      expectedClaimsMarketRevision: RATIONAL_OPEN_ABSENT_REVISION_V3,
      expectedActorPositionRevision: RATIONAL_OPEN_ABSENT_REVISION_V3,
      expectedCustodyPositionRevision: RATIONAL_OPEN_ABSENT_REVISION_V3,
      assets: [asset(11), asset(12), asset(13)],
    });
    expect(compiled.familyBytes).toHaveLength(488 + 3 * 160);
    expect(compiled.assetCount).toBe(3);
    expect(compiled.claimsAccountCount).toBe(44);
    expect(compiled.rawReceiptDelta).toBe(2n);
    expect(compiled.rawShardDeltas).toEqual([22n, 24n, 26n]);
    expect(new DataView(compiled.familyBytes.buffer).getUint32(480, true)).toBe(3);
    expect(new DataView(compiled.familyBytes.buffer).getUint32(476, true)).toBe(0xffff_ffff);
  });

  it('refuses shape substitution, aliases, unfunded raw debits, and u64 overflow', () => {
    const selected: RationalOpenHotInputV3 = {
      ...base(), action: 'denominate', receiptAccount: null, selectedOutcome: 1,
      expectedClaimsMarketRevision: 11n, expectedActorPositionRevision: 12n,
      expectedCustodyPositionRevision: 13n, assets: [asset(14)],
    };
    expect(() => encodeRationalOpenHotRequestV3({ ...selected, receiptAccount: key(9) })).toThrow(/selected open/);
    expect(() => encodeRationalOpenHotRequestV3({ ...selected, assets: [{ ...asset(14), actorShardAccount: key(14) }] })).toThrow(/aliases/);
    expect(() => encodeRationalOpenHotRequestV3({ ...selected, action: 'reconstitute', assets: [asset(14, 27n)] })).toThrow(/cannot fund/);
    expect(() => encodeRationalOpenHotRequestV3({ ...selected, quantity: 18_446_744_073_709_551_615n })).toThrow(/outside canonical u64/);
  });
});
