import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const root = new URL('../../../', import.meta.url);
const sources = Object.freeze({
  core: readFileSync(new URL('crates/dclutch-market-core-codec/src/generated.rs', root), 'utf8'),
  physical: readFileSync(new URL('crates/dclutch-market-core-codec/src/generated_physical.rs', root), 'utf8'),
  product: readFileSync(new URL('crates/dclutch-product-runtime-v2-admission/src/lib.rs', root), 'utf8'),
  realm: readFileSync(new URL('crates/dclutch-realm-contract/src/lib.rs', root), 'utf8'),
  source: readFileSync(new URL('crates/dclutch-source-contract/src/generated_source_material_v2.rs', root), 'utf8'),
  capability: readFileSync(new URL('crates/dclutch-capability-contract/src/lib.rs', root), 'utf8'),
  releaseSet: readFileSync(new URL('crates/dclutch-release-set-contract/src/lib.rs', root), 'utf8'),
  registry: readFileSync(new URL('crates/dclutch-registry-contract/src/artifact.rs', root), 'utf8'),
  rent: readFileSync(new URL('crates/dclutch-rent-contract/src/lib.rs', root), 'utf8'),
  lifecycleRent: readFileSync(new URL('crates/dclutch-rent-contract/src/lifecycle_v2.rs', root), 'utf8'),
  operator: readFileSync(new URL('crates/dclutch-product-runtime-v2-operator/src/found.rs', root), 'utf8'),
  claimsState: readFileSync(new URL('crates/dclutch-claims-svm/src/liability_basis_state_v2.rs', root), 'utf8'),
  claimsPosition: readFileSync(new URL('crates/dclutch-claims-svm/src/protocol_position_v2.rs', root), 'utf8'),
  claimsFounding: readFileSync(new URL('crates/dclutch-claims-svm/src/founding_v5.rs', root), 'utf8'),
});
const outputUrl = new URL('../lib/generated/coreFound.ts', import.meta.url);

function scalar(source, name) {
  const match = sources[source].match(new RegExp(`(?:pub )?const ${name}: [^=]+ = ([0-9]+);`));
  if (!match) throw new Error(`missing Rust scalar ${source}.${name}`);
  return Number(match[1]);
}

function bytes(source, name) {
  const match = sources[source].match(new RegExp(`(?:pub )?const ${name}: \\[u8; [^\\]]+\\] =\\s*(?:\\*b"([^"]+)"|\\[([\\s\\S]*?)\\]);`));
  if (!match) throw new Error(`missing Rust bytes ${source}.${name}`);
  if (match[1]) return [...new TextEncoder().encode(match[1])];
  return [...match[2].matchAll(/0x[0-9a-f]+|\b[0-9]+\b/g)].map((entry) => Number(entry[0]));
}

function byteString(source, name) {
  const match = sources[source].match(new RegExp(`(?:pub )?const ${name}: &\\[u8\\] = b"([^"]+)";`));
  if (!match) throw new Error(`missing Rust byte string ${source}.${name}`);
  return match[1];
}

function array(name, values) {
  return `export const ${name} = Uint8Array.from([${values.map((value) => `0x${value.toString(16).padStart(2, '0')}`).join(', ')}]);\n`;
}

function foundAccountMetas() {
  const marker = sources.operator.indexOf('fn found_metas');
  const start = sources.operator.indexOf('    vec![\n', marker);
  const end = sources.operator.indexOf('    ]\n', start);
  if (marker < 0 || start < 0 || end < 0) throw new Error('missing canonical Found account projection');
  return sources.operator.slice(start + 10, end).split('\n')
    .map((line) => line.trim())
    .filter(Boolean)
    .map((line) => {
      const meta = line.match(/^AccountMeta::(new|new_readonly)\((state\.[a-z_.]+)\.key, (true|false)\),$/);
      if (!meta) throw new Error(`unparsed canonical Found account meta ${line}`);
      return Object.freeze({ field: meta[2], writable: meta[1] === 'new', signer: meta[3] === 'true' });
    });
}

const FOUND_ACCOUNT_LABELS = Object.freeze({
  'state.payer': 'payer', 'state.market': 'Market destination', 'state.rent_credit': 'RentCredit', 'state.rent_program': 'Rent program',
  'state.realm.record.raw': 'Realm raw', 'state.realm.record.staging': 'Realm staging', 'state.product.raw': 'Product raw', 'state.product.staging': 'Product staging',
  'state.result_domain.raw': 'result domain raw', 'state.result_domain.staging': 'result domain staging', 'state.portfolio.raw': 'portfolio raw', 'state.portfolio.staging': 'portfolio staging',
  'state.source_material.record.raw': 'Source material raw', 'state.source_material.record.staging': 'Source staging', 'state.capability_manifest.record.raw': 'capability manifest raw', 'state.capability_manifest.record.staging': 'capability staging',
  'state.execution_release_set.record.raw': 'execution release set raw', 'state.execution_release_set.record.staging': 'release-set staging', 'state.activation_cache': 'activation cache', 'state.core_program': 'Core program',
  'state.core_programdata': 'Core ProgramData', 'state.registry_program': 'Registry program', 'state.rent': 'Rent sysvar', 'state.system_program': 'System program',
  'state.infrastructure_profile': 'infrastructure profile', 'state.registry_artifact.raw': 'Registry artifact raw', 'state.registry_artifact.staging': 'Registry artifact staging',
  'state.registry_programdata': 'Registry ProgramData', 'state.rent_artifact.raw': 'Rent artifact raw', 'state.rent_artifact.staging': 'Rent artifact staging', 'state.rent_programdata': 'Rent ProgramData',
});

function foundAccountLabel(field) {
  const label = FOUND_ACCOUNT_LABELS[field];
  if (label === undefined) throw new Error(`unrecognized canonical Found account field ${field}`);
  return label;
}

let output = '// @generated from canonical Rust/Lean-emitted Core Found ABIs; do not edit.\n';
output += '// Regenerate with: npm run abi:found\n\n';
for (const [source, name] of [
  ['core', 'VERSION'], ['core', 'REQUEST_BYTES'], ['core', 'STATE_BYTES'], ['core', 'ACTION_FOUND_TAG'],
]) output += `export const CORE_${name} = ${scalar(source, name)} as const;\n`;
for (const name of [
  'LIFECYCLE_RENT_CREDIT_BYTES_V2',
  'CREATE_LIFECYCLE_RENT_CREDIT_BYTES_V2',
  'LIFECYCLE_RENT_SCHEMA_VERSION_V2',
]) output += `export const ${name} = ${scalar('lifecycleRent', name)} as const;\n`;
output += `export const CORE_FOUND_ACCOUNT_COUNT_V2 = ${scalar('operator', 'FOUND_ACCOUNT_COUNT_V2')} as const;\n`;
const accountMetas = foundAccountMetas();
if (accountMetas.length !== scalar('operator', 'FOUND_ACCOUNT_COUNT_V2')) throw new Error('Found account count and projection differ');
output += `export const CORE_FOUND_ACCOUNT_LABELS_V2 = Object.freeze(${JSON.stringify(accountMetas.map((meta) => foundAccountLabel(meta.field)))}) as ReadonlyArray<string>;\n`;
output += `export const CORE_FOUND_ACCOUNT_ROLES_V2 = Object.freeze(${JSON.stringify(accountMetas.map(({ signer, writable }) => ({ signer, writable })))}) as ReadonlyArray<Readonly<{ signer: boolean; writable: boolean }>>;\n`;
output += array('CORE_REQUEST_MAGIC', bytes('core', 'REQUEST_MAGIC'));
output += array('MARKET_CORE_STATE_PDA_DOMAIN_V2', bytes('physical', 'MARKET_CORE_STATE_PDA_DOMAIN_V2'));
for (const [source, name] of [
  ['product', 'PRODUCT_RECORD_SCHEMA_ID_V2'], ['product', 'RESULT_DOMAIN_SCHEMA_ID_V2'], ['product', 'PORTFOLIO_SCHEMA_ID_V2'],
  ['realm', 'REALM_SCHEMA_RELEASE_ID_V1'], ['source', 'SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V2'],
  ['capability', 'CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1'],
  ['releaseSet', 'EXECUTION_RELEASE_SET_SCHEMA_RELEASE_ID_V1'],
  ['registry', 'ARTIFACT_RELEASE_SCHEMA_ID_V1'],
]) output += array(name, bytes(source, name));
output += `export const LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2 = new TextEncoder().encode('${byteString('lifecycleRent', 'LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2')}');\n`;
output += array('LIFECYCLE_RENT_INSTRUCTION_MAGIC_V2', bytes('lifecycleRent', 'LIFECYCLE_RENT_INSTRUCTION_MAGIC_V2'));

// ------------------------------------------------ the LIVE Core Market state
// `crates/dclutch-market-core-codec/src/generated.rs` is itself emitted by
// `formal/dclutch-semantics/EmitMarketCoreRust.lean`, so these coordinates
// reach the browser from the Lean semantics through exactly one intermediate
// artifact and are never retyped by hand. Everything the discovery, detail and
// portfolio surfaces need to read a real Market is here.
output += '\n';
output += array('CORE_STATE_MAGIC', bytes('core', 'STATE_MAGIC'));
for (const name of [
  'STATE_VERSION_OFFSET', 'STATE_PHASE_OFFSET', 'STATE_READINESS_OFFSET', 'STATE_TERMINAL_WINNER_OFFSET',
  'STATE_MARKET_ID_OFFSET', 'STATE_IDENTITY_REALM_OFFSET', 'STATE_PRODUCT_RECORD_OFFSET', 'STATE_PRODUCT_ID_OFFSET',
  'STATE_RESOLUTION_POLICY_OFFSET', 'STATE_CAPABILITY_MANIFEST_OFFSET', 'STATE_SELECTED_RELEASE_SET_OFFSET',
  'STATE_REGISTRY_PROGRAM_OFFSET', 'STATE_GENERATION_OFFSET', 'STATE_OUTSTANDING_CAPABILITIES_OFFSET',
  'STATE_RENT_BENEFICIARY_OFFSET', 'STATE_TERMINAL_RECEIPT_OFFSET',
]) output += `export const CORE_${name} = ${scalar('core', name)} as const;\n`;
for (const name of [
  'PHASE_FOUNDING_TAG', 'PHASE_OPEN_TAG', 'PHASE_TERMINAL_TAG', 'PHASE_RETIRING_TAG', 'PHASE_RETIRED_TAG',
  'READINESS_PREPAID_TAG', 'READINESS_READY_TAG', 'READINESS_CONSUMED_TAG',
]) output += `export const CORE_${name} = ${scalar('core', name)} as const;\n`;

// ------------------------------------- the LIVE Claims LiabilityBasisV2 state
// A Core Market root carries identity and lifecycle. The per-claim SUPPLY
// vector and every owner's BALANCE vector live in Claims-owned LiabilityBasisV2
// state, at PDAs derived from the Market and the owner. Without these the
// browser can decode a Market and still have nothing true to say about its
// economics.
const aggregateSeed = byteString('claimsState', 'LIABILITY_BASIS_MARKET_SEED_V2');
const foundingAggregateSeed = byteString('claimsFounding', 'CLAIMS_FOUNDING_AGGREGATE_SEED_V5');
if (aggregateSeed !== foundingAggregateSeed) {
  throw new Error(`the Claims aggregate seed domain has two spellings: ${aggregateSeed} vs ${foundingAggregateSeed}`);
}
output += '\n';
output += array('LIABILITY_BASIS_MARKET_MAGIC_V2', bytes('claimsState', 'LIABILITY_BASIS_MARKET_MAGIC_V2'));
output += array('LIABILITY_BASIS_POSITION_MAGIC_V2', bytes('claimsState', 'LIABILITY_BASIS_POSITION_MAGIC_V2'));
for (const name of [
  'LIABILITY_BASIS_STATE_VERSION_V2',
  'LIABILITY_BASIS_MARKET_HEADER_BYTES_V2',
  'LIABILITY_BASIS_POSITION_HEADER_BYTES_V2',
]) output += `export const ${name} = ${scalar('claimsState', name)} as const;\n`;
for (const name of [
  'MARKET_CLAIM_COUNT_OFFSET', 'MARKET_REVISION_OFFSET', 'MARKET_LOGICAL_ID_OFFSET', 'MARKET_RELEASE_SET_OFFSET',
  'MARKET_REGISTRY_OFFSET', 'MARKET_PRODUCT_OFFSET', 'MARKET_BASIS_OFFSET', 'MARKET_REALM_OFFSET',
  'MARKET_CUSTODY_CONTEXT_OFFSET', 'MARKET_GENERATION_OFFSET',
  'POSITION_CLAIM_COUNT_OFFSET', 'POSITION_REVISION_OFFSET', 'POSITION_MARKET_OFFSET', 'POSITION_OWNER_OFFSET',
  'POSITION_BASIS_OFFSET', 'POSITION_RESERVED_OFFSET',
]) output += `export const LIABILITY_BASIS_${name} = ${scalar('claimsState', name)} as const;\n`;
output += `export const LIABILITY_BASIS_MARKET_SEED_V2 = new TextEncoder().encode('${aggregateSeed}');\n`;
output += `export const LIABILITY_BASIS_POSITION_SEED_V2 = new TextEncoder().encode('${byteString('claimsPosition', 'PROTOCOL_POSITION_STATE_SEED_V2')}');\n`;

// -------------------------------------------------------- the Realm record
// A Market names its Realm by content identity, and on a live chain the
// canonical body is a finalized Registry record rather than a Core account. The
// browser reacquires it and re-hashes it, so it needs the body layout.
output += '\n';
output += array('REALM_MAGIC', bytes('realm', 'REALM_MAGIC'));
output += `export const REALM_BYTES = ${scalar('realm', 'REALM_BYTES')} as const;\n`;
output += `export const REALM_SCHEMA_VERSION = ${scalar('realm', 'REALM_SCHEMA_VERSION')} as const;\n`;
for (const name of [
  'REALM_MINT_AUTHORITY_POLICY_OFFSET', 'REALM_FREEZE_AUTHORITY_POLICY_OFFSET',
  'REALM_TOKEN_PROGRAM_OFFSET', 'REALM_COLLATERAL_MINT_OFFSET', 'REALM_ADAPTER_RELEASE_ID_OFFSET',
]) output += `export const ${name} = ${scalar('realm', name)} as const;\n`;

if (process.argv.includes('--check')) {
  if (readFileSync(outputUrl, 'utf8') !== output) {
    console.error('Core Found TypeScript ABI is stale');
    process.exit(1);
  }
} else {
  writeFileSync(fileURLToPath(outputUrl), output);
}
