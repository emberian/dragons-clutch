import { readFileSync } from 'node:fs';

import { PublicKey } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import { fromHex, hex, sha256 } from './bytes';
import {
  CORE_PHASE_OPEN_TAG,
  CORE_READINESS_CONSUMED_TAG,
  CORE_STATE_BYTES,
  CORE_STATE_CAPABILITY_MANIFEST_OFFSET,
  CORE_STATE_GENERATION_OFFSET,
  CORE_STATE_IDENTITY_REALM_OFFSET,
  CORE_STATE_MAGIC,
  CORE_STATE_MARKET_ID_OFFSET,
  CORE_STATE_PRINCIPAL_CAP_SETS_OFFSET,
  CORE_STATE_PRODUCT_ID_OFFSET,
  CORE_STATE_PRODUCT_RECORD_OFFSET,
  CORE_STATE_READINESS_OFFSET,
  CORE_STATE_REGISTRY_PROGRAM_OFFSET,
  CORE_STATE_RENT_BENEFICIARY_OFFSET,
  CORE_STATE_RESOLUTION_POLICY_OFFSET,
  CORE_STATE_SELECTED_RELEASE_SET_OFFSET,
  CORE_STATE_TERMINAL_RECEIPT_OFFSET,
  CORE_STATE_VERSION_OFFSET,
  CORE_STATE_PHASE_OFFSET,
  CORE_VERSION,
} from './generated/coreFound';
import {
  decodeClaimsAggregateV2,
  decodeClaimsPositionV2,
  decodeMarketCoreStateV2,
  deriveClaimsAggregateAddressV2,
  deriveClaimsPositionAddressV2,
  deriveCustodyAuthorityAddressV1,
  deriveMarketCoreAddressV2,
  deriveMarketHoardAddressV1,
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

/** Parser-only current-generation bytes; never presented as chain evidence. */
function syntheticCurrentMarket(): Uint8Array {
  const bytes = new Uint8Array(CORE_STATE_BYTES);
  bytes.set(CORE_STATE_MAGIC, 0);
  const view = new DataView(bytes.buffer);
  view.setUint16(CORE_STATE_VERSION_OFFSET, CORE_VERSION, true);
  bytes[CORE_STATE_PHASE_OFFSET] = CORE_PHASE_OPEN_TAG;
  bytes[CORE_STATE_READINESS_OFFSET] = CORE_READINESS_CONSUMED_TAG;
  for (const offset of [
    CORE_STATE_MARKET_ID_OFFSET,
    CORE_STATE_IDENTITY_REALM_OFFSET,
    CORE_STATE_PRODUCT_RECORD_OFFSET,
    CORE_STATE_PRODUCT_ID_OFFSET,
    CORE_STATE_RESOLUTION_POLICY_OFFSET,
    CORE_STATE_CAPABILITY_MANIFEST_OFFSET,
    CORE_STATE_SELECTED_RELEASE_SET_OFFSET,
    CORE_STATE_REGISTRY_PROGRAM_OFFSET,
    CORE_STATE_RENT_BENEFICIARY_OFFSET,
  ]) bytes.fill(1, offset, offset + 32);
  view.setBigUint64(CORE_STATE_GENERATION_OFFSET, 1n, true);
  view.setBigUint64(CORE_STATE_PRINCIPAL_CAP_SETS_OFFSET, 1n, true);
  return bytes;
}

/**
 * The Custody namespace the campaign that founded this Market chose.
 *
 * Two hashes, and neither is a protocol constant this browser may assume:
 *
 *   1. the FOUNDING ACTION CONTEXT, `SHA-256(campaign-domain || 0 || market ||
 *      generation_le || release_set)`. The domain is
 *      `dclutch/local-campaign/founding-context/v1` and it belongs to
 *      `tools/local-validator/bootstrap/successor/src/market.rs`, a campaign
 *      convenience the protocol never sees. Any 32 bytes are admissible there.
 *   2. the NAMESPACE, `SHA-256("dclutch:projected-hoard-context:v1" ||
 *      context)`, which the founding pins and Custody creates the Hoard under.
 *
 * Step 1 is restated here to reproduce ONE recorded campaign's artifact from
 * facts the fixture already carries, and for no other purpose. A shipped
 * surface must read the namespace off the Claims aggregate, which is why the
 * aggregate persisting it is what ADR 0008 is about.
 */
async function foundingCustodyNamespace(market: string, releaseSetId: string, generation: string): Promise<string> {
  const generationBytes = new Uint8Array(8);
  new DataView(generationBytes.buffer).setBigUint64(0, BigInt(generation), true);
  const context = await sha256(new Uint8Array([
    ...new TextEncoder().encode('dclutch/local-campaign/founding-context/v1'),
    0,
    ...new PublicKey(market).toBytes(),
    ...generationBytes,
    ...fromHex(releaseSetId, 'selected release set'),
  ]));
  return hex(await sha256(new Uint8Array([
    ...new TextEncoder().encode('dclutch:projected-hoard-context:v1'),
    ...context,
  ])));
}

describe('the Market a live dClutch chain actually holds', () => {
  it('refuses the superseded 352-byte finalized Market generation', () => {
    const market = account('market');
    expect(market.owner).toBe(live.programs.core);
    expect(market.data).toHaveLength(352);
    expect(() => decodeMarketCoreStateV2(market.address, market.data))
      .toThrow('This older devnet Market generation is incompatible');
  });

  it('does not derive a current Market address from superseded bytes', () => {
    const market = account('market');
    expect(() => deriveMarketCoreAddressV2(live.programs.core, market.data))
      .toThrow('This older devnet Market generation is incompatible');
  });

  it('refuses the superseded categorical Market layout rather than guessing', () => {
    const categorical = new Uint8Array(CORE_STATE_BYTES);
    categorical.set(new TextEncoder().encode('DCLTCAT1'));
    expect(() => decodeMarketCoreStateV2('11111111111111111111111111111111', categorical))
      .toThrow('Core Market magic is not DCLTCOR3');
  });

  it('refuses a Market whose phase and terminal receipt disagree', () => {
    const forged = syntheticCurrentMarket();
    forged[CORE_STATE_TERMINAL_RECEIPT_OFFSET] = 1;
    expect(() => decodeMarketCoreStateV2('11111111111111111111111111111111', forged))
      .toThrow('carries a terminal receipt');
  });

  it('refuses a current Market with no authenticated principal cap', () => {
    const forged = syntheticCurrentMarket();
    new DataView(forged.buffer).setBigUint64(CORE_STATE_PRINCIPAL_CAP_SETS_OFFSET, 0n, true);
    expect(() => decodeMarketCoreStateV2('11111111111111111111111111111111', forged))
      .toThrow('principal cap is absent');
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
    // The vault is owned by this Market's context-free Custody transfer
    // authority, which is what makes it THIS Market's Hoard and not a token
    // account at a coincidental address.
    expect(new PublicKey(vault.data.slice(32, 64)).toBase58()).toBe(
      deriveCustodyAuthorityAddressV1(live.programs.custody, account('market').address, aggregate.selectedReleaseSetId),
    );
  });

  it('names the live Hoard from the founding namespace, and only from that', async () => {
    const market = account('market').address;
    const aggregate = decodeClaimsAggregateV2(account('claimsAggregate').address, account('claimsAggregate').data);
    const namespace = await foundingCustodyNamespace(market, aggregate.selectedReleaseSetId, aggregate.generation);
    expect(deriveMarketHoardAddressV1(live.programs.custody, market, aggregate.selectedReleaseSetId, namespace))
      .toBe(account('hoardVault').address);
  });

  /**
   * The recorded chain is itself the witness for ADR 0008.
   *
   * This Market was founded before `FoundingV5` persisted the namespace it had
   * authenticated, so its aggregate says `custody_context = <the Market
   * address>` while its principal sits under the founding digest. Both
   * statements are finalized bytes off a real validator, and they do not agree:
   * every payout route deriving from the aggregate names an account that has
   * never existed. Kept as a case rather than a comment because it is the only
   * thing in this repository that proves the defect was live rather than
   * theoretical.
   */
  it('records a Market whose persisted namespace does not reach its own Hoard', () => {
    const market = account('market').address;
    const aggregate = decodeClaimsAggregateV2(account('claimsAggregate').address, account('claimsAggregate').data);
    expect(new PublicKey(fromHex(aggregate.custodyContext, 'persisted namespace')).toBase58()).toBe(market);
    expect(deriveMarketHoardAddressV1(live.programs.custody, market, aggregate.selectedReleaseSetId, aggregate.custodyContext))
      .not.toBe(account('hoardVault').address);
  });

  it('refuses a Position whose declared claim count does not match its width', () => {
    const position = account('founderPosition');
    const forged = new Uint8Array(position.data);
    new DataView(forged.buffer).setUint32(12, 5, true);
    expect(() => decodeClaimsPositionV2(position.address, forged)).toThrow('5 claims demand exactly 168');
  });
});
