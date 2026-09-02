/**
 * Emit the browser's Product V2 payoff evaluator from its Rust owner.
 *
 * `lib/productV2.ts` used to carry `evaluateProductV2`, a hand-written
 * TypeScript reimplementation of `ProductPayoffV2::evaluate_rational` with its
 * own ramp and its own rational comparison, and the Studio drew a payout curve
 * out of it. Two authorities for one piece of exact arithmetic, and only one of
 * them is the arithmetic the on-chain family links. The lane that shipped the
 * range-protection check refused to fix a mirror by building a second mirror
 * and left this one named "untouched, unexcused". This is the removal.
 *
 * Everything emitted here comes from Rust: the two JSON schema names are read
 * out of the wasm crate's own `const`s, and the crate additionally pins the
 * record width, magic, version and shape caps at compile time with
 * `const _: () = assert!(...)` read BY CONSTANT NAME from the codec — so a
 * rename or a resize fails the build rather than quietly producing a boundary
 * that decodes a record the chain would refuse.
 *
 * The record's own coordinates are NOT re-emitted here. `lib/generated/
 * productV2Payoff.ts` already carries them from the same codec, and a second
 * copy of a number is the defect this whole module exists to remove.
 *
 * Usage:
 *   node scripts/generate-product-payoff-v2-wasm.mjs           # regenerate
 *   node scripts/generate-product-payoff-v2-wasm.mjs --check   # verify only
 */
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = fileURLToPath(new URL('.', import.meta.url));
const app = resolve(here, '..');
const root = resolve(app, '../..');
const wasmOwner = join(root, 'crates/dclutch-product-payoff-v2-wasm/src/lib.rs');
const crate = 'dclutch-product-payoff-v2-wasm';
const output = join(app, 'lib/generated/productPayoffV2Wasm');
const facts = join(app, 'lib/generated/productPayoffV2WasmV1.ts');
const check = process.argv.includes('--check');
const ARTIFACTS = [
  'product_payoff_v2.js',
  'product_payoff_v2.d.ts',
  'product_payoff_v2_bg.wasm',
  'product_payoff_v2_bg.wasm.d.ts',
];

function rustString(source, name) {
  const match = source.match(new RegExp(`const ${name}: &str = "([^"]+)";`));
  if (match === null) throw new Error(`the payoff WASM owner does not expose ${name}`);
  return match[1];
}

function rustUsize(source, name) {
  const match = source.match(new RegExp(`const ${name}: usize = ([0-9_]+);`));
  if (match === null) throw new Error(`the payoff WASM owner does not expose a usize ${name}`);
  return Number(match[1].replaceAll('_', ''));
}

const temporary = mkdtempSync(join(tmpdir(), 'dclutch-payoff-wasm.'));
try {
  const target = join(temporary, 'target');
  execFileSync('cargo', ['build', '-p', crate, '--target', 'wasm32-unknown-unknown', '--release'], {
    cwd: root, env: { ...process.env, CARGO_TARGET_DIR: target }, stdio: 'inherit',
  });
  const built = join(target, 'wasm32-unknown-unknown/release/dclutch_product_payoff_v2_wasm.wasm');
  const generated = join(temporary, 'bindgen');
  execFileSync('wasm-bindgen', ['--target', 'web', '--out-dir', generated, '--out-name', 'product_payoff_v2', built], { cwd: root, stdio: 'inherit' });

  const owner = readFileSync(wasmOwner, 'utf8');
  const wasm = readFileSync(join(generated, 'product_payoff_v2_bg.wasm'));
  const digest = createHash('sha256').update(wasm).digest('hex');

  const generatedFacts = '// @generated from the authoritative Rust Product V2 payoff evaluator and WASM artifact; do not edit.\n'
    + '// Regenerate with: npm run abi:product-payoff-v2-wasm\n'
    + `export const PRODUCT_PAYOFF_V2_REQUEST_FORMAT_V1 = '${rustString(owner, 'REQUEST_FORMAT_V1')}' as const;\n`
    + `export const PRODUCT_PAYOFF_V2_RESPONSE_FORMAT_V1 = '${rustString(owner, 'RESPONSE_FORMAT_V1')}' as const;\n`
    + `export const PRODUCT_PAYOFF_V2_MAX_COORDINATES_V1 = ${rustUsize(owner, 'MAX_COORDINATES_V1')} as const;\n`
    + `export const PRODUCT_PAYOFF_V2_WASM_SHA256_V1 = '${digest}' as const;\n`
    + `export const PRODUCT_PAYOFF_V2_WASM_BYTES_V1 = ${wasm.length} as const;\n`;

  if (check) {
    if (readFileSync(facts, 'utf8') !== generatedFacts) throw new Error('generated payoff evaluator facts differ');
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
