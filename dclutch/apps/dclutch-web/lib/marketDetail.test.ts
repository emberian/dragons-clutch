import { PublicKey } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import fixture from '../fixtures/canonical-accounts.json';
import {
  capabilityProvenanceV1,
  inspectMarketDetailV1,
  marketPhaseMeaningV1,
  realmProvenanceV1,
  requiredBackingMeaningV1,
} from './marketDetail';
import { provenanceChipV1 } from './marketDiscovery';
import { type RpcAccount, type SolanaRpcClient } from './rpc';

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
const OUTCOMES = market.data[10];
const SETTLEMENT_OFFSET = 256 + OUTCOMES * 8;

/**
 * Market variants are produced by writing exact fields of the canonical Rust
 * fixture, never by inventing account bytes. Phase, Hoard, supply, and
 * settlement all live above the 32..200 identity window the Market PDA hashes,
 * so every derived address in the fixture stays correct.
 */
function marketVariant(mutate: (data: Uint8Array, view: DataView) => void): Uint8Array {
  const data = new Uint8Array(market.data);
  mutate(data, new DataView(data.buffer));
  return data;
}

function resolvedMarket(winner: number, supply: ReadonlyArray<bigint>, hoard: bigint): Uint8Array {
  return marketVariant((data, view) => {
    data[200] = 2;
    view.setBigUint64(248, hoard, true);
    supply.forEach((amount, index) => view.setBigUint64(256 + index * 8, amount, true));
    data[SETTLEMENT_OFFSET] = 1;
    data[SETTLEMENT_OFFSET + 1] = 0;
    data[SETTLEMENT_OFFSET + 2] = winner;
    view.setBigUint64(SETTLEMENT_OFFSET + 8, BigInt(41), true);
    data.fill(0x77, SETTLEMENT_OFFSET + 16, SETTLEMENT_OFFSET + 48);
  });
}

function coreAccount(data: Uint8Array, owner = fixture.programId): RpcAccount {
  return Object.freeze({ data, executable: false, lamports: '4242', owner, space: data.length });
}

function client(accounts: ReadonlyMap<string, RpcAccount>, slot = '4711'): SolanaRpcClient {
  return {
    finalizedSlot: async () => slot,
    multipleAccounts: async (addresses: ReadonlyArray<string>) => Object.freeze({
      slot,
      accounts: Object.freeze(addresses.map((address) => Object.freeze({ address, account: accounts.get(address) ?? null }))),
    }),
  } as unknown as SolanaRpcClient;
}

function chain(marketData: Uint8Array): Map<string, RpcAccount> {
  return new Map<string, RpcAccount>([
    [market.address, coreAccount(marketData)],
    [realm.address, coreAccount(realm.data)],
  ]);
}

describe('Market detail projection', () => {
  it('carries the immutable identities, the artifact profile, and an honest phase meaning', async () => {
    const detail = await inspectMarketDetailV1(client(chain(market.data)), {
      coreProgramId: fixture.programId,
      address: market.address,
    });
    expect(detail.floorSlot).toBe('4711');
    const card = detail.card;
    if (card.status !== 'decoded') throw new Error(card.refusal);
    expect(card.phase).toBe('Founding');
    expect(detail.phaseMeaning).toBe(marketPhaseMeaningV1('Founding'));
    expect(detail.phaseMeaning).toMatch(/No claim has been issued/);
    expect(card.identity).toMatchObject({
      schemaMagic: 'DCLTCAT1',
      schemaVersion: 1,
      categoricalProfile: 1,
      accountBytes: market.data.length,
      realmId: '3bda98b500c0de22309e1023ba42cc6cd5904eb9e09acfd0e94d04672bb15ba5',
      productInstanceId: '02'.repeat(32),
      claimBasisId: '03'.repeat(32),
      resolutionPolicyId: '04'.repeat(32),
      capabilityManifestId: '05'.repeat(32),
    });
    expect(card.identity.rentRefundAuthority).toBe(new PublicKey(new Uint8Array(32).fill(0x08)).toBase58());
    expect(provenanceChipV1(card.provenance)).toBe('CHAIN · finalized slot 4711');
  });

  it('states the exact required backing and the basis it is measured against', async () => {
    const founding = await inspectMarketDetailV1(client(chain(market.data)), {
      coreProgramId: fixture.programId,
      address: market.address,
    });
    const empty = founding.card;
    if (empty.status !== 'decoded') throw new Error(empty.refusal);
    expect(empty.hoardAtoms).toBe('0');
    expect(empty.requiredBackingAtoms).toBe('0');
    expect(empty.requiredBackingBasis).toBe('maximum-outcome-supply');
    expect(requiredBackingMeaningV1('maximum-outcome-supply')).toMatch(/largest outcome supply/);

    // Unresolved: the Hoard must cover the largest outcome, because any
    // outcome could still be the one that pays.
    const open = await inspectMarketDetailV1(client(chain(marketVariant((data, view) => {
      data[200] = 1;
      view.setBigUint64(248, BigInt(90), true);
      [BigInt(40), BigInt(90), BigInt(7)].forEach((amount, index) => view.setBigUint64(256 + index * 8, amount, true));
    }))), { coreProgramId: fixture.programId, address: market.address });
    const openCard = open.card;
    if (openCard.status !== 'decoded') throw new Error(openCard.refusal);
    expect(openCard.phase).toBe('Open');
    expect(openCard.supplyAtoms).toEqual(['40', '90', '7']);
    expect(openCard.requiredBackingAtoms).toBe('90');
    expect(openCard.requiredBackingBasis).toBe('maximum-outcome-supply');

    // Resolved: only the winning supply can still be paid.
    const resolved = await inspectMarketDetailV1(client(chain(resolvedMarket(2, [BigInt(40), BigInt(90), BigInt(7)], BigInt(90)))), {
      coreProgramId: fixture.programId,
      address: market.address,
    });
    const resolvedCard = resolved.card;
    if (resolvedCard.status !== 'decoded') throw new Error(resolvedCard.refusal);
    expect(resolvedCard.phase).toBe('Resolved');
    expect(resolvedCard.requiredBackingAtoms).toBe('7');
    expect(resolvedCard.requiredBackingBasis).toBe('winning-outcome-supply');
    expect(requiredBackingMeaningV1('winning-outcome-supply')).toMatch(/winning outcome supply/);
    if (resolvedCard.settlement.status !== 'resolved') throw new Error('expected terminal settlement');
    expect(resolvedCard.settlement).toMatchObject({ route: 'Occurrence', winner: 2, terminalSequence: '41' });
    expect(resolved.phaseMeaning).toMatch(/one collateral atom per winning claim atom/);
  });

  it('reports the Realm exactly as the Realm account decodes, never as Market hearsay', async () => {
    const detail = await inspectMarketDetailV1(client(chain(market.data)), {
      coreProgramId: fixture.programId,
      address: market.address,
    });
    const card = detail.card;
    if (card.status !== 'decoded') throw new Error(card.refusal);
    if (card.collateral.status !== 'bound') throw new Error(card.collateral.reason);
    expect(card.collateral.realmAddress).toBe(realm.address);
    expect(card.collateral.tokenProgram).toBe(new PublicKey(new Uint8Array(32).fill(0x0b)).toBase58());
    expect(card.collateral.collateralMint).toBe(new PublicKey(new Uint8Array(32).fill(0x0c)).toBase58());
    expect(card.collateral.adapterReleaseId).toBe('0d'.repeat(32));
    expect(card.collateral.mintAuthorityPolicy).toBe('Require absent');
    expect(card.collateral.freezeAuthorityPolicy).toBe('Admit issuer control');
    expect(realmProvenanceV1(card.collateral)).toEqual({ kind: 'chain', observedSlot: '4711' });
    expect(provenanceChipV1(detail.realmProvenance)).toBe('CHAIN · finalized slot 4711');
  });

  it('refuses every section a Registry-less read cannot authenticate, and says why', async () => {
    const detail = await inspectMarketDetailV1(client(chain(market.data)), {
      coreProgramId: fixture.programId,
      address: market.address,
    });
    const card = detail.card;
    if (card.status !== 'decoded') throw new Error(card.refusal);
    expect(card.capabilities.status).toBe('unread');
    expect(detail.capabilityProvenance.kind).toBe('refused');
    expect(provenanceChipV1(detail.capabilityProvenance)).toBe('REFUSED');
    expect(capabilityProvenanceV1(card.capabilities)).toEqual(detail.capabilityProvenance);
    if (detail.capabilityProvenance.kind !== 'refused') throw new Error('expected a refusal');
    expect(detail.capabilityProvenance.reason).toMatch(/No capability may be asserted from the Market root alone/);

    // A Realm that is not present at the floor is a refusal with its reason,
    // not a section that quietly renders blank.
    const unbound = await inspectMarketDetailV1(client(new Map([[market.address, coreAccount(market.data)]])), {
      coreProgramId: fixture.programId,
      address: market.address,
    });
    const unboundCard = unbound.card;
    if (unboundCard.status !== 'decoded') throw new Error(unboundCard.refusal);
    expect(unboundCard.collateral.status).toBe('refused');
    expect(unbound.realmProvenance.kind).toBe('refused');
    if (unbound.realmProvenance.kind !== 'refused') throw new Error('expected a refusal');
    expect(unbound.realmProvenance.reason).toMatch(/did not decode at this finalized floor/);
  });

  it('refuses the whole detail when the Market itself is absent or foreign', async () => {
    const absent = await inspectMarketDetailV1(client(new Map()), {
      coreProgramId: fixture.programId,
      address: market.address,
    });
    expect(absent.card.status).toBe('refused');
    expect(absent.phaseMeaning).toBeNull();
    expect(absent.realmProvenance.kind).toBe('refused');
    expect(absent.capabilityProvenance.kind).toBe('refused');
    expect(absent.reason).toMatch(/absent at the finalized observation floor/);

    const foreign = await inspectMarketDetailV1(client(new Map([[market.address, coreAccount(market.data, SYSTEM_PROGRAM)]])), {
      coreProgramId: fixture.programId,
      address: market.address,
    });
    const foreignCard = foreign.card;
    if (foreignCard.status !== 'refused') throw new Error('expected a refusal');
    expect(foreignCard.refusal).toMatch(/owner differs from the selected program ID/);
  });
});
