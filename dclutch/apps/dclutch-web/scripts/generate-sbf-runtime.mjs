/**
 * The SBF virtual machine's own vocabulary, mirrored for the explorer.
 *
 * A dClutch refusal is a `Custom` code with a band, a name and a meaning, and
 * `lib/explorer/refusals.ts` renders it from `refusalRegistryV1`. An ABORT is
 * the other half of the same reader's question and has never had an authority:
 * when the VM faults, the transaction carries NO custom code at all, the RPC
 * reports `InstructionError(n, ProgramFailedToComplete)`, and the sentence that
 * says what actually happened — `Access violation writing 8 bytes at address
 * 0x30000fcf8` — sits in `logMessages` as prose nobody parses.
 *
 * That sentence is not prose. It is `EbpfError`'s or `SyscallError`'s
 * `#[error(...)]` format string, from crates this workspace's `Cargo.lock`
 * pins, and the address is a coordinate in a memory map those same crates
 * declare. So the browser can name an abort exactly as precisely as it names a
 * refusal — provided it reads the vocabulary rather than writing it down.
 *
 * Four sources, each already an authority for its part:
 *
 *   - `solana-sbpf/src/ebpf.rs` — the memory map. `MM_HEAP_START` and friends
 *     are what an address is classified against; nothing here assumes a stride
 *     or a base.
 *   - `solana-sbpf/src/error.rs` — every `EbpfError` variant's Display format.
 *   - `solana-syscalls/src/lib.rs` — every `SyscallError` variant's Display
 *     format, which is what `SBF program panicked` actually is.
 *   - `solana-program-entrypoint/src/lib.rs` — `HEAP_START_ADDRESS` and
 *     `HEAP_LENGTH`, the heap a transaction gets without asking, plus
 *     `crates/dclutch-sbf-bump-heap/src/lib.rs` for the ceiling and
 *     granularity of a `RequestHeapFrame`. The allocator owns those two, and
 *     `entrypoint_adapter.rs` re-exports them under its own long-standing
 *     names; reading the adapter got a name, not a number.
 *
 * The first three are read at the version `Cargo.lock` pins, out of the local
 * cargo registry, so the emitted vocabulary is the vocabulary of the runtime
 * this tree builds against and not of whatever was current when someone typed
 * it. The pinned versions are emitted alongside, because a reader deserves to
 * know which runtime's words they are being shown.
 *
 * VALUE TEST, not a formality: `HEAP_START_ADDRESS` (the program's view) and
 * `MM_HEAP_START` (the VM's view) are declared in two unrelated crates and must
 * agree. They are the two ends of the same fact and this generator refuses if
 * they ever disagree.
 */
import { existsSync, readFileSync, readdirSync, renameSync, rmSync, writeFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = new URL('../../../', import.meta.url);
const outputUrl = new URL('../lib/generated/sbfRuntimeV1.ts', import.meta.url);
const lockSource = readFileSync(new URL('Cargo.lock', root), 'utf8');
const heapSource = readFileSync(new URL('crates/dclutch-sbf-bump-heap/src/lib.rs', root), 'utf8');
const adapterSource = readFileSync(
  new URL('programs/dclutch-trading-sbf/src/entrypoint_adapter.rs', root),
  'utf8',
);

// ------------------------------------------------------ the pinned registry

/**
 * The version `Cargo.lock` pins for one crate.
 *
 * Refuses on two entries rather than picking one: a workspace carrying two
 * copies of the runtime would make "the vocabulary" ambiguous, and guessing
 * which is the real one is exactly the kind of quiet choice this module exists
 * to remove.
 */
function lockedVersion(name) {
  const versions = [...lockSource.matchAll(
    new RegExp(`\\[\\[package\\]\\]\\nname = "${name}"\\nversion = "([^"]+)"`, 'g'),
  )].map((match) => match[1]);
  if (versions.length === 0) throw new Error(`Cargo.lock pins no ${name}`);
  if (versions.length > 1) throw new Error(`Cargo.lock pins ${versions.length} copies of ${name}: ${versions.join(', ')}`);
  return versions[0];
}

/**
 * Where cargo unpacked one pinned crate.
 *
 * The registry index directory carries a hash that changes with the protocol
 * cargo speaks, so it is searched rather than named. A miss is a loud refusal
 * with the fix in it: this generator reads sources, and an absent source is not
 * a reason to emit anything.
 */
function crateSource(name, relative) {
  const version = lockedVersion(name);
  const registry = join(process.env.CARGO_HOME ?? join(homedir(), '.cargo'), 'registry', 'src');
  if (!existsSync(registry)) throw new Error(`no cargo registry at ${registry}; run \`cargo fetch\` at the workspace root`);
  const hits = readdirSync(registry)
    .map((index) => join(registry, index, `${name}-${version}`))
    .filter((path) => existsSync(path));
  if (hits.length === 0) {
    throw new Error(`${name} ${version} is pinned but not unpacked under ${registry}; run \`cargo fetch\` at the workspace root`);
  }
  return { version, text: readFileSync(join(hits[0], relative), 'utf8') };
}

const sbpfEbpf = crateSource('solana-sbpf', 'src/ebpf.rs');
const sbpfError = crateSource('solana-sbpf', 'src/error.rs');
const syscalls = crateSource('solana-syscalls', 'src/lib.rs');
const entrypoint = crateSource('solana-program-entrypoint', 'src/lib.rs');

// ------------------------------------------------------------ Rust constants

/**
 * Evaluate one integer `const` from a Rust source.
 *
 * The memory map is written as relationships, not literals — `MM_HEAP_START`
 * is `MM_REGION_SIZE * 3` and `MM_REGION_SIZE` is `1 << VIRTUAL_ADDRESS_BITS`.
 * Transcribing the resolved numbers would erase exactly the structure that
 * makes them checkable, so this evaluates the same expressions Rust does, over
 * the same file. The grammar is shifts, products and sums of literals and other
 * named constants; anything else refuses rather than guessing.
 */
function scalar(source, name, seen = new Set()) {
  if (seen.has(name)) throw new Error(`cyclic Rust const ${name}`);
  seen.add(name);
  const match = source.match(new RegExp(`(?:pub )?const ${name}: (?:usize|u8|u16|u32|u64|i64) = ([^;]+);`));
  if (!match) throw new Error(`missing Rust scalar ${name}`);
  const expression = match[1].trim();
  const shift = expression.split('<<');
  if (shift.length === 2) return term(source, shift[0], seen) * 2 ** term(source, shift[1], seen);
  if (shift.length > 2) throw new Error(`unparsable Rust const ${name}: ${expression}`);
  return expression.split('+').reduce((total, part) => total + term(source, part, seen), 0);
}

function term(source, text, seen) {
  return text.split('*').reduce((product, factor) => {
    const token = factor.trim();
    if (/^0x[0-9A-Fa-f_]+$/.test(token)) return product * Number(token.replaceAll('_', ''));
    if (/^[0-9_]+$/.test(token)) return product * Number(token.replaceAll('_', ''));
    if (/^[A-Z][A-Z0-9_]*$/.test(token)) return product * scalar(source, token, new Set(seen));
    throw new Error(`unparsable Rust term: ${token}`);
  }, 1);
}

// ------------------------------------------------------------- the memory map

/**
 * Every `MM_<NAME>_START` the VM declares, in address order.
 *
 * Discovered rather than listed: a runtime that grows a region grows this
 * table, and an address that lands in it is classified by a name the VM chose.
 */
const regions = [...sbpfEbpf.text.matchAll(/pub const MM_([A-Z0-9_]+)_START: u64 = /g)]
  .map((match) => ({ name: match[1].toLowerCase(), start: scalar(sbpfEbpf.text, `MM_${match[1]}_START`) }))
  .sort((left, right) => left.start - right.start);
if (regions.length < 4) throw new Error(`found ${regions.length} SBF memory regions; the map moved`);
const regionSize = scalar(sbpfEbpf.text, 'MM_REGION_SIZE');
for (const region of regions) {
  if (region.start % regionSize !== 0) throw new Error(`region ${region.name} is not aligned to MM_REGION_SIZE`);
}

const heapRegion = regions.find((region) => region.name === 'heap');
if (heapRegion === undefined) throw new Error('the SBF memory map declares no heap region');

const heapStart = scalar(entrypoint.text, 'HEAP_START_ADDRESS');
const heapLength = scalar(entrypoint.text, 'HEAP_LENGTH');
// The value test. Two crates, no shared definition, one fact.
if (heapStart !== heapRegion.start) {
  throw new Error(`solana-program-entrypoint HEAP_START_ADDRESS ${heapStart} disagrees with solana-sbpf MM_HEAP_START ${heapRegion.start}`);
}

// The ceiling and the granularity, from the allocator that declares them. The
// adapter still has to be READING them -- a re-export that quietly became a
// second literal is exactly the drift this whole module exists against.
for (const [adapterName, allocatorName] of [
  ['ADAPTER_MAX_HEAP_BYTES', 'MAX_HEAP_BYTES_V1'],
  ['HEAP_FRAME_GRANULARITY_BYTES', 'HEAP_FRAME_GRANULARITY_BYTES_V1'],
  ['ADAPTER_DEFAULT_HEAP_BYTES', 'DEFAULT_HEAP_BYTES_V1'],
]) {
  if (!adapterSource.includes(`const ${adapterName}: usize = dclutch_sbf_bump_heap::${allocatorName};`)) {
    throw new Error(`the Trading adapter no longer takes ${adapterName} from the allocator's ${allocatorName}`);
  }
}
const maxHeap = scalar(heapSource, 'MAX_HEAP_BYTES_V1');
const granularity = scalar(heapSource, 'HEAP_FRAME_GRANULARITY_BYTES_V1');
if (maxHeap <= heapLength) throw new Error('the heap ceiling is not above the default heap');
if (maxHeap % granularity !== 0 || heapLength % granularity !== 0) {
  throw new Error('a heap bound is not a multiple of the request granularity');
}

// ---------------------------------------------------------- Display formats

/**
 * Every `#[error("…")]` format string in one enum, paired with its variant.
 *
 * `thiserror` writes the Display implementation from these, so they are the
 * exact sentences a validator log carries. Rust splits long ones across lines
 * with a trailing backslash, which eats the newline and the indentation that
 * follows it; joining without that rule silently invents whitespace the runtime
 * never prints.
 */
function displayFormats(source, enumName) {
  const body = source.match(new RegExp(`pub enum ${enumName} \\{\\n([\\s\\S]*?)\\n\\}`));
  if (!body) throw new Error(`missing Rust enum ${enumName}`);
  const found = [];
  const pattern = /#\[error\(([\s\S]*?)\)\]\s*(?:\/\/[^\n]*\n\s*)*([A-Za-z0-9_]+)/g;
  for (const match of body[1].matchAll(pattern)) {
    const literals = [...match[1].matchAll(/"((?:[^"\\]|\\[\s\S])*)"/g)].map((piece) => piece[1]);
    if (literals.length === 0) throw new Error(`${enumName}::${match[2]} carries no format string`);
    const format = literals.join('')
      .replace(/\\\n\s*/g, '')
      .replace(/\\"/g, '"')
      .replace(/\\\\/g, '\\');
    found.push({ variant: match[2], format });
  }
  if (found.length === 0) throw new Error(`parsed no #[error] formats out of ${enumName}`);
  return found;
}

const ebpfFormats = displayFormats(sbpfError.text, 'EbpfError');
const syscallFormats = displayFormats(syscalls.text, 'SyscallError');
for (const required of ['AccessViolation', 'StackAccessViolation', 'ExceededMaxInstructions', 'CallDepthExceeded']) {
  if (!ebpfFormats.some((entry) => entry.variant === required)) throw new Error(`EbpfError lost ${required}`);
}
if (!syscallFormats.some((entry) => entry.variant === 'Abort')) throw new Error('SyscallError lost Abort');

// -------------------------------------------------------------------- output

const ts = (value) => JSON.stringify(value);
const lines = [];
lines.push('// @generated from the pinned solana-sbpf, solana-syscalls and solana-program-entrypoint');
lines.push('// crates plus crates/dclutch-sbf-bump-heap/src/lib.rs; do not edit.');
lines.push('// Regenerate with: npm run abi:sbf-runtime');
lines.push('');
lines.push('/** The runtime crate versions this vocabulary was read from. */');
lines.push('export const SBF_RUNTIME_VERSIONS_V1 = Object.freeze({');
lines.push(`  sbpf: ${ts(sbpfEbpf.version)},`);
lines.push(`  syscalls: ${ts(syscalls.version)},`);
lines.push(`  entrypoint: ${ts(entrypoint.version)},`);
lines.push('} as const);');
lines.push('');
lines.push('export interface SbfMemoryRegionV1 {');
lines.push('  /** The VM\'s own name for the region, from its `MM_<NAME>_START` constant. */');
lines.push('  readonly name: string;');
lines.push('  readonly start: number;');
lines.push('}');
lines.push('');
lines.push('/** Size and alignment of every region: `MM_REGION_SIZE`. */');
lines.push(`export const SBF_MEMORY_REGION_SIZE_V1 = ${regionSize} as const;`);
lines.push('');
lines.push('/** The SBF address space, in address order. */');
lines.push('export const SBF_MEMORY_REGIONS_V1: ReadonlyArray<SbfMemoryRegionV1> = Object.freeze([');
for (const region of regions) {
  lines.push(`  Object.freeze({ name: ${ts(region.name)}, start: ${region.start} }),`);
}
lines.push(']);');
lines.push('');
lines.push('/** Heap a transaction gets without asking: `HEAP_LENGTH`. */');
lines.push(`export const SBF_DEFAULT_HEAP_BYTES_V1 = ${heapLength} as const;`);
lines.push('/** Largest heap frame the runtime will grant: `ADAPTER_MAX_HEAP_BYTES`. */');
lines.push(`export const SBF_MAX_HEAP_BYTES_V1 = ${maxHeap} as const;`);
lines.push('/** Multiple a `RequestHeapFrame` must be: `HEAP_FRAME_GRANULARITY_BYTES`. */');
lines.push(`export const SBF_HEAP_FRAME_GRANULARITY_BYTES_V1 = ${granularity} as const;`);
lines.push('');
lines.push('export interface SbfDisplayFormatV1 {');
lines.push('  readonly variant: string;');
lines.push('  /** The variant\'s `thiserror` format string, with its `{…}` holes intact. */');
lines.push('  readonly format: string;');
lines.push('}');
lines.push('');
lines.push('/** Every `EbpfError` the virtual machine can print. */');
lines.push('export const SBF_VM_ERROR_FORMATS_V1: ReadonlyArray<SbfDisplayFormatV1> = Object.freeze([');
for (const entry of ebpfFormats) {
  lines.push(`  Object.freeze({ variant: ${ts(entry.variant)}, format: ${ts(entry.format)} }),`);
}
lines.push(']);');
lines.push('');
lines.push('/** Every `SyscallError` a syscall can raise through the VM. */');
lines.push('export const SBF_SYSCALL_ERROR_FORMATS_V1: ReadonlyArray<SbfDisplayFormatV1> = Object.freeze([');
for (const entry of syscallFormats) {
  lines.push(`  Object.freeze({ variant: ${ts(entry.variant)}, format: ${ts(entry.format)} }),`);
}
lines.push(']);');
const generated = `${lines.join('\n')}\n`;

if (process.argv.includes('--check')) {
  if (readFileSync(outputUrl, 'utf8') !== generated) {
    console.error('lib/generated/sbfRuntimeV1.ts is stale — run `npm run abi:sbf-runtime`');
    process.exit(1);
  }
} else {
  // Emit beside the canonical output and rename: a generator that dies halfway
  // must leave the last accepted file byte-for-byte intact.
  const temporary = fileURLToPath(new URL('../lib/generated/.sbfRuntimeV1.ts.tmp', import.meta.url));
  writeFileSync(temporary, generated);
  try {
    renameSync(temporary, fileURLToPath(outputUrl));
  } catch (error) {
    rmSync(temporary, { force: true });
    throw error;
  }
}
