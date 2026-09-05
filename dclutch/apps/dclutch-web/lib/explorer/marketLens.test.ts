import { describe, expect, it } from 'vitest';

import { currentCoreMarketV3, LIVE, liveRpcAccount, mutate } from '@dclutch/sdk/fixtures/liveOpenMarket';
import { sha256 } from '@dclutch/sdk/bytes';
import {
  CORE_STATE_PHASE_OFFSET,
  CORE_STATE_TERMINAL_RECEIPT_OFFSET,
  CORE_STATE_TERMINAL_WINNER_OFFSET,
  REALM_SCHEMA_RELEASE_ID_V1,
} from '@dclutch/sdk/generated/coreFound';
import { deriveFinalizedRecordAddressesV1 } from '@dclutch/sdk/releaseRegistry';
import type { RpcAccount, SolanaRpcClient } from '@dclutch/sdk/rpc';
import { inspectMarketLens, type LensNode } from './marketLens';

/**
 * The Market lens over the first locally OPEN Market's real bytes.
 *
 * The lens does not re-derive the join — `lib/marketDiscovery.ts` owns that and
 * authenticates what it can. What these tests hold is the lens's own contract:
 * every edge becomes a node, every node says how far its identity was checked,
 * and nothing is made openable that was not actually derived.
 */

const SLOT = '4711';
const CORE = LIVE.programs.core;
const REGISTRY = LIVE.programs.registry;
const CLAIMS = LIVE.programs.claims;

function client(accounts: ReadonlyMap<string, RpcAccount>): Pick<SolanaRpcClient, 'finalizedSlot' | 'multipleAccounts'> {
  return {
    finalizedSlot: async () => SLOT,
    multipleAccounts: async (addresses: ReadonlyArray<string>) =>
      Object.freeze({
        slot: SLOT,
        accounts: Object.freeze(
          addresses.map((address) => Object.freeze({ address, account: accounts.get(address) ?? null })),
        ),
      }),
  };
}

async function chain(marketData: Uint8Array = currentCoreMarketV3()): Promise<Map<string, RpcAccount>> {
  const accounts = new Map<string, RpcAccount>([
    [LIVE.market.address, liveRpcAccount(LIVE.market, { data: marketData })],
    [LIVE.claimsAggregate.address, liveRpcAccount(LIVE.claimsAggregate)],
  ]);
  const realm = deriveFinalizedRecordAddressesV1(
    REGISTRY,
    REALM_SCHEMA_RELEASE_ID_V1,
    await sha256(LIVE.realmRecord.data),
  );
  accounts.set(realm.record, liveRpcAccount(LIVE.realmRecord));
  return accounts;
}

function node(nodes: ReadonlyArray<LensNode>, id: string): LensNode {
  const held = nodes.find((entry) => entry.id === id);
  if (held === undefined) throw new Error(`the lens has no ${id} node`);
  return held;
}

const full = { coreProgramId: CORE, registryProgramId: REGISTRY, claimsProgramId: CLAIMS };

describe('the Market lens', () => {
  it('makes the Market the root of a connected graph', async () => {
    const lens = await inspectMarketLens(client(await chain()), { ...full, address: LIVE.market.address });
    expect(node(lens.nodes, 'market').address).toBe(LIVE.market.address);
    // Every edge names nodes that exist: a link to nothing is a dead end the
    // reader would find by clicking it.
    const ids = new Set(lens.nodes.map((entry) => entry.id));
    for (const edge of lens.edges) {
      expect(ids.has(edge.from), `edge from ${edge.from} names no node`).toBe(true);
      expect(ids.has(edge.to), `edge to ${edge.to} names no node`).toBe(true);
    }
    // Nothing is orphaned: every node other than the root is reachable.
    const reachable = new Set(['market']);
    for (let pass = 0; pass < lens.nodes.length; pass += 1) {
      for (const edge of lens.edges) if (reachable.has(edge.from)) reachable.add(edge.to);
    }
    for (const entry of lens.nodes) {
      expect(reachable.has(entry.id), `${entry.id} is not reachable from the Market`).toBe(true);
    }
  });

  it('turns each of the five schema-paired identities into an openable record address', async () => {
    const lens = await inspectMarketLens(client(await chain()), { ...full, address: LIVE.market.address });
    for (const id of [
      'realmId',
      'productRecordId',
      'resolutionPolicyId',
      'capabilityManifestId',
      'selectedReleaseSetId',
    ]) {
      const held = node(lens.nodes, id);
      expect(held.contentId).toHaveLength(64);
      expect(held.address, `${id} was not derived`).toBeTruthy();
      expect(held.provenance.kind).toBe('derived');
      if (held.provenance.kind === 'derived') expect(held.provenance.how).toContain('not reacquired');
    }
  });

  it('derives the Realm identity to the address the Realm record was actually read at', async () => {
    const accounts = await chain();
    const lens = await inspectMarketLens(client(accounts), { ...full, address: LIVE.market.address });
    const realmRecord = deriveFinalizedRecordAddressesV1(
      REGISTRY,
      REALM_SCHEMA_RELEASE_ID_V1,
      await sha256(LIVE.realmRecord.data),
    ).record;
    // The lens's derivation and the discovery join's reacquisition are two
    // independent paths to the same address; if they disagreed, the lens would
    // be sending readers somewhere the join never looked.
    expect(node(lens.nodes, 'realmId').address).toBe(realmRecord);
    expect(node(lens.nodes, 'realm').address).toBe(realmRecord);
    expect(node(lens.nodes, 'realm').provenance.kind).toBe('observed');
  });

  it('never derives an address for the product instance, which is not a record', async () => {
    const lens = await inspectMarketLens(client(await chain()), { ...full, address: LIVE.market.address });
    const instance = node(lens.nodes, 'productInstance');
    expect(instance.contentId).toHaveLength(64);
    expect(instance.address).toBeNull();
    expect(instance.provenance.kind).toBe('stated');
  });

  it('carries the Claims aggregate with the Custody context only it records', async () => {
    const lens = await inspectMarketLens(client(await chain()), { ...full, address: LIVE.market.address });
    const aggregate = node(lens.nodes, 'aggregate');
    expect(aggregate.address).toBe(LIVE.claimsAggregate.address);
    expect(aggregate.facts.some((held) => held.label === 'Custody context')).toBe(true);
    expect(aggregate.facts.some((held) => held.label === 'Required backing')).toBe(true);
  });

  it('records a gap, with the reason, for every part it could not read', async () => {
    const lens = await inspectMarketLens(client(await chain()), { ...full, address: LIVE.market.address });
    // No Custody program is selected here, so there is no Hoard node and the
    // lens says why rather than showing an empty one.
    expect(lens.nodes.some((entry) => entry.id === 'hoard')).toBe(false);
    expect(lens.gaps.some((gap) => gap.startsWith('Hoard:'))).toBe(true);
    for (const gap of lens.gaps) expect(gap.length).toBeGreaterThan(16);
  });

  it('says so when no Registry program is selected, instead of showing bare digests', async () => {
    const lens = await inspectMarketLens(client(await chain()), {
      coreProgramId: CORE,
      claimsProgramId: CLAIMS,
      address: LIVE.market.address,
    });
    expect(node(lens.nodes, 'realmId').address).toBeNull();
    expect(node(lens.nodes, 'realmId').provenance.kind).toBe('stated');
    expect(lens.gaps.some((gap) => gap.includes('No Registry program is selected'))).toBe(true);
  });

  it('shows a terminal Market’s receipt identity without inventing an account for it', async () => {
    const terminal = mutate(
      mutate(mutate(currentCoreMarketV3(), CORE_STATE_PHASE_OFFSET, 2), CORE_STATE_TERMINAL_WINNER_OFFSET, 1),
      CORE_STATE_TERMINAL_RECEIPT_OFFSET,
      new Uint8Array(32).fill(0x77),
    );
    const lens = await inspectMarketLens(client(await chain(terminal)), {
      ...full,
      address: LIVE.market.address,
    });
    const receipt = node(lens.nodes, 'terminalReceipt');
    expect(receipt.contentId).toBe('77'.repeat(32));
    expect(receipt.address).toBeNull();
    expect(receipt.provenance.kind).toBe('stated');
  });

  it('degrades to one honest node when the Market itself does not decode', async () => {
    const broken = mutate(currentCoreMarketV3(), 0, new TextEncoder().encode('DCLTZZZ9'));
    const lens = await inspectMarketLens(client(await chain(broken)), {
      ...full,
      address: LIVE.market.address,
    });
    expect(lens.nodes).toHaveLength(1);
    expect(lens.nodes[0].provenance.kind).toBe('unavailable');
    expect(lens.edges).toEqual([]);
    expect(lens.gaps[0]).toContain('did not decode');
  });
});
