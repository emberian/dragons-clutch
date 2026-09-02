/**
 * The refusal registry, mirrored for clients.
 *
 * A Solana `custom program error` is a bare u32; the chain never says which
 * program refused or why. Protocol-wide, `crates/dclutch-refusal-registry` is
 * the authority that namespaces every code by program band (decision 0007),
 * and the census harvests each enum variant's own doc comment into
 * `docs/reference/refusals.md`. A client that renders `Custom(20608)` is
 * making its reader do the band arithmetic by hand; this module lets it say
 * `claims 0x5080 ClaimsFoundingV5Error::Instruction` instead.
 *
 * Two sources, each already an authority for its half:
 *   - the band table is scraped from the registry crate's own `BANDS` const —
 *     the same source the census parses with syn;
 *   - the per-code names and meanings come from `docs/reference/refusals.md`,
 *     which `tools/genref/generate.sh --check` byte-gates against the census
 *     sweep of every `#[repr(u32)]` refusal enum in the tree.
 *
 * So the chain of custody is: enum doc comment -> census -> refusals.md ->
 * this module, with a verify gate at every arrow.
 *
 * RUN genref FIRST. Because the band table comes from the registry crate and
 * the codes come from `refusals.md`, a band removed from the crate while
 * `refusals.md` still lists its codes makes this script throw "code 0x… sits
 * in no registered band" rather than emit a stale row -- so after any band
 * removal, `tools/genref/generate.sh` has to run before `abi:refusal-registry`,
 * not after (found deleting band 0x7 on 2026-09-02).
 */
import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const root = new URL('../../../', import.meta.url);
const registrySource = readFileSync(new URL('crates/dclutch-refusal-registry/src/lib.rs', root), 'utf8');
const referenceSource = readFileSync(new URL('docs/reference/refusals.md', root), 'utf8');
const outputUrl = new URL('../lib/generated/refusalRegistryV1.ts', import.meta.url);

// ---------------------------------------------------------------- band table

function constant(name) {
  const match = registrySource.match(new RegExp(`pub const ${name}: u32 = (0x[0-9A-Fa-f_]+|[0-9]+);`));
  if (!match) throw new Error(`missing registry constant ${name}`);
  return Number(match[1].replaceAll('_', ''));
}

const BAND_SHIFT = constant('BAND_SHIFT');
const BAND_SPAN = constant('BAND_SPAN');

const bandEntries = [...registrySource.matchAll(
  /RefusalBand \{\s*label: "([^"]+)",\s*package: "([^"]+)",\s*base: ([A-Z0-9_]+),\s*span: BAND_SPAN,\s*tier: BandTier::(Program|TestCaller),\s*\}/g,
)].map(([, label, pkg, baseName, tier]) => ({ label, package: pkg, base: constant(baseName), tier }));
if (bandEntries.length === 0) throw new Error('found no RefusalBand entries in the registry crate');
for (let index = 1; index < bandEntries.length; index += 1) {
  const previous = bandEntries[index - 1];
  const entry = bandEntries[index];
  if (entry.base <= previous.base) throw new Error(`band table is not ascending at ${entry.label}`);
}

// ---------------------------------------------------------------- code table

// docs/reference/refusals.md: one `## <band label>` section per program, each
// row `| \`0xNNNN\` | \`Enum::Variant\` | meaning | provenance |`. The band
// allocation table at the top has a different row shape and never matches.
const codeEntries = [];
let section = null;
for (const line of referenceSource.split('\n')) {
  const heading = line.match(/^## (.+)$/);
  if (heading) { section = heading[1].trim(); continue; }
  const row = line.match(/^\| `(0x[0-9A-Fa-f]+)` \| `([A-Za-z0-9]+)::([A-Za-z0-9]+)` \| (.+?) \| /);
  if (!row || section === 'Band allocation') continue;
  const code = Number(row[1]);
  codeEntries.push({ code, enumName: row[2], variant: row[3], meaning: row[4].replaceAll('\\|', '|').trim(), band: section });
}
if (codeEntries.length < 150) throw new Error(`only ${codeEntries.length} refusal rows parsed from refusals.md; the format moved`);
for (const entry of codeEntries) {
  const owner = bandEntries.find((band) => entry.code >= band.base && entry.code < band.base + BAND_SPAN);
  if (!owner) throw new Error(`refusals.md code 0x${entry.code.toString(16)} sits in no registered band`);
}
codeEntries.sort((left, right) => left.code - right.code);

// -------------------------------------------------------------------- output

const ts = (value) => JSON.stringify(value);
let generated = '// @generated from crates/dclutch-refusal-registry/src/lib.rs and docs/reference/refusals.md; do not edit.\n';
generated += '// Regenerate with: npm run abi:refusal-registry\n\n';
generated += `export const REFUSAL_BAND_SHIFT = ${BAND_SHIFT} as const;\n`;
generated += `export const REFUSAL_BAND_SPAN = ${BAND_SPAN} as const;\n\n`;
generated += 'export interface RefusalBandV1 {\n  readonly label: string;\n  readonly package: string;\n  readonly base: number;\n  readonly tier: \'program\' | \'test-caller\';\n}\n\n';
generated += 'export const REFUSAL_BANDS_V1: ReadonlyArray<RefusalBandV1> = [\n';
for (const band of bandEntries) {
  generated += `  { label: ${ts(band.label)}, package: ${ts(band.package)}, base: 0x${band.base.toString(16).toUpperCase()}, tier: ${ts(band.tier === 'Program' ? 'program' : 'test-caller')} },\n`;
}
generated += '];\n\n';
generated += 'export interface RefusalCodeV1 {\n  readonly code: number;\n  readonly name: string;\n  readonly meaning: string;\n  readonly band: string;\n}\n\n';
generated += 'export const REFUSAL_CODES_V1: ReadonlyArray<RefusalCodeV1> = [\n';
for (const entry of codeEntries) {
  generated += `  { code: 0x${entry.code.toString(16).toUpperCase()}, name: ${ts(`${entry.enumName}::${entry.variant}`)}, meaning: ${ts(entry.meaning)}, band: ${ts(entry.band)} },\n`;
}
generated += '];\n';

if (process.argv.includes('--check')) {
  const current = readFileSync(outputUrl, 'utf8');
  if (current !== generated) {
    console.error('refusal registry TypeScript mirror is stale');
    process.exit(1);
  }
} else {
  writeFileSync(fileURLToPath(outputUrl), generated);
}
