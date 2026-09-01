import { describe, expect, it } from 'vitest';

import type { CliContext } from '../src/context';
import {
  SPLINE_PRODUCT_COMMAND_V1,
  SPLINE_PRODUCT_COMPLETION_SCHEMA_V1,
  SPLINE_PRODUCT_INSPECTION_SCHEMA_V1,
  basisPointsV1,
  productCommand,
  snakeCaseV1,
  splineProductArgumentsV1,
  type ProductCommandDependenciesV1,
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
    deployment: null,
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

  /**
   * The compiler handoff, hoisted so both the JSON canary and the prose test
   * measure the SAME inspected object. Its `partitionQuality` section is the
   * one `inspectionDocumentV1` used to stop just short of.
   */
  const INSPECTED: Awaited<ReturnType<ProductInspectionDependenciesV1['inspect']>> = Object.freeze({
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
    partitionQuality: Object.freeze({
    model: 'triangular-plausible-band-v1',
    anchor: '15000000000',
    volatilityBps: 2_000,
    windowSlots: '10000',
    characteristicDisplacement: '3000000000',
    plausibleHalfWidth: '6000000000',
    dominantCell: 1,
    dominantShareBps: 5_000,
    maxCellShareBps: 9_000,
    cellShareBps: Object.freeze([2_500, 5_000, 2_500]),
    degenerate: false,
    }),
    foundRecords: Object.freeze({ productRecord: 'raw-0', resultDomainRecord: 'raw-1', portfolioRecord: 'raw-2', linkedBasisRecord: 'raw-3', priceGateRecord: 'raw-4' }),
  });

  function inspectionDependenciesV1(calls: string[] = []): ProductInspectionDependenciesV1 {
    return Object.freeze({
      canonicalPath: (path) => path,
      metadata: (path) => Object.freeze({ bytes: path.endsWith('report.json') ? 2 : 1, isFile: true }),
      read: (path) => { calls.push(path); return path.endsWith('report.json') ? new TextEncoder().encode('{}') : new Uint8Array([1]); },
      inspect: async (_report, files) => {
        expect(Object.values(files).every((value) => value.length === 1)).toBe(true);
        return INSPECTED;
      },
    });
  }

  const NO_PRODUCER: ProductCommandDependenciesV1 = Object.freeze({
    binary: () => { throw new Error('producer discovery must not run'); },
    spawn: () => { throw new Error('child must not run'); },
  });

  it('loads the exact compiler directory and emits the Found39 handoff without finding a producer', async () => {
    const calls: string[] = [];
    const out: string[] = [];
    const code = await productCommand(
      context({ report: '/work/spline-product/report.json' }, true),
      { out: (line) => out.push(line), err: () => undefined },
      'inspect', {}, NO_PRODUCER, inspectionDependenciesV1(calls),
    );
    expect(code).toBe(0);
    expect(calls).toEqual(['/work/spline-product/report.json', ...PRODUCT_FILES.map((file) => `/work/spline-product/${file}`)]);
    const document = JSON.parse(out.join('')) as Record<string, unknown>;
    expect(document).toMatchObject({ schema: SPLINE_PRODUCT_INSPECTION_SCHEMA_V1, degree: 2, basis_width: 3, found_records: INSPECTED.foundRecords });
    expect(JSON.stringify(document)).not.toContain('bytes":{"0"');
  });

  /**
   * THE CANARY.
   *
   * `inspectionDocumentV1` used to name its fields one at a time and stop at
   * `verified_price_gate`. A document assembled by enumeration does not fail
   * when the object it describes grows a section, and it does not warn: it
   * quietly describes an older version. When the compiler grew
   * `partition_quality` — the measurement that says whether an author is about
   * to found a degenerate market — the list would have dropped exactly the
   * number the author most needs before founding.
   *
   * So this compares key SETS derived from the inspected object, never a
   * hand-written list; a list here would just be the third copy of the same
   * enumeration, failing the same way one release later.
   */
  it('projects every inspected field, so a section added upstream cannot go missing', async () => {
    const out: string[] = [];
    await productCommand(
      context({ report: '/work/spline-product/report.json' }, true),
      { out: (line) => out.push(line), err: () => undefined },
      'inspect', {}, NO_PRODUCER, inspectionDependenciesV1(),
    );
    const document = JSON.parse(out.join('')) as Record<string, unknown>;
    const projected = Object.keys(INSPECTED)
      .filter((key) => key !== 'schema' && key !== 'command')
      .map(snakeCaseV1);
    expect(new Set(Object.keys(document))).toEqual(new Set([...projected, 'schema', 'report']));
    expect(document.partition_quality).toEqual(INSPECTED.partitionQuality);

    // One level down, for the row the loop deliberately cannot derive: the
    // document's `bytes` is a LENGTH where the inspection holds a byte array.
    const row = (document.records as Record<string, Record<string, unknown>>).product;
    expect(new Set(Object.keys(row))).toEqual(new Set(Object.keys(INSPECTED.records.product).map(snakeCaseV1)));
    expect(row.bytes).toBe(INSPECTED.records.product.bytes.length);
  });

  it('shows an author the partition quality in prose, above the record coordinates', async () => {
    const out: string[] = [];
    const code = await productCommand(
      context({ report: '/work/spline-product/report.json' }, false),
      { out: (line) => out.push(line), err: () => undefined },
      'inspect', {}, NO_PRODUCER, inspectionDependenciesV1(),
    );
    expect(code).toBe(0);
    const text = out.join('\n');
    expect(text).toContain('partition quality (triangular-plausible-band-v1)');
    expect(text).toContain('cell 1 holds 50.00% of the plausible band, ceiling 90.00%');
    expect(text).toContain('shares by cell: 25.00% / 50.00% / 25.00%');
    expect(text).toContain('anchor 15000000000 \u00b1 6000000000');
    expect(text).toContain('2000 bps over 10000 slots');
    // Above the coordinates, so a reader who stops at the first screen has
    // still seen the number that can tell them not to found this market.
    expect(out.findIndex((line) => line.includes('partition quality')))
      .toBeLessThan(out.findIndex((line) => line.includes('productRecord')));
  });

  /**
   * The claim this can actually refute is the PADDING, not exactness: a bare
   * `value / 100` prints 9,000 bps as "90%" and 9,009 as "90.09%", so a column
   * of shares stops lining up and 90.09 reads as the larger number. The
   * integer form also avoids a rounding mode, but no input in u32 distinguishes
   * it from `toFixed(2)`, so that is not claimed here.
   */
  it('pads the hundredths so a column of shares lines up', () => {
    expect(basisPointsV1(0)).toBe('0.00%');
    expect(basisPointsV1(1)).toBe('0.01%');
    expect(basisPointsV1(9_000)).toBe('90.00%');
    expect(basisPointsV1(9_009)).toBe('90.09%');
    expect(basisPointsV1(2_501)).toBe('25.01%');
    expect(basisPointsV1(10_000)).toBe('100.00%');
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
