import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const root = new URL('../../../', import.meta.url);
const lifecycle = readFileSync(new URL('crates/dclutch-direct-codec/src/generated_lifecycle.rs', root), 'utf8');
const controller = readFileSync(new URL('crates/dclutch-direct-codec/src/generated_registered_controller.rs', root), 'utf8');
const outputUrl = new URL('../lib/generated/registeredDirect.ts', import.meta.url);

function scalar(source, name) {
  const match = source.match(new RegExp(`const ${name}: [^=]+ = ([0-9]+);`));
  if (!match) throw new Error(`missing Rust scalar ${name}`);
  return Number(match[1]);
}

function bytes(source, name) {
  const match = source.match(new RegExp(`const ${name}: \\[u8; [0-9]+\\] = \\[([\\s\\S]*?)\\n\\];`));
  if (!match) throw new Error(`missing Rust byte array ${name}`);
  return [...match[1].matchAll(/0x[0-9a-f]+|\b[0-9]+\b/g)].map((entry) => Number(entry[0]));
}

const scalars = [
  ['REGISTERED_STATE_BYTES_VALUE', lifecycle], ['REGISTERED_STATE_ABI_VERSION', lifecycle],
  ['REGISTERED_STATE_MAGIC_OFFSET', lifecycle], ['REGISTERED_STATE_VERSION_OFFSET', lifecycle],
  ['REGISTERED_STATE_PHASE_OFFSET', lifecycle], ['REGISTERED_STATE_RESERVED_OFFSET', lifecycle],
  ['REGISTERED_STATE_CONTROLLER_OFFSET', lifecycle], ['REGISTERED_STATE_MAKER_OFFSET', lifecycle],
  ['REGISTERED_STATE_INTENT_OFFSET', lifecycle], ['REGISTERED_STATE_REMAINING_OFFSET', lifecycle],
  ['REGISTERED_STATE_SEQUENCE_OFFSET', lifecycle],
  ['REGISTERED_CONTROLLER_BYTES_VALUE', controller], ['REGISTERED_CONTROLLER_ABI_VERSION', controller],
  ['REGISTERED_CONTROLLER_MAGIC_OFFSET', controller], ['REGISTERED_CONTROLLER_VERSION_OFFSET', controller],
  ['REGISTERED_CONTROLLER_BUMP_OFFSET', controller], ['REGISTERED_SELLER_REGISTRATION_BUMP_OFFSET', controller],
  ['REGISTERED_BUYER_REGISTRATION_BUMP_OFFSET', controller], ['REGISTERED_SELLER_POSITION_BUMP_OFFSET', controller],
  ['REGISTERED_BUYER_POSITION_BUMP_OFFSET', controller], ['REGISTERED_CONTROLLER_RESERVED_OFFSET', controller],
  ['REGISTERED_CONTROLLER_FILL_OFFSET', controller], ['REGISTERED_CONTROLLER_EXECUTION_PRICE_OFFSET', controller],
  ['REGISTERED_TERMINAL_BYTES_VALUE', controller], ['REGISTERED_TERMINAL_ABI_VERSION', controller],
  ['REGISTERED_TERMINAL_MAGIC_OFFSET', controller], ['REGISTERED_TERMINAL_VERSION_OFFSET', controller],
  ['REGISTERED_TERMINAL_ACTION_OFFSET', controller], ['REGISTERED_TERMINAL_CONTROLLER_BUMP_OFFSET', controller],
  ['REGISTERED_TERMINAL_REGISTRATION_BUMP_OFFSET', controller], ['REGISTERED_TERMINAL_RESERVED_OFFSET', controller],
  ['REGISTERED_TERMINAL_EXPECTED_SEQUENCE_OFFSET', controller], ['REGISTERED_TERMINAL_CANCEL', controller],
  ['REGISTERED_TERMINAL_EXPIRE', controller],
];

const arrays = [
  ['REGISTERED_STATE_MAGIC_BYTES', lifecycle], ['REGISTERED_STATE_EXAMPLE', lifecycle],
  ['REGISTERED_CONTROLLER_MAGIC_BYTES', controller], ['REGISTERED_CONTROLLER_EXAMPLE', controller],
  ['REGISTERED_TERMINAL_MAGIC_BYTES', controller], ['REGISTERED_TERMINAL_CANCEL_EXAMPLE', controller],
  ['REGISTERED_TERMINAL_EXPIRE_EXAMPLE', controller],
];

let generated = '// @generated from dclutch-direct-codec Lean-emitted Rust ABI; do not edit.\n';
generated += '// Registered fill/terminal ABI baseline: git 7220fb7.\n';
generated += '// Regenerate with: npm run abi:registered\n\n';
for (const [name, source] of scalars) generated += `export const ${name} = ${scalar(source, name)} as const;\n`;
generated += '\n';
for (const [name, source] of arrays) {
  const values = bytes(source, name).map((value) => `0x${value.toString(16).padStart(2, '0')}`);
  const lines = [];
  for (let index = 0; index < values.length; index += 16) lines.push(`  ${values.slice(index, index + 16).join(', ')},`);
  generated += `export const ${name} = Uint8Array.from([\n${lines.join('\n')}\n]);\n`;
}

if (process.argv.includes('--check')) {
  const current = readFileSync(outputUrl, 'utf8');
  if (current !== generated) {
    console.error('registered Direct TypeScript ABI is stale');
    process.exit(1);
  }
} else {
  writeFileSync(fileURLToPath(outputUrl), generated);
}
