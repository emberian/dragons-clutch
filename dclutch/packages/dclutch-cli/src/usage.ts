/**
 * The command list and the flag table: what `dclutch-terminal --help` prints.
 *
 * A MODULE OF ITS OWN, with no imports, so that two readers can have it. The
 * first is `main.ts`, which parses with `FLAG_OPTIONS` and prints `USAGE`. The
 * second is `bin/dclutch-terminal.mjs`, the launcher, in the checkout where
 * `dist/` has not been built: node reads TypeScript directly, and a file that
 * imports nothing is a file the launcher can load without a bundler, an
 * install, or a second copy of the command list.
 *
 * WHY THAT MATTERS. `--help` is the one question a client must answer BEFORE
 * it is built, because it is how a reader finds out what building it would
 * give them -- and `tools/gate commands` asks it of every command the runbooks
 * publish. In a fresh checkout the launcher used to answer with a page that
 * named no verb and no flag, so eighteen published commands were reported as
 * rejected by their own program: `markets`, `portfolio`, `intent`, `walk` and
 * the flags beside them. The list did not move; the reader simply could not
 * reach it. It has one author either way, and this file is it.
 *
 * KEEP THIS FILE IMPORT-FREE, and keep its syntax erasable (no enums, no
 * parameter properties): both are what let node load it as it stands.
 */

/**
 * Every flag `parseArgs` accepts. THE authority, and `--help` renders from it.
 *
 * Exported so `test/cli.test.ts` can hold the two together rather than a lane
 * remembering to. A flag here that `--help` does not print is a flag a reader
 * cannot discover, which is how five of these came to be taught only by
 * `docs/guides/trencher.md`.
 */
export const FLAG_OPTIONS = {
  cluster: { type: 'string' },
  rpc: { type: 'string' },
  session: { type: 'string' },
  keypair: { type: 'string' },
  payer: { type: 'string' },
  recipient: { type: 'string' },
  json: { type: 'boolean' },
  'dry-run': { type: 'boolean' },
  spec: { type: 'string' },
  'keypair-seed': { type: 'string' },
  'session-out': { type: 'string' },
  'bootstrap-bin': { type: 'string' },
  input: { type: 'string' },
  'output-dir': { type: 'string' },
  report: { type: 'string' },
  'found-operation': { type: 'string' },
  'found-journal': { type: 'string' },
  execute: { type: 'boolean' },
  plan: { type: 'string' },
  'expected-plan-sha256': { type: 'string' },
  'core-checked': { type: 'string' },
  'expected-core-checked-sha256': { type: 'string' },
  'claims-checked': { type: 'string' },
  'expected-claims-checked-sha256': { type: 'string' },
  'trading-checked': { type: 'string' },
  'expected-trading-checked-sha256': { type: 'string' },
  'resolution-checked': { type: 'string' },
  'expected-resolution-checked-sha256': { type: 'string' },
  'custody-checked': { type: 'string' },
  'expected-custody-checked-sha256': { type: 'string' },
  'checked-execution-release': { type: 'string' },
  'expected-checked-execution-release-sha256': { type: 'string' },
  'registry-checked': { type: 'string' },
  'expected-registry-checked-sha256': { type: 'string' },
  'rent-checked': { type: 'string' },
  'expected-rent-checked-sha256': { type: 'string' },
  'campaign-evidence': { type: 'string' },
  output: { type: 'string' },
  'fee-payer-keypair': { type: 'string' },
  'minimum-finalized-slot': { type: 'string' },
  'collateral-source-owner-keypair': { type: 'string' },
  'collateral-source-account': { type: 'string' },
  'collateral-quantity-atoms': { type: 'string' },
  'payout-input': { type: 'string' },
  'payout-evidence': { type: 'string' },
  'payout-alt-plan': { type: 'string' },
  'payout-journal': { type: 'string' },
  'discard-unsigned-payout': { type: 'boolean' },
  'i-mean-devnet': { type: 'string' },
  'registry-program': { type: 'string' },
  'core-program': { type: 'string' },
  'claims-program': { type: 'string' },
  'trading-program': { type: 'string' },
  'resolution-program': { type: 'string' },
  'custody-program': { type: 'string' },
  'rent-credit-program': { type: 'string' },
  route: { type: 'string' },
  take: { type: 'string' },
  out: { type: 'string' },
  outcome: { type: 'string' },
  maker: { type: 'string' },
  fill: { type: 'string' },
  price: { type: 'string' },
  lifecycle: { type: 'string' },
  nonce: { type: 'string' },
  'valid-from': { type: 'string' },
  'valid-through': { type: 'string' },
  'duration-slots': { type: 'string' },
  collateral: { type: 'string' },
  'counter-keypair': { type: 'string' },
  'counter-collateral': { type: 'string' },
  'counter-nonce': { type: 'string' },
  market: { type: 'string' },
  book: { type: 'string' },
  generation: { type: 'string' },
  'terminal-sequence': { type: 'string' },
  help: { type: 'boolean' },
} as const;

/**
 * Every flag this client accepts, printed because `--help` READS THE PARSER.
 *
 * The prose above describes the flags a reader most often needs, and it is
 * hand-written, so it drifts. It had: `dclutch-terminal intent buy --route
 * --outcome --fill --price --collateral` is the exact command
 * `docs/guides/trencher.md` teaches, and not one of those five flags appeared
 * in `--help`. A reader who typed the guide's command and then typed `--help`
 * to check it was shown a help page that did not admit the flags existed.
 *
 * So the complete list is not written down twice. It is rendered from
 * `FLAG_OPTIONS`, which is the table `parseArgs` actually parses with, and a
 * flag added there appears here in the same edit or not at all.
 * `test/cli.test.ts` holds the two to each other.
 *
 * `--help` is on the list like every other flag, and it used to be filtered
 * out. A help page that does not name its own help flag is the same defect one
 * turn tighter -- and it was load-bearing outside this file: `tools/doc-commands`
 * holds every published command to the words its program's `--help` prints, so
 * `node packages/dclutch-cli/bin/dclutch-terminal.mjs --help`, which
 * `docs/guides/two-clients.md` publishes, named a flag this page denied.
 */
function acceptedFlagsV1(): string {
  const names = Object.keys(FLAG_OPTIONS)
    .sort()
    .map((name) => `--${name}`);
  const lines: string[] = [];
  let row = ' ';
  for (const name of names) {
    if (row.length + name.length + 1 > 92) { lines.push(row); row = ' '; }
    row += ` ${name}`;
  }
  if (row.trim().length > 0) lines.push(row);
  return lines.join('\n');
}

export const USAGE = `dclutch-terminal — the dClutch terminal client

usage: dclutch-terminal [global flags] <command> [args]

commands:
  markets ls                       enumerate and decode markets under the Core program
  markets show <address>           one market, in full, at one finalized floor
  portfolio [owner]                indexer-free position rollup (owner defaults to --keypair)
  offer sell                       derive seller state + nonce and sign one portable sell ticket (--out; never submits)
  intent sell|buy                  low-level: sign one fully explicit portable Direct intent (--out; never submits)
  route release-set|direct         produce pinned checked release/Direct route evidence (read-only devnet; no keys)
  product spline                   compile one canonical degree-2/3 Product graph (key-free; no chain access)
  product inspect                  verify its report + five files and print the exact Found39 handoff
  buy                              disabled: refuses before context, keys, signing, or RPC access
  sell                             disabled: refuses before context, keys, signing, or RPC access
  spine                            is this market Direct-tradable now, and which walls stand (--market)
  redeem                           resume or finalize one exact wallet payout
  found                            private-validator lifecycle (--spec), or durable permanent-devnet founding + participant admission
  join                             admit one participant into a founded market (--plan, --campaign-evidence, --output; preflight unless --execute)
  walk                             preview the funded failure walk (--dry-run required; submission disabled)
  refusal <code...>                name any custom program error via the band registry

global flags:
  --cluster <name>       devnet | local: take the seven program ids (and, absent --rpc, the
                         endpoint) from the SDK deployment manifest this client ships. The
                         endpoint must then prove that chain's identity before any id is used.
  --rpc <url>            JSON-RPC endpoint (default $DCLUTCH_RPC, then the session file, then
                         the --cluster endpoint, then http://127.0.0.1:20890/)
  --session <json>       a run spec, run evidence, or dclutch-terminal session file carrying program ids + markets
  --keypair <path>       Solana JSON keypair; also $DCLUTCH_KEYPAIR (never a default wallet path)
  --json                 machine-readable output where a command supports it
  --dry-run              where supported, build and print without signing or submitting; never enables buy/sell
  --bootstrap-bin <path> exact Rust successor producer used by route, product, found, join, and redeem
  --input <json>         canonical spline Product authoring input for product spline
  --output-dir <path>    new directory for the five compiled Product records and report
  --report <json>        exact report.json to inspect with its five sibling Product files
  --payout-input <json>  exact Rust payout-plan input for redeem
  --payout-evidence <json>
                         completed campaign evidence paired with --spec
  --payer <address>      exact Position owner and payout signer
  --recipient <address>  exact collateral token account for redeem
  --payout-alt-plan <json>
                         persist/resume the owner-funded ordered payout ALT
  --payout-journal <json>
                         crash-safe unsigned/submitted payout operation journal
  --discard-unsigned-payout
                         archive an unsigned payout journal without signing it
  --i-mean-devnet <hash> name devnet by its full genesis hash for found, join, redeem, and walk
  --found-operation <json>
                         exact permanent-devnet market + participant operation
  --found-journal <json> durable outer journal for that operation
  --execute              authorize the founding or join operation after read-only preparation
  --plan <json>          successor run plan naming the programs join admits against
  --campaign-evidence <json>
                         completed founding campaign evidence for join
  --output <json>        durable admission report join reads, resumes, and reports
  --fee-payer-keypair <path>
                         join fee payer; defaults to the --keypair position owner
  --minimum-finalized-slot <u64>
                         state join's finalized floor instead of reading it from the endpoint
  --collateral-source-owner-keypair <path>
                         fund the admitted position after admission; requires the two flags below
  --collateral-source-account <address>
                         exact source token account for that funding
  --collateral-quantity-atoms <u64>
                         exact raw-atom quantity for that funding

program ids come from --cluster, --session, or explicit --core-program/--claims-program/... flags,
in that order of increasing explicitness; the most explicit wins.
refusal codes: band = code >> 12; codes below 0x1000 are provably not dClutch's. See docs/guides/client-developers.md.

every flag this client accepts, rendered from the parser's own table (the prose above
describes the ones most readers need; this is the complete set):
${acceptedFlagsV1()}`;
