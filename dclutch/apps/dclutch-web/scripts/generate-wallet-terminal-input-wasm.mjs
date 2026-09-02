/**
 * Emit the browser's wallet-terminal payout INPUT derivation from its Rust owner.
 *
 * Stage two — the payout manifest — reached the browser in `eed52c57`, and the
 * reader still had to import the JSON that
 * `dclutch-local-successor-bootstrap wallet-terminal-payout-input` emits: the
 * last CLI command standing between a stranger and a redemption. The answer is
 * the one that has worked every time here — compile the derivation rather than
 * grow a second one in TypeScript.
 *
 * The three pure phases were extracted into
 * `crates/dclutch-wallet-terminal-input-operator` and the binary kept its shell
 * (arguments, two files, RPC, cluster policy), so what this builds is the SAME
 * code the operator toolchain runs.
 *
 * Everything emitted comes from Rust: the three JSON schema names are read out
 * of the WASM crate's own `const`s, and the Core Market width and the two
 * Claims header widths are read out of their owners. The crate additionally
 * pins all three at compile time with `const _: () = assert!(...)`, so a
 * rename or a resize fails the build instead of quietly deriving a DIFFERENT
 * aggregate address.
 *
 * Usage:
 *   node scripts/generate-wallet-terminal-input-wasm.mjs           # regenerate
 *   node scripts/generate-wallet-terminal-input-wasm.mjs --check   # verify only
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
const wasmOwner = join(root, 'crates/dclutch-wallet-terminal-input-wasm/src/lib.rs');
const claims = join(root, 'crates/dclutch-claims-svm/src/liability_basis_state_v2.rs');
const core = join(root, 'crates/dclutch-market-core-codec/src/generated.rs');
const crate = 'dclutch-wallet-terminal-input-wasm';
const output = join(app, 'lib/generated/walletTerminalInputWasm');
const facts = join(app, 'lib/generated/walletTerminalInputWasmV1.ts');
const check = process.argv.includes('--check');
const ARTIFACTS = [
  'wallet_terminal_input.js',
  'wallet_terminal_input.d.ts',
  'wallet_terminal_input_bg.wasm',
  'wallet_terminal_input_bg.wasm.d.ts',
];

function rustString(source, name) {
  const match = source.match(new RegExp(`const ${name}: &str = "([^"]+)";`));
  if (match === null) throw new Error(`the payout input WASM owner does not expose ${name}`);
  return match[1];
}

function rustUsize(source, name, owner) {
  const match = source.match(new RegExp(`const ${name}: usize = ([0-9_]+);`));
  if (match === null) throw new Error(`${owner} does not expose a usize ${name}`);
  return Number(match[1].replaceAll('_', ''));
}

const temporary = mkdtempSync(join(tmpdir(), 'dclutch-input-wasm.'));
try {
  const target = join(temporary, 'target');
  execFileSync('cargo', ['build', '-p', crate, '--target', 'wasm32-unknown-unknown', '--release'], {
    cwd: root, env: { ...process.env, CARGO_TARGET_DIR: target }, stdio: 'inherit',
  });
  const built = join(target, 'wasm32-unknown-unknown/release/dclutch_wallet_terminal_input_wasm.wasm');
  const generated = join(temporary, 'bindgen');
  execFileSync('wasm-bindgen', ['--target', 'web', '--out-dir', generated, '--out-name', 'wallet_terminal_input', built], { cwd: root, stdio: 'inherit' });

  const owner = readFileSync(wasmOwner, 'utf8');
  const claimsSource = readFileSync(claims, 'utf8');
  const coreSource = readFileSync(core, 'utf8');
  const wasm = readFileSync(join(generated, 'wallet_terminal_input_bg.wasm'));
  const digest = createHash('sha256').update(wasm).digest('hex');

  const generatedFacts = '// @generated from the authoritative Rust payout input derivation and WASM artifact; do not edit.\n'
    + '// Regenerate with: npm run abi:wallet-terminal-input\n'
    + `export const WALLET_TERMINAL_INPUT_REQUEST_FORMAT_V1 = '${rustString(owner, 'REQUEST_FORMAT_V1')}' as const;\n`
    + `export const WALLET_TERMINAL_INPUT_SNAPSHOT_FORMAT_V1 = '${rustString(owner, 'SNAPSHOT_FORMAT_V1')}' as const;\n`
    + `export const WALLET_TERMINAL_INPUT_ADDRESSES_FORMAT_V1 = '${rustString(owner, 'ADDRESSES_FORMAT_V1')}' as const;\n`
    + `export const CORE_STATE_BYTES_V1 = ${rustUsize(coreSource, 'STATE_BYTES', 'the Core state codec')} as const;\n`
    + `export const LIABILITY_BASIS_MARKET_HEADER_BYTES_V2 = ${rustUsize(claimsSource, 'LIABILITY_BASIS_MARKET_HEADER_BYTES_V2', 'Claims')} as const;\n`
    + `export const LIABILITY_BASIS_POSITION_HEADER_BYTES_V2 = ${rustUsize(claimsSource, 'LIABILITY_BASIS_POSITION_HEADER_BYTES_V2', 'Claims')} as const;\n`
    + `export const WALLET_TERMINAL_INPUT_WASM_SHA256_V1 = '${digest}' as const;\n`
    + `export const WALLET_TERMINAL_INPUT_WASM_BYTES_V1 = ${wasm.length} as const;\n`;

  if (check) {
    if (readFileSync(facts, 'utf8') !== generatedFacts) throw new Error('generated payout input facts differ');
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
