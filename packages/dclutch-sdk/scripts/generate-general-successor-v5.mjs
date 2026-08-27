import { readFileSync, renameSync, unlinkSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const root = new URL('../../../', import.meta.url);
const sources = Object.freeze({
  hot: readFileSync(new URL('crates/dclutch-capability-program-contract/src/hot_v3.rs', root), 'utf8'),
  request: readFileSync(new URL('crates/dclutch-general-codec/src/successor_request_v2.rs', root), 'utf8'),
  controller: readFileSync(new URL('crates/dclutch-general-codec/src/generated_general_controller.rs', root), 'utf8'),
  local: readFileSync(new URL('crates/dclutch-general-adapter-contract/src/local_state_v3.rs', root), 'utf8'),
  selection: readFileSync(new URL('crates/dclutch-general-adapter-contract/src/runtime_selection.rs', root), 'utf8'),
  runtime: readFileSync(new URL('crates/dclutch-general-adapter-contract/src/runtime_width.rs', root), 'utf8'),
  custody: readFileSync(new URL('crates/dclutch-custody-contract/src/generated.rs', root), 'utf8'),
  lib: readFileSync(new URL('crates/dclutch-general-adapter-contract/src/lib.rs', root), 'utf8'),
});
const outputUrl = new URL('../lib/generated/generalSuccessorV5.ts', import.meta.url);
const check = process.argv.includes('--check');

function scalar(source, name) {
  const match = sources[source].match(new RegExp(`(?:pub )?const ${name}: [^=]+ = ([0-9_]+);`));
  if (!match) throw new Error(`missing Rust scalar ${source}.${name}`);
  return Number(match[1].replaceAll('_', ''));
}

function bytes(source, name) {
  const match = sources[source].match(new RegExp(`(?:pub )?const ${name}: \\[u8; [^\\]]+\\] = \\*b"([^"]+)";`));
  if (!match) throw new Error(`missing Rust byte string ${source}.${name}`);
  return [...new TextEncoder().encode(match[1])];
}

function methodOffset(source, method) {
  const match = sources[source].match(new RegExp(`pub const fn ${method}\\(\\) -> u32 \\{\\s*([0-9_]+)\\s*\\}`));
  if (!match) throw new Error(`missing Rust typed offset ${source}.${method}`);
  return Number(match[1].replaceAll('_', ''));
}

function array(name, values) {
  return `export const ${name} = Uint8Array.from([${values.map((value) => `0x${value.toString(16).padStart(2, '0')}`).join(', ')}]);\n`;
}

const actionNames = [
  'ACTION_CONSIDER', 'ACTION_FREEZE', 'ACTION_INITIALIZE_SETTLEMENT', 'ACTION_COLLECT',
  'ACTION_MATERIALIZE', 'ACTION_DISTRIBUTE', 'ACTION_CLOSE',
];
const assertions = [
  ['request', 'const MAGIC: [u8; 8] = *b"DCGREQ02";'],
  ['request', 'const VERSION: u16 = 2;'],
  ['selection', 'const MAGIC: [u8; 8] = *b"DCGSEL02";'],
  ['selection', 'const VERSION: u16 = 2;'],
  ['runtime', 'const SETTLEMENT_CURSOR_MAGIC: [u8; 8] = *b"DCGSET02";'],
  ['runtime', 'pub const RUNTIME_WIDTH_VERSION_V2: u16 = 2;'],
  ['runtime', '4 => Ok(Self::Collecting)'],
  ['runtime', '5 => Ok(Self::Materializing)'],
  ['runtime', '6 => Ok(Self::Distributing)'],
  ['runtime', '7 => Ok(Self::ReadyToClose)'],
  ['runtime', '8 => Ok(Self::Terminal)'],
  ['request', 'expected_revision: read_u64(input, 16)?'],
  ['request', 'let raw_candidate = read_array32(input, 24)?'],
  ['request', 'page_index: read_u32(input, 56)?'],
  ['request', 'execution_index: byte(input, 60)?'],
  ['local', 'const KIND_SELECTION: u8 = 1;'],
  ['local', 'const KIND_SETTLEMENT: u8 = 2;'],
];
for (const [source, fragment] of assertions) {
  if (!sources[source].includes(fragment)) throw new Error(`canonical Rust semantics changed: ${source} lacks ${fragment}`);
}

let output = '// @generated from canonical Rust General V5 successor ABIs; do not edit.\n';
output += '// Regenerate with: node scripts/generate-general-successor-v5.mjs\n\n';
output += array('GENERAL_HOT_MAGIC_V3', bytes('hot', 'HOT_EXECUTION_MAGIC_V3'));
output += array('GENERAL_HOT_ACK_MAGIC_V3', bytes('hot', 'HOT_EXECUTION_ACK_MAGIC_V3'));
output += array('GENERAL_LOCAL_STATE_MAGIC_V3', bytes('local', 'GENERAL_LOCAL_STATE_MAGIC_V3'));
output += 'export const GENERAL_REQUEST_MAGIC_V2 = Uint8Array.from([0x44, 0x43, 0x47, 0x52, 0x45, 0x51, 0x30, 0x32]);\n';
output += 'export const GENERAL_SELECTION_MAGIC_V2 = Uint8Array.from([0x44, 0x43, 0x47, 0x53, 0x45, 0x4c, 0x30, 0x32]);\n';
output += 'export const GENERAL_SETTLEMENT_MAGIC_V2 = Uint8Array.from([0x44, 0x43, 0x47, 0x53, 0x45, 0x54, 0x30, 0x32]);\n';
for (const [name, value] of [
  ['GENERAL_HOT_VERSION_V3', scalar('hot', 'HOT_EXECUTION_VERSION_V3')],
  ['GENERAL_HOT_PROFILE_V3', scalar('hot', 'HOT_EXECUTION_PROFILE_V3')],
  ['GENERAL_HOT_ENVELOPE_BYTES_V3', scalar('hot', 'HOT_EXECUTION_ENVELOPE_BYTES_V3')],
  ['GENERAL_HOT_ACK_BYTES_V3', scalar('hot', 'HOT_EXECUTION_ACK_BYTES_V3')],
  ['GENERAL_HOT_FIXED_ACCOUNT_COUNT_V3', scalar('hot', 'HOT_FIXED_ACCOUNT_COUNT_V3')],
  ['GENERAL_REQUEST_BYTES_V2', scalar('request', 'CONTROLLER_REQUEST_BYTES_V2')],
  ['GENERAL_REQUEST_ACTION_OFFSET_V2', scalar('request', 'CONTROLLER_REQUEST_ACTION_OFFSET_V2')],
  ['GENERAL_REQUEST_MANIFEST_ORDER_OFFSET_V2', scalar('request', 'CONTROLLER_REQUEST_MANIFEST_ORDER_OFFSET_V2')],
  ['GENERAL_REQUEST_STATE_BUMP_OFFSET_V2', scalar('request', 'CONTROLLER_REQUEST_STATE_BUMP_OFFSET_V2')],
  ['GENERAL_REQUEST_TERMINAL_BUMP_OFFSET_V2', scalar('request', 'CONTROLLER_REQUEST_TERMINAL_BUMP_OFFSET_V2')],
  ['GENERAL_LOCAL_STATE_VERSION_V3', scalar('local', 'GENERAL_LOCAL_STATE_VERSION_V3')],
  ['GENERAL_LOCAL_STATE_HEADER_BYTES_V3', scalar('local', 'GENERAL_LOCAL_STATE_HEADER_BYTES_V3')],
  ['GENERAL_SELECTION_BYTES_V2', scalar('selection', 'RUNTIME_SELECTION_CURSOR_BYTES_V2')],
  ['GENERAL_SETTLEMENT_HEADER_BYTES_V2', scalar('runtime', 'SETTLEMENT_CURSOR_HEADER_BYTES_V2')],
  ['GENERAL_CUSTODY_RECEIPT_BYTES_V1', scalar('custody', 'CUSTODY_RECEIPT_BYTES_V1')],
  ['GENERAL_CANDIDATE_BYTES', scalar('controller', 'CANDIDATE_BYTES')],
  ['GENERAL_EXECUTION_BYTES', scalar('controller', 'EXECUTION_BYTES')],
  ['GENERAL_PAGE_BYTES', scalar('controller', 'PAGE_BYTES')],
  ['GENERAL_VERIFICATION_BYTES', scalar('lib', 'VERIFICATION_CURSOR_BYTES_V1')],
]) output += `export const ${name} = ${value} as const;\n`;
for (const name of [
  'ENVELOPE_REQUEST_BYTES_OFFSET', 'ENVELOPE_RELEASE_SET_OFFSET', 'ENVELOPE_MARKET_OFFSET',
  'ENVELOPE_GENERATION_OFFSET', 'ENVELOPE_ROOT_PRESTATE_DIGEST_OFFSET', 'ENVELOPE_RESERVED_OFFSET',
  'ACK_RELEASE_SET_OFFSET', 'ACK_MARKET_OFFSET', 'ACK_GENERATION_OFFSET', 'ACK_ROOT_OFFSET',
  'ACK_REQUEST_DIGEST_OFFSET', 'ACK_SELECTED_PROGRAM_OFFSET', 'ACK_ROOT_PRESTATE_DIGEST_OFFSET',
  'ACK_ROOT_POSTSTATE_DIGEST_OFFSET', 'ACK_EXECUTION_DIGEST_OFFSET',
]) output += `export const GENERAL_${name}_V3 = ${scalar('hot', name)} as const;\n`;
output += 'export const GENERAL_REQUEST_EXPECTED_REVISION_OFFSET_V2 = 16 as const;\n';
output += 'export const GENERAL_REQUEST_CANDIDATE_ID_OFFSET_V2 = 24 as const;\n';
output += 'export const GENERAL_REQUEST_PAGE_INDEX_OFFSET_V2 = 56 as const;\n';
output += 'export const GENERAL_REQUEST_EXECUTION_INDEX_OFFSET_V2 = 60 as const;\n';
output += 'export const GENERAL_LOCAL_STATE_SELECTION_KIND_V3 = 1 as const;\n';
output += 'export const GENERAL_LOCAL_STATE_SETTLEMENT_KIND_V3 = 2 as const;\n';
for (const [name, method] of [
  ['GENERAL_LOCAL_STATE_KIND_OFFSET_V3', 'kind'], ['GENERAL_LOCAL_STATE_BUMP_OFFSET_V3', 'bump'],
  ['GENERAL_LOCAL_STATE_RENT_PRINCIPAL_OFFSET_V3', 'rent_principal'],
  ['GENERAL_LOCAL_STATE_BENEFICIARY_OFFSET_V3', 'beneficiary'], ['GENERAL_LOCAL_STATE_BODY_OFFSET_V3', 'body'],
]) output += `export const ${name} = ${methodOffset('local', method)} as const;\n`;
for (const name of actionNames) output += `export const ${name}_V2 = ${scalar('controller', name)} as const;\n`;
for (const [name, method] of [
  ['GENERAL_SELECTION_PHASE_OFFSET_V2', 'phase'], ['GENERAL_SELECTION_OUTCOME_COUNT_OFFSET_V2', 'outcome_count'],
  ['GENERAL_SELECTION_REVISION_OFFSET_V2', 'revision'], ['GENERAL_SELECTION_SUBMITTED_COUNT_OFFSET_V2', 'submitted_count'],
  ['GENERAL_SELECTION_BEST_COORDINATE_OFFSET_V2', 'best_candidate_coordinate'],
  ['GENERAL_SELECTION_VERIFIED_REVISION_OFFSET_V2', 'best_verified_revision'],
  ['GENERAL_SELECTION_PRICE_SCALE_OFFSET_V2', 'price_scale'], ['GENERAL_SELECTION_PRODUCT_ID_OFFSET_V2', 'product_id'],
  ['GENERAL_SELECTION_BATCH_ID_OFFSET_V2', 'batch_id'], ['GENERAL_SELECTION_POLICY_ID_OFFSET_V2', 'policy_id'],
  ['GENERAL_SELECTION_BEST_CANDIDATE_OFFSET_V2', 'best_candidate_id'],
  ['GENERAL_SELECTION_VERIFIED_DIGEST_OFFSET_V2', 'best_verified_digest'],
  ['GENERAL_SELECTION_FILLED_LOTS_OFFSET_V2', 'best_filled_lots'],
  ['GENERAL_SELECTION_QUOTE_SURPLUS_OFFSET_V2', 'best_quote_surplus'],
]) output += `export const ${name} = ${methodOffset('selection', method)} as const;\n`;
for (const [name, method] of [
  ['GENERAL_SETTLEMENT_PHASE_OFFSET_V2', 'phase'], ['GENERAL_SETTLEMENT_OUTCOME_COUNT_OFFSET_V2', 'outcome_count'],
  ['GENERAL_SETTLEMENT_ORDER_COUNT_OFFSET_V2', 'order_count'], ['GENERAL_SETTLEMENT_NEXT_ORDER_OFFSET_V2', 'next_order'],
  ['GENERAL_SETTLEMENT_REVISION_OFFSET_V2', 'revision'], ['GENERAL_SETTLEMENT_CANDIDATE_ID_OFFSET_V2', 'candidate_id'],
  ['GENERAL_SETTLEMENT_QUOTE_INVENTORY_OFFSET_V2', 'quote_inventory'],
  ['GENERAL_SETTLEMENT_COMPLETE_SET_OFFSET_V2', 'complete_set_quantity'],
  ['GENERAL_SETTLEMENT_TERMINAL_OFFSET_V2', 'terminal_coordinate'],
  ['GENERAL_SETTLEMENT_INVENTORY_OFFSET_V2', 'inventory_base'],
]) output += `export const ${name} = ${methodOffset('runtime', method)} as const;\n`;
output += 'export const GENERAL_SETTLEMENT_INVENTORY_STRIDE_V2 = 8 as const;\n';
output += 'export const GENERAL_PHASE_COLLECTING_V2 = 4 as const;\nexport const GENERAL_PHASE_MATERIALIZING_V2 = 5 as const;\n';
output += 'export const GENERAL_PHASE_DISTRIBUTING_V2 = 6 as const;\nexport const GENERAL_PHASE_READY_TO_CLOSE_V2 = 7 as const;\n';
output += 'export const GENERAL_PHASE_TERMINAL_V2 = 8 as const;\n';

const target = fileURLToPath(outputUrl);
if (check) {
  if (readFileSync(outputUrl, 'utf8') !== output) throw new Error(`${target} is stale; regenerate the General V5 ABI`);
} else {
  const temporary = `${target}.tmp-${process.pid}`;
  try {
    writeFileSync(temporary, output);
    renameSync(temporary, target);
  } catch (error) {
    try { unlinkSync(temporary); } catch {}
    throw error;
  }
}
