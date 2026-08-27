import { PublicKey } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import { LIVE, liveRpcAccount, mutate } from '../fixtures/liveOpenMarket';
import { sha256 } from './bytes';
import { REALM_SCHEMA_RELEASE_ID_V1 } from './generated/coreFound';
import { deriveClaimsAggregateAddressV2, deriveClaimsPositionAddressV2 } from './marketCoreV2';
import { provenanceChipV1 } from './marketDiscovery';
import {
  inspectPortfolioV1,
  parsePortfolioOwnerV1,
  portfolioClaimV1,
  PORTFOLIO_MAX_MARKETS,
} from './portfolio';
import { deriveFinalizedRecordAddressesV1 } from './releaseRegistry';
import { type RpcAccount, type SolanaRpcClient } from './rpc';

/**
 * The portfolio, checked against the founder who actually holds claims.
 *
 * The founder's balances are in a Claims-owned LiabilityBasisV2 Position, at
 * `[dclutch:lbv2:position, aggregate, owner]` under the Claims program. This
 * suite runs against the real account bytes at the real derived address, so a
 * regression to any other derivation domain reports "no Position" and fails
 * here — which is exactly how the defect this replaces reached a live chain.
 */

const SYSTEM_PROGRAM = '11111111111111111111111111111111';
const CORE = LIVE.programs.core;
const REGISTRY = LIVE.programs.registry;
const CLAIMS = LIVE.programs.claims;
const OWNER = LIVE.founder;
const SLOT = '5150';

function client(accounts: ReadonlyMap<string, RpcAccount>): SolanaRpcClient {
  return {
    finalizedSlot: async () => SLOT,
    multipleAccounts: async (addresses: ReadonlyArray<string>) => Object.freeze({
      slot: SLOT,
      accounts: Object.freeze(addresses.map((address) => Object.freeze({ address, account: accounts.get(address) ?? null }))),
    }),
  } as unknown as SolanaRpcClient;
}

async function chain(overrides: Readonly<{ market?: Uint8Array; position?: Uint8Array | null }> = {}): Promise<Map<string, RpcAccount>> {
  const accounts = new Map<string, RpcAccount>([
    [LIVE.market.address, liveRpcAccount(LIVE.market, { data: overrides.market ?? LIVE.market.data })],
    [LIVE.claimsAggregate.address, liveRpcAccount(LIVE.claimsAggregate)],
  ]);
  if (overrides.position !== null) {
    accounts.set(LIVE.founderPosition.address, liveRpcAccount(LIVE.founderPosition, { data: overrides.position ?? LIVE.founderPosition.data }));
  }
  const realm = deriveFinalizedRecordAddressesV1(REGISTRY, REALM_SCHEMA_RELEASE_ID_V1, await sha256(LIVE.realmRecord.data));
  accounts.set(realm.record, liveRpcAccount(LIVE.realmRecord));
  return accounts;
}

const request = {
  coreProgramId: CORE,
  claimsProgramId: CLAIMS,
  registryProgramId: REGISTRY,
  owner: OWNER,
  marketAddresses: [LIVE.market.address],
};

describe('portfolio by Claims Position derivation', () => {
  it('derives the addresses the chain actually used', () => {
    const aggregate = deriveClaimsAggregateAddressV2(CLAIMS, LIVE.market.address);
    expect(aggregate).toBe(LIVE.claimsAggregate.address);
    expect(deriveClaimsPositionAddressV2(CLAIMS, aggregate, OWNER)).toBe(LIVE.founderPosition.address);
  });

  it('reads the founder raw claim balances and the complete sets they can merge while Open', async () => {
    const portfolio = await inspectPortfolioV1(client(await chain()), request);
    expect(portfolio.owner).toBe(OWNER);
    expect(portfolio.entries).toHaveLength(1);
    const [entry] = portfolio.entries;
    expect(entry.aggregateAddress).toBe(LIVE.claimsAggregate.address);
    expect(entry.positionAddress).toBe(LIVE.founderPosition.address);
    if (entry.position.status !== 'held') throw new Error(`expected a held Position: ${JSON.stringify(entry.position)}`);
    expect(entry.position.balances).toEqual(['500000000', '500000000', '500000000', '500000000']);
    expect(entry.position.claimCount).toBe(4);
    expect(entry.position.aggregateAddress).toBe(LIVE.claimsAggregate.address);
    expect(provenanceChipV1(entry.position.provenance)).toBe(`CHAIN · finalized slot ${SLOT}`);
    if (entry.position.claim.kind !== 'mergeable') throw new Error('an Open Market admits complete-set merge');
    expect(entry.position.claim.completeSetsAtoms).toBe('500000000');
    expect(entry.position.claim.note).toMatch(/This is arithmetic on these balances, not an offer/);
    expect(portfolio.reason).toMatch(/1 of 1 derived Claims Position hold state/);
  });

  it('takes the smallest balance as the mergeable complete-set count', async () => {
    // 128-byte header, then one u64 per claim: shrink claim 2 only.
    const smaller = new Uint8Array(LIVE.founderPosition.data);
    new DataView(smaller.buffer).setBigUint64(128 + 2 * 8, 7n, true);
    const portfolio = await inspectPortfolioV1(client(await chain({ position: smaller })), request);
    const [entry] = portfolio.entries;
    if (entry.position.status !== 'held' || entry.position.claim.kind !== 'mergeable') throw new Error('expected a mergeable Position');
    expect(entry.position.balances).toEqual(['500000000', '500000000', '7', '500000000']);
    expect(entry.position.claim.completeSetsAtoms).toBe('7');
  });

  it('reports the exact redeemable payout after a terminal receipt, including the zeros', async () => {
    const terminal = mutate(mutate(mutate(LIVE.market.data, 10, 2), 12, 1), 320, new Uint8Array(32).fill(0x77));
    const portfolio = await inspectPortfolioV1(client(await chain({ market: terminal })), request);
    const [entry] = portfolio.entries;
    if (entry.market.status !== 'decoded') throw new Error(entry.market.refusal);
    expect(entry.market.phase).toBe('Terminal');
    if (entry.position.status !== 'held' || entry.position.claim.kind !== 'redeemable') throw new Error('a terminal Market admits redemption');
    expect(entry.position.claim.winningClaim).toBe(1);
    expect(entry.position.claim.redeemableAtoms).toBe('500000000');
    expect(entry.position.claim.perClaimAtoms).toEqual(['0', '500000000', '0', '0']);
    expect(entry.position.claim.note).toMatch(/every losing claim pays zero/);
  });

  it('calls a derived address with no account exactly what it is', async () => {
    const portfolio = await inspectPortfolioV1(client(await chain({ position: null })), request);
    const [entry] = portfolio.entries;
    if (entry.position.status !== 'absent') throw new Error('expected an absent Position');
    expect(entry.position.note).toMatch(/No Claims Position exists at/);
    expect(entry.position.note).toMatch(/never been admitted to this Market's liability basis/);
    expect(provenanceChipV1(entry.position.provenance)).toBe(`CHAIN · finalized slot ${SLOT}`);
    expect(portfolio.reason).toMatch(/1 derived address holds no Position at all/);
  });

  it('refuses rather than deriving a different family Position when no Claims program is selected', async () => {
    const portfolio = await inspectPortfolioV1(client(await chain()), { ...request, claimsProgramId: null });
    const [entry] = portfolio.entries;
    expect(entry.positionAddress).toBeNull();
    expect(entry.aggregateAddress).toBeNull();
    if (entry.position.status !== 'refused') throw new Error('expected a refusal');
    expect(entry.position.reason).toMatch(/will not derive a different family's Position address and report its emptiness as an answer/);
  });

  it('refuses a Position from another owner, aggregate, width, or basis', async () => {
    const foreignOwner = mutate(LIVE.founderPosition.data, 56, new Uint8Array(32).fill(3));
    const owned = await inspectPortfolioV1(client(await chain({ position: foreignOwner })), request);
    if (owned.entries[0].position.status !== 'refused') throw new Error('expected a refusal');
    expect(owned.entries[0].position.reason).toMatch(/names owner .*, not /);

    const foreignAggregate = mutate(LIVE.founderPosition.data, 24, new Uint8Array(32).fill(4));
    const joined = await inspectPortfolioV1(client(await chain({ position: foreignAggregate })), request);
    if (joined.entries[0].position.status !== 'refused') throw new Error('expected a refusal');
    expect(joined.entries[0].position.reason).toMatch(/names Claims aggregate/);

    const otherBasis = mutate(LIVE.founderPosition.data, 88, new Uint8Array(32).fill(5));
    const basis = await inspectPortfolioV1(client(await chain({ position: otherBasis })), request);
    if (basis.entries[0].position.status !== 'refused') throw new Error('expected a refusal');
    expect(basis.entries[0].position.reason).toMatch(/names a different liability basis/);

    const foreignProgram = new Map(await chain());
    foreignProgram.set(LIVE.founderPosition.address, liveRpcAccount(LIVE.founderPosition, { owner: SYSTEM_PROGRAM }));
    const owner = await inspectPortfolioV1(client(foreignProgram), request);
    if (owner.entries[0].position.status !== 'refused') throw new Error('expected a refusal');
    expect(owner.entries[0].position.reason).toMatch(/the selected Claims program does not own/);
  });

  it('never claims what a Market that did not decode admits', async () => {
    const withoutMarket = new Map(await chain());
    withoutMarket.delete(LIVE.market.address);
    const portfolio = await inspectPortfolioV1(client(withoutMarket), request);
    const [entry] = portfolio.entries;
    expect(entry.market.status).toBe('refused');
    if (entry.position.status !== 'held') throw new Error('the Position itself still decodes');
    expect(entry.position.balances).toEqual(['500000000', '500000000', '500000000', '500000000']);
    expect(entry.position.claim.kind).toBe('unavailable');
    expect(entry.position.claim.note).toMatch(/nothing may be claimed about what these balances admit/);
  });

  it('parses one canonical owner identity from a wallet or from pasted text', () => {
    expect(parsePortfolioOwnerV1(` ${OWNER} `)).toBe(OWNER);
    expect(() => parsePortfolioOwnerV1('   ')).toThrow(/an owner address is required/);
    expect(() => parsePortfolioOwnerV1('not-base58')).toThrow(/canonical Solana address/);
  });

  it('answers an empty Market list honestly and refuses more than the browser bound', async () => {
    const empty = await inspectPortfolioV1(client(await chain()), { ...request, marketAddresses: [] });
    expect(empty.entries).toEqual([]);
    expect(empty.reason).toMatch(/without a Market address there is nothing to derive/);
    await expect(inspectPortfolioV1(client(await chain()), {
      ...request,
      marketAddresses: Array.from({ length: PORTFOLIO_MAX_MARKETS + 1 }, (_, index) => new PublicKey(new Uint8Array(32).fill(index + 1)).toBase58()),
    })).rejects.toThrow(/above the explicit/);
  });

  it('decides what balances admit from the Market phase, not from convenience', () => {
    const refused = Object.freeze({
      status: 'refused' as const,
      address: LIVE.market.address,
      provenance: Object.freeze({ kind: 'refused' as const, reason: 'x' }),
      observedSlot: SLOT,
      refusal: 'x',
    });
    expect(portfolioClaimV1(refused, ['1']).kind).toBe('unavailable');
  });
});
