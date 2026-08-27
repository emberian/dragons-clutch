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
  config: readFileSync(new URL('crates/dclutch-dealer-codec/src/config_v4.rs', root), 'utf8'),
  delta: readFileSync(new URL('crates/dclutch-claims-svm/src/signed_delta_v3.rs', root), 'utf8'),
  deltaFrame: readFileSync(new URL('crates/dclutch-claims-svm/src/frame_spec_v1.rs', root), 'utf8'),
  strategy: readFileSync(new URL('crates/dclutch-execution-strategy-contract/src/generated_v2.rs', root), 'utf8'),
  accountProfile: readFileSync(new URL('crates/dclutch-account-profile-contract/src/v2.rs', root), 'utf8'),
  lpProfile: readFileSync(new URL('programs/dclutch-trading-sbf/src/dealer/v3_lp_artifacts.rs', root), 'utf8'),
  scenarioProfile: readFileSync(new URL('programs/dclutch-trading-sbf/src/dealer/v3_trade_profile.rs', root), 'utf8'),
  scenarioArtifacts: readFileSync(new URL('programs/dclutch-trading-sbf/src/dealer/v3_trade_artifacts.rs', root), 'utf8'),
  basis: readFileSync(new URL('crates/dclutch-product-payoff-v2-codec/src/runtime_v3.rs', root), 'utf8'),
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
  ['config', 'DEALER_CONFIG_VERSION_V4'],
  ['config', 'DEALER_CONFIG_BYTES_V4'],
  ['config', 'DEALER_CONFIG_RELEASE_SET_OFFSET_V4'],
  ['config', 'DEALER_CONFIG_REALM_OFFSET_V4'],
  ['config', 'DEALER_CONFIG_POSITION_OWNER_OFFSET_V4'],
  ['config', 'DEALER_CONFIG_LOCKED_CAPITAL_FLOOR_OFFSET_V4'],
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
  ['deltaFrame', 'SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3', 'DEALER_SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3'],
  ['hot', 'DEALER_EQUITY_LOCAL_ACCOUNT_COUNT_V3'],
  ['hot', 'CUSTODY_SCALAR_BASE_V3'],
  ['hot', 'CUSTODY_SCALAR_STRIDE_V3'],
  ['hot', 'CUSTODY_IDENTITY_BASE_V3'],
  ['hot', 'CUSTODY_IDENTITY_STRIDE_V3'],
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
  ['accountProfile', 'VERSION', 'ACCOUNT_PROFILE_VERSION_V2'],
  ['accountProfile', 'TRUSTED_ENVIRONMENT_ARTIFACT_PROFILE'],
  ['accountProfile', 'LIFECYCLE_PRESTATE_ARTIFACT_PROFILE'],
  ['accountProfile', 'DYNAMIC_FIXED_SPAN_ARTIFACT_PROFILE'],
  ['accountProfile', 'HEADER_BYTES', 'ACCOUNT_PROFILE_HEADER_BYTES_V2'],
  ['accountProfile', 'DYNAMIC_FIXED_SPAN_HEADER_BYTES'],
  ['accountProfile', 'DYNAMIC_FIXED_SPAN_COUNT_OFFSET'],
  ['accountProfile', 'DYNAMIC_FIXED_SPAN_RESERVED_OFFSET'],
  ['accountProfile', 'DYNAMIC_FIXED_SPAN_ENTRY_BYTES'],
  ['accountProfile', 'DYNAMIC_FIXED_SPAN_ENTRY_INSERTION_OFFSET'],
  ['accountProfile', 'DYNAMIC_FIXED_SPAN_ENTRY_COUNT_SCALAR_OFFSET'],
  ['accountProfile', 'DYNAMIC_FIXED_SPAN_ENTRY_RULE_START_OFFSET'],
  ['accountProfile', 'DYNAMIC_FIXED_SPAN_ENTRY_RULE_STRIDE_OFFSET'],
  ['accountProfile', 'DYNAMIC_FIXED_SPAN_ENTRY_MIN_OFFSET'],
  ['accountProfile', 'DYNAMIC_FIXED_SPAN_ENTRY_MAX_OFFSET'],
  ['accountProfile', 'DYNAMIC_FIXED_SPAN_ENTRY_STEP_OFFSET'],
  ['accountProfile', 'RULE_BYTES', 'ACCOUNT_PROFILE_RULE_BYTES_V2'],
  ['accountProfile', 'OPERATION_BYTES', 'ACCOUNT_PROFILE_OPERATION_BYTES_V2'],
  ['accountProfile', 'TRUSTED_ENVIRONMENT_SCALAR_OFFSET'],
  ['accountProfile', 'TRUSTED_ENVIRONMENT_KIND_OFFSET'],
  ['accountProfile', 'TRUSTED_ENVIRONMENT_RESERVED_OFFSET'],
  ['accountProfile', 'TRUSTED_EXECUTING_PROGRAM_IDENTITY_OFFSET'],
  ['accountProfile', 'TRUSTED_EXECUTING_PROGRAM_KIND_OFFSET'],
  ['accountProfile', 'TRUSTED_EXECUTING_PROGRAM_RESERVED_OFFSET'],
  ['lpProfile', 'DEALER_LP_OPEN_ACCOUNT_COUNT_V3'],
  ['lpProfile', 'DEALER_LP_CLOSE_ACCOUNT_COUNT_V3'],
  ['lpProfile', 'DEALER_LP_STATE_ACCOUNT_V3'],
  ['lpProfile', 'DEALER_LP_SCALAR_COUNT_V3'],
  ['lpProfile', 'DEALER_LP_IDENTITY_COUNT_V3'],
  ['scenarioProfile', 'DEALER_SCENARIO_PROFILE_FIXED_RULES_V4'],
  ['scenarioProfile', 'DEALER_SCENARIO_PROFILE_SPANS_V4'],
  ['scenarioProfile', 'DEALER_SCENARIO_PROFILE_SPAN_RULES_V4'],
  ['scenarioArtifacts', 'DEALER_SCENARIO_COMMON_SCALAR_COUNT_V4'],
  ['scenarioArtifacts', 'DEALER_SCENARIO_COMMON_IDENTITY_COUNT_V4'],
  ['scenarioArtifacts', 'DEALER_SCENARIO_ITEM_SCALAR_STRIDE_V4'],
  ['scenarioArtifacts', 'DEALER_SCENARIO_ITEM_IDENTITY_STRIDE_V4'],
  ['scenarioArtifacts', 'DEALER_SCENARIO_CURRENT_SLOT_SCALAR_V4'],
  ['scenarioArtifacts', 'DEALER_SCENARIO_MAX_POSITION_COUNT_SCALAR_V4'],
  ['scenarioArtifacts', 'DEALER_SCENARIO_SCRATCH_PAGE_COUNT_SCALAR_V4'],
  ['scenarioArtifacts', 'DEALER_SCENARIO_CURRENT_TRADING_IDENTITY_V4'],
  ['scenarioArtifacts', 'DEALER_SCENARIO_OBLIGATION_IDENTITY_V4'],
  ['basis', 'BASIS_WIDTH_OFFSET_V3'],
]);

let output = '// @generated from canonical Rust Dealer V3 ABIs; do not edit.\n';
output += '// Regenerate with: npm run abi:dealer-v3\n\n';
for (const [source, name, outputName = name] of scalars) output += `export const ${outputName} = ${scalar(source, name)} as const;\n`;
for (const [source, name, outputName = name] of [
  ['config', 'DEALER_CONFIG_MAGIC_V4'],
  ['config', 'DEALER_CONFIG_SCHEMA_PREIMAGE_V4'],
  ['request', 'DEALER_EQUITY_REQUEST_MAGIC_V3'],
  ['lp', 'DEALER_LP_POSITION_MAGIC_V3'],
  ['lp', 'DEALER_LP_POSITION_PDA_DOMAIN_V3'],
  ['obligation', 'DEALER_OBLIGATION_MAGIC_V3'],
  ['obligation', 'DEALER_OBLIGATION_PDA_DOMAIN_V3'],
  ['delta', 'SIGNED_DELTA_PLAN_MAGIC_V3'],
  ['strategy', 'EXECUTION_STRATEGY_PROGRAM_MAGIC_V2'],
  ['accountProfile', 'MAGIC', 'ACCOUNT_PROFILE_MAGIC_V2'],
  ['dealer', 'DEALER_KIND_PREIMAGE_V2'],
  ['dealer', 'DEALER_ROOT_SCHEMA_PREIMAGE_V2'],
  ['release', 'DEALER_EQUITY_REQUEST_SCHEMA_PREIMAGE_V3'],
]) output += array(outputName, bytes(source, name));

const destination = fileURLToPath(outputUrl);
const check = process.argv.includes('--check');
if (check) {
  if (readFileSync(outputUrl, 'utf8') !== output) throw new Error('generated Dealer V3 ABI is stale; run npm run abi:dealer-v3');
} else {
  const temporary = `${destination}.tmp-${process.pid}`;
  try { writeFileSync(temporary, output); renameSync(temporary, destination); }
  catch (error) { try { unlinkSync(temporary); } catch {} throw error; }
}
