/**
 * dclutch-terminal — the dClutch terminal client.
 *
 * Command dispatch and flag parsing only; each command lives in
 * `commands/` and every chain fact it states comes through @dclutch/sdk,
 * whose generated modules are byte-gated against the protocol's own
 * authorities. Refusals render by NAME via the band registry.
 */
import { parseArgs } from 'node:util';

import { resolveContext } from './context';
import { found } from './commands/found';
import { join } from './commands/join';
import { marketsLs, marketsShow } from './commands/markets';
import { portfolio } from './commands/portfolio';
import { productCommand } from './commands/product';
import { redeem } from './commands/redeem';
import { refusal } from './commands/refusal';
import { routeCommand } from './commands/route';
import { spine } from './commands/spine';
import { intentCommand, offerCommand, tradeCommand } from './commands/trade';
import { walk } from './commands/walk';
import { fail, STDIO, type Io } from './output';

const USAGE = `dclutch-terminal — the dClutch terminal client

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
refusal codes: band = code >> 12; codes below 0x1000 are provably not dClutch's. See docs/guides/client-developers.md.`;

const FLAG_OPTIONS = {
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
 * The verbs of the OTHER dClutch client in this repository, whose executable
 * is named `dclutch`.
 *
 * This repository ships two clients and exactly one of them is distributed.
 * This one is the terminal client (`packages/dclutch-cli`), installed only
 * from this checkout: its manifest is `private: true`, `@dclutch/cli` is not
 * on any registry, and `docs/guides/client-developers.md` says so. The other
 * is the Rust reader/authoring binary (`tools/dclutch-cli`, cargo), and it is
 * the released artifact — signed cargo-dist tarballs and a shell installer —
 * so it keeps the bare executable name `dclutch`. This one is
 * `dclutch-terminal`.
 *
 * They used to declare the same executable name, and whichever came first on
 * `PATH` answered. The rename ends that. This list stays because the runbooks
 * a reader arrives with may still say the bare name for either program:
 * `docs/operators/author-a-ticket.md` teaches `dclutch ticket author` and
 * `apps/dclutch-web`'s General workspace teaches `dclutch general plan`, both
 * of which are the Rust binary, while `docs/guides/trencher.md` teaches
 * `dclutch-terminal markets ls`, which is this client.
 *
 * Listing them here implements none of them and weakens nothing: an unlisted
 * typo still gets the plain refusal plus usage. It turns one specific dead end
 * into a sentence naming the program that owns the verb. The reciprocal list
 * lives in `tools/dclutch-cli/src/main.rs`.
 */
export const RUST_READER_COMMANDS_V1: ReadonlyArray<string> = Object.freeze([
  'market',
  'capability',
  'ticket',
  'general',
  'fractional-retirement-next',
]);

/** The refusal for a verb this client does not have. */
export function unknownCommandV1(command: string): string {
  if (RUST_READER_COMMANDS_V1.includes(command)) {
    return `\`${command}\` is not a command of \`dclutch-terminal\`. This project ships two clients:`
      + ' this one is the terminal client (packages/dclutch-cli), and'
      + ` \`${command}\` belongs to the Rust reader/authoring binary \`dclutch\``
      + ' (tools/dclutch-cli), whose commands are market, capability, ticket,'
      + ' general and fractional-retirement-next. The usage below is this binary\'s.';
  }
  return `unknown command: ${command}`;
}

export async function run(argv: ReadonlyArray<string>, env: NodeJS.ProcessEnv, io: Io): Promise<number> {
  let parsed: ReturnType<typeof parseArgs<{ options: typeof FLAG_OPTIONS; allowPositionals: true }>>;
  try {
    parsed = parseArgs({ args: [...argv], options: FLAG_OPTIONS, allowPositionals: true, strict: true });
  } catch (error) {
    io.err(error instanceof Error ? error.message : String(error));
    io.err('run `dclutch-terminal --help` for usage');
    return 2;
  }
  const [command, ...rest] = parsed.positionals;
  if (parsed.values.help === true || command === undefined || command === 'help') {
    io.out(USAGE);
    return command === undefined && parsed.values.help !== true ? 2 : 0;
  }
  // These public mutation verbs are closed before `resolveContext`: even a
  // caller-named session, route, or key file is outside their reachable set.
  if (command === 'buy' || command === 'sell') {
    try {
      return await tradeCommand(command);
    } catch (error) {
      return fail(io, error);
    }
  }
  const context = resolveContext(parsed.values, env);
  try {
    switch (command) {
      case 'markets': {
        const sub = rest[0];
        if (sub === 'ls') return await marketsLs(context, io);
        if (sub === 'show' && rest[1] !== undefined) return await marketsShow(context, io, rest[1]);
        io.err('usage: dclutch-terminal markets ls | dclutch-terminal markets show <address>');
        return 2;
      }
      case 'portfolio':
        return await portfolio(context, io, rest[0], env);
      case 'intent':
        return await intentCommand(context, io, rest[0], env);
      case 'offer':
        return await offerCommand(context, io, rest[0], env);
      case 'route':
        return await routeCommand(context, io, rest[0], env);
      case 'product':
        return await productCommand(context, io, rest[0], env);
      case 'spine':
        return await spine(context, io, rest[0], env);
      case 'redeem':
        return await redeem(context, io, env);
      case 'found':
        return await found(context, io, env);
      case 'join':
        return await join(context, io, env);
      case 'walk':
        return await walk(context, io, env);
      case 'refusal':
        return refusal(io, rest);
      default:
        io.err(unknownCommandV1(command));
        io.err(USAGE);
        return 2;
    }
  } catch (error) {
    return fail(io, error);
  }
}

const entry = process.argv[1];
if (entry !== undefined && (entry.endsWith('dclutch-terminal.mjs') || entry.endsWith('main.ts') || entry.endsWith('dclutch-terminal'))) {
  run(process.argv.slice(2), process.env, STDIO).then((code) => {
    process.exitCode = code;
  });
}
