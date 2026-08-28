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

const forbiddenDirectExports = [
  'buildDirectNativeEvidenceInstructionV3',
  'compileDirectInlineTransactionV3',
  'encodeDirectInlineOrdinaryRequestV3',
  'validateDirectInlineInstructionSequenceV3',
  'validateDirectNativeEvidenceInstructionV3',
] as const;

const forbiddenWalletExports = [
  'requestWalletMessageSignatureV1',
  'requestWalletTransactionSignatureV1',
  'requireSubmittedSignatureMatchV1',
  'submitSignedTransactionV1',
  'transactionSignatureV1',
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

  it('routes Direct and wallet subpaths through read-only facades', () => {
    const manifest = JSON.parse(
      readFileSync(new URL('../package.json', import.meta.url), 'utf8'),
    ) as PackageManifest;

    expect(manifest.exports['./directInlineV3']).toBe('./lib/directInlinePublicV3.ts');
    expect(manifest.exports['./walletHandoff']).toBe('./lib/walletInspection.ts');
  });

  it('does not expose a Direct packet constructor from the root or public subpath', async () => {
    const root = await import('../index');
    const direct = await import('@dclutch/sdk/directInlineV3');

    expect(direct.previewDirectInlineV3).toBeTypeOf('function');
    expect(direct.encodeCompactIntentSigningMessageV2).toBeTypeOf('function');
    for (const name of forbiddenDirectExports) {
      expect(name in root, `${name} escaped through the SDK root`).toBe(false);
      expect(name in direct, `${name} escaped through the Direct public facade`).toBe(false);
    }
  });

  it('does not expose generic signing or submission through the wallet subpath', async () => {
    const wallet = await import('@dclutch/sdk/walletHandoff');

    expect(wallet.inspectUnsignedTransactionV1).toBeTypeOf('function');
    for (const name of forbiddenWalletExports) {
      expect(name in wallet, `${name} bypasses a caller-specific durable journal`).toBe(false);
    }
  });
});
