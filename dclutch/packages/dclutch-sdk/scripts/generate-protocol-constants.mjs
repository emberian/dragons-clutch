/**
 * Emit `lib/generated/protocolConstantsV1.ts`: the record magics and PDA seed
 * domains the client used to state in its own words, read from the Rust
 * constant that owns each one.
 *
 * One table, one scrape, one module. A row names the TypeScript export, the
 * Rust source file and the Rust constant; the generator reads the constant's
 * literal out of that file (`*b"..."`, `b"..."` or `"..."`, any visibility)
 * and refuses when the constant is missing, when a magic is not eight
 * uppercase alphanumerics, or when two rows disagree about one value. Seeds
 * are emitted as the bytes a PDA derivation takes; kinds and formats as the
 * text a JSON envelope carries.
 *
 * A fact an existing generated module already emits is NOT repeated here --
 * one author per fact -- so `PRODUCT_RECORD_MAGIC_V2`,
 * `PROTOCOL_POSITION_ADMISSION_SEED_V2`, `CALLER_AUTHORITY_PDA_DOMAIN_V1` and
 * `REGISTRY_ACTIVATION_PDA_DOMAIN_V1` are imported from theirs.
 *
 * Usage:
 *   node scripts/generate-protocol-constants.mjs           # regenerate
 *   node scripts/generate-protocol-constants.mjs --check   # verify only
 */
import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const root = new URL('../../../', import.meta.url);
const outputUrl = new URL('../lib/generated/protocolConstantsV1.ts', import.meta.url);

/** `[export name, Rust source (repo-relative), Rust constant, form]`. */
const ROWS = [
  // --- record and instruction magics ---------------------------------------
  ['ACTIVATION_CACHE_MAGIC_V1', 'crates/dclutch-registry/src/activation.rs', 'ACTIVATED_EXECUTION_RELEASE_SET_MAGIC_V1', 'magic'],
  ['ARTIFACT_RELEASE_MAGIC_V1', 'crates/dclutch-registry/src/artifact.rs', 'ARTIFACT_RELEASE_MAGIC_V1', 'magic'],
  ['CAPABILITY_SEAL_MAGIC_V1', 'crates/dclutch-vm/src/capability_seal/mod.rs', 'CAPABILITY_SEAL_MAGIC_V1', 'magic'],
  ['CHECKED_INFRASTRUCTURE_MAGIC_V1', 'crates/dclutch-release-tool/src/infrastructure.rs', 'CHECKED_INFRASTRUCTURE_MAGIC_V1', 'magic'],
  ['CHECKED_MULTIPROGRAM_MAGIC_V1', 'crates/dclutch-release-tool/src/multiprogram.rs', 'CHECKED_MULTIPROGRAM_MAGIC_V1', 'magic'],
  ['CHECKED_RELEASE_MAGIC_V1', 'crates/dclutch-release-tool/src/lib.rs', 'CHECKED_RELEASE_MAGIC_V1', 'magic'],
  ['EXECUTION_RELEASE_SET_MAGIC_V1', 'crates/dclutch-registry/src/release_set/mod.rs', 'EXECUTION_RELEASE_SET_MAGIC_V1', 'magic'],
  ['PRODUCT_RUNTIME_DOMAIN_MAGIC_V2', 'crates/dclutch-product/src/generated.rs', 'DOMAIN_MAGIC', 'magic'],
  ['PRODUCT_RUNTIME_PORTFOLIO_MAGIC_V2', 'crates/dclutch-product/src/generated.rs', 'PORTFOLIO_MAGIC', 'magic'],
  ['PYTH_SPONSORED_PUSH_RELEASE_MAGIC_V1', 'crates/dclutch-source/src/pyth/sponsored_push.rs', 'PYTH_SPONSORED_PUSH_RELEASE_V1_MAGIC', 'magic'],
  ['REGISTRY_INSTRUCTION_MAGIC_V1', 'crates/dclutch-registry/src/svm/mod.rs', 'REGISTRY_INSTRUCTION_MAGIC_V1', 'magic'],
  ['RELEASE_LINEAGE_MAGIC_V1', 'crates/dclutch-registry/src/lineage.rs', 'RELEASE_LINEAGE_MAGIC_V1', 'magic'],
  ['TOKEN_BEHAVIOR_SELECTION_MAGIC_V2', 'crates/dclutch-custody/src/token_svm/behavior_binding_v2.rs', 'TOKEN_BEHAVIOR_SELECTION_MAGIC_V2', 'magic'],
  // --- PDA seed domains, as the bytes a derivation takes -------------------
  ['CAPABILITY_SEAL_PDA_DOMAIN_V1', 'crates/dclutch-vm/src/capability_seal/mod.rs', 'CAPABILITY_SEAL_PDA_DOMAIN_V1', 'seed'],
  ['DIRECT_MAKER_REPLAY_PDA_DOMAIN_V1', 'crates/dclutch-trading/src/successor.rs', 'DIRECT_MAKER_REPLAY_PDA_DOMAIN_V1', 'seed'],
  ['PROTOCOL_POSITION_CLAIMS_CAPABILITY_SEED_V2', 'crates/dclutch-claims/src/protocol_position_v2.rs', 'PROTOCOL_POSITION_CLAIMS_CAPABILITY_SEED_V2', 'seed'],
  ['RATIONAL_RECEIPT_MINT_SEED_V2', 'crates/dclutch-claims/src/rational/seeds.rs', 'RATIONAL_RECEIPT_MINT_SEED_V2', 'seed'],
  ['RATIONAL_REPRESENTATION_AUTHORITY_SEED_V2', 'crates/dclutch-claims/src/rational_kernel/mod.rs', 'RATIONAL_REPRESENTATION_AUTHORITY_SEED_V2', 'seed'],
  ['RATIONAL_SHARD_MINT_SEED_V2', 'crates/dclutch-claims/src/rational/mod.rs', 'RATIONAL_SHARD_MINT_SEED_V2', 'seed'],
  ['RATIONAL_STRUCTURED_CUSTODY_SEED_V2', 'crates/dclutch-claims/src/rational/mod.rs', 'RATIONAL_STRUCTURED_CUSTODY_SEED_V2', 'seed'],
  ['RELEASE_LINEAGE_PDA_DOMAIN_V1', 'crates/dclutch-registry/src/lineage.rs', 'RELEASE_LINEAGE_PDA_DOMAIN_V1', 'seed'],
  // --- envelope kinds and formats, as the text a JSON document carries -----
  ['DIRECT_TICKET_KIND_V1', 'crates/dclutch-direct-ticket/src/envelope.rs', 'PORTABLE_DIRECT_TICKET_KIND_V1', 'text'],
  ['GENERAL_SUCCESSOR_PLAN_FORMAT_V5', 'crates/dclutch-operator/src/general_successor.rs', 'PLAN_FORMAT_V5', 'text'],
];

const sources = new Map();
function sourceOf(path) {
  if (!sources.has(path)) sources.set(path, readFileSync(new URL(path, root), 'utf8'));
  return sources.get(path);
}

/** The literal a Rust `const NAME: <type> = *b"..." | b"..." | "...";` declares, any visibility. */
function rustLiteral(path, name) {
  // The type may itself carry a semicolon (`[u8; 8]`), so it runs to the `=`.
  const pattern = new RegExp(`\\bconst\\s+${name}\\s*:\\s*[^=]+=\\s*\\*?b?"((?:[^"\\\\]|\\\\.)*)"\\s*;`);
  const match = sourceOf(path).match(pattern);
  if (!match) throw new Error(`${path} declares no const ${name} with a string literal; the table names an authority that moved`);
  return match[1];
}

/** A single-quoted TypeScript string, the spelling every other generated module uses. */
const ts = (value) => `'${value.replace(/\\/g, '\\\\').replace(/'/g, "\\'")}'`;

const seen = new Map();
let generated = '// @generated by scripts/generate-protocol-constants.mjs from the Rust constants its table names; do not edit.\n';
generated += '// Regenerate with: npm run abi:protocol-constants\n\n';
let section = null;
for (const [name, path, rust, form] of ROWS) {
  const value = rustLiteral(path, rust);
  if (form === 'magic' && !/^[A-Z0-9]{8}$/.test(value)) throw new Error(`${rust} in ${path} is ${ts(value)}, not an eight-character magic`);
  if (seen.has(name)) throw new Error(`${name} is emitted twice`);
  seen.set(name, value);
  if (section !== form) {
    section = form;
    generated += form === 'magic' ? '// Record and instruction magics.\n' : form === 'seed' ? '\n// PDA seed domains, as the bytes a derivation takes.\n' : '\n// Envelope kinds and formats, as the text a JSON document carries.\n';
  }
  const provenance = `${path}::${rust}`;
  generated += form === 'seed'
    ? `export const ${name} = new TextEncoder().encode(${ts(value)}); // ${provenance}\n`
    : `export const ${name} = ${ts(value)} as const; // ${provenance}\n`;
}

if (process.argv.includes('--check')) {
  if (readFileSync(outputUrl, 'utf8') !== generated) {
    console.error('lib/generated/protocolConstantsV1.ts is stale -- run `npm run abi:protocol-constants`');
    process.exit(1);
  }
} else {
  writeFileSync(fileURLToPath(outputUrl), generated);
  console.log(`wrote lib/generated/protocolConstantsV1.ts: ${ROWS.length} constants`);
}
