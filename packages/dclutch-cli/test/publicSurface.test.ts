import { mkdtempSync, mkdirSync, readFileSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { build } from 'esbuild';
import { describe, expect, it } from 'vitest';
import ts from 'typescript';

type PackageManifest = Readonly<{
  bin: Readonly<Record<string, string>>;
  exports: Readonly<Record<string, string | null>>;
}>;

function installExternalCli(): Readonly<{ temporary: string; packageRoot: string }> {
  const temporary = mkdtempSync(join(tmpdir(), 'dclutch-cli-consumer-'));
  const packageRoot = fileURLToPath(new URL('..', import.meta.url));
  const packageLink = join(temporary, 'node_modules', '@dclutch', 'cli');
  mkdirSync(dirname(packageLink), { recursive: true });
  symlinkSync(packageRoot, packageLink, 'dir');
  return Object.freeze({ temporary, packageRoot });
}

describe('CLI package boundary', () => {
  it('exports only package metadata while preserving the dclutch binary', () => {
    const manifest = JSON.parse(readFileSync(new URL('../package.json', import.meta.url), 'utf8')) as PackageManifest;
    expect(manifest.bin).toEqual({ dclutch: './bin/dclutch.mjs' });
    expect(manifest.exports).toEqual({ './package.json': './package.json' });
  });

  it('typechecks as an outside consumer only when source and transport deep imports stay refused', () => {
    const { temporary } = installExternalCli();
    try {
      const consumer = join(temporary, 'consumer.ts');
      writeFileSync(consumer, `
        // @ts-expect-error the CLI has a binary entry, not a library root
        import('@dclutch/cli');
        // @ts-expect-error the signed-packet transport is caller-private
        import('@dclutch/cli/src/internal/rpcSubmission');
      `);
      const options: ts.CompilerOptions = {
        target: ts.ScriptTarget.ES2022,
        module: ts.ModuleKind.ESNext,
        moduleResolution: ts.ModuleResolutionKind.Bundler,
        strict: true,
        noEmit: true,
        skipLibCheck: true,
      };
      const diagnostics = ts.getPreEmitDiagnostics(ts.createProgram([consumer], options));
      expect(diagnostics.map((diagnostic) => ts.flattenDiagnosticMessageText(diagnostic.messageText, '\n'))).toEqual([]);
    } finally {
      rmSync(temporary, { recursive: true, force: true });
    }
  });

  it('refuses the private transport through an outside esbuild consumer', async () => {
    const { temporary } = installExternalCli();
    try {
      await expect(build({
        absWorkingDir: temporary,
        bundle: true,
        logLevel: 'silent',
        platform: 'node',
        stdin: {
          contents: "import { submitExactDevnetSignedPacketInternal } from '@dclutch/cli/src/internal/rpcSubmission'; void submitExactDevnetSignedPacketInternal;",
          loader: 'ts',
          resolveDir: temporary,
        },
        write: false,
      })).rejects.toThrow(/not exported|Could not resolve/);
    } finally {
      rmSync(temporary, { recursive: true, force: true });
    }
  });

  it('refuses the private transport through Node package resolution', async () => {
    const require = createRequire(import.meta.url);
    const privatePath = ['@dclutch/cli', 'src', 'internal', 'rpcSubmission'].join('/');
    expect(() => require.resolve(privatePath)).toThrow(/not (?:exported|defined)|Cannot find module/);
  });
});
