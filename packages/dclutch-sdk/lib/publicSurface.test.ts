import { readFileSync } from 'node:fs';

import { describe, expect, it } from 'vitest';

type PackageManifest = Readonly<{
  exports: Readonly<Record<string, string | null>>;
}>;

const retiredDirectPaths = [
  './directTransaction',
  './directCodec',
  './registeredDirect',
  './generated/registeredDirect',
] as const;

describe('package public surface', () => {
  it('refuses retired Direct V1 entry points even through wildcard exports', () => {
    const manifest = JSON.parse(
      readFileSync(new URL('../package.json', import.meta.url), 'utf8'),
    ) as PackageManifest;

    for (const path of retiredDirectPaths) {
      expect(manifest.exports[path]).toBeNull();
    }
  });
});
