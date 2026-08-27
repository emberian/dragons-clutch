import { readFileSync, renameSync, unlinkSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

// TWO live sources, and only live ones.
//
//   crates/dclutch-product-runtime-v2-admission/src/lib.rs
//     the wire: DCLTPRQ2 request, DCLTPRM2 Product record, DCLTPRA2 receipt,
//     their exact widths, their reserved spans, and the three schema IDs the
//     receipt decoder pins.
//   programs/dclutch-product-runtime-v2-sbf/src/lib.rs
//     the frame: the exact executable account count and the refusal band the
//     adapter raises from.
//
// WHY A SECOND PRODUCT GENERATOR EXISTS. `DCLTPRQ2` names TWO incompatible
// 112-byte requests. The dead one belonged to `dclutch-product-payoff-v2-svm`,
// whose browser encoder was deleted on 2026-08-27 with the rest of an evaluator
// chain no `programs/` package linked; it wrote 1 at byte 10. The live one is
// this crate's `AdmissionRequestV2`, whose decoder REQUIRES ZERO at bytes
// 10..16. A browser that mirrored the dead layout would have built a request
// the deployed program refuses as `NonCanonical`, and the two would have looked
// identical in a diff: same magic, same width. So every coordinate below is
// read out of the live crate rather than typed, and the reserved span is read
// as a span rather than assumed.
//
// The offset constants in the admission crate are private (`const`, not
// `pub const`). That is deliberate on the Rust side and irrelevant here: the
// generator reads source text, and a private constant is still the crate's
// single statement of where a field sits.
const root = new URL('../../../', import.meta.url);
const sources = Object.freeze({
  admission: readFileSync(new URL('crates/dclutch-product-runtime-v2-admission/src/lib.rs', root), 'utf8'),
  adapter: readFileSync(new URL('programs/dclutch-product-runtime-v2-sbf/src/lib.rs', root), 'utf8'),
});
const outputUrl = new URL('../lib/generated/productRuntimeV2Admission.ts', import.meta.url);

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

function byteString(source, name) {
  const match = sources[source].match(new RegExp(`(?:pub )?const ${name}: &\\[u8\\] =\\s*b"([^"]+)";`));
  if (!match) throw new Error(`missing Rust byte string ${source}.${name}`);
  return match[1];
}

function digest(source, name) {
  const match = sources[source].match(new RegExp(`(?:pub )?const ${name}: \\[u8; 32\\] = \\[([^\\]]+)\\];`));
  if (!match) throw new Error(`missing Rust 32-byte identity ${source}.${name}`);
  const values = match[1].trim().split(',').map((entry) => entry.trim()).filter((entry) => entry.length > 0);
  if (values.length !== 32) throw new Error(`Rust identity ${source}.${name} is not 32 bytes`);
  return values.map((entry) => {
    const value = Number(entry);
    if (!Number.isInteger(value) || value < 0 || value > 255) throw new Error(`Rust identity ${source}.${name} holds a non-byte`);
    return value;
  });
}

/**
 * A reserved span written as `require_zero(bytes, <offset>, <length>)` inside a
 * named decoder. Read from the call site rather than restated, because the
 * whole point of this ABI is that the browser must zero exactly what the
 * program checks -- no more, and above all no less.
 */
function reservedSpan(source, decoderMarker) {
  const body = sources[source].slice(sources[source].indexOf(decoderMarker));
  const match = body.match(/require_zero\(bytes, (\d+), (\d+)\)\?;/);
  if (!match) throw new Error(`missing require_zero span after ${decoderMarker}`);
  return { offset: Number(match[1]), length: Number(match[2]) };
}

/**
 * The header read itself, taken from the decoder's own expression rather than
 * assumed to sit at 0 and 8. All three records are checked and required to
 * agree, so a future record that moved its version field could not be silently
 * folded into one shared TypeScript constant.
 */
function headerRead(source, decoderMarker) {
  const body = sources[source].slice(sources[source].indexOf(decoderMarker));
  const match = body.match(/array::<(\d+)>\(bytes, (\d+)\)\? != [A-Z_0-9]+\s*\n?\s*\|\| read_u16\(bytes, (\d+)\)\?/);
  if (!match) throw new Error(`missing magic/version header read after ${decoderMarker}`);
  return { magicBytes: Number(match[1]), magicOffset: Number(match[2]), versionOffset: Number(match[3]) };
}

const requestReserved = reservedSpan('admission', 'impl AdmissionRequestV2 {');
const recordReserved = reservedSpan('admission', 'impl ProductRecordV2 {');
const receiptReserved = reservedSpan('admission', 'impl AdmissionReceiptV2 {');
const headers = ['impl AdmissionRequestV2 {', 'impl ProductRecordV2 {', 'impl AdmissionReceiptV2 {'].map((marker) => headerRead('admission', marker));
const header = headers[0];
for (const other of headers.slice(1)) {
  if (other.magicBytes !== header.magicBytes || other.magicOffset !== header.magicOffset || other.versionOffset !== header.versionOffset) {
    throw new Error('the three Product Runtime V2 records no longer share one header layout; emit them separately');
  }
}

let output = '// @generated from the live Rust Product Runtime V2 admission ABI; do not edit.\n';
output += '// Regenerate with: npm run abi:product-runtime-v2-admission\n\n';
for (const [name, rustName] of [
  ['ADMISSION_REQUEST_MAGIC_V2', 'ADMISSION_REQUEST_MAGIC_V2'],
  ['PRODUCT_RECORD_MAGIC_V2', 'PRODUCT_RECORD_MAGIC_V2'],
  ['ADMISSION_RECEIPT_MAGIC_V2', 'ADMISSION_RECEIPT_MAGIC_V2'],
]) output += `export const ${name} = '${magic('admission', rustName)}' as const;\n`;
output += `export const ADMISSION_RECEIPT_PDA_DOMAIN_V2 = '${byteString('admission', 'ADMISSION_RECEIPT_PDA_DOMAIN_V2')}' as const;\n`;
for (const [name, rustName] of [
  ['ADMISSION_VERSION_V2', 'ADMISSION_VERSION_V2'],
  ['ADMISSION_RECORD_COUNT_V2', 'ADMISSION_RECORD_COUNT_V2'],
  ['ADMISSION_REQUEST_BYTES_V2', 'ADMISSION_REQUEST_BYTES_V2'],
  ['PRODUCT_RECORD_BYTES_V2', 'PRODUCT_RECORD_BYTES_V2'],
  ['ADMISSION_RECEIPT_BYTES_V2', 'ADMISSION_RECEIPT_BYTES_V2'],
  ['REQUEST_PRODUCT_DIGEST_OFFSET_V2', 'REQUEST_PRODUCT_DIGEST_OFFSET'],
  ['REQUEST_DOMAIN_DIGEST_OFFSET_V2', 'REQUEST_DOMAIN_DIGEST_OFFSET'],
  ['REQUEST_PORTFOLIO_DIGEST_OFFSET_V2', 'REQUEST_PORTFOLIO_DIGEST_OFFSET'],
  ['PRODUCT_ID_OFFSET_V2', 'PRODUCT_ID_OFFSET'],
  ['PRODUCT_DOMAIN_DIGEST_OFFSET_V2', 'PRODUCT_DOMAIN_DIGEST_OFFSET'],
  ['PRODUCT_PORTFOLIO_DIGEST_OFFSET_V2', 'PRODUCT_PORTFOLIO_DIGEST_OFFSET'],
  ['RECEIPT_COUNT_OFFSET_V2', 'RECEIPT_COUNT_OFFSET'],
  ['RECEIPT_RECORDS_OFFSET_V2', 'RECEIPT_RECORDS_OFFSET'],
  ['RECORD_COORDINATE_BYTES_V2', 'RECORD_COORDINATE_BYTES'],
]) output += `export const ${name} = ${scalar('admission', rustName)} as const;\n`;
output += `export const ADMISSION_ACCOUNT_COUNT_V2 = ${scalar('adapter', 'ADMISSION_ACCOUNT_COUNT_V2')} as const;\n`;
output += `export const ADMISSION_MAGIC_OFFSET_V2 = ${header.magicOffset} as const;\n`;
output += `export const ADMISSION_MAGIC_BYTES_V2 = ${header.magicBytes} as const;\n`;
output += `export const ADMISSION_VERSION_OFFSET_V2 = ${header.versionOffset} as const;\n`;
for (const [name, span] of [
  ['REQUEST_RESERVED', requestReserved],
  ['PRODUCT_RECORD_RESERVED', recordReserved],
  ['RECEIPT_RESERVED', receiptReserved],
]) {
  output += `export const ${name}_OFFSET_V2 = ${span.offset} as const;\n`;
  output += `export const ${name}_BYTES_V2 = ${span.length} as const;\n`;
}
for (const [name, rustName] of [
  ['PRODUCT_RECORD_SCHEMA_ID_V2', 'PRODUCT_RECORD_SCHEMA_ID_V2'],
  ['RESULT_DOMAIN_SCHEMA_ID_V2', 'RESULT_DOMAIN_SCHEMA_ID_V2'],
  ['PORTFOLIO_SCHEMA_ID_V2', 'PORTFOLIO_SCHEMA_ID_V2'],
]) output += `export const ${name} = Uint8Array.from([${digest('admission', rustName).join(', ')}]);\n`;
for (const [name, rustName] of [
  ['PRODUCT_RECORD_SCHEMA_PREIMAGE_V2', 'PRODUCT_RECORD_SCHEMA_PREIMAGE_V2'],
  ['RESULT_DOMAIN_SCHEMA_PREIMAGE_V2', 'RESULT_DOMAIN_SCHEMA_PREIMAGE_V2'],
  ['PORTFOLIO_SCHEMA_PREIMAGE_V2', 'PORTFOLIO_SCHEMA_PREIMAGE_V2'],
  ['ADMISSION_RECEIPT_SCHEMA_PREIMAGE_V2', 'ADMISSION_RECEIPT_SCHEMA_PREIMAGE_V2'],
]) output += `export const ${name} = '${byteString('admission', rustName)}' as const;\n`;

if (process.argv.includes('--check')) {
  if (readFileSync(outputUrl, 'utf8') !== output) {
    console.error('Product Runtime V2 admission TypeScript ABI is stale');
    process.exit(1);
  }
} else {
  const outputPath = fileURLToPath(outputUrl);
  const temporaryPath = `${outputPath}.tmp-${process.pid}`;
  try {
    writeFileSync(temporaryPath, output, { flag: 'wx' });
    const staged = readFileSync(temporaryPath, 'utf8');
    if (!staged.startsWith('// @generated from the live Rust Product Runtime V2 admission ABI; do not edit.\n')
        || !staged.includes("export const ADMISSION_REQUEST_MAGIC_V2 = 'DCLTPRQ2'")
        || !staged.includes('export const ADMISSION_REQUEST_BYTES_V2 = 112')
        || !staged.includes('export const REQUEST_RESERVED_OFFSET_V2 = 10')
        || !staged.includes('export const REQUEST_RESERVED_BYTES_V2 = 6')) {
      throw new Error('generated Product Runtime V2 admission TypeScript ABI failed its header/width validation');
    }
    renameSync(temporaryPath, outputPath);
  } catch (error) {
    try { unlinkSync(temporaryPath); } catch {}
    throw error;
  }
}
