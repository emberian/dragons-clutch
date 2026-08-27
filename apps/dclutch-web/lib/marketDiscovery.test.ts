import { describe, expect, it } from 'vitest';

import { LIVE, liveRpcAccount, mutate } from '../fixtures/liveOpenMarket';
import { sha256 } from './bytes';
import {
  enumerateCoreMarketAddressesV1,
  inspectMarketDiscoveryV1,
  isCoreMarketHeaderV2,
  parseMarketAddressListV1,
  provenanceChipV1,
  shortAddressV1,
  MARKET_DISCOVERY_MAX_ADDRESSES,
} from './marketDiscovery';
import { deriveFinalizedRecordAddressesV1 } from './releaseRegistry';
import { CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1, REALM_SCHEMA_RELEASE_ID_V1 } from './generated/coreFound';
import { type RpcAccount, type SolanaRpcClient } from './rpc';

/**
 * Discovery, checked against the chain the campaign actually produced.
 *
 * Every account below is finalized bytes read off a live successor validator.
 * The Market is `DCLTCOR2`; its liabilities are in a Claims-owned aggregate;
 * its Realm is a finalized Registry record. Adversarial cases mutate one field
 * of the real bytes, so the only thing wrong with the input is the thing the
 * case is about.
 */

const SYSTEM_PROGRAM = '11111111111111111111111111111111';
const CORE = LIVE.programs.core;
const REGISTRY = LIVE.programs.registry;
const CLAIMS = LIVE.programs.claims;
const SLOT = '99';

function client(
  accounts: ReadonlyMap<string, RpcAccount>,
  options: Readonly<{ headers?: ReadonlyArray<Readonly<[string, Uint8Array]>> | Error }> = {},
): SolanaRpcClient {
  return {
    finalizedSlot: async () => SLOT,
    multipleAccounts: async (addresses: ReadonlyArray<string>) => Object.freeze({
      slot: SLOT,
      accounts: Object.freeze(addresses.map((address) => Object.freeze({ address, account: accounts.get(address) ?? null }))),
    }),
    programHeaders: async () => {
      if (options.headers instanceof Error) throw options.headers;
      return Object.freeze({
        slot: SLOT,
        accounts: Object.freeze((options.headers ?? []).map(([address, data]) => Object.freeze({
          address,
          account: Object.freeze({ data, executable: false, lamports: '1', owner: CORE, space: data.length }),
        }))),
      });
    },
  } as unknown as SolanaRpcClient;
}

async function recordAddresses(schema: Uint8Array, body: Uint8Array): Promise<Readonly<{ record: string; staging: string }>> {
  return deriveFinalizedRecordAddressesV1(REGISTRY, schema, await sha256(body));
}

/** The whole finalized graph the campaign left behind. */
async function liveChain(): Promise<Map<string, RpcAccount>> {
  const accounts = new Map<string, RpcAccount>([
    [LIVE.market.address, liveRpcAccount(LIVE.market)],
    [LIVE.claimsAggregate.address, liveRpcAccount(LIVE.claimsAggregate)],
  ]);
  const realm = await recordAddresses(REALM_SCHEMA_RELEASE_ID_V1, LIVE.realmRecord.data);
  accounts.set(realm.record, liveRpcAccount(LIVE.realmRecord));
  return accounts;
}

describe('Market discovery cards', () => {
  it('decodes the live Open Market, its Claims liabilities and its Realm record', async () => {
    const discovery = await inspectMarketDiscoveryV1(client(await liveChain()), {
      coreProgramId: CORE,
      registryProgramId: REGISTRY,
      claimsProgramId: CLAIMS,
      addresses: [LIVE.market.address],
    });
    expect(discovery.floorSlot).toBe(SLOT);
    expect(discovery.cards).toHaveLength(1);
    const card = discovery.cards[0];
    if (card.status !== 'decoded') throw new Error(card.refusal);
    expect(card.phase).toBe('Open');
    expect(card.readiness).toBe('Consumed');
    expect(card.generation).toBe('2');
    expect(card.outstandingCapabilities).toBe('0');
    expect(card.settlement).toEqual({ status: 'open', label: 'no terminal receipt' });
    expect(card.identity.schemaMagic).toBe('DCLTCOR2');
    expect(card.identity.accountBytes).toBe(352);
    expect(card.identity.registryProgram).toBe(REGISTRY);
    expect(provenanceChipV1(card.provenance)).toBe(`CHAIN · finalized slot ${SLOT}`);

    if (card.liability.status !== 'bound') throw new Error(card.liability.reason);
    expect(card.liability.claimCount).toBe(4);
    expect(card.liability.supplyAtoms).toEqual(['500000000', '500000000', '500000000', '500000000']);
    expect(card.liability.requiredBackingAtoms).toBe('500000000');
    expect(card.liability.requiredBackingBasis).toBe('maximum-claim-supply');
    expect(card.liability.aggregateAddress).toBe(LIVE.claimsAggregate.address);

    if (card.collateral.status !== 'bound') throw new Error(card.collateral.reason);
    expect(card.collateral.realmContentId).toBe(card.identity.realmId);
    expect(card.collateral.collateralMintShort).toBe(shortAddressV1(card.collateral.collateralMint));
    expect(card.collateral.tokenProgram).toBe('TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb');

    // The account has to derive the address it was found at, from the eight
    // identities and the generation it declares.
    expect(card.bindings.find((check) => check.label === 'Market PDA')?.ok).toBe(true);
    expect(card.bindings.find((check) => check.label === 'Market self-identity')?.ok).toBe(true);
    expect(card.bindings.find((check) => check.label === 'Registry authority')?.ok).toBe(true);
  });

  it('never presents a Hoard figure it cannot name the account for', async () => {
    const discovery = await inspectMarketDiscoveryV1(client(await liveChain()), {
      coreProgramId: CORE, registryProgramId: REGISTRY, claimsProgramId: CLAIMS, addresses: [LIVE.market.address],
    });
    const card = discovery.cards[0];
    if (card.status !== 'decoded') throw new Error(card.refusal);
    expect(card.hoard.status).toBe('not-derivable');
    expect(card.hoard.reason).toMatch(/namespaced by the founding action context/);
    expect(JSON.stringify(card)).not.toContain('hoardAtoms');
  });

  it('says liabilities are UNREAD rather than showing an empty supply vector', async () => {
    const discovery = await inspectMarketDiscoveryV1(client(await liveChain()), {
      coreProgramId: CORE, registryProgramId: REGISTRY, addresses: [LIVE.market.address],
    });
    const card = discovery.cards[0];
    if (card.status !== 'decoded') throw new Error(card.refusal);
    expect(card.liability.status).toBe('unread');
    if (card.liability.status !== 'unread') throw new Error('expected unread liabilities');
    expect(card.liability.reason).toMatch(/this is an unread section, not a Market with no claims/);
  });

  it('refuses a Claims aggregate that names another Market or another generation', async () => {
    const foreign = new Map(await liveChain());
    // LiabilityBasisV2 aggregate: logical Market at offset 24.
    foreign.set(LIVE.claimsAggregate.address, liveRpcAccount(LIVE.claimsAggregate, {
      data: mutate(LIVE.claimsAggregate.data, 24, new Uint8Array(32).fill(9)),
    }));
    const discovery = await inspectMarketDiscoveryV1(client(foreign), {
      coreProgramId: CORE, registryProgramId: REGISTRY, claimsProgramId: CLAIMS, addresses: [LIVE.market.address],
    });
    const card = discovery.cards[0];
    if (card.status !== 'decoded') throw new Error(card.refusal);
    expect(card.liability.status).toBe('refused');
    if (card.liability.status !== 'refused') throw new Error('expected refused liabilities');
    expect(card.liability.reason).toMatch(/the aggregate names Market/);

    const stale = new Map(await liveChain());
    // Aggregate generation at offset 248.
    const generation = new Uint8Array(8);
    new DataView(generation.buffer).setBigUint64(0, 99n, true);
    stale.set(LIVE.claimsAggregate.address, liveRpcAccount(LIVE.claimsAggregate, {
      data: mutate(LIVE.claimsAggregate.data, 248, generation),
    }));
    const second = await inspectMarketDiscoveryV1(client(stale), {
      coreProgramId: CORE, registryProgramId: REGISTRY, claimsProgramId: CLAIMS, addresses: [LIVE.market.address],
    });
    const staleCard = second.cards[0];
    if (staleCard.status !== 'decoded') throw new Error(staleCard.refusal);
    expect(staleCard.liability.status).toBe('refused');
    if (staleCard.liability.status !== 'refused') throw new Error('expected refused liabilities');
    expect(staleCard.liability.reason).toMatch(/two incarnations and are not shown as one/);
  });

  it('never asserts a capability the Market root alone cannot authenticate', async () => {
    const unreadRead = await inspectMarketDiscoveryV1(client(await liveChain()), {
      coreProgramId: CORE, claimsProgramId: CLAIMS, addresses: [LIVE.market.address],
    });
    const unread = unreadRead.cards[0];
    if (unread.status !== 'decoded') throw new Error(unread.refusal);
    expect(unread.capabilities.status).toBe('unread');
    if (unread.capabilities.status !== 'unread') throw new Error('expected an unread manifest');
    expect(unread.capabilities.reason).toMatch(/No capability may be asserted from the Market root alone/);
    expect(unread.collateral.status).toBe('unread');

    // A Registry that holds no manifest record refuses; it does not return an
    // empty badge list.
    const refusedRead = await inspectMarketDiscoveryV1(client(await liveChain()), {
      coreProgramId: CORE, registryProgramId: REGISTRY, claimsProgramId: CLAIMS, addresses: [LIVE.market.address],
    });
    const refused = refusedRead.cards[0];
    if (refused.status !== 'decoded') throw new Error(refused.refusal);
    expect(refused.capabilities.status).toBe('refused');
    if (refused.capabilities.status !== 'refused') throw new Error('expected a refused manifest');
    expect(refused.capabilities.reason).toMatch(/is absent at finalized slot 99/);
    expect(refused.capabilities.manifestId).toBe(refused.identity.capabilityManifestId);
  });

  it('refuses an absent, foreign-owned, or damaged Market with its exact reason', async () => {
    const absent = await inspectMarketDiscoveryV1(client(new Map()), { coreProgramId: CORE, addresses: [LIVE.market.address] });
    expect(absent.cards[0]).toMatchObject({ status: 'refused' });
    expect(absent.cards[0].refusal).toMatch(/absent at the finalized observation floor/);
    expect(provenanceChipV1(absent.cards[0].provenance)).toBe('REFUSED');

    const foreign = await inspectMarketDiscoveryV1(client(new Map([[LIVE.market.address, liveRpcAccount(LIVE.market, { owner: SYSTEM_PROGRAM })]])), {
      coreProgramId: CORE, addresses: [LIVE.market.address],
    });
    expect(foreign.cards[0].refusal).toMatch(/owner differs from the selected Core program/);

    const truncated = LIVE.market.data.slice(0, LIVE.market.data.length - 1);
    const damaged = await inspectMarketDiscoveryV1(client(new Map([[LIVE.market.address, liveRpcAccount(LIVE.market, { data: truncated })]])), {
      coreProgramId: CORE, addresses: [LIVE.market.address],
    });
    expect(damaged.cards[0].refusal).toMatch(/the exact width is 352/);
  });

  it('refuses a Realm record whose bytes do not hash to the committed identity', async () => {
    const accounts = new Map(await liveChain());
    const realm = await recordAddresses(REALM_SCHEMA_RELEASE_ID_V1, LIVE.realmRecord.data);
    // The record is still 112 bytes and still decodes; only its content moved.
    accounts.set(realm.record, liveRpcAccount(LIVE.realmRecord, { data: mutate(LIVE.realmRecord.data, 48, new Uint8Array(32).fill(7)) }));
    const discovery = await inspectMarketDiscoveryV1(client(accounts), {
      coreProgramId: CORE, registryProgramId: REGISTRY, claimsProgramId: CLAIMS, addresses: [LIVE.market.address],
    });
    const card = discovery.cards[0];
    if (card.status !== 'decoded') throw new Error(card.refusal);
    expect(card.collateral.status).toBe('refused');
    if (card.collateral.status !== 'refused') throw new Error('expected a refused Realm');
    expect(card.collateral.reason).toMatch(/differ from the identity the Market committed to/);
  });

  it('answers an empty address set honestly instead of scanning for something to show', async () => {
    const discovery = await inspectMarketDiscoveryV1(client(await liveChain()), { coreProgramId: CORE, addresses: [] });
    expect(discovery.cards).toEqual([]);
    expect(discovery.reason).toMatch(/No Market address has been supplied or enumerated/);
  });
});

describe('Market enumeration', () => {
  it('recognizes the live Core Market header and nothing else', () => {
    expect(isCoreMarketHeaderV2(LIVE.market.data)).toBe(true);
    expect(isCoreMarketHeaderV2(LIVE.claimsAggregate.data)).toBe(false);
    expect(isCoreMarketHeaderV2(LIVE.realmRecord.data)).toBe(false);
    // The superseded categorical Market layout is not a live Market header.
    const categorical = new Uint8Array(352);
    categorical.set(new TextEncoder().encode('DCLTCAT1'));
    expect(isCoreMarketHeaderV2(categorical)).toBe(false);
  });

  it('lists only Market-headered Core accounts from a bounded finalized scan', async () => {
    const enumeration = await enumerateCoreMarketAddressesV1(client(new Map(), {
      headers: [
        [LIVE.market.address, LIVE.market.data.slice(0, 16)],
        [LIVE.claimsAggregate.address, LIVE.claimsAggregate.data.slice(0, 16)],
      ],
    }), CORE);
    expect(enumeration.mode).toBe('program-scan');
    expect(enumeration.addresses).toEqual([LIVE.market.address]);
    expect(enumeration.note).toMatch(/2 finalized Core accounts at slot 99; 1 carry the DCLTCOR2 Market header/);
  });

  it('reports the indexer-shaped gap when getProgramAccounts is unavailable', async () => {
    const enumeration = await enumerateCoreMarketAddressesV1(
      client(new Map(), { headers: new Error('getProgramAccounts refused: RPC method is disabled') }),
      CORE,
    );
    expect(enumeration.mode).toBe('refused');
    if (enumeration.mode !== 'refused') throw new Error('expected a refusal');
    expect(enumeration.reason).toMatch(/RPC method is disabled/);
    expect(enumeration.note).toMatch(/dClutch has no index and this browser will not invent one/);
    expect(enumeration.addresses).toEqual([]);
  });

  it('parses only canonical, distinct, bounded known-Market address lists', () => {
    expect(parseMarketAddressListV1(` ${LIVE.market.address}\n${LIVE.claimsAggregate.address} `))
      .toEqual([LIVE.market.address, LIVE.claimsAggregate.address]);
    expect(parseMarketAddressListV1('   ')).toEqual([]);
    expect(() => parseMarketAddressListV1(`${LIVE.market.address} ${LIVE.market.address}`)).toThrow(/repeats a Market address/);
    expect(() => parseMarketAddressListV1('not-base58')).toThrow(/canonical Solana address/);
    expect(() => parseMarketAddressListV1(Array.from({ length: MARKET_DISCOVERY_MAX_ADDRESSES + 1 }, () => LIVE.market.address).join('\n')))
      .toThrow(new RegExp(`above the explicit ${MARKET_DISCOVERY_MAX_ADDRESSES}-Market browser bound`));
  });
});

describe('capability manifest authentication', () => {
  it('authenticates the live manifest record against the identity the Market committed to', async () => {
    // The manifest body is large; deriving its record address here proves the
    // browser looks in the same place the campaign published to.
    const accounts = new Map(await liveChain());
    const addresses = deriveFinalizedRecordAddressesV1(REGISTRY, CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1, new Uint8Array(32).fill(1));
    expect(addresses.record).not.toBe(addresses.staging);
    const discovery = await inspectMarketDiscoveryV1(client(accounts), {
      coreProgramId: CORE, registryProgramId: REGISTRY, claimsProgramId: CLAIMS, addresses: [LIVE.market.address],
    });
    const card = discovery.cards[0];
    if (card.status !== 'decoded') throw new Error(card.refusal);
    // Not published into this test chain, so it refuses by address, naming it.
    if (card.capabilities.status !== 'refused') throw new Error('expected a refused manifest');
    expect(card.capabilities.reason).toMatch(/^capability manifest record [1-9A-HJ-NP-Za-km-z]{32,44} is absent/);
  });
});
