import { PublicKey } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import { currentCoreMarketV3, LIVE, liveRpcAccount, mutate } from '../fixtures/liveOpenMarket';
import { sha256 } from './bytes';
import {
  CORE_STATE_PHASE_OFFSET,
  CORE_STATE_TERMINAL_RECEIPT_OFFSET,
  CORE_STATE_TERMINAL_WINNER_OFFSET,
  REALM_SCHEMA_RELEASE_ID_V1,
} from './generated/coreFound';
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
    [LIVE.market.address, liveRpcAccount(LIVE.market, { data: overrides.market ?? currentCoreMarketV3() })],
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
    const terminal = mutate(
      mutate(mutate(currentCoreMarketV3(), CORE_STATE_PHASE_OFFSET, 2), CORE_STATE_TERMINAL_WINNER_OFFSET, 1),
      CORE_STATE_TERMINAL_RECEIPT_OFFSET,
      new Uint8Array(32).fill(0x77),
    );
    const portfolio = await inspectPortfolioV1(client(await chain({ market: terminal })), request);
    const [entry] = portfolio.entries;
    if (entry.market.status !== 'decoded') throw new Error(entry.market.refusal);
    expect(entry.market.phase).toBe('Terminal');
    if (entry.position.status !== 'held' || entry.position.claim.kind !== 'redeemable') throw new Error('a terminal Market admits redemption');
    expect(entry.position.claim.winningClaim).toBe(1);
    expect(entry.position.claim.redeemableAtoms).toBe('500000000');
    expect(entry.position.claim.perClaimAtoms).toEqual(['0', '500000000', '0', '0']);
    expect(entry.position.claim.note).toMatch(/Every losing claim pays zero/);
    // The 1:1 payout claim is scoped to the categorical Q=1 basis; a graded
    // basis pays its own exact rate and the note must not overstate it.
    expect(entry.position.claim.note).toMatch(/categorical Q=1 basis/);
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

  /**
   * The merge count is arithmetic and stays arithmetic. On a Market whose
   * trading can never be switched on there is no route that performs it, so the
   * note stops short of reading like an offer that is merely unexercised.
   */
  it('says the merge cannot be performed on a Market whose trading can never be switched on', async () => {
    const portfolio = await inspectPortfolioV1(client(await chain()), request);
    const [entry] = portfolio.entries;
    if (entry.position.status !== 'held') throw new Error('expected a held Position');
    const market = entry.market;
    if (market.status !== 'decoded') throw new Error('expected a decoded Market');

    const shut = Object.freeze({
      ...market,
      outstandingCapabilities: '0',
      capabilities: Object.freeze({
        status: 'authenticated' as const,
        manifestId: market.identity.capabilityManifestId,
        recordAddress: LIVE.market.address,
        observedSlot: market.observedSlot,
        badges: Object.freeze([Object.freeze({
          index: 0,
          kindId: 'ab'.repeat(32),
          label: 'Direct successor',
          recognized: true,
          programSetId: 'cd'.repeat(32),
          configId: 'ef'.repeat(32),
          activation: 'deadline' as const,
          deadline: (BigInt(market.observedSlot) - BigInt(1)).toString(),
          dependencies: Object.freeze([]),
          funding: Object.freeze({
            compartments: Object.freeze([]),
            nativeLamportsTotal: BigInt(0),
            realmCollateralTotal: BigInt(0),
            realmCollateral: null,
          }),
        })]),
      }),
    });

    const claim = portfolioClaimV1(shut, entry.position.balances);
    if (claim.kind !== 'mergeable') throw new Error('the count is still arithmetic and is still reported');
    expect(claim.completeSetsAtoms).toBe('500000000');
    expect(claim.note).toMatch(/On this Market it is arithmetic only/);
    expect(claim.note).toMatch(/its trading can never be switched on/);
    // On a Market that can still trade, the extra clause is absent entirely.
    expect(portfolioClaimV1(market, entry.position.balances).note).not.toMatch(/arithmetic only/);
  });

  /**
   * The scale-1 assumption, and the two states it collapsed into one.
   *
   * "Merging burns one complete set and withdraws exactly one collateral atom"
   * is true of every categorical basis and of every fixture in this tree, and
   * false of the first graded market. The SET COUNT is scale-free and is stated
   * either way; the COLLATERAL is `basis_scale` atoms and is stated only when a
   * caller has authenticated `ProductBasisV3::payout_scale`.
   */
  it('states a collateral quantity only when the basis scale was authenticated', async () => {
    const portfolio = await inspectPortfolioV1(client(await chain()), request);
    const [entry] = portfolio.entries;
    if (entry.position.status !== 'held') throw new Error('expected a held Position');
    const market = entry.market;
    if (market.status !== 'decoded') throw new Error('expected a decoded Market');

    const unknown = portfolioClaimV1(market, entry.position.balances);
    if (unknown.kind !== 'mergeable') throw new Error('an Open Market admits the complete-set merge');
    expect(unknown.completeSetsAtoms).toBe('500000000');
    expect(unknown.collateralPerSetAtoms).toBeNull();
    expect(unknown.mergeableCollateralAtoms).toBeNull();
    expect(unknown.note).toMatch(/has not read that record/);
    expect(unknown.note).not.toMatch(/exactly one collateral atom/);

    const known = portfolioClaimV1(market, entry.position.balances, 1_000_000n);
    if (known.kind !== 'mergeable') throw new Error('the scale changes nothing about which act is admitted');
    // Scale-free: the same sets, whatever a set is worth.
    expect(known.completeSetsAtoms).toBe(unknown.completeSetsAtoms);
    expect(known.collateralPerSetAtoms).toBe('1000000');
    expect(known.mergeableCollateralAtoms).toBe('500000000000000');
    expect(known.note).toMatch(/1000000 collateral atoms/);
    expect(known.note).not.toMatch(/has not read that record/);

    // The default is not 1. A default of 1 is what made the defect invisible:
    // at scale 1 the two branches print the same sentence.
    const unit = portfolioClaimV1(market, entry.position.balances, 1n);
    if (unit.kind !== 'mergeable') throw new Error('the scale changes nothing about which act is admitted');
    expect(unit.collateralPerSetAtoms).toBe('1');
  });
});
