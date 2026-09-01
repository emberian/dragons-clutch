/**
 * Emit the browser's wallet-terminal payout derivation from its Rust owner.
 *
 * `RedeemFlow` says "This browser never creates or completes a payout plan"
 * and asks a reader to import JSON that `dclutch-local-successor-bootstrap`
 * emits. That made redemption the last of the three capabilities in "the
 * browser can sign but cannot originate". The answer is the one that worked
 * twice already: compile the derivation rather than grow a second one here.
 *
 * The derivation was extracted verbatim into
 * `crates/dclutch-wallet-terminal-payout-operator` and the binary kept its
 * shell, so what this builds is the SAME code the operator toolchain runs.
 *
 * Everything emitted comes from Rust: the two JSON schema names are read out
 * of the WASM crate's own `const`s, and the settlement frame width and request
 * width are read out of CLAIMS. The crate additionally pins both at compile
 * time with `const _: () = assert!(...)`, so a rename or a resize fails the
 * build instead of quietly producing a thirty-five-account frame the runtime
 * refuses with no useful reason.
 *
 * Usage:
 *   node scripts/generate-wallet-terminal-payout-wasm.mjs           # regenerate
 *   node scripts/generate-wallet-terminal-payout-wasm.mjs --check   # verify only
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
const wasmOwner = join(root, 'crates/dclutch-wallet-terminal-payout-wasm/src/lib.rs');
const claims = join(root, 'crates/dclutch-claims-svm/src/terminal_settlement_v3.rs');
const operator = join(root, 'crates/dclutch-wallet-terminal-payout-operator/src/wire.rs');
const crate = 'dclutch-wallet-terminal-payout-wasm';
const output = join(app, 'lib/generated/walletTerminalPayoutWasm');
const facts = join(app, 'lib/generated/walletTerminalPayoutWasmV1.ts');
const check = process.argv.includes('--check');
const ARTIFACTS = [
  'wallet_terminal_payout.js',
  'wallet_terminal_payout.d.ts',
  'wallet_terminal_payout_bg.wasm',
  'wallet_terminal_payout_bg.wasm.d.ts',
];

function rustString(source, name) {
  const match = source.match(new RegExp(`const ${name}: &str = "([^"]+)";`));
  if (match === null) throw new Error(`the payout WASM owner does not expose ${name}`);
  return match[1];
}

function rustUsize(source, name) {
  const match = source.match(new RegExp(`const ${name}: usize = ([0-9_]+);`));
  if (match === null) throw new Error(`Claims does not expose a usize ${name}`);
  return Number(match[1].replaceAll('_', ''));
}

const temporary = mkdtempSync(join(tmpdir(), 'dclutch-payout-wasm.'));
try {
  const target = join(temporary, 'target');
  execFileSync('cargo', ['build', '-p', crate, '--target', 'wasm32-unknown-unknown', '--release'], {
    cwd: root, env: { ...process.env, CARGO_TARGET_DIR: target }, stdio: 'inherit',
  });
  const built = join(target, 'wasm32-unknown-unknown/release/dclutch_wallet_terminal_payout_wasm.wasm');
  const generated = join(temporary, 'bindgen');
  execFileSync('wasm-bindgen', ['--target', 'web', '--out-dir', generated, '--out-name', 'wallet_terminal_payout', built], { cwd: root, stdio: 'inherit' });

  const owner = readFileSync(wasmOwner, 'utf8');
  const claimsSource = readFileSync(claims, 'utf8');
  const operatorSource = readFileSync(operator, 'utf8');
  const wasm = readFileSync(join(generated, 'wallet_terminal_payout_bg.wasm'));
  const digest = createHash('sha256').update(wasm).digest('hex');

  const generatedFacts = '// @generated from the authoritative Rust payout derivation and WASM artifact; do not edit.\n'
    + '// Regenerate with: npm run abi:wallet-terminal-payout\n'
    + `export const WALLET_TERMINAL_PAYOUT_SNAPSHOT_FORMAT_V1 = '${rustString(owner, 'SNAPSHOT_FORMAT_V1')}' as const;\n`
    + `export const WALLET_TERMINAL_PAYOUT_ADDRESSES_FORMAT_V1 = '${rustString(owner, 'ADDRESSES_FORMAT_V1')}' as const;\n`
    // The stage-one artifact a reader still imports. Read from the operator
    // crate so the browser can recognise it without writing its name down.
    + `export const WALLET_TERMINAL_PAYOUT_INPUT_FORMAT_V1 = '${rustString(operatorSource, 'INPUT_FORMAT')}' as const;\n`
    + `export const TERMINAL_SETTLEMENT_ACCOUNT_COUNT_V3 = ${rustUsize(claimsSource, 'TERMINAL_SETTLEMENT_ACCOUNT_COUNT_V3')} as const;\n`
    + `export const TERMINAL_SETTLEMENT_REQUEST_BYTES_V3 = ${rustUsize(claimsSource, 'TERMINAL_SETTLEMENT_REQUEST_BYTES_V3')} as const;\n`
    + `export const WALLET_TERMINAL_PAYOUT_WASM_SHA256_V1 = '${digest}' as const;\n`
    + `export const WALLET_TERMINAL_PAYOUT_WASM_BYTES_V1 = ${wasm.length} as const;\n`;

  if (check) {
    if (readFileSync(facts, 'utf8') !== generatedFacts) throw new Error('generated payout facts differ');
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
