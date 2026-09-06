/**
 * Emit `lib/generated/protocolConstantsV1.ts`: the record magics, PDA seed
 * domains and digest domains the client used to state in its own words, read
 * from the Rust constant that owns each one.
 *
 * One table, one scrape, one module. A row names the TypeScript export, the
 * Rust source file, the Rust constant and its KIND; the generator reads the
 * constant's literal out of that file (`*b"..."`, `b"..."` or `"..."`, any
 * visibility) and refuses when the constant is missing, when a magic is not
 * eight uppercase alphanumerics, or when two rows disagree about one value.
 * Seeds are emitted as the bytes a PDA derivation takes; kinds and formats as
 * the text a JSON envelope carries.
 *
 * WHY `record` AND `instruction` ARE TWO KINDS AND NOT ONE `magic`. An eight
 * byte magic identifies a persisted RECORD or selects an INSTRUCTION, and the
 * value alone does not say which: `DCLTRIX1` is the Registry's instruction
 * magic and nothing in this tree persists a record under it. A consumer that
 * cannot tell them apart classifies by the only thing it has, which is the
 * declaration -- and that is how the explorer's coverage survey came to
 * report `DCLTRIX1` as a record the explorer refuses to render, while the
 * explorer was rendering it as the Registry instruction it is. The kind is
 * emitted, not inferred from the export's name, so a survey reads it instead
 * of guessing.
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

/**
 * `[export name, Rust source (repo-relative), Rust constant, kind]`.
 *
 * `kind` is one of `record`, `instruction`, `seed`, `digest` or `text`, and
 * rows are grouped by it: the emitted module carries one section comment per
 * kind, in this order.
 *
 * WHY `digest` IS NOT `seed`. Both emit bytes and both are a domain, so the
 * temptation is to file a digest domain under the seed kind and be done. They
 * are not the same fact: a seed domain is an argument to
 * `findProgramAddress` and names an ACCOUNT; a digest domain is the prefix of
 * a hash preimage and names a NUMBER. The kind column exists so a consumer
 * classifies by the declaration rather than by the value -- which is the whole
 * reason `record` and `instruction` are two kinds -- and a survey of "the
 * addresses this client can derive" that swept in a digest domain would be
 * wrong in exactly that way.
 */
const ROWS = [
  // --- record magics -------------------------------------------------------
  ['ACTIVATION_CACHE_MAGIC_V1', 'crates/dclutch-registry/src/activation.rs', 'ACTIVATED_EXECUTION_RELEASE_SET_MAGIC_V1', 'record'],
  ['ARTIFACT_RELEASE_MAGIC_V1', 'crates/dclutch-registry/src/artifact.rs', 'ARTIFACT_RELEASE_MAGIC_V1', 'record'],
  ['CAPABILITY_SEAL_MAGIC_V1', 'crates/dclutch-vm/src/capability_seal/mod.rs', 'CAPABILITY_SEAL_MAGIC_V1', 'record'],
  ['CHECKED_INFRASTRUCTURE_MAGIC_V1', 'crates/dclutch-release-tool/src/infrastructure.rs', 'CHECKED_INFRASTRUCTURE_MAGIC_V1', 'record'],
  ['CHECKED_MULTIPROGRAM_MAGIC_V1', 'crates/dclutch-release-tool/src/multiprogram.rs', 'CHECKED_MULTIPROGRAM_MAGIC_V1', 'record'],
  ['CHECKED_RELEASE_MAGIC_V1', 'crates/dclutch-release-tool/src/lib.rs', 'CHECKED_RELEASE_MAGIC_V1', 'record'],
  ['EXECUTION_RELEASE_SET_MAGIC_V1', 'crates/dclutch-registry/src/release_set/mod.rs', 'EXECUTION_RELEASE_SET_MAGIC_V1', 'record'],
  ['PRODUCT_RUNTIME_DOMAIN_MAGIC_V2', 'crates/dclutch-product/src/generated.rs', 'DOMAIN_MAGIC', 'record'],
  ['PRODUCT_RUNTIME_PORTFOLIO_MAGIC_V2', 'crates/dclutch-product/src/generated.rs', 'PORTFOLIO_MAGIC', 'record'],
  ['PYTH_SPONSORED_PUSH_RELEASE_MAGIC_V1', 'crates/dclutch-source/src/pyth/sponsored_push.rs', 'PYTH_SPONSORED_PUSH_RELEASE_V1_MAGIC', 'record'],
  ['RELEASE_LINEAGE_MAGIC_V1', 'crates/dclutch-registry/src/lineage.rs', 'RELEASE_LINEAGE_MAGIC_V1', 'record'],
  ['TOKEN_BEHAVIOR_SELECTION_MAGIC_V2', 'crates/dclutch-custody/src/token_svm/behavior_binding_v2.rs', 'TOKEN_BEHAVIOR_SELECTION_MAGIC_V2', 'record'],
  // --- instruction magics: these SELECT a route, they identify no record ---
  ['REGISTRY_INSTRUCTION_MAGIC_V1', 'crates/dclutch-registry/src/svm/mod.rs', 'REGISTRY_INSTRUCTION_MAGIC_V1', 'instruction'],
  // --- PDA seed domains, as the bytes a derivation takes -------------------
  ['CAPABILITY_SEAL_PDA_DOMAIN_V1', 'crates/dclutch-vm/src/capability_seal/mod.rs', 'CAPABILITY_SEAL_PDA_DOMAIN_V1', 'seed'],
  ['DIRECT_MAKER_REPLAY_PDA_DOMAIN_V1', 'crates/dclutch-trading/src/successor.rs', 'DIRECT_MAKER_REPLAY_PDA_DOMAIN_V1', 'seed'],
  ['PROTOCOL_POSITION_CLAIMS_CAPABILITY_SEED_V2', 'crates/dclutch-claims/src/protocol_position_v2.rs', 'PROTOCOL_POSITION_CLAIMS_CAPABILITY_SEED_V2', 'seed'],
  ['RATIONAL_RECEIPT_MINT_SEED_V2', 'crates/dclutch-claims/src/rational/seeds.rs', 'RATIONAL_RECEIPT_MINT_SEED_V2', 'seed'],
  ['RATIONAL_REPRESENTATION_AUTHORITY_SEED_V2', 'crates/dclutch-claims/src/rational_kernel/mod.rs', 'RATIONAL_REPRESENTATION_AUTHORITY_SEED_V2', 'seed'],
  ['RATIONAL_SHARD_MINT_SEED_V2', 'crates/dclutch-claims/src/rational/mod.rs', 'RATIONAL_SHARD_MINT_SEED_V2', 'seed'],
  ['RATIONAL_STRUCTURED_CUSTODY_SEED_V2', 'crates/dclutch-claims/src/rational/mod.rs', 'RATIONAL_STRUCTURED_CUSTODY_SEED_V2', 'seed'],
  ['RELEASE_LINEAGE_PDA_DOMAIN_V1', 'crates/dclutch-registry/src/lineage.rs', 'RELEASE_LINEAGE_PDA_DOMAIN_V1', 'seed'],
  // --- digest domains, as the bytes a hash preimage is prefixed with -------
  ['FAMILY_REQUEST_DIGEST_DOMAIN_V3', 'crates/dclutch-market/src/execution_strategy/shadow_digest_v3.rs', 'FAMILY_REQUEST_DIGEST_DOMAIN_V3', 'digest'],
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

/** The section comment each kind opens with, and the order the kinds are emitted in. */
const SECTIONS = new Map([
  ['record', 'Record magics: each identifies a persisted record.'],
  ['instruction', 'Instruction magics: each SELECTS a route and identifies no record.'],
  ['seed', 'PDA seed domains, as the bytes a derivation takes.'],
  ['digest', 'Digest domains, as the bytes a hash preimage is prefixed with.'],
  ['text', 'Envelope kinds and formats, as the text a JSON document carries.'],
]);

/** The kinds emitted as bytes rather than as a string. */
const BYTE_KINDS = new Set(['seed', 'digest']);

const seen = new Map();
const instructionMagics = [];
let generated = '// @generated by scripts/generate-protocol-constants.mjs from the Rust constants its table names; do not edit.\n';
generated += '// Regenerate with: npm run abi:protocol-constants\n';
let section = null;
for (const [name, path, rust, kind] of ROWS) {
  if (!SECTIONS.has(kind)) throw new Error(`${name} declares kind ${ts(kind)}, which is not one of ${[...SECTIONS.keys()].join(', ')}`);
  const value = rustLiteral(path, rust);
  const magic = kind === 'record' || kind === 'instruction';
  if (magic && !/^[A-Z0-9]{8}$/.test(value)) throw new Error(`${rust} in ${path} is ${ts(value)}, not an eight-character magic`);
  if (seen.has(name)) throw new Error(`${name} is emitted twice`);
  seen.set(name, value);
  if (section !== kind) {
    // Rows are grouped by kind, and a kind that opens twice would emit its
    // section comment twice -- which is the table having lost its grouping,
    // not a formatting slip: a consumer reading kinds by section would then
    // read some of them under the wrong heading.
    if (SECTIONS.get(kind) === null) throw new Error(`the ${kind} rows are not contiguous; group the table by kind`);
    section = kind;
    generated += `\n// ${SECTIONS.get(kind)}\n`;
    SECTIONS.set(kind, null);
  }
  if (kind === 'instruction') instructionMagics.push(name);
  const provenance = `${path}::${rust}`;
  generated += BYTE_KINDS.has(kind)
    ? `export const ${name} = new TextEncoder().encode(${ts(value)}); // ${provenance}\n`
    : `export const ${name} = ${ts(value)} as const; // ${provenance}\n`;
}

// The kind column, in the one form a consumer that does not load TypeScript
// can read: the export names above whose value SELECTS a route. The explorer's
// coverage survey joins declared magics against what the explorer renders, and
// without this it has only the declaration to classify by -- which is how
// `DCLTRIX1`, rendered by the explorer as the Registry instruction it is, was
// reported as an unrendered record.
generated += '\n// Of the magics above, the ones that select an INSTRUCTION rather than identify a record.\n';
generated += `export const INSTRUCTION_MAGIC_EXPORTS_V1: ReadonlyArray<string> = ${JSON.stringify(instructionMagics).replace(/","/g, "', '").replace(/^\["/, "['").replace(/"\]$/, "']")};\n`;

if (process.argv.includes('--check')) {
  if (readFileSync(outputUrl, 'utf8') !== generated) {
    console.error('lib/generated/protocolConstantsV1.ts is stale -- run `npm run abi:protocol-constants`');
    process.exit(1);
  }
} else {
  writeFileSync(fileURLToPath(outputUrl), generated);
  console.log(`wrote lib/generated/protocolConstantsV1.ts: ${ROWS.length} constants`);
}
