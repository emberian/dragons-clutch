import { readFileSync } from 'node:fs';

import { describe, expect, it } from 'vitest';

import {
  MARKET_HOARD_UNDERIVABLE_V1,
  decodeClaimsAggregateV2,
  decodeClaimsPositionV2,
  decodeMarketCoreStateV2,
  deriveClaimsAggregateAddressV2,
  deriveClaimsPositionAddressV2,
  deriveMarketCoreAddressV2,
} from './marketCoreV2';

/**
 * These bytes came off a chain, not out of a builder.
 *
 * `fixtures/live-open-market.json` holds the finalized account bytes of the
 * first locally OPEN dClutch Market, copied verbatim from a successor-campaign
 * validator with `tools/gauntlet/frontend/expect.mjs --fixture-out`. A decoder
 * checked only against buffers this repository built agrees with itself; these
 * cases can only pass if the decoder agrees with a real Core program.
 *
 * The values asserted below were independently decoded by
 * `tools/gauntlet/frontend/chain-witness.mjs`, which shares no code with `lib/`.
 */
const live = JSON.parse(readFileSync(new URL('../fixtures/live-open-market.json', import.meta.url), 'utf8')) as {
  programs: Readonly<Record<string, string>>;
  founder: string;
  accounts: Readonly<Record<string, { address: string; owner: string; dataHex: string } | null>>;
};

function accountBytes(value: string, field: string): Uint8Array {
  if (value.length % 2 !== 0 || !/^[0-9a-f]*$/.test(value)) throw new Error(`${field} is not whole lowercase hexadecimal bytes`);
  return Uint8Array.from(value.match(/../g) ?? [], (byte) => Number.parseInt(byte, 16));
}

const account = (name: string) => {
  const entry = live.accounts[name];
  if (entry === null || entry === undefined) throw new Error(`live fixture is missing ${name}`);
  return Object.freeze({ ...entry, data: accountBytes(entry.dataHex, `${name} bytes`) });
};

describe('the Market a live dClutch chain actually holds', () => {
  it('decodes the first locally Open Market from its finalized bytes', () => {
    const market = account('market');
    const state = decodeMarketCoreStateV2(market.address, market.data);
    expect(market.owner).toBe(live.programs.core);
    expect(state.accountBytes).toBe(352);
    expect(state.version).toBe(2);
    expect(state.phase).toBe('Open');
    expect(state.readiness).toBe('Consumed');
    expect(state.marketId).toBe(market.address);
    expect(state.identity.generation).toBe('2');
    expect(state.identity.registryProgram).toBe(live.programs.registry);
    expect(state.identity.realmId).toBe('632c1f76f09491f39b197d8f67f61a922dee326941d8e663834fc1ed917f7760');
    expect(state.identity.capabilityManifestId).toBe('7dc55519e971550586f4b69ef3386131791cdfc3e2a401667798f3234d450711');
    expect(state.outstandingCapabilities).toBe('0');
    expect(state.settlement).toEqual({ status: 'open', label: 'no terminal receipt' });
  });

  it('re-derives the Market address from the state account own identity seeds', () => {
    const market = account('market');
    expect(deriveMarketCoreAddressV2(live.programs.core, market.data)).toBe(market.address);
  });

  it('refuses the superseded categorical Market layout rather than guessing', () => {
    const categorical = new Uint8Array(352);
    categorical.set(new TextEncoder().encode('DCLTCAT1'));
    expect(() => decodeMarketCoreStateV2('11111111111111111111111111111111', categorical))
      .toThrow('Core Market magic is not DCLTCOR2');
  });

  it('refuses a Market whose phase and terminal receipt disagree', () => {
    const market = account('market');
    const forged = new Uint8Array(market.data);
    forged[320] = 1; // a terminal receipt on an Open Market
    expect(() => decodeMarketCoreStateV2(market.address, forged)).toThrow('carries a terminal receipt');
  });

  it('decodes the founding supply vector from the Claims aggregate, not from the Market', () => {
    const aggregate = account('claimsAggregate');
    const decoded = decodeClaimsAggregateV2(aggregate.address, aggregate.data);
    expect(aggregate.owner).toBe(live.programs.claims);
    expect(decoded.claimCount).toBe(4);
    expect(decoded.logicalMarket).toBe(account('market').address);
    expect(decoded.generation).toBe('2');
    expect(decoded.supplyAtoms).toEqual(['500000000', '500000000', '500000000', '500000000']);
    expect(decoded.maximumSupplyAtoms).toBe('500000000');
    expect(decoded.realmId).toBe('632c1f76f09491f39b197d8f67f61a922dee326941d8e663834fc1ed917f7760');
  });

  it('decodes the founder balances and the complete sets they admit', () => {
    const position = account('founderPosition');
    const decoded = decodeClaimsPositionV2(position.address, position.data);
    expect(position.owner).toBe(live.programs.claims);
    expect(decoded.owner).toBe(live.founder);
    expect(decoded.aggregate).toBe(account('claimsAggregate').address);
    expect(decoded.balances).toEqual(['500000000', '500000000', '500000000', '500000000']);
    expect(decoded.completeSetsAtoms).toBe('500000000');
  });

  it('derives the Claims aggregate and Position addresses the chain actually used', () => {
    const market = account('market');
    const aggregate = deriveClaimsAggregateAddressV2(live.programs.claims, market.address);
    expect(aggregate).toBe(account('claimsAggregate').address);
    expect(deriveClaimsPositionAddressV2(live.programs.claims, aggregate, live.founder))
      .toBe(account('founderPosition').address);
  });

  it('holds a Hoard the required backing is exactly covered by', () => {
    const aggregate = decodeClaimsAggregateV2(account('claimsAggregate').address, account('claimsAggregate').data);
    const vault = account('hoardVault');
    // Token account layout: mint@0, owner@32, amount u64@64.
    const amount = new DataView(vault.data.buffer, vault.data.byteOffset, vault.data.byteLength).getBigUint64(64, true);
    expect(amount.toString()).toBe(aggregate.maximumSupplyAtoms);
    // And the browser still cannot NAME that vault from the Market alone.
    expect(MARKET_HOARD_UNDERIVABLE_V1).toContain('founding action context');
  });

  it('refuses a Position whose declared claim count does not match its width', () => {
    const position = account('founderPosition');
    const forged = new Uint8Array(position.data);
    new DataView(forged.buffer).setUint32(12, 5, true);
    expect(() => decodeClaimsPositionV2(position.address, forged)).toThrow('5 claims demand exactly 168');
  });
});
