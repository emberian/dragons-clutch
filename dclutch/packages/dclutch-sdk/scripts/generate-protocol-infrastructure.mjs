import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const root = new URL('../../../', import.meta.url);
const generatedRust = readFileSync(
  new URL('crates/dclutch-release-set-contract/src/generated_protocol_infrastructure.rs', root),
  'utf8',
);
const contractRust = readFileSync(
  new URL('crates/dclutch-release-set-contract/src/protocol_infrastructure.rs', root),
  'utf8',
);
const outputUrl = new URL('../lib/generated/protocolInfrastructure.ts', import.meta.url);

function scalar(name) {
  const match = generatedRust.match(new RegExp(`const ${name}: [^=]+ = ([0-9]+);`));
  if (!match) throw new Error(`missing Rust scalar ${name}`);
  return Number(match[1]);
}

function bytes(name) {
  const match = generatedRust.match(
    new RegExp(`const ${name}: \\[u8; [0-9]+\\] =\\s*\\[([\\s\\S]*?)\\];`),
  );
  if (!match) throw new Error(`missing Rust byte array ${name}`);
  return [...match[1].matchAll(/0x[0-9a-f]+|\b[0-9]+\b/g)].map((entry) => Number(entry[0]));
}

function byteString(name) {
  const match = contractRust.match(new RegExp(`const ${name}: &\\[u8\\] = b"([^"]+)";`));
  if (!match) throw new Error(`missing Rust byte string ${name}`);
  return match[1];
}

const scalars = [
  'PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V1',
  'PROTOCOL_INFRASTRUCTURE_PROFILE_SCHEMA_VERSION_V1',
  'PROTOCOL_INFRASTRUCTURE_PROFILE_ARTIFACT_PROFILE_V1',
  'PROTOCOL_INFRASTRUCTURE_PROFILE_MAGIC_OFFSET_V1',
  'PROTOCOL_INFRASTRUCTURE_PROFILE_SCHEMA_VERSION_OFFSET_V1',
  'PROTOCOL_INFRASTRUCTURE_PROFILE_ARTIFACT_PROFILE_OFFSET_V1',
  'PROTOCOL_INFRASTRUCTURE_PROFILE_RESERVED_OFFSET_V1',
  'PROTOCOL_INFRASTRUCTURE_PROFILE_REGISTRY_PROGRAM_OFFSET_V1',
  'PROTOCOL_INFRASTRUCTURE_PROFILE_REGISTRY_ARTIFACT_OFFSET_V1',
  'PROTOCOL_INFRASTRUCTURE_PROFILE_RENT_PROGRAM_OFFSET_V1',
  'PROTOCOL_INFRASTRUCTURE_PROFILE_RENT_ARTIFACT_OFFSET_V1',
];

// V2 is the succession layout (docs/design/PROFILE_UPGRADE_RULING_2026_08_31.md):
// V1's eight fields at V1's exact offsets, then the two predecessor artifact ids
// that make the succession content-walkable, then a reserved tail. There is no
// ARTIFACT_PROFILE_V2 value -- the artifact-record format did not move, so a V2
// profile's artifact-profile field is compared against the V1 constant.
const scalarsV2 = [
  'PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V2',
  'PROTOCOL_INFRASTRUCTURE_PROFILE_SCHEMA_VERSION_V2',
  'PROTOCOL_INFRASTRUCTURE_PROFILE_MAGIC_OFFSET_V2',
  'PROTOCOL_INFRASTRUCTURE_PROFILE_SCHEMA_VERSION_OFFSET_V2',
  'PROTOCOL_INFRASTRUCTURE_PROFILE_ARTIFACT_PROFILE_OFFSET_V2',
  'PROTOCOL_INFRASTRUCTURE_PROFILE_RESERVED_OFFSET_V2',
  'PROTOCOL_INFRASTRUCTURE_PROFILE_REGISTRY_PROGRAM_OFFSET_V2',
  'PROTOCOL_INFRASTRUCTURE_PROFILE_REGISTRY_ARTIFACT_OFFSET_V2',
  'PROTOCOL_INFRASTRUCTURE_PROFILE_RENT_PROGRAM_OFFSET_V2',
  'PROTOCOL_INFRASTRUCTURE_PROFILE_RENT_ARTIFACT_OFFSET_V2',
  'PROTOCOL_INFRASTRUCTURE_PROFILE_PREDECESSOR_REGISTRY_ARTIFACT_OFFSET_V2',
  'PROTOCOL_INFRASTRUCTURE_PROFILE_PREDECESSOR_RENT_ARTIFACT_OFFSET_V2',
  'PROTOCOL_INFRASTRUCTURE_PROFILE_RESERVED_TAIL_OFFSET_V2',
];

let output = '// @generated from the Lean-emitted ProtocolInfrastructureProfile Rust ABI; do not edit.\n';
output += '// Regenerate with: npm run abi:infrastructure\n\n';
for (const name of scalars) output += `export const ${name} = ${scalar(name)} as const;\n`;
output += '\n';
const magic = bytes('PROTOCOL_INFRASTRUCTURE_PROFILE_MAGIC_V1');
output += `export const PROTOCOL_INFRASTRUCTURE_PROFILE_MAGIC_V1 = Uint8Array.from([${magic.map((value) => `0x${value.toString(16).padStart(2, '0')}`).join(', ')}]);\n`;
output += `export const PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1 = new TextEncoder().encode('${byteString('PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1')}');\n`;
output += '\n';
for (const name of scalarsV2) output += `export const ${name} = ${scalar(name)} as const;\n`;
output += '\n';
const magicV2 = bytes('PROTOCOL_INFRASTRUCTURE_PROFILE_MAGIC_V2');
output += `export const PROTOCOL_INFRASTRUCTURE_PROFILE_MAGIC_V2 = Uint8Array.from([${magicV2.map((value) => `0x${value.toString(16).padStart(2, '0')}`).join(', ')}]);\n`;
output += `export const PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V2 = new TextEncoder().encode('${byteString('PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V2')}');\n`;

if (process.argv.includes('--check')) {
  const current = readFileSync(outputUrl, 'utf8');
  if (current !== output) {
    console.error('Protocol infrastructure TypeScript ABI is stale');
    process.exit(1);
  }
} else {
  writeFileSync(fileURLToPath(outputUrl), output);
}
