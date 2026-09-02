import { AddressLookupTableAccount, PublicKey } from '@solana/web3.js';
import { beforeAll, describe, expect, it } from 'vitest';

import * as Abi from './generated/rationalTerminalHotV3';
import * as Core from './generated/coreFound';
import { HOT_EXECUTION_ENVELOPE_BYTES_V3, HOT_FIXED_ACCOUNT_COUNT_V3 } from './generated/directInlineV3';
import {
  buildRationalOpenCandidateV4,
  projectRationalOpenTokenPoststateV4,
  rationalOpenClaimsMetasV4,
  rationalOpenChainSummaryV4,
  verifyRationalOpenFinalizedPoststateV4,
  type RationalOpenChainInspectionV4,
  type RationalOpenPoststateV4,
} from './rationalOpenChainV4';
import {
  compileRationalOpenHotV3,
  type RationalOpenAssetV3,
} from './rationalOpenHotV3';
import { loadRationalOpenWasmV1ForTest } from './rationalOpenWasmV1.testSupport';
import { TOKEN_2022_PROGRAM_ID } from './rationalTokenV2';
import { type RpcAccount } from './rpc';

const MAX_U64 = 18_446_744_073_709_551_615n;

function bytes(value: number): Uint8Array { return new Uint8Array(32).fill(value); }
function address(value: number): string { return new PublicKey(bytes(value)).toBase58(); }

function putU16(output: Uint8Array, offset: number, value: number): void {
  new DataView(output.buffer, output.byteOffset + offset, 2).setUint16(0, value, true);
}

function putU32(output: Uint8Array, offset: number, value: number): void {
  new DataView(output.buffer, output.byteOffset + offset, 4).setUint32(0, value, true);
}

function putU64(output: Uint8Array, offset: number, value: bigint): void {
  new DataView(output.buffer, output.byteOffset + offset, 8).setBigUint64(0, value, true);
}

function rpcAccount(owner: string, data: Uint8Array): RpcAccount {
  return Object.freeze({ owner, data, executable: false, lamports: '1000000', space: data.length });
}

function mintAccount(mint: string, controller: string, supply: bigint): RpcAccount {
  const base = new Uint8Array(166);
  putU32(base, 0, 1); base.set(new PublicKey(controller).toBytes(), 4);
  putU64(base, 36, supply); base[44] = 0; base[45] = 1; base[165] = 1;
  const extension = (kind: number) => {
    const value = new Uint8Array(36); putU16(value, 0, kind); putU16(value, 2, 32);
    value.set(new PublicKey(controller).toBytes(), 4); return value;
  };
  const output = new Uint8Array(base.length + 72); output.set(base); output.set(extension(3), 166); output.set(extension(28), 202);
  return rpcAccount(TOKEN_2022_PROGRAM_ID, output);
}

function tokenAccount(mint: string, owner: string, amount: bigint): RpcAccount {
  const output = new Uint8Array(165); output.set(new PublicKey(mint).toBytes(), 0); output.set(new PublicKey(owner).toBytes(), 32);
  putU64(output, 64, amount); output[108] = 1; return rpcAccount(TOKEN_2022_PROGRAM_ID, output);
}

function replayAccount(claims: string, descriptor: Uint8Array, actor: string, revision: bigint): RpcAccount {
  const output = new Uint8Array(88); output.set(new TextEncoder().encode('DCRRREP2'), 0); putU16(output, 8, 2);
  output.set(descriptor, 16); output.set(new PublicKey(actor).toBytes(), 48); putU64(output, 80, revision);
  return rpcAccount(claims, output);
}

function aggregateAccount(input: Readonly<{
  claims: string; market: string; releaseSet: Uint8Array; registry: string; product: Uint8Array; basis: Uint8Array;
  realm: Uint8Array; custody: Uint8Array; generation: bigint; revision: bigint; balances: ReadonlyArray<bigint>;
}>): RpcAccount {
  const output = new Uint8Array(Core.LIABILITY_BASIS_MARKET_HEADER_BYTES_V2 + input.balances.length * 8);
  output.set(Core.LIABILITY_BASIS_MARKET_MAGIC_V2, 0); putU16(output, 8, Core.LIABILITY_BASIS_STATE_VERSION_V2);
  putU32(output, Core.LIABILITY_BASIS_MARKET_CLAIM_COUNT_OFFSET, input.balances.length);
  putU64(output, Core.LIABILITY_BASIS_MARKET_REVISION_OFFSET, input.revision);
  output.set(new PublicKey(input.market).toBytes(), Core.LIABILITY_BASIS_MARKET_LOGICAL_ID_OFFSET);
  output.set(input.releaseSet, Core.LIABILITY_BASIS_MARKET_RELEASE_SET_OFFSET);
  output.set(new PublicKey(input.registry).toBytes(), Core.LIABILITY_BASIS_MARKET_REGISTRY_OFFSET);
  output.set(input.product, Core.LIABILITY_BASIS_MARKET_PRODUCT_OFFSET); output.set(input.basis, Core.LIABILITY_BASIS_MARKET_BASIS_OFFSET);
  output.set(input.realm, Core.LIABILITY_BASIS_MARKET_REALM_OFFSET); output.set(input.custody, Core.LIABILITY_BASIS_MARKET_CUSTODY_CONTEXT_OFFSET);
  putU64(output, Core.LIABILITY_BASIS_MARKET_GENERATION_OFFSET, input.generation);
  input.balances.forEach((value, index) => putU64(output, Core.LIABILITY_BASIS_MARKET_HEADER_BYTES_V2 + index * 8, value));
  return rpcAccount(input.claims, output);
}

function positionAccount(input: Readonly<{
  claims: string; aggregate: string; owner: string; basis: Uint8Array; revision: bigint; balances: ReadonlyArray<bigint>;
}>): RpcAccount {
  const output = new Uint8Array(Core.LIABILITY_BASIS_POSITION_HEADER_BYTES_V2 + input.balances.length * 8);
  output.set(Core.LIABILITY_BASIS_POSITION_MAGIC_V2, 0); putU16(output, 8, Core.LIABILITY_BASIS_STATE_VERSION_V2);
  putU32(output, Core.LIABILITY_BASIS_POSITION_CLAIM_COUNT_OFFSET, input.balances.length);
  putU64(output, Core.LIABILITY_BASIS_POSITION_REVISION_OFFSET, input.revision);
  output.set(new PublicKey(input.aggregate).toBytes(), Core.LIABILITY_BASIS_POSITION_MARKET_OFFSET);
  output.set(new PublicKey(input.owner).toBytes(), Core.LIABILITY_BASIS_POSITION_OWNER_OFFSET);
  output.set(input.basis, Core.LIABILITY_BASIS_POSITION_BASIS_OFFSET);
  input.balances.forEach((value, index) => putU64(output, Core.LIABILITY_BASIS_POSITION_HEADER_BYTES_V2 + index * 8, value));
  return rpcAccount(input.claims, output);
}

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
  beforeAll(loadRationalOpenWasmV1ForTest);

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

  // The wallet-side twin of `ClaimsSbfError::ReceiptAlias`. Physical ABI v3
  // deleted the `distinct` helper because two of its three operands left the
  // wire; all four still arrive in this FRAME, and these are the substitutions
  // a caller can actually make.
  it('refuses a Claims frame that names the receipt, or one account twice, as a coordinate role', () => {
    const honest = [asset(40, 10n), asset(50, 0n), asset(60, 7n)];
    expect(claims(honest, true)).toHaveLength(44);
    const substitute = (index: number, field: keyof RationalOpenAssetV3, value: string) => {
      const rows = honest.map((row, at) => (at === index ? Object.freeze({ ...row, [field]: value }) : row));
      return () => claims(rows, true);
    };
    // The receipt Mint and the receipt Account, at every coordinate role that
    // can be handed one.
    for (const [field, role] of [['shardMint', 'shard Mint 2'], ['structuredCustodyAccount', 'Structured custody Account 2'],
      ['actorShardAccount', 'actor shard Account 2']] as const) {
      expect(substitute(2, field, address(19))).toThrow(`both the receipt Mint and ${role}`);
      expect(substitute(2, field, address(20))).toThrow(`both the receipt Account and ${role}`);
    }
    // And the pairwise half: two coordinates presenting one shard Mint.
    expect(substitute(2, 'shardMint', address(40))).toThrow('both shard Mint 0 and shard Mint 2');
    // A selected frame has no receipt Account, and its vacant slots are the
    // Claims program, which is not a coordinate role and must not trip this.
    expect(claims([asset(40, 10n)], false)).toHaveLength(36);
  });

  it('projects all four Rust-owned raw-delta plans into exact Token-2022 poststates', () => {
    const token = (seed: number) => Object.freeze({
      mint: address(seed), mintSupply: 100n, actorAccount: address(seed + 1), actorAmount: 60n,
      structuredAccount: address(seed + 2), structuredAmount: 40n,
    });
    const selected = token(40);
    expect(projectRationalOpenTokenPoststateV4({ action: 'denominate', rawQuantity: 2n,
      rawShardDeltas: [20n], receipt: null, assets: [selected] }).assets[0])
      .toMatchObject({ mintSupply: 120n, actorAmount: 80n, structuredAmount: 40n });
    expect(projectRationalOpenTokenPoststateV4({ action: 'reconstitute', rawQuantity: 2n,
      rawShardDeltas: [20n], receipt: null, assets: [selected] }).assets[0])
      .toMatchObject({ mintSupply: 80n, actorAmount: 40n, structuredAmount: 40n });

    const receipt = Object.freeze({ mint: address(70), supply: 9n, account: address(71), amount: 3n });
    const issued = projectRationalOpenTokenPoststateV4({ action: 'issue-structured', rawQuantity: 2n,
      rawShardDeltas: [20n, 14n], receipt, assets: [token(50), token(60)] });
    expect(issued.receipt).toMatchObject({ supply: 11n, amount: 5n });
    expect(issued.assets.map((row) => [row.mintSupply, row.actorAmount, row.structuredAmount]))
      .toEqual([[100n, 40n, 60n], [100n, 46n, 54n]]);
    const unwrapped = projectRationalOpenTokenPoststateV4({ action: 'unwrap-structured', rawQuantity: 2n,
      rawShardDeltas: [20n, 14n], receipt, assets: [token(50), token(60)] });
    expect(unwrapped.receipt).toMatchObject({ supply: 7n, amount: 1n });
    expect(unwrapped.assets.map((row) => [row.mintSupply, row.actorAmount, row.structuredAmount]))
      .toEqual([[100n, 80n, 20n], [100n, 74n, 26n]]);

    expect(() => projectRationalOpenTokenPoststateV4({ action: 'reconstitute', rawQuantity: 2n,
      rawShardDeltas: [101n], receipt: null, assets: [selected] })).toThrow('supply underflows');
    expect(() => projectRationalOpenTokenPoststateV4({ action: 'issue-structured', rawQuantity: 2n,
      rawShardDeltas: [20n], receipt: { ...receipt, supply: MAX_U64 }, assets: [selected] })).toThrow('supply overflows');
  });

  it('reacquires and authenticates every exact finalized Claims and Token-2022 poststate', async () => {
    const claimsProgram = address(80); const actor = address(81); const authority = address(82);
    const aggregate = address(83); const market = address(84); const registry = address(85);
    const descriptor = bytes(86); const releaseSet = bytes(87); const product = bytes(88);
    const realm = bytes(89); const basis = bytes(90); const custodyContext = bytes(91);
    const replay = address(92); const actorPosition = address(93); const custodyPosition = address(94);
    const custodyOwner = address(95); const mint = address(96); const actorToken = address(97); const structured = address(98);
    const poststate = Object.freeze({
      context: Object.freeze({ claimsProgram, descriptorId: descriptor, actor, representationAuthority: authority,
        aggregate, market, releaseSet, registry, product, realm, generation: 3n, outcomes: 3, basis, custodyContext }),
      replay: Object.freeze({ address: replay, revision: 4n }),
      aggregate: Object.freeze({ address: aggregate, revision: 5n, balances: Object.freeze([10n, 20n, 30n]) }),
      positions: Object.freeze([
        Object.freeze({ address: actorPosition, owner: actor, revision: 6n, balances: Object.freeze([7n, 18n, 9n]) }),
        Object.freeze({ address: custodyPosition, owner: custodyOwner, revision: 7n, balances: Object.freeze([1n, 4n, 3n]) }),
      ]),
      receipt: null,
      assets: Object.freeze([Object.freeze({
        mint, mintSupply: 1_020n, actorAccount: actorToken, actorAmount: 520n,
        structuredAccount: structured, structuredAmount: 100n,
      })]),
    }) satisfies RationalOpenPoststateV4;
    const accounts = new Map<string, RpcAccount | null>([
      [replay, replayAccount(claimsProgram, descriptor, actor, 4n)],
      [aggregate, aggregateAccount({ claims: claimsProgram, market, releaseSet, registry, product, basis, realm,
        custody: custodyContext, generation: 3n, revision: 5n, balances: [10n, 20n, 30n] })],
      [actorPosition, positionAccount({ claims: claimsProgram, aggregate, owner: actor, basis, revision: 6n,
        balances: [7n, 18n, 9n] })],
      [custodyPosition, positionAccount({ claims: claimsProgram, aggregate, owner: custodyOwner, basis, revision: 7n,
        balances: [1n, 4n, 3n] })],
      [mint, mintAccount(mint, authority, 1_020n)],
      [actorToken, tokenAccount(mint, actor, 520n)],
      [structured, tokenAccount(mint, authority, 100n)],
    ]);
    const client = Object.freeze({
      finalizedSlot: async () => '90',
      multipleAccounts: async (addresses: ReadonlyArray<string>, _minimumSlot?: string) => Object.freeze({
        slot: '91', accounts: Object.freeze(addresses.map((value) => Object.freeze({ address: value, account: accounts.get(value) ?? null }))),
      }),
    });
    const verified = await verifyRationalOpenFinalizedPoststateV4(client, 'denominate', poststate, '90');
    expect(verified.observedSlot).toBe('91');
    expect(verified.poststate.assets[0]?.actorAmount).toBe(520n);

    accounts.set(actorToken, tokenAccount(mint, actor, 519n));
    await expect(verifyRationalOpenFinalizedPoststateV4(client, 'denominate', poststate, '90'))
      .rejects.toThrow('differs from the exact finalized poststate');
  });

  it('compiles one exact blocked v0+ALT packet with only payer and actor wallet signers', async () => {
    const actor = address(220); const payer = address(221); const market = address(222);
    const rows = [asset(70, 10n)];
    const family = await compileRationalOpenHotV3({
      action: 'denominate', releaseSet: bytes(223), market, graphId: bytes(224), descriptorId: bytes(225), actor,
      receiptMint: address(226), receiptAccount: null, representationAuthority: address(228), tokenProgram: TOKEN_2022_PROGRAM_ID,
      expectedRepresentationRevision: 0n, expectedClaimsMarketRevision: 4n,
      expectedActorPositionRevision: 5n, expectedCustodyPositionRevision: 6n,
      generation: 3n, quantity: 2n, denominator: 10n, expectedReceiptSupply: 0n,
      outcomeCount: 3, selectedOutcome: 1, assets: rows,
    });
    const fixed = Array.from({ length: HOT_FIXED_ACCOUNT_COUNT_V3 }, (_, index) => Object.freeze({
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
      representationWidth: 3, resultOutcomeCount: 258, selectedOutcome: 1,
      rawQuantity: 2n, displayDecimals: 255,
      descriptorId: bytes(225), tokenBehaviorDigest: bytes(231), capabilityDigest: bytes(232), rootDigest: bytes(233),
      family, fixedAccounts: fixed, physicalClaimsAccounts: physical, lookupTable: table,
      poststate: Object.freeze({
        context: Object.freeze({
          claimsProgram: address(13), descriptorId: bytes(225), actor, representationAuthority: address(228),
          aggregate: address(11), market, releaseSet: bytes(223), registry: address(15), product: bytes(235),
          realm: bytes(236), generation: 3n, outcomes: 3, basis: bytes(237), custodyContext: bytes(238),
        }),
        replay: Object.freeze({ address: address(10), revision: 1n }),
        aggregate: Object.freeze({ address: address(11), revision: 5n, balances: Object.freeze([0n, 0n, 0n]) }),
        positions: Object.freeze([
          Object.freeze({ address: address(21), owner: actor, revision: 6n, balances: Object.freeze([0n, 0n, 0n]) }),
          Object.freeze({ address: address(100), owner: address(73), revision: 7n, balances: Object.freeze([0n, 0n, 0n]) }),
        ]),
        receipt: null,
        assets: Object.freeze([Object.freeze({ mint: address(70), mintSupply: 1_020n, actorAccount: address(71),
          actorAmount: 520n, structuredAccount: address(72), structuredAmount: 100n })]),
      }),
      executionStatus: 'blocked' as const, refusal: 'checked common Hot release pending',
    }) satisfies RationalOpenChainInspectionV4;
    const candidate = buildRationalOpenCandidateV4(inspection, address(234));
    // Denominate is a SELECTED action.
    expect(candidate.outerBytes).toHaveLength(HOT_EXECUTION_ENVELOPE_BYTES_V3 + Abi.REQUEST_SELECTED_HEADER_BYTES_V3 + Abi.ASSET_BYTES_V3);
    expect(candidate.logicalClaimsAccounts).toBe(36);
    expect(candidate.physicalClaimsAccounts).toBe(31);
    expect(candidate.loadedAddresses).toBeGreaterThan(0);
    expect(candidate.wireBytes.length).toBeLessThanOrEqual(1232);
    expect([...candidate.requiredSigners].sort()).toEqual([actor, payer].sort());
    expect(candidate.executionStatus).toBe('blocked');
    expect(rationalOpenChainSummaryV4(inspection).width)
      .toBe('K=3 claims over N=258 terminal results');
  });
});
