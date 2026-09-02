/**
 * Emit `lib/generated/capabilitySurfaceV1.ts` from the browser's own import graph.
 *
 * THE MECHANISM THIS CLOSES. `/console` and `/operate` used to read their
 * status from a hand-typed `implementation:` string on each row of
 * `capabilityModel.ts`. Nothing anywhere connected that string to code. A lane
 * could type `browser-wallet` beside an act no component could construct, or
 * leave `rust-unsigned` beside one the browser had been signing for weeks, and
 * every test stayed green because the tests asserted the string. Four
 * capabilities changed state in a single night; the board changed for none of
 * them, and it was wrong in BOTH directions at once.
 *
 * So the status is no longer written down. `capabilityModel.ts` names, per act,
 * two anchors -- the component or module that OWNS the act, and the module that
 * CONSTRUCTS its bytes -- and this generator answers, from the source tree
 * itself, what that owner can actually do:
 *
 *   - which routes reach it, so an act cannot be `browser-*` unless a stranger
 *     can open a page that reaches it (an unrouted workspace is not a
 *     capability, however complete it is);
 *   - the strongest wallet request in its transitive closure, by the SDK export
 *     name that performs it -- `requestWalletMessageSignatureV1` is a detached
 *     message and nothing else, the three transaction requests are transaction
 *     authority, and their ABSENCE is the only thing that makes an act unsigned;
 *   - whether `submitSignedTransactionV1` -- the single submission primitive --
 *     is reachable, which is the whole difference between a browser that signs
 *     and exports a packet and one that sends it and verifies the poststate;
 *   - which `lib/generated/` modules it decodes against, each paired with the
 *     `abi:*:verify` script that byte-checks it. A generated module without a
 *     verify script is a surface with no authority behind it, so the pairing is
 *     emitted rather than assumed.
 *
 * These are FACTS ABOUT THE WEB APP, which is why the surface is generated here
 * and not in the SDK: the SDK owns the semantics of an act and cannot know what
 * this application routes. `capabilityModel.ts` in the SDK consumes what this
 * emits and derives the venue, authority and status from it.
 *
 * Gated like every other `lib/generated/` module: `--check` byte-compares and
 * writes nothing, and `lib/abiVerification.test.ts` runs it inside `npm test`.
 * When it goes red, the browser really did change what it can do -- regenerate
 * and read the diff; never edit the output.
 *
 * Usage:
 *   node scripts/generate-capability-surface.mjs           # regenerate
 *   node scripts/generate-capability-surface.mjs --check   # verify only
 */
import { readFileSync, readdirSync, renameSync, statSync, unlinkSync, writeFileSync } from 'node:fs';
import { dirname, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const webRoot = fileURLToPath(new URL('..', import.meta.url));
const repoRoot = fileURLToPath(new URL('../../../', import.meta.url));
const sdkRoot = join(repoRoot, 'packages', 'dclutch-sdk');
const outputPath = join(webRoot, 'lib', 'generated', 'capabilitySurfaceV1.ts');
const check = process.argv.includes('--check');

/**
 * The SDK module that owns every wallet request and the sole submission.
 *
 * Named as a file rather than by symbol so a rename cannot quietly make every
 * act look unsigned: the assertion below fails instead.
 */
const WALLET_OWNER = join(sdkRoot, 'lib', 'walletHandoff.ts');

/** Detached message signature: a portable artifact, never a transaction. */
const MESSAGE_REQUESTS = ['requestWalletMessageSignatureV1'];
/** The three ways a wallet is asked to sign a transaction. */
const TRANSACTION_REQUESTS = [
  'requestWalletTransactionSignatureV1',
  'requestWalletCosignTransactionV1',
  'requestWalletSubmitCosignTransactionV1',
];
/** The only function in the client tree that sends a packet to a cluster. */
const SUBMISSION = 'submitSignedTransactionV1';

/**
 * A file picker: the browser reading bytes it did not and cannot author.
 *
 * WHAT THIS ADDS THAT THE WALLET SURVEY MISSED. Everything above answers what
 * a workspace can DO. None of it answers what a workspace cannot START. That
 * gap put `claims.redeem` on `/console` as "This browser · one wallet
 * signature, sent from here" -- all three clauses true and derived -- over an
 * act whose second step opens a file picker for a payout plan `RedeemFlow`
 * says outright it will never author. The producer is a Rust binary under
 * `tools/local-validator/`, so a reader holding a wallet and nothing else
 * cannot begin. A derivation that has never been run against a prerequisite
 * is the same species of claim as a status somebody typed.
 *
 * A file input is the exact syntactic mark of that dependency, in either of
 * the two spellings the app uses -- the bare `<input type="file">` and the
 * `<Input type="file">` primitive -- and it propagates over the closure
 * exactly as wallet reach does. Granularity is therefore the workspace, which
 * is the granularity a capability anchor already has and the one the reader
 * needs: it is the page in front of them that will ask for the file.
 */
const FILE_INTAKE = /type=(?:"file"|'file'|\{'file'\}|\{"file"\})/;

/**
 * The second signal, because the first one alone is a guard with a hole in it.
 *
 * Every artifact intake in this app offers a paste box beside its picker --
 * `ArtifactInput` calls it "the fallback for a machine where the file cannot
 * be picked", and `RedeemFlow` has a textarea carrying the same bytes as its
 * file input. Detecting only the picker means deleting one JSX line while
 * keeping the paste path makes an act look self-contained again, and nothing
 * goes red. That is the two-sides-move-together defect reappearing inside the
 * fix for it.
 *
 * So a module also depends on outside bytes when it IMPORTS a producer-artifact
 * parser. The naming is not a convention invented here: `walletTerminalPayoutV3`
 * exports `importRustWalletTerminalPayoutArtifactV3` as a separate name from
 * the parser it wraps for exactly this reason -- "keeping this as a separate
 * name makes the browser handoff explicit: a file is imported, never authored
 * or completed from partial chain state here."
 */
const ARTIFACT_IMPORT = /^import[A-Z][A-Za-z0-9]*Artifact[A-Za-z0-9]*$/;

/**
 * The two canaries for the signals above: the component whose entire contract
 * is naming an artifact's producer, and the module that names the handoff in
 * its own export. If either stops matching, the detector has silently gone
 * blind and every act looks self-contained. Failing loudly beats surveying an
 * absence.
 */
const ARTIFACT_INTAKE_CANARY = join(webRoot, 'components', 'ArtifactInput.tsx');
const ARTIFACT_IMPORT_CANARY = join(webRoot, 'lib', 'walletTerminalPayoutV3.ts');

/**
 * `packages/dclutch-sdk/package.json`'s exports map, honoured exactly.
 *
 * `@dclutch/sdk/walletHandoff` resolves to the INSPECTION-ONLY facade, not to
 * the module above, and several subpaths are `null` -- deliberately unreachable
 * from outside the package. Resolving these by filename would credit a surface
 * with authority the package refuses to hand it.
 */
const sdkExports = JSON.parse(readFileSync(join(sdkRoot, 'package.json'), 'utf8')).exports;

/** Web `package.json` scripts, for pairing each generated module with its verifier. */
const webScripts = JSON.parse(readFileSync(join(webRoot, 'package.json'), 'utf8')).scripts;

function exists(path) {
  try {
    return statSync(path).isFile();
  } catch {
    return false;
  }
}

/** Resolve one extensionless module path the way the bundler does. */
function resolveFile(base) {
  for (const suffix of ['.ts', '.tsx', '/index.ts', '/index.tsx', '']) {
    if (exists(`${base}${suffix}`)) return `${base}${suffix}`;
  }
  return null;
}

/**
 * Resolve one import specifier to an absolute file, or null when it leaves the
 * client trees (node builtins, react, @solana/web3.js, CSS).
 */
function resolveSpecifier(fromFile, specifier) {
  if (specifier === '@dclutch/sdk') return resolveFile(join(sdkRoot, 'index'));
  if (specifier.startsWith('@dclutch/sdk/')) {
    const subpath = `./${specifier.slice('@dclutch/sdk/'.length)}`;
    const exact = sdkExports[subpath];
    if (exact === null) return null;
    if (typeof exact === 'string') return resolveFile(join(sdkRoot, exact.replace(/\.tsx?$/, '')));
    if (subpath.startsWith('./generated/')) return resolveFile(join(sdkRoot, 'lib', subpath.slice(2)));
    return resolveFile(join(sdkRoot, 'lib', subpath.slice(2)));
  }
  if (specifier.startsWith('@/')) return resolveFile(join(webRoot, specifier.slice(2)));
  if (specifier.startsWith('.')) return resolveFile(resolve(dirname(fromFile), specifier));
  return null;
}

/** Every `import`/`export ... from '<specifier>'` in one module, with its named bindings. */
function importsOf(source) {
  const found = [];
  const pattern = /(?:^|\n)\s*(?:import|export)\s+([\s\S]*?)\s*from\s*'([^']+)'/g;
  for (const match of source.matchAll(pattern)) {
    const bindings = [...match[1].matchAll(/[A-Za-z_$][\w$]*/g)].map((entry) => entry[0]);
    found.push({ specifier: match[2], bindings });
  }
  for (const match of source.matchAll(/(?:^|\n)\s*import\s*'([^']+)'/g)) {
    found.push({ specifier: match[1], bindings: [] });
  }
  return found;
}

const sourceCache = new Map();
function sourceOf(file) {
  if (!sourceCache.has(file)) sourceCache.set(file, readFileSync(file, 'utf8'));
  return sourceCache.get(file);
}

// The wallet owner must define every symbol this generator recognizes. If a
// rename lands, the failure is here rather than a board that quietly reports
// every browser act as unsigned.
{
  const owner = sourceOf(WALLET_OWNER);
  for (const symbol of [...MESSAGE_REQUESTS, ...TRANSACTION_REQUESTS, SUBMISSION]) {
    if (!owner.includes(`export async function ${symbol}(`)) {
      throw new Error(`${relative(repoRoot, WALLET_OWNER)} no longer exports ${symbol}; capability authority cannot be derived`);
    }
  }
  if (!FILE_INTAKE.test(sourceOf(ARTIFACT_INTAKE_CANARY))) {
    throw new Error(`${relative(repoRoot, ARTIFACT_INTAKE_CANARY)} no longer renders a file input; the external-artifact prerequisite cannot be derived`);
  }
  const parsers = [...sourceOf(ARTIFACT_IMPORT_CANARY).matchAll(/export function ([A-Za-z0-9]+)\(/g)]
    .map((match) => match[1])
    .filter((name) => ARTIFACT_IMPORT.test(name));
  if (parsers.length === 0) {
    throw new Error(`${relative(repoRoot, ARTIFACT_IMPORT_CANARY)} no longer exports a producer-artifact parser; the external-artifact prerequisite cannot be derived`);
  }
}

/**
 * Whether one module IS the wallet owner, or hands its whole surface on.
 *
 * `apps/dclutch-web/lib/walletHandoff.ts` is a single `export *` over the SDK
 * module, so every browser act reaches the real primitives through the web
 * path. Comparing filenames alone would have credited the entire application
 * with no wallet authority at all -- which is exactly the shape of mistake this
 * generator exists to make impossible.
 */
const ownerCache = new Map();
function isWalletOwner(file) {
  if (file === WALLET_OWNER) return true;
  const held = ownerCache.get(file);
  if (held !== undefined) return held;
  // Set before recursing: a cycle of re-exports must not become a stack
  // overflow, and a module cannot become the owner by way of itself.
  ownerCache.set(file, false);
  let owner = false;
  for (const match of sourceOf(file).matchAll(/(?:^|\n)\s*export\s+\*\s+from\s*'([^']+)'/g)) {
    const target = resolveSpecifier(file, match[1]);
    if (target !== null && isWalletOwner(target)) owner = true;
  }
  ownerCache.set(file, owner);
  return owner;
}

/** Direct facts about one module, before any propagation. */
function localFacts(file) {
  const source = sourceOf(file);
  let message = false;
  let transaction = false;
  let submits = false;
  let readsExternalFile = FILE_INTAKE.test(source);
  const edges = [];
  for (const { specifier, bindings } of importsOf(source)) {
    const target = resolveSpecifier(file, specifier);
    if (target === null) continue;
    edges.push(target);
    // A parser reached by name, not merely defined somewhere in the closure:
    // `walletTerminalPayoutV3` declares the importer and is itself reached by
    // modules that only decode payouts, which do not depend on a file.
    if (bindings.some((name) => ARTIFACT_IMPORT.test(name))) readsExternalFile = true;
    // A wallet request counts only when the specifier really resolves to the
    // owning module. The SDK's public `walletHandoff` subpath is a different
    // file on purpose, and naming a symbol is not the same as reaching it.
    if (!isWalletOwner(target)) continue;
    if (bindings.some((name) => MESSAGE_REQUESTS.includes(name))) message = true;
    if (bindings.some((name) => TRANSACTION_REQUESTS.includes(name))) transaction = true;
    if (bindings.includes(SUBMISSION)) submits = true;
  }
  return { edges, message, transaction, submits, readsExternalFile };
}

const factsCache = new Map();
function facts(file) {
  if (!factsCache.has(file)) factsCache.set(file, localFacts(file));
  return factsCache.get(file);
}

/** Every module reachable from `entry`, including itself. */
function closure(entry) {
  const seen = new Set();
  const stack = [entry];
  while (stack.length > 0) {
    const file = stack.pop();
    if (seen.has(file)) continue;
    seen.add(file);
    for (const edge of facts(file).edges) if (!seen.has(edge)) stack.push(edge);
  }
  return seen;
}

const closureCache = new Map();
function closureOf(file) {
  if (!closureCache.has(file)) closureCache.set(file, closure(file));
  return closureCache.get(file);
}

/** Every `app/**\/page.tsx`, as the route a reader can actually open. */
function routes() {
  const found = [];
  const walk = (absolute, path) => {
    for (const entry of readdirSync(absolute, { withFileTypes: true }).sort((left, right) => left.name.localeCompare(right.name))) {
      if (entry.isDirectory()) walk(join(absolute, entry.name), `${path}/${entry.name}`);
      else if (entry.name === 'page.tsx') found.push({ route: path === '' ? '/' : path, file: join(absolute, entry.name) });
    }
  };
  walk(join(webRoot, 'app'), '');
  return found;
}

/** The `abi:<name>:verify` script that byte-checks one generated module, if any. */
function verifierFor(generatedModule) {
  const target = generatedModule.replace(/^lib\/generated\//, '').replace(/\.ts$/, '');
  for (const [name, command] of Object.entries(webScripts)) {
    if (!name.startsWith('abi:') || !name.endsWith(':verify')) continue;
    if (command.includes(`lib/generated/${target}.ts`)) return name;
  }
  // Most generators name their output only inside the script file itself.
  for (const [name, command] of Object.entries(webScripts)) {
    if (!name.startsWith('abi:') || !name.endsWith(':verify')) continue;
    const script = command.split(/\s+/)[1];
    if (script === undefined || !script.startsWith('scripts/')) continue;
    const path = join(webRoot, script);
    if (exists(path) && sourceOf(path).includes(`lib/generated/${target}.ts`)) return name;
  }
  return null;
}

const web = (file) => relative(webRoot, file).split('\\').join('/');
const inWeb = (file) => file.startsWith(webRoot);

/**
 * The generated ABI a reachable module IS, following re-export shims into the
 * package.
 *
 * `apps/dclutch-web/lib/*.ts` is increasingly a two-line
 * `export * from '@dclutch/sdk/*'`, and this census resolves those specifiers
 * correctly -- but then credited a generated module only when the file it
 * landed on was under the WEB root. So the moment a surface's semantic owner
 * moved into the package, every generated ABI beneath it fell off the survey
 * and the route it serves looked like it depended on nothing.
 *
 * Measured on 2026-09-02, converging `lib/operatorSurface.ts` onto its SDK
 * owner: `/operate` and `/workbench` lost the infrastructure, refusal-band and
 * refusal-registry ABIs in one commit, and `lib/infrastructure.ts` lost both
 * routes. Nothing was less true about the browser; the instrument had stopped
 * being able to see. Ten shims already carried this blind spot.
 *
 * The two trees hold BYTE-IDENTICAL generated modules at the same relative
 * path -- `lib/twinIdentity.test.ts` is the gate, and it names every deliberate
 * exception -- and the web's `abi:*:verify` script is what checks the web copy.
 * So an SDK generated module is credited under its web-relative name, and only
 * when that web file actually exists; a package-only generated module is not
 * something a web verifier can speak for and is left uncredited.
 */
function generatedModuleName(file) {
  if (inWeb(file)) {
    const name = web(file);
    return name.startsWith('lib/generated/') ? name : null;
  }
  if (!file.startsWith(sdkRoot)) return null;
  const name = relative(sdkRoot, file).split('\\').join('/');
  if (!name.startsWith('lib/generated/')) return null;
  return exists(join(webRoot, name)) ? name : null;
}

const routeEntries = routes();
/** Route -> its own closure, so a module can be asked which routes reach it. */
const routeClosures = routeEntries.map((entry) => ({ ...entry, files: closureOf(entry.file) }));

function routesReaching(file) {
  return routeClosures.filter((entry) => entry.files.has(file)).map((entry) => entry.route).sort();
}

/**
 * The modules worth emitting a row for: every one a route can reach.
 *
 * An earlier draft emitted only modules carrying a fact DIRECTLY -- a wallet
 * import, or a `lib/generated/` import of their own -- to keep the file small.
 * That was wrong in the one way that matters here: `ProductV2Studio` reaches
 * its generated authorities through `lib/productV2.ts` and carried no fact of
 * its own, so it had no row, so an act anchored to it resolved to nothing and
 * SILENTLY lost its browser venue. A survey a status can fall through is worse
 * than a large survey. Every reachable module gets a row; a row changes only
 * when its own closure changes.
 */
function surveyed() {
  const found = new Set(routeEntries.map((entry) => entry.file));
  for (const entry of routeClosures) {
    for (const file of entry.files) if (inWeb(file)) found.add(file);
  }
  return [...found].sort((left, right) => web(left).localeCompare(web(right)));
}

function surfaceRow(file) {
  const reachable = closureOf(file);
  let message = false;
  let transaction = false;
  let submits = false;
  let readsExternalFile = false;
  const generated = new Set();
  for (const member of reachable) {
    const local = facts(member);
    message = message || local.message;
    transaction = transaction || local.transaction;
    submits = submits || local.submits;
    readsExternalFile = readsExternalFile || local.readsExternalFile;
    const generatedName = generatedModuleName(member);
    if (generatedName !== null) generated.add(generatedName);
  }
  return {
    module: web(file),
    routes: routesReaching(file),
    authority: transaction ? 'wallet-transaction' : message ? 'wallet-message' : 'none',
    submits,
    readsExternalFile,
    generatedAbis: [...generated].sort().map((module) => ({ module, verify: verifierFor(module) })),
  };
}

const rows = surveyed().map(surfaceRow);
const routeList = routeEntries.map((entry) => entry.route).sort();

/**
 * The generated authorities, once.
 *
 * Modules name them by index rather than repeating the pairing: the same
 * twenty rows appearing under two hundred modules was eighty kilobytes of
 * client bundle saying one thing.
 */
const abiTable = [...new Map(rows.flatMap((row) => row.generatedAbis).map((entry) => [entry.module, entry])).values()]
  .sort((left, right) => left.module.localeCompare(right.module));
const abiIndex = new Map(abiTable.map((entry, index) => [entry.module, index]));

/** The text of one `command={...}` attribute, brace-balanced from its opening. */
function commandExpression(source, start) {
  let depth = 0;
  for (let index = start; index < source.length; index += 1) {
    if (source[index] === '{') depth += 1;
    else if (source[index] === '}') {
      depth -= 1;
      if (depth === 0) return source.slice(start + 1, index);
    }
  }
  return '';
}

/**
 * The CLI runbooks the browser publishes, with the page that publishes each.
 *
 * A runbook is `<CommandRunbook command={…}>`, not a constant with a particular
 * name: `/found` holds its command in an exported constant and `market.join`
 * builds one per market inside its component, and both are the same claim to a
 * reader. The command TEXT is what is searched for `--execute`, never the
 * page's prose -- a paragraph explaining that a campaign needs authorization is
 * not itself a campaign.
 */
function runbooks() {
  const found = [];
  const seen = new Set();
  for (const entry of routeClosures) {
    for (const file of entry.files) {
      if (!inWeb(file) || seen.has(file)) continue;
      seen.add(file);
      const source = sourceOf(file);
      const constants = new Map(
        [...source.matchAll(/const ([A-Za-z0-9_]+)\s*=\s*`([\s\S]*?)`;/g)].map((match) => [match[1], match[2]]),
      );
      const commands = [];
      for (const match of source.matchAll(/<CommandRunbook\b[\s\S]*?command=\{/g)) {
        const expression = commandExpression(source, match.index + match[0].length - 1);
        const named = expression.trim().replace(/[`${}\s]/g, '');
        commands.push(`${expression}\n${constants.get(named) ?? ''}`);
      }
      if (commands.length === 0) continue;
      // A component may publish more than one command; the page carries
      // execution authority if any of them does.
      const text = commands.join('\n');
      found.push({
        module: web(file),
        commands: commands.length,
        routes: routesReaching(file),
        // `--execute` is what a campaign command has and a read-only export
        // does not: it is the flag that records devnet execution authority
        // before any child may read a signing key.
        namesExecutionAuthority: text.includes('--execute'),
      });
    }
  }
  return found.sort((left, right) => left.module.localeCompare(right.module));
}

/** Rust operator crates, so an act cannot name an owner the workspace lost. */
function operatorCrates() {
  return readdirSync(join(repoRoot, 'crates'), { withFileTypes: true })
    .filter((entry) => entry.isDirectory() && exists(join(repoRoot, 'crates', entry.name, 'Cargo.toml')))
    .map((entry) => entry.name)
    .sort();
}

const json = (value) => JSON.stringify(value);
const list = (values) => `[${values.map(json).join(', ')}]`;

let output = `// @generated by scripts/generate-capability-surface.mjs from the browser's own import graph; do not edit.
// Regenerate with: npm run abi:capability-surface
//
// Sources:
//   apps/dclutch-web/app/**/page.tsx                    (the routes a reader can open)
//   packages/dclutch-sdk/lib/walletHandoff.ts           (every wallet request and the sole submission)
//   apps/dclutch-web/package.json                       (the abi:*:verify pairing)
//
// ${rows.length} surveyed modules, ${routeList.length} routes, ${abiTable.length} generated authorities, ${runbooks().length} published runbooks.
// ${rows.filter((row) => row.readsExternalFile).length} of those modules cannot start without a file produced outside this browser.

/** What a module's transitive closure is able to ask a wallet for. */
export type ClientAuthorityV1 = 'none' | 'wallet-message' | 'wallet-transaction';

/** One generated decode authority, and the script that byte-checks it. */
export type GeneratedAbiReachV1 = Readonly<{ module: string; verify: string | null }>;

/** What one client module can do, read off the import graph rather than stated. */
export type ClientModuleSurfaceV1 = Readonly<{
  /** Path inside \`apps/dclutch-web\`. */
  module: string;
  /** Routes whose page transitively imports it. Empty means unreachable. */
  routes: ReadonlyArray<string>;
  /** The strongest wallet request reachable from it. */
  authority: ClientAuthorityV1;
  /** Whether the sole submission primitive is reachable from it. */
  submits: boolean;
  /**
   * Whether it opens a file picker somewhere in its closure -- the mark of an
   * act that cannot be STARTED here, whatever it can do once it has the bytes.
   */
  readsExternalFile: boolean;
  /** Indices into \`GENERATED_ABI_AUTHORITIES_V1\` for the authorities it reaches. */
  generatedAbis: ReadonlyArray<number>;
}>;

/** One page that publishes an exact CLI command a reader is meant to run. */
export type OperatorRunbookReachV1 = Readonly<{
  module: string;
  /** How many exact commands that page publishes. */
  commands: number;
  routes: ReadonlyArray<string>;
  /** Whether any command records explicit execution authority before it may sign. */
  namesExecutionAuthority: boolean;
}>;

/** Every route this application serves. */
export const CLIENT_ROUTES_V1: ReadonlyArray<string> = Object.freeze(${list(routeList)});

/** Every generated decode authority any route reaches, and its verifier. */
export const GENERATED_ABI_AUTHORITIES_V1: ReadonlyArray<GeneratedAbiReachV1> = Object.freeze([
${abiTable.map((entry) => `  Object.freeze({ module: ${json(entry.module)}, verify: ${entry.verify === null ? 'null' : json(entry.verify)} }),`).join('\n')}
]);

/** Every module whose facts a capability status can depend on. */
export const CLIENT_MODULE_SURFACES_V1: ReadonlyArray<ClientModuleSurfaceV1> = Object.freeze([
`;
for (const row of rows) {
  const abis = row.generatedAbis.map((entry) => abiIndex.get(entry.module)).join(', ');
  output += `  Object.freeze({ module: ${json(row.module)}, routes: Object.freeze(${list(row.routes)}), authority: ${json(row.authority)}, submits: ${row.submits}, readsExternalFile: ${row.readsExternalFile}, generatedAbis: Object.freeze([${abis}]) }),\n`;
}
output += `]);

/** Every CLI runbook the browser publishes, and where it is published. */
export const OPERATOR_RUNBOOKS_V1: ReadonlyArray<OperatorRunbookReachV1> = Object.freeze([
`;
for (const entry of runbooks()) {
  output += `  Object.freeze({ module: ${json(entry.module)}, commands: ${entry.commands}, routes: Object.freeze(${list(entry.routes)}), namesExecutionAuthority: ${entry.namesExecutionAuthority} }),\n`;
}
output += `]);

/** Every Rust crate in the workspace, so an act cannot name an owner that is gone. */
export const OPERATOR_CRATES_V1: ReadonlyArray<string> = Object.freeze(${list(operatorCrates())});
`;

if (check) {
  const current = exists(outputPath) ? readFileSync(outputPath, 'utf8') : '';
  if (current !== output) {
    process.stderr.write('lib/generated/capabilitySurfaceV1.ts no longer matches the browser import graph.\n');
    process.exit(1);
  }
  process.exit(0);
}

// Atomic replacement: a half-written surface would be a half-true board.
const scratch = `${outputPath}.tmp`;
writeFileSync(scratch, output);
try {
  renameSync(scratch, outputPath);
} catch (error) {
  unlinkSync(scratch);
  throw error;
}
process.stdout.write(`wrote lib/generated/capabilitySurfaceV1.ts (${rows.length} modules, ${routeList.length} routes)\n`);
