import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const root = new URL('../../../', import.meta.url);
const sources = Object.freeze({
  core: readFileSync(new URL('crates/dclutch-market-core-codec/src/generated.rs', root), 'utf8'),
  foundFrame: readFileSync(new URL('crates/dclutch-market-core-codec/src/found_frame_v3.rs', root), 'utf8'),
  physical: readFileSync(new URL('crates/dclutch-market-core-codec/src/generated_physical.rs', root), 'utf8'),
  product: readFileSync(new URL('crates/dclutch-product-runtime-v2-admission/src/lib.rs', root), 'utf8'),
  realm: readFileSync(new URL('crates/dclutch-realm-contract/src/lib.rs', root), 'utf8'),
  source: readFileSync(new URL('crates/dclutch-source-contract/src/generated_source_material_v3.rs', root), 'utf8'),
  sourceJoin: readFileSync(new URL('crates/dclutch-source-contract/src/provider_join_v2.rs', root), 'utf8'),
  sourceCapacity: readFileSync(new URL('crates/dclutch-source-contract/src/generated_principal_capacity_v1.rs', root), 'utf8'),
  payoff: readFileSync(new URL('crates/dclutch-product-payoff-v2-codec/src/generated_admission_v3.rs', root), 'utf8'),
  capability: readFileSync(new URL('crates/dclutch-capability-contract/src/lib.rs', root), 'utf8'),
  releaseSet: readFileSync(new URL('crates/dclutch-release-set-contract/src/lib.rs', root), 'utf8'),
  registry: readFileSync(new URL('crates/dclutch-registry-contract/src/artifact.rs', root), 'utf8'),
  rent: readFileSync(new URL('crates/dclutch-rent-contract/src/lib.rs', root), 'utf8'),
  lifecycleRent: readFileSync(new URL('crates/dclutch-rent-contract/src/lifecycle_v2.rs', root), 'utf8'),
  operator: readFileSync(new URL('crates/dclutch-product-runtime-v2-operator/src/found.rs', root), 'utf8'),
  splineAuthoring: readFileSync(new URL('tools/local-validator/bootstrap/successor/src/spline_product.rs', root), 'utf8'),
  claimsState: readFileSync(new URL('crates/dclutch-claims-svm/src/liability_basis_state_v2.rs', root), 'utf8'),
  claimsPosition: readFileSync(new URL('crates/dclutch-claims-svm/src/protocol_position_v2.rs', root), 'utf8'),
  claimsFounding: readFileSync(new URL('crates/dclutch-claims-svm/src/founding_v5.rs', root), 'utf8'),
  custody: readFileSync(new URL('crates/dclutch-custody-contract/src/lib.rs', root), 'utf8'),
});
const outputUrl = new URL('../lib/generated/coreFound.ts', import.meta.url);

function scalar(source, name) {
  const literal = sources[source].match(new RegExp(`(?:pub )?const ${name}: [^=]+ = ([0-9]+);`));
  if (literal) return Number(literal[1]);
  const additive = sources[source].match(new RegExp(`(?:pub )?const ${name}: [^=]+ = ([A-Z][A-Z0-9_]+) \\+ ([0-9]+);`));
  if (additive) return scalar(source, additive[1]) + Number(additive[2]);
  const alias = sources[source].match(new RegExp(`(?:pub )?const ${name}: [^=]+ = ([A-Z][A-Z0-9_]+);`));
  if (alias) return scalar(source, alias[1]);
  throw new Error(`missing Rust scalar ${source}.${name}`);
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

function stringConstant(source, name) {
  const match = sources[source].match(new RegExp(`(?:pub\\([^)]*\\) )?const ${name}: &str = "([^"]+)";`));
  if (!match) throw new Error(`missing Rust string ${source}.${name}`);
  return match[1];
}

/** Read the offset the state encoder writes a magic at. */
function lifecycleRentMagicOffset(name) {
  const match = sources.lifecycleRent.match(new RegExp(`put\\(&mut output, ([0-9]+), &${name}\\)`));
  if (!match) throw new Error(`missing Rust magic write for ${name}`);
  return Number(match[1]);
}

/** Read one `LifecycleRentActionV2` discriminant, which the wire carries as u8. */
function lifecycleRentAction(variant) {
  const enumeration = sources.lifecycleRent.match(/pub enum LifecycleRentActionV2 \{([\s\S]*?)\n\}/);
  if (!enumeration) throw new Error('missing Rust LifecycleRentActionV2 enumeration');
  const match = enumeration[1].match(new RegExp(`\\n\\s*${variant} = ([0-9]+),`));
  if (!match) throw new Error(`missing Rust LifecycleRentActionV2::${variant} discriminant`);
  return Number(match[1]);
}

function compartmentTag(variant) {
  const enumeration = sources.custody.match(/pub enum CompartmentV1 \{([\s\S]*?)\n\}/);
  if (!enumeration) throw new Error('missing Rust CompartmentV1 enumeration');
  const match = enumeration[1].match(new RegExp(`\\n\\s*${variant} = ([0-9]+),`));
  if (!match) throw new Error(`missing Rust CompartmentV1::${variant} discriminant`);
  return Number(match[1]);
}

function array(name, values) {
  return `export const ${name} = Uint8Array.from([${values.map((value) => `0x${value.toString(16).padStart(2, '0')}`).join(', ')}]);\n`;
}

function foundAccountMetas() {
  const marker = sources.operator.indexOf('fn found_metas');
  const nextFunction = sources.operator.indexOf('\nfn ', marker + 1);
  const body = marker < 0 ? '' : sources.operator.slice(marker, nextFunction < 0 ? undefined : nextFunction);
  const projection = body.match(/let accounts = vec!\[\s*([\s\S]*?)\n\s*\];/);
  if (!projection) throw new Error('missing canonical Found account projection');
  return projection[1].split('\n')
    .map((line) => line.trim())
    .filter(Boolean)
    .map((line) => {
      const meta = line.match(/^AccountMeta::(new|new_readonly)\((state\.[a-z_.]+)\.key, (true|false)\),$/);
      if (!meta) throw new Error(`unparsed canonical Found account meta ${line}`);
      return Object.freeze({ field: meta[2], writable: meta[1] === 'new', signer: meta[3] === 'true' });
    });
}

function foundPriceGateAccountMetas() {
  const marker = sources.operator.indexOf('fn extend_with_price_gate');
  const nextFunction = sources.operator.indexOf('\nfn ', marker + 1);
  const body = marker < 0 ? '' : sources.operator.slice(marker, nextFunction < 0 ? undefined : nextFunction);
  const entries = [...body.matchAll(/accounts\.push\(AccountMeta::(new|new_readonly)\((certificate\.[a-z_.]+)\.key, (true|false)\)\);/g)]
    .map((match) => Object.freeze({ field: match[2], writable: match[1] === 'new', signer: match[3] === 'true' }));
  if (entries.length === 0) throw new Error('missing canonical Found price-gate extension');
  return entries;
}

const FOUND_ACCOUNT_LABELS = Object.freeze({
  'state.payer': 'payer', 'state.market': 'Market destination', 'state.rent_credit': 'RentCredit', 'state.rent_program': 'Rent program',
  'state.realm.record.raw': 'Realm raw', 'state.realm.record.staging': 'Realm staging', 'state.product.raw': 'Product raw', 'state.product.staging': 'Product staging',
  'state.result_domain.raw': 'result domain raw', 'state.result_domain.staging': 'result domain staging', 'state.portfolio.raw': 'portfolio raw', 'state.portfolio.staging': 'portfolio staging',
  'state.linked_basis.raw': 'linked basis raw', 'state.linked_basis.staging': 'linked basis staging', 'state.source_material.record.raw': 'Source material raw', 'state.source_material.record.staging': 'Source staging',
  'state.source_spec.record.raw': 'Source spec raw', 'state.source_spec.record.staging': 'Source spec staging', 'state.capacity_profile.record.raw': 'capacity profile raw', 'state.capacity_profile.record.staging': 'capacity profile staging',
  'state.manipulation_floor.record.raw': 'manipulation floor raw', 'state.manipulation_floor.record.staging': 'manipulation floor staging', 'state.capability_manifest.record.raw': 'capability manifest raw', 'state.capability_manifest.record.staging': 'capability staging',
  'state.activation_cache': 'activation cache', 'state.core_program': 'Core program',
  'state.core_programdata': 'Core ProgramData', 'state.registry_program': 'Registry program', 'state.rent': 'Rent sysvar', 'state.system_program': 'System program',
  'state.infrastructure_profile': 'infrastructure profile', 'state.registry_artifact.raw': 'Registry artifact raw', 'state.registry_artifact.staging': 'Registry artifact staging',
  'state.registry_programdata': 'Registry ProgramData', 'state.rent_artifact.raw': 'Rent artifact raw', 'state.rent_artifact.staging': 'Rent artifact staging', 'state.rent_programdata': 'Rent ProgramData',
  'certificate.raw': 'price-gate certificate raw', 'certificate.staging': 'price-gate certificate staging',
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
// The operator deliberately re-exports this frame width from the codec. Read
// the defining module, not the re-export, so the generator follows the one
// semantic owner and does not mistake a `pub use` for a numeric definition.
const foundAccountCount = scalar('foundFrame', 'FOUND_ACCOUNT_COUNT_V3');
output += `export const CORE_FOUND_ACCOUNT_COUNT_V3 = ${foundAccountCount} as const;\n`;
const accountMetas = foundAccountMetas();
if (accountMetas.length !== foundAccountCount) throw new Error('Found account count and projection differ');
output += `export const CORE_FOUND_ACCOUNT_LABELS_V3 = Object.freeze(${JSON.stringify(accountMetas.map((meta) => foundAccountLabel(meta.field)))}) as ReadonlyArray<string>;\n`;
output += `export const CORE_FOUND_ACCOUNT_ROLES_V3 = Object.freeze(${JSON.stringify(accountMetas.map(({ signer, writable }) => ({ signer, writable })))}) as ReadonlyArray<Readonly<{ signer: boolean; writable: boolean }>>;\n`;
const foundPriceGateAccountCount = scalar('foundFrame', 'FOUND_PRICE_GATE_ACCOUNT_COUNT_V3');
const foundPriceGateRawIndex = scalar('foundFrame', 'FOUND_PRICE_GATE_RAW_INDEX_V3');
const foundPriceGateStagingIndex = scalar('foundFrame', 'FOUND_PRICE_GATE_STAGING_INDEX_V3');
const priceGateAccountMetas = foundPriceGateAccountMetas();
const extendedAccountMetas = Object.freeze([...accountMetas, ...priceGateAccountMetas]);
if (foundPriceGateRawIndex !== foundAccountCount
    || foundPriceGateStagingIndex !== foundPriceGateRawIndex + 1
    || extendedAccountMetas.length !== foundPriceGateAccountCount) {
  throw new Error('Found price-gate count, indices, and projection differ');
}
output += `export const CORE_FOUND_PRICE_GATE_RAW_INDEX_V3 = ${foundPriceGateRawIndex} as const;\n`;
output += `export const CORE_FOUND_PRICE_GATE_STAGING_INDEX_V3 = ${foundPriceGateStagingIndex} as const;\n`;
output += `export const CORE_FOUND_PRICE_GATE_ACCOUNT_COUNT_V3 = ${foundPriceGateAccountCount} as const;\n`;
output += `export const CORE_FOUND_PRICE_GATE_ACCOUNT_LABELS_V3 = Object.freeze(${JSON.stringify(extendedAccountMetas.map((meta) => foundAccountLabel(meta.field)))}) as ReadonlyArray<string>;\n`;
output += `export const CORE_FOUND_PRICE_GATE_ACCOUNT_ROLES_V3 = Object.freeze(${JSON.stringify(extendedAccountMetas.map(({ signer, writable }) => ({ signer, writable })))}) as ReadonlyArray<Readonly<{ signer: boolean; writable: boolean }>>;\n`;
output += `export const SPLINE_PRODUCT_AUTHORING_COMMAND_V1 = '${stringConstant('splineAuthoring', 'COMMAND_V1')}' as const;\n`;
output += `export const SPLINE_PRODUCT_AUTHORING_REPORT_SCHEMA_V1 = '${stringConstant('splineAuthoring', 'REPORT_SCHEMA_V1')}' as const;\n`;
output += array('CORE_REQUEST_MAGIC', bytes('core', 'REQUEST_MAGIC'));
output += array('MARKET_CORE_STATE_PDA_DOMAIN_V2', bytes('physical', 'MARKET_CORE_STATE_PDA_DOMAIN_V2'));
for (const [source, name] of [
  ['product', 'PRODUCT_RECORD_SCHEMA_ID_V2'], ['product', 'RESULT_DOMAIN_SCHEMA_ID_V2'], ['product', 'PORTFOLIO_SCHEMA_ID_V2'],
  ['realm', 'REALM_SCHEMA_RELEASE_ID_V1'], ['source', 'SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3'],
  ['sourceJoin', 'SOURCE_SPEC_SCHEMA_ID_V1'], ['sourceJoin', 'SOURCE_CAPACITY_PROFILE_SCHEMA_ID_V1'],
  ['sourceCapacity', 'MANIPULATION_FLOOR_SCHEMA_RELEASE_ID_V1'], ['payoff', 'GRADED_BASIS_RECORD_SCHEMA_ID_V3'],
  ['payoff', 'PRICE_GATE_RECORD_SCHEMA_ID_V1'],
  ['capability', 'CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1'],
  ['releaseSet', 'EXECUTION_RELEASE_SET_SCHEMA_RELEASE_ID_V1'],
  ['registry', 'ARTIFACT_RELEASE_SCHEMA_ID_V1'],
]) output += array(name, bytes(source, name));
output += `export const LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2 = new TextEncoder().encode('${byteString('lifecycleRent', 'LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2')}');\n`;
output += array('LIFECYCLE_RENT_INSTRUCTION_MAGIC_V2', bytes('lifecycleRent', 'LIFECYCLE_RENT_INSTRUCTION_MAGIC_V2'));
output += array('LIFECYCLE_RENT_CREDIT_MAGIC_V2', bytes('lifecycleRent', 'LIFECYCLE_RENT_CREDIT_MAGIC_V2'));
// Read where the state encoder actually writes its magic rather than assuming
// the front. Zero is the obvious answer, which is exactly why it should come
// from the authority: a decoder that hard-codes a coordinate the encoder owns
// is the drift `abi-coverage` exists to refuse.
output += `export const LIFECYCLE_RENT_CREDIT_MAGIC_OFFSET_V2 = ${lifecycleRentMagicOffset('LIFECYCLE_RENT_CREDIT_MAGIC_V2')} as const;\n`;
// The action byte and the offset it sits at. Both were literals in
// `lib/coreFound.ts`, and the action literal was WRONG -- it said 0 where
// `LifecycleRentActionV2::Create` is 1, so every lifecycle RentCredit the
// browser built was refused at decode by the contract. It went unseen because
// `/found` only ever downloaded the packet. Emitting the discriminant is the
// fix; a second hand-written copy of a wire constant is the defect.
output += `export const LIFECYCLE_RENT_INSTRUCTION_ACTION_OFFSET_V2 = ${scalar('lifecycleRent', 'INSTRUCTION_ACTION_OFFSET')} as const;\n`;
output += `export const LIFECYCLE_RENT_ACTION_CREATE_V2 = ${lifecycleRentAction('Create')} as const;\n`;

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
  'STATE_PRINCIPAL_CAP_SETS_OFFSET',
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

// --------------------------------------------- the Market's Custody namespace
// The Hoard Vault address is NOT a fact of the Market root: it is
// `[custody-vault-domain, market, release_set, context, compartment]` under the
// Custody program, and `context` is caller-chosen at founding. The Claims
// aggregate persists it, so a reader holding a Market can now name the Hoard --
// but only by taking these three coordinates from the crate that derives them
// on chain, never by retyping them.
output += '\n';
output += `export const CUSTODY_VAULT_PDA_DOMAIN_V1 = new TextEncoder().encode('${byteString('custody', 'CUSTODY_VAULT_PDA_DOMAIN_V1')}');\n`;
output += `export const CUSTODY_AUTHORITY_PDA_DOMAIN_V1 = new TextEncoder().encode('${byteString('custody', 'CUSTODY_AUTHORITY_PDA_DOMAIN_V1')}');\n`;
output += `export const CUSTODY_COMPARTMENT_HOARD_PRINCIPAL_TAG_V1 = ${compartmentTag('HoardPrincipal')} as const;\n`;

// -------------------------------------------------------- the Realm record
// The Realm body layout used to be re-emitted here from the crate's literals.
// It is now Lean-emitted directly into lib/generated/realmPositionV1.ts by
// `npm run abi:realm-position`; a Realm coordinate stated in two generated
// modules would be exactly the drift this pipeline exists to remove.

if (process.argv.includes('--check')) {
  if (readFileSync(outputUrl, 'utf8') !== output) {
    console.error('Core Found TypeScript ABI is stale');
    process.exit(1);
  }
} else {
  writeFileSync(fileURLToPath(outputUrl), output);
}
