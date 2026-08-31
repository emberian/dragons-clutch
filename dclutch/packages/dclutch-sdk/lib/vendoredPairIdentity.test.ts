import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

import { describe, expect, it } from 'vitest';

/**
 * The vendored-pair rule, executed.
 *
 * `apps/dclutch-web` and `packages/dclutch-sdk` carry copies of the same wire
 * builders rather than one importing the other, so the browser bundle never
 * depends on a published package version. That is a deliberate trade and it has
 * exactly one failure mode: a fix landed in one copy and not the other, which
 * no per-file test can see because each copy is green against its own fixture.
 *
 * This test is the joint author for the files THIS lane vendored. It is
 * deliberately an explicit list rather than a directory sweep: several older
 * pairs have already drifted on purpose (`directMakerReplay.ts` is a subset in
 * the web tree), and a sweep would either go red for someone else's decision or
 * be softened until it proved nothing. A pair joins this list when its two
 * copies are meant to be byte-identical forever.
 */
const VENDORED_PAIRS = Object.freeze([
  'lib/directHotBumpHintsV1.ts',
  'lib/directHotBumpHintsV1.test.ts',
  'lib/directInlineV3.ts',
  'lib/directInlineV3.test.ts',
  'lib/directHotChain.ts',
  'lib/releaseRegistry.ts',
  'fixtures/hotBumpHintSource.ts',
  'fixtures/direct-hot-bump-hints.json',
] as const);

const SDK_ROOT = fileURLToPath(new URL('../', import.meta.url));
const WEB_ROOT = fileURLToPath(new URL('../../../apps/dclutch-web/', import.meta.url));

describe('vendored SDK/web pair identity', () => {
  it.each(VENDORED_PAIRS)('%s is byte-identical in both trees', (relative) => {
    const sdk = readFileSync(`${SDK_ROOT}${relative}`);
    const web = readFileSync(`${WEB_ROOT}${relative}`);
    expect(
      web.equals(sdk),
      `${relative} differs between packages/dclutch-sdk and apps/dclutch-web; the vendored pair is one implementation, mirrored`,
    ).toBe(true);
  });

  it('names the caller-mined bump-hint miner, which is why this file exists', () => {
    // The miner reconstructs seven PDA seed orders. A copy that drifted would
    // mine a hint naming a different address, and the route would REFUSE rather
    // than fall back -- so a browser trade would stop working while the SDK's
    // own suite stayed green. That is the specific accident this list prevents.
    expect(VENDORED_PAIRS).toContain('lib/directHotBumpHintsV1.ts');
    expect(VENDORED_PAIRS.length).toBeGreaterThan(0);
  });
});
