import { readFileSync, renameSync, unlinkSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

import { requireGeneratorFollowsRoute } from './route-binding.mjs';

// TWO live sources scraped, and only live ones -- plus one read to prove they
// are the right ones.
//
//   crates/dclutch-product-runtime-v2-admission/src/lib.rs
//     the wire: DCLTPRQ2 request, DCLTPRM2 Product record, DCLTPRA2 receipt,
//     their exact widths, their reserved spans, and the three schema IDs the
//     receipt decoder pins.
//   programs/dclutch-product-runtime-v2-sbf/src/lib.rs
//     the frame: the exact executable account count and the refusal band the
//     adapter raises from. NOT the authority for any schema ID below -- it
//     binds none of them, which is exactly why the gate reads a third file.
//   crates/dclutch-product-runtime-v2-svm-reader/src/lib.rs
//     the route: read, never scraped. It is where the three schema IDs
//     actually key Registry records, so it is what the binding gate follows.
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
const ADMISSION_FILE = 'crates/dclutch-product-runtime-v2-admission/src/generated_admission_v2.rs';
const readCrateFile = (file) => readFileSync(new URL(file, root), 'utf8');
const sources = Object.freeze({
  admission: readCrateFile(ADMISSION_FILE),
  adapter: readCrateFile('programs/dclutch-product-runtime-v2-sbf/src/lib.rs'),
});

// The route-binding gate (see scripts/route-binding.mjs for the conviction it
// generalizes).
//
// The SBF program above is the FRAME -- it counts accounts and raises the
// refusal band, and it binds none of the three schema IDs emitted below; it
// delegates to `authenticate_product_runtime_v2`. So the file that would have
// been read as this generator's route proves nothing about these values. The
// actual binder is the SVM reader, where each ID keys a Registry record
// against a digest. That is the file this gate follows.
const ROUTE_FILE = 'crates/dclutch-product-runtime-v2-svm-reader/src/lib.rs';
const ROUTE_CRATE = 'dclutch_product_runtime_v2_svm_reader';
const routeText = readCrateFile(ROUTE_FILE);
for (const [constant, conjunct] of [
  ['PRODUCT_RECORD_SCHEMA_ID_V2', 'PRODUCT_RECORD_SCHEMA_ID_V2, expected_product_digest,'],
  ['RESULT_DOMAIN_SCHEMA_ID_V2', 'RESULT_DOMAIN_SCHEMA_ID_V2, product.result_domain_digest(),'],
  ['PORTFOLIO_SCHEMA_ID_V2', 'PORTFOLIO_SCHEMA_ID_V2, product.portfolio_digest(),'],
]) {
  requireGeneratorFollowsRoute({
    routeText,
    routeCrate: ROUTE_CRATE,
    readSource: readCrateFile,
    binding: { routeName: constant, conjunct, sourceFile: ADMISSION_FILE, sourceConstant: constant },
  });
}
const outputUrl = new URL('../lib/generated/productRuntimeV2Admission.ts', import.meta.url);

function scalar(source, name) {
  const match = sources[source].match(new RegExp(`(?:pub(?:\\(crate\\))? )?const ${name}: [^=]+ = ([0-9_]+);`));
  if (!match) throw new Error(`missing Rust scalar ${source}.${name}`);
  return Number(match[1].replaceAll('_', ''));
}

// A magic has TWO spellings and this reader knows both, which is the lesson
// d0c0990f paid for in the route census: a hand-written magic is `*b"DCLTPRM2"`
// and a Lean-EMITTED one is a hex array, because an emitter prints bytes and
// not text. A reader that knows only the first goes blind in exact proportion
// to how much of the tree is properly authored.
function magic(source, name) {
  const text = sources[source];
  const literal = text.match(new RegExp(`(?:pub(?:\\(crate\\))? )?const ${name}: \\[u8; 8\\] = \\*b"([A-Z0-9]{8})";`));
  if (literal) return literal[1];
  const emitted = text.match(new RegExp(`(?:pub(?:\\(crate\\))? )?const ${name}: \\[u8; 8\\] =\\s*\\[([^\\]]*)\\];`));
  if (!emitted) throw new Error(`missing Rust 8-byte magic ${source}.${name}`);
  const bytes = [...emitted[1].matchAll(/0x[0-9a-fA-F]{2}|\b\d+\b/g)].map((entry) => Number(entry[0]));
  if (bytes.length !== 8) {
    throw new Error(`Rust magic ${source}.${name} is ${bytes.length} bytes, not 8`);
  }
  return String.fromCharCode(...bytes);
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

// The reserved spans and the header coordinates used to be recovered by
// REGULAR EXPRESSION over the decoder's own call sites -- `require_zero(bytes,
// 10, 6)` and `array::<8>(bytes, 0)` -- because the crate had no constants for
// them. That is the shape `c131407b` named when the browser read a lifecycle
// magic's offset out of a function argument: a coordinate with no name can only
// be scraped, and a scraper reads whatever the argument happens to be.
//
// `DClutch.ProductAdmissionV2Abi` places them now, so both scrapers are gone
// and these are constant reads like every other value in this file. The two
// records share a reserved span; the receipt spends its first byte on a record
// count and reserves the remaining five, which is why there are two spans here
// and not three.
const bodyReserved = {
  offset: scalar('admission', 'ADMISSION_BODY_RESERVED_OFFSET_V2'),
  length: scalar('admission', 'ADMISSION_BODY_RESERVED_BYTES_V2'),
};
const requestReserved = bodyReserved;
const recordReserved = bodyReserved;
const receiptReserved = {
  offset: scalar('admission', 'ADMISSION_RECEIPT_RESERVED_OFFSET_V2'),
  length: scalar('admission', 'ADMISSION_RECEIPT_RESERVED_BYTES_V2'),
};
const header = {
  magicBytes: scalar('admission', 'ADMISSION_MAGIC_BYTES_V2'),
  magicOffset: scalar('admission', 'ADMISSION_BODY_MAGIC_OFFSET_V2'),
  versionOffset: scalar('admission', 'ADMISSION_BODY_VERSION_OFFSET_V2'),
};
// The receipt is a separate record with its own placements, so the claim that
// all three share one header is still CHECKED rather than assumed -- it just no
// longer needs a regex to find out.
const receiptHeader = {
  magicOffset: scalar('admission', 'ADMISSION_RECEIPT_MAGIC_OFFSET_V2'),
  versionOffset: scalar('admission', 'ADMISSION_RECEIPT_VERSION_OFFSET_V2'),
};
if (
  receiptHeader.magicOffset !== header.magicOffset ||
  receiptHeader.versionOffset !== header.versionOffset
) {
  throw new Error('the three Product Runtime V2 records no longer share one header layout; emit them separately');
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
  // The request and the Product record are ONE SHAPE, which the crate used to
  // express by declaring these three coordinates twice under two names. The
  // browser keeps both TypeScript names because both are part of its published
  // surface, but they now come from one source constant each -- so the two
  // records cannot drift apart here either.
  ['REQUEST_PRODUCT_DIGEST_OFFSET_V2', 'PRODUCT_ID_OFFSET'],
  ['REQUEST_DOMAIN_DIGEST_OFFSET_V2', 'PRODUCT_DOMAIN_DIGEST_OFFSET'],
  ['REQUEST_PORTFOLIO_DIGEST_OFFSET_V2', 'PRODUCT_PORTFOLIO_DIGEST_OFFSET'],
  ['PRODUCT_ID_OFFSET_V2', 'PRODUCT_ID_OFFSET'],
  ['PRODUCT_DOMAIN_DIGEST_OFFSET_V2', 'PRODUCT_DOMAIN_DIGEST_OFFSET'],
  ['PRODUCT_PORTFOLIO_DIGEST_OFFSET_V2', 'PRODUCT_PORTFOLIO_DIGEST_OFFSET'],
  ['RECEIPT_COUNT_OFFSET_V2', 'ADMISSION_RECEIPT_COUNT_OFFSET_V2'],
  ['RECEIPT_RECORDS_OFFSET_V2', 'ADMISSION_RECEIPT_RECORDS_OFFSET_V2'],
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
