import { describe, expect, it } from 'vitest';

import { LIVE, liveRpcAccount, mutate } from '../fixtures/liveOpenMarket';
import { sha256 } from './bytes';
import {
  CORE_STATE_BYTES,
  CORE_STATE_MAGIC,
  CORE_STATE_PRINCIPAL_CAP_SETS_OFFSET,
  CORE_STATE_RENT_BENEFICIARY_OFFSET,
  CORE_STATE_TERMINAL_RECEIPT_OFFSET,
  CORE_STATE_VERSION_OFFSET,
  CORE_VERSION,
  REALM_SCHEMA_RELEASE_ID_V1,
} from './generated/coreFound';
import {
  capabilityProvenanceV1,
  inspectMarketDetailV1,
  liabilityProvenanceV1,
  marketPhaseMeaningV1,
  realmProvenanceV1,
  requiredBackingMeaningV1,
} from './marketDetail';
import { provenanceChipV1 } from './marketDiscovery';
import { deriveFinalizedRecordAddressesV1 } from './releaseRegistry';
import { type RpcAccount, type SolanaRpcClient } from './rpc';

/**
 * The detail projection over a parser-only current Core body joined to the
 * historical campaign's finalized companion records.
 *
 * The 352-byte finalized Market is retained as legacy-refusal evidence. The
 * current body below is synthetic unit input, not post-upgrade chain evidence.
 * Variants are written into that current body. The Core V2 Market
 * address is derived from the eight identities plus the generation at offsets
 * 48..280, so mutating the phase byte at 10 or the generated receipt offset
 * leaves every derived address in the fixture correct — which is what lets a
 * terminal variant be tested without forging a whole account.
 */

const SYSTEM_PROGRAM = '11111111111111111111111111111111';
const CORE = LIVE.programs.core;
const REGISTRY = LIVE.programs.registry;
const CLAIMS = LIVE.programs.claims;
const SLOT = '4711';

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

function client(accounts: ReadonlyMap<string, RpcAccount>): SolanaRpcClient {
  return {
    finalizedSlot: async () => SLOT,
    multipleAccounts: async (addresses: ReadonlyArray<string>) => Object.freeze({
      slot: SLOT,
      accounts: Object.freeze(addresses.map((address) => Object.freeze({ address, account: accounts.get(address) ?? null }))),
    }),
  } as unknown as SolanaRpcClient;
}

async function chain(marketData: Uint8Array = CURRENT_MARKET_DATA): Promise<Map<string, RpcAccount>> {
  const accounts = new Map<string, RpcAccount>([
    [LIVE.market.address, liveRpcAccount(LIVE.market, { data: marketData })],
    [LIVE.claimsAggregate.address, liveRpcAccount(LIVE.claimsAggregate)],
  ]);
  const realm = deriveFinalizedRecordAddressesV1(REGISTRY, REALM_SCHEMA_RELEASE_ID_V1, await sha256(LIVE.realmRecord.data));
  accounts.set(realm.record, liveRpcAccount(LIVE.realmRecord));
  return accounts;
}

/** A copy of the live Market with a terminal receipt and a winning claim. */
function terminalMarket(winner: number): Uint8Array {
  // Core V2: phase@10, terminal winner@12, generated terminal receipt offset.
  const withPhase = mutate(CURRENT_MARKET_DATA, 10, 2);
  const withWinner = mutate(withPhase, 12, winner);
  return mutate(withWinner, CORE_STATE_TERMINAL_RECEIPT_OFFSET, new Uint8Array(32).fill(0x77));
}

const full = { coreProgramId: CORE, registryProgramId: REGISTRY, claimsProgramId: CLAIMS };

describe('Market detail projection', () => {
  it('refuses the superseded finalized Market generation', async () => {
    const detail = await inspectMarketDetailV1(
      client(new Map([[LIVE.market.address, liveRpcAccount(LIVE.market)]])),
      { ...full, address: LIVE.market.address },
    );
    expect(detail.card).toMatchObject({ status: 'refused' });
    expect(detail.card.refusal).toMatch(/older devnet Market generation is incompatible/);
  });

  it('carries the immutable identities, the artifact profile, and an honest phase meaning', async () => {
    const detail = await inspectMarketDetailV1(client(await chain()), { ...full, address: LIVE.market.address });
    expect(detail.floorSlot).toBe(SLOT);
    const card = detail.card;
    if (card.status !== 'decoded') throw new Error(card.refusal);
    expect(card.phase).toBe('Open');
    expect(detail.phaseMeaning).toBe(marketPhaseMeaningV1('Open'));
    expect(detail.phaseMeaning).toMatch(/nothing can be cashed in/);
    expect(card.identity).toMatchObject({
      schemaMagic: 'DCLTCOR3',
      schemaVersion: 3,
      accountBytes: 368,
      marketId: LIVE.market.address,
      registryProgram: REGISTRY,
    });
    for (const identity of [
      card.identity.realmId, card.identity.productRecordId, card.identity.productInstanceId,
      card.identity.resolutionPolicyId, card.identity.capabilityManifestId, card.identity.selectedReleaseSetId,
    ]) expect(identity).toMatch(/^[0-9a-f]{64}$/);
    expect(detail.claimsProgramId).toBe(CLAIMS);
  });

  it('states the exact required backing and the basis it is measured against', async () => {
    const open = await inspectMarketDetailV1(client(await chain()), { ...full, address: LIVE.market.address });
    const openCard = open.card;
    if (openCard.status !== 'decoded' || openCard.liability.status !== 'bound') throw new Error('expected bound liabilities');
    expect(openCard.liability.requiredBackingAtoms).toBe('500000000');
    expect(openCard.liability.requiredBackingBasis).toBe('maximum-claim-supply');
    expect(requiredBackingMeaningV1('maximum-claim-supply')).toMatch(/every claim could still be the one that pays/);

    const terminal = await inspectMarketDetailV1(client(await chain(terminalMarket(2))), { ...full, address: LIVE.market.address });
    const terminalCard = terminal.card;
    if (terminalCard.status !== 'decoded' || terminalCard.liability.status !== 'bound') throw new Error('expected bound liabilities');
    expect(terminalCard.phase).toBe('Terminal');
    expect(terminalCard.settlement).toMatchObject({ status: 'terminal', winner: 2 });
    expect(terminalCard.liability.requiredBackingBasis).toBe('winning-claim-supply');
    expect(requiredBackingMeaningV1('winning-claim-supply')).toMatch(/only winning claims can still be paid/);
  });

  it('sources the supply vector from the Claims aggregate, never from the Market root', async () => {
    const detail = await inspectMarketDetailV1(client(await chain()), { ...full, address: LIVE.market.address });
    const card = detail.card;
    if (card.status !== 'decoded' || card.liability.status !== 'bound') throw new Error('expected bound liabilities');
    expect(card.liability.aggregateAddress).toBe(LIVE.claimsAggregate.address);
    expect(card.liability.supplyAtoms).toEqual(['500000000', '500000000', '500000000', '500000000']);
    expect(provenanceChipV1(liabilityProvenanceV1(card.liability))).toBe(`CHAIN · finalized slot ${SLOT}`);
    // The Market bytes themselves hold no such vector; if the Claims aggregate
    // is missing, the section refuses rather than falling back to the root.
    const withoutAggregate = new Map(await chain());
    withoutAggregate.delete(LIVE.claimsAggregate.address);
    const blind = await inspectMarketDetailV1(client(withoutAggregate), { ...full, address: LIVE.market.address });
    const blindCard = blind.card;
    if (blindCard.status !== 'decoded') throw new Error(blindCard.refusal);
    expect(blindCard.liability.status).toBe('refused');
    expect(provenanceChipV1(blind.liabilityProvenance)).toBe('REFUSED');
  });

  it('reports the Realm exactly as its finalized record decodes, never as Market hearsay', async () => {
    const detail = await inspectMarketDetailV1(client(await chain()), { ...full, address: LIVE.market.address });
    const card = detail.card;
    if (card.status !== 'decoded' || card.collateral.status !== 'bound') throw new Error('expected a bound Realm');
    expect(card.collateral.realmContentId).toBe(card.identity.realmId);
    expect(card.collateral.tokenProgram).toBe('TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb');
    expect(card.collateral.mintAuthorityPolicy).toBe('Require absent');
    expect(card.collateral.freezeAuthorityPolicy).toBe('Require absent');
    expect(provenanceChipV1(realmProvenanceV1(card.collateral))).toBe(`CHAIN · finalized slot ${SLOT}`);
  });

  it('refuses every section a Registry-less read cannot authenticate, and says why', async () => {
    const detail = await inspectMarketDetailV1(client(await chain()), { coreProgramId: CORE, claimsProgramId: CLAIMS, address: LIVE.market.address });
    const card = detail.card;
    if (card.status !== 'decoded') throw new Error(card.refusal);
    expect(card.capabilities.status).toBe('unread');
    expect(card.collateral.status).toBe('unread');
    expect(provenanceChipV1(capabilityProvenanceV1(card.capabilities))).toBe('REFUSED');
    expect(provenanceChipV1(realmProvenanceV1(card.collateral))).toBe('REFUSED');
    // Liabilities were still read: they need Claims, not Registry.
    expect(card.liability.status).toBe('bound');
  });

  it('refuses the whole detail when the Market itself is absent or foreign', async () => {
    const absent = await inspectMarketDetailV1(client(new Map()), { ...full, address: LIVE.market.address });
    expect(absent.card.status).toBe('refused');
    expect(absent.reason).toMatch(/absent at the finalized observation floor/);
    expect(absent.phaseMeaning).toBeNull();
    expect(provenanceChipV1(absent.realmProvenance)).toBe('REFUSED');
    expect(provenanceChipV1(absent.liabilityProvenance)).toBe('REFUSED');

    const foreign = await inspectMarketDetailV1(
      client(new Map([[LIVE.market.address, liveRpcAccount(LIVE.market, { owner: SYSTEM_PROGRAM, data: CURRENT_MARKET_DATA })]])),
      { ...full, address: LIVE.market.address },
    );
    expect(foreign.card.status).toBe('refused');
    expect(foreign.reason).toMatch(/owner differs from the selected Core program/);
  });
});
