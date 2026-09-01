import { describe, expect, it } from 'vitest';

import type { CliContext } from '../src/context';
import {
  SPLINE_PRODUCT_COMMAND_V1,
  SPLINE_PRODUCT_COMPLETION_SCHEMA_V1,
  SPLINE_PRODUCT_INSPECTION_SCHEMA_V1,
  productCommand,
  splineProductArgumentsV1,
  type ProductInspectionDependenciesV1,
} from '../src/commands/product';
import { run } from '../src/main';

const INPUT = '/work/spline-product.json';
const OUTPUT = '/work/spline-product';
const REPORT = `${OUTPUT}/report.json`;
const SHA = 'a'.repeat(64);
const PRODUCT_FILES = ['product.bin', 'result-domain.bin', 'portfolio.bin', 'product-basis.bin', 'price-gate.bin'] as const;

function context(flags: Readonly<Record<string, string | boolean | undefined>>, json = false): CliContext {
  return Object.freeze({
    rpcUrl: 'http://127.0.0.1:20890/',
    session: Object.freeze({ rpcUrl: null, programs: Object.freeze({}), markets: Object.freeze([]) }),
    flags: Object.freeze(flags),
    json,
  });
}

function completion() {
  return Object.freeze({
    schema: SPLINE_PRODUCT_COMPLETION_SCHEMA_V1,
    output_dir: OUTPUT,
    report: REPORT,
    report_sha256: SHA,
  });
}

describe('public spline Product authoring', () => {
  it('passes only the exact input and new output directory to the Rust semantic owner', () => {
    expect(splineProductArgumentsV1(INPUT, OUTPUT)).toEqual([
      SPLINE_PRODUCT_COMMAND_V1,
      '--input', INPUT,
      '--output-dir', OUTPUT,
    ]);
    expect(splineProductArgumentsV1(INPUT, OUTPUT)).not.toContain('--keypair');
    expect(splineProductArgumentsV1(INPUT, OUTPUT)).not.toContain('--rpc-url');
  });

  it('admits the exact machine completion and states the authority boundary', async () => {
    const calls: string[] = [];
    const out: string[] = [];
    const code = await productCommand(
      context({ input: INPUT, 'output-dir': OUTPUT }),
      { out: (line) => out.push(line), err: () => undefined },
      'spline',
      {},
      {
        binary: () => { calls.push('locate-binary'); return '/bin/successor'; },
        spawn: (binary, args) => {
          calls.push('spawn');
          expect(binary).toBe('/bin/successor');
          expect(args).toEqual(splineProductArgumentsV1(INPUT, OUTPUT));
          return Object.freeze({ status: 0, signal: null, stdout: JSON.stringify(completion()), stderr: '' });
        },
      },
    );
    expect(code).toBe(0);
    expect(calls).toEqual(['locate-binary', 'spawn']);
    expect(out.join('\n')).toContain('canonical spline Product graph written');
    expect(out.join('\n')).toContain('key-free compiler; no wallet, signature, transaction, or submission');
  });

  it('refuses relative paths before locating the binary or spawning a child', async () => {
    let capabilities = 0;
    const dependencies = {
      binary: () => { capabilities += 1; return '/bin/successor'; },
      spawn: () => { capabilities += 1; return Object.freeze({ status: 0, signal: null, stdout: '{}', stderr: '' }); },
    };
    await expect(productCommand(context({ input: 'relative.json', 'output-dir': OUTPUT }), { out: () => undefined, err: () => undefined }, 'spline', {}, dependencies))
      .rejects.toThrow('--input must be an absolute path');
    await expect(productCommand(context({ input: INPUT, 'output-dir': 'relative-output' }), { out: () => undefined, err: () => undefined }, 'spline', {}, dependencies))
      .rejects.toThrow('--output-dir must be an absolute path');
    expect(capabilities).toBe(0);
  });

  it('refuses a substituted completion even after a zero child exit', async () => {
    await expect(productCommand(
      context({ input: INPUT, 'output-dir': OUTPUT }),
      { out: () => undefined, err: () => undefined },
      'spline',
      {},
      {
        binary: () => '/bin/successor',
        spawn: () => Object.freeze({
          status: 0,
          signal: null,
          stdout: JSON.stringify({ ...completion(), output_dir: '/work/other-product' }),
          stderr: '',
        }),
      },
    )).rejects.toThrow('does not bind');
  });

  it('is dispatched publicly and refuses before producer discovery', async () => {
    const out: string[] = [];
    const err: string[] = [];
    const code = await run([
      '--input', 'relative.json',
      '--output-dir', OUTPUT,
      'product', 'spline',
    ], {}, { out: (line) => out.push(line), err: (line) => err.push(line) });
    expect(code).toBe(1);
    expect(out).toEqual([]);
    expect(err.join('\n')).toContain('--input must be an absolute path');
  });

  it('loads the exact compiler directory and emits the Found39 handoff without finding a producer', async () => {
    const calls: string[] = [];
    const inspected: Awaited<ReturnType<ProductInspectionDependenciesV1['inspect']>> = Object.freeze({
      schema: 'dclutch/product-spline-authoring-report/v1' as const,
      command: 'product-spline-compile-v1' as const,
      keyFree: true as const,
      signs: false as const,
      submits: false as const,
      inputSha256: '1'.repeat(64),
      registryProgram: 'registry',
      productOutcomeCount: 3,
      basisWidth: 3,
      degree: 2 as const,
      interiorMultiplicity: false,
      payoutScale: '7',
      roundingBoundary: 'cumulative-floor-v3' as const,
      semanticBasisId: '2'.repeat(64),
      records: Object.freeze({
        product: Object.freeze({ file: PRODUCT_FILES[0], bytes: new Uint8Array([1]), schemaId: '1'.repeat(64), contentSha256: '2'.repeat(64), rawAccount: 'raw-0', stagingAccount: 'staging-0' }),
        result_domain: Object.freeze({ file: PRODUCT_FILES[1], bytes: new Uint8Array([2]), schemaId: '2'.repeat(64), contentSha256: '3'.repeat(64), rawAccount: 'raw-1', stagingAccount: 'staging-1' }),
        portfolio: Object.freeze({ file: PRODUCT_FILES[2], bytes: new Uint8Array([3]), schemaId: '3'.repeat(64), contentSha256: '4'.repeat(64), rawAccount: 'raw-2', stagingAccount: 'staging-2' }),
        product_basis: Object.freeze({ file: PRODUCT_FILES[3], bytes: new Uint8Array([4]), schemaId: '4'.repeat(64), contentSha256: '5'.repeat(64), rawAccount: 'raw-3', stagingAccount: 'staging-3' }),
        price_gate: Object.freeze({ file: PRODUCT_FILES[4], bytes: new Uint8Array([5]), schemaId: '5'.repeat(64), contentSha256: '6'.repeat(64), rawAccount: 'raw-4', stagingAccount: 'staging-4' }),
      }),
      verifiedPriceGate: Object.freeze({ scale: 7, mass: '1', degree: 2 as const, width: 3, atomCount: 1, prices: Object.freeze(['1', '4', '2']) }),
      foundRecords: Object.freeze({ productRecord: 'raw-0', resultDomainRecord: 'raw-1', portfolioRecord: 'raw-2', linkedBasisRecord: 'raw-3', priceGateRecord: 'raw-4' }),
    });
    const dependencies: ProductInspectionDependenciesV1 = Object.freeze({
      canonicalPath: (path) => path,
      metadata: (path) => Object.freeze({ bytes: path.endsWith('report.json') ? 2 : 1, isFile: true }),
      read: (path) => { calls.push(path); return path.endsWith('report.json') ? new TextEncoder().encode('{}') : new Uint8Array([1]); },
      inspect: async (_report, files) => {
        expect(Object.values(files).every((value) => value.length === 1)).toBe(true);
        return inspected;
      },
    });
    const out: string[] = [];
    const code = await productCommand(
      context({ report: '/work/spline-product/report.json' }, true),
      { out: (line) => out.push(line), err: () => undefined },
      'inspect', {},
      { binary: () => { throw new Error('producer discovery must not run'); }, spawn: () => { throw new Error('child must not run'); } },
      dependencies,
    );
    expect(code).toBe(0);
    expect(calls).toEqual(['/work/spline-product/report.json', ...PRODUCT_FILES.map((file) => `/work/spline-product/${file}`)]);
    const document = JSON.parse(out.join('')) as Record<string, unknown>;
    expect(document).toMatchObject({ schema: SPLINE_PRODUCT_INSPECTION_SCHEMA_V1, degree: 2, basis_width: 3, found_records: inspected.foundRecords });
    expect(JSON.stringify(document)).not.toContain('bytes":{"0"');
  });

  it('refuses a renamed, relative, or noncanonical report before reading files', async () => {
    let reads = 0;
    const dependencies: ProductInspectionDependenciesV1 = Object.freeze({
      canonicalPath: (path) => path.replace('/alias/', '/real/'),
      metadata: () => Object.freeze({ bytes: 1, isFile: true }),
      read: () => { reads += 1; return new Uint8Array([1]); },
      inspect: async () => { throw new Error('inspection must not run'); },
    });
    const io = { out: () => undefined, err: () => undefined };
    await expect(productCommand(context({ report: 'relative/report.json' }), io, 'inspect', {}, undefined, dependencies)).rejects.toThrow('absolute path');
    await expect(productCommand(context({ report: '/alias/report.json' }), io, 'inspect', {}, undefined, dependencies)).rejects.toThrow('not canonical');
    await expect(productCommand(context({ report: '/real/renamed.json' }), io, 'inspect', {}, undefined, dependencies)).rejects.toThrow('must name');
    expect(reads).toBe(0);
  });
});
