import { readFileSync, renameSync, unlinkSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const root = new URL('../../../', import.meta.url);
const sources = Object.freeze({
  codec: readFileSync(new URL('crates/dclutch-product-payoff-v2-codec/src/lib.rs', root), 'utf8'),
  svm: readFileSync(new URL('crates/dclutch-product-payoff-v2-svm/src/lib.rs', root), 'utf8'),
  admission: readFileSync(new URL('crates/dclutch-product-admission-contract/src/lib.rs', root), 'utf8'),
});
const outputUrl = new URL('../lib/generated/productV2Payoff.ts', import.meta.url);

function scalar(source, name) {
  const match = sources[source].match(new RegExp(`(?:pub(?:\\(crate\\))? )?const ${name}: [^=]+ = ([0-9_]+);`));
  if (!match) throw new Error(`missing Rust scalar ${source}.${name}`);
  return Number(match[1].replaceAll('_', ''));
}

let output = '// @generated from canonical Rust Product V2 payoff/admission ABIs; do not edit.\n';
output += '// Regenerate with: npm run abi:product-v2-payoff\n\n';
for (const [name, source, rustName] of [
  ['PRODUCT_V2_BYTES', 'codec', 'ABI_BYTES_V2'],
  ['PAYOFF_REQUEST_BYTES_V2', 'svm', 'PAYOFF_REQUEST_BYTES_V2'],
  ['PAYOFF_ADMISSION_REQUEST_BYTES_V1', 'admission', 'PAYOFF_ADMISSION_REQUEST_BYTES_V1'],
]) output += `export const ${name} = ${scalar(source, rustName)} as const;\n`;

if (process.argv.includes('--check')) {
  if (readFileSync(outputUrl, 'utf8') !== output) {
    console.error('Product V2 payoff/admission TypeScript ABI is stale');
    process.exit(1);
  }
} else {
  const outputPath = fileURLToPath(outputUrl);
  const temporaryPath = `${outputPath}.tmp-${process.pid}`;
  try {
    writeFileSync(temporaryPath, output, { flag: 'wx' });
    const staged = readFileSync(temporaryPath, 'utf8');
    if (!staged.startsWith('// @generated from canonical Rust Product V2 payoff/admission ABIs; do not edit.\n')
        || !staged.includes('export const PRODUCT_V2_BYTES =')) {
      throw new Error('generated Product V2 payoff/admission TypeScript ABI failed its header/width validation');
    }
    renameSync(temporaryPath, outputPath);
  } catch (error) {
    try { unlinkSync(temporaryPath); } catch {}
    throw error;
  }
}
