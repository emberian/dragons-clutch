import { PublicKey } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import fixture from '../fixtures/canonical-accounts.json';
import { derivePositionAddressV1 } from './decoders';
import {
  inspectPortfolioV1,
  parsePortfolioOwnerV1,
  portfolioClaimV1,
  PORTFOLIO_MAX_MARKETS,
} from './portfolio';
import { provenanceChipV1, type MarketDiscoveryCardV1 } from './marketDiscovery';
import { type RpcAccount, type SolanaRpcClient } from './rpc';

function bytes(value: string): Uint8Array {
  const pairs = value.match(/../g);
  if (pairs === null || pairs.join('') !== value) throw new Error('fixture contains malformed hexadecimal bytes');
  return Uint8Array.from(pairs, (pair) => Number.parseInt(pair, 16));
}

function fixtureAccount(kind: string): Readonly<{ address: string; data: Uint8Array }> {
  const account = fixture.accounts.find((entry) => entry.kind === kind);
  if (account === undefined) throw new Error(`fixture omitted ${kind}`);
  return Object.freeze({ address: account.address, data: bytes(account.dataHex) });
}

const market = fixtureAccount('Market');
const realm = fixtureAccount('Realm');
const position = fixtureAccount('Position');
const OUTCOMES = market.data[10];
const SETTLEMENT_OFFSET = 256 + OUTCOMES * 8;

/** The exact owner the canonical Rust Position fixture already names. */
const OWNER = '2VDW9dFE1ZXz4zWAbaBDQFynNVdRpQ73HyfSHMzBSL6Z';

function marketVariant(mutate: (data: Uint8Array, view: DataView) => void): Uint8Array {
  const data = new Uint8Array(market.data);
  mutate(data, new DataView(data.buffer));
  return data;
}

function openMarket(supply: ReadonlyArray<bigint>, hoard: bigint): Uint8Array {
  return marketVariant((data, view) => {
    data[200] = 1;
    view.setBigUint64(248, hoard, true);
    supply.forEach((amount, index) => view.setBigUint64(256 + index * 8, amount, true));
  });
}

function resolvedMarket(winner: number, supply: ReadonlyArray<bigint>, hoard: bigint): Uint8Array {
  return marketVariant((data, view) => {
    data[200] = 2;
    view.setBigUint64(248, hoard, true);
    supply.forEach((amount, index) => view.setBigUint64(256 + index * 8, amount, true));
    data[SETTLEMENT_OFFSET] = 1;
    data[SETTLEMENT_OFFSET + 2] = winner;
    view.setBigUint64(SETTLEMENT_OFFSET + 8, BigInt(12), true);
    data.fill(0x55, SETTLEMENT_OFFSET + 16, SETTLEMENT_OFFSET + 48);
  });
}

/** Rewrite only balances or generation; the Position PDA depends on neither. */
function positionVariant(mutate: (data: Uint8Array, view: DataView) => void): Uint8Array {
  const data = new Uint8Array(position.data);
  mutate(data, new DataView(data.buffer));
  return data;
}

function withBalances(balances: ReadonlyArray<bigint>): Uint8Array {
  return positionVariant((_, view) => balances.forEach((amount, index) => view.setBigUint64(88 + index * 8, amount, true)));
}

function coreAccount(data: Uint8Array, owner = fixture.programId): RpcAccount {
  return Object.freeze({ data, executable: false, lamports: '9001', owner, space: data.length });
}

function client(accounts: ReadonlyMap<string, RpcAccount>, slot = '820'): SolanaRpcClient {
  return {
    finalizedSlot: async () => slot,
    multipleAccounts: async (addresses: ReadonlyArray<string>) => Object.freeze({
      slot,
      accounts: Object.freeze(addresses.map((address) => Object.freeze({ address, account: accounts.get(address) ?? null }))),
    }),
  } as unknown as SolanaRpcClient;
}

function chain(marketData: Uint8Array, positionData: Uint8Array | null): Map<string, RpcAccount> {
  const accounts = new Map<string, RpcAccount>([
    [market.address, coreAccount(marketData)],
    [realm.address, coreAccount(realm.data)],
  ]);
  if (positionData !== null) accounts.set(position.address, coreAccount(positionData));
  return accounts;
}

function request(overrides: Partial<Readonly<{ owner: string; marketAddresses: ReadonlyArray<string> }>> = {}) {
  return {
    coreProgramId: fixture.programId,
    owner: overrides.owner ?? OWNER,
    marketAddresses: overrides.marketAddresses ?? [market.address],
  };
}

describe('portfolio by direct Position derivation', () => {
  it('derives the same Position address the canonical Rust fixture already occupies', () => {
    // No index is consulted and none exists: the Market and the owner are the
    // whole derivation, and it lands exactly on the emitted fixture account.
    expect(derivePositionAddressV1(fixture.programId, market.address, OWNER)).toBe(position.address);
  });

  it('reads raw outcome balances and the complete sets they can merge while Open', async () => {
    const portfolio = await inspectPortfolioV1(
      client(chain(openMarket([BigInt(60), BigInt(60), BigInt(60)], BigInt(60)), withBalances([BigInt(9), BigInt(4), BigInt(17)]))),
      request(),
    );
    expect(portfolio.owner).toBe(OWNER);
    expect(portfolio.floorSlot).toBe('820');
    expect(portfolio.entries).toHaveLength(1);
    const [entry] = portfolio.entries;
    expect(entry.positionAddress).toBe(position.address);
    if (entry.position.status !== 'held') throw new Error(`expected a held Position, saw ${entry.position.status}`);
    expect(entry.position.balances).toEqual(['9', '4', '17']);
    expect(entry.position.generation).toBe('7');
    expect(provenanceChipV1(entry.position.provenance)).toBe('CHAIN · finalized slot 820');
    expect(entry.position.bindings.some((check) => check.label === 'Position PDA' && check.ok)).toBe(true);
    if (entry.position.claim.kind !== 'mergeable') throw new Error('expected a mergeable claim');
    expect(entry.position.claim.completeSetsAtoms).toBe('4');
    expect(entry.position.claim.note).toMatch(/smallest owned outcome balance/);
  });

  it('reports the exact redeemable payout after resolution, including the zeros', async () => {
    const winning = await inspectPortfolioV1(
      client(chain(resolvedMarket(2, [BigInt(60), BigInt(60), BigInt(60)], BigInt(60)), withBalances([BigInt(9), BigInt(4), BigInt(17)]))),
      request(),
    );
    const held = winning.entries[0].position;
    if (held.status !== 'held' || held.claim.kind !== 'redeemable') throw new Error('expected a redeemable Position');
    expect(held.claim.winningOutcome).toBe(2);
    // One collateral atom per winning claim atom; every losing atom pays zero.
    expect(held.claim.redeemableAtoms).toBe('17');
    expect(held.claim.perOutcomeAtoms).toEqual(['0', '0', '17']);

    const losing = await inspectPortfolioV1(
      client(chain(resolvedMarket(0, [BigInt(60), BigInt(60), BigInt(60)], BigInt(60)), withBalances([BigInt(0), BigInt(4), BigInt(17)]))),
      request(),
    );
    const loser = losing.entries[0].position;
    if (loser.status !== 'held' || loser.claim.kind !== 'redeemable') throw new Error('expected a redeemable Position');
    // A zero payout is stated, never hidden behind an empty section.
    expect(loser.claim.redeemableAtoms).toBe('0');
    expect(loser.claim.perOutcomeAtoms).toEqual(['0', '0', '0']);
  });

  it('calls a derived address with no account exactly what it is', async () => {
    const portfolio = await inspectPortfolioV1(client(chain(openMarket([BigInt(1), BigInt(1), BigInt(1)], BigInt(1)), null)), request());
    const [entry] = portfolio.entries;
    expect(entry.positionAddress).toBe(position.address);
    if (entry.position.status !== 'absent') throw new Error(`expected an absent Position, saw ${entry.position.status}`);
    expect(entry.position.note).toMatch(/No Position exists at the derived address/);
    // Absence is a finalized observation, not a refusal.
    expect(provenanceChipV1(entry.position.provenance)).toBe('CHAIN · finalized slot 820');
    expect(portfolio.reason).toMatch(/1 derived address holds no Position at all/);
  });

  it('refuses a Position from another generation, width, owner, or Market', async () => {
    const stale = await inspectPortfolioV1(
      client(chain(openMarket([BigInt(9), BigInt(9), BigInt(9)], BigInt(9)), positionVariant((_, view) => view.setBigUint64(80, BigInt(6), true)))),
      request(),
    );
    const staleEntry = stale.entries[0].position;
    if (staleEntry.status !== 'refused') throw new Error('expected a refusal');
    expect(staleEntry.reason).toMatch(/names generation 6 while the Market is at generation 7/);
    expect(provenanceChipV1(staleEntry.provenance)).toBe('REFUSED');

    // The fixture Position names its own owner. Asking for a different owner
    // derives a different address; planting the fixture bytes there must be
    // refused rather than shown as that owner's balances.
    const otherOwner = realm.address;
    const plantedAddress = derivePositionAddressV1(fixture.programId, market.address, otherOwner);
    const planted = chain(openMarket([BigInt(9), BigInt(9), BigInt(9)], BigInt(9)), null);
    planted.set(plantedAddress, coreAccount(position.data));
    const impostor = await inspectPortfolioV1(client(planted), request({ owner: otherOwner }));
    const impostorEntry = impostor.entries[0].position;
    if (impostorEntry.status !== 'refused') throw new Error('expected a refusal');
    expect(impostorEntry.reason).toMatch(new RegExp(`names owner ${OWNER}, not ${otherOwner}`));
  });

  it('never claims what a Market that did not decode admits', async () => {
    const portfolio = await inspectPortfolioV1(client(new Map([[position.address, coreAccount(position.data)]])), request());
    const [entry] = portfolio.entries;
    expect(entry.market.status).toBe('refused');
    if (entry.position.status !== 'held') throw new Error('expected the Position itself to still decode');
    expect(entry.position.balances).toEqual(['5', '0', '12']);
    expect(entry.position.claim.kind).toBe('unavailable');
    expect(entry.position.claim.note).toMatch(/The Market did not decode/);
  });

  it('states the phases that admit neither merge nor redemption', () => {
    const founding = { status: 'decoded', phase: 'Founding', settlement: { status: 'empty' } } as unknown as MarketDiscoveryCardV1;
    expect(portfolioClaimV1(founding, ['0', '0'])).toMatchObject({ kind: 'unavailable' });
    const retiring = { status: 'decoded', phase: 'Retiring', settlement: { status: 'empty' } } as unknown as MarketDiscoveryCardV1;
    const claim = portfolioClaimV1(retiring, ['0', '0']);
    if (claim.kind !== 'unavailable') throw new Error('expected no available transition');
    expect(claim.note).toMatch(/Phase Retiring admits neither complete-set merge nor redemption/);
  });

  it('refuses noncanonical input and an unbounded Market list before any read', async () => {
    expect(() => parsePortfolioOwnerV1('   ')).toThrow(/an owner address is required/);
    expect(() => parsePortfolioOwnerV1('not-an-address')).toThrow(/not one canonical Solana address/);
    expect(parsePortfolioOwnerV1(`  ${OWNER} `)).toBe(OWNER);
    await expect(inspectPortfolioV1(client(new Map()), request({ owner: 'nope' }))).rejects.toThrow(/owner address is not one canonical Solana address/);
    const overBound = Array.from({ length: PORTFOLIO_MAX_MARKETS + 1 }, (_, index) => new PublicKey(new Uint8Array(32).fill(index + 1)).toBase58());
    await expect(inspectPortfolioV1(client(new Map()), request({ marketAddresses: overBound })))
      .rejects.toThrow(new RegExp(`above the explicit ${PORTFOLIO_MAX_MARKETS}-Market browser bound`));
  });

  it('says plainly that with no Market named there is nothing to derive', async () => {
    const portfolio = await inspectPortfolioV1(client(new Map()), request({ marketAddresses: [] }));
    expect(portfolio.entries).toHaveLength(0);
    expect(portfolio.reason).toMatch(/without a Market address there is nothing to derive/);
  });
});
