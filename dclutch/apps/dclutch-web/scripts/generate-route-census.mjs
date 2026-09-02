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
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
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
out += `// ${programs.length} programs, ${magics.length} magic-selected routes, ${refusals.length} refusal codes.\n\n`;

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
out += ']);\n';

if (process.argv.includes('--check')) {
  const current = readFileSync(outputPath, 'utf8');
  if (current !== out) {
    console.error('lib/generated/routeCensus.ts is stale — run `npm run abi:route-census`');
    process.exit(1);
  }
  console.log(`route census up to date: ${refusals.length} refusals, ${magics.length} magic-selected routes`);
} else {
  writeFileSync(outputPath, out);
  console.log(`wrote lib/generated/routeCensus.ts: ${refusals.length} refusals, ${magics.length} magic-selected routes`);
}
