import { readFileSync, renameSync, unlinkSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

// One source: the crate that owns the canonical 576-byte DCLTPAY2 payoff record.
//
// Until 2026-08-27 this generator also read `dclutch-product-payoff-v2-svm` and
// `dclutch-product-admission-contract` for the evidence/admission request widths,
// and lib/productV2.ts hand-mirrored a dozen 32-byte identities beside them. That
// whole chain is dead: no package under programs/ links either crate, so no ELF
// existed for the transaction the browser composed — and the hand-mirror had
// already drifted, pinning RESOLUTION_CONTROLLER_RELEASE_ID_V3 against a chain
// that publishes V4. The dead half is deleted; what remains is authored data.
const root = new URL('../../../', import.meta.url);
const sources = Object.freeze({
  codec: readFileSync(new URL('crates/dclutch-product-payoff-v2-codec/src/lib.rs', root), 'utf8'),
});
const outputUrl = new URL('../lib/generated/productV2Payoff.ts', import.meta.url);

function scalar(source, name) {
  const match = sources[source].match(new RegExp(`(?:pub(?:\\(crate\\))? )?const ${name}: [^=]+ = ([0-9_]+);`));
  if (!match) throw new Error(`missing Rust scalar ${source}.${name}`);
  return Number(match[1].replaceAll('_', ''));
}

function magic(source, name) {
  const match = sources[source].match(new RegExp(`(?:pub(?:\\(crate\\))? )?const ${name}: \\[u8; 8\\] = \\*b"([A-Z0-9]{8})";`));
  if (!match) throw new Error(`missing Rust 8-byte magic ${source}.${name}`);
  return match[1];
}

let output = '// @generated from the canonical Rust Product V2 payoff ABI; do not edit.\n';
output += '// Regenerate with: npm run abi:product-v2-payoff\n\n';
output += `export const PRODUCT_V2_MAGIC = '${magic('codec', 'MAGIC_V2')}' as const;\n`;
for (const [name, rustName] of [
  ['PRODUCT_V2_VERSION', 'VERSION_V2'],
  ['PRODUCT_V2_BYTES', 'ABI_BYTES_V2'],
  ['PRODUCT_V2_MAX_KNOTS', 'MAX_KNOTS_V2'],
  ['PRODUCT_V2_MAX_TERMS', 'MAX_TERMS_V2'],
  ['PRODUCT_V2_HEADER_BYTES', 'HEADER_BYTES_V2'],
  ['PRODUCT_V2_KNOT_BYTES', 'KNOT_BYTES_V2'],
  ['PRODUCT_V2_TERM_BYTES', 'TERM_BYTES_V2'],
  ['PRODUCT_V2_KNOTS_OFFSET', 'KNOTS_OFFSET_V2'],
  ['PRODUCT_V2_TERMS_OFFSET', 'TERMS_OFFSET_V2'],
]) output += `export const ${name} = ${scalar('codec', rustName)} as const;\n`;

if (process.argv.includes('--check')) {
  if (readFileSync(outputUrl, 'utf8') !== output) {
    console.error('Product V2 payoff TypeScript ABI is stale');
    process.exit(1);
  }
} else {
  const outputPath = fileURLToPath(outputUrl);
  const temporaryPath = `${outputPath}.tmp-${process.pid}`;
  try {
    writeFileSync(temporaryPath, output, { flag: 'wx' });
    const staged = readFileSync(temporaryPath, 'utf8');
    if (!staged.startsWith('// @generated from the canonical Rust Product V2 payoff ABI; do not edit.\n')
        || !staged.includes('export const PRODUCT_V2_BYTES =')
        || !staged.includes("export const PRODUCT_V2_MAGIC = 'DCLTPAY2'")) {
      throw new Error('generated Product V2 payoff TypeScript ABI failed its header/width validation');
    }
    renameSync(temporaryPath, outputPath);
  } catch (error) {
    try { unlinkSync(temporaryPath); } catch {}
    throw error;
  }
}
