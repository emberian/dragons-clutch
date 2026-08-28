import { PublicKey } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import checkpointJson from '../fixtures/successor-checkpoint.json';
import {
  LOCAL_SUCCESSOR_CHECKPOINT,
  decodeCheckpointFixtureAccount,
  decodeLocalSuccessorCheckpoint,
  parseSuccessorAccount,
} from './localSuccessor';
import { UPGRADEABLE_LOADER_ID } from './releaseRegistry';
import type { RpcAccount } from './rpc';

function fixture(name: keyof typeof checkpointJson.parser_fixtures.accounts): RpcAccount {
  return decodeCheckpointFixtureAccount(checkpointJson.parser_fixtures.accounts[name].account);
}

function mutate(account: RpcAccount, offset: number, value: number): RpcAccount {
  const data = account.data.slice();
  data[offset] = value;
  return Object.freeze({ ...account, data });
}

const AUTHORITY = new PublicKey(Uint8Array.from({ length: 32 }, () => 0x5a)).toBase58();

/** A checkpoint whose Registry role records the given pin. */
function pinned(deploymentSlot: number, upgradeAuthority: string | null) {
  const moved = structuredClone(checkpointJson) as unknown as Record<string, unknown>;
  const registry = (moved.programs as Record<string, Record<string, unknown>>).registry;
  registry.deployment_slot = deploymentSlot;
  registry.upgrade_authority = upgradeAuthority;
  registry.upgrade_authority_effectively_disabled = upgradeAuthority === null;
  return decodeLocalSuccessorCheckpoint(moved);
}

/**
 * A Loader-v3 ProgramData body.
 *
 * `authorityLitter` writes a key into 13..45 with the tag left at 0 — the exact
 * bytes `SetAuthority(Some -> None)` leaves behind on a revoked program, which
 * this module's `requireZero(bytes, 13, 32)` used to call malformed.
 */
function programData(slot: number, authority: string | null, authorityLitter = false): RpcAccount {
  const data = new Uint8Array(45 + 8);
  const view = new DataView(data.buffer);
  view.setUint32(0, 3, true);
  view.setBigUint64(4, BigInt(slot), true);
  if (authority !== null) { data[12] = 1; data.set(new PublicKey(authority).toBytes(), 13); }
  else if (authorityLitter) data.set(new PublicKey(AUTHORITY).toBytes(), 13);
  return Object.freeze({ owner: UPGRADEABLE_LOADER_ID, executable: false, data, space: data.length, lamports: '1' });
}

describe('immutable localhost successor checkpoint', () => {
  it('binds one loopback genesis, immutable Loader programs, and explicit evidence limits', () => {
    expect(LOCAL_SUCCESSOR_CHECKPOINT.network.rpc_url).toBe('http://127.0.0.1:20890/');
    expect(LOCAL_SUCCESSOR_CHECKPOINT.network.genesis_hash).toBe('F6x7Lf6PBNu5e8cecisH2o25y9cj4xW9xd4iZzWKZeqn');
    expect(Object.keys(LOCAL_SUCCESSOR_CHECKPOINT.expected_accounts)).toHaveLength(33);
    expect(LOCAL_SUCCESSOR_CHECKPOINT.expected_transactions).toHaveLength(9);
    expect(LOCAL_SUCCESSOR_CHECKPOINT.evidence.checked_production_release_claimed).toBe(false);
    expect(LOCAL_SUCCESSOR_CHECKPOINT.evidence.captured_release_identity_claimed).toBe(false);
    expect(LOCAL_SUCCESSOR_CHECKPOINT.programs.registry.upgrade_authority).toBeNull();
    expect(LOCAL_SUCCESSOR_CHECKPOINT.programs.resolution.upgrade_authority).toBeNull();
  });

  it('decodes representative live RPC bodies through exact account layouts', () => {
    const activation = parseSuccessorAccount('registry.activation', fixture('registry.activation'));
    const certificate = parseSuccessorAccount('primary.certificate.success', fixture('primary.certificate.success'));
    const state = parseSuccessorAccount('lifecycle.state', fixture('lifecycle.state'));
    const funding = parseSuccessorAccount('lifecycle.funding.failure', fixture('lifecycle.funding.failure'));
    const hostile = parseSuccessorAccount('rollback.certificate.failure.occupied', fixture('rollback.certificate.failure.occupied'));

    expect([activation.kind, activation.headline]).toEqual(['Registry activation cache', 'five checked roles']);
    expect([certificate.kind, certificate.headline]).toEqual(['signed Resolution certificate', 'primary success']);
    expect([state.kind, state.headline]).toEqual(['Source resolution state', 'failure committed']);
    expect([funding.kind, funding.headline]).toEqual(['typed capability funding', 'active']);
    expect([hostile.kind, hostile.headline]).toEqual(['hostile preoccupied certificate', 'deliberately malformed']);
  });

  it('refuses reserved bytes, invalid certificate kinds, and substituted hostile output', () => {
    expect(() => parseSuccessorAccount('registry.activation', mutate(fixture('registry.activation'), 12, 1))).toThrow('reserved');
    expect(() => parseSuccessorAccount('primary.certificate.success', mutate(fixture('primary.certificate.success'), 10, 0))).toThrow('certificate');
    expect(() => parseSuccessorAccount('rollback.certificate.failure.occupied', mutate(fixture('rollback.certificate.failure.occupied'), 311, 0))).toThrow('occupied pattern');
  });

  it('refuses a checkpoint that promotes localhost evidence into a release claim', () => {
    const promoted = structuredClone(checkpointJson) as unknown as Record<string, unknown>;
    (promoted.evidence as Record<string, unknown>).checked_production_release_claimed = true;
    expect(() => decodeLocalSuccessorCheckpoint(promoted)).toThrow('must not claim production');
  });

  it('accepts any loopback base, because the validator origin is a parameter and not a constant', () => {
    const relocate = (rpcUrl: string) => {
      const moved = structuredClone(checkpointJson) as unknown as Record<string, unknown>;
      (moved.network as Record<string, unknown>).rpc_url = rpcUrl;
      return moved;
    };
    // Campaigns now run concurrently on disjoint 42-port blocks, so a
    // checkpoint captured from one of them is not a malformed checkpoint.
    expect(decodeLocalSuccessorCheckpoint(relocate('http://127.0.0.1:31890/')).network.rpc_url).toBe('http://127.0.0.1:31890/');
    expect(decodeLocalSuccessorCheckpoint(relocate('http://127.0.0.1:20890/')).network.rpc_url).toBe('http://127.0.0.1:20890/');
    // What the gate is actually for is unchanged: a checkpoint can only ever
    // point the browser at a validator on this machine.
    for (const hostile of ['https://127.0.0.1:20890/', 'http://example.com:20890/', 'http://8.8.8.8:20890/', 'http://localhost:20890/', 'http://127.0.0.1/']) {
      expect(() => decodeLocalSuccessorCheckpoint(relocate(hostile))).toThrow('loopback explicit-port profile');
    }
  });
});

/**
 * Decision 0012 in the browser.
 *
 * Before this lane, `decodeLocalSuccessorCheckpoint` refused any checkpoint
 * whose `deployment_slot` was not 0 or whose `upgrade_authority` was not null,
 * and `decodeLoader` refused any ProgramData that did not read as a slot-zero
 * `None` header. Both were the Immutable-only gate the decision retired, and
 * together they meant the browser could not look at an iterated devnet
 * substrate at all — not to show it, and not even to explain why it refused.
 */
describe('slot-pinned successor substrate', () => {
  it('reads an iterated substrate the Immutable-only gate refused outright', () => {
    const checkpoint = pinned(531, AUTHORITY);
    const parsed = parseSuccessorAccount('loader.registry.programdata', programData(531, AUTHORITY), checkpoint);
    expect(parsed.kind).toBe('slot-pinned Loader ProgramData');
    expect(parsed.facts).toContainEqual({ label: 'deployment slot', value: '531' });
    expect(parsed.facts).toContainEqual({ label: 'upgrade authority', value: AUTHORITY });
  });

  it('still reads the immutable substrate exactly as before', () => {
    const parsed = parseSuccessorAccount('loader.registry.programdata', programData(0, null));
    expect(parsed.kind).toBe('immutable Loader ProgramData');
    expect(parsed.facts).toContainEqual({ label: 'upgrade authority', value: 'none' });
  });

  // The Loader writes `ProgramData { slot, upgrade_authority: None }` as
  // thirteen bytes over a forty-five byte header, so a REVOKED program keeps
  // the old key inert at 13..45. `releaseRegistry.ts` stopped calling that
  // malformed on 2026-08-27 after measuring it live; this copy had not.
  it('accepts the retained authority litter a revocation leaves behind', () => {
    const parsed = parseSuccessorAccount('loader.registry.programdata', programData(0, null, true));
    expect(parsed.kind).toBe('immutable Loader ProgramData');
    expect(parsed.facts).toContainEqual({ label: 'upgrade authority', value: 'none' });
  });

  it('names a moved slot ReleaseSupersededByUpgrade in the protocol’s registered words', () => {
    const checkpoint = pinned(531, AUTHORITY);
    expect(() => parseSuccessorAccount('loader.registry.programdata', programData(532, AUTHORITY), checkpoint))
      .toThrow(/ReleaseSupersededByUpgrade.*pins deployment slot 531, and the chain reports slot 532.*substrate was upgraded/s);
  });

  it('keeps a backward slot as plain staleness, because the Loader never moves one backward', () => {
    const checkpoint = pinned(531, AUTHORITY);
    let message = '';
    try { parseSuccessorAccount('loader.registry.programdata', programData(530, AUTHORITY), checkpoint); }
    catch (error) { message = error instanceof Error ? error.message : String(error); }
    expect(message).toContain('DeploymentSlotMismatch');
    expect(message).not.toContain('Superseded');
  });

  it('refuses a substituted upgrade authority even when the slot still holds', () => {
    const checkpoint = pinned(531, AUTHORITY);
    const other = new PublicKey(Uint8Array.from({ length: 32 }, () => 0x21)).toBase58();
    expect(() => parseSuccessorAccount('loader.registry.programdata', programData(531, other), checkpoint))
      .toThrow('UpgradeAuthorityMismatch');
  });

  // What `MutableRegistryRelease` still means: a pairing the chain refuses.
  it('refuses a checkpoint whose authority and disabled flag disagree', () => {
    const disagreeing = structuredClone(checkpointJson) as unknown as Record<string, unknown>;
    (disagreeing.programs as Record<string, Record<string, unknown>>).registry.upgrade_authority = AUTHORITY;
    expect(() => decodeLocalSuccessorCheckpoint(disagreeing)).toThrow('not a canonical pinned Loader profile');
  });
});
