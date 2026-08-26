import { AddressLookupTableAccount, PublicKey } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import {
  buildRationalOpenCandidateV4,
  rationalOpenClaimsMetasV4,
  type RationalOpenChainInspectionV4,
} from './rationalOpenChainV4';
import {
  compileRationalOpenHotV3,
  type RationalOpenAssetV3,
} from './rationalOpenHotV3';

const MAX_U64 = 18_446_744_073_709_551_615n;

function bytes(value: number): Uint8Array { return new Uint8Array(32).fill(value); }
function address(value: number): string { return new PublicKey(bytes(value)).toBase58(); }

function asset(seed: number, coefficient: bigint): RationalOpenAssetV3 {
  return Object.freeze({
    shardMint: address(seed), actorShardAccount: address(seed + 1), structuredCustodyAccount: address(seed + 2),
    claimsCustodyOwner: address(seed + 3), coefficient, expectedShardSupply: 1_000n,
    expectedActorShards: 500n, expectedStructuredShards: 100n,
  });
}

function claims(assets: ReadonlyArray<RationalOpenAssetV3>, structured: boolean) {
  return rationalOpenClaimsMetasV4({
    caller: address(1), trading: address(2), tradingProgramData: address(3), actor: address(4), authority: address(5),
    descriptorRaw: address(6), descriptorStaging: address(7), graphRaw: address(8), graphStaging: address(9),
    replay: address(10), aggregate: address(11), activation: address(12), claims: address(13), claimsProgramData: address(14),
    registry: address(15), market: address(16), core: address(17), coreProgramData: address(18), receiptMint: address(19),
    receiptAccount: structured ? address(20) : null, actorPosition: structured ? null : address(21), linkedRaw: address(22),
    linkedStaging: address(23), productRaw: address(24), productStaging: address(25), domainRaw: address(26),
    domainStaging: address(27), portfolioRaw: address(28), portfolioStaging: address(29), structured,
    assets: assets.map((row, index) => Object.freeze({ position: address(100 + index), asset: row })),
  });
}

describe('chain-derived Rational open V4', () => {
  it('constructs the exact selected Claims36 frame with canonical vacant aliases', () => {
    const metas = claims([asset(40, 10n)], false);
    expect(metas).toHaveLength(36);
    expect(metas[0]).toMatchObject({ address: address(1), isSigner: true, isWritable: false });
    expect(metas[21]?.address).toBe(address(13));
    expect(metas[23]).toMatchObject({ address: address(21), isWritable: true });
    expect(metas[32]).toMatchObject({ address: address(100), isWritable: true });
    expect(metas[35]).toMatchObject({ address: address(42), isWritable: false });
  });

  it('constructs the exact Structured 32+4N frame, including zero-coefficient Product coordinates', () => {
    const metas = claims([asset(40, 10n), asset(50, 0n), asset(60, 7n)], true);
    expect(metas).toHaveLength(44);
    expect(metas[21]).toMatchObject({ address: address(20), isWritable: true });
    expect(metas[23]).toMatchObject({ address: address(13), isWritable: false });
    expect(metas[36]).toMatchObject({ address: address(101), isWritable: false });
    expect(metas[38]).toMatchObject({ address: address(51), isWritable: true });
    expect(metas[39]).toMatchObject({ address: address(52), isWritable: true });
  });

  it('compiles one exact blocked v0+ALT packet with only payer and actor wallet signers', async () => {
    const actor = address(220); const payer = address(221); const market = address(222);
    const rows = [asset(70, 10n)];
    const family = await compileRationalOpenHotV3({
      action: 'denominate', releaseSet: bytes(223), market, graphId: bytes(224), descriptorId: bytes(225), actor,
      receiptMint: address(226), receiptAccount: null, representationAuthority: address(228), tokenProgram: address(229),
      expectedRepresentationRevision: 0n, expectedClaimsMarketRevision: 4n,
      expectedActorPositionRevision: 5n, expectedCustodyPositionRevision: 6n,
      generation: 3n, quantity: 2n, denominator: 10n, expectedReceiptSupply: 0n,
      outcomeCount: 3, selectedOutcome: 1, assets: rows,
    });
    const fixed = Array.from({ length: 38 }, (_, index) => Object.freeze({
      address: index === 0 ? market : address(120 + index), isSigner: false, isWritable: index === 1,
    }));
    const physical = [
      Object.freeze({ address: actor, isSigner: true, isWritable: false }),
      ...Array.from({ length: 30 }, (_, index) => Object.freeze({ address: address(10 + index), isSigner: false, isWritable: index % 5 === 0 })),
    ];
    const table = new AddressLookupTableAccount({
      key: new PublicKey(bytes(230)),
      state: { deactivationSlot: MAX_U64, lastExtendedSlot: 0, lastExtendedSlotStartIndex: 0, authority: undefined,
        addresses: [...fixed, ...physical].map((meta) => new PublicKey(meta.address)) },
    });
    const inspection = Object.freeze({
      observedSlot: '80', action: 'denominate' as const, payer, actor, market, generation: 3n,
      outcomeCount: 3, selectedOutcome: 1, rawQuantity: 2n, displayDecimals: 255,
      descriptorId: bytes(225), tokenBehaviorDigest: bytes(231), capabilityDigest: bytes(232), rootDigest: bytes(233),
      family, fixedAccounts: fixed, physicalClaimsAccounts: physical, lookupTable: table,
      executionStatus: 'blocked' as const, refusal: 'checked common Hot release pending',
    }) satisfies RationalOpenChainInspectionV4;
    const candidate = buildRationalOpenCandidateV4(inspection, address(234));
    expect(candidate.outerBytes).toHaveLength(128 + 648);
    expect(candidate.logicalClaimsAccounts).toBe(36);
    expect(candidate.physicalClaimsAccounts).toBe(31);
    expect(candidate.loadedAddresses).toBeGreaterThan(0);
    expect(candidate.wireBytes.length).toBeLessThanOrEqual(1232);
    expect([...candidate.requiredSigners].sort()).toEqual([actor, payer].sort());
    expect(candidate.executionStatus).toBe('blocked');
  });
});
