/**
 * Emit the browser's User Position admission planner from its Rust owner.
 *
 * Admission is the step that turns a wallet into a market participant, and
 * `JoinPanel` used to say plainly that the browser could not compose it: the
 * frame "needs the position owner's signature over a frame the browser cannot
 * yet assemble byte-exactly". Twenty-seven accounts, per-coordinate
 * privileges, two rent deficits and a predicted Claims receipt, reimplemented
 * in TypeScript, is the mirror this repository keeps convicting. So the
 * planner is compiled rather than copied, exactly as the Source-readiness
 * owner is.
 *
 * Everything this emits comes from the Rust: the two JSON schema names are
 * read out of the wasm crate's own `const`s, and the frame width and outer
 * selector are read out of the ADMISSION CONTRACT rather than restated here.
 * The crate additionally pins both at compile time with `const _: () =
 * assert!(...)`, so a rename or a resize fails the build instead of quietly
 * producing a twenty-six-account frame the runtime refuses with no useful
 * reason.
 *
 * Usage:
 *   node scripts/generate-user-position-admission-wasm.mjs           # regenerate
 *   node scripts/generate-user-position-admission-wasm.mjs --check   # verify only
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
const wasmOwner = join(root, 'crates/dclutch-user-position-admission-wasm/src/lib.rs');
const contract = join(root, 'crates/dclutch-user-position-admission-contract/src/lib.rs');
const crate = 'dclutch-user-position-admission-wasm';
const output = join(app, 'lib/generated/userPositionAdmissionWasm');
const facts = join(app, 'lib/generated/userPositionAdmissionWasmV1.ts');
const check = process.argv.includes('--check');
const ARTIFACTS = [
  'user_position_admission.js',
  'user_position_admission.d.ts',
  'user_position_admission_bg.wasm',
  'user_position_admission_bg.wasm.d.ts',
];

function rustString(source, name) {
  const match = source.match(new RegExp(`const ${name}: &str = "([^"]+)";`));
  if (match === null) throw new Error(`the admission WASM owner does not expose ${name}`);
  return match[1];
}

/** A `usize` const read from the contract, following one alias hop. */
function rustUsize(source, name) {
  const literal = source.match(new RegExp(`const ${name}: usize =\\s*([0-9]+);`));
  if (literal !== null) return Number(literal[1]);
  const sum = source.match(new RegExp(`const ${name}: usize =\\s*([A-Z][A-Z0-9_]*)\\s*\\+\\s*([A-Z][A-Z0-9_]*);`));
  if (sum !== null) return rustUsize(source, sum[1]) + rustUsize(source, sum[2]);
  throw new Error(`the admission contract does not expose a usize ${name}`);
}

const temporary = mkdtempSync(join(tmpdir(), 'dclutch-admission-wasm.'));
try {
  // Shared with the other wasm generators when the caller names a directory
  // (`tools/ci/run.sh abi` does, so its eight builds pay one crate closure
  // between them); private, cold and deleted on the way out otherwise, which
  // is the right default for a lane running this one by hand in a checkout a
  // dozen other lanes are also building in.
  const target = process.env.DCLUTCH_WASM_TARGET_DIR ?? join(temporary, 'target');
  // `--no-default-features` is load-bearing and not an optimisation. The
  // default `dealer-series` feature links `dclutch-trading-sbf`, which pins
  // its layouts for a 64-bit target with `const _: [(); N] = [(); size_of]`;
  // on wasm32 those assertions fail because pointers are half the width. They
  // are correct for the program's real target, and the admission planner does
  // not need the program at all.
  execFileSync('cargo', ['build', '-p', crate, '--target', 'wasm32-unknown-unknown', '--release'], {
    cwd: root, env: { ...process.env, CARGO_TARGET_DIR: target }, stdio: 'inherit',
  });
  const built = join(target, 'wasm32-unknown-unknown/release/dclutch_user_position_admission_wasm.wasm');
  const generated = join(temporary, 'bindgen');
  execFileSync('wasm-bindgen', ['--target', 'web', '--out-dir', generated, '--out-name', 'user_position_admission', built], { cwd: root, stdio: 'inherit' });

  const owner = readFileSync(wasmOwner, 'utf8');
  const contractSource = readFileSync(contract, 'utf8');
  const wasm = readFileSync(join(generated, 'user_position_admission_bg.wasm'));
  const digest = createHash('sha256').update(wasm).digest('hex');
  const accountCount = rustUsize(contractSource, 'USER_POSITION_ADMISSION_ACCOUNT_COUNT_V1');
  const ownerAccount = rustUsize(contractSource, 'USER_POSITION_ADMISSION_OWNER_ACCOUNT_V1');
  const authorityAccount = rustUsize(contractSource, 'USER_POSITION_ADMISSION_AUTHORITY_ACCOUNT_V1');

  const generatedFacts = '// @generated from the authoritative Rust admission planner and WASM artifact; do not edit.\n'
    + '// Regenerate with: npm run abi:user-position-admission\n'
    + `export const USER_POSITION_ADMISSION_SNAPSHOT_FORMAT_V1 = '${rustString(owner, 'SNAPSHOT_FORMAT_V1')}' as const;\n`
    + `export const USER_POSITION_ADMISSION_PLAN_FORMAT_V1 = '${rustString(owner, 'PLAN_FORMAT_V1')}' as const;\n`
    + `export const USER_POSITION_ADMISSION_ACCOUNT_COUNT_V1 = ${accountCount} as const;\n`
    + `export const USER_POSITION_ADMISSION_OWNER_ACCOUNT_V1 = ${ownerAccount} as const;\n`
    + `export const USER_POSITION_ADMISSION_AUTHORITY_ACCOUNT_V1 = ${authorityAccount} as const;\n`
    + `export const USER_POSITION_ADMISSION_WASM_SHA256_V1 = '${digest}' as const;\n`
    + `export const USER_POSITION_ADMISSION_WASM_BYTES_V1 = ${wasm.length} as const;\n`;

  if (check) {
    if (readFileSync(facts, 'utf8') !== generatedFacts) throw new Error('generated admission facts differ');
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
