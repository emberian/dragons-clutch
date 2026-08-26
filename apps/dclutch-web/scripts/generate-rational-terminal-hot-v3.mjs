import { readFileSync, renameSync, unlinkSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const root = new URL('../../../', import.meta.url);
const sources = Object.freeze({
  child: readFileSync(new URL('crates/dclutch-rational-representation-v2-contract/src/generated.rs', root), 'utf8'),
  hot: readFileSync(new URL('crates/dclutch-rational-representation-v2-contract/src/generated_hot_v3.rs', root), 'utf8'),
});
const outputUrl = new URL('../lib/generated/rationalTerminalHotV3.ts', import.meta.url);

function scalar(source, name) {
  const match = sources[source].match(new RegExp(`pub const ${name}: [^=]+ = ([0-9_]+);`));
  if (!match) throw new Error(`missing Rust scalar ${source}.${name}`);
  return Number(match[1].replaceAll('_', ''));
}

function bytes(source, name) {
  const match = sources[source].match(new RegExp(`pub const ${name}: \\[u8; [^\\]]+\\] =\\s*(?:\\*b"([^"]+)"|\\[([\\s\\S]*?)\\]);`));
  if (!match) throw new Error(`missing Rust bytes ${source}.${name}`);
  if (match[1]) return [...new TextEncoder().encode(match[1])];
  return [...match[2].matchAll(/0x[0-9a-f]+|\b[0-9]+\b/g)].map((entry) => Number(entry[0]));
}

function array(name, values) {
  return `export const ${name} = Uint8Array.from([${values.map((value) => `0x${value.toString(16).padStart(2, '0')}`).join(', ')}]);\n`;
}

const scalars = [
  ['child', 'PHYSICAL_ABI_VERSION_V2'], ['child', 'REQUEST_HEADER_BYTES_V2'], ['child', 'ASSET_BYTES_V2'],
  ['child', 'REQUEST_MAGIC_OFFSET'], ['child', 'REQUEST_VERSION_OFFSET'], ['child', 'REQUEST_ACTION_OFFSET'],
  ['child', 'REQUEST_CALLER_ROLE_OFFSET'], ['child', 'REQUEST_RESERVED_HEADER_OFFSET'], ['child', 'REQUEST_RELEASE_SET_OFFSET'],
  ['child', 'REQUEST_MARKET_OFFSET'], ['child', 'REQUEST_GRAPH_ID_OFFSET'], ['child', 'REQUEST_DESCRIPTOR_ID_OFFSET'],
  ['child', 'REQUEST_PARENT_CONTEXT_OFFSET'], ['child', 'REQUEST_ACTOR_OFFSET'], ['child', 'REQUEST_RECEIPT_MINT_OFFSET'],
  ['child', 'REQUEST_RECEIPT_ACCOUNT_OFFSET'], ['child', 'REQUEST_REPRESENTATION_AUTHORITY_OFFSET'],
  ['child', 'REQUEST_TOKEN_PROGRAM_OFFSET'], ['child', 'REQUEST_REALM_OFFSET'], ['child', 'REQUEST_COLLATERAL_RECIPIENT_OFFSET'],
  ['child', 'REQUEST_EXPECTED_REPRESENTATION_REVISION_OFFSET'], ['child', 'REQUEST_EXPECTED_CLAIMS_MARKET_REVISION_OFFSET'],
  ['child', 'REQUEST_EXPECTED_ACTOR_POSITION_REVISION_OFFSET'], ['child', 'REQUEST_EXPECTED_CUSTODY_POSITION_REVISION_OFFSET'],
  ['child', 'REQUEST_EXPECTED_CUSTODY_REPLAY_REVISION_OFFSET'], ['child', 'REQUEST_GENERATION_OFFSET'],
  ['child', 'REQUEST_QUANTITY_OFFSET'], ['child', 'REQUEST_DENOMINATOR_OFFSET'], ['child', 'REQUEST_EXPECTED_RECEIPT_SUPPLY_OFFSET'],
  ['child', 'REQUEST_OUTCOME_COUNT_OFFSET'], ['child', 'REQUEST_SELECTED_OUTCOME_OFFSET'], ['child', 'REQUEST_ASSET_COUNT_OFFSET'],
  ['child', 'REQUEST_RESERVED_TAIL_OFFSET'], ['child', 'ASSET_SHARD_MINT_OFFSET'], ['child', 'ASSET_ACTOR_SHARD_ACCOUNT_OFFSET'],
  ['child', 'ASSET_STRUCTURED_CUSTODY_ACCOUNT_OFFSET'], ['child', 'ASSET_CLAIMS_CUSTODY_OWNER_OFFSET'],
  ['child', 'ASSET_COEFFICIENT_OFFSET'], ['child', 'ASSET_EXPECTED_SHARD_SUPPLY_OFFSET'],
  ['child', 'ASSET_EXPECTED_ACTOR_SHARDS_OFFSET'], ['child', 'ASSET_EXPECTED_STRUCTURED_SHARDS_OFFSET'],
  ['child', 'ACTION_REDEEM_TERMINAL'], ['child', 'CALLER_ROLE_TRADING'],
  ['hot', 'RATIONAL_TERMINAL_HOT_VERSION_V3'], ['hot', 'RATIONAL_TERMINAL_HOT_REQUEST_BYTES_V3'],
  ['hot', 'RATIONAL_TERMINAL_HOT_FIXED_ASSET_COUNT_V3'], ['hot', 'RATIONAL_TERMINAL_HOT_MAGIC_OFFSET_V3'],
  ['hot', 'RATIONAL_TERMINAL_HOT_VERSION_OFFSET_V3'], ['hot', 'RATIONAL_TERMINAL_HOT_ACTION_OFFSET_V3'],
  ['hot', 'RATIONAL_TERMINAL_HOT_CALLER_ROLE_OFFSET_V3'], ['hot', 'RATIONAL_TERMINAL_HOT_PARENT_CONTEXT_OFFSET_V3'],
];

let output = '// @generated from the Lean-owned Rational terminal Hot V3 and Claims-child ABIs; do not edit.\n';
output += '// Regenerate with: npm run abi:rational-terminal-v3\n\n';
for (const [source, name] of scalars) output += `export const ${name} = ${scalar(source, name)} as const;\n`;
output += '\n';
for (const [source, name] of [
  ['child', 'REQUEST_MAGIC_V2'], ['hot', 'RATIONAL_TERMINAL_HOT_MAGIC_V3'],
]) output += array(name, bytes(source, name));

if (process.argv.includes('--check')) {
  if (readFileSync(outputUrl, 'utf8') !== output) {
    console.error('Rational terminal Hot V3 TypeScript ABI is stale');
    process.exit(1);
  }
} else {
  const outputPath = fileURLToPath(outputUrl);
  const temporaryPath = `${outputPath}.tmp-${process.pid}`;
  try {
    writeFileSync(temporaryPath, output, { flag: 'wx' });
    const staged = readFileSync(temporaryPath, 'utf8');
    if (!staged.startsWith('// @generated from the Lean-owned Rational terminal Hot V3')
        || !staged.includes('RATIONAL_TERMINAL_HOT_REQUEST_BYTES_V3 = 648')) {
      throw new Error('generated Rational terminal Hot V3 ABI failed header/width validation');
    }
    renameSync(temporaryPath, outputPath);
  } catch (error) {
    try { unlinkSync(temporaryPath); } catch {}
    throw error;
  }
}
