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
  operator: readFileSync(new URL('crates/dclutch-product-runtime-v2-operator/src/found.rs', root), 'utf8'),
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

function foundAccountLabels() {
  const marker = sources.operator.indexOf('fn all_accounts');
  const start = sources.operator.indexOf('    [\n', marker);
  const end = sources.operator.indexOf('    ]\n', start);
  if (marker < 0 || start < 0 || end < 0) throw new Error('missing canonical Found account projection');
  const fields = sources.operator.slice(start + 6, end).split('\n')
    .map((line) => line.trim().replace(/,$/, ''))
    .filter(Boolean);
  const labels = Object.freeze({
    'state.payer': 'payer', 'state.market': 'Market destination', 'state.rent_credit': 'RentCredit', 'state.rent_program': 'Rent program',
    'state.realm.record.raw': 'Realm raw', 'state.realm.record.staging': 'Realm staging', 'state.product.raw': 'Product raw', 'state.product.staging': 'Product staging',
    'state.result_domain.raw': 'result domain raw', 'state.result_domain.staging': 'result domain staging', 'state.portfolio.raw': 'portfolio raw', 'state.portfolio.staging': 'portfolio staging',
    'state.source_material.record.raw': 'Source material raw', 'state.source_material.record.staging': 'Source staging', 'state.capability_manifest.record.raw': 'capability manifest raw', 'state.capability_manifest.record.staging': 'capability staging',
    'state.execution_release_set.record.raw': 'execution release set raw', 'state.execution_release_set.record.staging': 'release-set staging', 'state.activation_cache': 'activation cache', 'state.core_program': 'Core program',
    'state.core_programdata': 'Core ProgramData', 'state.registry_program': 'Registry program', 'state.rent': 'Rent sysvar', 'state.system_program': 'System program',
    'state.infrastructure_profile': 'infrastructure profile', 'state.registry_artifact.raw': 'Registry artifact raw', 'state.registry_artifact.staging': 'Registry artifact staging',
    'state.registry_programdata': 'Registry ProgramData', 'state.rent_artifact.raw': 'Rent artifact raw', 'state.rent_artifact.staging': 'Rent artifact staging', 'state.rent_programdata': 'Rent ProgramData',
  });
  return fields.map((field) => {
    const label = labels[field];
    if (label === undefined) throw new Error(`unrecognized canonical Found account field ${field}`);
    return label;
  });
}

let output = '// @generated from canonical Rust/Lean-emitted Core Found ABIs; do not edit.\n';
output += '// Regenerate with: npm run abi:found\n\n';
for (const [source, name] of [
  ['core', 'VERSION'], ['core', 'REQUEST_BYTES'], ['core', 'STATE_BYTES'], ['core', 'ACTION_FOUND_TAG'],
]) output += `export const CORE_${name} = ${scalar(source, name)} as const;\n`;
output += `export const CORE_FOUND_ACCOUNT_COUNT_V2 = ${scalar('operator', 'FOUND_ACCOUNT_COUNT_V2')} as const;\n`;
const accountLabels = foundAccountLabels();
if (accountLabels.length !== scalar('operator', 'FOUND_ACCOUNT_COUNT_V2')) throw new Error('Found account count and projection differ');
output += `export const CORE_FOUND_ACCOUNT_LABELS_V2 = Object.freeze(${JSON.stringify(accountLabels)}) as ReadonlyArray<string>;\n`;
output += array('CORE_REQUEST_MAGIC', bytes('core', 'REQUEST_MAGIC'));
output += array('MARKET_CORE_STATE_PDA_DOMAIN_V2', bytes('physical', 'MARKET_CORE_STATE_PDA_DOMAIN_V2'));
for (const [source, name] of [
  ['product', 'PRODUCT_RECORD_SCHEMA_ID_V2'], ['product', 'RESULT_DOMAIN_SCHEMA_ID_V2'], ['product', 'PORTFOLIO_SCHEMA_ID_V2'],
  ['realm', 'REALM_SCHEMA_RELEASE_ID_V1'], ['source', 'SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V2'],
  ['capability', 'CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1'],
  ['releaseSet', 'EXECUTION_RELEASE_SET_SCHEMA_RELEASE_ID_V1'],
  ['registry', 'ARTIFACT_RELEASE_SCHEMA_ID_V1'],
]) output += array(name, bytes(source, name));
output += `export const RENT_CREDIT_PDA_DOMAIN_V1 = new TextEncoder().encode('${byteString('rent', 'RENT_CREDIT_PDA_DOMAIN_V1')}');\n`;

if (process.argv.includes('--check')) {
  if (readFileSync(outputUrl, 'utf8') !== output) {
    console.error('Core Found TypeScript ABI is stale');
    process.exit(1);
  }
} else {
  writeFileSync(fileURLToPath(outputUrl), output);
}
