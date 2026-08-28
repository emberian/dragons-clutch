import { mkdtempSync, mkdirSync, readFileSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { describe, expect, it } from 'vitest';
import ts from 'typescript';

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

const forbiddenRpcExports = [
  'request',
  'sendRawTransaction',
  'sendTransaction',
  'submitSignedTransaction',
] as const;

function externalConsumerDiagnostics(source: string): ReadonlyArray<ts.Diagnostic> {
  const temporary = mkdtempSync(join(tmpdir(), 'dclutch-sdk-consumer-'));
  try {
    const packageRoot = fileURLToPath(new URL('..', import.meta.url));
    const packageLink = join(temporary, 'node_modules', '@dclutch', 'sdk');
    mkdirSync(dirname(packageLink), { recursive: true });
    symlinkSync(packageRoot, packageLink, 'dir');
    const consumer = join(temporary, 'consumer.ts');
    writeFileSync(consumer, source);
    const options: ts.CompilerOptions = {
      target: ts.ScriptTarget.ES2022,
      module: ts.ModuleKind.ESNext,
      moduleResolution: ts.ModuleResolutionKind.Bundler,
      strict: true,
      noEmit: true,
      skipLibCheck: true,
    };
    return ts.getPreEmitDiagnostics(ts.createProgram([consumer], options));
  } finally {
    rmSync(temporary, { recursive: true, force: true });
  }
}

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

  it('keeps the root and RPC subpath read-only at runtime', async () => {
    const root = await import('@dclutch/sdk');
    const rpc = await import('@dclutch/sdk/rpc');
    const prototype = rpc.SolanaRpcClient.prototype as unknown as Record<string, unknown>;
    const client = new rpc.SolanaRpcClient('http://127.0.0.1:8899/') as unknown as Record<string, unknown>;

    expect(rpc.SolanaRpcClient).toBeTypeOf('function');
    for (const name of forbiddenRpcExports) {
      expect(name in root, `${name} escaped through the SDK root`).toBe(false);
      expect(name in rpc, `${name} escaped through the RPC subpath`).toBe(false);
      expect(name in prototype, `${name} escaped as an RPC client method`).toBe(false);
      expect(name in client, `${name} escaped as an RPC client instance property`).toBe(false);
    }
    expect(() => (client.request as (...args: unknown[]) => unknown)('sendTransaction', [])).toThrow(TypeError);
  });

  it('typechecks as an outside consumer only when submission and deep imports stay refused', () => {
    const diagnostics = externalConsumerDiagnostics(`
      import { SolanaRpcClient as RootClient } from '@dclutch/sdk';
      import { SolanaRpcClient as RpcClient } from '@dclutch/sdk/rpc';
      const rootClient = new RootClient('http://127.0.0.1:8899/');
      // @ts-expect-error the JSON-RPC dispatcher is an ECMAScript private slot
      rootClient['request']('sendTransaction', []);
      // @ts-expect-error the package root exposes read-only RPC, never submission
      rootClient.sendRawTransaction(new Uint8Array([1]));
      // @ts-expect-error the RPC subpath exposes read-only RPC, never submission
      new RpcClient('http://127.0.0.1:8899/').sendRawTransaction(new Uint8Array([1]));
      // @ts-expect-error package exports do not admit filesystem-style deep imports
      import('@dclutch/sdk/lib/rpc');
    `);
    expect(diagnostics.map((diagnostic) => ts.flattenDiagnosticMessageText(diagnostic.messageText, '\n'))).toEqual([]);
  });

  it('refuses filesystem-style RPC deep imports at runtime', async () => {
    // @ts-expect-error the package export map intentionally refuses this path
    await expect(import('@dclutch/sdk/lib/rpc')).rejects.toThrow();
  });
});
