import { PublicKey } from '@solana/web3.js';
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
  LIABILITY_BASIS_STATE_VERSION_V2,
  REALM_SCHEMA_RELEASE_ID_V1,
} from './generated/coreFound';
import {
  POSITION_ADMISSION_POSITION_LAMPORTS_OFFSET_V2,
  POSITION_ADMISSION_POSITION_RENT_OFFSET_V2,
  PROTOCOL_POSITION_ADMISSION_BYTES_V2,
  PROTOCOL_POSITION_ADMISSION_MAGIC_V2,
  PROTOCOL_POSITION_WIRE_VERSION_V2,
} from './generated/directParticipantV1';
import {
  capabilityProvenanceV1,
  failureEscrowAccountsV1,
  failureEscrowOwnerV1,
  failureEscrowV1,
  founderBondV1,
  inspectMarketDetailV1,
  refundsOnFailureFromEscrowV1,
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

  // Cohort-16.1's REAL devnet coordinates, the same pin the host's
  // `crates/dclutch-operator/src/failure_escrow_v1.rs` carries: a live 160-byte
  // DCLLBP02 Position holding [0, 0, 0, 166666667]. A derivation that drifts
  // stops reproducing a real address rather than stopping agreeing with itself.
  const cohort161 = Object.freeze({
    claims: '8JfHfBBGaoUP1yV6VzXcvWwhQSZNV8eQmDAiYmCpNQJk',
    market: '3xoSXBVsAXENB1RPq4sqS8euCksT1qsnnz83eWQPEtgY',
    aggregate: 'CBzv1hhtToxpCaExaA7QqES4bMu5UjxiAiBW9bMUrCdg',
    owner: 'Hq6sF5pv3i8CBkH46dsyN9fnzJi1jooS2gj6USCQmke3',
    position: '7FQCfc4RrrsATEe969eNVYoLjDukmBVKMAxM1yg7AzcQ',
    admission: '4WUZ2qZKz7nkgGnnNejP8cLNHhjKCFCpHwNVDikE3T9b',
  });

  it('derives the whole escrow the host derives, pinned to a chain', () => {
    const escrow = failureEscrowV1(cohort161.claims, cohort161.market, cohort161.aggregate, 4);
    expect(escrow.failureSelector).toBe(3);
    expect(escrow.owner).toBe(cohort161.owner);
    expect(escrow.position).toBe(cohort161.position);
    expect(escrow.admission).toBe(cohort161.admission);
    // A width that seats no escrow is not an escrow this function invents.
    expect(() => failureEscrowV1(cohort161.claims, cohort161.market, cohort161.aggregate, 1)).toThrow();
  });

  /**
   * A DCLLBP02 Position at the generated offsets: magic 0, state version u16
   * @8, claim_count u32 @12, aggregate @24, owner @56, balances after the
   * 128-byte header.
   *
   * The version is the GENERATED constant. Written as a literal it would be a
   * second author for the number `header()` refuses on, and left out entirely
   * it reads 0 -- which is how this fixture spent its whole life being decoded
   * as `state version 0 is unsupported`, caught, and returned as the absent
   * seating, so every `false` this suite asserted was true for the wrong
   * reason and the two `true`s could never have passed.
   */
  function positionBytes(aggregate: string, owner: string, balances: ReadonlyArray<bigint>): Uint8Array {
    const bytes = new Uint8Array(128 + balances.length * 8);
    bytes.set(new TextEncoder().encode('DCLLBP02'), 0);
    const view = new DataView(bytes.buffer);
    view.setUint16(8, LIABILITY_BASIS_STATE_VERSION_V2, true);
    view.setUint32(12, balances.length, true);
    view.setBigUint64(16, 1n, true);
    bytes.set(new PublicKey(aggregate).toBytes(), 24);
    bytes.set(new PublicKey(owner).toBytes(), 56);
    balances.forEach((atoms, index) => view.setBigUint64(128 + index * 8, atoms, true));
    return bytes;
  }

  it('reads refundsOnFailure off the escrow Position, and only off a seated one', () => {
    const escrow = failureEscrowV1(cohort161.claims, cohort161.market, cohort161.aggregate, 4);
    const supply = ['166666667', '166666667', '166666667', '166666667'];
    const seated = refundsOnFailureFromEscrowV1({
      escrow,
      claimsProgramId: cohort161.claims,
      outcomeCount: 4,
      supplyAtoms: supply,
      account: { owner: cohort161.claims, executable: false, data: positionBytes(cohort161.aggregate, cohort161.owner, [0n, 0n, 0n, 166666667n]) },
    });
    expect(seated).toEqual({ present: true, seated: true, heldAtoms: '166666667', refundsOnFailure: true });
    // An empty address is a categorical founding: nothing seated, no refund.
    expect(refundsOnFailureFromEscrowV1({ escrow, claimsProgramId: cohort161.claims, outcomeCount: 4, supplyAtoms: supply, account: null }))
      .toEqual({ present: false, seated: false, heldAtoms: '0', refundsOnFailure: false });
    // A tradeable claim beside the residue is not the seated shape.
    const beside = refundsOnFailureFromEscrowV1({
      escrow,
      claimsProgramId: cohort161.claims,
      outcomeCount: 4,
      supplyAtoms: supply,
      account: { owner: cohort161.claims, executable: false, data: positionBytes(cohort161.aggregate, cohort161.owner, [0n, 5n, 0n, 166666667n]) },
    });
    expect(beside.present).toBe(true);
    expect(beside.seated).toBe(false);
    expect(beside.refundsOnFailure).toBe(false);
    // Nothing issued on the failure outcome seats nothing.
    const nothing = refundsOnFailureFromEscrowV1({
      escrow,
      claimsProgramId: cohort161.claims,
      outcomeCount: 4,
      supplyAtoms: ['1', '1', '1', '0'],
      account: { owner: cohort161.claims, executable: false, data: positionBytes(cohort161.aggregate, cohort161.owner, [0n, 0n, 0n, 0n]) },
    });
    expect(nothing.seated).toBe(false);
  });

  /**
   * THE JOIN THE MARKET PAGE MAKES, end to end, and in ONE DIRECTION.
   *
   * `MarketDetailWorkspace` read the escrow Position for its lamports and threw
   * the bytes away, so it passed `founderBondV1` a hard-coded `null` and the
   * bond's exit sentence was unreachable on every market. It derives the
   * seating from the same read now, and this is that composition: no new RPC,
   * no address the page did not derive.
   *
   * The asymmetry is the point and is asserted both ways. A SEATED escrow is
   * evidence -- only a refunding founding seats the whole failure column -- so
   * the exit is named. An UNSEATED one is NOT evidence of a categorical market,
   * so the page passes the unread `null` and the exit stays unnamed; passing
   * `false` there would print `This market posted no founder bond`, which is
   * the sentence `outageDisclosureV1` exists to prevent.
   */
  it('lets a seated escrow name the founder bond exit, and leaves an unseated one unread', () => {
    const escrow = failureEscrowV1(cohort161.claims, cohort161.market, cohort161.aggregate, 4);
    const supply = ['166666667', '166666667', '166666667', '166666667'];
    const rent = 1_823_904n;
    const heldLamports = rent + 4_031_465n;
    const admissionBytes = new Uint8Array(PROTOCOL_POSITION_ADMISSION_BYTES_V2);
    admissionBytes.set(PROTOCOL_POSITION_ADMISSION_MAGIC_V2, 0);
    const view = new DataView(admissionBytes.buffer);
    view.setUint16(PROTOCOL_POSITION_ADMISSION_MAGIC_V2.length, PROTOCOL_POSITION_WIRE_VERSION_V2, true);
    view.setBigUint64(POSITION_ADMISSION_POSITION_RENT_OFFSET_V2, rent, true);
    view.setBigUint64(POSITION_ADMISSION_POSITION_LAMPORTS_OFFSET_V2, heldLamports, true);

    // Exactly the page's expression, with the account as its only variable.
    const bondFrom = (account: Readonly<{ owner: string; executable: boolean; data: Uint8Array }> | null) => {
      const seating = refundsOnFailureFromEscrowV1({
        escrow, claimsProgramId: cohort161.claims, outcomeCount: 4, supplyAtoms: supply, account,
      });
      return founderBondV1({
        escrowPositionLamports: heldLamports.toString(),
        escrowAdmissionBytes: admissionBytes,
        refundsOnFailure: seating.seated ? true : null,
        phase: 'Terminal',
        terminalWinner: 1,
        failureOutcome: 3,
      });
    };

    const seatedAccount = {
      owner: cohort161.claims,
      executable: false,
      data: positionBytes(cohort161.aggregate, cohort161.owner, [0n, 0n, 0n, 166666667n]),
    };
    const named = bondFrom(seatedAccount);
    expect(named!.bondLamports).toBe('4031465');
    expect(named!.exit).toBe('honest');
    expect(named!.sentence).toContain('goes back to the founder’s refund wallet');

    // An empty escrow address: the bond amount still stands, the direction does not.
    const unread = bondFrom(null);
    expect(unread!.bondLamports).toBe('4031465');
    expect(unread!.exit).toBeNull();
    expect(unread!.sentence).not.toContain('posted no founder bond');
  });

  it('says HOLDERS ARE REFUNDED off the payout scale, not off the seating', () => {
    // Cohort-13's numbers with the ruling applied end to end: a refunding
    // record AND the failure column seated where nobody can be paid for it.
    const disclosure = outageDisclosureV1({
      ...cohort13,
      refundsOnFailure: true,
      failureEscrowOwner: witnessEscrow,
      positions: [
        { owner: founder, balances: ['499999800', '500000000', '500000000', '0'] },
        { owner: stranger, balances: ['200', '0', '0', '0'] },
        { owner: witnessEscrow, balances: ['0', '0', '0', '500000000'] },
      ],
    });
    expect(disclosure!.refundsOnFailure).toBe(true);
    expect(disclosure!.escrowSeated).toBe(true);
    expect(disclosure!.escrowAtoms).toBe('500000000');
    expect(disclosure!.headline).toContain('HOLDERS ARE REFUNDED');
    expect(disclosure!.columnNote).toContain(witnessEscrow);
    // The founder is named nowhere as a payee, which is the whole ruling.
    expect(disclosure!.payee).not.toContain(founder);
  });

  /**
   * THE CASE THAT MAKES THE SCALE LOAD-BEARING, and it is the one cohort-16
   * actually founds: a refunding record whose failure column is still seated
   * with the founder, because the founding that seats the escrow is not built.
   * A disclosure keyed on the seating would say the founder takes all of it,
   * which is the OPPOSITE of what the payout vector does.
   */
  it('refunds on a refunding record even while the founder still holds the column', () => {
    const disclosure = outageDisclosureV1({
      ...cohort13,
      refundsOnFailure: true,
      failureEscrowOwner: witnessEscrow,
      positions: [{ owner: founder, balances: ['499999800', '500000000', '500000000', '500000000'] }],
    });
    expect(disclosure!.escrowSeated).toBe(false);
    expect(disclosure!.headline).toContain('HOLDERS ARE REFUNDED');
    expect(disclosure!.payee).toContain('founded to REFUND');
    expect(disclosure!.payee).not.toContain(founder);
    // And the seating is still reported, as the separate fact it is: those
    // claims are worth nothing and are still in somebody's hands to sell.
    expect(disclosure!.columnNote).toContain('None of the 500000000 atoms');
  });

  it('names the holder on a LEGACY record, whatever the escrow holds', () => {
    const disclosure = outageDisclosureV1({
      ...cohort13,
      refundsOnFailure: false,
      failureEscrowOwner: witnessEscrow,
      positions: [{ owner: founder, balances: ['499999800', '500000000', '500000000', '500000000'] }],
    });
    expect(disclosure!.refundsOnFailure).toBe(false);
    expect(disclosure!.payee).toContain(founder);
    expect(disclosure!.payee).toContain('every one of the 500000000 atoms');
    expect(disclosure!.payee).not.toContain('payout scale');
  });

  it('says what it did not read when the payout scale was not read', () => {
    const disclosure = outageDisclosureV1({
      ...cohort13,
      failureEscrowOwner: witnessEscrow,
      positions: [{ owner: founder, balances: ['499999800', '500000000', '500000000', '500000000'] }],
    });
    expect(disclosure!.refundsOnFailure).toBeNull();
    expect(disclosure!.payee).toContain(founder);
    expect(disclosure!.payee).toContain('has not read this market\u2019s payout scale');
    // AND THE HEADLINE MUST NOT ANSWER WHAT THE PAYEE SENTENCE JUST SAID IT
    // COULD NOT. Caught on the live cohort-16 market page: `payee` said the
    // scale had not been read and the headline directly above it asserted the
    // legacy outcome as fact -- on a market founded to refund. The unread
    // headline names both answers and commits to neither.
    expect(disclosure!.headline).toContain('has not read it');
    expect(disclosure!.headline).toContain('legacy scale');
    expect(disclosure!.headline).toContain('refunding scale');
    const decided = outageDisclosureV1({
      ...cohort13,
      failureEscrowOwner: witnessEscrow,
      positions: [{ owner: founder, balances: ['499999800', '500000000', '500000000', '500000000'] }],
      refundsOnFailure: false,
    });
    expect(decided!.headline).toContain('the whole collateral is paid to whoever holds that claim');
    expect(decided!.headline).not.toContain('has not read it');
  });

  it('reports a partly seated escrow as the partial fact it is', () => {
    const disclosure = outageDisclosureV1({
      ...cohort13,
      refundsOnFailure: true,
      failureEscrowOwner: witnessEscrow,
      positions: [
        { owner: witnessEscrow, balances: ['0', '0', '0', '300000000'] },
        { owner: founder, balances: ['0', '0', '0', '200000000'] },
      ],
    });
    expect(disclosure!.escrowSeated).toBe(false);
    expect(disclosure!.escrowAtoms).toBe('300000000');
    expect(disclosure!.columnNote).toContain('a partly seated escrow');
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

/**
 * THE FOUNDER BOND, and the numbers are cohort-15's own.
 *
 * Devnet's rent rate was 6,333 lamports a byte at that founding; a four-outcome
 * Claims Position is 160 bytes (a 128-byte header plus 8 an outcome) and its
 * exempt minimum is 1,823,904; the size rule prices the bond at 4,031,465. So a
 * seated escrow reads 5,855,369 lamports with 1,823,904 recorded as its rent,
 * and the difference is the whole disclosure. Every figure here is the same one
 * `founder_bond_v1::tests` decides in Rust, to the lamport.
 */
describe('founderBondV1', () => {
  const rent = 1_823_904n;
  const bond = 4_031_465n;
  const seated = rent + bond;

  function admission(
    recordedRent: bigint,
    atFounding: bigint,
    mutate?: (bytes: Uint8Array) => void,
  ): Uint8Array {
    const bytes = new Uint8Array(PROTOCOL_POSITION_ADMISSION_BYTES_V2);
    bytes.set(PROTOCOL_POSITION_ADMISSION_MAGIC_V2, 0);
    const view = new DataView(bytes.buffer);
    view.setUint16(PROTOCOL_POSITION_ADMISSION_MAGIC_V2.length, PROTOCOL_POSITION_WIRE_VERSION_V2, true);
    view.setBigUint64(POSITION_ADMISSION_POSITION_RENT_OFFSET_V2, recordedRent, true);
    view.setBigUint64(POSITION_ADMISSION_POSITION_LAMPORTS_OFFSET_V2, atFounding, true);
    mutate?.(bytes);
    return bytes;
  }

  const refunding = {
    escrowPositionLamports: seated.toString(),
    escrowAdmissionBytes: admission(rent, seated),
    refundsOnFailure: true,
    phase: 'Open' as const,
    terminalWinner: null,
    failureOutcome: 3,
  };

  /**
   * The join to the kernel, asserted rather than trusted. These three are typed
   * in `crates/dclutch-claims/src/protocol_position_v2.rs` -- line 76
   * `EVIDENCE_POSITION_RENT_OFFSET = 448`, line 81
   * `EVIDENCE_POSITION_LAMPORTS_OFFSET = 488`, line 21
   * `PROTOCOL_POSITION_ADMISSION_MAGIC_V2 = *b"DCLPPS02"` -- and reading the
   * bond one field off would report a founder's stake as somebody's rent.
   */
  it('reads the escrow admission where the kernel writes it', () => {
    expect(POSITION_ADMISSION_POSITION_RENT_OFFSET_V2).toBe(448);
    expect(POSITION_ADMISSION_POSITION_LAMPORTS_OFFSET_V2).toBe(488);
    expect(PROTOCOL_POSITION_ADMISSION_BYTES_V2).toBe(512);
    expect(new TextDecoder('ascii').decode(PROTOCOL_POSITION_ADMISSION_MAGIC_V2)).toBe('DCLPPS02');
  });

  it('reads cohort-15’s bond as the escrow’s balance above its recorded rent', () => {
    const posted = founderBondV1(refunding);
    expect(posted).not.toBeNull();
    expect(posted!.bondLamports).toBe('4031465');
    expect(posted!.recordedRentLamports).toBe('1823904');
    expect(posted!.postedAtFoundingLamports).toBe('5855369');
    expect(posted!.exit).toBeNull();
    expect(posted!.sentence).toContain('The founder posted a bond of 4031465 lamports (0.004031465 SOL)');
    expect(posted!.sentence).toContain('paid pro rata to the holders of ordinary claims if the feed goes quiet');
  });

  it('names the honest exit when an ordinary outcome won', () => {
    const honest = founderBondV1({ ...refunding, phase: 'Terminal', terminalWinner: 1 });
    expect(honest!.exit).toBe('honest');
    expect(honest!.bondLamports).toBe('4031465');
    expect(honest!.sentence).toContain('The data source reported');
    expect(honest!.sentence).toContain('goes back to the founder’s refund wallet');
    // And once the market is retired it has already happened, so it is not
    // still promised in the future tense.
    const retired = founderBondV1({ ...refunding, phase: 'Retired', terminalWinner: 1 });
    expect(retired!.exit).toBe('honest');
    expect(retired!.sentence).toContain('went back to the founder’s refund wallet');
  });

  it('names the exhausted exit and says how each redemption draws its share', () => {
    const exhausted = founderBondV1({ ...refunding, phase: 'Terminal', terminalWinner: 3 });
    expect(exhausted!.exit).toBe('exhausted');
    expect(exhausted!.sentence).toContain('The data source never reported');
    expect(exhausted!.sentence).toContain('4031465 lamports (0.004031465 SOL) go pro rata to the holders of ordinary claims');
    expect(exhausted!.sentence).toContain('the last one drawing the rest');
    // Nobody has redeemed yet, so nothing has been drawn and the page does not
    // invent a walk that has not started.
    expect(exhausted!.sentence).not.toContain('still in the escrow');
  });

  it('reports a partly drawn bond as the difference it is', () => {
    const midWalk = founderBondV1({
      ...refunding,
      escrowPositionLamports: (rent + 1_000_000n).toString(),
      phase: 'Terminal',
      terminalWinner: 3,
    });
    expect(midWalk!.bondLamports).toBe('1000000');
    expect(midWalk!.postedAtFoundingLamports).toBe('5855369');
    expect(midWalk!.sentence).toContain('Of the 4031465 lamports posted at founding, 1000000 are still in the escrow.');
  });

  it('says a categorical market posted no bond, without reading an account', () => {
    const none = founderBondV1({
      escrowPositionLamports: null,
      escrowAdmissionBytes: null,
      refundsOnFailure: false,
      phase: 'Open',
      terminalWinner: null,
      failureOutcome: 3,
    });
    expect(none!.bondLamports).toBe('0');
    expect(none!.exit).toBeNull();
    expect(none!.sentence).toBe('This market posted no founder bond: it was founded before the bond, and an outage pays whoever holds the failure column.');
  });

  /**
   * UNREAD IS NOT ZERO, and the type says so: `'0'` is the read fact that no
   * bond stands here and `null` is the refusal to assert either way. Collapsing
   * them would let one unreachable account read, on the page, as a founder who
   * staked nothing on the oracle they chose -- which is a claim about a person
   * and would be made from no evidence at all.
   */
  it('refuses rather than calling an unread escrow a founder who staked nothing', () => {
    const wrongMagic = founderBondV1({
      ...refunding,
      escrowAdmissionBytes: admission(rent, seated, (bytes) => { bytes[0] = 0x00; }),
    });
    expect(wrongMagic!.bondLamports).toBeNull();
    expect(wrongMagic!.recordedRentLamports).toBeNull();
    expect(wrongMagic!.exit).toBeNull();
    expect(wrongMagic!.sentence).toContain('This page has not read this market’s failure escrow admission');
    expect(wrongMagic!.sentence).not.toContain('no founder bond');

    expect(founderBondV1({ ...refunding, escrowAdmissionBytes: null })!.bondLamports).toBeNull();
    expect(founderBondV1({
      ...refunding,
      escrowAdmissionBytes: admission(rent, seated).slice(0, 511),
    })!.bondLamports).toBeNull();

    // The admission read and the balance did not: the two figures the founding
    // recorded are still stated, and only today's bond is withheld.
    const halfRead = founderBondV1({ ...refunding, escrowPositionLamports: null });
    expect(halfRead!.bondLamports).toBeNull();
    expect(halfRead!.recordedRentLamports).toBe('1823904');
    expect(halfRead!.sentence).toContain('not the escrow Position’s own balance');
  });

  it('says an escrow at its own rent posted nothing, which is a read fact', () => {
    const bare = founderBondV1({
      ...refunding,
      escrowPositionLamports: rent.toString(),
      escrowAdmissionBytes: admission(rent, rent),
    });
    expect(bare!.bondLamports).toBe('0');
    expect(bare!.recordedRentLamports).toBe('1823904');
    expect(bare!.sentence).toContain('no founder bond stands behind its oracle');
  });

  /**
   * The exit is the SCALE's answer, exactly as `payee` is. An unread scale still
   * gets the amount -- a subtraction of two numbers off one account, which no
   * scale changes -- and gets no direction, because only the direction depends
   * on it.
   */
  it('states the amount but never the exit when the payout scale was not read', () => {
    const unread = founderBondV1({ ...refunding, refundsOnFailure: null, phase: 'Terminal', terminalWinner: 3 });
    expect(unread!.bondLamports).toBe('4031465');
    expect(unread!.exit).toBeNull();
    expect(unread!.sentence).toContain('The founder posted a bond of 4031465 lamports');
    expect(unread!.sentence).not.toContain('never reported');
  });

  it('refuses a failure outcome no market could have', () => {
    expect(founderBondV1({ ...refunding, failureOutcome: 0 })).toBeNull();
    expect(founderBondV1({ ...refunding, failureOutcome: -1 })).toBeNull();
    expect(founderBondV1({ ...refunding, failureOutcome: 1.5 })).toBeNull();
  });

  it('carries the bond into the outage disclosure the page renders', () => {
    const founderBond = founderBondV1({ ...refunding, phase: 'Terminal', terminalWinner: 3 });
    const disclosure = outageDisclosureV1({
      outcomeCount: 4,
      supplyAtoms: ['500000000', '500000000', '500000000', '500000000'],
      refundsOnFailure: true,
      positions: [{ owner: 'FBYW95Fo', balances: ['500000000', '500000000', '500000000', '500000000'] }],
      founderBond,
    });
    expect(disclosure!.founderBond).toBe(founderBond);
    expect(disclosure!.headline).toContain('HOLDERS ARE REFUNDED');
    expect(disclosure!.headline).toContain('4031465 lamports');
    // Unread stays unread here too: a disclosure handed no bond says nothing
    // about one rather than reporting that none was posted.
    const without = outageDisclosureV1({
      outcomeCount: 4,
      supplyAtoms: ['500000000', '500000000', '500000000', '500000000'],
      refundsOnFailure: true,
      positions: [],
    });
    expect(without!.founderBond).toBeNull();
    expect(without!.headline).not.toContain('bond');
  });

  it('derives the escrow’s Position and admission from the market’s own aggregate', () => {
    const owner = failureEscrowOwnerV1(CLAIMS, LIVE.market.address, 3);
    const accounts = failureEscrowAccountsV1(CLAIMS, LIVE.claimsAggregate.address, owner);
    // Two different seed domains, so two different accounts -- reading the
    // Position's balance out of the admission's address would read nothing.
    expect(accounts.position).not.toBe(accounts.admission);
    expect(accounts.position).not.toBe(owner);
    // And the pair is a pure derivation: the same three addresses, always the
    // same two accounts.
    expect(failureEscrowAccountsV1(CLAIMS, LIVE.claimsAggregate.address, owner)).toEqual(accounts);
  });
});
