/**
 * `LifecycleRequestV2`'s byte coordinates, its action tags, and its frame widths.
 *
 * This is the record the browser WRITES rather than reads, which is the more
 * expensive half to state by hand: a decoder at the wrong offset shows the
 * wrong number, an ENCODER at the wrong offset produces a request the chain
 * refuses, or worse accepts against fields the wallet did not mean.
 * `lib/rationalRetireReceiptV4.ts` built both the compact `DCRLHC04` family
 * request and the specialized `DCRRLC02` child from nineteen header
 * coordinates, eight row coordinates, two magics, two versions, an action tag,
 * a 272-byte row stride, a 400-byte header width, a sentinel revision, a
 * 32-byte request-schema id, and a per-row Claims account count -- none of
 * which it had any authority for.
 *
 * That last one had already drifted. `e78fa027d` gave the compact vacancy row
 * its custody-owner account, taking the Rust count from four to five; the
 * client stayed at four, so it built a `20 + 4 x support` Claims frame for a
 * program reading `20 + 5 x support`, and nothing in either tree was able to
 * notice. The count is emitted here for that reason.
 *
 * AUTHORITY: hand-written Rust, NOT Lean. `LifecycleRequestV2` has no Lean
 * schema in `formal/`;
 * `crates/dclutch-rational-representation-v2-lifecycle-contract` declares the
 * header, the coordinate row and the account counts in `src/lib.rs`, and the
 * compact family that specializes into it in `src/compact_hot_v4.rs`.
 *
 * The completion receipt (`DCRRLR02`) is a separate record in the same crate
 * and is deliberately NOT mirrored here: no client decodes one, and a layout
 * emitted for nobody is a surface to keep in step for nothing.
 */
import { readFileSync, writeFileSync } from 'node:fs';

const root = new URL('../../../', import.meta.url);
const crate = new URL('crates/dclutch-rational-representation-v2-lifecycle-contract/src/', root);
const contract = readFileSync(new URL('lib.rs', crate), 'utf8');
const compact = readFileSync(new URL('compact_hot_v4.rs', crate), 'utf8');
const outputUrl = new URL('../lib/generated/rationalLifecycleRequestV2.ts', import.meta.url);

function scalar(text, name) {
  const match = text.match(new RegExp(`(?:pub(?:\\(crate\\))? )?const ${name}: (?:usize|u8|u16|u32|u64) = ([0-9_]+);`));
  if (!match) throw new Error(`missing Rust scalar ${name}`);
  return match[1].replaceAll('_', '');
}

/** A sentinel declared as `T::MAX` rather than as a numeral. */
function sentinel(text, name) {
  const match = text.match(new RegExp(`const ${name}: (u32|u64) = (u32|u64)::MAX;`));
  if (!match) throw new Error(`missing Rust sentinel ${name}`);
  if (match[1] !== match[2]) throw new Error(`${name} is declared ${match[1]} but bounded by ${match[2]}`);
  return match[1] === 'u32' ? '4294967295' : '18446744073709551615n';
}

/** A `*b"…"` literal magic, with its declared width proved against the text. */
function asciiMagic(text, name) {
  const match = text.match(new RegExp(`const ${name}: \\[u8; ([0-9]+)\\] = \\*b"([\\x20-\\x7e]*)";`));
  if (!match) throw new Error(`missing Rust ASCII magic ${name}`);
  if (match[2].length !== Number(match[1])) throw new Error(`${name} is ${match[2].length} bytes, declared ${match[1]}`);
  return { text: match[2], bytes: [...match[2]].map((character) => character.charCodeAt(0)) };
}

function fixedBytes(text, name) {
  const match = text.match(new RegExp(`const ${name}: \\[u8; ([0-9]+)\\] = \\[([\\s\\S]*?)\\n\\];`));
  if (!match) throw new Error(`missing Rust byte array ${name}`);
  const bytes = [...match[2].matchAll(/0x([0-9a-f]{2})/g)].map((entry) => Number.parseInt(entry[1], 16));
  if (bytes.length !== Number(match[1])) throw new Error(`${name} has ${bytes.length} bytes, expected ${match[1]}`);
  return bytes;
}

function byteString(text, name) {
  const match = text.match(new RegExp(`const ${name}: &\\[u8\\] =\\s*b"([\\x20-\\x7e]*)";`));
  if (!match) throw new Error(`missing Rust byte string ${name}`);
  return match[1];
}

function list(bytes) { return `Uint8Array.from([${bytes.map((byte) => `0x${byte.toString(16).padStart(2, '0')}`).join(', ')}])`; }

function body(text, pattern, label) {
  const match = text.match(pattern);
  if (!match) throw new Error(`missing Rust ${label}`);
  return match[0];
}

function pinned(region, name, label) {
  const match = region.match(new RegExp(`require_zero\\(input, ${name}, ([0-9]+)\\)`));
  if (!match) throw new Error(`${label} no longer pins a fixed reserved run at ${name}`);
  return match[1];
}

const header = body(contract, /pub fn decode\(input: &\[u8\]\) -> Result<Self> \{\n {8}if input\.len\(\) < LIFECYCLE_HEADER_BYTES_V2[\s\S]*?\n {4}\}/, 'LifecycleHeaderV2::decode');
const magicSite = header.match(/exact\(input, ([0-9]+), &LIFECYCLE_REQUEST_MAGIC_V2\)/);
if (!magicSite) throw new Error('LifecycleHeaderV2::decode no longer reads its magic where this scrape expects it');
const versionSite = header.match(/read_u16\(input, ([0-9]+)\)\? != LIFECYCLE_VERSION_V2/);
if (!versionSite) throw new Error('LifecycleHeaderV2::decode no longer reads its version where this scrape expects it');
const row = body(contract, /pub fn decode\(input: &\[u8\]\) -> Result<Self> \{\n {8}if input\.len\(\) != LIFECYCLE_COORDINATE_BYTES_V2[\s\S]*?\n {4}\}/, 'LifecycleCoordinateV2::decode');

/**
 * Every offset the contract declares, partitioned by the record part it
 * addresses, in declaration order.
 *
 * Scraped rather than listed, so a coordinate added to the header or to the
 * coordinate row arrives here by regeneration instead of by somebody
 * remembering a name -- and so this script cannot silently emit a SUBSET of
 * the layout the browser then encodes against.
 *
 * The three blocks are split BY POSITION, not by name prefix. Two of the
 * header's own fields are called `RECEIPT_MINT_OFFSET` and
 * `RECEIPT_RENT_PRINCIPAL_OFFSET` -- they name the receipt Mint the request
 * acts on, not the completion receipt record -- so a prefix filter silently
 * dropped both and emitted a header short two coordinates, one of them a
 * 32-byte identity. The contract declares the three runs contiguously: header,
 * then the coordinate row, then the completion receipt.
 */
const declared = [...contract.matchAll(/^const ([A-Z0-9_]+_OFFSET): usize = [0-9_]+;/gm)];
const firstRow = declared.findIndex((match) => match[1].startsWith('ROW_'));
const lastRow = declared.findLastIndex((match) => match[1].startsWith('ROW_'));
if (firstRow < 1 || lastRow < firstRow) throw new Error('the lifecycle offset blocks are no longer header, then row, then receipt');
const headerOffsets = declared.slice(0, firstRow).map((match) => match[1]);
const rowOffsets = declared.slice(firstRow, lastRow + 1).map((match) => match[1]);
if (headerOffsets.length < 21) throw new Error(`only ${headerOffsets.length} lifecycle header offsets found; the scrape is wrong`);
if (rowOffsets.length < 20) throw new Error(`only ${rowOffsets.length} lifecycle row offsets found; the scrape is wrong`);
if (rowOffsets.some((name) => !name.startsWith('ROW_'))) throw new Error('the coordinate-row offset block is no longer contiguous');

const actions = [...body(contract, /pub enum LifecycleActionV2 \{[\s\S]*?\n\}/, 'LifecycleActionV2')
  .matchAll(/^ {4}([A-Za-z0-9]+) = ([0-9]+),$/gm)];
if (actions.length < 4) throw new Error(`only ${actions.length} lifecycle actions found; the scrape is wrong`);

function screamingSnake(name) { return name.replace(/([a-z0-9])([A-Z])/g, '$1_$2').toUpperCase(); }

const compactWidth = compact.match(/const RATIONAL_LIFECYCLE_COMPACT_HOT_REQUEST_BYTES_V4: usize = ([A-Z0-9_]+);/);
if (!compactWidth || compactWidth[1] !== 'LIFECYCLE_HEADER_BYTES_V2') {
  throw new Error('the compact family request is no longer exactly the lifecycle header width');
}

const requestMagic = asciiMagic(contract, 'LIFECYCLE_REQUEST_MAGIC_V2');
const compactMagic = asciiMagic(compact, 'RATIONAL_LIFECYCLE_COMPACT_HOT_MAGIC_V4');
const lines = [
  '// @generated by scripts/generate-rational-lifecycle-request-v2.mjs; do not edit.',
  '// Source: crates/dclutch-rational-representation-v2-lifecycle-contract/src/lib.rs',
  '// (request header, coordinate row, account counts) and src/compact_hot_v4.rs',
  '// (the compact family that specializes into one such child).',
  '//',
  '// AUTHORITY IS HAND-WRITTEN RUST, NOT LEAN. LifecycleRequestV2 has no Lean schema;',
  '// the lifecycle contract crate above declares this record and is its only owner.',
  '',
  `export const LIFECYCLE_REQUEST_MAGIC_V2 = ${list(requestMagic.bytes)};`,
  `export const LIFECYCLE_MAGIC_BYTES = ${requestMagic.bytes.length} as const;`,
  `export const LIFECYCLE_MAGIC_OFFSET = ${magicSite[1]} as const;`,
  `export const LIFECYCLE_VERSION_OFFSET = ${versionSite[1]} as const;`,
  `export const LIFECYCLE_VERSION_V2 = ${scalar(contract, 'LIFECYCLE_VERSION_V2')} as const;`,
  `export const LIFECYCLE_HEADER_BYTES_V2 = ${scalar(contract, 'LIFECYCLE_HEADER_BYTES_V2')} as const;`,
  `export const LIFECYCLE_COORDINATE_BYTES_V2 = ${scalar(contract, 'LIFECYCLE_COORDINATE_BYTES_V2')} as const;`,
  `export const LIFECYCLE_HEADER_RESERVED_BYTES = ${pinned(header, 'HEADER_RESERVED_OFFSET', 'LifecycleHeaderV2::decode')} as const;`,
  `export const LIFECYCLE_ROW_RESERVED_HEAD_BYTES = ${pinned(row, 'ROW_RESERVED_HEAD_OFFSET', 'LifecycleCoordinateV2::decode')} as const;`,
  `export const LIFECYCLE_ROW_RESERVED_TAIL_BYTES = ${pinned(row, 'ROW_RESERVED_TAIL_OFFSET', 'LifecycleCoordinateV2::decode')} as const;`,
  '',
  '/**',
  ' * The Claims account frame, per action.',
  ' *',
  ' * `LIFECYCLE_VACANCY_ACCOUNT_COUNT_V2` is the count a compact RetireReceipt adds',
  ' * per proven-vacant support row, in the same physical order the coordinate actions',
  ' * use: shard Mint, Structured custody, Claims custody owner, Position, admission.',
  ' */',
  `export const LIFECYCLE_COMMON_ACCOUNT_COUNT_V2 = ${scalar(contract, 'LIFECYCLE_COMMON_ACCOUNT_COUNT_V2')} as const;`,
  `export const LIFECYCLE_COORDINATE_ACCOUNT_COUNT_V2 = ${scalar(contract, 'LIFECYCLE_COORDINATE_ACCOUNT_COUNT_V2')} as const;`,
  `export const LIFECYCLE_VACANCY_ACCOUNT_COUNT_V2 = ${scalar(contract, 'LIFECYCLE_VACANCY_ACCOUNT_COUNT_V2')} as const;`,
  '',
  '/** Sentinels only receipt-wide actions and proven-vacant rows may carry. */',
  `export const LIFECYCLE_ABSENT_OUTCOME_V2 = ${sentinel(contract, 'ABSENT_OUTCOME_V2')} as const;`,
  `export const LIFECYCLE_ABSENT_POSITION_REVISION_V2 = ${sentinel(contract, 'ABSENT_POSITION_REVISION_V2')} as const;`,
  '',
  '/** Granular lifecycle action tags. */',
  ...actions.map(([, name, tag]) => `export const LIFECYCLE_ACTION_${screamingSnake(name)}_V2 = ${tag} as const;`),
  '',
  '/** The fixed request header. */',
  ...headerOffsets.map((name) => `export const LIFECYCLE_${name} = ${scalar(contract, name)} as const;`),
  '',
  '/** One physical nonzero-support coordinate row, at the header width plus a stride. */',
  ...rowOffsets.map((name) => `export const LIFECYCLE_${name} = ${scalar(contract, name)} as const;`),
  '',
  '/** The compact family request, specialized into exactly one Claims child. */',
  `export const RATIONAL_LIFECYCLE_COMPACT_HOT_MAGIC_V4 = ${list(compactMagic.bytes)};`,
  `export const RATIONAL_LIFECYCLE_COMPACT_HOT_VERSION_V4 = ${scalar(compact, 'RATIONAL_LIFECYCLE_COMPACT_HOT_VERSION_V4')} as const;`,
  `export const RATIONAL_LIFECYCLE_COMPACT_HOT_REQUEST_BYTES_V4 = ${scalar(contract, compactWidth[1])} as const;`,
  `export const RATIONAL_LIFECYCLE_COMPACT_HOT_SCHEMA_PREIMAGE_V4 = '${byteString(compact, 'RATIONAL_LIFECYCLE_COMPACT_HOT_SCHEMA_PREIMAGE_V4')}' as const;`,
  `export const RATIONAL_LIFECYCLE_COMPACT_HOT_SCHEMA_RELEASE_ID_V4 = ${list(fixedBytes(compact, 'RATIONAL_LIFECYCLE_COMPACT_HOT_SCHEMA_RELEASE_ID_V4'))};`,
  '',
  '/** The capability-kind identity a Rational manifest entry carries. */',
  `export const RATIONAL_LIFECYCLE_CAPABILITY_KIND_PREIMAGE_V1 = '${byteString(contract, 'RATIONAL_LIFECYCLE_CAPABILITY_KIND_PREIMAGE_V1')}' as const;`,
  `export const RATIONAL_LIFECYCLE_CAPABILITY_KIND_ID_V1 = ${list(fixedBytes(contract, 'RATIONAL_LIFECYCLE_CAPABILITY_KIND_ID_V1'))};`,
  '',
].join('\n');

if (process.argv.includes('--check')) {
  if (readFileSync(outputUrl, 'utf8') !== lines) {
    process.stderr.write('lib/generated/rationalLifecycleRequestV2.ts is stale; run npm run abi:rational-lifecycle-v2\n');
    process.exit(1);
  }
  process.stdout.write('lib/generated/rationalLifecycleRequestV2.ts matches its Rust source\n');
} else {
  writeFileSync(outputUrl, lines);
  process.stdout.write('wrote lib/generated/rationalLifecycleRequestV2.ts\n');
}
