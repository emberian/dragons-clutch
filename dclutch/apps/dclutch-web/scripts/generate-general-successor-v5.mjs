import { readFileSync, renameSync, unlinkSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const root = new URL('../../../', import.meta.url);
const sources = Object.freeze({
  hot: readFileSync(new URL('crates/dclutch-market/src/capability_program/hot_v3.rs', root), 'utf8'),
  request: readFileSync(new URL('crates/dclutch-trading/src/general_codec/successor_request_v2.rs', root), 'utf8'),
  requestV3: readFileSync(new URL('crates/dclutch-trading/src/general_codec/successor_request_v3.rs', root), 'utf8'),
  requestV3Generated: readFileSync(new URL('crates/dclutch-trading/src/general_codec/generated_general_controller_request_v3.rs', root), 'utf8'),
  controller: readFileSync(new URL('crates/dclutch-trading/src/general_codec/generated_general_controller.rs', root), 'utf8'),
  local: readFileSync(new URL('crates/dclutch-trading/src/general/local_state_v3.rs', root), 'utf8'),
  collection: readFileSync(new URL('crates/dclutch-trading/src/general/collection_v1.rs', root), 'utf8'),
  candidate: readFileSync(new URL('crates/dclutch-trading/src/general/candidate_v1.rs', root), 'utf8'),
  selection: readFileSync(new URL('crates/dclutch-trading/src/general/runtime_selection.rs', root), 'utf8'),
  // The runtime wire's magics, versions and coordinates have ONE author:
  // `DClutchSemantics.GeneralRuntimeWireV2`, printed here. `runtime_selection.rs`
  // and `runtime_width.rs` used to spell them and now keep only aliases whose
  // right-hand sides are this file's names, so the readers below follow the
  // alias rather than restating a value the emission owns.
  runtimeWire: readFileSync(new URL('crates/dclutch-trading/src/general/generated_runtime_wire_v2.rs', root), 'utf8'),
  verifier: readFileSync(new URL('crates/dclutch-trading/src/general/runtime_verify.rs', root), 'utf8'),
  runtime: readFileSync(new URL('crates/dclutch-trading/src/general/runtime_width.rs', root), 'utf8'),
  state: readFileSync(new URL('crates/dclutch-trading/src/general/state_artifacts_v3.rs', root), 'utf8'),
  custody: readFileSync(new URL('crates/dclutch-custody/src/generated.rs', root), 'utf8'),
  lib: readFileSync(new URL('crates/dclutch-trading/src/general/mod.rs', root), 'utf8'),
  operator: readFileSync(new URL('crates/dclutch-operator/src/general_hot_v3.rs', root), 'utf8'),
  producer: readFileSync(new URL('crates/dclutch-general-successor-operator/src/lib.rs', root), 'utf8'),
});
const outputUrl = new URL('../lib/generated/generalSuccessorV5.ts', import.meta.url);
const check = process.argv.includes('--check');

function scalar(source, name) {
  const match = sources[source].match(new RegExp(`(?:pub(?:\\(crate\\))? )?const ${name}: [^=]+ =\\s*(wire::)?([A-Z0-9_]+)(?: \\+ ([0-9_]+))?;`));
  if (!match) throw new Error(`missing Rust scalar ${source}.${name}`);
  const base = match[1] !== undefined
    ? scalar('runtimeWire', match[2])
    : (/^[0-9_]+$/.test(match[2]) ? Number(match[2].replaceAll('_', '')) : scalar(source, match[2]));
  return base + Number((match[3] ?? '0').replaceAll('_', ''));
}

/** One `[u8; N]` the wire emission prints as hexadecimal bytes. */
function emittedBytes(name) {
  const match = sources.runtimeWire.match(new RegExp(`pub const ${name}: \\[u8; [0-9]+\\] = \\[([^\\]]*)\\];`));
  if (!match) throw new Error(`missing emitted byte array runtimeWire.${name}`);
  return match[1].split(',').map((byte) => byte.trim()).filter(Boolean).map(Number);
}

function bytes(source, name) {
  const alias = sources[source].match(new RegExp(`(?:pub )?const ${name}: \\[u8; [^\\]]+\\] = wire::([A-Z0-9_]+);`));
  if (alias) return emittedBytes(alias[1]);
  const match = sources[source].match(new RegExp(`(?:pub )?const ${name}: \\[u8; [^\\]]+\\] = \\*b"([^"]+)";`));
  if (!match) throw new Error(`missing Rust byte string ${source}.${name}`);
  return [...new TextEncoder().encode(match[1])];
}

/**
 * One typed offset projection, SCOPED TO ITS OWN `impl`.
 *
 * `runtime_width.rs` holds two layouts that share six method names --
 * `SettlementCursorLayoutV2` and `VerifiedCandidateLayoutV2` both project
 * `magic`, `version`, `phase`, `outcome_count`, `revision` and `candidate_id`
 * -- and an unscoped search took whichever came first in the file, which is the
 * settlement one, for BOTH tables below. It was right only because the two
 * records happen to share a prologue. Naming the owner makes that a fact the
 * reader states rather than one it depends on.
 */
function methodOffset(source, owner, method) {
  const body = sources[source].match(new RegExp(`impl ${owner} \\{([\\s\\S]*?)\\n\\}`))?.[1];
  if (body === undefined) throw new Error(`missing Rust layout ${source}.${owner}`);
  const match = body.match(new RegExp(`pub const fn ${method}\\(\\) -> u32 \\{\\s*(wire::)?([A-Z0-9_]+|[0-9_]+)(?: as u32)?\\s*\\}`));
  if (!match) throw new Error(`missing Rust typed offset ${source}.${owner}.${method}`);
  if (match[1] !== undefined) return { offset: scalar('runtimeWire', match[2]), emitted: match[2] };
  return { offset: Number(match[2].replaceAll('_', '')), emitted: null };
}

/**
 * A whole layout's offsets, in field order, with the two checks a
 * constant-at-a-time read cannot make.
 *
 * A projection that FORWARDS must forward to a constant of its own record:
 * crossing records is the mis-wiring the conversion to emitted names makes
 * possible, and it is invisible in a value comparison because the two records
 * share a prologue. And the offsets must strictly increase in field order,
 * which is what catches a forward wired to the wrong field of the RIGHT
 * record. Neither check restates a coordinate here; both would have to be
 * defeated deliberately.
 */
function layoutOffsets(source, owner, prefix, entries) {
  let previous = -1;
  return entries.map(([exported, method]) => {
    const { offset, emitted } = methodOffset(source, owner, method);
    if (emitted !== null && !emitted.startsWith(prefix)) {
      throw new Error(`${source}.${owner}.${method} forwards to ${emitted}, which is not a ${prefix} coordinate`);
    }
    if (offset <= previous) {
      throw new Error(`${source}.${owner}.${method} is at ${offset}, not after the field before it at ${previous}`);
    }
    previous = offset;
    return `export const ${exported} = ${offset} as const;\n`;
  }).join('');
}

function associatedOffset(source, owner, name) {
  const body = sources[source].match(new RegExp(`impl ${owner} \\{([\\s\\S]*?)\\n\\}`))?.[1];
  const match = body?.match(new RegExp(`pub const ${name}: usize = ([A-Z0-9_]+)(?: \\+ ([0-9_]+))?;`));
  if (!match) throw new Error(`missing Rust associated offset ${source}.${owner}.${name}`);
  const base = /^[0-9_]+$/.test(match[1])
    ? Number(match[1].replaceAll('_', ''))
    : scalar(source, match[1]);
  return base + Number((match[2] ?? '0').replaceAll('_', ''));
}

function associatedScalar(source, owner, name) {
  const body = sources[source].match(new RegExp(`impl ${owner} \\{([\\s\\S]*?)\\n\\}`))?.[1];
  const match = body?.match(new RegExp(`pub const ${name}: [^=]+ = ([0-9_]+);`));
  if (!match) throw new Error(`missing Rust associated scalar ${source}.${owner}.${name}`);
  return Number(match[1].replaceAll('_', ''));
}

function associatedBytes(source, owner, name) {
  const body = sources[source].match(new RegExp(`impl ${owner} \\{([\\s\\S]*?)\\n\\}`))?.[1];
  const match = body?.match(new RegExp(`pub const ${name}: \\[u8; [^\\]]+\\] = \\*b"([^"]+)";`));
  if (!match) throw new Error(`missing Rust associated bytes ${source}.${owner}.${name}`);
  return [...new TextEncoder().encode(match[1])];
}

function enumTag(source, owner, variant) {
  const body = sources[source].match(new RegExp(`pub enum ${owner} \\{([\\s\\S]*?)\\n\\}`))?.[1];
  const match = body?.match(new RegExp(`\\b${variant}\\s*=\\s*([0-9_]+),`));
  if (!match) throw new Error(`missing Rust enum tag ${source}.${owner}.${variant}`);
  return Number(match[1].replaceAll('_', ''));
}

/**
 * The `u16` one `const fn` over `Action` returns for one variant.
 *
 * Two things this now tolerates, because the crate does both and neither
 * changes the fact being read. A projection that only FORWARDS is followed, so
 * this reader keeps naming the concept it wants -- where an action's readonly
 * evidence starts -- rather than whichever function currently holds the
 * number. And arms are GROUPED with `|`, so the patterns are split instead of
 * matched whole: a regrouping that leaves a variant's value alone must not
 * make this reader lose it, and one that changes the value must still red.
 */
function actionMatchScalar(source, functionName, variant) {
  const body = sources[source].match(new RegExp(`(?:pub )?const fn ${functionName}\\(action: Action\\) -> u16 \\{([\\s\\S]*?)\\n\\}`))?.[1];
  if (body === undefined) throw new Error(`missing Rust action function ${source}.${functionName}`);
  const text = body.replaceAll(/\/\/[^\n]*/g, '');
  const forward = text.match(/^\s*([a-z_0-9]+)\(action\)\s*$/);
  if (forward) return actionMatchScalar(source, forward[1], variant);
  for (const arm of text.matchAll(/([^=;{}]*?)=>\s*([0-9_]+),/g)) {
    if (new RegExp(`Action::${variant}\\b`).test(arm[1])) return Number(arm[2].replaceAll('_', ''));
  }
  throw new Error(`missing Rust action scalar ${source}.${functionName}.${variant}`);
}

function array(name, values) {
  return `export const ${name} = Uint8Array.from([${values.map((value) => `0x${value.toString(16).padStart(2, '0')}`).join(', ')}]);\n`;
}

const actionNames = [
  'ACTION_CONSIDER', 'ACTION_FREEZE', 'ACTION_INITIALIZE_SETTLEMENT', 'ACTION_COLLECT',
  'ACTION_MATERIALIZE', 'ACTION_DISTRIBUTE', 'ACTION_CLOSE',
];
const actionNamesV3 = Object.freeze([
  ['ACTION_CONSIDER', 'Consider'], ['ACTION_FREEZE', 'Freeze'], ['ACTION_INITIALIZE_SETTLEMENT', 'InitializeSettlement'],
  ['ACTION_COLLECT', 'Collect'], ['ACTION_MATERIALIZE', 'Materialize'], ['ACTION_DISTRIBUTE', 'Distribute'], ['ACTION_CLOSE', 'Close'],
  ['ACTION_OPEN_BATCH', 'OpenBatch'], ['ACTION_PLACE_ORDER', 'PlaceOrder'], ['ACTION_CANCEL_ORDER', 'CancelOrder'], ['ACTION_CLOSE_BATCH', 'CloseBatch'],
  ['ACTION_SUBMIT_CANDIDATE', 'SubmitCandidate'], ['ACTION_VERIFY_CANDIDATE_ROW', 'VerifyCandidateRow'], ['ACTION_RELEASE_ORDER', 'ReleaseOrder'],
  ['ACTION_CLOSE_CANDIDATE', 'CloseCandidate'],
]);
const assertions = [
  ['request', 'const MAGIC: [u8; 8] = *b"DCGREQ02";'],
  ['request', 'const VERSION: u16 = 2;'],
  ['requestV3', 'Exact 64-byte General successor request V3.'],
  ['requestV3Generated', 'REQUEST_V3_MAGIC: [u8; 8]'],
  ['requestV3Generated', 'REQUEST_V3_ABI_VERSION: u16 = 3;'],
  // Each of these pairs is one fact stated twice on purpose: the emission's
  // line pins the VALUE, and the crate's line pins that the crate still reads
  // it. Either alone can go stale without the other noticing.
  ['runtimeWire', 'pub const RUNTIME_SELECTION_MAGIC_V2: [u8; 8] = [0x44, 0x43, 0x47, 0x53, 0x45, 0x4c, 0x30, 0x32];'],
  ['selection', 'const MAGIC: [u8; 8] = wire::RUNTIME_SELECTION_MAGIC_V2;'],
  ['runtimeWire', 'pub const RUNTIME_WIRE_VERSION_V2: u16 = 2;'],
  ['selection', 'const VERSION: u16 = wire::RUNTIME_WIRE_VERSION_V2;'],
  ['runtimeWire', 'pub const SETTLEMENT_CURSOR_MAGIC_V2: [u8; 8] = [0x44, 0x43, 0x47, 0x53, 0x45, 0x54, 0x30, 0x32];'],
  ['runtime', 'const SETTLEMENT_CURSOR_MAGIC: [u8; 8] = wire::SETTLEMENT_CURSOR_MAGIC_V2;'],
  ['runtime', 'pub const RUNTIME_WIDTH_VERSION_V2: u16 = wire::RUNTIME_WIRE_VERSION_V2;'],
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
  ['collection', 'const BATCH_MAGIC: [u8; 8] = *b"DCGBAT01";'],
  ['collection', 'const BATCH_OCCURRENCE_TERMS_MAGIC: [u8; 8] = *b"DCGBOC01";'],
  ['collection', 'const ORDER_MAGIC: [u8; 8] = *b"DCGORD01";'],
  ['candidate', 'pub const MAGIC: [u8; 8] = *b"DCGSUB01";'],
  ['verifier', 'const VERIFIER_MAGIC: [u8; 8] = *b"DCGVFY02";'],
  ['runtimeWire', 'pub const VERIFIED_CANDIDATE_MAGIC_V2: [u8; 8] = [0x44, 0x43, 0x47, 0x56, 0x45, 0x52, 0x30, 0x32];'],
  ['runtime', 'const VERIFIED_CANDIDATE_MAGIC: [u8; 8] = wire::VERIFIED_CANDIDATE_MAGIC_V2;'],
  ['hot', 'pub struct HotBumpHintsV1 {'],
  ['operator', 'pub const GENERAL_HOT_HEAP_FRAME_BYTES_V3: u32 = DIRECT_HOT_HEAP_FRAME_BYTES_V1;'],
];
for (const [source, fragment] of assertions) {
  if (!sources[source].includes(fragment)) throw new Error(`canonical Rust semantics changed: ${source} lacks ${fragment}`);
}
if (scalar('hot', 'HOT_BUMP_HINTS_OFFSET_V1') + scalar('hot', 'HOT_BUMP_HINT_COUNT_V1') !== scalar('hot', 'HOT_EXECUTION_ENVELOPE_BYTES_V3')) {
  throw new Error('canonical Rust semantics changed: the bump hint block is no longer the hot envelope tail');
}

let output = '// @generated from canonical Rust General V5 successor ABIs; do not edit.\n';
output += '// Regenerate with: node scripts/generate-general-successor-v5.mjs\n\n';
output += array('GENERAL_HOT_MAGIC_V3', bytes('hot', 'HOT_EXECUTION_MAGIC_V3'));
output += array('GENERAL_HOT_ACK_MAGIC_V3', bytes('hot', 'HOT_EXECUTION_ACK_MAGIC_V3'));
output += array('GENERAL_LOCAL_STATE_MAGIC_V3', bytes('local', 'GENERAL_LOCAL_STATE_MAGIC_V3'));
output += 'export const GENERAL_REQUEST_MAGIC_V2 = Uint8Array.from([0x44, 0x43, 0x47, 0x52, 0x45, 0x51, 0x30, 0x32]);\n';
output += 'export const GENERAL_REQUEST_MAGIC_V3 = Uint8Array.from([0x44, 0x43, 0x47, 0x52, 0x45, 0x51, 0x30, 0x33]);\n';
output += 'export const GENERAL_SELECTION_MAGIC_V2 = Uint8Array.from([0x44, 0x43, 0x47, 0x53, 0x45, 0x4c, 0x30, 0x32]);\n';
output += 'export const GENERAL_SETTLEMENT_MAGIC_V2 = Uint8Array.from([0x44, 0x43, 0x47, 0x53, 0x45, 0x54, 0x30, 0x32]);\n';
output += 'export const GENERAL_BATCH_MAGIC_V1 = Uint8Array.from([0x44, 0x43, 0x47, 0x42, 0x41, 0x54, 0x30, 0x31]);\n';
output += array('GENERAL_BATCH_OCCURRENCE_TERMS_MAGIC_V1', bytes('collection', 'BATCH_OCCURRENCE_TERMS_MAGIC'));
output += 'export const GENERAL_ORDER_MAGIC_V1 = Uint8Array.from([0x44, 0x43, 0x47, 0x4f, 0x52, 0x44, 0x30, 0x31]);\n';
output += array('GENERAL_SUBMISSION_MAGIC_V1', associatedBytes('candidate', 'GeneralCandidateLayoutV1', 'MAGIC'));
output += array('GENERAL_VERIFIER_MAGIC_V2', bytes('verifier', 'VERIFIER_MAGIC'));
output += array('GENERAL_VERIFIED_CANDIDATE_MAGIC_V2', bytes('runtime', 'VERIFIED_CANDIDATE_MAGIC'));
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
  ['GENERAL_REQUEST_BYTES_V3', scalar('requestV3Generated', 'REQUEST_V3_BYTES')],
  ['GENERAL_REQUEST_ACTION_OFFSET_V3', scalar('requestV3Generated', 'REQUEST_V3_ACTION_OFFSET')],
  ['GENERAL_REQUEST_MANIFEST_ORDER_OFFSET_V3', scalar('requestV3Generated', 'REQUEST_V3_MANIFEST_ORDER_OFFSET')],
  ['GENERAL_REQUEST_PRIMARY_BUMP_OFFSET_V3', scalar('requestV3Generated', 'REQUEST_V3_PRIMARY_BUMP_OFFSET')],
  ['GENERAL_REQUEST_SECONDARY_BUMP_OFFSET_V3', scalar('requestV3Generated', 'REQUEST_V3_SECONDARY_BUMP_OFFSET')],
  ['GENERAL_REQUEST_RESULT_BUMP_OFFSET_V3', scalar('requestV3Generated', 'REQUEST_V3_RESULT_BUMP_OFFSET')],
  ['GENERAL_LOCAL_STATE_VERSION_V3', scalar('local', 'GENERAL_LOCAL_STATE_VERSION_V3')],
  ['GENERAL_LOCAL_STATE_HEADER_BYTES_V3', scalar('local', 'GENERAL_LOCAL_STATE_HEADER_BYTES_V3')],
  ['GENERAL_SELECTION_BYTES_V2', scalar('selection', 'RUNTIME_SELECTION_CURSOR_BYTES_V2')],
  ['GENERAL_SELECTION_VERSION_V2', scalar('selection', 'VERSION')],
  ['GENERAL_SETTLEMENT_HEADER_BYTES_V2', scalar('runtime', 'SETTLEMENT_CURSOR_HEADER_BYTES_V2')],
  ['GENERAL_SETTLEMENT_VERSION_V2', scalar('runtime', 'RUNTIME_WIDTH_VERSION_V2')],
  ['GENERAL_BATCH_BYTES_V1', scalar('collection', 'GENERAL_BATCH_BYTES_V1')],
  ['GENERAL_BATCH_VERSION_V1', scalar('collection', 'VERSION')],
  ['GENERAL_BATCH_OCCURRENCE_TERMS_BYTES_V1', scalar('collection', 'GENERAL_BATCH_OCCURRENCE_TERMS_BYTES_V1')],
  ['GENERAL_BATCH_OCCURRENCE_TERMS_VERSION_V1', scalar('collection', 'VERSION')],
  ['GENERAL_ORDER_HEADER_BYTES_V1', scalar('collection', 'GENERAL_ORDER_HEADER_BYTES_V1')],
  ['GENERAL_ORDER_VERSION_V1', scalar('collection', 'VERSION')],
  ['GENERAL_ORDER_STATE_BYTES_V1', scalar('collection', 'GENERAL_ORDER_STATE_BYTES_V1')],
  ['GENERAL_ORDER_ROW_STRIDE_V1', scalar('collection', 'GENERAL_ORDER_ROW_STRIDE_V1')],
  ['GENERAL_SUBMISSION_BYTES_V1', scalar('candidate', 'GENERAL_CANDIDATE_BYTES_V1')],
  ['GENERAL_VERIFIER_HEADER_BYTES_V2', scalar('verifier', 'RUNTIME_VERIFIER_HEADER_BYTES_V2')],
  ['GENERAL_VERIFIER_VERSION_V2', scalar('verifier', 'VERSION')],
  ['GENERAL_VERIFIED_CANDIDATE_HEADER_BYTES_V2', scalar('runtime', 'VERIFIED_CANDIDATE_HEADER_BYTES_V2')],
  ['GENERAL_VERIFIED_CANDIDATE_VERSION_V2', scalar('runtime', 'RUNTIME_WIDTH_VERSION_V2')],
  ['GENERAL_CUSTODY_RECEIPT_BYTES_V1', scalar('custody', 'CUSTODY_RECEIPT_BYTES_V1')],
  ['GENERAL_HOT_HEAP_FRAME_BYTES_V3', scalar('hot', 'DIRECT_HOT_HEAP_FRAME_BYTES_V1')],
  ['GENERAL_CANDIDATE_BYTES', scalar('controller', 'CANDIDATE_BYTES')],
  ['GENERAL_EXECUTION_BYTES', scalar('controller', 'EXECUTION_BYTES')],
  ['GENERAL_PAGE_BYTES', scalar('controller', 'PAGE_BYTES')],
  ['GENERAL_VERIFICATION_BYTES', scalar('lib', 'VERIFICATION_CURSOR_BYTES_V1')],
  ['GENERAL_SUCCESSOR_PLAN_MAX_BYTES_V5', scalar('producer', 'GENERAL_SUCCESSOR_PLAN_MAX_BYTES_V5')],
  ['GENERAL_PRIMARY_STATE_ACCOUNT_V3', scalar('state', 'GENERAL_PRIMARY_STATE_ACCOUNT_V3')],
  ['GENERAL_TERMINAL_STATE_ACCOUNT_V3', scalar('state', 'GENERAL_TERMINAL_STATE_ACCOUNT_V3')],
  ['GENERAL_VERIFY_VERIFIER_STATE_ACCOUNT_V3', scalar('state', 'GENERAL_VERIFY_VERIFIER_STATE_ACCOUNT_V3')],
  ['GENERAL_VERIFY_RESULT_STATE_ACCOUNT_V3', scalar('state', 'GENERAL_VERIFY_RESULT_STATE_ACCOUNT_V3')],
  ['GENERAL_CLOSE_CANDIDATE_BATCH_ACCOUNT_V3', actionMatchScalar('state', 'general_readonly_evidence_start_v3', 'CloseCandidate')],
  ['GENERAL_CLOSE_CANDIDATE_CHILD_START_V3', actionMatchScalar('state', 'general_readonly_evidence_start_v3', 'CloseCandidate') + 1],
]) output += `export const ${name} = ${value} as const;\n`;
output += `export const GENERAL_ORDER_STATE_OFFSET_V1 = ${scalar('collection', 'GENERAL_ORDER_HEADER_BYTES_V1')} as const;\n`;
output += `export const GENERAL_ORDER_ROW_BASE_V1 = ${scalar('collection', 'GENERAL_ORDER_HEADER_BYTES_V1') + scalar('collection', 'GENERAL_ORDER_STATE_BYTES_V1')} as const;\n`;
for (const name of [
  'ENVELOPE_REQUEST_BYTES_OFFSET', 'ENVELOPE_RELEASE_SET_OFFSET', 'ENVELOPE_MARKET_OFFSET',
  'ENVELOPE_GENERATION_OFFSET', 'ENVELOPE_ROOT_PRESTATE_DIGEST_OFFSET',
]) output += `export const GENERAL_${name}_V3 = ${scalar('hot', name)} as const;\n`;
// The last eight envelope bytes were `ENVELOPE_RESERVED_OFFSET` until d0306a64
// gave them to `HotBumpHintsV1`. The block is family-neutral, so the General
// mirror emits its offset and slot count under the names the Rust now uses.
output += `export const GENERAL_ENVELOPE_BUMP_HINTS_OFFSET_V3 = ${scalar('hot', 'HOT_BUMP_HINTS_OFFSET_V1')} as const;\n`;
output += `export const GENERAL_ENVELOPE_BUMP_HINT_COUNT_V3 = ${scalar('hot', 'HOT_BUMP_HINT_COUNT_V1')} as const;\n`;
for (const name of [
  'ACK_RELEASE_SET_OFFSET', 'ACK_MARKET_OFFSET', 'ACK_GENERATION_OFFSET', 'ACK_ROOT_OFFSET',
  'ACK_REQUEST_DIGEST_OFFSET', 'ACK_SELECTED_PROGRAM_OFFSET', 'ACK_ROOT_PRESTATE_DIGEST_OFFSET',
  'ACK_ROOT_POSTSTATE_DIGEST_OFFSET', 'ACK_EXECUTION_DIGEST_OFFSET',
]) output += `export const GENERAL_${name}_V3 = ${scalar('hot', name)} as const;\n`;
output += 'export const GENERAL_REQUEST_EXPECTED_REVISION_OFFSET_V2 = 16 as const;\n';
output += 'export const GENERAL_REQUEST_CANDIDATE_ID_OFFSET_V2 = 24 as const;\n';
output += 'export const GENERAL_REQUEST_PAGE_INDEX_OFFSET_V2 = 56 as const;\n';
output += 'export const GENERAL_REQUEST_EXECUTION_INDEX_OFFSET_V2 = 60 as const;\n';
output += `export const GENERAL_REQUEST_EXPECTED_REVISION_OFFSET_V3 = ${scalar('requestV3Generated', 'REQUEST_V3_EXPECTED_REVISION_OFFSET')} as const;\n`;
output += `export const GENERAL_REQUEST_SUBJECT_ID_OFFSET_V3 = ${scalar('requestV3Generated', 'REQUEST_V3_SUBJECT_ID_OFFSET')} as const;\n`;
output += `export const GENERAL_REQUEST_PAGE_INDEX_OFFSET_V3 = ${scalar('requestV3Generated', 'REQUEST_V3_PAGE_INDEX_OFFSET')} as const;\n`;
output += `export const GENERAL_REQUEST_EXECUTION_INDEX_OFFSET_V3 = ${scalar('requestV3Generated', 'REQUEST_V3_EXECUTION_INDEX_OFFSET')} as const;\n`;
output += 'export const GENERAL_LOCAL_STATE_SELECTION_KIND_V3 = 1 as const;\n';
output += 'export const GENERAL_LOCAL_STATE_SETTLEMENT_KIND_V3 = 2 as const;\n';
output += `export const GENERAL_LOCAL_STATE_BATCH_KIND_V3 = ${scalar('local', 'KIND_BATCH')} as const;\n`;
output += `export const GENERAL_LOCAL_STATE_ORDER_KIND_V3 = ${scalar('local', 'KIND_ORDER')} as const;\n`;
output += `export const GENERAL_LOCAL_STATE_CANDIDATE_KIND_V3 = ${scalar('local', 'KIND_CANDIDATE')} as const;\n`;
output += `export const GENERAL_LOCAL_STATE_VERIFIER_KIND_V3 = ${scalar('local', 'KIND_VERIFIER')} as const;\n`;
output += layoutOffsets('local', 'GeneralLocalStateLayoutV3', 'GENERAL_LOCAL_STATE_', [
  ['GENERAL_LOCAL_STATE_MAGIC_OFFSET_V3', 'magic'], ['GENERAL_LOCAL_STATE_VERSION_OFFSET_V3', 'version'],
  ['GENERAL_LOCAL_STATE_KIND_OFFSET_V3', 'kind'], ['GENERAL_LOCAL_STATE_BUMP_OFFSET_V3', 'bump'],
  ['GENERAL_LOCAL_STATE_RENT_PRINCIPAL_OFFSET_V3', 'rent_principal'],
  ['GENERAL_LOCAL_STATE_BENEFICIARY_OFFSET_V3', 'beneficiary'], ['GENERAL_LOCAL_STATE_BODY_OFFSET_V3', 'body'],
]);
for (const name of actionNames) output += `export const ${name}_V2 = ${scalar('controller', name)} as const;\n`;
for (const [name, variant] of actionNamesV3) output += `export const ${name}_V3 = ${enumTag('requestV3', 'ControllerActionV3', variant)} as const;\n`;
for (const name of [
  'MAGIC', 'VERSION', 'PHASE', 'OUTCOME_COUNT', 'SEQUENCE', 'GENERATION', 'MARKET', 'PRODUCT_ID', 'CONFIG_ID', 'PRICE_SCALE',
  'COLLECTION_CLOSE_SLOT', 'MAX_ORDERS', 'SETTLEMENT_CLOSE_SLOT', 'STATUS', 'ORDER_COUNT',
  'OPENED_ROOT_REVISION', 'CLOSED_ROOT_REVISION', 'COMMITTED_QUOTE_RESERVE', 'CANCELLED_COUNT',
]) output += `export const GENERAL_BATCH_${name}_OFFSET_V1 = ${associatedOffset('collection', 'GeneralBatchLayoutV1', name)} as const;\n`;
for (const name of [
  'MAGIC', 'VERSION', 'PHASE', 'RESERVED_A', 'OUTCOME_COUNT', 'SEQUENCE', 'GENERATION',
  'MARKET', 'PRODUCT_ID', 'CONFIG_ID', 'PRICE_SCALE', 'MAX_ORDERS', 'RESERVED_B',
]) output += `export const GENERAL_BATCH_OCCURRENCE_TERMS_${name}_OFFSET_V1 = ${associatedOffset('collection', 'GeneralBatchOccurrenceTermsLayoutV1', name)} as const;\n`;
for (const name of [
  'MAGIC', 'VERSION', 'PHASE', 'OUTCOME_COUNT', 'NONCE', 'OWNER_ID', 'MARKET', 'BATCH_ID', 'GENERATION', 'MAX_LOTS',
  'MAX_QUOTE_DEBIT_PER_LOT', 'VALID_UNTIL_SLOT', 'STATE_PHASE', 'STATE_ADMITTED_SLOT', 'STATE_RELEASED_SLOT',
]) output += `export const GENERAL_ORDER_${name}_OFFSET_V1 = ${associatedOffset('collection', 'GeneralOrderLayoutV1', name)} as const;\n`;
output += `export const GENERAL_BATCH_PHASE_V1 = ${scalar('collection', 'BATCH_PHASE')} as const;\n`;
output += `export const GENERAL_BATCH_STATUS_COLLECTING_V1 = ${scalar('collection', 'STATUS_COLLECTING')} as const;\n`;
output += `export const GENERAL_BATCH_STATUS_CLOSED_V1 = ${scalar('collection', 'STATUS_CLOSED')} as const;\n`;
output += `export const GENERAL_ORDER_PHASE_V1 = ${scalar('collection', 'ORDER_PHASE')} as const;\n`;
output += `export const GENERAL_ORDER_STATE_PLACED_V1 = ${scalar('collection', 'ORDER_PHASE_PLACED')} as const;\n`;
output += `export const GENERAL_ORDER_STATE_CANCELLED_V1 = ${scalar('collection', 'ORDER_PHASE_CANCELLED')} as const;\n`;
output += `export const GENERAL_ORDER_STATE_RELEASED_V1 = ${scalar('collection', 'ORDER_PHASE_RELEASED')} as const;\n`;
output += `export const GENERAL_SUBMISSION_VERSION_V1 = ${associatedScalar('candidate', 'GeneralCandidateLayoutV1', 'VERSION')} as const;\n`;
output += `export const GENERAL_SUBMISSION_PHASE_V1 = ${associatedScalar('candidate', 'GeneralCandidateLayoutV1', 'PHASE')} as const;\n`;
output += `export const GENERAL_SUBMISSION_STATUS_SUBMITTED_V1 = ${scalar('candidate', 'STATUS_SUBMITTED')} as const;\n`;
output += `export const GENERAL_SUBMISSION_STATUS_VERIFIED_V1 = ${scalar('candidate', 'STATUS_VERIFIED')} as const;\n`;
output += `export const GENERAL_SUBMISSION_STATUS_CONSIDERED_V1 = ${scalar('candidate', 'STATUS_CONSIDERED')} as const;\n`;
for (const name of [
  'MAGIC', 'VERSION', 'PHASE', 'HEADER_RESERVED', 'OUTCOME_COUNT', 'PAGE_COUNT', 'STATUS',
  'STATUS_RESERVED', 'PAGE_REVISION', 'CANDIDATE_ID', 'BATCH_ID', 'SOLVER_ID',
  'VERIFIED_DIGEST', 'SUBMITTED_SLOT', 'VERIFIED_REVISION', 'ROW_COUNT', 'ROW_RESERVED',
  'REWARD_RATE', 'VERIFICATION_REMAINING', 'CLEANUP_REMAINING', 'TAIL_RESERVED',
]) output += `export const GENERAL_SUBMISSION_${name}_OFFSET_V1 = ${associatedOffset('candidate', 'GeneralCandidateLayoutV1', `${name}_OFFSET`)} as const;\n`;
output += layoutOffsets('verifier', 'RuntimeVerifierLayoutV2', 'RUNTIME_VERIFIER_', [
  ['MAGIC', 'magic'], ['VERSION', 'version'], ['HAS_CURRENT_ORDER', 'has_current_order'],
  ['OUTCOME_COUNT', 'outcome_count'], ['PAGE_COUNT', 'page_count'], ['NEXT_PAGE_INDEX', 'next_page_index'],
  ['NEXT_ROW_INDEX', 'next_row_index'], ['ORDER_COUNT', 'order_count'], ['REVISION', 'revision'],
  ['CANDIDATE_COORDINATE', 'candidate_coordinate'], ['CANDIDATE_ID', 'candidate_id'],
  ['PRODUCT_ID', 'product_id'], ['BATCH_ID', 'batch_id'], ['PRICE_SCALE', 'price_scale'],
  ['FILLED_LOTS', 'filled_lots'], ['QUOTE_DEBIT', 'quote_debit'], ['QUOTE_CREDIT', 'quote_credit'],
  ['CURRENT_ORDER_ID', 'current_order_id'], ['CURRENT_OWNER_ID', 'current_owner_id'],
  ['CURRENT_NONCE', 'current_nonce'], ['CURRENT_MAX_LOTS', 'current_max_lots'],
  ['CURRENT_MAX_QUOTE_DEBIT_PER_LOT', 'current_max_quote_debit_per_lot'], ['CURRENT_LOTS', 'current_lots'],
  ['CURRENT_SOURCE_PAGE_INDEX', 'current_source_page_index'],
  ['CURRENT_SOURCE_EXECUTION_INDEX', 'current_source_execution_index'], ['TAILS_BASE', 'tails_base'],
].map(([name, method]) => [`GENERAL_VERIFIER_${name}_OFFSET_V2`, method]));
output += `export const GENERAL_VERIFIER_TAIL_ITEM_STRIDE_V2 = ${methodOffset('verifier', 'RuntimeVerifierLayoutV2', 'tail_item_stride').offset} as const;\n`;
for (const name of ['PRICES', 'CURRENT_RECEIVE', 'CURRENT_DELIVER', 'CLAIM_INPUTS', 'CLAIM_OUTPUTS']) {
  output += `export const GENERAL_VERIFIER_${name}_TAIL_V2 = ${scalar('verifier', `${name}_TAIL`)} as const;\n`;
}
output += layoutOffsets('runtime', 'VerifiedCandidateLayoutV2', 'VERIFIED_CANDIDATE_', [
  ['MAGIC', 'magic'], ['VERSION', 'version'], ['PHASE', 'phase'], ['OUTCOME_COUNT', 'outcome_count'],
  ['PAGE_COUNT', 'page_count'], ['CANDIDATE_COORDINATE', 'candidate_coordinate'], ['REVISION', 'revision'],
  ['CANDIDATE_ID', 'candidate_id'], ['PRODUCT_ID', 'product_id'], ['BATCH_ID', 'batch_id'],
  ['FILLED_LOTS', 'filled_lots'], ['QUOTE_DEBIT', 'quote_debit'], ['QUOTE_CREDIT', 'quote_credit'],
  ['PRICE_SCALE', 'price_scale'], ['CLAIM_INPUTS_BASE', 'claim_inputs_base'],
].map(([name, method]) => [`GENERAL_VERIFIED_CANDIDATE_${name}_OFFSET_V2`, method]));
output += `export const GENERAL_VERIFIED_CANDIDATE_TAIL_ITEM_STRIDE_V2 = ${methodOffset('runtime', 'VerifiedCandidateLayoutV2', 'tail_item_stride').offset} as const;\n`;
output += `export const GENERAL_VERIFIED_CANDIDATE_PHASE_V2 = ${scalar('runtime', 'VERIFIED_PHASE')} as const;\n`;
output += layoutOffsets('selection', 'RuntimeSelectionLayoutV2', 'RUNTIME_SELECTION_', [
  ['GENERAL_SELECTION_MAGIC_OFFSET_V2', 'magic'], ['GENERAL_SELECTION_VERSION_OFFSET_V2', 'version'],
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
]);
output += layoutOffsets('runtime', 'SettlementCursorLayoutV2', 'SETTLEMENT_CURSOR_', [
  ['GENERAL_SETTLEMENT_MAGIC_OFFSET_V2', 'magic'], ['GENERAL_SETTLEMENT_VERSION_OFFSET_V2', 'version'],
  ['GENERAL_SETTLEMENT_PHASE_OFFSET_V2', 'phase'], ['GENERAL_SETTLEMENT_OUTCOME_COUNT_OFFSET_V2', 'outcome_count'],
  ['GENERAL_SETTLEMENT_ORDER_COUNT_OFFSET_V2', 'order_count'], ['GENERAL_SETTLEMENT_NEXT_ORDER_OFFSET_V2', 'next_order'],
  ['GENERAL_SETTLEMENT_REVISION_OFFSET_V2', 'revision'], ['GENERAL_SETTLEMENT_CANDIDATE_ID_OFFSET_V2', 'candidate_id'],
  ['GENERAL_SETTLEMENT_QUOTE_INVENTORY_OFFSET_V2', 'quote_inventory'],
  ['GENERAL_SETTLEMENT_COMPLETE_SET_OFFSET_V2', 'complete_set_quantity'],
  ['GENERAL_SETTLEMENT_TERMINAL_OFFSET_V2', 'terminal_coordinate'],
  ['GENERAL_SETTLEMENT_INVENTORY_OFFSET_V2', 'inventory_base'],
]);
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
