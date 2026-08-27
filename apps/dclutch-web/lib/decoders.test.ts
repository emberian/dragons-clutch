import { describe, expect, it } from 'vitest';

import fixture from '../fixtures/canonical-accounts.json';
import {
  crossCheckBindings,
  decodeCoreAccount,
  type AccountProjection,
  type FullAccountObservation,
  verifyLocalBindings,
} from './decoders';

function bytes(value: string): Uint8Array {
  const pairs = value.match(/../g);
  if (pairs === null || pairs.join('') !== value) throw new Error('fixture contains malformed hexadecimal bytes');
  return Uint8Array.from(pairs, (pair) => Number.parseInt(pair, 16));
}

function observation(account: (typeof fixture.accounts)[number], data = bytes(account.dataHex)): FullAccountObservation {
  return Object.freeze({
    address: account.address,
    owner: fixture.programId,
    executable: false,
    lamports: '1234567',
    observedSlot: '99',
    data,
  });
}

describe('frozen canonical Rust account fixtures', () => {
  it('strict-decodes and locally authenticates every emitted account', async () => {
    const decoded: AccountProjection[] = [];
    for (const account of fixture.accounts) {
      const projection = decodeCoreAccount(observation(account), fixture.programId);
      expect(projection.status).toBe('decoded');
      if (projection.status !== 'decoded') throw new Error(projection.reason);
      expect(projection.kind).toBe(account.kind);
      decoded.push(await verifyLocalBindings(projection, fixture.programId));
    }
    const joined = crossCheckBindings(decoded);
    for (const projection of joined) {
      expect(projection.status).toBe('decoded');
      if (projection.status !== 'decoded') throw new Error(projection.reason);
      expect(projection.bindings.length).toBeGreaterThan(0);
      expect(projection.bindings.every((check) => check.ok)).toBe(true);
    }
  });

  it('refuses version drift, trailing bytes, and owner substitution', () => {
    const realm = fixture.accounts.find((account) => account.kind === 'Realm');
    if (realm === undefined) throw new Error('fixture omitted Realm');

    const versionDrift = bytes(realm.dataHex);
    versionDrift[8] = 2;
    expect(decodeCoreAccount(observation(realm, versionDrift), fixture.programId).status).toBe('refused');

    const trailing = new Uint8Array(bytes(realm.dataHex).length + 1);
    trailing.set(bytes(realm.dataHex));
    expect(decodeCoreAccount(observation(realm, trailing), fixture.programId).status).toBe('refused');

    const substituted = Object.freeze({ ...observation(realm), owner: '11111111111111111111111111111111' });
    expect(decodeCoreAccount(substituted, fixture.programId).status).toBe('refused');
  });

  it('classifies no DCLTCAT1 or DCLTPOS1 header, because nothing writes them', () => {
    const encoder = new TextEncoder();
    for (const magic of ['DCLTCAT1', 'DCLTPOS1']) {
      const data = new Uint8Array(344);
      data.set(encoder.encode(magic), 0);
      const projection = decodeCoreAccount(observation(fixture.accounts[0], data), fixture.programId);
      expect(projection.status).toBe('refused');
      if (projection.status !== 'refused') throw new Error('a buried representation decoded');
      expect(projection.kind).toBe('Unknown');
    }
  });

  it('refuses aliased lifecycle identities and detects cross-generation RentCredit reuse', async () => {
    const rentCredit = fixture.accounts.find((account) => account.kind === 'RentCredit');
    if (rentCredit === undefined) throw new Error('fixture omitted lifecycle RentCredit');

    const aliased = bytes(rentCredit.dataHex);
    aliased.set(aliased.slice(48, 80), 80);
    expect(decodeCoreAccount(observation(rentCredit, aliased), fixture.programId).status).toBe('refused');

    const differentGeneration = bytes(rentCredit.dataHex);
    new DataView(differentGeneration.buffer).setBigUint64(112, 8n, true);
    const decoded = decodeCoreAccount(observation(rentCredit, differentGeneration), fixture.programId);
    expect(decoded.status).toBe('decoded');
    if (decoded.status !== 'decoded') throw new Error(decoded.reason);
    const verified = await verifyLocalBindings(decoded, fixture.programId);
    expect(verified.bindings).toHaveLength(1);
    expect(verified.bindings[0].ok).toBe(false);
  });
});
