import { describe, expect, it } from 'vitest';

import vector from '../fixtures/direct-descriptor-v4.devnet.json';
import { hex, sha256 } from './bytes';
import { decodeDirectDescriptorV4, validateDirectSignedRequestProfileV2 } from './directHotChain';
import {
  CAPABILITY_PROGRAM_V4_EFFECT_SCHEMA_OFFSET,
  DIRECT_ORDINARY_COMMON_SCALARS_V3,
  DIRECT_ORDINARY_ITEM_SCALAR_STRIDE_V3,
  EFFECT_SCHEMA_RELEASE_ID_V3,
  EFFECT_SCHEMA_RELEASE_ID_V4,
  REQUEST_PROFILE_COMMON_SCALARS_OFFSET,
  REQUEST_PROFILE_ITEM_SCALAR_STRIDE_OFFSET,
  REQUEST_PROFILE_V2_HEADER_BYTES,
} from './generated/directInlineV3';

/**
 * The descriptor the deployed chain actually publishes, byte for byte.
 *
 * Every other test of `decodeDirectDescriptorV4` builds its subject from the
 * same generated constants the decoder checks against, so the two agree by
 * construction and agree just as happily when both are wrong. That is not a
 * hypothetical: this file exists because `EFFECT_SCHEMA_RELEASE_ID` was
 * generated from the effect kernel's `v3.rs` while every publisher and every
 * Rust authenticator had moved to `v4.rs`. The synthetic tests stayed green and
 * the refusal reached a reader on clutch.dregg.pro instead.
 *
 * These bytes are cohort-8's real CapabilityProgramV4 record, read at finalized
 * commitment from devnet — the record both market21 and market22 select. A pin
 * that drifts away from what the chain publishes now reds here.
 */
const BYTES = Uint8Array.from((vector.descriptorHex.match(/../g) ?? []).map((b) => Number.parseInt(b, 16)));

describe('the CapabilityProgramV4 descriptor cohort-8 actually published', () => {
  it('is the exact record the fixture names, at its own content identity', async () => {
    expect(vector.schema).toBe('dclutch-direct-descriptor-v4-vector-v1');
    expect(vector.cluster).toBe('devnet');
    expect(BYTES.length).toBe(600);
    expect(hex(await sha256(BYTES))).toBe(vector.contentDigest);
  });

  it('is accepted by the browser authenticator', () => {
    const descriptor = decodeDirectDescriptorV4(BYTES);
    expect(hex(descriptor.effect.schema)).toBe(hex(EFFECT_SCHEMA_RELEASE_ID_V4));
    expect(hex(descriptor.derivationPolicy)).toBe(hex(descriptor.lifecycle.program));
    expect(descriptor.rootStateBytes).toBeGreaterThan(0);
  });

  it('names the effect generation the chain carries, not the one before it', () => {
    expect(hex(EFFECT_SCHEMA_RELEASE_ID_V4)).not.toBe(hex(EFFECT_SCHEMA_RELEASE_ID_V3));
    const observed = BYTES.slice(
      CAPABILITY_PROGRAM_V4_EFFECT_SCHEMA_OFFSET,
      CAPABILITY_PROGRAM_V4_EFFECT_SCHEMA_OFFSET + 32,
    );
    expect(hex(observed)).toBe(hex(EFFECT_SCHEMA_RELEASE_ID_V4));
  });

  /**
   * The red proof. Substituting the superseded V3 effect schema into the real
   * record must refuse — otherwise the acceptance above would be reporting a
   * decoder that does not read this field at all.
   */
  it('refuses the same record carrying the superseded V3 effect schema', () => {
    const hostile = Uint8Array.from(BYTES);
    hostile.set(EFFECT_SCHEMA_RELEASE_ID_V3, CAPABILITY_PROGRAM_V4_EFFECT_SCHEMA_OFFSET);
    expect(() => decodeDirectDescriptorV4(hostile)).toThrow(
      'selected CapabilityProgramV4 is not the schema-bound signed Direct InlineOrdinary bundle',
    );
  });
});

/**
 * The RequestProfile that descriptor names, also byte for byte.
 *
 * `validateDirectSignedRequestProfileV2` refused this record too, on a second
 * pin of the same kind: it demanded a literal zero item scalar stride while the
 * emitter writes `DIRECT_ORDINARY_ITEM_SCALAR_STRIDE_V3`, because the scalar
 * register file is affine in the Product's outcome count.
 *
 * Cohort-8 published this record before the execution price became the derived
 * equal split, which added three common scalars. Under decision 0012 an upgrade
 * is a re-found, so the record is not stale-and-fixable: it is a SUPERSEDED
 * release's record, and this client is right to refuse it. What the tests below
 * assert is that it is refused for exactly that one field and accepted on every
 * other, which is the strongest statement available until a cohort carrying the
 * derived price publishes a fresh vector.
 *
 * OWED: a `direct-descriptor-v4.devnet.json` read at finalized commitment from
 * the first cohort that carries the derived price, replacing this fixture and
 * restoring the plain acceptance assertion.
 */
const PROFILE = Uint8Array.from((vector.requestProfile.bytesHex.match(/../g) ?? []).map((b) => Number.parseInt(b, 16)));

describe('the InlineOrdinary RequestProfile cohort-8 actually published', () => {
  it('is the exact record at its own content identity', async () => {
    expect(PROFILE.length).toBe(1_272);
    expect(hex(await sha256(PROFILE))).toBe(vector.requestProfile.contentDigest);
  });

  /** The width the record was published under, read once and named here. */
  const PUBLISHED_COMMON_SCALARS = 68;

  function withCommonScalars(width: number): Uint8Array {
    const patched = Uint8Array.from(PROFILE);
    new DataView(patched.buffer).setUint16(
      REQUEST_PROFILE_V2_HEADER_BYTES + REQUEST_PROFILE_COMMON_SCALARS_OFFSET, width, true,
    );
    return patched;
  }

  it('is refused by this client on the one field its release moved', () => {
    expect(PUBLISHED_COMMON_SCALARS).not.toBe(DIRECT_ORDINARY_COMMON_SCALARS_V3);
    expect(new DataView(PROFILE.buffer, PROFILE.byteOffset, PROFILE.byteLength)
      .getUint16(REQUEST_PROFILE_V2_HEADER_BYTES + REQUEST_PROFILE_COMMON_SCALARS_OFFSET, true))
      .toBe(PUBLISHED_COMMON_SCALARS);
    expect(() => validateDirectSignedRequestProfileV2(PROFILE)).toThrow(
      new RegExp(`common scalar width of ${PUBLISHED_COMMON_SCALARS}, and this client reads ${DIRECT_ORDINARY_COMMON_SCALARS_V3}`),
    );
  });

  it('is accepted by the browser validator on every other field', () => {
    expect(() => validateDirectSignedRequestProfileV2(withCommonScalars(DIRECT_ORDINARY_COMMON_SCALARS_V3))).not.toThrow();
  });

  it('carries the affine scalar stride the emitter writes, not a flat zero', () => {
    const embedded = REQUEST_PROFILE_V2_HEADER_BYTES;
    const stride = new DataView(PROFILE.buffer, PROFILE.byteOffset, PROFILE.byteLength)
      .getUint16(embedded + REQUEST_PROFILE_ITEM_SCALAR_STRIDE_OFFSET, true);
    expect(stride).toBe(DIRECT_ORDINARY_ITEM_SCALAR_STRIDE_V3);
    expect(stride).not.toBe(0);
  });

  /**
   * The red proof for this one: flattening the stride to the zero the browser
   * used to demand must refuse, or the acceptance above reads nothing.
   */
  it('refuses the same record with its scalar stride flattened to zero', () => {
    // On the width-repaired record, so the stride is the only thing left to
    // accuse and the two conjuncts are proved to be separately named.
    const hostile = withCommonScalars(DIRECT_ORDINARY_COMMON_SCALARS_V3);
    new DataView(hostile.buffer).setUint16(
      REQUEST_PROFILE_V2_HEADER_BYTES + REQUEST_PROFILE_ITEM_SCALAR_STRIDE_OFFSET, 0, true,
    );
    expect(() => validateDirectSignedRequestProfileV2(hostile)).toThrow(
      'embedded RequestProfile does not select the exact fixed-width InlineOrdinary request',
    );
  });
});
