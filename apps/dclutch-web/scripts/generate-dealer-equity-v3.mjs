import { readFileSync, renameSync, unlinkSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const root = new URL('../../../', import.meta.url);
const sources = Object.freeze({
  request: readFileSync(new URL('programs/dclutch-trading-sbf/src/dealer/v3_equity_operator.rs', root), 'utf8'),
  hot: readFileSync(new URL('programs/dclutch-trading-sbf/src/dealer/v3_hot_artifact.rs', root), 'utf8'),
  lp: readFileSync(new URL('programs/dclutch-trading-sbf/src/dealer/v3_multi_lp.rs', root), 'utf8'),
  obligation: readFileSync(new URL('programs/dclutch-trading-sbf/src/dealer/v3_obligation.rs', root), 'utf8'),
  release: readFileSync(new URL('programs/dclutch-trading-sbf/src/dealer/v3_release.rs', root), 'utf8'),
  dealer: readFileSync(new URL('programs/dclutch-trading-sbf/src/dealer/mod.rs', root), 'utf8'),
  delta: readFileSync(new URL('crates/dclutch-claims-svm/src/signed_delta_v3.rs', root), 'utf8'),
  strategy: readFileSync(new URL('crates/dclutch-execution-strategy-contract/src/generated_v2.rs', root), 'utf8'),
});
const outputUrl = new URL('../lib/generated/dealerEquityV3.ts', import.meta.url);

function scalar(source, name) {
  const match = sources[source].match(new RegExp(`(?:pub(?:\\(crate\\))? )?const ${name}: [^=]+ = ([0-9_]+);`));
  if (!match) throw new Error(`missing Rust scalar ${source}.${name}`);
  return Number(match[1].replaceAll('_', ''));
}

function bytes(source, name) {
  const match = sources[source].match(new RegExp(`(?:pub(?:\\(crate\\))? )?const ${name}: (?:&\\[u8\\]|\\[u8; [^\\]]+\\]) =\\s*(?:\\*?b"([^"]+)"|\\[([\\s\\S]*?)\\]);`));
  if (!match) throw new Error(`missing Rust bytes ${source}.${name}`);
  if (match[1]) return [...new TextEncoder().encode(match[1])];
  return [...match[2].matchAll(/0x[0-9a-f]+|\b[0-9]+\b/g)].map((entry) => Number(entry[0]));
}

function array(name, values) {
  return `export const ${name} = Uint8Array.from([${values.map((value) => `0x${value.toString(16).padStart(2, '0')}`).join(', ')}]);\n`;
}

const scalars = Object.freeze([
  ['request', 'DEALER_EQUITY_REQUEST_VERSION_V3'],
  ['request', 'DEALER_EQUITY_SELECTOR_OFFSET_V3'],
  ['request', 'DEALER_EQUITY_HEADER_BYTES_V3'],
  ['request', 'DEALER_EQUITY_CLAIMS_PACKET_BYTES_OFFSET_V3'],
  ['request', 'DEALER_EQUITY_CONTRIBUTE_P0_SELECTOR_V3'],
  ['request', 'DEALER_EQUITY_CONTRIBUTE_P1_SELECTOR_V3'],
  ['request', 'DEALER_EQUITY_CONTRIBUTE_P2_SELECTOR_V3'],
  ['request', 'DEALER_EQUITY_REDEEM_P0_SELECTOR_V3'],
  ['request', 'DEALER_EQUITY_REDEEM_P1_SELECTOR_V3'],
  ['request', 'DEALER_EQUITY_REDEEM_P2_SELECTOR_V3'],
  ['hot', 'DEALER_HOT_INJECTED_ACCOUNT_COUNT_V3'],
  ['hot', 'DEALER_CUSTODY_TRANSFER_ACCOUNT_COUNT_V3'],
  ['hot', 'DEALER_SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3'],
  ['hot', 'DEALER_EQUITY_LOCAL_ACCOUNT_COUNT_V3'],
  ['lp', 'DEALER_LP_POSITION_BYTES_V3'],
  ['lp', 'DEALER_LP_POSITION_VERSION_V3'],
  ['obligation', 'DEALER_OBLIGATION_HEADER_BYTES_V3'],
  ['obligation', 'DEALER_OBLIGATION_VERSION_V3'],
  ['delta', 'SIGNED_DELTA_PLAN_HEADER_BYTES_V3'],
  ['delta', 'SIGNED_DELTA_POSITION_BYTES_V3'],
  ['delta', 'SIGNED_DELTA_BYTES_V3'],
  ['delta', 'SIGNED_DELTA_ROW_BYTES_V3'],
  ['delta', 'SIGNED_DELTA_WIRE_VERSION_V3'],
  ['strategy', 'EXECUTION_STRATEGY_PROGRAM_BYTES_V2'],
  ['strategy', 'EXECUTION_STRATEGY_SCHEMA_VERSION_V2'],
  ['strategy', 'EXECUTION_STRATEGY_ARTIFACT_PROFILE_V2'],
  ['strategy', 'STRATEGY_DISPOSITION_OFFSET_V2'],
]);

let output = '// @generated from canonical Rust Dealer V3 ABIs; do not edit.\n';
output += '// Regenerate with: npm run abi:dealer-v3\n\n';
for (const [source, name] of scalars) output += `export const ${name} = ${scalar(source, name)} as const;\n`;
for (const [source, name] of [
  ['request', 'DEALER_EQUITY_REQUEST_MAGIC_V3'],
  ['lp', 'DEALER_LP_POSITION_MAGIC_V3'],
  ['lp', 'DEALER_LP_POSITION_PDA_DOMAIN_V3'],
  ['obligation', 'DEALER_OBLIGATION_MAGIC_V3'],
  ['obligation', 'DEALER_OBLIGATION_PDA_DOMAIN_V3'],
  ['delta', 'SIGNED_DELTA_PLAN_MAGIC_V3'],
  ['strategy', 'EXECUTION_STRATEGY_PROGRAM_MAGIC_V2'],
  ['dealer', 'DEALER_KIND_PREIMAGE_V2'],
  ['dealer', 'DEALER_CONFIG_SCHEMA_PREIMAGE_V2'],
  ['dealer', 'DEALER_ROOT_SCHEMA_PREIMAGE_V2'],
  ['release', 'DEALER_EQUITY_REQUEST_SCHEMA_PREIMAGE_V3'],
]) output += array(name, bytes(source, name));

const destination = fileURLToPath(outputUrl);
const check = process.argv.includes('--check');
if (check) {
  if (readFileSync(outputUrl, 'utf8') !== output) throw new Error('generated Dealer V3 ABI is stale; run npm run abi:dealer-v3');
} else {
  const temporary = `${destination}.tmp-${process.pid}`;
  try { writeFileSync(temporary, output); renameSync(temporary, destination); }
  catch (error) { try { unlinkSync(temporary); } catch {} throw error; }
}
