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

describe('canonical Rust account fixtures', () => {
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
    const market = fixture.accounts.find((account) => account.kind === 'Market');
    if (market === undefined) throw new Error('fixture omitted Market');

    const versionDrift = bytes(market.dataHex);
    versionDrift[8] = 2;
    expect(decodeCoreAccount(observation(market, versionDrift), fixture.programId).status).toBe('refused');

    const trailing = new Uint8Array(bytes(market.dataHex).length + 1);
    trailing.set(bytes(market.dataHex));
    expect(decodeCoreAccount(observation(market, trailing), fixture.programId).status).toBe('refused');

    const substituted = Object.freeze({ ...observation(market), owner: '11111111111111111111111111111111' });
    expect(decodeCoreAccount(substituted, fixture.programId).status).toBe('refused');
  });
});
