/**
 * The hand-mirror inventory: every protocol fact the browser still states in
 * its own words instead of importing from `lib/generated/`.
 *
 * A hand-mirror is a record magic, a PDA seed domain, or a byte offset written
 * as a literal in browser source. Each one is a second authority for something
 * a Lean schema or a contract already owns, and each is a place the browser can
 * silently disagree with the chain — the failure mode is not a crash, it is a
 * page that confidently shows the wrong thing, or a transaction built against a
 * layout that moved.
 *
 * This survey is the done-criterion for that genus: it enumerates what is left.
 * `lib/abiCoverage.test.ts` fails CI when the inventory grows, so a new
 * hand-mirror cannot be added quietly; the baseline shrinks as surfaces are
 * converted to Lean-emitted modules (see `DClutchSemantics/TsEmit.lean`).
 *
 * Test files are surveyed but never gate. A test that writes `DCLTFQ01` by hand
 * is pinning the value the decoder must accept — that is a check, not a mirror.
 *
 * Usage:
 *   node scripts/abi-coverage.mjs            # print the inventory, check baseline
 *   node scripts/abi-coverage.mjs --write    # rewrite the baseline
 *   node scripts/abi-coverage.mjs --json     # machine-readable survey
 */
import { existsSync, readFileSync, readdirSync, statSync, writeFileSync } from 'node:fs';
import { join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

const webRoot = fileURLToPath(new URL('..', import.meta.url));
const baselinePath = join(webRoot, 'scripts', 'abi-coverage.baseline.json');

/**
 * Directories surveyed. Everything else is build output or third-party. The
 * list is filtered for existence so the same survey runs in the SDK package
 * (lib only) and in the web app (lib + components + app).
 */
const SURVEYED = ['lib', 'components', 'app'].filter((directory) => existsSync(join(webRoot, directory)));
/** The one place a protocol fact is allowed to be written down. */
const GENERATED = join('lib', 'generated');

/** An eight-character canonical record magic. */
const MAGIC = /'((?:DCLT|DCLR)[A-Z0-9]{4})'/g;
/** A PDA seed domain, in either of the two punctuation conventions in use. */
const DOMAIN = /'(dclutch[:/][A-Za-z0-9:/._-]*)'/g;
/**
 * A byte coordinate written as a literal at a hostile-decode call site. These
 * are counted per file rather than listed per site: the number is the ratchet,
 * and the file name is enough to find them.
 */
const OFFSET = /\b(?:u8|u16|u32|u64|slice|ascii|requireZero|requireNonzero)\(\s*[A-Za-z_$][\w$.]*\s*,\s*(\d+)/g;

function sourceFiles(directory) {
  const found = [];
  const walk = (absolute) => {
    for (const entry of readdirSync(absolute)) {
      if (entry === 'node_modules' || entry === 'dist' || entry.startsWith('.')) continue;
      const child = join(absolute, entry);
      if (statSync(child).isDirectory()) {
        walk(child);
      } else if (/\.tsx?$/.test(entry) && !entry.endsWith('.d.ts')) {
        found.push(child);
      }
    }
  };
  walk(join(webRoot, directory));
  return found;
}

function matches(text, pattern) {
  return [...text.matchAll(pattern)].map((match) => match[1]);
}

/**
 * Survey the browser source.
 *
 * Returns `gating` — the inventory CI holds to a ratchet — and `pins`, the
 * same shapes inside test files, reported for completeness only.
 */
export function surveyHandMirrors() {
  const gating = { magics: [], domains: [], offsets: {} };
  const pins = { magics: [], domains: [], offsets: {} };
  for (const directory of SURVEYED) {
    for (const absolute of sourceFiles(directory)) {
      const path = relative(webRoot, absolute);
      if (path.startsWith(GENERATED)) continue;
      const into = /\.test\.tsx?$/.test(path) ? pins : gating;
      const text = readFileSync(absolute, 'utf8');
      for (const magic of new Set(matches(text, MAGIC))) into.magics.push(`${path}\t${magic}`);
      for (const domain of new Set(matches(text, DOMAIN))) into.domains.push(`${path}\t${domain}`);
      const offsets = matches(text, OFFSET).length;
      if (offsets > 0) into.offsets[path] = offsets;
    }
  }
  gating.magics.sort();
  gating.domains.sort();
  pins.magics.sort();
  pins.domains.sort();
  return { gating, pins };
}

/**
 * Compare a survey against the baseline.
 *
 * Magics and domains are exact sets: a converted one must leave the baseline,
 * and a new one must not appear. Offsets are a ratchet: a file may only ever
 * hold fewer literal coordinates than it holds today.
 */
export function auditAgainstBaseline(gating, baseline) {
  const failures = [];
  for (const kind of ['magics', 'domains']) {
    const current = new Set(gating[kind]);
    const recorded = new Set(baseline[kind]);
    for (const entry of current) {
      if (!recorded.has(entry)) failures.push(`new hand-stated ${kind.slice(0, -1)}: ${entry.replace('\t', ' → ')}`);
    }
    for (const entry of recorded) {
      if (!current.has(entry)) failures.push(`converted ${kind.slice(0, -1)} still in the baseline: ${entry.replace('\t', ' → ')}`);
    }
  }
  for (const [path, count] of Object.entries(gating.offsets)) {
    const recorded = baseline.offsets[path] ?? 0;
    if (count > recorded) failures.push(`${path} states ${count} literal byte coordinates, above its baseline of ${recorded}`);
  }
  return failures;
}

/**
 * Every `lib/generated/` module, paired with the `abi:*:verify` that byte-checks
 * it.
 *
 * The hand-mirror survey above SKIPS `lib/generated/` on purpose: a fact stated
 * there is stated by its authority. That leaves the question it cannot ask --
 * whether the generated module HAS an authority. AGENTS.md's words: "every
 * generated module carries an `abi:*:verify` that `npm test` runs, so a surface
 * with neither is a surface with no authority behind it."
 *
 * `packages/dclutch-sdk/lib/generated/routeCensus.ts` sat exactly that way. It
 * had no generator and no verify in its own package; the only thing holding it
 * to anything was the web tree's byte-identity test, and on 2026-09-02 the two
 * copies were found stale by DIFFERENT amounts, having drifted apart through a
 * program edit with one gate noticing (2fe2b9f84). Neither tree's coverage
 * census named the file at all, which is the hole this arm closes.
 *
 * A verifier is resolved in three steps, because one generator legitimately
 * writes into both trees:
 *   1. an `abi:*:verify` in THIS package whose command names the module --
 *      the Lean-emitted modules pass their output path to `lean-emit.mjs`, so
 *      for those the path is an argument rather than the script;
 *   2. one whose GENERATOR SOURCE names the module;
 *   3. one in the SIBLING tree whose generator source names THIS tree's path.
 *      `generate-source-provider-wasm.mjs` writes the web module and the SDK
 *      module in a single run, and a second script here would be a second
 *      author for a file that already has one.
 */
const TREES = ['apps/dclutch-web', 'packages/dclutch-sdk'];
const repoRoot = join(webRoot, '..', '..');

function scriptsOf(root) {
  try {
    return JSON.parse(readFileSync(join(root, 'package.json'), 'utf8')).scripts ?? {};
  } catch {
    return {};
  }
}

function sourceOf(root, command) {
  const parts = command.split(/\s+/);
  const script = parts.find((part) => part.endsWith('.mjs'));
  if (script === undefined) return '';
  try {
    return readFileSync(join(root, script), 'utf8');
  } catch {
    return '';
  }
}

/** The `abi:<name>:verify` that byte-checks one generated module, and where it lives. */
function verifierFor(module, ownScripts, siblingRoot, siblingScripts) {
  for (const [name, command] of Object.entries(ownScripts)) {
    if (!name.startsWith('abi:') || !name.endsWith(':verify')) continue;
    if (command.includes(module)) return { verify: name, tree: 'this package' };
  }
  for (const [name, command] of Object.entries(ownScripts)) {
    if (!name.startsWith('abi:') || !name.endsWith(':verify')) continue;
    if (sourceOf(webRoot, command).includes(module)) return { verify: name, tree: 'this package' };
  }
  if (siblingRoot !== null) {
    // The sibling generator names this tree's file by its REPO-RELATIVE path,
    // which is the spelling `generate-source-provider-wasm.mjs` writes.
    const fromRepo = relative(repoRoot, join(webRoot, module)).split('\\').join('/');
    for (const [name, command] of Object.entries(siblingScripts)) {
      if (!name.startsWith('abi:') || !name.endsWith(':verify')) continue;
      if (sourceOf(siblingRoot, command).includes(fromRepo)) {
        return { verify: name, tree: relative(repoRoot, siblingRoot) };
      }
    }
  }
  return { verify: null, tree: null };
}

/**
 * Survey `lib/generated/`, newest question first: does each module have a
 * verifier, and whose.
 */
export function surveyGeneratedAuthorities() {
  const here = TREES.find((tree) => join(repoRoot, tree) === join(webRoot, '.').replace(/\/\.$/, ''))
    ?? TREES.find((tree) => webRoot.replace(/\/$/, '').endsWith(tree));
  const siblingName = TREES.find((tree) => tree !== here) ?? null;
  const siblingRoot = siblingName === null ? null : join(repoRoot, siblingName);
  const ownScripts = scriptsOf(webRoot);
  const siblingScripts = siblingRoot === null ? {} : scriptsOf(siblingRoot);
  const directory = join(webRoot, GENERATED);
  let entries = [];
  try {
    entries = readdirSync(directory, { withFileTypes: true });
  } catch {
    return [];
  }
  return entries
    .filter((entry) => entry.isFile() && /\.tsx?$/.test(entry.name) && !entry.name.endsWith('.d.ts'))
    .map((entry) => `${GENERATED}/${entry.name}`.split('\\').join('/'))
    .sort()
    .map((module) => ({ module, ...verifierFor(module, ownScripts, siblingRoot, siblingScripts) }));
}

/** A generated module with no verifier anywhere is a surface with no authority. */
export function auditGeneratedAuthorities(rows) {
  return rows
    .filter((row) => row.verify === null)
    .map((row) => `${row.module} is generated and NOTHING verifies it: no abi:*:verify in this package names it, and no generator in the twin tree writes it`);
}

export function readBaseline() {
  return JSON.parse(readFileSync(baselinePath, 'utf8'));
}

function main() {
  const { gating, pins } = surveyHandMirrors();
  if (process.argv.includes('--json')) {
    console.log(JSON.stringify({ gating, pins }, null, 2));
    return;
  }
  if (process.argv.includes('--write')) {
    writeFileSync(baselinePath, `${JSON.stringify(gating, null, 2)}\n`);
    console.log(`wrote ${relative(webRoot, baselinePath)}`);
    return;
  }
  const offsetTotal = Object.values(gating.offsets).reduce((sum, count) => sum + count, 0);
  console.log('Hand-mirror inventory — protocol facts the browser still states itself\n');
  console.log(`  record magics   ${gating.magics.length}`);
  console.log(`  seed domains    ${gating.domains.length}`);
  console.log(`  byte offsets    ${offsetTotal} across ${Object.keys(gating.offsets).length} files\n`);
  for (const entry of gating.magics) console.log(`  magic   ${entry.replace('\t', '  ')}`);
  console.log('');
  for (const entry of gating.domains) console.log(`  domain  ${entry.replace('\t', '  ')}`);
  console.log('');
  for (const [path, count] of Object.entries(gating.offsets).sort((left, right) => right[1] - left[1])) {
    console.log(`  offsets ${String(count).padStart(4)}  ${path}`);
  }
  const authorities = surveyGeneratedAuthorities();
  console.log(`\nGenerated modules and the verify that byte-checks each -- ${authorities.length} of them\n`);
  for (const row of authorities) {
    console.log(`  ${row.verify === null ? 'NO AUTHORITY' : row.verify.padEnd(36)}  ${row.module}${row.tree !== null && row.tree !== 'this package' ? `  (in ${row.tree})` : ''}`);
  }

  const failures = [
    ...auditAgainstBaseline(gating, readBaseline()),
    ...auditGeneratedAuthorities(authorities),
  ];
  if (failures.length > 0) {
    console.error(`\n${failures.length} inventory change(s):`);
    for (const failure of failures) console.error(`  ${failure}`);
    console.error('\nConvert the surface, or run `npm run abi:coverage -- --write` if the baseline shrank.');
    console.error('A module with NO AUTHORITY needs a generator and an `abi:<name>:verify` in');
    console.error('this package -- or, if a twin-tree generator already writes it, nothing but');
    console.error('that generator naming this path.');
    process.exit(1);
  }
  console.log('\ninventory matches its baseline, and every generated module has a verifier');
}

if (process.argv[1] && process.argv[1].endsWith('abi-coverage.mjs')) main();
