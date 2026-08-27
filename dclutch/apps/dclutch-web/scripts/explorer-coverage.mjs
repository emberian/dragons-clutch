/**
 * The explorer's coverage ratchet.
 *
 * `lib/generated/` is the browser's decode authority: every record magic and
 * every instruction magic the protocol has is emitted into it from a Lean
 * schema, a Rust contract, or the route census. The explorer's job is to render
 * them. This survey enumerates what the generated modules declare and what the
 * explorer's render maps actually handle, so the gap is a number rather than an
 * impression.
 *
 * The gate is `lib/explorerCoverage.test.ts`: a magic that appears in a
 * generated module and in no render map fails the build. When a new record
 * lands — a new family, a new version of an existing one — the failing test is
 * the notification, and the only two honest ways to clear it are to render the
 * record or to record it in `explorer-coverage.exempt.json` with a reason. An
 * unrendered magic is never silent.
 *
 * Usage:
 *   node scripts/explorer-coverage.mjs           # print the coverage table
 *   node scripts/explorer-coverage.mjs --json    # machine-readable survey
 */
import { readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const webRoot = fileURLToPath(new URL('..', import.meta.url));
const generatedDir = join(webRoot, 'lib', 'generated');
const exemptPath = join(webRoot, 'scripts', 'explorer-coverage.exempt.json');

/**
 * The census module is emitted like the rest but declares INSTRUCTION magics,
 * not account-record layouts. It is surveyed separately, below.
 */
const CENSUS_MODULE = 'routeCensus.ts';

/** `export const NAME_MAGIC... = 'DCLTXXXX'` — the string form. */
const STRING_MAGIC = /export const ([A-Z0-9_]*MAGIC[A-Z0-9_]*)\s*=\s*'([A-Z0-9]{8})'/g;
/** `export const NAME_MAGIC... = Uint8Array.from([0x44, ...])` — the byte form. */
const BYTES_MAGIC = /export const ([A-Z0-9_]*MAGIC[A-Z0-9_]*)\s*=\s*Uint8Array\.from\(\[([^\]]*)\]\)/g;

function asciiFromBytes(text) {
  const bytes = [...text.matchAll(/0x[0-9a-fA-F]{1,2}|\b\d{1,3}\b/g)].map((entry) => Number(entry[0]));
  if (bytes.length !== 8) return null;
  if (bytes.some((byte) => byte < 0x20 || byte > 0x7e)) return null;
  return String.fromCharCode(...bytes);
}

/**
 * Every record magic the generated modules declare, as
 * `{ magic, constants: [{ module, constant }] }`. One magic can be declared by
 * more than one module — `DCLTAP02` is exported by two — and that is a fact
 * about the emission, not a duplicate to collapse away.
 */
export function surveyRecordMagics() {
  const found = new Map();
  for (const entry of readdirSync(generatedDir).sort()) {
    if (!entry.endsWith('.ts') || entry === CENSUS_MODULE) continue;
    const text = readFileSync(join(generatedDir, entry), 'utf8');
    const record = (constant, magic) => {
      if (magic === null) return;
      const held = found.get(magic) ?? [];
      held.push({ module: entry, constant });
      found.set(magic, held);
    };
    for (const match of text.matchAll(STRING_MAGIC)) record(match[1], match[2]);
    for (const match of text.matchAll(BYTES_MAGIC)) record(match[1], asciiFromBytes(match[2]));
  }
  return [...found.entries()]
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([magic, constants]) => ({ magic, constants }));
}

/**
 * Modules that emit a record's LAYOUT but not the magic that identifies it.
 *
 * This is the inverse failure of an unrendered magic, and the coverage join
 * above is structurally blind to it: a magic that was never declared cannot
 * appear in a survey of declared magics. The symptom is a module carrying a
 * `..._MAGIC_OFFSET` — an offset for a field whose VALUE it never emits —
 * alongside a full set of field offsets. The browser then has everything it
 * needs to read the record and no way to know it is looking at one, which is
 * worse than having neither: the layout invites a decode the emission cannot
 * justify.
 *
 * Found by asking, per module, whether every `MAGIC` constant it declares is an
 * offset. A module with no magic constants at all is not a finding.
 */
export function surveyMagiclessLayouts() {
  const found = [];
  for (const entry of readdirSync(generatedDir).sort()) {
    if (!entry.endsWith('.ts') || entry === CENSUS_MODULE) continue;
    const text = readFileSync(join(generatedDir, entry), 'utf8');
    const declared = [...text.matchAll(/export const ([A-Z0-9_]*MAGIC[A-Z0-9_]*)\s*=/g)].map((match) => match[1]);
    if (declared.length === 0) continue;
    const values = declared.filter((name) => !name.endsWith('_OFFSET') && !name.endsWith('_OFFSET_V1'));
    if (values.length > 0) continue;
    found.push({ module: entry, constants: declared });
  }
  return found;
}

/** Every instruction magic the route census emits, with the route it selects. */
export function surveyInstructionMagics() {
  const text = readFileSync(join(generatedDir, CENSUS_MODULE), 'utf8');
  const table = text.match(/export const INSTRUCTION_MAGICS[\s\S]*?\n\]\);/);
  if (!table) throw new Error('routeCensus.ts: INSTRUCTION_MAGICS table not found');
  const found = new Map();
  for (const entry of table[0].matchAll(/magic: "([^"]+)", hex: [^,]+, constant: "([^"]+)", program: "([^"]+)", routeId: "([^"]+)"/g)) {
    const held = found.get(entry[1]) ?? [];
    held.push({ constant: entry[2], program: entry[3], routeId: entry[4] });
    found.set(entry[1], held);
  }
  return [...found.entries()]
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([magic, routes]) => ({ magic, routes }));
}

/**
 * What the explorer's two render maps handle.
 *
 * Read from the render modules' source rather than by importing them, so this
 * script needs no TypeScript loader and can run from any tooling.
 *
 * The account map is matched by CONSTANT NAME — it writes `magic:
 * REALM_MAGIC_V1`, never `magic: 'DCLTRLM1'` — and the constant is then
 * resolved against the generated modules' own declarations. That is deliberate:
 * `lib/abiCoverage.test.ts` forbids the browser from writing a magic string by
 * hand, so a render map that tried to pass this gate that way would fail that
 * one.
 *
 * The instruction map is matched by CENSUS ROUTE ID, because an instruction
 * magic's constant name in the census is a Rust path with no TypeScript
 * counterpart. A route id is a census identifier, not a protocol fact.
 */
export function surveyRenderMaps() {
  const accountText = readFileSync(join(webRoot, 'lib', 'explorer', 'accountRecords.ts'), 'utf8');
  const instructionText = readFileSync(join(webRoot, 'lib', 'explorer', 'instructions.ts'), 'utf8');
  return {
    accountConstants: new Set(namesUnder(accountText, 'RECORD_RENDERERS', /magic:\s*([A-Z0-9_]+)\b/g)),
    instructionRoutes: new Set(namesUnder(instructionText, 'INSTRUCTION_RENDERERS', /routeId:\s*'([^']+)'/g)),
  };
}

/** Values matched by `pattern` inside a named table. The table ends at `\n]);`. */
function namesUnder(text, tableName, pattern) {
  const start = text.indexOf(`const ${tableName}`);
  if (start < 0) throw new Error(`render map ${tableName} not found`);
  const end = text.indexOf('\n]);', start);
  if (end < 0) throw new Error(`render map ${tableName} is not terminated by ']);'`);
  const body = text.slice(start, end);
  return [...body.matchAll(pattern)].map((match) => match[1]);
}

export function readExemptions() {
  return JSON.parse(readFileSync(exemptPath, 'utf8'));
}

/**
 * Join the survey into a coverage report.
 *
 * A record magic is COVERED when the render map names one of the constants that
 * declares it. It is EXEMPT when `explorer-coverage.exempt.json` records it with
 * a reason. Otherwise it is UNRENDERED, and the gate fails.
 */
export function coverage() {
  const records = surveyRecordMagics();
  const instructions = surveyInstructionMagics();
  const { accountConstants, instructionRoutes } = surveyRenderMaps();
  const exempt = readExemptions();
  const recordExempt = new Map(Object.entries(exempt.records ?? {}));
  const instructionExempt = new Map(Object.entries(exempt.instructions ?? {}));

  const recordRows = records.map((entry) => {
    const rendered = entry.constants.some((held) => accountConstants.has(held.constant));
    const reason = recordExempt.get(entry.magic) ?? null;
    return {
      magic: entry.magic,
      modules: entry.constants.map((held) => `${held.module}:${held.constant}`),
      state: rendered ? 'rendered' : reason !== null ? 'exempt' : 'unrendered',
      reason,
    };
  });

  const instructionRows = instructions.map((entry) => {
    const rendered = entry.routes.some((route) => instructionRoutes.has(route.routeId));
    const reason = instructionExempt.get(entry.magic) ?? null;
    return {
      magic: entry.magic,
      routes: entry.routes.map((route) => route.routeId),
      programs: [...new Set(entry.routes.map((route) => route.program))],
      state: rendered ? 'rendered' : reason !== null ? 'exempt' : 'unrendered',
      reason,
    };
  });

  const magicless = new Map(Object.entries(exempt.magiclessLayouts ?? {}));
  const layoutRows = surveyMagiclessLayouts().map((entry) => {
    const reason = magicless.get(entry.module) ?? null;
    return {
      magic: entry.module,
      constants: entry.constants,
      state: reason !== null ? 'exempt' : 'unrendered',
      reason,
    };
  });

  return { records: recordRows, instructions: instructionRows, magiclessLayouts: layoutRows };
}

function tally(rows) {
  return {
    total: rows.length,
    rendered: rows.filter((row) => row.state === 'rendered').length,
    exempt: rows.filter((row) => row.state === 'exempt').length,
    unrendered: rows.filter((row) => row.state === 'unrendered').length,
  };
}

function main() {
  const report = coverage();
  if (process.argv.includes('--json')) {
    console.log(JSON.stringify(report, null, 2));
    return;
  }
  const recordTally = tally(report.records);
  const instructionTally = tally(report.instructions);
  const layoutTally = tally(report.magiclessLayouts);
  console.log('Explorer coverage — what lib/generated/ declares vs what the explorer renders\n');
  console.log(`  account records     ${recordTally.rendered}/${recordTally.total} rendered, ${recordTally.exempt} exempt, ${recordTally.unrendered} unrendered`);
  console.log(`  instruction magics  ${instructionTally.rendered}/${instructionTally.total} rendered, ${instructionTally.exempt} exempt, ${instructionTally.unrendered} unrendered`);
  console.log(`  magicless layouts   ${layoutTally.unrendered} unexempted, ${layoutTally.exempt} recorded\n`);
  for (const row of report.magiclessLayouts) {
    console.log(`${row.state === 'exempt' ? ' skip ' : ' MISS '}layout  ${row.magic}  emits ${row.constants.join(', ')} and no magic value${row.reason ? `  — ${row.reason}` : ''}`);
  }
  for (const row of report.records) {
    const mark = row.state === 'rendered' ? '  ok  ' : row.state === 'exempt' ? ' skip ' : ' MISS ';
    console.log(`${mark}record  ${row.magic}  ${row.modules.join(', ')}${row.reason ? `  — ${row.reason}` : ''}`);
  }
  console.log('');
  for (const row of report.instructions) {
    const mark = row.state === 'rendered' ? '  ok  ' : row.state === 'exempt' ? ' skip ' : ' MISS ';
    console.log(`${mark}ix      ${row.magic}  ${row.programs.join(', ')}${row.reason ? `  — ${row.reason}` : ''}`);
  }
  const missing = [...report.records, ...report.instructions, ...report.magiclessLayouts].filter((row) => row.state === 'unrendered');
  if (missing.length > 0) {
    console.error(`\n${missing.length} finding(s):`);
    for (const row of missing) console.error(`  ${row.magic}`);
    console.error('\nRender it in lib/explorer/, or record it in scripts/explorer-coverage.exempt.json with a reason.');
    process.exit(1);
  }
  console.log('\nevery declared magic is rendered or explicitly exempt');
}

if (process.argv[1] && process.argv[1].endsWith('explorer-coverage.mjs')) main();
