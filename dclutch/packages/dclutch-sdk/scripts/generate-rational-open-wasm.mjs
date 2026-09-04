import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const sdk = resolve(here, '..');
const root = resolve(sdk, '../..');
const wire = join(root, 'crates/dclutch-rational-open-wasm/src/wire.rs');
const requestOwner = join(root, 'crates/dclutch-rational-representation-v2-request-contract/src/open_hot_v3.rs');
const parityFixture = join(root, 'crates/dclutch-rational-open-wasm/fixtures/issue-structured-v1.json');
const crate = 'dclutch-rational-open-wasm';
const output = join(sdk, 'lib/generated/rationalOpenWasm');
const facts = join(sdk, 'lib/generated/rationalOpenWasmV1.ts');
const check = process.argv.includes('--check');
const temporary = mkdtempSync(join(tmpdir(), 'dclutch-rational-open-wasm.'));

function rustString(source, name) {
  const match = source.match(new RegExp(`pub const ${name}: &str\\s*=\\s*"([^"]+)";`));
  if (match === null) throw new Error(`Rust Rational-open owner does not expose ${name}`);
  return match[1];
}

function rustByteArrayHex(source, name, width) {
  const match = source.match(new RegExp(`pub const ${name}: \\[u8; ${width}\\]\\s*=\\s*\\[([\\s\\S]*?)\\];`));
  if (match === null) throw new Error(`Rust Rational-open request owner does not expose ${name}`);
  const values = [...match[1].matchAll(/0x([0-9a-fA-F]{2})/g)].map((entry) => entry[1].toLowerCase());
  if (values.length !== width) throw new Error(`Rust ${name} has ${values.length} bytes, expected ${width}`);
  return values.join('');
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
  const built = join(target, 'wasm32-unknown-unknown/release/dclutch_rational_open_wasm.wasm');
  const generated = join(temporary, 'bindgen');
  execFileSync('wasm-bindgen', ['--target', 'web', '--out-dir', generated, '--out-name', 'rational_open', built], { cwd: root, stdio: 'inherit' });
  const owner = readFileSync(wire, 'utf8');
  const request = readFileSync(requestOwner, 'utf8');
  const parityInput = readFileSync(parityFixture, 'utf8').trim();
  const parityOutput = execFileSync('cargo', ['run', '--quiet', '-p', crate, '--bin', 'rational-open-native-v1'], {
    cwd: root, input: parityInput, encoding: 'utf8',
  });
  JSON.parse(parityInput);
  JSON.parse(parityOutput);
  const wasm = readFileSync(join(generated, 'rational_open_bg.wasm'));
  const digest = createHash('sha256').update(wasm).digest('hex');
  const generatedFacts = `// @generated from the canonical Rust Rational-open owner and WASM artifact; do not edit.\n`+
    `export const RATIONAL_OPEN_INPUT_FORMAT_V1 = '${rustString(owner, 'RATIONAL_OPEN_INPUT_FORMAT_V1')}' as const;\n`+
    `export const RATIONAL_OPEN_PLAN_FORMAT_V1 = '${rustString(owner, 'RATIONAL_OPEN_PLAN_FORMAT_V1')}' as const;\n`+
    `export const RATIONAL_OPEN_REQUEST_SCHEMA_HEX_V3 = '${rustByteArrayHex(request, 'OPEN_REPRESENTATION_HOT_REQUEST_SCHEMA_ID_V3', 32)}' as const;\n`+
    `export const RATIONAL_OPEN_WASM_SHA256_V1 = '${digest}' as const;\n`+
    `export const RATIONAL_OPEN_WASM_BYTES_V1 = ${wasm.length} as const;\n`+
    `export const RATIONAL_OPEN_NATIVE_PARITY_INPUT_V1 = ${JSON.stringify(parityInput)} as const;\n`+
    `export const RATIONAL_OPEN_NATIVE_PARITY_OUTPUT_V1 = ${JSON.stringify(parityOutput)} as const;\n`;
  if (check) {
    if (readFileSync(facts, 'utf8') !== generatedFacts) throw new Error('generated Rational-open facts differ');
    for (const name of ['rational_open.js', 'rational_open.d.ts', 'rational_open_bg.wasm', 'rational_open_bg.wasm.d.ts']) {
      if (!readFileSync(join(output, name)).equals(readFileSync(join(generated, name)))) throw new Error(`generated ${name} differs`);
    }
  } else {
    mkdirSync(output, { recursive: true });
    for (const name of ['rational_open.js', 'rational_open.d.ts', 'rational_open_bg.wasm', 'rational_open_bg.wasm.d.ts']) {
      writeFileSync(join(output, name), readFileSync(join(generated, name)));
    }
    writeFileSync(facts, generatedFacts);
  }
} finally {
  rmSync(temporary, { recursive: true, force: true });
}
