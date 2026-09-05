import { mkdtempSync, mkdirSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { describe, expect, it } from 'vitest';
import ts from 'typescript';

/**
 * The package's export map, held to what it promises.
 *
 * Two things an outside consumer must not be able to do: dispatch an
 * arbitrary JSON-RPC method through the client (the dispatcher is an
 * ECMAScript private slot, so nothing can be coerced into `sendTransaction`
 * by name), and reach a module by its filesystem path instead of its export
 * (`@dclutch/sdk/rpc`, never `@dclutch/sdk/lib/rpc`), which is what lets the
 * map say what is public at all.
 *
 * Submission itself is public: `SolanaRpcClient.sendRawTransaction` is one
 * bounded primitive (one packet, preflight on, genesis rechecked, no loop),
 * and every caller that reaches it owns a durable journal around it. The
 * package used to refuse it from the export map and hand the web app a
 * private fork of the client instead; the fork was the only consumer of that
 * refusal, and it is gone.
 */

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
  it('keeps the JSON-RPC dispatcher a private slot at runtime', async () => {
    const rpc = await import('@dclutch/sdk/rpc');
    const prototype = rpc.SolanaRpcClient.prototype as unknown as Record<string, unknown>;
    const client = new rpc.SolanaRpcClient('http://127.0.0.1:8899/') as unknown as Record<string, unknown>;

    expect(rpc.SolanaRpcClient).toBeTypeOf('function');
    expect('request' in rpc, 'a raw dispatcher escaped through the RPC subpath').toBe(false);
    expect('request' in prototype, 'a raw dispatcher escaped as an RPC client method').toBe(false);
    expect('request' in client, 'a raw dispatcher escaped as an RPC client instance property').toBe(false);
    expect(() => (client.request as (...args: unknown[]) => unknown)('sendTransaction', [])).toThrow(TypeError);
  });

  it('typechecks as an outside consumer only when the dispatcher and deep imports stay refused', () => {
    const diagnostics = externalConsumerDiagnostics(`
      import { SolanaRpcClient } from '@dclutch/sdk/rpc';
      const client = new SolanaRpcClient('http://127.0.0.1:8899/');
      // @ts-expect-error the JSON-RPC dispatcher is an ECMAScript private slot
      client['request']('sendTransaction', []);
      // @ts-expect-error package exports do not admit filesystem-style deep imports
      import('@dclutch/sdk/lib/rpc');
      void client.sendRawTransaction(new Uint8Array([1]));
    `);
    expect(diagnostics.map((diagnostic) => ts.flattenDiagnosticMessageText(diagnostic.messageText, '\n'))).toEqual([]);
    // 30s: this is the only case here that builds a whole TypeScript program.
    // Under vitest's 5s default it passed warm and failed on a cold runner.
  }, 30_000);

  it('refuses filesystem-style RPC deep imports at runtime', async () => {
    // @ts-expect-error the package export map intentionally refuses this path
    await expect(import('@dclutch/sdk/lib/rpc')).rejects.toThrow();
  });
});
