import { readFileSync, renameSync, unlinkSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

import { requireGeneratorFollowsRoute } from './route-binding.mjs';

const root = new URL('../../../', import.meta.url);

/**
 * The `DCLTCAP1` manifest coordinates moved to a Lean schema
 * (`DClutchSemantics/CapabilityManifestV1Abi.lean`), so the crate no longer
 * writes them as literals — it projects `generated_abi.rs`, which is where they
 * are read from now. The names this ABI has always exported are kept by
 * relabelling; the values are the Lean-emitted ones either way.
 *
 * The browser's own copy of this layout lives in
 * `lib/generated/capabilityManifestV1.ts`, emitted from the same schema. A
 * later pass should retire these aliases in favour of that module; doing it
 * here would edit direct-hot surfaces another lane owns.
 */
const MANIFEST_ALIASES = Object.freeze({
  MANIFEST_HEADER_BYTES: 'CAPABILITY_MANIFEST_HEADER_BYTES_V1',
  CAPABILITY_ENTRY_BYTES: 'CAPABILITY_ENTRY_BYTES_V1',
  MAX_CAPABILITIES: 'MAX_CAPABILITIES_V1',
  MANIFEST_MAGIC: 'CAPABILITY_MANIFEST_MAGIC_V1',
  MANIFEST_SCHEMA_OFFSET: 'CAPABILITY_MANIFEST_SCHEMA_OFFSET_V1',
  MANIFEST_PROFILE_OFFSET: 'CAPABILITY_MANIFEST_PROFILE_OFFSET_V1',
  MANIFEST_COUNT_OFFSET: 'CAPABILITY_MANIFEST_COUNT_OFFSET_V1',
  MANIFEST_RESERVED_OFFSET: 'CAPABILITY_MANIFEST_RESERVED_OFFSET_V1',
  KIND_ID_OFFSET: 'CAPABILITY_ENTRY_KIND_ID_OFFSET_V1',
  RELEASE_ID_OFFSET: 'CAPABILITY_ENTRY_RELEASE_ID_OFFSET_V1',
  CONFIG_ID_OFFSET: 'CAPABILITY_ENTRY_CONFIG_ID_OFFSET_V1',
  CAPACITY_PROFILE_ID_OFFSET: 'CAPABILITY_ENTRY_CAPACITY_PROFILE_ID_OFFSET_V1',
  CHILD_SCHEMA_ID_OFFSET: 'CAPABILITY_ENTRY_CHILD_SCHEMA_ID_OFFSET_V1',
  CHILD_DERIVATION_ID_OFFSET: 'CAPABILITY_ENTRY_CHILD_DERIVATION_ID_OFFSET_V1',
  ACTIVATION_POLICY_OFFSET: 'CAPABILITY_ENTRY_ACTIVATION_POLICY_OFFSET_V1',
  DEPENDENCY_COUNT_OFFSET: 'CAPABILITY_ENTRY_DEPENDENCY_COUNT_OFFSET_V1',
  ENTRY_RESERVED_OFFSET: 'CAPABILITY_ENTRY_RESERVED_OFFSET_V1',
  ACTIVATION_DEADLINE_OFFSET: 'CAPABILITY_ENTRY_ACTIVATION_DEADLINE_OFFSET_V1',
  DEPENDENCIES_OFFSET: 'CAPABILITY_ENTRY_DEPENDENCIES_OFFSET_V1',
});

function relabel(rust, aliases) {
  let text = rust;
  for (const [exported, emitted] of Object.entries(aliases)) {
    const before = text;
    text = text.replace(new RegExp(`(const )${emitted}(:)`), `$1${exported}$2`);
    if (text === before) throw new Error(`generated_abi.rs no longer emits ${emitted}`);
  }
  return text;
}

// The single place a scraped file's path is written. The route-binding gate
// below reads these same strings, so a source that moves cannot be checked
// against a stale copy of where it used to live.
const SOURCE_FILES = Object.freeze({
  hot: 'crates/dclutch-capability-program-contract/src/hot_v3.rs',
  descriptor: 'crates/dclutch-capability-program-contract/src/generated_v3.rs',
  descriptorContract: 'crates/dclutch-capability-program-contract/src/v3.rs',
  descriptorV4: 'crates/dclutch-capability-program-contract/src/generated_v4.rs',
  descriptorContractV4: 'crates/dclutch-capability-program-contract/src/v4.rs',
  root: 'crates/dclutch-capability-program-contract/src/generated.rs',
  rootContract: 'crates/dclutch-capability-program-contract/src/lib.rs',
  selection: 'crates/dclutch-release-set-contract/src/generated_capability_execution.rs',
  manifest: 'crates/dclutch-capability-contract/src/generated_abi.rs',
  set: 'crates/dclutch-capability-program-contract/src/generated_set_v1.rs',
  setV2: 'crates/dclutch-capability-program-contract/src/generated_set_v2.rs',
  direct: 'crates/dclutch-direct-codec/src/execution_v3.rs',
  nativeEvidence: 'crates/dclutch-direct-codec/src/native_evidence_v3.rs',
  intent: 'crates/dclutch-direct-codec/src/generated_intent_v2.rs',
  ordinary: 'crates/dclutch-direct-codec/src/generated_ordinary_v3.rs',
  ordinaryArtifacts: 'crates/dclutch-direct-codec/src/ordinary_artifacts_v3.rs',
  ordinaryBundle: 'crates/dclutch-direct-codec/src/ordinary_bundle_v4.rs',
  successor: 'crates/dclutch-direct-codec/src/successor.rs',
  successorGenerated: 'crates/dclutch-direct-codec/src/generated_successor.rs',
  account: 'crates/dclutch-account-profile-contract/src/v2.rs',
  accountProfile14: 'crates/dclutch-account-profile-contract/src/v2/generated_profile14.rs',
  request: 'crates/dclutch-request-profile-contract/src/v2.rs',
  requestGenerated: 'crates/dclutch-request-profile-contract/src/generated.rs',
  transition: 'crates/dclutch-transition-vm/src/v3.rs',
  // The effect kernel has three live generations and their names do not track
  // their preimages: `v3.rs`'s schema preimage reads
  // `effect-program-v4-ordered-receipt-dependencies-v1`, while `v4.rs`'s reads
  // `effect-program-v5-...`. Both are emitted, under names that say which
  // generation they are, because the two are not interchangeable: every current
  // authenticator binds V4 (`dclutch-direct-codec/src/artifacts_v4.rs`,
  // `dclutch-rational-lifecycle-hot-v3/src/selected_bundle_v6.rs`), and V3 is
  // kept only so the explorer can still name records published before cohort-8.
  // Which of the two the route binds is no longer a comment's word: the gate
  // below walks the route's own use-tree to the defining file.
  effect: 'crates/dclutch-effect-kernel/src/v3.rs',
  effectV4: 'crates/dclutch-effect-kernel/src/v4.rs',
  lifecycle: 'crates/dclutch-account-profile-contract/src/lifecycle_v3.rs',
  strategy: 'crates/dclutch-execution-strategy-contract/src/v2.rs',
  strategyGenerated: 'crates/dclutch-execution-strategy-contract/src/generated_v2.rs',
  // The `DCLTPAY3` layout scalars this generator scrapes are Lean-emitted
  // and live in `generated_runtime_v3.rs`; `runtime_v3.rs` `include!`s that
  // file and keeps only private aliases whose right-hand sides are names,
  // not numbers, so the scalar regex below cannot see them there. Reading
  // the emitted file directly also points this mirror at the actual
  // authority rather than at a handwritten restatement of it.
  basis: 'crates/dclutch-product-payoff-v2-codec/src/generated_runtime_v3.rs',
  basisGenerated: 'crates/dclutch-product-payoff-v2-codec/src/generated_admission_v3.rs',
});

function readCrateFile(file) {
  return readFileSync(new URL(file, root), 'utf8');
}

const sources = Object.freeze(Object.fromEntries(
  Object.entries(SOURCE_FILES).map(([key, file]) => [
    key,
    key === 'manifest' ? relabel(readCrateFile(file), MANIFEST_ALIASES) : readCrateFile(file),
  ]),
));

/**
 * The route-binding gate.
 *
 * A `--check` byte gate proves this file's OUTPUT is fresh against whatever
 * source it points at. It cannot prove the source is the one the live route
 * authenticates against -- and that is the exact hole that shipped: this
 * generator scraped the effect kernel's `v3.rs` while
 * `authenticate_direct_artifacts_v4` bound `v4.rs`, and everything stayed
 * green while a real reader was turned away.
 *
 * So for every authority-selecting constant (anything whose VALUE picks a
 * generation), walk the route's own use-tree -- through re-exports, in real
 * source text -- to the file that defines it, and require that file and
 * constant to be the ones scraped here. Layout scalars need no gate: an
 * offset that moves breaks the byte gate loudly.
 */
const ROUTE_FILE = 'crates/dclutch-direct-codec/src/artifacts_v4.rs';
const ROUTE_CRATE = 'dclutch_direct_codec';
const routeText = readCrateFile(ROUTE_FILE);

const ROUTE_BOUND_AUTHORITIES = Object.freeze([
  {
    routeName: 'CAPABILITY_PROGRAM_SCHEMA_ID_V4',
    conjunct: 'content(CAPABILITY_PROGRAM_SCHEMA_ID_V4)?',
    source: 'descriptorContractV4', sourceConstant: 'SCHEMA_RELEASE_ID',
  },
  {
    routeName: 'DIRECT_SUCCESSOR_KIND_ID_V3',
    conjunct: 'descriptor.kind().to_bytes() != DIRECT_SUCCESSOR_KIND_ID_V3',
    source: 'direct', sourceConstant: 'DIRECT_SUCCESSOR_KIND_ID_V3',
  },
  {
    routeName: 'DIRECT_EXECUTION_CONFIG_SCHEMA_ID_V1',
    conjunct: 'descriptor.config_schema().to_bytes() != DIRECT_EXECUTION_CONFIG_SCHEMA_ID_V1',
    source: 'successor', sourceConstant: 'DIRECT_EXECUTION_CONFIG_SCHEMA_ID_V1',
  },
  {
    routeName: 'DIRECT_EXECUTION_REQUEST_SCHEMA_ID_V3',
    conjunct: 'descriptor.request_schema().to_bytes() != DIRECT_EXECUTION_REQUEST_SCHEMA_ID_V3',
    source: 'direct', sourceConstant: 'DIRECT_EXECUTION_REQUEST_SCHEMA_ID_V3',
  },
  {
    routeName: 'DIRECT_ROOT_SCHEMA_ID_V1',
    conjunct: 'descriptor.root_schema().to_bytes() != DIRECT_ROOT_SCHEMA_ID_V1',
    source: 'successor', sourceConstant: 'DIRECT_ROOT_SCHEMA_ID_V1',
  },
  {
    routeName: 'ACCOUNT_PROFILE_SCHEMA_ID_V2',
    conjunct: 'descriptor.account_profile().schema().to_bytes() != ACCOUNT_PROFILE_SCHEMA_ID_V2',
    source: 'account', sourceConstant: 'SCHEMA_RELEASE_ID',
  },
  {
    // Two hops, both read from source: the route takes this from the
    // capability contract's `v4`, which `pub use`s it from `lifecycle_v3` as
    // CURRENT_RENT_QUOTE_SCHEMA_RELEASE_ID_V5. If the route ever moves to a V6
    // lifecycle, one of those hops stops leading here and this reds.
    routeName: 'SELECTED_LIFECYCLE_SCHEMA_RELEASE_ID_V5',
    conjunct: 'descriptor.lifecycle().schema().to_bytes() != SELECTED_LIFECYCLE_SCHEMA_RELEASE_ID_V5',
    source: 'lifecycle', sourceConstant: 'CURRENT_RENT_QUOTE_SCHEMA_RELEASE_ID_V5',
  },
  {
    routeName: 'EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2',
    conjunct: 'descriptor.strategy().schema().to_bytes() != EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2',
    source: 'strategy', sourceConstant: 'EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2',
  },
  {
    // The route spells this one as a full path in the conjunct rather than
    // binding it in a use-tree, so there is no alias to resolve.
    qualified: 'dclutch_transition_vm::v3::SCHEMA_RELEASE_ID',
    conjunct: 'descriptor.transition().schema().to_bytes() != dclutch_transition_vm::v3::SCHEMA_RELEASE_ID',
    source: 'transition', sourceConstant: 'SCHEMA_RELEASE_ID',
  },
  {
    // The convicted one. `EFFECT_SCHEMA_ID_V4` must resolve to the effect
    // kernel's v4.rs, never v3.rs, whose preimage misleadingly reads
    // `effect-program-v4-...`.
    routeName: 'EFFECT_SCHEMA_ID_V4',
    conjunct: 'descriptor.effect().schema().to_bytes() != EFFECT_SCHEMA_ID_V4',
    source: 'effectV4', sourceConstant: 'SCHEMA_RELEASE_ID_V4',
  },
  {
    routeName: 'REQUEST_PROFILE_V2_SCHEMA_RELEASE_ID',
    conjunct: 'descriptor.request_profile().schema().to_bytes() != REQUEST_PROFILE_V2_SCHEMA_RELEASE_ID',
    source: 'request', sourceConstant: 'REQUEST_PROFILE_V2_SCHEMA_RELEASE_ID',
  },
]);

for (const binding of ROUTE_BOUND_AUTHORITIES) {
  const sourceFile = SOURCE_FILES[binding.source];
  if (sourceFile === undefined) throw new Error(`route gate names unknown generator source ${binding.source}`);
  requireGeneratorFollowsRoute({
    routeText,
    routeCrate: ROUTE_CRATE,
    readSource: readCrateFile,
    binding: { ...binding, sourceFile },
  });
}
const outputUrl = new URL('../lib/generated/directInlineV3.ts', import.meta.url);

/**
 * The text behind a source, named either by a `SOURCE_FILES` key or -- when a
 * forward below resolves one -- by a repository path.
 */
const aliasTexts = new Map();
function sourceText(source) {
  if (sources[source] !== undefined) return sources[source];
  if (!aliasTexts.has(source)) aliasTexts.set(source, readCrateFile(source));
  return aliasTexts.get(source);
}

function sourcePath(source) {
  return SOURCE_FILES[source] ?? source;
}

function siblingOf(file, relative) {
  const stack = [];
  for (const part of `${file.slice(0, file.lastIndexOf('/'))}/${relative}`.split('/')) {
    if (part === '.' || part === '') continue;
    if (part === '..') stack.pop();
    else stack.push(part);
  }
  return stack.join('/');
}

/**
 * The file one module name inside a scraped source actually is.
 *
 * Read from the crate, in the three spellings this tree uses -- an attributed
 * `#[path = "…"] mod m;`, a `use crate::other as m;` rename, and a plain
 * `mod m;`. Reading it means the module-to-file mapping has no copy here to
 * go stale; an unrecognised spelling returns null and the caller refuses in
 * its own words rather than guessing.
 */
function moduleFile(source, moduleName) {
  const text = sourceText(source);
  const file = sourcePath(source);
  const attributed = text.match(new RegExp(`#\\[path = "([^"]+)"\\][^;]*?\\bmod ${moduleName};`));
  if (attributed) return siblingOf(file, attributed[1]);
  const renamed = text.match(new RegExp(`use crate::([a-z_0-9]+) as ${moduleName};`));
  if (renamed) return `${file.slice(0, file.indexOf('/src/'))}/src/${renamed[1]}.rs`;
  if (new RegExp(`^[ \\t]*(?:pub(?:\\([^)]*\\))? )?mod ${moduleName};`, 'm').test(text)) {
    // A plain `mod m;` in `foo.rs` is `foo/m.rs`, and in a crate or directory
    // root it is the sibling `m.rs`. Getting this wrong reads a file that does
    // not exist, which is how it was caught.
    const stem = file.slice(file.lastIndexOf('/') + 1, -'.rs'.length);
    return stem === 'lib' || stem === 'mod'
      ? siblingOf(file, `${moduleName}.rs`)
      : siblingOf(file, `${stem}/${moduleName}.rs`);
  }
  return null;
}

/**
 * A constant whose value is `module::NAME` in a Lean-emitted sibling.
 *
 * Crates that gained a Lean author kept their public names and made each value
 * a forward: `account_profile_contract::v2`'s coordinates are
 * `generated_abi::ACCOUNT_PROFILE_V2_*`, and the effect kernel's V4 identity is
 * `generated::EFFECT_V4_SCHEMA_RELEASE_ID_LEAN`. Following the forward reads
 * the value at its author, while the route-binding gate above still sees this
 * generator scraping the file the live route's use-tree resolves to -- which
 * is the forwarding file, not the emission.
 */
function aliasTarget(source, name) {
  const match = sourceText(source).match(new RegExp(`(?:pub(?:\\(crate\\))? )?const ${name}: [^=]+ =\\s*([a-z_0-9]+)::([A-Z0-9_]+);`));
  if (!match) return null;
  const file = moduleFile(source, match[1]);
  if (file === null) throw new Error(`${sourcePath(source)} forwards ${name} to \`${match[1]}\`, a module it does not say the file of`);
  return Object.freeze({ source: file, name: match[2] });
}

function scalar(source, name) {
  const alias = aliasTarget(source, name);
  if (alias) return scalar(alias.source, alias.name);
  const match = sourceText(source).match(new RegExp(`(?:pub(?:\\(crate\\))? )?const ${name}: [^=]+ = ([0-9_]+);`));
  if (!match) throw new Error(`missing Rust scalar ${source}.${name}`);
  return Number(match[1].replaceAll('_', ''));
}

function scalarExpression(source, name, expected) {
  const match = sourceText(source).match(new RegExp(`(?:pub(?:\\(crate\\))? )?const ${name}: [^=]+ = ([^;]+);`));
  if (!match || match[1].trim() !== expected) throw new Error(`unexpected Rust expression ${source}.${name}`);
}

function bytes(source, name) {
  const alias = aliasTarget(source, name);
  if (alias) return bytes(alias.source, alias.name);
  const match = sourceText(source).match(new RegExp(`(?:pub(?:\\(crate\\))? )?const ${name}: \\[u8; [^\\]]+\\] =\\s*(?:\\*b"([^"]+)"|\\[([\\s\\S]*?)\\]);`));
  if (!match) throw new Error(`missing Rust bytes ${source}.${name}`);
  if (match[1]) return [...new TextEncoder().encode(match[1])];
  return [...match[2].matchAll(/0x[0-9a-f]+|\b[0-9]+\b/g)].map((entry) => Number(entry[0]));
}

function byteString(source, name) {
  const alias = aliasTarget(source, name);
  if (alias) return byteString(alias.source, alias.name);
  const match = sourceText(source).match(new RegExp(`(?:pub )?const ${name}: &\\[u8(?:; [0-9]+)?\\] =\\s*b"([^"]+)";`));
  if (!match) throw new Error(`missing Rust byte string ${source}.${name}`);
  return match[1];
}

function array(name, values) {
  return `export const ${name} = Uint8Array.from([${values.map((value) => `0x${value.toString(16).padStart(2, '0')}`).join(', ')}]);\n`;
}

const scalars = Object.freeze([
  ['hot', 'HOT_EXECUTION_VERSION_V3'], ['hot', 'HOT_EXECUTION_PROFILE_V3'],
  ['hot', 'HOT_EXECUTION_ENVELOPE_BYTES_V3'], ['hot', 'HOT_FIXED_ACCOUNT_COUNT_V3'],
  ['hot', 'HOT_MARKET_ACCOUNT_V3'], ['hot', 'HOT_ROOT_ACCOUNT_V3'],
  ['hot', 'HOT_MANIFEST_RAW_ACCOUNT_V3'], ['hot', 'HOT_MANIFEST_STAGING_ACCOUNT_V3'],
  ['hot', 'HOT_PROGRAM_SET_RAW_ACCOUNT_V3'], ['hot', 'HOT_PROGRAM_SET_STAGING_ACCOUNT_V3'],
  ['hot', 'HOT_DESCRIPTOR_RAW_ACCOUNT_V3'], ['hot', 'HOT_DESCRIPTOR_STAGING_ACCOUNT_V3'],
  ['hot', 'HOT_CONFIG_RAW_ACCOUNT_V3'], ['hot', 'HOT_CONFIG_STAGING_ACCOUNT_V3'],
  ['hot', 'HOT_ACCOUNT_PROFILE_RAW_ACCOUNT_V3'], ['hot', 'HOT_ACCOUNT_PROFILE_STAGING_ACCOUNT_V3'],
  ['hot', 'HOT_REQUEST_PROFILE_RAW_ACCOUNT_V3'], ['hot', 'HOT_REQUEST_PROFILE_STAGING_ACCOUNT_V3'],
  ['hot', 'HOT_TRANSITION_RAW_ACCOUNT_V3'], ['hot', 'HOT_TRANSITION_STAGING_ACCOUNT_V3'],
  ['hot', 'HOT_EFFECT_RAW_ACCOUNT_V3'], ['hot', 'HOT_EFFECT_STAGING_ACCOUNT_V3'],
  ['hot', 'HOT_LIFECYCLE_RAW_ACCOUNT_V3'], ['hot', 'HOT_LIFECYCLE_STAGING_ACCOUNT_V3'],
  ['hot', 'HOT_STRATEGY_RAW_ACCOUNT_V3'], ['hot', 'HOT_STRATEGY_STAGING_ACCOUNT_V3'],
  ['hot', 'HOT_ACTIVATION_CACHE_ACCOUNT_V3'], ['hot', 'HOT_CORE_PROGRAM_ACCOUNT_V3'],
  ['hot', 'HOT_CORE_PROGRAMDATA_ACCOUNT_V3'], ['hot', 'HOT_TRADING_PROGRAM_ACCOUNT_V3'],
  ['hot', 'HOT_TRADING_PROGRAMDATA_ACCOUNT_V3'], ['hot', 'HOT_REGISTRY_PROGRAM_ACCOUNT_V3'],
  ['hot', 'HOT_RENT_SYSVAR_ACCOUNT_V3'], ['hot', 'HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3'],
  ['hot', 'HOT_PRODUCT_RAW_ACCOUNT_V3'], ['hot', 'HOT_PRODUCT_STAGING_ACCOUNT_V3'],
  ['hot', 'HOT_RESULT_DOMAIN_RAW_ACCOUNT_V3'], ['hot', 'HOT_RESULT_DOMAIN_STAGING_ACCOUNT_V3'],
  ['hot', 'HOT_PORTFOLIO_RAW_ACCOUNT_V3'], ['hot', 'HOT_PORTFOLIO_STAGING_ACCOUNT_V3'],
  ['hot', 'HOT_LINKED_BASIS_RAW_ACCOUNT_V3'], ['hot', 'HOT_LINKED_BASIS_STAGING_ACCOUNT_V3'],
  ['hot', 'HOT_CAPABILITY_SEAL_ACCOUNT_V3'],
  ['basis', 'BASIS_SCHEMA_V3'], ['basis', 'BASIS_HEADER_BYTES_V3'], ['basis', 'BASIS_WIDTH_OFFSET_V3'],
  ['basis', 'KNOT_BYTES_V3'], ['basis', 'TERM_BYTES_V3'], ['basis', 'EXACT_CATEGORICAL_BOUNDARY_V3'],
  ['basis', 'TERM_FLOOR_EXACT_COMPLEMENT_BOUNDARY_V3'],
  ['descriptor', 'CAPABILITY_PROGRAM_V3_BYTES'], ['descriptor', 'CAPABILITY_PROGRAM_V3_SCHEMA_VERSION'],
  ['descriptor', 'CAPABILITY_PROGRAM_V3_ARTIFACT_PROFILE'], ['descriptor', 'CAPABILITY_PROGRAM_V3_TRANSITION_SCHEMA_VERSION'],
  ['descriptor', 'CAPABILITY_PROGRAM_V3_REQUEST_PROFILE_SCHEMA_VERSION'], ['descriptor', 'CAPABILITY_PROGRAM_V3_KIND_OFFSET'],
  ['descriptor', 'CAPABILITY_PROGRAM_V3_SCHEMA_VERSION_OFFSET'], ['descriptor', 'CAPABILITY_PROGRAM_V3_ARTIFACT_PROFILE_OFFSET'],
  ['descriptor', 'CAPABILITY_PROGRAM_V3_TRANSITION_SCHEMA_VERSION_OFFSET'], ['descriptor', 'CAPABILITY_PROGRAM_V3_REQUEST_PROFILE_SCHEMA_VERSION_OFFSET'],
  ['descriptor', 'CAPABILITY_PROGRAM_V3_CONFIG_SCHEMA_OFFSET'], ['descriptor', 'CAPABILITY_PROGRAM_V3_REQUEST_SCHEMA_OFFSET'],
  ['descriptor', 'CAPABILITY_PROGRAM_V3_ROOT_SCHEMA_OFFSET'], ['descriptor', 'CAPABILITY_PROGRAM_V3_ACCOUNT_PROFILE_OFFSET'],
  ['descriptor', 'CAPABILITY_PROGRAM_V3_DERIVATION_POLICY_OFFSET'], ['descriptor', 'CAPABILITY_PROGRAM_V3_CAPACITY_PROFILE_OFFSET'],
  ['descriptor', 'CAPABILITY_PROGRAM_V3_EFFECT_PROGRAM_OFFSET'], ['descriptor', 'CAPABILITY_PROGRAM_V3_REQUEST_PROFILE_SCHEMA_OFFSET'],
  ['descriptor', 'CAPABILITY_PROGRAM_V3_REQUEST_PROFILE_PROGRAM_OFFSET'], ['descriptor', 'CAPABILITY_PROGRAM_V3_TRANSITION_SCHEMA_OFFSET'],
  ['descriptor', 'CAPABILITY_PROGRAM_V3_TRANSITION_PROGRAM_OFFSET'], ['descriptor', 'CAPABILITY_PROGRAM_V3_ROOT_STATE_BYTES_OFFSET'],
  ['descriptor', 'CAPABILITY_PROGRAM_V3_TAIL_RESERVED_OFFSET'],
  ['descriptorV4', 'CAPABILITY_PROGRAM_V4_BYTES'], ['descriptorV4', 'CAPABILITY_PROGRAM_V4_SCHEMA_VERSION'],
  ['descriptorV4', 'CAPABILITY_PROGRAM_V4_ARTIFACT_PROFILE'], ['descriptorV4', 'CAPABILITY_PROGRAM_V4_SCHEMA_VERSION_OFFSET'],
  ['descriptorV4', 'CAPABILITY_PROGRAM_V4_ARTIFACT_PROFILE_OFFSET'], ['descriptorV4', 'CAPABILITY_PROGRAM_V4_HEADER_RESERVED_OFFSET'],
  ['descriptorV4', 'CAPABILITY_PROGRAM_V4_KIND_OFFSET'], ['descriptorV4', 'CAPABILITY_PROGRAM_V4_CONFIG_SCHEMA_OFFSET'],
  ['descriptorV4', 'CAPABILITY_PROGRAM_V4_REQUEST_SCHEMA_OFFSET'], ['descriptorV4', 'CAPABILITY_PROGRAM_V4_ROOT_SCHEMA_OFFSET'],
  ['descriptorV4', 'CAPABILITY_PROGRAM_V4_DERIVATION_POLICY_OFFSET'], ['descriptorV4', 'CAPABILITY_PROGRAM_V4_CAPACITY_PROFILE_OFFSET'],
  ['descriptorV4', 'CAPABILITY_PROGRAM_V4_ACCOUNT_PROFILE_SCHEMA_OFFSET'], ['descriptorV4', 'CAPABILITY_PROGRAM_V4_ACCOUNT_PROFILE_PROGRAM_OFFSET'],
  ['descriptorV4', 'CAPABILITY_PROGRAM_V4_REQUEST_PROFILE_SCHEMA_OFFSET'], ['descriptorV4', 'CAPABILITY_PROGRAM_V4_REQUEST_PROFILE_PROGRAM_OFFSET'],
  ['descriptorV4', 'CAPABILITY_PROGRAM_V4_LIFECYCLE_SCHEMA_OFFSET'], ['descriptorV4', 'CAPABILITY_PROGRAM_V4_LIFECYCLE_PROGRAM_OFFSET'],
  ['descriptorV4', 'CAPABILITY_PROGRAM_V4_STRATEGY_SCHEMA_OFFSET'], ['descriptorV4', 'CAPABILITY_PROGRAM_V4_STRATEGY_PROGRAM_OFFSET'],
  ['descriptorV4', 'CAPABILITY_PROGRAM_V4_TRANSITION_SCHEMA_OFFSET'], ['descriptorV4', 'CAPABILITY_PROGRAM_V4_TRANSITION_PROGRAM_OFFSET'],
  ['descriptorV4', 'CAPABILITY_PROGRAM_V4_EFFECT_SCHEMA_OFFSET'], ['descriptorV4', 'CAPABILITY_PROGRAM_V4_EFFECT_PROGRAM_OFFSET'],
  ['descriptorV4', 'CAPABILITY_PROGRAM_V4_ROOT_STATE_BYTES_OFFSET'], ['descriptorV4', 'CAPABILITY_PROGRAM_V4_TAIL_RESERVED_OFFSET'],
  ['root', 'CAPABILITY_ROOT_HEADER_BYTES_V1'], ['root', 'CAPABILITY_ROOT_SCHEMA_VERSION_V1'],
  ['root', 'CAPABILITY_ROOT_PROFILE_V1'], ['root', 'CAPABILITY_ROOT_MAGIC_OFFSET'],
  ['root', 'CAPABILITY_ROOT_SCHEMA_VERSION_OFFSET'], ['root', 'CAPABILITY_ROOT_PROFILE_OFFSET'],
  ['root', 'CAPABILITY_ROOT_RESERVED_OFFSET'], ['root', 'CAPABILITY_ROOT_RELEASE_SET_OFFSET'],
  ['root', 'CAPABILITY_ROOT_MARKET_OFFSET'], ['root', 'CAPABILITY_ROOT_GENERATION_OFFSET'],
  ['root', 'CAPABILITY_ROOT_SELECTION_OFFSET'],
  ['selection', 'CAPABILITY_EXECUTION_SELECTION_BYTES_V1'], ['selection', 'CAPABILITY_EXECUTION_SELECTION_SCHEMA_VERSION_V1'],
  ['selection', 'CAPABILITY_EXECUTION_SELECTION_PROFILE_V1'], ['selection', 'CAPABILITY_EXECUTION_SELECTION_MAGIC_OFFSET'],
  ['selection', 'CAPABILITY_EXECUTION_SELECTION_SCHEMA_VERSION_OFFSET'], ['selection', 'CAPABILITY_EXECUTION_SELECTION_PROFILE_OFFSET'],
  ['selection', 'CAPABILITY_EXECUTION_SELECTION_ENTRY_INDEX_OFFSET'],
  ['selection', 'CAPABILITY_EXECUTION_SELECTION_RESERVED_OFFSET'], ['selection', 'CAPABILITY_EXECUTION_SELECTION_MANIFEST_OFFSET'],
  ['selection', 'CAPABILITY_EXECUTION_SELECTION_KIND_OFFSET'], ['selection', 'CAPABILITY_EXECUTION_SELECTION_RELEASE_OFFSET'],
  ['selection', 'CAPABILITY_EXECUTION_SELECTION_CONFIG_OFFSET'],
  ['manifest', 'MANIFEST_HEADER_BYTES'], ['manifest', 'CAPABILITY_ENTRY_BYTES'], ['manifest', 'MAX_CAPABILITIES'],
  ['manifest', 'MANIFEST_SCHEMA_OFFSET'], ['manifest', 'MANIFEST_PROFILE_OFFSET'], ['manifest', 'MANIFEST_COUNT_OFFSET'],
  ['manifest', 'MANIFEST_RESERVED_OFFSET'], ['manifest', 'KIND_ID_OFFSET'], ['manifest', 'RELEASE_ID_OFFSET'],
  ['manifest', 'CONFIG_ID_OFFSET'], ['manifest', 'CAPACITY_PROFILE_ID_OFFSET'], ['manifest', 'CHILD_SCHEMA_ID_OFFSET'],
  ['manifest', 'CHILD_DERIVATION_ID_OFFSET'], ['manifest', 'ACTIVATION_POLICY_OFFSET'], ['manifest', 'DEPENDENCY_COUNT_OFFSET'],
  ['manifest', 'ENTRY_RESERVED_OFFSET'], ['manifest', 'ACTIVATION_DEADLINE_OFFSET'], ['manifest', 'DEPENDENCIES_OFFSET'],
  ['set', 'CAPABILITY_PROGRAM_SET_HEADER_BYTES_V1'], ['set', 'CAPABILITY_PROGRAM_SET_ENTRY_BYTES_V1'],
  ['setV2', 'CAPABILITY_PROGRAM_SET_HEADER_BYTES_V2'], ['setV2', 'CAPABILITY_PROGRAM_SET_ENTRY_BYTES_V2'],
  ['setV2', 'CAPABILITY_PROGRAM_SET_MAX_BYTES_V2'], ['setV2', 'CAPABILITY_PROGRAM_SET_MAX_ENTRIES_V2'],
  ['setV2', 'CAPABILITY_PROGRAM_SET_SCHEMA_VERSION_V2'], ['setV2', 'CAPABILITY_PROGRAM_SET_ARTIFACT_PROFILE_V2'],
  ['setV2', 'CAPABILITY_PROGRAM_SET_CANONICAL_ENDIAN_V2'], ['setV2', 'CAPABILITY_PROGRAM_SET_SELECTOR_OFFSET_OFFSET_V2'],
  ['setV2', 'CAPABILITY_PROGRAM_SET_SELECTOR_WIDTH_OFFSET_V2'], ['setV2', 'CAPABILITY_PROGRAM_SET_SELECTOR_ENDIAN_OFFSET_V2'],
  ['setV2', 'CAPABILITY_PROGRAM_SET_ENTRY_COUNT_OFFSET_V2'], ['setV2', 'CAPABILITY_PROGRAM_SET_RESERVED_OFFSET_V2'],
  ['setV2', 'CAPABILITY_PROGRAM_SET_ENTRY_SELECTOR_OFFSET_V2'], ['setV2', 'CAPABILITY_PROGRAM_SET_ENTRY_DESCRIPTOR_SCHEMA_OFFSET_V2'],
  ['setV2', 'CAPABILITY_PROGRAM_SET_ENTRY_DESCRIPTOR_PROGRAM_OFFSET_V2'], ['setV2', 'CAPABILITY_PROGRAM_SET_ENTRY_RESERVED_OFFSET_V2'],
  ['direct', 'DIRECT_EXECUTION_REQUEST_VERSION_V3'], ['direct', 'DIRECT_EXECUTION_REQUEST_HEADER_BYTES_V3'],
  ['direct', 'DIRECT_EXECUTION_REQUEST_SELECTOR_OFFSET_V3'],
  ['nativeEvidence', 'DIRECT_NATIVE_EVIDENCE_DIRECT_BIAS_V3'],
  ['intent', 'COMPACT_INTENT_VERSION_V2'], ['intent', 'COMPACT_INTENT_BYTES_V2'],
  ['intent', 'COMPACT_INTENT_SIGNED_PREIMAGE_BYTES_V2'],
  ['intent', 'COMPACT_INTENT_MAGIC_OFFSET_V2'],
  ['intent', 'COMPACT_INTENT_SIDE_OFFSET_V2'], ['intent', 'COMPACT_INTENT_LIFECYCLE_OFFSET_V2'],
  ['intent', 'COMPACT_INTENT_OUTCOME_OFFSET_V2'], ['intent', 'COMPACT_INTENT_MARKET_OFFSET_V2'],
  ['intent', 'COMPACT_INTENT_GENERATION_OFFSET_V2'], ['intent', 'COMPACT_INTENT_NONCE_OFFSET_V2'],
  ['intent', 'COMPACT_INTENT_VALID_FROM_OFFSET_V2'], ['intent', 'COMPACT_INTENT_VALID_THROUGH_OFFSET_V2'],
  ['intent', 'COMPACT_INTENT_MAXIMUM_FILL_OFFSET_V2'], ['intent', 'COMPACT_INTENT_LIMIT_PRICE_OFFSET_V2'],
  ['intent', 'COMPACT_INTENT_FEE_BASIS_POINTS_OFFSET_V2'], ['intent', 'COMPACT_INTENT_COLLATERAL_ACCOUNT_OFFSET_V2'],
  ['ordinary', 'DIRECT_ORDINARY_COMMON_SCALARS_V3'], ['ordinary', 'DIRECT_ORDINARY_COMMON_IDENTITIES_V3'],
  // The register file is affine in the Product's outcome count --
  // `common + stride * tail_count` -- so these two strides are geometry the
  // emitter carries as named constants, not incidental zeroes. The browser
  // read them as literal 0 and refused cohort-8, whose scalar stride is 2.
  ['ordinary', 'DIRECT_ORDINARY_ITEM_SCALAR_STRIDE_V3'], ['ordinary', 'DIRECT_ORDINARY_ITEM_IDENTITY_STRIDE_V3'],
  ['ordinary', 'IDENTITY_SELLER_NATIVE_SIGNER_V3'], ['ordinary', 'IDENTITY_BUYER_NATIVE_SIGNER_V3'],
  ['successorGenerated', 'DIRECT_EXECUTION_CONFIG_BYTES_V1'], ['successorGenerated', 'DIRECT_ROOT_STATE_BYTES_V1'],
  ['successorGenerated', 'DIRECT_CONFIG_MAGIC_OFFSET_V1'], ['successorGenerated', 'DIRECT_CONFIG_VERSION_OFFSET_V1'],
  ['successorGenerated', 'DIRECT_CONFIG_RESERVED_A_OFFSET_V1'], ['successorGenerated', 'DIRECT_CONFIG_PRICE_SCALE_OFFSET_V1'],
  ['successorGenerated', 'DIRECT_CONFIG_FEE_BPS_OFFSET_V1'], ['successorGenerated', 'DIRECT_CONFIG_RESERVED_B_OFFSET_V1'],
  ['successorGenerated', 'DIRECT_CONFIG_FEE_RECIPIENT_OFFSET_V1'],
  ['account', 'HEADER_BYTES'], ['account', 'RULE_BYTES'], ['account', 'OPERATION_BYTES'],
  ['accountProfile14', 'FIXED_DATA_PREDICATE_ARTIFACT_PROFILE'],
  ['accountProfile14', 'FIXED_DATA_PREDICATE_HEADER_BYTES'],
  ['accountProfile14', 'FIXED_DATA_PREDICATE_DYNAMIC_SPAN_ENTRY_BYTES'],
  ['accountProfile14', 'FIXED_DATA_PREDICATE_BYTES'],
  ['accountProfile14', 'FIXED_DATA_PREDICATE_DYNAMIC_SPAN_COUNT_OFFSET'],
  ['accountProfile14', 'FIXED_DATA_PREDICATE_COUNT_OFFSET'],
  ['accountProfile14', 'FIXED_DATA_PREDICATE_HEADER_RESERVED_OFFSET'],
  ['accountProfile14', 'FIXED_DATA_PREDICATE_REQUIRE_U8'],
  ['accountProfile14', 'FIXED_DATA_PREDICATE_REQUIRE_U16'],
  ['accountProfile14', 'FIXED_DATA_PREDICATE_REQUIRE_U32'],
  ['accountProfile14', 'FIXED_DATA_PREDICATE_REQUIRE_U64'],
  ['accountProfile14', 'FIXED_DATA_PREDICATE_REQUIRE_ZERO_RANGE'],
  ['accountProfile14', 'FIXED_DATA_PREDICATE_OPCODE_OFFSET_V2'],
  ['accountProfile14', 'FIXED_DATA_PREDICATE_RESERVED_OFFSET_V2'],
  ['accountProfile14', 'FIXED_DATA_PREDICATE_ACCOUNT_OFFSET_V2'],
  ['accountProfile14', 'FIXED_DATA_PREDICATE_DATA_OFFSET_V2'],
  ['accountProfile14', 'FIXED_DATA_PREDICATE_PAYLOAD_OFFSET_V2'],
  ['request', 'REQUEST_PROFILE_V2_SCHEMA_VERSION'], ['request', 'REQUEST_PROFILE_V2_ARTIFACT_PROFILE'],
  ['request', 'REQUEST_PROFILE_V2_HEADER_BYTES'], ['request', 'NATIVE_SIGNATURE_REQUIREMENT_BYTES_V1'],
  ['request', 'EMBEDDED_V1_BYTES_OFFSET'], ['request', 'REQUIREMENT_COUNT_OFFSET'],
  ['request', 'HEADER_RESERVED_OFFSET'],
  ['requestGenerated', 'REQUEST_PROFILE_HEADER_BYTES_V1'], ['requestGenerated', 'REQUEST_PROFILE_OPERATION_BYTES_V1'],
  ['requestGenerated', 'REQUEST_PROFILE_MAX_BYTES_V1'], ['requestGenerated', 'REQUEST_PROFILE_SCHEMA_VERSION_V1'],
  ['requestGenerated', 'REQUEST_PROFILE_ARTIFACT_PROFILE_V1'], ['requestGenerated', 'REQUEST_PROFILE_VERSION_OFFSET'],
  ['requestGenerated', 'REQUEST_PROFILE_ARTIFACT_OFFSET'], ['requestGenerated', 'REQUEST_PROFILE_FIXED_REQUEST_BYTES_OFFSET'],
  ['requestGenerated', 'REQUEST_PROFILE_ITEM_REQUEST_BYTES_OFFSET'], ['requestGenerated', 'REQUEST_PROFILE_FIXED_OPERATIONS_OFFSET'],
  ['requestGenerated', 'REQUEST_PROFILE_ITEM_OPERATIONS_OFFSET'], ['requestGenerated', 'REQUEST_PROFILE_COMMON_SCALARS_OFFSET'],
  ['requestGenerated', 'REQUEST_PROFILE_ITEM_SCALAR_STRIDE_OFFSET'], ['requestGenerated', 'REQUEST_PROFILE_COMMON_IDENTITIES_OFFSET'],
  ['requestGenerated', 'REQUEST_PROFILE_ITEM_IDENTITY_STRIDE_OFFSET'], ['requestGenerated', 'REQUEST_OPERATION_OPCODE_OFFSET'],
  ['requestGenerated', 'REQUEST_OPERATION_REQUEST_SPACE_OFFSET'], ['requestGenerated', 'REQUEST_OPERATION_REGISTER_SPACE_OFFSET'],
  ['requestGenerated', 'REQUEST_OPERATION_RESERVED_BYTE_OFFSET'], ['requestGenerated', 'REQUEST_OPERATION_REQUEST_OFFSET_OFFSET'],
  ['requestGenerated', 'REQUEST_OPERATION_REGISTER_OFFSET'], ['requestGenerated', 'REQUEST_OPERATION_RESERVED_SHORT_OFFSET'],
  ['requestGenerated', 'REQUEST_OPERATION_IMMEDIATE_OFFSET'], ['requestGenerated', 'REQUEST_OPERATION_RESERVED_OFFSET'],
  ['strategyGenerated', 'EXECUTION_STRATEGY_PROGRAM_BYTES_V2'],
  ['strategyGenerated', 'EXECUTION_STRATEGY_SCHEMA_VERSION_V2'],
  ['strategyGenerated', 'EXECUTION_STRATEGY_ARTIFACT_PROFILE_V2'],
  ['strategyGenerated', 'STRATEGY_DISPOSITION_OFFSET_V2'],
  ['strategyGenerated', 'STRATEGY_TRANSITION_SCHEMA_OFFSET_V2'],
  ['strategyGenerated', 'STRATEGY_TRANSITION_PROGRAM_OFFSET_V2'],
]);

scalarExpression('hot', 'HOT_FAMILY_REQUEST_OFFSET_V3', 'HOT_EXECUTION_ENVELOPE_BYTES_V3');
scalarExpression('nativeEvidence', 'HEADER_BYTES', '2 + SIGNATURES * DESCRIPTOR_BYTES');
scalarExpression('nativeEvidence', 'PARTICIPANT_BYTES', '64');
scalarExpression('nativeEvidence', 'DIRECT_NATIVE_EVIDENCE_BYTES_V3', 'HEADER_BYTES + SIGNATURES * PARTICIPANT_BYTES');
const evidenceSignatures = scalar('nativeEvidence', 'SIGNATURES');
const evidenceDescriptorBytes = scalar('nativeEvidence', 'DESCRIPTOR_BYTES');
if (evidenceSignatures !== 2
    || !sources.nativeEvidence.includes('native_signature_slice_v3')
    || !sources.nativeEvidence.includes('HOT_FAMILY_REQUEST_OFFSET_V3')) {
  throw new Error('Direct native evidence no longer has the exact two-party InlineOrdinary geometry');
}

// The Registry continuation is headerless: its instruction data is the exact
// Hot bytes from byte zero, so Registry evidence carries the Direct bias and
// the retired headered container must not reappear under any name.
function evidenceContainerVariants() {
  const start = sources.nativeEvidence.indexOf('pub enum DirectNativeEvidenceContainerV3 {');
  const end = sources.nativeEvidence.indexOf('\n}', start);
  if (start < 0 || end < 0) throw new Error('missing canonical Direct native evidence container enum');
  return sources.nativeEvidence.slice(start, end).split('\n')
    .map((line) => line.trim())
    .filter((line) => /^[A-Z][A-Za-z0-9]*,$/.test(line))
    .map((line) => line.slice(0, -1));
}

function headerlessRegistrySuccessorBias() {
  const marker = 'pub fn encode_direct_headerless_registry_native_evidence_many_v4_atomic(';
  const start = sources.nativeEvidence.indexOf(marker);
  const end = sources.nativeEvidence.indexOf('\n}', start);
  if (start < 0 || end < 0) throw new Error('missing headerless Registry native evidence successor');
  const body = sources.nativeEvidence.slice(start + marker.length, end);
  const delegation = body.match(/encode_direct_native_evidence_many_v3_atomic\(\s*DirectNativeEvidenceContainerV3::([A-Za-z0-9]+),/);
  if (!delegation) throw new Error('headerless Registry successor no longer delegates to one named container');
  return delegation[1];
}

const evidenceContainers = evidenceContainerVariants();
if (evidenceContainers.length !== 1 || evidenceContainers[0] !== 'TradingHot') {
  throw new Error(`Direct native evidence containers changed: ${evidenceContainers.join(', ')}`);
}
if (!sources.nativeEvidence.includes('pub fn encode_direct_headerless_registry_native_evidence_v4_atomic(')
    || headerlessRegistrySuccessorBias() !== 'TradingHot') {
  throw new Error('the headerless Registry native evidence successor is absent or no longer bias-zero');
}
if (sources.nativeEvidence.includes('DIRECT_NATIVE_EVIDENCE_REGISTRY_BIAS_V3')
    || sources.nativeEvidence.includes('RegistryContinuation')) {
  throw new Error('the retired headered Registry evidence container reappeared in the Direct codec');
}
let output = '// @generated from canonical Rust/Lean-emitted Direct Hot V3 / Capability V4 ABIs; do not edit.\n';
output += '// Regenerate with: npm run abi:direct-v3\n\n';
for (const [source, name] of scalars) output += `export const ${name} = ${scalar(source, name)} as const;\n`;
output += 'export const HOT_FAMILY_REQUEST_OFFSET_V3 = HOT_EXECUTION_ENVELOPE_BYTES_V3;\n';
output += 'export const DIRECT_SIGNED_PARTICIPANT_BYTES_V3 = 32 + COMPACT_INTENT_SIGNED_PREIMAGE_BYTES_V2;\n';
output += 'export const DIRECT_INLINE_ORDINARY_ACTION_V3 = 1 as const;\n';
output += 'export const DIRECT_INLINE_ORDINARY_REQUEST_BYTES_V3 = DIRECT_EXECUTION_REQUEST_HEADER_BYTES_V3 + 2 * DIRECT_SIGNED_PARTICIPANT_BYTES_V3 + 16;\n';
output += `export const DIRECT_NATIVE_EVIDENCE_SIGNATURE_COUNT_V3 = ${evidenceSignatures} as const;\n`;
output += `export const DIRECT_NATIVE_EVIDENCE_DESCRIPTOR_BYTES_V3 = ${evidenceDescriptorBytes} as const;\n`;
output += 'export const DIRECT_NATIVE_EVIDENCE_HEADER_BYTES_V3 = 2 + DIRECT_NATIVE_EVIDENCE_SIGNATURE_COUNT_V3 * DIRECT_NATIVE_EVIDENCE_DESCRIPTOR_BYTES_V3;\n';
output += 'export const DIRECT_NATIVE_EVIDENCE_PARTICIPANT_BYTES_V3 = 64;\n';
output += 'export const DIRECT_NATIVE_EVIDENCE_BYTES_V3 = DIRECT_NATIVE_EVIDENCE_HEADER_BYTES_V3 + DIRECT_NATIVE_EVIDENCE_SIGNATURE_COUNT_V3 * DIRECT_NATIVE_EVIDENCE_PARTICIPANT_BYTES_V3;\n';
output += 'export const DIRECT_NATIVE_EVIDENCE_HEADERLESS_REGISTRY_BIAS_V4 = DIRECT_NATIVE_EVIDENCE_DIRECT_BIAS_V3;\n';
output += 'export const DIRECT_NATIVE_EVIDENCE_SELLER_MESSAGE_OFFSET_V3 = DIRECT_NATIVE_EVIDENCE_DIRECT_BIAS_V3 + HOT_FAMILY_REQUEST_OFFSET_V3 + DIRECT_EXECUTION_REQUEST_HEADER_BYTES_V3 + 32;\n';
output += 'export const DIRECT_NATIVE_EVIDENCE_SELLER_MAKER_OFFSET_V3 = DIRECT_NATIVE_EVIDENCE_SELLER_MESSAGE_OFFSET_V3 - 32;\n';
output += 'export const DIRECT_NATIVE_EVIDENCE_BUYER_MESSAGE_OFFSET_V3 = DIRECT_NATIVE_EVIDENCE_SELLER_MESSAGE_OFFSET_V3 + DIRECT_SIGNED_PARTICIPANT_BYTES_V3;\n';
output += 'export const DIRECT_NATIVE_EVIDENCE_BUYER_MAKER_OFFSET_V3 = DIRECT_NATIVE_EVIDENCE_BUYER_MESSAGE_OFFSET_V3 - 32;\n\n';
// The composite Trading child root PDA domain: [domain, market, generation_le,
// manifest, entry_index_le, kind, capability_release, config] under the
// Trading program (CapabilityRootSeedsV1::as_slices, the sole author).
output += `export const CAPABILITY_ROOT_PDA_DOMAIN_V1 = new TextEncoder().encode('${byteString('rootContract', 'CAPABILITY_ROOT_PDA_DOMAIN_V1')}');\n`;
for (const [source, name] of [
  ['hot', 'HOT_EXECUTION_MAGIC_V3'], ['descriptor', 'CAPABILITY_PROGRAM_V3_MAGIC'],
  ['descriptorV4', 'CAPABILITY_PROGRAM_V4_MAGIC'], ['setV2', 'CAPABILITY_PROGRAM_SET_MAGIC_V2'],
  ['basis', 'BASIS_MAGIC_V3'], ['basisGenerated', 'GRADED_BASIS_RECORD_SCHEMA_ID_V3'],
  ['root', 'CAPABILITY_ROOT_MAGIC_V1'], ['selection', 'CAPABILITY_EXECUTION_SELECTION_MAGIC_V1'],
  ['manifest', 'MANIFEST_MAGIC'], ['direct', 'DIRECT_EXECUTION_REQUEST_MAGIC_V3'],
  ['direct', 'DIRECT_EXECUTION_REQUEST_SCHEMA_ID_V3'], ['direct', 'DIRECT_SUCCESSOR_KIND_ID_V3'],
  ['intent', 'COMPACT_INTENT_MAGIC_V2'], ['intent', 'COMPACT_INTENT_SIGNATURE_DOMAIN_ID_V2'],
  ['successor', 'DIRECT_EXECUTION_CONFIG_SCHEMA_ID_V1'], ['successor', 'DIRECT_ROOT_SCHEMA_ID_V1'],
  ['successorGenerated', 'DIRECT_CONFIG_MAGIC_V1'],
  ['descriptorContract', 'SCHEMA_RELEASE_ID'], ['set', 'CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V1'],
  ['descriptorContractV4', 'SCHEMA_RELEASE_ID'], ['setV2', 'CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2'],
  ['account', 'MAGIC'], ['account', 'SCHEMA_RELEASE_ID'],
  ['accountProfile14', 'FIXED_DATA_PREDICATE_PROFILE_ID'],
  ['request', 'REQUEST_PROFILE_V2_MAGIC'], ['request', 'REQUEST_PROFILE_V2_SCHEMA_RELEASE_ID'],
  ['requestGenerated', 'REQUEST_PROFILE_MAGIC_V1'], ['transition', 'SCHEMA_RELEASE_ID'],
  ['effect', 'SCHEMA_RELEASE_ID'], ['effectV4', 'SCHEMA_RELEASE_ID_V4'],
  ['lifecycle', 'SCHEMA_RELEASE_ID'],
  ['strategy', 'EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2'],
  ['strategyGenerated', 'EXECUTION_STRATEGY_PROGRAM_MAGIC_V2'],
  ['lifecycle', 'CURRENT_RENT_QUOTE_SCHEMA_RELEASE_ID_V5'],
  ['ordinaryArtifacts', 'DIRECT_INLINE_ORDINARY_REQUEST_PROFILE_ID_V3'],
  ['ordinaryArtifacts', 'DIRECT_INLINE_ORDINARY_TRANSITION_ID_V3'],
  ['ordinaryArtifacts', 'DIRECT_INLINE_ORDINARY_STRATEGY_ID_V3'],
  ['ordinaryBundle', 'DIRECT_INLINE_ORDINARY_ACCOUNT_PROFILE_ID_V3'],
  ['ordinaryBundle', 'DIRECT_INLINE_ORDINARY_LIFECYCLE_ID_V5'],
  ['ordinaryBundle', 'DIRECT_INLINE_ORDINARY_EFFECT_ID_V4'],
]) {
  const alias = source === 'descriptorContractV4' && name === 'SCHEMA_RELEASE_ID'
    ? 'CAPABILITY_PROGRAM_V4_SCHEMA_RELEASE_ID'
    : source === 'lifecycle' && name === 'CURRENT_RENT_QUOTE_SCHEMA_RELEASE_ID_V5'
      ? 'SELECTED_LIFECYCLE_SCHEMA_RELEASE_ID_V5'
      : source === 'effect' && name === 'SCHEMA_RELEASE_ID'
        ? 'EFFECT_SCHEMA_RELEASE_ID_V3'
        : source === 'effectV4' && name === 'SCHEMA_RELEASE_ID_V4'
          ? 'EFFECT_SCHEMA_RELEASE_ID_V4'
          : name === 'SCHEMA_RELEASE_ID'
            ? `${source.toUpperCase()}_SCHEMA_RELEASE_ID`
            : name;
  output += array(alias, bytes(source, name));
}

if (process.argv.includes('--check')) {
  if (readFileSync(outputUrl, 'utf8') !== output) {
    console.error('Direct Hot V3 / Capability V4 TypeScript ABI is stale');
    process.exit(1);
  }
} else {
  const outputPath = fileURLToPath(outputUrl);
  const temporaryPath = `${outputPath}.tmp-${process.pid}`;
  try {
    writeFileSync(temporaryPath, output, { flag: 'wx' });
    const staged = readFileSync(temporaryPath, 'utf8');
    if (!staged.startsWith('// @generated from canonical Rust/Lean-emitted Direct Hot V3 / Capability V4 ABIs; do not edit.\n')
        || !staged.includes('export const DIRECT_INLINE_ORDINARY_REQUEST_BYTES_V3 =')) {
      throw new Error('generated Direct Hot V3 / Capability V4 TypeScript ABI failed its header/width validation');
    }
    renameSync(temporaryPath, outputPath);
  } catch (error) {
    try { unlinkSync(temporaryPath); } catch {}
    throw error;
  }
}
