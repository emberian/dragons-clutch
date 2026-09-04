import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const app = resolve(here, '..');
const root = resolve(app, '../..');
const wire = join(root, 'crates/dclutch-source-provider-wasm/src/wire.rs');
const crate = 'dclutch-source-provider-wasm';
const output = join(app, 'lib/generated/sourceProviderWasm');
const facts = join(app, 'lib/generated/sourceProviderWasmV1.ts');
const sdkFacts = join(root, 'packages/dclutch-sdk/lib/generated/sourceProviderWasmV1.ts');
const check = process.argv.includes('--check');
const temporary = mkdtempSync(join(tmpdir(), 'dclutch-source-provider-wasm.'));

function rustString(source, name) {
  const match = source.match(new RegExp(`const ${name}: &str\\s*=\\s*"([^"]+)";`));
  if (match === null) throw new Error(`Rust Source-provider owner does not expose ${name}`);
  return match[1];
}

function rustNumber(source, name) {
  const match = source.match(new RegExp(`const ${name}: usize\\s*=\\s*(?:dclutch_resolution_codec::PROVIDER_UPDATE_LIFECYCLE_BYTES_V3|([0-9_]+));`));
  if (match === null) throw new Error(`Rust Source-provider owner does not expose ${name}`);
  if (match[1] !== undefined) return Number(match[1].replaceAll('_', ''));
  // The protocol codec freezes this public constant; ask rustc's source owner
  // rather than maintaining a second literal here.
  const codec = readFileSync(join(root, 'crates/dclutch-resolution-codec/src/provider_transport_v3.rs'), 'utf8');
  const value = codec.match(/pub const PROVIDER_UPDATE_LIFECYCLE_BYTES_V3: usize = ([0-9_]+);/);
  if (value === null) throw new Error('Resolution codec does not expose provider lifecycle bytes');
  return Number(value[1].replaceAll('_', ''));
}

try {
  // Shared with the other wasm generators when the caller names a directory
  // (`tools/ci/run.sh abi` does, so its eight builds pay one crate closure
  // between them); private, cold and deleted on the way out otherwise, which
  // is the right default for a lane running this one by hand in a checkout a
  // dozen other lanes are also building in.
  const target = process.env.DCLUTCH_WASM_TARGET_DIR ?? join(temporary, 'target');
  execFileSync('cargo', ['build', '-p', crate, '--target', 'wasm32-unknown-unknown', '--release', '--lib'], {
    cwd: root, env: { ...process.env, CARGO_TARGET_DIR: target }, stdio: 'inherit',
  });
  const built = join(target, 'wasm32-unknown-unknown/release/dclutch_source_provider_wasm.wasm');
  const generated = join(temporary, 'bindgen');
  execFileSync('wasm-bindgen', ['--target', 'web', '--out-dir', generated, '--out-name', 'source_provider', built], { cwd: root, stdio: 'inherit' });
  const owner = readFileSync(wire, 'utf8');
  const wasm = readFileSync(join(generated, 'source_provider_bg.wasm'));
  const digest = createHash('sha256').update(wasm).digest('hex');
  const generatedFacts = `// @generated from the authoritative Rust Source-provider owner and WASM artifact; do not edit.\n`+
    `export const SOURCE_PROVIDER_RECLAIM_INPUT_FORMAT_V1 = '${rustString(owner, 'RECLAIM_INPUT_FORMAT_V1')}' as const;\n`+
    `export const SOURCE_PROVIDER_PLAN_FORMAT_V1 = '${rustString(owner, 'PLAN_FORMAT_V1')}' as const;\n`+
    `export const SOURCE_PROVIDER_COORDINATES_INPUT_FORMAT_V1 = '${rustString(owner, 'COORDINATES_INPUT_FORMAT_V1')}' as const;\n`+
    `export const SOURCE_PROVIDER_COORDINATES_FORMAT_V1 = '${rustString(owner, 'COORDINATES_FORMAT_V1')}' as const;\n`+
    `export const SOURCE_PROVIDER_PROGRAM_INPUT_FORMAT_V1 = '${rustString(owner, 'PROGRAM_INPUT_FORMAT_V1')}' as const;\n`+
    `export const SOURCE_PROVIDER_PROGRAM_FORMAT_V1 = '${rustString(owner, 'PROGRAM_FORMAT_V1')}' as const;\n`+
    `export const SOURCE_PROVIDER_SUBMIT_BASE_INPUT_FORMAT_V1 = '${rustString(owner, 'SUBMIT_BASE_INPUT_FORMAT_V1')}' as const;\n`+
    `export const SOURCE_PROVIDER_SUBMIT_BASE_FORMAT_V1 = '${rustString(owner, 'SUBMIT_BASE_FORMAT_V1')}' as const;\n`+
    `export const SOURCE_PROVIDER_SUBMIT_MATERIAL_INPUT_FORMAT_V1 = '${rustString(owner, 'SUBMIT_MATERIAL_INPUT_FORMAT_V1')}' as const;\n`+
    `export const SOURCE_PROVIDER_SUBMIT_MATERIAL_FORMAT_V1 = '${rustString(owner, 'SUBMIT_MATERIAL_FORMAT_V1')}' as const;\n`+
    `export const SOURCE_PROVIDER_SUBMIT_RECORD_INPUT_FORMAT_V1 = '${rustString(owner, 'SUBMIT_RECORD_INPUT_FORMAT_V1')}' as const;\n`+
    `export const SOURCE_PROVIDER_SUBMIT_RECORD_FORMAT_V1 = '${rustString(owner, 'SUBMIT_RECORD_FORMAT_V1')}' as const;\n`+
    `export const SOURCE_PROVIDER_SUBMIT_PYTH_INPUT_FORMAT_V1 = '${rustString(owner, 'SUBMIT_PYTH_INPUT_FORMAT_V1')}' as const;\n`+
    `export const SOURCE_PROVIDER_SUBMIT_PYTH_FORMAT_V1 = '${rustString(owner, 'SUBMIT_PYTH_FORMAT_V1')}' as const;\n`+
    `export const SOURCE_PROVIDER_PRICE_INPUT_FORMAT_V1 = '${rustString(owner, 'PRICE_INPUT_FORMAT_V1')}' as const;\n`+
    `export const SOURCE_PROVIDER_PRICE_FORMAT_V1 = '${rustString(owner, 'PRICE_FORMAT_V1')}' as const;\n`+
    `export const SOURCE_PROVIDER_SUBMIT_FRESH_INPUT_FORMAT_V1 = '${rustString(owner, 'SUBMIT_FRESH_INPUT_FORMAT_V1')}' as const;\n`+
    `export const SOURCE_PROVIDER_SUBMIT_FRESH_FORMAT_V1 = '${rustString(owner, 'SUBMIT_FRESH_FORMAT_V1')}' as const;\n`+
    `export const SOURCE_PROVIDER_SUBMIT_INPUT_FORMAT_V1 = '${rustString(owner, 'SUBMIT_INPUT_FORMAT_V1')}' as const;\n`+
    `export const SOURCE_PROVIDER_SUBMIT_PLAN_FORMAT_V1 = '${rustString(owner, 'SUBMIT_PLAN_FORMAT_V1')}' as const;\n`+
    `export const SOURCE_PROVIDER_SUBMIT_POSTSTATE_INPUT_FORMAT_V1 = '${rustString(owner, 'SUBMIT_POSTSTATE_INPUT_FORMAT_V1')}' as const;\n`+
    `export const SOURCE_PROVIDER_SUBMIT_POSTSTATE_FORMAT_V1 = '${rustString(owner, 'SUBMIT_POSTSTATE_FORMAT_V1')}' as const;\n`+
    `export const SOURCE_PROVIDER_SUBMIT_LIFECYCLE_BYTES_V1 = ${rustNumber(owner, 'SUBMIT_LIFECYCLE_BYTES_V1')} as const;\n`+
    `export const SOURCE_PROVIDER_WASM_SHA256_V1 = '${digest}' as const;\n`+
    `export const SOURCE_PROVIDER_WASM_BYTES_V1 = ${wasm.length} as const;\n`;
  if (check) {
    if (readFileSync(facts, 'utf8') !== generatedFacts) throw new Error('generated Source-provider facts differ');
    if (readFileSync(sdkFacts, 'utf8') !== generatedFacts) throw new Error('generated SDK Source-provider facts differ');
    for (const name of ['source_provider.js', 'source_provider.d.ts', 'source_provider_bg.wasm', 'source_provider_bg.wasm.d.ts']) {
      if (!readFileSync(join(output, name)).equals(readFileSync(join(generated, name)))) throw new Error(`generated ${name} differs`);
    }
  } else {
    mkdirSync(output, { recursive: true });
    for (const name of ['source_provider.js', 'source_provider.d.ts', 'source_provider_bg.wasm', 'source_provider_bg.wasm.d.ts']) {
      writeFileSync(join(output, name), readFileSync(join(generated, name)));
    }
    writeFileSync(facts, generatedFacts);
    mkdirSync(dirname(sdkFacts), { recursive: true });
    writeFileSync(sdkFacts, generatedFacts);
  }
} finally {
  rmSync(temporary, { recursive: true, force: true });
}
