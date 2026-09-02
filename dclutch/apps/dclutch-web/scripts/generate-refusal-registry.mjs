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
 * This module emits the CODE table only. The band allocation moved to
 * `lib/generated/refusalBandsV1.ts`, which `DClutchSemantics.RefusalBandsV1`
 * emits directly: this script used to obtain the same table by running a
 * regular expression over `crates/dclutch-refusal-registry/src/lib.rs`, and
 * once that crate became generated too the scrape was reading a generated file
 * to rebuild what the generator already knew. The bases are still read here,
 * from the emitted module, for one purpose only -- proving that every code in
 * `refusals.md` lands inside an allocated band.
 *
 * The per-code names and meanings come from `docs/reference/refusals.md`,
 * which `tools/genref/generate.sh --check` byte-gates against the census sweep
 * of every `#[repr(u32)]` refusal enum in the tree.
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
const bandsSource = readFileSync(new URL('../lib/generated/refusalBandsV1.ts', import.meta.url), 'utf8');
const referenceSource = readFileSync(new URL('docs/reference/refusals.md', root), 'utf8');
const outputUrl = new URL('../lib/generated/refusalRegistryV1.ts', import.meta.url);

// ------------------------------------------------- the allocated band bases
//
// Read back from the Lean-emitted module rather than matched out of Rust
// source. This is a membership test, not a second copy of the table: nothing
// below re-emits a band.

function emitted(name) {
  const match = bandsSource.match(new RegExp(`export const ${name} = ([0-9]+) as const;`));
  if (!match) throw new Error(`missing ${name} in lib/generated/refusalBandsV1.ts`);
  return Number(match[1]);
}

const BAND_SPAN = emitted('REFUSAL_BAND_SPAN');
const BAND_COUNT = emitted('REFUSAL_BAND_COUNT');

const bandEntries = [...bandsSource.matchAll(
  /\{ label: '([^']+)', package: '([^']+)', base: (0x[0-9A-Fa-f]+), tier: '(program|test-caller)' \}/g,
)].map(([, label, pkg, base, tier]) => ({ label, package: pkg, base: Number(base), tier }));
if (bandEntries.length !== BAND_COUNT) {
  throw new Error(`parsed ${bandEntries.length} bands but the emitted module declares ${BAND_COUNT}`);
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
let generated = '// @generated from lib/generated/refusalBandsV1.ts and docs/reference/refusals.md; do not edit.\n';
generated += '// Regenerate with: npm run abi:refusal-registry\n\n';
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
