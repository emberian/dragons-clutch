import { PublicKey } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import { LIVE, liveRpcAccount, mutate } from '../fixtures/liveOpenMarket';
import { sha256 } from './bytes';
import {
  collateralSubtotalsV1,
  curateMarketListingV1,
  enumerateCoreMarketAddressesV1,
  formatAtomsV1,
  inspectMarketDiscoveryV1,
  isCoreMarketHeaderV2,
  isCurrentCoreMarketAccountV1,
  isIncompatibleCoreMarketAccountV1,
  marketActivationOutlookV1,
  parseMarketAddressListV1,
  provenanceChipV1,
  shortAddressV1,
  MARKET_DISCOVERY_MAX_ADDRESSES,
  type DecodedMarketDiscoveryCardV1,
  type MarketCapabilityBadgeV1,
  type MarketDiscoveryCardV1,
} from './marketDiscovery';
import { type CapabilityFundingQuoteV1 } from './capabilityManifest';
import { deriveFinalizedRecordAddressesV1 } from './releaseRegistry';
import {
  CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
  CORE_STATE_BYTES,
  CORE_STATE_MAGIC,
  CORE_STATE_PRINCIPAL_CAP_SETS_OFFSET,
  CORE_STATE_RENT_BENEFICIARY_OFFSET,
  CORE_STATE_TERMINAL_RECEIPT_OFFSET,
  CORE_STATE_VERSION_OFFSET,
  CORE_VERSION,
  LIABILITY_BASIS_MARKET_CUSTODY_CONTEXT_OFFSET,
  LIABILITY_BASIS_MARKET_GENERATION_OFFSET,
  LIABILITY_BASIS_MARKET_RELEASE_SET_OFFSET,
  REALM_SCHEMA_RELEASE_ID_V1,
} from './generated/coreFound';
import { type RpcAccount, type SolanaRpcClient } from './rpc';

/**
 * Discovery joins historical finalized companion records to a parser-only
 * current Core body.
 *
 * The 352-byte Market fixture is retained only to prove this generation refuses
 * it. `CURRENT_MARKET_DATA` moves its stable identity prefix into the generated
 * current layout and supplies a nonzero test cap. It is unit-test input, never
 * external evidence of a post-upgrade Market. The Claims and Registry records
 * remain verbatim finalized bytes from the earlier campaign.
 */

const SYSTEM_PROGRAM = '11111111111111111111111111111111';
const CORE = LIVE.programs.core;
const REGISTRY = LIVE.programs.registry;
const CLAIMS = LIVE.programs.claims;
const CUSTODY = LIVE.programs.custody;
const SLOT = '99';

const CURRENT_MARKET_DATA = (() => {
  const bytes = new Uint8Array(CORE_STATE_BYTES);
  bytes.set(LIVE.market.data.slice(0, CORE_STATE_PRINCIPAL_CAP_SETS_OFFSET));
  bytes.set(CORE_STATE_MAGIC, 0);
  const view = new DataView(bytes.buffer);
  view.setUint16(CORE_STATE_VERSION_OFFSET, CORE_VERSION, true);
  view.setBigUint64(CORE_STATE_PRINCIPAL_CAP_SETS_OFFSET, 500_000_000n, true);
  bytes.set(LIVE.market.data.slice(288, 320), CORE_STATE_RENT_BENEFICIARY_OFFSET);
  bytes.set(LIVE.market.data.slice(320, 352), CORE_STATE_TERMINAL_RECEIPT_OFFSET);
  return bytes;
})();

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

/**
 * The Custody namespace the campaign that founded this Market chose.
 *
 * `SHA-256("dclutch:projected-hoard-context:v1" || SHA-256(campaign-domain ||
 * 0 || market || generation_le || release_set))`. The inner domain belongs to
 * `tools/local-validator/bootstrap/successor/src/market.rs` and is a campaign
 * convenience, not a protocol constant -- restated here only to reproduce ONE
 * recorded artifact. Shipped code reads the namespace off the Claims aggregate.
 */
async function liveFoundingNamespace(): Promise<Uint8Array> {
  const aggregate = LIVE.claimsAggregate.data;
  const generation = new DataView(aggregate.buffer, aggregate.byteOffset, aggregate.byteLength)
    .getBigUint64(LIABILITY_BASIS_MARKET_GENERATION_OFFSET, true);
  const generationBytes = new Uint8Array(8);
  new DataView(generationBytes.buffer).setBigUint64(0, generation, true);
  const context = await sha256(new Uint8Array([
    ...new TextEncoder().encode('dclutch/local-campaign/founding-context/v1'),
    0,
    ...new PublicKey(LIVE.market.address).toBytes(),
    ...generationBytes,
    ...aggregate.slice(LIABILITY_BASIS_MARKET_RELEASE_SET_OFFSET, LIABILITY_BASIS_MARKET_RELEASE_SET_OFFSET + 32),
  ]));
  return sha256(new Uint8Array([
    ...new TextEncoder().encode('dclutch:projected-hoard-context:v1'),
    ...context,
  ]));
}

/** The whole finalized graph the campaign left behind. */
async function liveChain(): Promise<Map<string, RpcAccount>> {
  const accounts = new Map<string, RpcAccount>([
    [LIVE.market.address, liveRpcAccount(LIVE.market, { data: CURRENT_MARKET_DATA })],
    [LIVE.claimsAggregate.address, liveRpcAccount(LIVE.claimsAggregate)],
  ]);
  const realm = await recordAddresses(REALM_SCHEMA_RELEASE_ID_V1, LIVE.realmRecord.data);
  accounts.set(realm.record, liveRpcAccount(LIVE.realmRecord));
  return accounts;
}

describe('Market discovery cards', () => {
  it('refuses the superseded finalized Market generation explicitly', async () => {
    const discovery = await inspectMarketDiscoveryV1(
      client(new Map([[LIVE.market.address, liveRpcAccount(LIVE.market)]])),
      { coreProgramId: CORE, addresses: [LIVE.market.address] },
    );
    expect(discovery.cards[0]).toMatchObject({ status: 'refused' });
    expect(discovery.cards[0].refusal).toMatch(/older devnet Market generation is incompatible/);
  });

  it('decodes a current parser body with the historical Claims and Realm records', async () => {
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
    expect(card.principalCapSets).toBe('500000000');
    expect(card.settlement).toEqual({ status: 'open', label: 'no terminal receipt' });
    expect(card.identity.schemaMagic).toBe('DCLTCOR3');
    expect(card.identity.accountBytes).toBe(368);
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

  it('says the Hoard is UNREAD when no Custody program was selected', async () => {
    const discovery = await inspectMarketDiscoveryV1(client(await liveChain()), {
      coreProgramId: CORE, registryProgramId: REGISTRY, claimsProgramId: CLAIMS, addresses: [LIVE.market.address],
    });
    const card = discovery.cards[0];
    if (card.status !== 'decoded') throw new Error(card.refusal);
    expect(card.hoard.status).toBe('unread');
    expect(JSON.stringify(card)).not.toContain('principalAtoms');
  });

  /**
   * The recorded chain is a Market whose aggregate does not reach its own
   * Hoard: it was founded before `FoundingV5` persisted the namespace it had
   * authenticated, so `custody_context` is the Market address while the
   * principal sits under the founding digest. The browser must say so and show
   * no figure, rather than deriving a plausible address and going quiet.
   */
  it('refuses the Hoard of a Market whose persisted namespace does not reach it', async () => {
    const discovery = await inspectMarketDiscoveryV1(client(await liveChain()), {
      coreProgramId: CORE, registryProgramId: REGISTRY, claimsProgramId: CLAIMS, custodyProgramId: CUSTODY,
      addresses: [LIVE.market.address],
    });
    const card = discovery.cards[0];
    if (card.status !== 'decoded') throw new Error(card.refusal);
    expect(card.hoard.status).toBe('refused');
    if (card.hoard.status !== 'refused') throw new Error('unreachable');
    expect(card.hoard.address).not.toBe(LIVE.hoardVault.address);
    expect(card.hoard.reason).toMatch(/no account exists at the derived Hoard Vault/);
    expect(JSON.stringify(card)).not.toContain('principalAtoms');
  });

  /**
   * One field, and the whole collateral path opens. Nothing else about these
   * bytes changes: the same aggregate, the same live Hoard, the same Realm --
   * only the 32 bytes `FoundingV5` now writes truthfully.
   */
  it('derives and authenticates the Hoard once the aggregate tells the truth', async () => {
    const accounts = await liveChain();
    accounts.set(LIVE.hoardVault.address, liveRpcAccount(LIVE.hoardVault));
    accounts.set(LIVE.claimsAggregate.address, liveRpcAccount(LIVE.claimsAggregate, {
      data: mutate(LIVE.claimsAggregate.data, LIABILITY_BASIS_MARKET_CUSTODY_CONTEXT_OFFSET, await liveFoundingNamespace()),
    }));
    const discovery = await inspectMarketDiscoveryV1(client(accounts), {
      coreProgramId: CORE, registryProgramId: REGISTRY, claimsProgramId: CLAIMS, custodyProgramId: CUSTODY,
      addresses: [LIVE.market.address],
    });
    const card = discovery.cards[0];
    if (card.status !== 'decoded') throw new Error(card.refusal);
    if (card.hoard.status !== 'derived') throw new Error(card.hoard.reason);
    expect(card.hoard.address).toBe(LIVE.hoardVault.address);
    expect(card.hoard.tokenProgram).toBe(card.collateral.status === 'bound' ? card.collateral.tokenProgram : '');
    if (card.liability.status !== 'bound') throw new Error(card.liability.reason);
    expect(card.hoard.principalAtoms).toBe(card.liability.requiredBackingAtoms);
  });

  it('refuses a Hoard whose token owner is not this Market\'s Custody authority', async () => {
    const accounts = await liveChain();
    accounts.set(LIVE.claimsAggregate.address, liveRpcAccount(LIVE.claimsAggregate, {
      data: mutate(LIVE.claimsAggregate.data, LIABILITY_BASIS_MARKET_CUSTODY_CONTEXT_OFFSET, await liveFoundingNamespace()),
    }));
    accounts.set(LIVE.hoardVault.address, liveRpcAccount(LIVE.hoardVault, {
      data: mutate(LIVE.hoardVault.data, 32, new PublicKey(LIVE.founder).toBytes()),
    }));
    const discovery = await inspectMarketDiscoveryV1(client(accounts), {
      coreProgramId: CORE, registryProgramId: REGISTRY, claimsProgramId: CLAIMS, custodyProgramId: CUSTODY,
      addresses: [LIVE.market.address],
    });
    const card = discovery.cards[0];
    if (card.status !== 'decoded') throw new Error(card.refusal);
    expect(card.hoard.status).toBe('refused');
    if (card.hoard.status !== 'refused') throw new Error('unreachable');
    expect(card.hoard.address).toBe(LIVE.hoardVault.address);
    expect(card.hoard.reason).toMatch(/Custody transfer authority/);
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

    const foreign = await inspectMarketDiscoveryV1(client(new Map([[LIVE.market.address, liveRpcAccount(LIVE.market, { owner: SYSTEM_PROGRAM, data: CURRENT_MARKET_DATA })]])), {
      coreProgramId: CORE, addresses: [LIVE.market.address],
    });
    expect(foreign.cards[0].refusal).toMatch(/owner differs from the selected Core program/);

    const truncated = CURRENT_MARKET_DATA.slice(0, CURRENT_MARKET_DATA.length - 1);
    const damaged = await inspectMarketDiscoveryV1(client(new Map([[LIVE.market.address, liveRpcAccount(LIVE.market, { data: truncated })]])), {
      coreProgramId: CORE, addresses: [LIVE.market.address],
    });
    expect(damaged.cards[0].refusal).toMatch(/the exact current width is 368/);
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
    expect(discovery.reason).toMatch(/No current compatible Market address has been supplied or enumerated/);
  });
});

describe('Market enumeration', () => {
  it('recognizes the live Core Market header and nothing else', () => {
    expect(isCoreMarketHeaderV2(CURRENT_MARKET_DATA)).toBe(true);
    expect(isCoreMarketHeaderV2(LIVE.claimsAggregate.data)).toBe(false);
    expect(isCoreMarketHeaderV2(LIVE.realmRecord.data)).toBe(false);
    // The superseded categorical Market layout is not a live Market header.
    const categorical = new Uint8Array(CORE_STATE_BYTES);
    categorical.set(new TextEncoder().encode('DCLTCAT1'));
    expect(isCoreMarketHeaderV2(categorical)).toBe(false);
  });

  it('recognizes each superseded generation, including the one sharing the current magic', () => {
    expect(isIncompatibleCoreMarketAccountV1(liveRpcAccount(LIVE.market))).toBe(true);
    expect(isIncompatibleCoreMarketAccountV1(liveRpcAccount(LIVE.market, {
      data: mutate(LIVE.market.data, 0, new TextEncoder().encode('DCLTCOR3')),
    }))).toBe(false);
    expect(isIncompatibleCoreMarketAccountV1({
      ...liveRpcAccount(LIVE.market),
      space: 351,
    })).toBe(false);
    // The bump tail widened DCLTCOR3 without moving its schema version, so the
    // pre-tail generation carries the CURRENT magic and version and is told
    // apart by width alone. Listing it as current would offer the reader an
    // account it cannot decode.
    const preTail = { data: CURRENT_MARKET_DATA.slice(0, 360), space: 360 };
    expect(isIncompatibleCoreMarketAccountV1(preTail)).toBe(true);
    expect(isCurrentCoreMarketAccountV1(preTail)).toBe(false);
    expect(isCoreMarketHeaderV2(preTail.data)).toBe(true);
    expect(isCurrentCoreMarketAccountV1({ data: CURRENT_MARKET_DATA, space: CORE_STATE_BYTES })).toBe(true);
  });

  it('lists current Markets while separately disclosing incompatible historical accounts', async () => {
    const enumeration = await enumerateCoreMarketAddressesV1(client(new Map(), {
      headers: [
        [LIVE.market.address, CURRENT_MARKET_DATA],
        [LIVE.founder, LIVE.market.data],
        [LIVE.claimsAggregate.address, LIVE.claimsAggregate.data.slice(0, 16)],
      ],
    }), CORE);
    expect(enumeration.mode).toBe('program-scan');
    expect(enumeration.addresses).toEqual([LIVE.market.address]);
    if (enumeration.mode !== 'program-scan') throw new Error('expected a program scan');
    expect(enumeration.incompatibleMarketAccounts).toEqual([{
      address: LIVE.founder,
      magic: 'DCLTCOR2',
      accountBytes: 352,
    }]);
    expect(enumeration.note).toMatch(/3 finalized Core accounts at slot 99; 1 carry the current DCLTCOR3 Market header/);
    expect(enumeration.note).toMatch(/1 historical Market account \(1 DCLTCOR2 at 352 bytes\); it is not listed as current/);
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

/**
 * The read arithmetic, pinned so it cannot silently return to a join per Market.
 *
 * Discovery once spent a one- or two-address `getMultipleAccounts` per Market
 * per companion record, inside a sequential loop: a full listing of 32 Markets
 * cost 129 round trips against public endpoints whose burst allowance is single
 * digits, and was refused partway through. The same reads are now collected per
 * round and asked for together, so the call count follows the 32-address batch
 * width rather than the Market count.
 */
describe('batched discovery reads', () => {
  it('spends one call per 32 addresses per round, not one call per Market per record', async () => {
    const accounts = await liveChain();
    accounts.set(LIVE.hoardVault.address, liveRpcAccount(LIVE.hoardVault));
    accounts.set(LIVE.claimsAggregate.address, liveRpcAccount(LIVE.claimsAggregate, {
      data: mutate(LIVE.claimsAggregate.data, LIABILITY_BASIS_MARKET_CUSTODY_CONTEXT_OFFSET, await liveFoundingNamespace()),
    }));
    // Thirty-one further Markets carrying the same finalized body at addresses
    // of their own. They commit to the same Realm and manifest identities, so
    // those records are collected once; their Claims aggregates are derived
    // from the Market address, so they are 31 distinct further reads.
    const extras = Array.from(
      { length: MARKET_DISCOVERY_MAX_ADDRESSES - 1 },
      (_, index) => new PublicKey(new Uint8Array(32).fill(index + 1)).toBase58(),
    );
    for (const address of extras) accounts.set(address, liveRpcAccount(LIVE.market, { data: CURRENT_MARKET_DATA }));

    const widths: number[] = [];
    const counted = client(accounts);
    const discovery = await inspectMarketDiscoveryV1(
      {
        finalizedSlot: () => counted.finalizedSlot(),
        multipleAccounts: (addresses: ReadonlyArray<string>, floor?: string) => {
          widths.push(addresses.length);
          return counted.multipleAccounts(addresses, floor);
        },
      } as unknown as SolanaRpcClient,
      {
        coreProgramId: CORE, registryProgramId: REGISTRY, claimsProgramId: CLAIMS, custodyProgramId: CUSTODY,
        addresses: [LIVE.market.address, ...extras],
      },
    );

    expect(discovery.cards).toHaveLength(MARKET_DISCOVERY_MAX_ADDRESSES);
    // Round one: 32 Market roots. Round two: 36 companion addresses -- one
    // Realm record and its staging cursor, one manifest record and its staging
    // cursor, and 32 Claims aggregates -- chunked at the 32-address batch
    // width. Round three: the single Hoard Vault a bound aggregate reaches,
    // plus the one collateral mint the Realm names. Four calls. One call per
    // Market per record would have been 129.
    //
    // The mint is the reason round three is two addresses rather than one, and
    // it is the reason the CALL COUNT did not move: display metadata that
    // earned its own round trip would not be worth reading.
    expect(widths).toEqual([32, 32, 4, 2]);
    expect(widths.every((width) => width >= 1 && width <= 32)).toBe(true);

    // And the join still lands: the campaign's real Market keeps the
    // authenticated Hoard it had when each record was read on its own.
    const card = discovery.cards[0];
    if (card.status !== 'decoded') throw new Error(card.refusal);
    if (card.hoard.status !== 'derived') throw new Error(card.hoard.reason);
    expect(card.hoard.address).toBe(LIVE.hoardVault.address);
  });

  /**
   * A refused batch is not a thrown listing. Each address in the chunk the
   * endpoint refused carries that refusal to the helper that asked for it, so
   * the card states the endpoint's own reason instead of the page dying.
   */
  it('carries a refused companion batch into the cards that asked for it', async () => {
    const counted = client(await liveChain());
    let call = 0;
    const discovery = await inspectMarketDiscoveryV1(
      {
        finalizedSlot: () => counted.finalizedSlot(),
        multipleAccounts: (addresses: ReadonlyArray<string>, floor?: string) => {
          call += 1;
          if (call > 1) throw new Error('429 Too Many Requests: rate limit exceeded');
          return counted.multipleAccounts(addresses, floor);
        },
      } as unknown as SolanaRpcClient,
      { coreProgramId: CORE, registryProgramId: REGISTRY, claimsProgramId: CLAIMS, addresses: [LIVE.market.address] },
    );
    const card = discovery.cards[0];
    if (card.status !== 'decoded') throw new Error(card.refusal);
    expect(card.collateral.status).toBe('refused');
    if (card.collateral.status !== 'refused') throw new Error('expected a refused Realm');
    expect(card.collateral.reason).toMatch(/429 Too Many Requests/);
    expect(card.liability.status).toBe('refused');
    expect(card.capabilities.status).toBe('refused');
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

/**
 * The collateral mint's display byte, and the curation that arranges a listing.
 *
 * These exist because a listing is not only a set of facts, it is an ORDER and a
 * SET OF TOTALS, and both can lie while every individual fact stays true. A
 * page that sums two mints into one figure has invented a unit; a page that
 * puts fourteen abandoned foundings ahead of the two live markets has told the
 * reader the wrong thing without stating a single falsehood. The cases below
 * pin the arrangement the same way the decoders are pinned.
 */

/** One base SPL Mint, optionally with Token-2022 extension bytes after it. */
function mintAccount(owner: string, decimals: number, extensionBytes = 0): RpcAccount {
  const data = new Uint8Array(82 + extensionBytes);
  new DataView(data.buffer).setBigUint64(36, 1_000_000_000n, true);
  data[44] = decimals;
  data[45] = 1;
  if (data.length > 165) data[165] = 1;
  return Object.freeze({ data, executable: false, lamports: '1', owner, space: data.length });
}

/** The live chain with an authenticated Hoard, plus whatever stands at its mint. */
async function chainWithHoard(mint: (address: string, tokenProgram: string) => RpcAccount | null): Promise<Map<string, RpcAccount>> {
  const accounts = await liveChain();
  accounts.set(LIVE.hoardVault.address, liveRpcAccount(LIVE.hoardVault));
  accounts.set(LIVE.claimsAggregate.address, liveRpcAccount(LIVE.claimsAggregate, {
    data: mutate(LIVE.claimsAggregate.data, LIABILITY_BASIS_MARKET_CUSTODY_CONTEXT_OFFSET, await liveFoundingNamespace()),
  }));
  // The Hoard's own bytes name the mint the Realm committed to, so the fixture
  // reads it off the vault rather than restating an address.
  const mintAddress = new PublicKey(LIVE.hoardVault.data.slice(0, 32)).toBase58();
  const account = mint(mintAddress, LIVE.hoardVault.owner);
  if (account !== null) accounts.set(mintAddress, account);
  return accounts;
}

async function derivedHoardCard(mint: (address: string, tokenProgram: string) => RpcAccount | null): Promise<DecodedMarketDiscoveryCardV1> {
  const discovery = await inspectMarketDiscoveryV1(client(await chainWithHoard(mint)), {
    coreProgramId: CORE, registryProgramId: REGISTRY, claimsProgramId: CLAIMS, custodyProgramId: CUSTODY,
    addresses: [LIVE.market.address],
  });
  const card = discovery.cards[0];
  if (card.status !== 'decoded') throw new Error(card.refusal);
  if (card.hoard.status !== 'derived') throw new Error(card.hoard.reason);
  return card;
}

describe('collateral mint display metadata', () => {
  it('reads the mint decimals beside the principal and never scales it', async () => {
    const card = await derivedHoardCard((_, tokenProgram) => mintAccount(tokenProgram, 6));
    if (card.hoard.status !== 'derived') throw new Error('unreachable');
    expect(card.hoard.mintDisplayDecimals).toBe(6);
    // The economics are unchanged. A mint authority picking a display byte may
    // not move the quantity this protocol settles in.
    if (card.liability.status !== 'bound') throw new Error(card.liability.reason);
    expect(card.hoard.principalAtoms).toBe(card.liability.requiredBackingAtoms);
  });

  it('accepts a Token-2022 mint carrying extensions, by its account-type byte', async () => {
    const card = await derivedHoardCard((_, tokenProgram) => mintAccount(tokenProgram, 9, 84));
    if (card.hoard.status !== 'derived') throw new Error('unreachable');
    expect(card.hoard.mintDisplayDecimals).toBe(9);
  });

  it('leaves decimals null rather than guessing when the mint does not authenticate', async () => {
    const absent = await derivedHoardCard(() => null);
    if (absent.hoard.status !== 'derived') throw new Error('unreachable');
    expect(absent.hoard.mintDisplayDecimals).toBeNull();

    const foreign = await derivedHoardCard(() => mintAccount(SYSTEM_PROGRAM, 6));
    if (foreign.hoard.status !== 'derived') throw new Error('unreachable');
    expect(foreign.hoard.mintDisplayDecimals).toBeNull();

    // A 165-byte token account past the base mint width is not a mint, and its
    // byte 44 is somebody's balance rather than a display precision.
    const impostor = await derivedHoardCard((_, tokenProgram) => Object.freeze({
      data: LIVE.hoardVault.data, executable: false, lamports: '1', owner: tokenProgram, space: LIVE.hoardVault.data.length,
    }));
    if (impostor.hoard.status !== 'derived') throw new Error('unreachable');
    expect(impostor.hoard.mintDisplayDecimals).toBeNull();

    // An uninitialized mint asserts nothing at all.
    const uninitialized = await derivedHoardCard((_, tokenProgram) => {
      const account = mintAccount(tokenProgram, 6);
      return Object.freeze({ ...account, data: mutate(account.data, 45, 0) });
    });
    if (uninitialized.hoard.status !== 'derived') throw new Error('unreachable');
    expect(uninitialized.hoard.mintDisplayDecimals).toBeNull();
  });

  it('costs the listing no extra round trip', async () => {
    // The mint joins the Hoard round because the Realm already named it. If it
    // ever earns its own round this count moves and this case says so.
    let rounds = 0;
    const accounts = await chainWithHoard((_, tokenProgram) => mintAccount(tokenProgram, 6));
    const counted = {
      finalizedSlot: async () => SLOT,
      multipleAccounts: async (addresses: ReadonlyArray<string>) => {
        rounds += 1;
        return Object.freeze({
          slot: SLOT,
          accounts: Object.freeze(addresses.map((address) => Object.freeze({ address, account: accounts.get(address) ?? null }))),
        });
      },
    } as unknown as SolanaRpcClient;
    await inspectMarketDiscoveryV1(counted, {
      coreProgramId: CORE, registryProgramId: REGISTRY, claimsProgramId: CLAIMS, custodyProgramId: CUSTODY,
      addresses: [LIVE.market.address],
    });
    expect(rounds).toBe(3);
  });
});

describe('collateral subtotals', () => {
  it('totals each token in its own units and never across two of them', async () => {
    const card = await derivedHoardCard((_, tokenProgram) => mintAccount(tokenProgram, 6));
    if (card.hoard.status !== 'derived') throw new Error('unreachable');
    const otherMint = new PublicKey(new Uint8Array(32).fill(3)).toBase58();
    const second: MarketDiscoveryCardV1 = Object.freeze({
      ...card,
      address: new PublicKey(new Uint8Array(32).fill(4)).toBase58(),
      hoard: Object.freeze({ ...card.hoard, collateralMint: otherMint, principalAtoms: '7', mintDisplayDecimals: 0 }),
    });
    const third: MarketDiscoveryCardV1 = Object.freeze({
      ...card,
      address: new PublicKey(new Uint8Array(32).fill(5)).toBase58(),
      hoard: Object.freeze({ ...card.hoard, principalAtoms: '1' }),
    });

    const rows = collateralSubtotalsV1([card, second, third]);
    expect(rows).toHaveLength(2);
    // Biggest first, and each row is exactly the sum of its OWN mint's vaults.
    expect(rows[0]).toMatchObject({
      collateralMint: card.hoard.collateralMint,
      principalAtoms: (BigInt(card.hoard.principalAtoms) + 1n).toString(),
      vaults: 2,
      mintDisplayDecimals: 6,
    });
    expect(rows[1]).toMatchObject({ collateralMint: otherMint, principalAtoms: '7', vaults: 1, mintDisplayDecimals: 0 });
    expect(rows[0].collateralMintShort).toBe(shortAddressV1(card.hoard.collateralMint, 5));
    // Nothing anywhere in the result is the two mints added together.
    expect(rows.map((row) => row.principalAtoms)).not.toContain((BigInt(card.hoard.principalAtoms) + 8n).toString());
  });

  it('omits a vault that refused authentication rather than counting it as zero', async () => {
    const card = await derivedHoardCard((_, tokenProgram) => mintAccount(tokenProgram, 6));
    const refusedHoard: MarketDiscoveryCardV1 = Object.freeze({
      ...card,
      address: new PublicKey(new Uint8Array(32).fill(6)).toBase58(),
      hoard: Object.freeze({ status: 'refused', address: null, reason: 'test refusal' }),
    });
    const rows = collateralSubtotalsV1([card, refusedHoard]);
    expect(rows).toHaveLength(1);
    expect(rows[0].vaults).toBe(1);
    expect(collateralSubtotalsV1([refusedHoard])).toEqual([]);
  });

  it('distrusts a mint whose decimals two reads disagree about', async () => {
    const card = await derivedHoardCard((_, tokenProgram) => mintAccount(tokenProgram, 6));
    if (card.hoard.status !== 'derived') throw new Error('unreachable');
    const disagreeing: MarketDiscoveryCardV1 = Object.freeze({
      ...card,
      address: new PublicKey(new Uint8Array(32).fill(7)).toBase58(),
      hoard: Object.freeze({ ...card.hoard, mintDisplayDecimals: 9 }),
    });
    expect(collateralSubtotalsV1([card, disagreeing])[0].mintDisplayDecimals).toBeNull();
  });
});

/**
 * Whether a Market that reads `Open` can ever have trading switched on.
 *
 * The chain has five phases and none of them is "permanently untradeable", so a
 * Market whose activation window shut is byte-identical, in phase, to one whose
 * trading has simply not started. The difference is decidable anyway -- Core
 * refuses activation once `current_slot > deadline`, and the manifest is sealed
 * into the Market's own address -- and these pin that it is decided from those
 * two facts and from nothing softer. The failure that matters is a false
 * POSITIVE: telling a reader a Market is dead on a read that did not happen.
 */
const NO_FUNDING: CapabilityFundingQuoteV1 = Object.freeze({
  compartments: Object.freeze([]),
  nativeLamportsTotal: BigInt(0),
  realmCollateralTotal: BigInt(0),
  realmCollateral: null,
});

function capabilityBadge(index: number, activation: 'immediate' | 'deadline', deadline: string | null): MarketCapabilityBadgeV1 {
  return Object.freeze({
    index,
    kindId: 'ab'.repeat(32),
    label: 'Direct successor',
    recognized: true,
    programSetId: 'cd'.repeat(32),
    configId: 'ef'.repeat(32),
    activation,
    deadline,
    dependencies: Object.freeze([]),
    funding: NO_FUNDING,
  });
}

/** One `Open` card carrying exactly the badges and outstanding count given. */
async function activationCard(
  address: string,
  badges: ReadonlyArray<MarketCapabilityBadgeV1>,
  outstandingCapabilities = '0',
): Promise<DecodedMarketDiscoveryCardV1> {
  const card = await derivedHoardCard((_, tokenProgram) => mintAccount(tokenProgram, 6));
  return Object.freeze({
    ...card,
    address,
    phase: 'Open',
    outstandingCapabilities,
    capabilities: Object.freeze({
      status: 'authenticated',
      manifestId: 'ab'.repeat(32),
      recordAddress: LIVE.market.address,
      observedSlot: card.observedSlot,
      badges: Object.freeze([...badges]),
    }),
  });
}

const UNTRADEABLE = 'shut1111111111111111111111111111111111111111';

describe('whether an open Market can ever trade', () => {
  it('calls the window shut only once the deadline is strictly below a finalized slot', async () => {
    const card = await activationCard(UNTRADEABLE, [capabilityBadge(0, 'deadline', '98')]);
    expect(card.observedSlot).toBe('99');
    const shut = marketActivationOutlookV1(card);
    if (shut.status !== 'never') throw new Error(`expected never, got ${shut.status}: ${shut.reason}`);
    expect(shut.lastActivationSlot).toBe('98');
    expect(shut.observedSlot).toBe('99');
    expect(shut.reason).toMatch(/had to be activated by slot 98/);

    // Core refuses at `current_slot > deadline`, so the deadline slot itself is
    // still live. Rounding that the other way would bury a Market a slot early.
    const onTheDeadline = await activationCard(UNTRADEABLE, [capabilityBadge(0, 'deadline', '99')]);
    expect(marketActivationOutlookV1(onTheDeadline).status).toBe('reachable');
  });

  it('takes the last deadline of several, and needs every one of them elapsed', async () => {
    const allShut = await activationCard(UNTRADEABLE, [capabilityBadge(0, 'deadline', '50'), capabilityBadge(1, 'deadline', '90')]);
    const verdict = marketActivationOutlookV1(allShut);
    if (verdict.status !== 'never') throw new Error('expected never');
    expect(verdict.lastActivationSlot).toBe('90');

    // One entry still live is the whole Market still live.
    const oneLive = await activationCard(UNTRADEABLE, [capabilityBadge(0, 'deadline', '50'), capabilityBadge(1, 'deadline', '400')]);
    expect(marketActivationOutlookV1(oneLive)).toMatchObject({ status: 'reachable' });
  });

  it('treats an on-demand capability as reachable however old the Market is', async () => {
    const immediate = await activationCard(UNTRADEABLE, [capabilityBadge(0, 'deadline', '10'), capabilityBadge(1, 'immediate', null)]);
    expect(marketActivationOutlookV1(immediate).status).toBe('reachable');
  });

  it('never calls a Market untradeable while it already holds an activated capability', async () => {
    const activated = await activationCard(UNTRADEABLE, [capabilityBadge(0, 'deadline', '10')], '1');
    expect(marketActivationOutlookV1(activated).status).toBe('reachable');
  });

  it('leaves a manifest it could not authenticate UNKNOWN rather than shut', async () => {
    // The base fixture publishes no manifest record, so its manifest refuses by
    // address. A refused read is not a closed window and never becomes one.
    const unread = await derivedHoardCard((_, tokenProgram) => mintAccount(tokenProgram, 6));
    expect(unread.capabilities.status).toBe('refused');
    const verdict = marketActivationOutlookV1(unread);
    expect(verdict.status).toBe('unknown');
    expect(verdict.reason).toMatch(/capability manifest record/);

    const refusedAccount: MarketDiscoveryCardV1 = Object.freeze({
      status: 'refused',
      address: 'refuse11111111111111111111111111111111111111',
      provenance: Object.freeze({ kind: 'refused' as const, reason: 'undecodable' }),
      observedSlot: SLOT,
      refusal: 'undecodable',
    });
    expect(marketActivationOutlookV1(refusedAccount).status).toBe('unknown');
  });

  it('will not read an entryless manifest as a closed window', async () => {
    const empty = await activationCard(UNTRADEABLE, []);
    expect(marketActivationOutlookV1(empty).status).toBe('unknown');
  });
});

/**
 * The 360-byte generation, from the program scan through to the bucket.
 *
 * `(DCLTCOR3, version 3, 360 bytes)` is the pre-bump-tail Core state: CURRENT
 * magic, CURRENT schema version, superseded width, told apart by width alone.
 * Every Market live on this cluster is one of these, and when the reader and
 * the deployed cohort next disagree it is not an edge case at all -- it is
 * every card on the page.
 *
 * The predicate pair is already pinned on it directly. What was not pinned is
 * what it does to a LISTING: the only end-to-end scan case was `DCLTCOR2 at
 * 352`, so the scan note, the card refusal, and the group it lands in had never
 * been run for the width that is about to be universal.
 *
 * The load-bearing case is the last one. An account this reader cannot decode
 * must never be reported as a market that can never trade. That verdict is
 * spoken only from an authenticated manifest, and a read that failed is not
 * evidence of a shut window -- it is the absence of evidence about one.
 */
describe('the 360-byte Core generation, from scan to bucket', () => {
  // The Markets actually standing on devnet, at the width the deployed cohort
  // wrote and this reader no longer decodes.
  const FLAGSHIP = '7Mcu1ZT9KZBnvLZ2vhSvLeQMRA1ejQWD93yyPF2k8WAC';
  const ORPHAN = 'CasyDFowGxqREDW5iWvKRgSMCgk5HnLQjnjegvRsSNPM';
  const preTail = CURRENT_MARKET_DATA.slice(0, 360);

  async function refusedPreTailCard(): Promise<MarketDiscoveryCardV1> {
    const discovery = await inspectMarketDiscoveryV1(
      client(new Map([[FLAGSHIP, Object.freeze({ data: preTail, executable: false, lamports: '1', owner: CORE, space: 360 })]])),
      { coreProgramId: CORE, addresses: [FLAGSHIP] },
    );
    return discovery.cards[0];
  }

  it('is separated from the current generation by width alone, not by magic or version', () => {
    const view = new DataView(preTail.buffer, preTail.byteOffset, preTail.byteLength);
    expect(new TextDecoder().decode(preTail.slice(0, CORE_STATE_MAGIC.length))).toBe('DCLTCOR3');
    expect(view.getUint16(CORE_STATE_VERSION_OFFSET, true)).toBe(CORE_VERSION);
    expect(isCoreMarketHeaderV2(preTail)).toBe(true);
    // Everything a magic-only or version-only check could look at agrees with
    // the current generation. Only the width disagrees, and it is decisive.
    expect(preTail.length).toBe(360);
    expect(CORE_STATE_BYTES).toBe(368);
    expect(isCurrentCoreMarketAccountV1({ data: preTail, space: 360 })).toBe(false);
    expect(isIncompatibleCoreMarketAccountV1({ data: preTail, space: 360 })).toBe(true);
  });

  it('is enumerated as historical and never offered as an address to read', async () => {
    const enumeration = await enumerateCoreMarketAddressesV1(client(new Map(), {
      headers: [[FLAGSHIP, preTail], [ORPHAN, preTail], [LIVE.market.address, CURRENT_MARKET_DATA]],
    }), CORE);
    if (enumeration.mode !== 'program-scan') throw new Error('expected a program scan');
    expect(enumeration.addresses).toEqual([LIVE.market.address]);
    expect(enumeration.incompatibleMarketAccounts).toEqual([
      { address: FLAGSHIP, magic: 'DCLTCOR3', accountBytes: 360 },
      { address: ORPHAN, magic: 'DCLTCOR3', accountBytes: 360 },
    ]);
    // The note names the generation by its own width, so a reader is never
    // told "DCLTCOR3" and left to assume it was one this build can read.
    expect(enumeration.note).toMatch(/2 historical Market accounts \(2 DCLTCOR3 at 360 bytes\); they are not listed as current/);
    expect(enumeration.note).toMatch(/1 carry the current DCLTCOR3 Market header/);
  });

  it('refuses the card with its exact reason instead of decoding a width it does not know', async () => {
    const card = await refusedPreTailCard();
    if (card.status !== 'refused') throw new Error('a width this reader does not know is never decoded');
    expect(card.address).toBe(FLAGSHIP);
    // A different refusal from the 352-byte generation's, which is exactly why
    // this needed its own case: that one is caught by magic, this one only by
    // the width, and the message a reader gets says so.
    expect(card.refusal).toBe('Core Market state is 360 bytes; the exact current width is 368.');
    expect(provenanceChipV1(card.provenance)).toBe('REFUSED');
  });

  it('lands in the unreadable group, and in neither open nor untradeable', async () => {
    const card = await refusedPreTailCard();
    const live = await derivedHoardCard((_, tokenProgram) => mintAccount(tokenProgram, 6));
    const listing = curateMarketListingV1([card, live]);
    expect(listing.unreadable.map((entry) => entry.address)).toEqual([FLAGSHIP]);
    expect(listing.open.map((entry) => entry.address)).toEqual([live.address]);
    expect(listing.untradeable).toEqual([]);
    // Even named as the deployment's headline it is not promoted, because a
    // featured address only ever reorders what the chain already said.
    expect(curateMarketListingV1([card, live], FLAGSHIP).open.map((entry) => entry.address)).toEqual([live.address]);
  });

  it('is never reported as a market that can never trade', async () => {
    const card = await refusedPreTailCard();
    const verdict = marketActivationOutlookV1(card);
    expect(verdict.status).toBe('unknown');
    expect(verdict.reason).toMatch(/did not decode/);
    // Stated as the thing that must not happen, not only as the thing that
    // does: a failed decode may never be spoken as a verdict about trading.
    expect(verdict.status).not.toBe('never');
  });
});

describe('listing curation', () => {
  async function phasedCard(address: string, phase: 'Founding' | 'Open' | 'Terminal' | 'Retiring' | 'Retired'): Promise<MarketDiscoveryCardV1> {
    const card = await derivedHoardCard((_, tokenProgram) => mintAccount(tokenProgram, 6));
    return Object.freeze({ ...card, address, phase });
  }

  it('partitions the listing exactly, with nothing dropped and nothing doubled', async () => {
    const open = await phasedCard('open1111111111111111111111111111111111111111', 'Open');
    const second = await phasedCard('open2222222222222222222222222222222222222222', 'Open');
    const founding = await phasedCard('found111111111111111111111111111111111111111', 'Founding');
    const terminal = await phasedCard('term1111111111111111111111111111111111111111', 'Terminal');
    const retired = await phasedCard('retire11111111111111111111111111111111111111', 'Retired');
    const refused: MarketDiscoveryCardV1 = Object.freeze({
      status: 'refused',
      address: 'refuse11111111111111111111111111111111111111',
      provenance: Object.freeze({ kind: 'refused' as const, reason: 'an older layout this reader cannot decode' }),
      observedSlot: SLOT,
      refusal: 'an older layout this reader cannot decode',
    });

    const cards = [founding, open, refused, terminal, second, retired];
    const listing = curateMarketListingV1(cards);
    expect(listing.open.map((card) => card.address)).toEqual([open.address, second.address]);
    expect(listing.founding.map((card) => card.address)).toEqual([founding.address]);
    expect(listing.settled.map((card) => card.address)).toEqual([terminal.address, retired.address]);
    expect(listing.unreadable.map((card) => card.address)).toEqual([refused.address]);

    const grouped = [...listing.open, ...listing.settled, ...listing.founding, ...listing.unreadable];
    expect(grouped).toHaveLength(cards.length);
    expect(new Set(grouped.map((card) => card.address)).size).toBe(cards.length);
  });

  it('moves the featured Market to the front of the open group and nowhere else', async () => {
    const first = await phasedCard('open1111111111111111111111111111111111111111', 'Open');
    const featured = await phasedCard('open2222222222222222222222222222222222222222', 'Open');
    const founding = await phasedCard('found111111111111111111111111111111111111111', 'Founding');

    const listing = curateMarketListingV1([first, featured, founding], featured.address);
    expect(listing.open.map((card) => card.address)).toEqual([featured.address, first.address]);

    // A featured address the chain does not say is open is NOT promoted. The
    // page may reorder what the chain reports; it may not restate it.
    const unfounded = curateMarketListingV1([first, featured, founding], founding.address);
    expect(unfounded.open.map((card) => card.address)).toEqual([first.address, featured.address]);
    expect(unfounded.founding.map((card) => card.address)).toEqual([founding.address]);

    const absent = curateMarketListingV1([first, featured], 'missing11111111111111111111111111111111111111');
    expect(absent.open.map((card) => card.address)).toEqual([first.address, featured.address]);
  });

  it('files an open Market whose activation window shut apart from the open ones', async () => {
    const live = await phasedCard('open1111111111111111111111111111111111111111', 'Open');
    const shut = await activationCard(UNTRADEABLE, [capabilityBadge(0, 'deadline', '98')]);
    const founding = await phasedCard('found111111111111111111111111111111111111111', 'Founding');

    const listing = curateMarketListingV1([live, shut, founding]);
    expect(listing.open.map((card) => card.address)).toEqual([live.address]);
    expect(listing.untradeable.map((card) => card.address)).toEqual([shut.address]);
    // It is separated, never dropped: the five groups still partition exactly.
    const grouped = [...listing.open, ...listing.untradeable, ...listing.settled, ...listing.founding, ...listing.unreadable];
    expect(grouped).toHaveLength(3);
    expect(new Set(grouped.map((card) => card.address)).size).toBe(3);
    // And the phase it prints is still the chain's: the separation is the
    // page's arrangement, not a restatement of what the account says.
    expect(listing.untradeable[0].phase).toBe('Open');
  });

  it('does not promote a featured Market that can never trade into the open group', async () => {
    const live = await phasedCard('open1111111111111111111111111111111111111111', 'Open');
    const shut = await activationCard(UNTRADEABLE, [capabilityBadge(0, 'deadline', '98')]);
    const listing = curateMarketListingV1([live, shut], shut.address);
    expect(listing.open.map((card) => card.address)).toEqual([live.address]);
    expect(listing.untradeable.map((card) => card.address)).toEqual([shut.address]);
  });

  it('returns five empty groups for an empty listing rather than inventing one', () => {
    expect(curateMarketListingV1([])).toEqual({ open: [], untradeable: [], settled: [], founding: [], unreadable: [] });
  });
});

describe('exact display formatting', () => {
  it('scales atoms by a mint precision without floating point', () => {
    expect(formatAtomsV1('500000000', 6)).toBe('500');
    expect(formatAtomsV1('500000001', 6)).toBe('500.000001');
    expect(formatAtomsV1('1', 6)).toBe('0.000001');
    expect(formatAtomsV1('0', 6)).toBe('0');
    expect(formatAtomsV1('123', 0)).toBe('123');
    // A u64 whose exact value no IEEE double can carry.
    expect(formatAtomsV1('18446744073709551615', 9)).toBe('18446744073.709551615');
    expect(() => formatAtomsV1('1', -1)).toThrow(/one u8/);
    expect(() => formatAtomsV1('1', 256)).toThrow(/one u8/);
  });
});
