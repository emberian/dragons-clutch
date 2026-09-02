import { describe, expect, it } from 'vitest';

import vector from '../fixtures/dealer-scenario-account-profile-v4.json';
import { expectedScenarioProfile } from './dealerAccountProfileV3';

/**
 * The selector-9 AccountProfile, from both sides.
 *
 * `dealerAccountProfileV3.ts` re-implements the Rust profile encoder byte for
 * byte so `validateDealerAccountProfileV3` can refuse a profile the route did
 * not publish. It is a hand-maintained mirror with no generator behind it, and
 * it HAD DRIFTED: 8e4aa710e found three wrong bytes -- a signer bit copied from
 * the frame spec instead of from the encoder that drops it, and a `RequireKey`
 * guard removed by f5d4912e still mirrored at two offsets. Between efca6966 and
 * that commit this file would have refused every real selector-9 profile, and
 * nothing went red, because `dealerAccountProfileV3.test.ts` covers selector 1.
 *
 * The comparison that found those bytes was made by hand, out of band. This is
 * the durable form of it: the fixture is written by
 * `programs/dclutch-trading-sbf/tests/dealer_scenario_profile_vector.rs` from
 * the encoder itself, and the authority stays there -- if the profile moves,
 * the Rust test goes red first and this one follows.
 */
describe('selector-9 account profile', () => {
  const lengths: number[] = vector.commonDataLengths;
  const expected = Uint8Array.from(
    (vector.profileHex.match(/../g) ?? []).map((byte) => Number.parseInt(byte, 16)),
  );

  it('decodes the committed vector at its declared width', () => {
    expect(expected.length).toBe(vector.profileBytes);
    expect(expected.length).toBeGreaterThan(0);
  });

  it('mirrors the Rust encoder on every byte', () => {
    const mirrored = expectedScenarioProfile(lengths);
    expect(mirrored.length).toBe(expected.length);
    // Compare as a whole rather than byte by byte: a per-index loop that threw
    // on the first mismatch would hide how far the drift ran, and the drift
    // this test exists for was three bytes in two unrelated places.
    const differing = [...expected].reduce<number[]>(
      (found, byte, index) => (mirrored[index] === byte ? found : [...found, index]),
      [],
    );
    expect(differing).toEqual([]);
  });
});
