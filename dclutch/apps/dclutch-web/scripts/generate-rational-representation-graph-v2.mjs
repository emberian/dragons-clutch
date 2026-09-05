/**
 * `RepresentationGraphV2`'s byte coordinates, for the clients that read one.
 *
 * The Rational capability route reads a finalized representation graph on the
 * way to every Open and Terminal transaction: it is the record that proves the
 * descriptor's payoff row is the one the graph root actually carries. The
 * browser decoded it through fourteen literal coordinates of its own -- the
 * magic, the version, two reserved runs, three counts, the scale, the node and
 * edge strides -- and through a hand-copied 32-byte schema release id, which
 * together made `lib/rationalCapabilityChainV4.ts` a second authority for a
 * record whose first authority is a kernel it never mentions.
 *
 * AUTHORITY: hand-written Rust, NOT Lean. `RepresentationGraphV2` has no Lean
 * schema in `formal/`; the graph header, the node and edge strides and every
 * offset below are declared in
 * `crates/dclutch-claims/src/rational_kernel/mod.rs` and nowhere
 * else. That crate is the semantic owner and this module tracks it. Its
 * sibling record, the descriptor, IS Lean-emitted
 * (`EmitRationalRepresentationV2DescriptorRust.lean` ->
 * `src/generated_descriptor.rs`), so the graph is the layout still owed a
 * schema; when one lands, this generator should read the emitted Rust instead
 * and the change here is one path.
 *
 * The magic and version offsets are not declared as constants in the kernel --
 * they are literals at the decode site, inside `exact_magic` and
 * `exact_version`. They are scraped from those two functions rather than
 * written down here, so a header convention that moves takes this generator
 * red instead of leaving the browser reading the old one.
 */
import { readFileSync, writeFileSync } from 'node:fs';

const root = new URL('../../../', import.meta.url);
const sourcePath = new URL('crates/dclutch-claims/src/rational_kernel/mod.rs', root);
const source = readFileSync(sourcePath, 'utf8');
const outputUrl = new URL('../lib/generated/rationalRepresentationGraphV2.ts', import.meta.url);

function scalar(name) {
  const match = source.match(new RegExp(`(?:pub(?:\\(crate\\))? )?const ${name}: (?:usize|u8|u16|u32|u64) = ([0-9_]+);`));
  if (!match) throw new Error(`missing Rust scalar ${name}`);
  return match[1].replaceAll('_', '');
}

/** A `*b"…"` literal magic, with its declared width proved against the text. */
function asciiMagic(name) {
  const match = source.match(new RegExp(`const ${name}: \\[u8; ([0-9]+)\\] = \\*b"([\\x20-\\x7e]*)";`));
  if (!match) throw new Error(`missing Rust ASCII magic ${name}`);
  if (match[2].length !== Number(match[1])) throw new Error(`${name} is ${match[2].length} bytes, declared ${match[1]}`);
  return { text: match[2], bytes: [...match[2]].map((character) => character.charCodeAt(0)) };
}

function fixedBytes(name) {
  const match = source.match(new RegExp(`const ${name}: \\[u8; ([0-9]+)\\] = \\[([\\s\\S]*?)\\n\\];`));
  if (!match) throw new Error(`missing Rust byte array ${name}`);
  const bytes = [...match[2].matchAll(/0x([0-9a-f]{2})/g)].map((entry) => Number.parseInt(entry[1], 16));
  if (bytes.length !== Number(match[1])) throw new Error(`${name} has ${bytes.length} bytes, expected ${match[1]}`);
  return bytes;
}

function byteString(name) {
  const match = source.match(new RegExp(`const ${name}: &\\[u8\\] =\\s*b"([\\x20-\\x7e]*)";`));
  if (!match) throw new Error(`missing Rust byte string ${name}`);
  return match[1];
}

function list(bytes) { return `Uint8Array.from([${bytes.map((byte) => `0x${byte.toString(16).padStart(2, '0')}`).join(', ')}])`; }

/** One coordinate read out of a named Rust function body rather than declared. */
function siteInFunction(functionName, pattern, label) {
  const body = source.match(new RegExp(`fn ${functionName}\\([\\s\\S]*?\\n\\}`));
  if (!body) throw new Error(`missing Rust fn ${functionName}`);
  const match = body[0].match(pattern);
  if (!match) throw new Error(`fn ${functionName} no longer reads ${label} where this scrape expects it`);
  return match[1];
}

/**
 * Every offset the kernel declares for this record, in declaration order.
 *
 * Scraped by prefix rather than listed, so a coordinate added to the graph,
 * node or edge layout arrives here by regeneration instead of by somebody
 * remembering a name -- and so this script cannot silently emit a SUBSET of
 * the layout, which is the shape a hand-kept list drifts into.
 */
function offsetNames(prefix, minimum) {
  const found = [...source.matchAll(new RegExp(`^(?:pub )?const (${prefix}[A-Z0-9_]*_OFFSET): usize = [0-9_]+;`, 'gm'))]
    .map((match) => match[1]);
  if (found.length < minimum) throw new Error(`only ${found.length} ${prefix}* offsets found; the scrape is wrong`);
  return found;
}

const magic = asciiMagic('GRAPH_MAGIC_V2');
const magicOffset = siteInFunction('exact_magic', /array_at::<8>\(input, ([0-9]+)\)/, 'the magic');
const versionOffset = siteInFunction('exact_version', /u16_at\(input, ([0-9]+)\)/, 'the schema version');
const decode = source.match(/pub fn decode\(input: &'a \[u8\], admission: ContentAdmissionV2\)[\s\S]*?\n {4}\}/);
if (!decode) throw new Error('missing RepresentationGraphV2::decode');
const reservedHeader = (() => {
  const match = decode[0].match(/require_zero\(input, ([0-9]+), ([0-9]+)\)/);
  if (!match) throw new Error('RepresentationGraphV2::decode no longer pins a fixed reserved header run');
  return { offset: match[1], bytes: match[2] };
})();
const reservedBody = (() => {
  const match = decode[0].match(/require_zero\(input, GRAPH_RESERVED_OFFSET, ([0-9]+)\)/);
  if (!match) throw new Error('RepresentationGraphV2::decode no longer pins a fixed reserved body run');
  return match[1];
})();

const lines = [
  '// @generated by scripts/generate-rational-representation-graph-v2.mjs; do not edit.',
  '// Source: crates/dclutch-claims/src/rational_kernel/mod.rs.',
  '//',
  '// AUTHORITY IS HAND-WRITTEN RUST, NOT LEAN. RepresentationGraphV2 has no Lean',
  '// schema; the kernel crate above declares this layout and is its only owner. The',
  '// descriptor beside it IS Lean-emitted, so this record is the one still owed a',
  '// schema -- see generated/rationalRepresentationDescriptorV3.ts for the contrast.',
  '',
  `export const GRAPH_MAGIC_V2 = ${list(magic.bytes)};`,
  `export const GRAPH_MAGIC_BYTES = ${magic.bytes.length} as const;`,
  `export const GRAPH_MAGIC_OFFSET = ${magicOffset} as const;`,
  `export const GRAPH_VERSION_OFFSET = ${versionOffset} as const;`,
  `export const GRAPH_SCHEMA_VERSION_V2 = ${scalar('SCHEMA_VERSION_V2')} as const;`,
  `export const GRAPH_RESERVED_HEADER_OFFSET = ${reservedHeader.offset} as const;`,
  `export const GRAPH_RESERVED_HEADER_BYTES = ${reservedHeader.bytes} as const;`,
  `export const GRAPH_RESERVED_BYTES = ${reservedBody} as const;`,
  `export const GRAPH_HEADER_BYTES = ${scalar('GRAPH_HEADER_BYTES')} as const;`,
  `export const GRAPH_NODE_BYTES = ${scalar('GRAPH_NODE_BYTES')} as const;`,
  `export const GRAPH_EDGE_BYTES = ${scalar('GRAPH_EDGE_BYTES')} as const;`,
  `export const SCALAR_BYTES = ${scalar('SCALAR_BYTES')} as const;`,
  '',
  '/** The graph header, then one fixed node record, then one fixed edge record. */',
  ...offsetNames('GRAPH_', 7).map((name) => `export const ${name} = ${scalar(name)} as const;`),
  ...offsetNames('NODE_', 8).map((name) => `export const ${name} = ${scalar(name)} as const;`),
  ...offsetNames('EDGE_', 4).map((name) => `export const ${name} = ${scalar(name)} as const;`),
  '',
  '/** The finalized-record schema this graph is published under. */',
  `export const REPRESENTATION_GRAPH_SCHEMA_RELEASE_PREIMAGE_V2 = '${byteString('REPRESENTATION_GRAPH_SCHEMA_RELEASE_PREIMAGE_V2')}' as const;`,
  `export const REPRESENTATION_GRAPH_SCHEMA_RELEASE_ID_V2 = ${list(fixedBytes('REPRESENTATION_GRAPH_SCHEMA_RELEASE_ID_V2'))};`,
  '',
].join('\n');

if (process.argv.includes('--check')) {
  if (readFileSync(outputUrl, 'utf8') !== lines) {
    process.stderr.write('lib/generated/rationalRepresentationGraphV2.ts is stale; run npm run abi:rational-graph-v2\n');
    process.exit(1);
  }
  process.stdout.write('lib/generated/rationalRepresentationGraphV2.ts matches its Rust source\n');
} else {
  writeFileSync(outputUrl, lines);
  process.stdout.write('wrote lib/generated/rationalRepresentationGraphV2.ts\n');
}
