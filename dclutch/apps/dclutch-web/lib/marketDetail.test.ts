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
  failureEscrowOwnerV1,
  inspectMarketDetailV1,
  liabilityProvenanceV1,
  marketPhaseMeaningV1,
  outageDisclosureV1,
  realmProvenanceV1,
  requiredBackingMeaningV1,
  terminalOutcomeMeaningV1,
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
    // Renegotiated 2026-08-31: the meaning strings carried a trailing
    // justification ("until the answer is in, every claim could still be the
    // one that pays"). They are captions on a labelled figure now.
    expect(requiredBackingMeaningV1('maximum-claim-supply')).toMatch(/biggest claim count on any one outcome/);

    const terminal = await inspectMarketDetailV1(client(await chain(terminalMarket(2))), { ...full, address: LIVE.market.address });
    const terminalCard = terminal.card;
    if (terminalCard.status !== 'decoded' || terminalCard.liability.status !== 'bound') throw new Error('expected bound liabilities');
    expect(terminalCard.phase).toBe('Terminal');
    expect(terminalCard.settlement).toMatchObject({ status: 'terminal', winner: 2 });
    expect(terminalCard.liability.requiredBackingBasis).toBe('winning-claim-supply');
    expect(requiredBackingMeaningV1('winning-claim-supply')).toMatch(/claim count on the outcome that won/);
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

describe('what a resolved market\'s answer means for a holder', () => {
  it('names the source-failure outcome as a disclosed fallback, never as a fault', () => {
    // Cohort-13's exact shape: four outcomes, the last one won.
    const meaning = terminalOutcomeMeaningV1({ winner: 3, outcomeCount: 4, outcomeName: 'The source failed to report' });
    expect(meaning.sourceFailure).toBe(true);
    expect(meaning.headline).toMatch(/data source never reported/);
    expect(meaning.headline, 'the fallback was named and paid for in advance').toMatch(/before it opened/);
    expect(meaning.headline, 'not a fault, and the page must not let it read as one').toMatch(/did not get stuck/);
    // The derived label IS the sentence "The source failed to report", so
    // naming it inside a sentence that already says so reads as a stutter.
    expect(meaning.headline).not.toMatch(/which is The source failed to report/);
    expect(meaning.headline).toMatch(/claim 3/);
    expect(meaning.forTheWinners).toMatch(/Anyone holding claim 3 can cash in/);
  });

  it('uses the outcome\'s own name when the source did report', () => {
    const meaning = terminalOutcomeMeaningV1({ winner: 1, outcomeCount: 4, outcomeName: 'Between $120 and $180' });
    expect(meaning.sourceFailure).toBe(false);
    expect(meaning.headline).toBe('The data source reported, and Between $120 and $180 is the outcome that won.');
    expect(meaning.forTheWinners).toMatch(/Anyone holding Between \$120 and \$180 can cash in/);
  });

  it('falls back to the claim index when nothing named the outcome', () => {
    const meaning = terminalOutcomeMeaningV1({ winner: 1, outcomeCount: 4, outcomeName: undefined });
    expect(meaning.headline).toBe('The data source reported, and claim 1 is the outcome that won.');
  });

  it('tells a losing holder they are finished, not waiting', () => {
    // The sentence a wallet with zero at the winning claim needs, and the one
    // the redemption flow only ever produced after two clicks.
    for (const winner of [0, 3]) {
      const meaning = terminalOutcomeMeaningV1({ winner, outcomeCount: 4 });
      expect(meaning.forEveryoneElse).toMatch(/worth exactly nothing/);
      expect(meaning.forEveryoneElse).toMatch(/not stuck and it is not pending/);
      expect(meaning.forEveryoneElse).toMatch(/pays zero/);
    }
  });

  it('claims no source failure when the outcome width is unread', () => {
    // A width of zero means the claims ledger was not read, not that the last
    // outcome won. Saying "the source never reported" off an unread width
    // would be an invented fact on the page's most load-bearing sentence.
    expect(terminalOutcomeMeaningV1({ winner: 0, outcomeCount: 0 }).sourceFailure).toBe(false);
  });
});

describe('outageDisclosureV1', () => {
  const cohort13 = {
    outcomeCount: 4,
    supplyAtoms: ['500000000', '500000000', '500000000', '500000000'],
  };
  const founder = 'FBYW95Fo';
  const stranger = 'BVBriJDj';

  it('names the founder when the founder holds the whole failure column', () => {
    // Cohort-13's measured table, 2026-09-02, before anything moved.
    const disclosure = outageDisclosureV1({
      ...cohort13,
      positions: [
        { owner: founder, balances: ['499999800', '500000000', '500000000', '500000000'] },
        { owner: stranger, balances: ['200', '0', '0', '0'] },
        { owner: 'H1cYAJL3', balances: ['0', '0', '0', '0'] },
      ],
    });
    expect(disclosure).not.toBeNull();
    expect(disclosure!.failureOutcome).toBe(3);
    expect(disclosure!.complete).toBe(true);
    expect(disclosure!.holders).toHaveLength(1);
    expect(disclosure!.holders[0]!.owner).toBe(founder);
    expect(disclosure!.holders[0]!.wholeColumn).toBe(true);
    expect(disclosure!.payee).toContain(founder);
    expect(disclosure!.payee).toContain('every one of the 500000000 atoms');
    // The stranger who bought a real outcome is named nowhere in the payee,
    // which is the whole point of showing it before somebody trades.
    expect(disclosure!.payee).not.toContain(stranger);
  });

  it('reports what it could not see rather than presenting a partial read as the answer', () => {
    const disclosure = outageDisclosureV1({
      ...cohort13,
      positions: [{ owner: founder, balances: ['499999800', '500000000', '500000000', '400000000'] }],
    });
    expect(disclosure!.complete).toBe(false);
    expect(disclosure!.accountedAtoms).toBe('400000000');
    expect(disclosure!.unaccountedAtoms).toBe('100000000');
    expect(disclosure!.payee).toContain('partial answer');
    expect(disclosure!.payee).toContain('100000000');
  });

  it('splits the column between holders in proportion, and says so', () => {
    const disclosure = outageDisclosureV1({
      ...cohort13,
      positions: [
        { owner: founder, balances: ['0', '0', '0', '250000000'] },
        { owner: stranger, balances: ['0', '0', '0', '250000000'] },
      ],
    });
    expect(disclosure!.complete).toBe(true);
    expect(disclosure!.holders).toHaveLength(2);
    expect(disclosure!.holders.every((holder) => !holder.wholeColumn)).toBe(true);
    expect(disclosure!.payee).toContain('2 holders split');
  });

  it('says an outage pays nobody when nothing is issued on the failure outcome', () => {
    const disclosure = outageDisclosureV1({
      outcomeCount: 4,
      supplyAtoms: ['500000000', '500000000', '500000000', '0'],
      positions: [{ owner: founder, balances: ['500000000', '500000000', '500000000', '0'] }],
    });
    expect(disclosure!.payee).toContain('pays nobody');
  });

  // Pinned in `programs/dclutch-claims-sbf/src/lib.rs`, test
  // `the_failure_escrow_owner_is_the_address_the_market_page_derives`, from the
  // same market and Claims program. The browser is a hand-mirror of this seed
  // domain with no generated module joining it to the program, so ONE literal
  // asserted on both sides is the join. A page deriving a different escrow
  // would tell a buyer an outage refunds them when it does not.
  const witnessMarket = 'US517G5965aydkZ46HS38QLi7UQiSojurfbQfKCELFx';
  const witnessClaims = 'cGfHiC6Kgg3FpFZvgwGcswsCRtp4aBP2fzuXRQPizuN';
  const witnessEscrow = 'AGEyQ6gMncbX3PymFaas3CjZUNWfjLYbGfdq5Mwpcm3';

  it('derives the same failure escrow the Claims program authenticates', () => {
    expect(failureEscrowOwnerV1(witnessClaims, witnessMarket, 3)).toBe(witnessEscrow);
    // A different selector is a different escrow, so the seed really carries it.
    expect(failureEscrowOwnerV1(witnessClaims, witnessMarket, 2)).not.toBe(witnessEscrow);
  });

  it('says HOLDERS ARE REFUNDED when the failure column is seated in the escrow', () => {
    // Cohort-13's numbers with the ruling applied: the founder holds the three
    // ordinary columns short the 200 atoms the crossing sold, and the failure
    // column is seated where nobody can be paid for it.
    const disclosure = outageDisclosureV1({
      ...cohort13,
      failureEscrowOwner: witnessEscrow,
      positions: [
        { owner: founder, balances: ['499999800', '500000000', '500000000', '0'] },
        { owner: stranger, balances: ['200', '0', '0', '0'] },
        { owner: witnessEscrow, balances: ['0', '0', '0', '500000000'] },
      ],
    });
    expect(disclosure!.refunds).toBe(true);
    expect(disclosure!.escrowAtoms).toBe('500000000');
    expect(disclosure!.headline).toContain('HOLDERS ARE REFUNDED');
    expect(disclosure!.payee).toContain(witnessEscrow);
    // The founder is named nowhere as a payee, which is the whole ruling.
    expect(disclosure!.payee).not.toContain(founder);
  });

  it('refuses to claim a refund when the escrow holds only part of the column', () => {
    const disclosure = outageDisclosureV1({
      ...cohort13,
      failureEscrowOwner: witnessEscrow,
      positions: [
        { owner: witnessEscrow, balances: ['0', '0', '0', '300000000'] },
        { owner: founder, balances: ['0', '0', '0', '200000000'] },
      ],
    });
    expect(disclosure!.refunds).toBe(false);
    expect(disclosure!.escrowAtoms).toBe('300000000');
    expect(disclosure!.payee).toContain('A partly seated escrow refunds nobody.');
    expect(disclosure!.headline).not.toContain('HOLDERS ARE REFUNDED');
  });

  it('keeps saying the founder is paid on a market whose column was never seated', () => {
    // The same page, the same derivation, on cohort-13 as it actually stands.
    // A disclosure that read the escrow into existence would be worse than none.
    const disclosure = outageDisclosureV1({
      ...cohort13,
      failureEscrowOwner: witnessEscrow,
      positions: [{ owner: founder, balances: ['499999800', '500000000', '500000000', '500000000'] }],
    });
    expect(disclosure!.refunds).toBe(false);
    expect(disclosure!.escrowAtoms).toBe('0');
    expect(disclosure!.payee).toContain(founder);
    expect(disclosure!.payee).toContain('every one of the 500000000 atoms');
  });

  it('refuses rather than guessing when the read does not line up', () => {
    expect(outageDisclosureV1({ outcomeCount: 4, supplyAtoms: ['1', '1', '1'], positions: [] })).toBeNull();
    expect(outageDisclosureV1({ outcomeCount: 1, supplyAtoms: ['1'], positions: [] })).toBeNull();
    expect(outageDisclosureV1({ outcomeCount: 4, supplyAtoms: ['1', '1', '1', 'x'], positions: [] })).toBeNull();
    // A Position of another width is not this market's, so it contributes
    // nothing rather than being read at the wrong coordinate.
    const narrow = outageDisclosureV1({
      ...cohort13,
      positions: [{ owner: founder, balances: ['1', '2', '3'] }],
    });
    expect(narrow!.holders).toHaveLength(0);
    expect(narrow!.payee).toContain('cannot say who an outage would pay');
  });
});
