/**
 * Emit the browser's partition-quality gate from its Rust owner.
 *
 * `apps/dclutch-web` had ZERO occurrences of `max_cell_share_bps`,
 * `founding_band`, or volatility-as-input, so a market founded through the
 * create wizard was never measured by the gate that refuses degenerate
 * partitions. What ran instead was a strictly weaker unit-sanity check with a
 * provisional constant of its own — and `lib/founding/rangeProtection.ts` said
 * so, including the lifting plan: "delete it, and call
 * `require_interesting_partition_v1` with the market's own founding band once
 * `dclutch-product-compiler` reaches the browser."
 *
 * Everything emitted here comes from Rust: the two JSON schema names and the
 * transport bound are read out of the wasm crate's own `const`s, and the crate
 * pins the ceiling-on-the-ceiling, the basis-point unit and the volatility
 * bound at compile time with `const _: () = assert!(...)` read BY CONSTANT NAME
 * from the compiler — so a wizard can never offer a ceiling the gate refuses.
 *
 * Usage:
 *   node scripts/generate-partition-quality-wasm.mjs           # regenerate
 *   node scripts/generate-partition-quality-wasm.mjs --check   # verify only
 */import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = fileURLToPath(new URL('.', import.meta.url));
const app = resolve(here, '..');
const root = resolve(app, '../..');
const wasmOwner = join(root, 'crates/dclutch-partition-quality-wasm/src/lib.rs');
const crate = 'dclutch-partition-quality-wasm';
const output = join(app, 'lib/generated/partitionQualityWasm');
const facts = join(app, 'lib/generated/partitionQualityWasmV1.ts');
const check = process.argv.includes('--check');
const ARTIFACTS = [
  'partition_quality.js',
  'partition_quality.d.ts',
  'partition_quality_bg.wasm',
  'partition_quality_bg.wasm.d.ts',
];

function rustString(source, name) {
  const match = source.match(new RegExp(`const ${name}: &str = "([^"]+)";`));
  if (match === null) throw new Error(`the partition-quality WASM owner does not expose ${name}`);
  return match[1];
}

function rustUsize(source, name) {
  const match = source.match(new RegExp(`const ${name}: usize = ([0-9_]+);`));
  if (match === null) throw new Error(`the partition-quality WASM owner does not expose a usize ${name}`);
  return Number(match[1].replaceAll('_', ''));
}

const temporary = mkdtempSync(join(tmpdir(), 'dclutch-payoff-wasm.'));
try {
  const target = join(temporary, 'target');
  execFileSync('cargo', ['build', '-p', crate, '--target', 'wasm32-unknown-unknown', '--release'], {
    cwd: root, env: { ...process.env, CARGO_TARGET_DIR: target }, stdio: 'inherit',
  });
  const built = join(target, 'wasm32-unknown-unknown/release/dclutch_partition_quality_wasm.wasm');
  const generated = join(temporary, 'bindgen');
  execFileSync('wasm-bindgen', ['--target', 'web', '--out-dir', generated, '--out-name', 'partition_quality', built], { cwd: root, stdio: 'inherit' });

  const owner = readFileSync(wasmOwner, 'utf8');
  const wasm = readFileSync(join(generated, 'partition_quality_bg.wasm'));
  const digest = createHash('sha256').update(wasm).digest('hex');

  const generatedFacts = '// @generated from the authoritative Rust partition-quality gate and WASM artifact; do not edit.\n'
    + '// Regenerate with: npm run abi:partition-quality-wasm\n'
    + `export const PARTITION_QUALITY_REQUEST_FORMAT_V1 = '${rustString(owner, 'REQUEST_FORMAT_V1')}' as const;\n`
    + `export const PARTITION_QUALITY_REPORT_FORMAT_V1 = '${rustString(owner, 'REPORT_FORMAT_V1')}' as const;\n`
    + `export const PARTITION_QUALITY_MAX_CUTS_V1 = ${rustUsize(owner, 'MAX_CUTS_V1')} as const;\n`
    + `export const PARTITION_QUALITY_WASM_SHA256_V1 = '${digest}' as const;\n`
    + `export const PARTITION_QUALITY_WASM_BYTES_V1 = ${wasm.length} as const;\n`;

  if (check) {
    if (readFileSync(facts, 'utf8') !== generatedFacts) throw new Error('generated partition-quality facts differ');
    for (const name of ARTIFACTS) {
      if (!readFileSync(join(output, name)).equals(readFileSync(join(generated, name)))) throw new Error(`generated ${name} differs`);
    }
  } else {
    mkdirSync(output, { recursive: true });
    for (const name of ARTIFACTS) writeFileSync(join(output, name), readFileSync(join(generated, name)));
    writeFileSync(facts, generatedFacts);
  }
} finally {
  rmSync(temporary, { recursive: true, force: true });
}
