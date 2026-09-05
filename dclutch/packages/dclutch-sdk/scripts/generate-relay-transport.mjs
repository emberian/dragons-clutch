/**
 * The deadline-failure walk wire, mirrored for clients.
 *
 * `CommitDeadlineFailure` is deliberately the narrowest instruction in the
 * relay family: no provider, no record, no observation — the route that must
 * work when the relayer has stopped answering, funded at founding so the
 * walker is paid from escrow, not hoped at. This mirrors exactly what a
 * terminal needs to build it: the 32-byte instruction layout and the exact
 * 22-account frame with its per-slot privileges.
 *
 * Authority: `crates/dclutch-source/src/relay/{instruction,frame}.rs` and
 * the Lean-emitted `generated_relayed_abi.rs` for the schema version. The
 * frame order and privileges are scraped from `COMMIT_DEADLINE_FAILURE_FRAME_V1`
 * itself, never restated.
 */
import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const root = new URL('../../../', import.meta.url);
const instruction = readFileSync(new URL('crates/dclutch-source/src/relay/instruction.rs', root), 'utf8');
const frame = readFileSync(new URL('crates/dclutch-source/src/relay/frame.rs', root), 'utf8');
const abi = readFileSync(new URL('crates/dclutch-source/src/relay/generated_relayed_abi.rs', root), 'utf8');
const outputUrl = new URL('../lib/generated/relayTransportV1.ts', import.meta.url);

function required(source, pattern, what) {
  const match = source.match(pattern);
  if (!match) throw new Error(`missing ${what}`);
  return match[1];
}

const magic = required(instruction, /pub const RELAY_INSTRUCTION_MAGIC: \[u8; 8\] = \*b"([A-Z0-9]{8})";/, 'relay instruction magic');
const schemaVersion = Number(required(abi, /pub const RELAYED_SCHEMA_VERSION: u16 = ([0-9]+);/, 'relayed schema version'));
const instructionBytes = Number(required(instruction, /pub const COMMIT_DEADLINE_FAILURE_INSTRUCTION_BYTES: usize = ([0-9]+);/, 'deadline-failure width'));
const actionOffset = Number(required(instruction, /const ACTION_OFFSET: usize = ([0-9]+);/, 'action offset'));
const actionByte = Number(required(instruction, /CommitDeadlineFailure = ([0-9]+),/, 'deadline-failure action byte'));

// The two body offsets, read from the encoder's own put() calls rather than
// restated: to_bytes writes generation first, terminal_sequence second.
const deadlineBody = required(instruction, /(impl CommitDeadlineFailureInstructionV1 \{[\s\S]*?fn to_bytes[\s\S]*?\n    \})/, 'deadline-failure to_bytes body');
const generationOffset = Number(required(deadlineBody, /put\(&mut out, ([0-9]+), &self\.generation\.to_le_bytes\(\)\)/, 'generation offset'));
const terminalOffset = Number(required(deadlineBody, /put\(&mut out, ([0-9]+), &self\.terminal_sequence\.to_le_bytes\(\)\)/, 'terminal-sequence offset'));

// Role constants: `const NAME: RelayAccountRoleV1 = role(RelayAccountNameV1::X, signer, writable);`
const roles = new Map();
for (const match of frame.matchAll(/const ([A-Z_]+): RelayAccountRoleV1 =\s*role\(\s*RelayAccountNameV1::([A-Za-z0-9]+),\s*(true|false),\s*(true|false),?\s*\)/g)) {
  roles.set(match[1], { name: match[2], signer: match[3] === 'true', writable: match[4] === 'true' });
}
if (roles.size < 15) throw new Error(`only ${roles.size} relay role constants parsed; the format moved`);

const frameCount = Number(required(frame, /pub const COMMIT_DEADLINE_FAILURE_FRAME_V1: \[RelayAccountRoleV1; ([0-9]+)\]/, 'deadline-failure frame count'));
const frameBody = required(frame, /pub const COMMIT_DEADLINE_FAILURE_FRAME_V1: \[RelayAccountRoleV1; [0-9]+\] = \[([\s\S]*?)\];/, 'deadline-failure frame');
const entries = [...frameBody.matchAll(/\b([A-Z][A-Z_]*)\b/g)].map((match) => match[1]).filter((name) => roles.has(name));
if (entries.length !== frameCount) throw new Error(`frame lists ${entries.length} parsed roles against a declared ${frameCount}`);

const ts = (value) => JSON.stringify(value);
let generated = '// @generated from crates/dclutch-source/src/relay/{instruction,frame}.rs and generated_relayed_abi.rs; do not edit.\n';
generated += '// Regenerate with: npm run abi:relay-transport\n\n';
generated += `export const RELAY_INSTRUCTION_MAGIC = ${ts(magic)} as const;\n`;
generated += `export const RELAYED_SCHEMA_VERSION = ${schemaVersion} as const;\n`;
generated += `export const RELAY_ACTION_OFFSET = ${actionOffset} as const;\n`;
generated += `export const COMMIT_DEADLINE_FAILURE_ACTION = ${actionByte} as const;\n`;
generated += `export const COMMIT_DEADLINE_FAILURE_INSTRUCTION_BYTES = ${instructionBytes} as const;\n`;
generated += `export const COMMIT_DEADLINE_FAILURE_GENERATION_OFFSET = ${generationOffset} as const;\n`;
generated += `export const COMMIT_DEADLINE_FAILURE_TERMINAL_SEQUENCE_OFFSET = ${terminalOffset} as const;\n\n`;
generated += 'export interface RelayFrameSlotV1 {\n  readonly name: string;\n  readonly signer: boolean;\n  readonly writable: boolean;\n}\n\n';
generated += 'export const COMMIT_DEADLINE_FAILURE_FRAME_V1: ReadonlyArray<RelayFrameSlotV1> = [\n';
for (const entry of entries) {
  const role = roles.get(entry);
  generated += `  { name: ${ts(role.name)}, signer: ${role.signer}, writable: ${role.writable} },\n`;
}
generated += '];\n';

if (process.argv.includes('--check')) {
  const current = readFileSync(outputUrl, 'utf8');
  if (current !== generated) {
    console.error('relay transport TypeScript mirror is stale');
    process.exit(1);
  }
} else {
  writeFileSync(fileURLToPath(outputUrl), generated);
}
