import { describe, expect, it } from 'vitest';

import fixture from '../fixtures/canonical-accounts.json';
import { type RpcAccount, type SolanaRpcClient } from './rpc';
import {
  enumerateCoreMarketAddressesV1,
  inspectMarketDiscoveryV1,
  parseMarketAddressListV1,
  provenanceChipV1,
  shortAddressV1,
  MARKET_DISCOVERY_MAX_ADDRESSES,
} from './marketDiscovery';

const SYSTEM_PROGRAM = '11111111111111111111111111111111';

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

function coreAccount(data: Uint8Array, owner = fixture.programId): RpcAccount {
  return Object.freeze({ data, executable: false, lamports: '1234567', owner, space: data.length });
}

function client(
  accounts: ReadonlyMap<string, RpcAccount>,
  options: Readonly<{ slot?: string; headers?: ReadonlyArray<Readonly<[string, Uint8Array]>> | Error }> = {},
): SolanaRpcClient {
  const slot = options.slot ?? '99';
  return {
    finalizedSlot: async () => slot,
    multipleAccounts: async (addresses: ReadonlyArray<string>) => Object.freeze({
      slot,
      accounts: Object.freeze(addresses.map((address) => Object.freeze({ address, account: accounts.get(address) ?? null }))),
    }),
    programHeaders: async () => {
      if (options.headers instanceof Error) throw options.headers;
      return Object.freeze({
        slot,
        accounts: Object.freeze((options.headers ?? []).map(([address, data]) => Object.freeze({ address, account: coreAccount(data) }))),
      });
    },
  } as unknown as SolanaRpcClient;
}

const chainAccounts = new Map<string, RpcAccount>([
  [market.address, coreAccount(market.data)],
  [realm.address, coreAccount(realm.data)],
]);

describe('Market discovery cards', () => {
  it('derives every card value from finalized Core state, including the Realm it never received', async () => {
    const discovery = await inspectMarketDiscoveryV1(client(chainAccounts), {
      coreProgramId: fixture.programId,
      addresses: [market.address],
    });
    expect(discovery.floorSlot).toBe('99');
    expect(discovery.enumeration.mode).toBe('address-list');
    expect(discovery.cards).toHaveLength(1);
    const card = discovery.cards[0];
    if (card.status !== 'decoded') throw new Error(card.refusal);
    expect(card.address).toBe(market.address);
    expect(card.phase).toBe('Founding');
    expect(card.generation).toBe('7');
    expect(card.outcomeCount).toBe(3);
    expect(card.hoardAtoms).toBe('0');
    expect(card.supplyAtoms).toEqual(['0', '0', '0']);
    expect(card.settlement.status).toBe('empty');
    expect(card.outstandingChildren).toBe('0');
    expect(provenanceChipV1(card.provenance)).toBe('CHAIN · finalized slot 99');
    if (card.collateral.status !== 'bound') throw new Error(card.collateral.reason);
    // The Realm address is never caller input: it is the content-addressed PDA
    // of the identity the Market itself commits to.
    expect(card.collateral.realmAddress).toBe(realm.address);
    expect(card.collateral.realmContentId).toBe('3bda98b500c0de22309e1023ba42cc6cd5904eb9e09acfd0e94d04672bb15ba5');
    expect(card.collateral.collateralMintShort).toBe(shortAddressV1(card.collateral.collateralMint));
    expect(card.bindings.some((check) => check.label === 'Market → Realm content' && check.ok)).toBe(true);
  });

  it('never asserts a capability the Market root alone cannot authenticate', async () => {
    const withoutRegistry = await inspectMarketDiscoveryV1(client(chainAccounts), {
      coreProgramId: fixture.programId,
      addresses: [market.address],
    });
    const unread = withoutRegistry.cards[0];
    if (unread.status !== 'decoded') throw new Error(unread.refusal);
    expect(unread.capabilities.status).toBe('unread');
    expect(unread.capabilities.manifestId).toBe('05'.repeat(32));
    if (unread.capabilities.status !== 'unread') throw new Error('expected an unread manifest');
    expect(unread.capabilities.reason).toMatch(/No capability may be asserted from the Market root alone/);

    const withRegistry = await inspectMarketDiscoveryV1(client(chainAccounts), {
      coreProgramId: fixture.programId,
      registryProgramId: SYSTEM_PROGRAM,
      addresses: [market.address],
    });
    const refused = withRegistry.cards[0];
    if (refused.status !== 'decoded') throw new Error(refused.refusal);
    expect(refused.capabilities.status).toBe('refused');
    if (refused.capabilities.status !== 'refused') throw new Error('expected a refused manifest');
    expect(refused.capabilities.reason).toMatch(/is absent at finalized slot 99/);
  });

  it('refuses an absent, foreign-owned, or undecodable Market with its exact reason', async () => {
    const absent = await inspectMarketDiscoveryV1(client(new Map()), {
      coreProgramId: fixture.programId,
      addresses: [market.address],
    });
    expect(absent.cards[0]).toMatchObject({ status: 'refused' });
    expect(absent.cards[0].refusal).toMatch(/absent at the finalized observation floor/);
    expect(provenanceChipV1(absent.cards[0].provenance)).toBe('REFUSED');

    const foreign = await inspectMarketDiscoveryV1(client(new Map([[market.address, coreAccount(market.data, SYSTEM_PROGRAM)]])), {
      coreProgramId: fixture.programId,
      addresses: [market.address],
    });
    expect(foreign.cards[0].refusal).toMatch(/owner differs from the selected program/);

    const truncated = market.data.slice(0, market.data.length - 1);
    const damaged = await inspectMarketDiscoveryV1(client(new Map([[market.address, coreAccount(truncated)]])), {
      coreProgramId: fixture.programId,
      addresses: [market.address],
    });
    expect(damaged.cards[0].refusal).toMatch(/expected exactly/);
  });

  it('reports an unbound collateral identity rather than inventing a mint', async () => {
    const discovery = await inspectMarketDiscoveryV1(client(new Map([[market.address, coreAccount(market.data)]])), {
      coreProgramId: fixture.programId,
      addresses: [market.address],
    });
    const card = discovery.cards[0];
    if (card.status !== 'decoded') throw new Error(card.refusal);
    expect(card.collateral.status).toBe('refused');
    if (card.collateral.status !== 'refused') throw new Error('expected an unbound Realm');
    expect(card.collateral.realmAddress).toBe(realm.address);
    expect(card.collateral.reason).toMatch(/collateral identity is unbound/);
  });

  it('answers an empty address set honestly instead of scanning for something to show', async () => {
    const discovery = await inspectMarketDiscoveryV1(client(chainAccounts), { coreProgramId: fixture.programId, addresses: [] });
    expect(discovery.cards).toEqual([]);
    expect(discovery.reason).toMatch(/No Market address has been supplied or enumerated/);
  });
});

describe('Market enumeration', () => {
  it('lists only Market-headered Core accounts from a bounded finalized scan', async () => {
    const enumeration = await enumerateCoreMarketAddressesV1(client(chainAccounts, {
      headers: [[market.address, market.data.slice(0, 16)], [realm.address, realm.data.slice(0, 16)]],
    }), fixture.programId);
    expect(enumeration.mode).toBe('program-scan');
    expect(enumeration.addresses).toEqual([market.address]);
    expect(enumeration.note).toMatch(/2 finalized Core accounts at slot 99; 1 carry the Market header/);
  });

  it('reports the indexer-shaped gap when getProgramAccounts is unavailable', async () => {
    const enumeration = await enumerateCoreMarketAddressesV1(
      client(chainAccounts, { headers: new Error('getProgramAccounts refused: RPC method is disabled') }),
      fixture.programId,
    );
    expect(enumeration.mode).toBe('refused');
    if (enumeration.mode !== 'refused') throw new Error('expected a refusal');
    expect(enumeration.reason).toMatch(/RPC method is disabled/);
    expect(enumeration.note).toMatch(/dClutch has no index and this browser will not invent one/);
    expect(enumeration.addresses).toEqual([]);
  });

  it('parses only canonical, distinct, bounded known-Market address lists', () => {
    expect(parseMarketAddressListV1(` ${market.address}\n${realm.address} `)).toEqual([market.address, realm.address]);
    expect(parseMarketAddressListV1('   ')).toEqual([]);
    expect(() => parseMarketAddressListV1(`${market.address} ${market.address}`)).toThrow(/repeats a Market address/);
    expect(() => parseMarketAddressListV1('not-base58')).toThrow(/canonical Solana address/);
    expect(() => parseMarketAddressListV1(Array.from({ length: MARKET_DISCOVERY_MAX_ADDRESSES + 1 }, () => market.address).join('\n')))
      .toThrow(new RegExp(`above the explicit ${MARKET_DISCOVERY_MAX_ADDRESSES}-Market browser bound`));
  });
});
