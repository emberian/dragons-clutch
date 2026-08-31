import { PublicKey } from '@solana/web3.js';

import {
  ACTIVATION_CACHE_BYTES,
  ARTIFACT_RELEASE_BYTES,
  REGISTRY_ACTIVATED_ROLE_BYTES,
  REGISTRY_ACTIVATION_CACHE_ROLES_OFFSET,
  REGISTRY_ROLES,
  type RegistryRole,
} from '../lib/releaseRegistry';

/**
 * A Registry activation cache that names one program per role.
 *
 * The bump-hint miner reaches the release set's Custody deployment exactly the
 * way `direct_inline_hot_bump_hints_v1` does -- through the activation cache in
 * the hot fixed frame, because Custody itself is not in that frame. Mining
 * therefore depends on this body decoding, and a fixture that hand-wrote the
 * Custody program instead would leave that dependence untested.
 *
 * Only the fields `decodeArtifactReleaseV1` actually reads are filled, and each
 * is a distinct nonzero fill derived from the role index so no two roles alias.
 */
export function activationCacheFixtureV1(
  releaseSet: Uint8Array,
  programs: Readonly<Partial<Record<RegistryRole, string>>>,
): Uint8Array {
  if (releaseSet.length !== 32) throw new Error('activation cache fixture needs one 32-byte release set');
  const cache = new Uint8Array(ACTIVATION_CACHE_BYTES);
  cache.set(new TextEncoder().encode('DCLTACT1'), 0);
  new DataView(cache.buffer).setUint16(8, 1, true);
  new DataView(cache.buffer).setUint16(10, 1, true);
  cache.set(releaseSet, 16);
  REGISTRY_ROLES.forEach((role, index) => {
    const offset = REGISTRY_ACTIVATION_CACHE_ROLES_OFFSET + index * REGISTRY_ACTIVATED_ROLE_BYTES;
    cache.set(new Uint8Array(32).fill(0xa0 + index), offset);
    cache.set(artifactReleaseFixtureV1(index, programs[role]), offset + 32);
  });
  return cache;
}

/** One `DCLTARF1` body whose five identities are distinct and nonzero. */
export function artifactReleaseFixtureV1(seed: number, program?: string): Uint8Array {
  const bytes = new Uint8Array(ARTIFACT_RELEASE_BYTES);
  bytes.set(new TextEncoder().encode('DCLTARF1'), 0);
  const view = new DataView(bytes.buffer);
  view.setUint16(8, 1, true);
  view.setUint16(10, 1, true);
  // Upgrade policy 0 is `immutable`, whose authority field must stay zero.
  bytes[12] = 0;
  bytes.set(program === undefined ? new Uint8Array(32).fill(0xb0 + seed) : new PublicKey(program).toBytes(), 16);
  bytes.set(new Uint8Array(32).fill(0xc0 + seed), 48);
  bytes.set(new Uint8Array(32).fill(0xd0 + seed), 80);
  bytes.set(new Uint8Array(32).fill(0xe0 + seed), 112);
  bytes.set(new Uint8Array(32).fill(0xf0 + seed), 144);
  view.setBigUint64(176, 700n, true);
  return bytes;
}
