import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const app = resolve(here, '..');
const root = resolve(app, '../..');
const wire = join(root, 'crates/dclutch-source-readiness-operator/src/wire.rs');
const crate = 'dclutch-source-readiness-wasm';
const output = join(app, 'lib/generated/sourceReadinessWasm');
const facts = join(app, 'lib/generated/sourceReadinessWasmV1.ts');
const sdkFacts = join(root, 'packages/dclutch-sdk/lib/generated/sourceReadinessWasmV1.ts');
const check = process.argv.includes('--check');
const temporary = mkdtempSync(join(tmpdir(), 'dclutch-source-wasm.'));

function rustString(source, name) {
  const match = source.match(new RegExp(`const ${name}: &str = "([^"]+)";`));
  if (match === null) throw new Error(`Rust Source-readiness owner does not expose ${name}`);
  return match[1];
}

try {
  // Shared with the other wasm generators when the caller names a directory
  // (`tools/ci/run.sh abi` does, so its eight builds pay one crate closure
  // between them); private, cold and deleted on the way out otherwise, which
  // is the right default for a lane running this one by hand in a checkout a
  // dozen other lanes are also building in.
  const target = process.env.DCLUTCH_WASM_TARGET_DIR ?? join(temporary, 'target');
  execFileSync('cargo', ['build', '-p', crate, '--target', 'wasm32-unknown-unknown', '--release'], {
    cwd: root, env: { ...process.env, CARGO_TARGET_DIR: target }, stdio: 'inherit',
  });
  const built = join(target, 'wasm32-unknown-unknown/release/dclutch_source_readiness_wasm.wasm');
  const generated = join(temporary, 'bindgen');
  execFileSync('wasm-bindgen', ['--target', 'web', '--out-dir', generated, '--out-name', 'source_readiness', built], { cwd: root, stdio: 'inherit' });

  const owner = readFileSync(wire, 'utf8');
  const wasm = readFileSync(join(generated, 'source_readiness_bg.wasm'));
  const digest = createHash('sha256').update(wasm).digest('hex');
  const generatedFacts = `// @generated from the authoritative Rust Source-readiness owner and WASM artifact; do not edit.\n`+
    `export const SOURCE_READINESS_SNAPSHOT_FORMAT_V1 = '${rustString(owner, 'SNAPSHOT_FORMAT_V1')}' as const;\n`+
    `export const SOURCE_READINESS_PLAN_FORMAT_V1 = '${rustString(owner, 'PLAN_FORMAT_V1')}' as const;\n`+
    `export const SOURCE_READINESS_MARKET_FORMAT_V1 = '${rustString(owner, 'MARKET_FORMAT_V1')}' as const;\n`+
    `export const SOURCE_READINESS_RECORDS_FORMAT_V1 = '${rustString(owner, 'RECORDS_FORMAT_V1')}' as const;\n`+
    `export const SOURCE_READINESS_SOURCE_FORMAT_V1 = '${rustString(owner, 'SOURCE_FORMAT_V1')}' as const;\n`+
    `export const SOURCE_TERMINAL_BASE_FORMAT_V1 = '${rustString(owner, 'TERMINAL_BASE_FORMAT_V1')}' as const;\n`+
    `export const SOURCE_TERMINAL_PRODUCT_FORMAT_V1 = '${rustString(owner, 'TERMINAL_PRODUCT_FORMAT_V1')}' as const;\n`+
    `export const SOURCE_TERMINAL_DETAIL_FORMAT_V1 = '${rustString(owner, 'TERMINAL_DETAIL_FORMAT_V1')}' as const;\n`+
    `export const SOURCE_TERMINAL_SNAPSHOT_FORMAT_V1 = '${rustString(owner, 'TERMINAL_SNAPSHOT_FORMAT_V1')}' as const;\n`+
    `export const SOURCE_TERMINAL_PLAN_FORMAT_V1 = '${rustString(owner, 'TERMINAL_PLAN_FORMAT_V1')}' as const;\n`+
    `export const SOURCE_CLOSE_DETAIL_FORMAT_V1 = '${rustString(owner, 'CLOSE_DETAIL_FORMAT_V1')}' as const;\n`+
    `export const SOURCE_CLOSE_SNAPSHOT_FORMAT_V1 = '${rustString(owner, 'CLOSE_SNAPSHOT_FORMAT_V1')}' as const;\n`+
    `export const SOURCE_CLOSE_PLAN_FORMAT_V1 = '${rustString(owner, 'CLOSE_PLAN_FORMAT_V1')}' as const;\n`+
    `export const SOURCE_CLOSE_VERIFY_FORMAT_V1 = '${rustString(owner, 'CLOSE_VERIFY_FORMAT_V1')}' as const;\n`+
    `export const SOURCE_READINESS_WASM_SHA256_V1 = '${digest}' as const;\n`+
    `export const SOURCE_READINESS_WASM_BYTES_V1 = ${wasm.length} as const;\n`;

  if (check) {
    if (readFileSync(facts, 'utf8') !== generatedFacts) throw new Error('generated Source-readiness facts differ');
    if (readFileSync(sdkFacts, 'utf8') !== generatedFacts) throw new Error('generated SDK Source-readiness facts differ');
    for (const name of ['source_readiness.js', 'source_readiness.d.ts', 'source_readiness_bg.wasm', 'source_readiness_bg.wasm.d.ts']) {
      if (!readFileSync(join(output, name)).equals(readFileSync(join(generated, name)))) throw new Error(`generated ${name} differs`);
    }
  } else {
    mkdirSync(output, { recursive: true });
    for (const name of ['source_readiness.js', 'source_readiness.d.ts', 'source_readiness_bg.wasm', 'source_readiness_bg.wasm.d.ts']) {
      writeFileSync(join(output, name), readFileSync(join(generated, name)));
    }
    writeFileSync(facts, generatedFacts);
    mkdirSync(dirname(sdkFacts), { recursive: true });
    writeFileSync(sdkFacts, generatedFacts);
  }
} finally {
  rmSync(temporary, { recursive: true, force: true });
}
