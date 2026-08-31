import { describe, expect, it } from 'vitest';

import { currentCoreMarketV3, LIVE, liveRpcAccount, mutate } from '@/fixtures/liveOpenMarket';
import { sha256 } from '../bytes';
import { CORE_STATE_GENERATION_OFFSET, CORE_STATE_MAGIC, REALM_SCHEMA_RELEASE_ID_V1 } from '../generated/coreFound';
import { deriveFinalizedRecordAddressesV1 } from '../releaseRegistry';
import type { RpcAccount, SolanaRpcClient } from '../rpc';
import { inspectAccount } from './account';

/**
 * The account view, read against the first locally OPEN Market's real bytes.
 *
 * `fixtures/liveOpenMarket.ts` holds finalized account data copied verbatim off
 * a successor-campaign validator, so these are the shapes the chain actually
 * produced — not buffers this repository built to agree with itself.
 */

const SLOT = '4711';
const RENT_MINIMUM = '1000000';

function client(accounts: ReadonlyMap<string, RpcAccount>): Pick<
  SolanaRpcClient,
  'accountInfo' | 'finalizedSlot' | 'minimumBalanceForRentExemption'
> {
  return {
    finalizedSlot: async () => SLOT,
    accountInfo: async (address: string) =>
      Object.freeze({ slot: SLOT, account: accounts.get(address) ?? null }),
    minimumBalanceForRentExemption: async (dataLength: number) =>
      Object.freeze({ dataLength, lamports: RENT_MINIMUM }),
  };
}

function one(address: string, account: RpcAccount): Map<string, RpcAccount> {
  return new Map([[address, account]]);
}

describe('the account view over live chain bytes', () => {
  it('decodes the Market and reproduces its address from its own seeds', async () => {
    const result = await inspectAccount(
      client(one(LIVE.market.address, liveRpcAccount(LIVE.market, { data: currentCoreMarketV3() }))),
      { address: LIVE.market.address },
    );
    expect(result.status).toBe('found');
    if (result.status !== 'found') return;
    const account = result.account;
    expect(account.header).toBe(new TextDecoder().decode(CORE_STATE_MAGIC));
    expect(account.decoded?.spec.name).toBe('Market Core state');
    expect(account.decoded?.widthCheck.ok).toBe(true);
    expect(account.owner).toBe(LIVE.programs.core);

    const marketId = account.decoded?.fields.find((entry) => entry.label === 'Market ID');
    expect(marketId?.value).toEqual({ form: 'address', base58: LIVE.market.address });

    // The nine seeds in the account reproduce the address it sits at.
    expect(account.derivations).toHaveLength(1);
    expect(account.derivations[0].name).toBe('Market Core state');
    expect(account.derivations[0].matches).toBe(true);
    expect(account.derivations[0].derived).toBe(LIVE.market.address);
  });

  it('decodes the Claims aggregate and reproduces it from the Market it names', async () => {
    const result = await inspectAccount(
      client(one(LIVE.claimsAggregate.address, liveRpcAccount(LIVE.claimsAggregate))),
      { address: LIVE.claimsAggregate.address },
    );
    if (result.status !== 'found') throw new Error(result.reason);
    expect(result.account.decoded?.spec.name).toBe('Claims aggregate');
    const logical = result.account.decoded?.fields.find((entry) => entry.label === 'Logical Market');
    expect(logical?.value).toEqual({ form: 'address', base58: LIVE.market.address });
    expect(result.account.derivations[0]?.matches).toBe(true);
    // The claim-supply rows are sized by the record's own claim count.
    expect(result.account.decoded?.rows?.scalars?.length).toBe(result.account.decoded?.rows?.count);
  });

  it('identifies a finalized record by re-deriving its raw-record PDA', async () => {
    const digest = await sha256(LIVE.realmRecord.data);
    const address = deriveFinalizedRecordAddressesV1(
      LIVE.programs.registry,
      REALM_SCHEMA_RELEASE_ID_V1,
      digest,
    ).record;
    const result = await inspectAccount(
      client(one(address, liveRpcAccount(LIVE.realmRecord, { owner: LIVE.programs.registry }))),
      { address },
    );
    if (result.status !== 'found') throw new Error(result.reason);
    expect(result.account.decoded?.spec.name).toBe('Realm');
    // The schema is not read off the content: it is the one whose PDA lands here.
    expect(result.account.record?.schema).toBe('Realm');
    expect(result.account.record?.stagingAddress).toBeTruthy();
  });

  it('does not name a schema for a record whose PDA no schema reproduces', async () => {
    const result = await inspectAccount(
      client(one(LIVE.realmRecord.address, liveRpcAccount(LIVE.realmRecord, { owner: LIVE.programs.core }))),
      { address: LIVE.realmRecord.address },
    );
    if (result.status !== 'found') throw new Error(result.reason);
    expect(result.account.record?.schema).toBeNull();
    expect(result.account.record?.contentDigest).toHaveLength(64);
  });

  it('reports a derivation MISMATCH when one field of the real bytes is changed', async () => {
    // Mutating the generation moves the address the seeds derive; the account
    // still sits where it did. Silence here would hide a real disagreement.
    const tampered = mutate(currentCoreMarketV3(), CORE_STATE_GENERATION_OFFSET, new Uint8Array([0xff, 0, 0, 0, 0, 0, 0, 0]));
    const result = await inspectAccount(
      client(one(LIVE.market.address, liveRpcAccount(LIVE.market, { data: tampered }))),
      { address: LIVE.market.address },
    );
    if (result.status !== 'found') throw new Error(result.reason);
    expect(result.account.derivations[0].matches).toBe(false);
    expect(result.account.derivations[0].derived).not.toBe(LIVE.market.address);
  });

  it('refuses to guess a layout for an unrecognized magic', async () => {
    const foreign = mutate(currentCoreMarketV3(), 0, new TextEncoder().encode('DCLTZZZ9'));
    const result = await inspectAccount(
      client(one(LIVE.market.address, liveRpcAccount(LIVE.market, { data: foreign }))),
      { address: LIVE.market.address },
    );
    if (result.status !== 'found') throw new Error(result.reason);
    expect(result.account.decoded).toBeNull();
    expect(result.account.header).toBe('DCLTZZZ9');
    expect(result.account.note).toContain('declares no record with the magic');
    // The bytes are still shown, so an unknown account is not a blank page.
    expect(result.account.headHex.length).toBeGreaterThan(0);
  });

  it('reports rent as a number, not a verdict it invented', async () => {
    const result = await inspectAccount(
      client(one(LIVE.market.address, liveRpcAccount(LIVE.market, { data: currentCoreMarketV3() }))),
      { address: LIVE.market.address },
    );
    if (result.status !== 'found') throw new Error(result.reason);
    expect(result.account.rent.exemptionMinimum).toBe(RENT_MINIMUM);
    expect(result.account.rent.exempt).toBe(BigInt(LIVE.market.lamports) >= BigInt(RENT_MINIMUM));
  });

  it('says an address is empty rather than returning a hollow account', async () => {
    const result = await inspectAccount(client(new Map()), { address: LIVE.market.address });
    expect(result.status).toBe('empty');
    if (result.status !== 'empty') return;
    expect(result.reason).toContain('No account exists');
  });

  it('labels the owner only when the runtime owns it or the reader named it', async () => {
    const accounts = one(LIVE.market.address, liveRpcAccount(LIVE.market, { data: currentCoreMarketV3() }));
    const unnamed = await inspectAccount(client(accounts), { address: LIVE.market.address });
    if (unnamed.status !== 'found') throw new Error(unnamed.reason);
    expect(unnamed.account.ownerLabel).toBeNull();

    const named = await inspectAccount(client(accounts), {
      address: LIVE.market.address,
      programLabels: { [LIVE.programs.core]: 'Core (selected)' },
    });
    if (named.status !== 'found') throw new Error(named.reason);
    expect(named.account.ownerLabel).toBe('Core (selected)');
  });
});
