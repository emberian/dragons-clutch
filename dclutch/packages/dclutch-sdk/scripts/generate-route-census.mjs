/**
 * Emit `lib/generated/routeCensus.ts` from the gauntlet route census.
 *
 * The browser needs three protocol facts it must never state in its own words:
 *
 *   1. the refusal bands (`crates/dclutch-refusal-registry`), so a bare
 *      `Custom(N)` from an RPC simulation can be attributed to the program that
 *      raised it — `band = code >> 12`, band 0 is not ours;
 *   2. every refusal code's enum variant and its own doc comment, so a refusal
 *      renders by NAME and MEANING instead of as a hexadecimal number;
 *   3. every entry route's instruction magic, so a transaction's instruction
 *      data can be attributed to the route that would have consumed it.
 *
 * All three already have exactly one authority: `dclutch-route-census
 * inventory`, which enumerates them from the program sources on every run and
 * is what `docs/reference/{programs,routes,refusals}.md` is generated from
 * (tools/genref). This script is that same enumeration, narrowed to what a
 * browser can use, and gated the way every other `lib/generated/` module is:
 * `--check` byte-compares and writes nothing.
 *
 * The census is deliberately NOT checked in (tools/genref/generate.sh says so:
 * "it is the enumeration authority and is never checked in"), so this script
 * runs it, exactly as genref does. Pass `--inventory FILE` to reuse one.
 *
 * Usage:
 *   node scripts/generate-route-census.mjs                    # regenerate
 *   node scripts/generate-route-census.mjs --check            # verify only
 *   node scripts/generate-route-census.mjs --inventory FILE   # reuse one
 */
import { execFileSync } from 'node:child_process';
import { mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = fileURLToPath(new URL('../../../', import.meta.url));
const outputPath = fileURLToPath(new URL('../lib/generated/routeCensus.ts', import.meta.url));
const censusDir = join(repoRoot, 'tools', 'gauntlet', 'census');

function inventoryPathFromArgv(argv) {
  const index = argv.indexOf('--inventory');
  return index >= 0 ? argv[index + 1] : null;
}

/** Run the census over the tree, exactly as `tools/genref/generate.sh` does. */
function runCensus() {
  const scratch = mkdtempSync(join(tmpdir(), 'dclutch-route-census-'));
  const out = join(scratch, 'inventory.json');
  try {
    execFileSync(
      'cargo',
      ['run', '--release', '--quiet', '--', 'inventory', '--root', repoRoot, '--out', out, '--check-unique'],
      { cwd: censusDir, stdio: ['ignore', 'ignore', 'inherit'] },
    );
    return JSON.parse(readFileSync(out, 'utf8'));
  } finally {
    rmSync(scratch, { recursive: true, force: true });
  }
}

const inventoryOverride = inventoryPathFromArgv(process.argv);
const inventory = inventoryOverride
  ? JSON.parse(readFileSync(inventoryOverride, 'utf8'))
  : runCensus();

if (inventory.schema !== 'dclutch-gauntlet-route-inventory-v1') {
  throw new Error(`unexpected inventory schema ${inventory.schema}`);
}

/**
 * The band table's file, read rather than the inventory: the inventory carries
 * bands only as a uniqueness check input, and the registry is where the
 * allocation and its prose live.
 *
 * It moved out of the crate's `lib.rs` and into `generated_bands.rs` on
 * 2026-09-02 (`1d8b999a`, "decision 0007's band allocation gets an author"),
 * which made `DClutchSemantics.RefusalBandsV1` its authority. This reader was
 * not swept with it, so `abi:route-census` threw `BANDS table not found` from
 * that commit until it was noticed: a browser surface with no authority behind
 * it, which is the exact failure `AGENTS.md` names when a Rust fact moves and
 * its non-Rust consumers are left pointing at the old address.
 */
const bandAllocationPath = join(
  repoRoot,
  'crates',
  'dclutch-refusal-registry',
  'src',
  'generated_bands.rs',
);

/**
 * The band table, read from the registry crate rather than the inventory.
 */
function readBands() {
  const source = readFileSync(bandAllocationPath, 'utf8');
  const table = source.match(/pub const BANDS: &\[RefusalBand\] = &\[([\s\S]*?)\n\];/);
  if (!table) throw new Error('refusal registry: BANDS table not found');
  const entries = [...table[1].matchAll(/RefusalBand \{([\s\S]*?)\}/g)].map((entry) => entry[1]);
  const bases = new Map(
    [...source.matchAll(/pub const ([A-Z0-9_]+): u32 = (0x[0-9A-Fa-f_]+);/g)].map((match) => [
      match[1],
      Number(match[2].replaceAll('_', '')),
    ]),
  );
  const span = bases.get('BAND_SPAN') ?? Number(source.match(/pub const BAND_SPAN: u32 = (0x[0-9A-Fa-f]+);/)?.[1]);
  if (!Number.isSafeInteger(span)) throw new Error('refusal registry: BAND_SPAN not resolved');
  return entries.map((entry) => {
    const field = (name) => entry.match(new RegExp(`${name}:\\s*([^,\\n]+)`))?.[1]?.trim();
    const label = field('label')?.replace(/^"|"$/g, '');
    const pkg = field('package')?.replace(/^"|"$/g, '');
    const baseName = field('base');
    const base = bases.get(baseName);
    const spanExpr = field('span');
    const tier = field('tier') === 'BandTier::Program' ? 'program' : 'test-caller';
    if (label === undefined || pkg === undefined || base === undefined) {
      throw new Error(`refusal registry: unresolved band entry ${entry}`);
    }
    if (spanExpr !== 'BAND_SPAN') throw new Error(`refusal registry: band ${label} has a non-BAND_SPAN span`);
    return { label, package: pkg, base, span, tier };
  });
}

const bands = readBands();
const bandShift = Number(readFileSync(bandAllocationPath, 'utf8')
  .match(/pub const BAND_SHIFT: u32 = (\d+);/)?.[1]);
if (!Number.isSafeInteger(bandShift)) throw new Error('refusal registry: BAND_SHIFT not resolved');

const programs = [...inventory.programs].sort((left, right) => left.label.localeCompare(right.label));

const refusals = [];
for (const program of programs) {
  for (const refusal of [...program.refusals].sort((left, right) => (left.code ?? 0) - (right.code ?? 0))) {
    if (refusal.code === null || refusal.code === undefined) continue;
    refusals.push({
      code: refusal.code,
      id: refusal.id,
      program: program.label,
      package: program.package,
      enumName: refusal.enum_name,
      variant: refusal.variant,
      meaning: refusal.summary ?? null,
      provenance: refusal.provenance,
    });
  }
}
refusals.sort((left, right) => left.code - right.code);

/**
 * A predicate arm's magic, resolved from the Rust the predicate is.
 *
 * WHY THIS EXISTS. The census records a predicate arm as
 * `predicate hot_v3::is_hot_execution_v3()` and stops there, so
 * `INSTRUCTION_MAGICS` carried none of Trading's twenty-four arms and none of
 * Resolution's ten. A consumer holding a compiled instruction could name the
 * route behind `DCLTSQ03` and not the route behind `DCLTHOT3` -- and the
 * second is the one this browser actually builds. The predicate is not an
 * absence of a magic; it is a magic comparison written as a function, and
 * every one of these reads the same leading eight bytes the direct arms do.
 *
 * So this resolves the function: find its body, find the constant it compares
 * the leading bytes against, and resolve that constant to its ASCII. What does
 * NOT resolve is reported by name in `UNRESOLVED_PREDICATE_ARMS_V1` rather
 * than dropped, because a predicate that decodes a struct instead of comparing
 * a magic (`GenericMarketFoundingCallerBumpsV3::decode(..).is_ok()`) is a real
 * arm that no leading-byte view can ever name, and a consumer must be able to
 * tell that from a resolution this script merely failed at.
 */
const rustSources = new Map();
function collectRustSources(directory) {
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) {
      if (entry.name === 'target' || entry.name === 'node_modules') continue;
      collectRustSources(path);
    } else if (entry.name.endsWith('.rs')) {
      rustSources.set(path, readFileSync(path, 'utf8'));
    }
  }
}
collectRustSources(join(repoRoot, 'crates'));
collectRustSources(join(repoRoot, 'programs'));

/** The body of `fn <name>`, at any visibility, brace-balanced, or null. */
function functionBody(source, name) {
  // `pub(crate) fn` is as much a dispatch predicate as `pub fn`: eight of the
  // ten arms this failed to find were crate-visible, which read here as "the
  // function does not exist" and is the exact confusion this table exists to
  // stop reporting.
  const start = source.search(new RegExp(`(?:^|\\n)\\s*(?:pub(?:\\([a-z]+\\))?\\s+)?fn ${name}\\s*[(<]`));
  if (start < 0) return null;
  const open = source.indexOf('{', start);
  if (open < 0) return null;
  let depth = 0;
  for (let index = open; index < source.length; index += 1) {
    if (source[index] === '{') depth += 1;
    else if (source[index] === '}') {
      depth -= 1;
      if (depth === 0) return source.slice(open + 1, index);
    }
  }
  return null;
}

/** Files a `a::b::is_x` path could live in, most specific first. */
function predicateCandidates(functionPath, programPackage) {
  const parts = functionPath.split('::');
  const moduleHint = parts.length > 1 ? parts[parts.length - 2] : null;
  // `dclutch_direct_codec::token_setup_v1::is_x` names a crate then a module;
  // `dclutch_user_position_admission_contract::is_x` names a crate and its
  // root. Both spellings appear, so the crate is whichever leading segment
  // carries the prefix rather than a fixed position.
  const crateHint = parts.length > 1 && parts[0].startsWith('dclutch_')
    ? parts[0].replaceAll('_', '-')
    : null;
  const paths = [...rustSources.keys()];
  const inCrate = crateHint === null ? [] : paths.filter((path) => path.includes(`/crates/${crateHint}/src/`));
  const inProgram = paths.filter((path) => path.includes(`/programs/${programPackage}/src/`));
  const byModule = moduleHint === null
    ? []
    : paths.filter((path) => path.endsWith(`/${moduleHint}.rs`) || path.endsWith(`/${moduleHint}/mod.rs`));
  const ordered = [];
  for (const group of [
    crateHint === null ? [] : inCrate.filter((path) => byModule.includes(path)),
    inProgram.filter((path) => byModule.includes(path)),
    inCrate,
    inProgram,
    byModule,
  ]) {
    for (const path of group) if (!ordered.includes(path)) ordered.push(path);
  }
  return ordered;
}

const MAGIC_COMPARISONS = [
  // `input.get(..8) == Some(MAGIC.as_slice())`, its `..MAGIC.len()` form, and
  // the `Some(&MAGIC)` spelling two Resolution arms use.
  /get\(\.\.(?:8|[A-Za-z0-9_:]+\.len\(\))\)\s*==\s*Some\(&?([A-Za-z0-9_:]+)(?:\.as_slice\(\))?\)/g,
  // `data == MAGIC`, where the whole instruction IS the eight-byte magic.
  /^\s*(?:instruction_)?data\s*==\s*([A-Z][A-Z0-9_]+)\s*$/gm,
  /^\s*input\s*==\s*([A-Z][A-Z0-9_]+)\s*$/gm,
  // `matches!(bytes.get(..8), Some(magic) if magic == A || magic == B)` --
  // one arm that admits several magics, which is a fact about the route and
  // not an ambiguity: each one selects it.
  /\bmagic\s*==\s*([A-Z][A-Z0-9_]+)/g,
  // `input[..8] == MAGIC` / `input.starts_with(&MAGIC)`.
  /\[\.\.8\]\s*==\s*([A-Za-z0-9_:]+)/g,
  /starts_with\(&([A-Za-z0-9_:]+)\)/g,
];

/** The eight ASCII bytes a `[u8; 8]` constant holds, or null. */
function constantAscii(name, preferredFiles) {
  const short = name.split('::').pop();
  const order = [...preferredFiles, ...rustSources.keys()];
  for (const path of order) {
    const source = rustSources.get(path);
    if (source === undefined) continue;
    const byteString = source.match(new RegExp(`const ${short}: \\[u8; 8\\] = \\*b"([^"]{8})";`));
    if (byteString) return { ascii: byteString[1], provenance: path };
    const array = source.match(new RegExp(`const ${short}: \\[u8; 8\\] = \\[([^\\]]+)\\]`));
    if (array) {
      const bytes = array[1].split(',').map((piece) => piece.trim()).filter((piece) => piece.length > 0);
      if (bytes.length !== 8) continue;
      const values = bytes.map((piece) => Number(piece));
      if (values.some((value) => !Number.isInteger(value) || value < 0x20 || value > 0x7e)) continue;
      return { ascii: values.map((value) => String.fromCharCode(value)).join(''), provenance: path };
    }
  }
  return null;
}

/** Resolve one predicate arm to the magic it compares, or say why not. */
function resolvePredicate(functionPath, programPackage) {
  const candidates = predicateCandidates(functionPath, programPackage);
  const name = functionPath.split('::').pop();
  for (const path of candidates) {
    const body = functionBody(rustSources.get(path), name);
    if (body === null) continue;
    for (const pattern of MAGIC_COMPARISONS) {
      const names = [...new Set([...body.matchAll(pattern)].map((match) => match[1]))];
      if (names.length === 0) continue;
      const magics = [];
      for (const constant of names) {
        const resolved = constantAscii(constant, [path]);
        if (resolved === null) {
          return { magics: [], source: relative(path), reason: `${constant} did not resolve to eight ASCII bytes` };
        }
        magics.push({ ascii: resolved.ascii, constant });
      }
      return { magics, source: relative(path), reason: null };
    }
    return {
      magics: [],
      source: relative(path),
      reason: `${name} compares no leading magic: ${body.trim().split('\n')[0].trim()}`,
    };
  }
  return { magics: [], source: null, reason: `${functionPath} was not found in crates/ or programs/` };
}

function relative(path) {
  return path.startsWith(repoRoot) ? path.slice(repoRoot.length) : path;
}

const predicateArms = [];
const unresolvedPredicateArms = [];
for (const program of programs) {
  for (const route of [...program.routes].sort((left, right) => left.id.localeCompare(right.id))) {
    for (const selector of route.selectors) {
      if (selector.kind !== 'predicate') continue;
      const resolved = resolvePredicate(selector.function, program.package);
      if (resolved.magics.length === 0) {
        unresolvedPredicateArms.push({
          routeId: route.id,
          program: program.label,
          function: selector.function,
          reason: resolved.reason,
        });
        continue;
      }
      for (const magic of resolved.magics) {
        predicateArms.push({
          magic: magic.ascii,
          constant: magic.constant,
          program: program.label,
          routeId: route.id,
          handler: route.handler,
          function: selector.function,
          provenance: resolved.source,
        });
      }
    }
  }
}
predicateArms.sort((left, right) => left.magic.localeCompare(right.magic) || left.routeId.localeCompare(right.routeId));
unresolvedPredicateArms.sort((left, right) => left.routeId.localeCompare(right.routeId));

const magics = [];
for (const program of programs) {
  for (const route of [...program.routes].sort((left, right) => left.id.localeCompare(right.id))) {
    for (const selector of route.selectors) {
      if (selector.kind !== 'magic' || !selector.ascii) continue;
      magics.push({
        magic: selector.ascii,
        hex: selector.bytes ?? null,
        constant: selector.constant,
        program: program.label,
        routeId: route.id,
        handler: route.handler,
        provenance: route.provenance,
      });
    }
  }
}
magics.sort((left, right) => left.magic.localeCompare(right.magic) || left.routeId.localeCompare(right.routeId));

/**
 * Entry routes that no magic selects. These are the honest gap in a
 * transaction view: the program dispatches on a predicate, a decoded action
 * variant, or an exact length, and the browser cannot name the route from the
 * leading eight bytes alone. Carried rather than dropped, for the same reason
 * genref carries unrendered exports.
 */
const unselectedEntries = [];
for (const program of programs) {
  for (const route of [...program.routes].sort((left, right) => left.id.localeCompare(right.id))) {
    if (route.kind !== 'entry') continue;
    if (route.selectors.some((selector) => selector.kind === 'magic' && selector.ascii)) continue;
    unselectedEntries.push({
      routeId: route.id,
      program: program.label,
      handler: route.handler,
      selectors: route.selectors.map(renderSelector),
      provenance: route.provenance,
    });
  }
}

function renderSelector(selector) {
  switch (selector.kind) {
    case 'magic':
      return selector.ascii ? `magic ${selector.constant} = "${selector.ascii}"` : `magic ${selector.constant}`;
    case 'length':
      return selector.value === null || selector.value === undefined
        ? `len == ${selector.constant}`
        : `len == ${selector.constant} (${selector.value})`;
    case 'predicate':
      return `predicate ${selector.function}()`;
    case 'variant':
      return `tag ${selector.path}`;
    case 'tag':
      return `tag ${selector.text}`;
    case 'literal':
      return `literal ${selector.text}`;
    case 'fallthrough':
      return 'fallthrough (no earlier guard matched)';
    default:
      throw new Error(`unknown selector kind ${selector.kind}`);
  }
}

function literal(value) {
  return JSON.stringify(value);
}

function record(fields) {
  return `Object.freeze({ ${fields.map(([name, value]) => `${name}: ${value}`).join(', ')} })`;
}

let out = '';
out += '// @generated by scripts/generate-route-census.mjs from `dclutch-route-census inventory`; do not edit.\n';
out += '// Regenerate with: npm run abi:route-census\n';
out += '//\n';
out += '// Sources:\n';
out += '//   tools/gauntlet/census                        (the route/refusal enumeration)\n';
out += '//   crates/dclutch-refusal-registry/src/generated_bands.rs   (the band allocation)\n';
out += '//\n';
out += `// ${programs.length} programs, ${magics.length} magic-selected routes, ${predicateArms.length} predicate-selected routes, ${refusals.length} refusal codes.\n\n`;

out += '/** Whether a band is deployed to a real cluster or exists only under `program-test`. */\n';
out += "export type RefusalBandTier = 'program' | 'test-caller';\n\n";

out += '/** One package\'s exclusive allocation of custom program error codes. */\n';
out += 'export type RefusalBand = Readonly<{\n';
out += '  label: string;\n  package: string;\n  base: number;\n  span: number;\n  tier: RefusalBandTier;\n}>;\n\n';

out += '/** One refusal, with the meaning its own enum doc comment states. */\n';
out += 'export type ProtocolRefusal = Readonly<{\n';
out += '  code: number;\n  id: string;\n  program: string;\n  package: string;\n';
out += '  enumName: string;\n  variant: string;\n  meaning: string | null;\n  provenance: string;\n}>;\n\n';

out += '/** An instruction magic that selects one entry route. */\n';
out += 'export type InstructionMagic = Readonly<{\n';
out += '  magic: string;\n  hex: string | null;\n  constant: string;\n  program: string;\n';
out += '  routeId: string;\n  handler: string;\n  provenance: string;\n}>;\n\n';

out += '/** An entry route no leading-bytes magic selects. */\n';
out += 'export type UnselectedEntryRoute = Readonly<{\n';
out += '  routeId: string;\n  program: string;\n  handler: string;\n';
out += '  selectors: ReadonlyArray<string>;\n  provenance: string;\n}>;\n\n';

out += '/**\n';
out += ' * A route a `fn is_x(instruction_data) -> bool` predicate selects, with the\n';
out += ' * magic that predicate compares resolved from its own Rust.\n';
out += ' */\n';
out += 'export type PredicateSelectedRoute = Readonly<{\n';
out += '  magic: string;\n  constant: string;\n  program: string;\n  routeId: string;\n';
out += '  handler: string;\n  predicate: string;\n  provenance: string;\n}>;\n\n';

out += '/** A predicate arm whose selector is not a leading magic at all. */\n';
out += 'export type UnresolvedPredicateArm = Readonly<{\n';
out += '  routeId: string;\n  program: string;\n  predicate: string;\n  reason: string;\n}>;\n\n';

out += '/** Width of one refusal band, in codes. */\n';
out += `export const REFUSAL_BAND_SPAN = ${bands[0]?.span ?? 0} as const;\n`;
out += '/** `code >> REFUSAL_BAND_SHIFT` is the band index. */\n';
out += `export const REFUSAL_BAND_SHIFT = ${bandShift} as const;\n\n`;

out += '/** Every allocated band, ascending by base. Band 0 is never allocated. */\n';
out += 'export const REFUSAL_BANDS: ReadonlyArray<RefusalBand> = Object.freeze([\n';
for (const band of bands) {
  out += `  ${record([
    ['label', literal(band.label)],
    ['package', literal(band.package)],
    ['base', String(band.base)],
    ['span', String(band.span)],
    ['tier', literal(band.tier)],
  ])},\n`;
}
out += ']);\n\n';

out += '/** Every enumerated refusal code, ascending. */\n';
out += 'export const PROTOCOL_REFUSALS: ReadonlyArray<ProtocolRefusal> = Object.freeze([\n';
for (const refusal of refusals) {
  out += `  ${record([
    ['code', String(refusal.code)],
    ['id', literal(refusal.id)],
    ['program', literal(refusal.program)],
    ['package', literal(refusal.package)],
    ['enumName', literal(refusal.enumName)],
    ['variant', literal(refusal.variant)],
    ['meaning', refusal.meaning === null ? 'null' : literal(refusal.meaning)],
    ['provenance', literal(refusal.provenance)],
  ])},\n`;
}
out += ']);\n\n';

out += '/** Every instruction magic that selects an entry route. */\n';
out += 'export const INSTRUCTION_MAGICS: ReadonlyArray<InstructionMagic> = Object.freeze([\n';
for (const magic of magics) {
  out += `  ${record([
    ['magic', literal(magic.magic)],
    ['hex', magic.hex === null ? 'null' : literal(magic.hex)],
    ['constant', literal(magic.constant)],
    ['program', literal(magic.program)],
    ['routeId', literal(magic.routeId)],
    ['handler', literal(magic.handler)],
    ['provenance', literal(magic.provenance)],
  ])},\n`;
}
out += ']);\n\n';

out += '/**\n';
out += ' * Entry routes selected by something other than a leading magic — a\n';
out += ' * predicate, a decoded action tag, or an exact instruction length. A\n';
out += ' * transaction view cannot name these from the first eight bytes; it says so\n';
out += ' * rather than guessing, and this list is what it says it from.\n';
out += ' */\n';
out += 'export const UNSELECTED_ENTRY_ROUTES: ReadonlyArray<UnselectedEntryRoute> = Object.freeze([\n';
for (const route of unselectedEntries) {
  out += `  ${record([
    ['routeId', literal(route.routeId)],
    ['program', literal(route.program)],
    ['handler', literal(route.handler)],
    ['selectors', `Object.freeze([${route.selectors.map(literal).join(', ')}])`],
    ['provenance', literal(route.provenance)],
  ])},\n`;
}
out += ']);\n\n';

out += '/**\n';
out += ' * Every route a dispatch predicate selects, and the magic it compares.\n';
out += ' *\n';
out += ' * `INSTRUCTION_MAGICS` above carries the arms whose dispatch compares the\n';
out += ' * leading bytes INLINE. Trading compares them inside `fn is_x()` instead, and\n';
out += ' * so does Resolution -- so a consumer holding a compiled instruction could\n';
out += ' * name the route behind `DCLTSQ03` and not the one behind `DCLTHOT3`, which is\n';
out += ' * the route this browser actually builds. The predicate is a magic comparison\n';
out += ' * written as a function; this table is that function resolved, with the file\n';
out += ' * it was resolved from as provenance. Read the two tables together: an\n';
out += ' * instruction is named by its program and its first eight bytes either way.\n';
out += ' */\n';
out += 'export const PREDICATE_SELECTED_ROUTES: ReadonlyArray<PredicateSelectedRoute> = Object.freeze([\n';
for (const arm of predicateArms) {
  out += `  ${record([
    ['magic', literal(arm.magic)],
    ['constant', literal(arm.constant)],
    ['program', literal(arm.program)],
    ['routeId', literal(arm.routeId)],
    ['handler', literal(arm.handler)],
    ['predicate', literal(arm.function)],
    ['provenance', literal(arm.provenance)],
  ])},\n`;
}
out += ']);\n\n';

out += '/**\n';
out += ' * Predicate arms that no leading-byte view can ever name, and why.\n';
out += ' *\n';
out += ' * A predicate that decodes a whole struct rather than comparing a magic\n';
out += ' * (`GenericMarketFoundingCallerBumpsV3::decode(..).is_ok()`) selects a real\n';
out += ' * route on bytes no eight-byte prefix distinguishes. That is a fact about the\n';
out += ' * program, not a failure of this scrape, and it is carried by name so a\n';
out += ' * consumer can tell it from a resolution that merely did not happen.\n';
out += ' */\n';
out += 'export const UNRESOLVED_PREDICATE_ARMS_V1: ReadonlyArray<UnresolvedPredicateArm> = Object.freeze([\n';
for (const arm of unresolvedPredicateArms) {
  out += `  ${record([
    ['routeId', literal(arm.routeId)],
    ['program', literal(arm.program)],
    ['predicate', literal(arm.function)],
    ['reason', literal(arm.reason)],
  ])},\n`;
}
out += ']);\n';

if (process.argv.includes('--check')) {
  const current = readFileSync(outputPath, 'utf8');
  if (current !== out) {
    console.error('lib/generated/routeCensus.ts is stale — run `npm run abi:route-census`');
    process.exit(1);
  }
  console.log(`route census up to date: ${refusals.length} refusals, ${magics.length} magic-selected and ${predicateArms.length} predicate-selected routes`);
} else {
  writeFileSync(outputPath, out);
  console.log(`wrote lib/generated/routeCensus.ts: ${refusals.length} refusals, ${magics.length} magic-selected and ${predicateArms.length} predicate-selected routes, ${unresolvedPredicateArms.length} predicate arms unresolved`);
}
