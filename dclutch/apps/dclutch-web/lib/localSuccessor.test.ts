import { PublicKey } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import {
  RESOLUTION_CERTIFICATE_V2_REFUSAL_CORPUS_HEX,
  RESOLUTION_CERTIFICATE_V2_WIDE_SUCCESS_EXAMPLE_HEX,
} from '@dclutch/sdk/generated/resolutionCertificateV2';

import { SOURCE_RESOLUTION_STATE_V2_WIDE_TERMINAL_EXAMPLE_HEX } from './generated/sourceResolutionStateV2';

import machineVectors from '../fixtures/state-machines.devnet.json';
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
    const hostile = parseSuccessorAccount('rollback.certificate.failure.occupied', fixture('rollback.certificate.failure.occupied'));

    expect([activation.kind, activation.headline]).toEqual(['Registry activation cache', 'five checked roles']);
    expect([hostile.kind, hostile.headline]).toEqual(['hostile preoccupied certificate', 'deliberately malformed']);
  });

  it('refuses reserved bytes, invalid certificate kinds, and substituted hostile output', () => {
    // Offset 13, not 12. Byte 12 is the activation cache's own PDA bump
    // (`ACTIVATION_CACHE_BUMP_OFFSET_V1`), which took the first of what used to
    // be four reserved bytes; three remain, at 13. Mutating 12 tested a rule
    // the Registry itself now breaks on every cache it writes.
    expect(() => parseSuccessorAccount('registry.activation', mutate(fixture('registry.activation'), 13, 1))).toThrow('reserved');
    // And the tolerance the Rust owner requires, in the same breath: a cache
    // carrying a bump is an ordinary cache, and a reader that refuses one
    // refuses every cache the current Registry signs into existence.
    expect(() => parseSuccessorAccount('registry.activation', mutate(fixture('registry.activation'), 12, 254))).not.toThrow();
    expect(() => parseSuccessorAccount('rollback.certificate.failure.occupied', mutate(fixture('rollback.certificate.failure.occupied'), 311, 0))).toThrow('occupied pattern');
  });

  it('does not decode the buried categorical Market representation', () => {
    // `decoders.test.ts` already asserts that nothing writes DCLTCAT1. This
    // file decoded it anyway, all the way down to a nested DCLTROOT at 16 --
    // two client surfaces in one tree disagreeing about the same magic, and
    // the one with a layout was the wrong one. The generic arm is the honest
    // reading: name the magic, claim no schema.
    // A body that satisfied the deleted arm exactly: 344 bytes, schema 1,
    // three outcomes, the nested DCLTROOT at 16. It reads as an undecoded
    // record now, which is the point.
    const encoder = new TextEncoder();
    const data = new Uint8Array(344);
    data.set(encoder.encode('DCLTCAT1'), 0);
    data[8] = 1; data[10] = 3; data[11] = 1;
    data.set(encoder.encode('DCLTROOT'), 16);
    const market = parseSuccessorAccount('primary.market', Object.freeze({ owner: UPGRADEABLE_LOADER_ID, executable: false, data, space: data.length, lamports: '1' }));
    expect([market.kind, market.headline]).toEqual(['finalized semantic record', 'DCLTCAT1']);
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
 * The two records this surface used to decode in its own words.
 *
 * `localSuccessor.ts` carried a hand-written `DCLTSRS1` arm and a hand-written
 * `DCLTCFS1` arm. Neither magic has a producer:
 * `SourceResolutionStateV1::to_bytes` is reachable only from its own crate's
 * `#[cfg(test)] mod tests`, and `FundingStateV1`'s only allocator anywhere,
 * `stage_pending_funding`, has no caller. Both are now read through
 * `stateMachines`, whose table is emitted from the machines' own Rust
 * decoders, and the two cases below are the pair that makes that real: what
 * the LIVE records decode to, and what the frozen checkpoint's superseded
 * bodies say now that nothing pretends they are current.
 */
describe('Source and funding read through the derived decoder', () => {
  type MachineVector = Readonly<{ machine: string; address: string; owner: string; accountBytes: number; recordOffset: number; recordHex: string }>;
  const VECTORS = machineVectors.records as ReadonlyArray<MachineVector>;
  const vector = (machine: string, index: number): RpcAccount => {
    const entries = VECTORS.filter((entry) => entry.machine === machine);
    const entry = entries[index];
    if (entry === undefined) throw new Error(`cohort-15 vector ${machine}[${index}] is absent`);
    const data = Uint8Array.from(entry.recordHex.match(/../g) ?? [], (byte) => Number.parseInt(byte, 16));
    if (data.length !== entry.accountBytes || entry.recordOffset !== 0) throw new Error(`${machine}[${index}] is not a whole account`);
    return Object.freeze({ owner: entry.owner, executable: false, data, space: data.length, lamports: '1' });
  };

  // Read off devnet cohort-15 at slot 492837406 and committed as
  // `fixtures/state-machines.devnet.json`; these are the records the chain
  // actually holds, against the decoders this surface now uses.
  it('reads cohort-15 Source bodies the chain actually holds', () => {
    const unresolved = parseSuccessorAccount('lifecycle.state', vector('source', 0));
    const resolved = parseSuccessorAccount('lifecycle.state', vector('source', 1));

    expect([unresolved.kind, unresolved.headline]).toEqual(['Source resolution state', 'Primary']);
    expect([resolved.kind, resolved.headline]).toEqual(['Source resolution state', 'Resolved']);
    // The record identity is sourced, not stated: it comes from the generated
    // table, so a machine renamed in Rust moves this line by regenerating.
    expect(resolved.facts.map((entry) => [entry.label, entry.value])).toContainEqual(['record', 'SourceResolutionStateV2']);
    expect(resolved.facts.map((entry) => [entry.label, entry.value])).toContainEqual(['magic', 'DCLTSRS2']);
    expect(resolved.facts.map((entry) => [entry.label, entry.value])).toContainEqual(['wire tag', '2']);
  });

  // The six fields that left with the hand-written field map, against the two
  // devnet bodies that separate them. Every coordinate comes from
  // `generated/sourceResolutionStateV2.ts`; this file names none, which is why
  // it can assert them at all.
  it('prints the Source fields the derived table does not carry', () => {
    const unresolved = parseSuccessorAccount('lifecycle.state', vector('source', 0)).facts.map((entry) => [entry.label, entry.value]);
    const resolved = parseSuccessorAccount('lifecycle.state', vector('source', 1)).facts.map((entry) => [entry.label, entry.value]);

    expect(unresolved).toContainEqual(['market', '6aqy89GhhXFtDbawC5ors4HLkGvzdHC4R26TXTaaXRKj']);
    expect(resolved).toContainEqual(['market', '3QytL1bBMtCvRoXWR5h7MgutRBZqtv7emUVubEo5a4T2']);
    expect(resolved).toContainEqual(['generation', '2']);

    // The four that separate an unresolved Source from a resolved one. A
    // reader that had them at the wrong offsets would print the same value
    // for both, which is what makes this pair the test and not one body.
    expect(unresolved).toContainEqual(['terminal route', '0']);
    expect(resolved).toContainEqual(['terminal route', '1']);
    expect(unresolved).toContainEqual(['selector', '0']);
    expect(resolved).toContainEqual(['selector', '1']);
    expect(unresolved).toContainEqual(['terminal sequence', '0']);
    expect(resolved).toContainEqual(['terminal sequence', '1']);
    expect(unresolved).toContainEqual(['resolved at', '0']);
    expect(resolved).toContainEqual(['resolved at', '1788493373']);

    // Neither body has entered recovery, so this one is the same on both and
    // is asserted for presence rather than for a distinction it cannot make.
    expect(resolved).toContainEqual(['active attempt', '0']);
  });

  // The devnet pair above cannot pin the selector's WIDTH: both bodies carry a
  // selector of 0 or 1, so a reader taking it as a single byte agrees with
  // them and is wrong on every selector above 255. Measured -- that exact
  // mutation stayed green against the pair alone. The Lean-emitted wide
  // terminal example exists for this, and carries 257.
  it('reads the Source selector at its native width, not its leading byte', () => {
    const data = Uint8Array.from(SOURCE_RESOLUTION_STATE_V2_WIDE_TERMINAL_EXAMPLE_HEX.match(/../g) ?? [], (byte) => Number.parseInt(byte, 16));
    const wide = parseSuccessorAccount('lifecycle.state', Object.freeze({ owner: UPGRADEABLE_LOADER_ID, executable: false, data, space: data.length, lamports: '1' }));
    const facts = wide.facts.map((entry) => [entry.label, entry.value]);

    expect([wide.kind, wide.headline]).toEqual(['Source resolution state', 'Resolved']);
    expect(facts).toContainEqual(['selector', '257']);
    expect(facts).toContainEqual(['generation', '9']);
    expect(facts).toContainEqual(['terminal sequence', '1']);
    expect(facts).toContainEqual(['resolved at', '100']);
  });

  it('reads a cohort-15 funding ledger, including its slot count', () => {
    const single = parseSuccessorAccount('lifecycle.funding.failure', vector('funding-ledger', 0));
    const three = parseSuccessorAccount('lifecycle.funding.failure', vector('funding-ledger', 1));

    expect([single.kind, single.headline]).toEqual(['capability funding ledger', 'Active']);
    expect([three.kind, three.headline]).toEqual(['capability funding ledger', 'Active']);
    // A ledger's width is a function of how many manifest entries were
    // selected, so the slot count is the one fact that separates these two.
    expect(single.facts.map((entry) => [entry.label, entry.value])).toContainEqual(['slots', '1']);
    expect(three.facts.map((entry) => [entry.label, entry.value])).toContainEqual(['slots', '3']);
    expect(three.facts.map((entry) => [entry.label, entry.value])).toContainEqual(['record', 'FundingLedgerV2']);
  });

  it('names the committed checkpoint as a superseded generation instead of rendering it', () => {
    // The checkpoint was captured on 2026-08-25 against a program pair whose
    // ids appear nowhere in this tree but the fixture. Its Source and funding
    // bodies are real bytes of a generation nothing writes any more, and the
    // refusal has to say THAT and not "corrupt account" -- which is what the
    // derived decoder's own words ("opens with X and not Y") would read as on
    // a card.
    expect(() => parseSuccessorAccount('lifecycle.state', fixture('lifecycle.state')))
      .toThrow(/SupersededRecordGeneration: lifecycle.state opens with DCLTSRS1, and the record this client reads is SourceResolutionStateV2 \(DCLTSRS2\)/);
    expect(() => parseSuccessorAccount('lifecycle.funding.failure', fixture('lifecycle.funding.failure')))
      .toThrow(/SupersededRecordGeneration: lifecycle.funding.failure opens with DCLTCFS1, and the record this client reads is FundingLedgerV2 \(DCLTFL02\)/);
  });

  // The refusal above is about the GENERATION, and it must not swallow the
  // ordinary case. A body carrying the live magic and a byte its own Rust enum
  // admits no state for is a different accusation, and it keeps the derived
  // decoder's words.
  it('keeps an undecodable live record distinct from a superseded one', () => {
    const live = vector('source', 1);
    const corrupted = Object.freeze({ ...live, data: live.data.slice() });
    corrupted.data[10] = 9;
    expect(() => parseSuccessorAccount('lifecycle.state', corrupted))
      .toThrow('SourceResolutionStateV2: SourceResolutionPhaseV1 admits no state for byte 9');
  });
});

/**
 * The certificate, after `DCSRCER1` left.
 *
 * That arm was a 312-byte hand-written decoder over fourteen literal
 * coordinates, and `ResolutionCertificateV1` has no producer: every use of it
 * in `crates/dclutch-source/src/resolution/mod.rs` sits inside the
 * `#[cfg(test)] mod tests` that opens at line 2090, its only out-of-crate
 * consumer is the svm-harness `resolution-receipt-caller` test program, and
 * `programs/dclutch-resolution-proof-sbf/src/funded.rs:16-18` records that the
 * V1 generation of that accounting was orphaned dead code and was deleted.
 * What the Resolution program writes is `ResolutionCertificateV2`, and the SDK
 * already had the decoder for it.
 *
 * So the three cases below are the ones that make the swap real: a canonical
 * V2 body the Rust fixtures pin, the frozen checkpoint's V1 body saying what
 * it actually is, and a V2 body that is genuinely malformed keeping the
 * derived decoder's own words instead of being called a superseded generation.
 */
describe('Resolution certificates read through the derived V2 decoder', () => {
  const certificateAccount = (hex: string): RpcAccount => {
    const data = Uint8Array.from(hex.match(/../g) ?? [], (byte) => Number.parseInt(byte, 16));
    return Object.freeze({ owner: UPGRADEABLE_LOADER_ID, executable: false, data, space: data.length, lamports: '1' });
  };

  it('reads the canonical V2 certificate its own Rust fixture pins', () => {
    // `RESOLUTION_CERTIFICATE_V2_WIDE_SUCCESS_EXAMPLE_HEX` is emitted from
    // `crates/dclutch-source/src/resolution/generated_v2.rs`, so this is the
    // authority's own example read by the browser's own card.
    const certificate = parseSuccessorAccount('primary.certificate.success', certificateAccount(RESOLUTION_CERTIFICATE_V2_WIDE_SUCCESS_EXAMPLE_HEX));
    const facts = certificate.facts.map((entry) => [entry.label, entry.value]);

    expect([certificate.kind, certificate.headline]).toEqual(['signed Resolution certificate', 'resolution-success']);
    expect(facts).toContainEqual(['record', 'ResolutionCertificateV2']);
    expect(facts).toContainEqual(['magic', 'DCSRCER2']);
    expect(facts).toContainEqual(['generation', '9']);
    expect(facts).toContainEqual(['attempt / schedule', '0 / 0']);
    // 257 and not 1: the selector is a native u32, and a reader that took it
    // as the leading byte would report 1 here and be wrong on every selector
    // above 255. The example is wide on purpose.
    expect(facts).toContainEqual(['selector', '257']);
    expect(facts).toContainEqual(['result', '7/1']);
    expect(facts).toContainEqual(['observed at', '100']);
  });

  it('names the committed checkpoint certificate as a superseded generation', () => {
    // The same disposition its Source and funding siblings already have. The
    // capture predates the successor; the observed magic is read from the
    // bytes, never written down here.
    expect(() => parseSuccessorAccount('primary.certificate.success', fixture('primary.certificate.success')))
      .toThrow(/SupersededRecordGeneration: primary.certificate.success opens with DCSRCER1, and the record this client reads is ResolutionCertificateV2 \(DCSRCER2\)/);
  });

  it('keeps a malformed live certificate distinct from a superseded one', () => {
    // Corpus entry 2 opens with the LIVE magic at the live version and carries
    // kind 9, which the Rust enum admits no variant for. A refusal that called
    // that a superseded generation would be accusing the wrong thing.
    const malformed = RESOLUTION_CERTIFICATE_V2_REFUSAL_CORPUS_HEX[2] ?? '';
    expect(() => parseSuccessorAccount('primary.certificate.success', certificateAccount(malformed)))
      .toThrow('ResolutionCertificateV2 has an unknown kind');
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
