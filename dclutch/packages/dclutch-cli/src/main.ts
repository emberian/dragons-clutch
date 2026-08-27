/**
 * dclutch — the dClutch trader loop from a terminal.
 *
 * Command dispatch and flag parsing only; each command lives in
 * `commands/` and every chain fact it states comes through @dclutch/sdk,
 * whose generated modules are byte-gated against the protocol's own
 * authorities. Refusals render by NAME via the band registry.
 */
import { parseArgs } from 'node:util';

import { resolveContext } from './context';
import { found } from './commands/found';
import { marketsLs, marketsShow } from './commands/markets';
import { portfolio } from './commands/portfolio';
import { redeem } from './commands/redeem';
import { refusal } from './commands/refusal';
import { spine } from './commands/spine';
import { intentCommand, tradeCommand } from './commands/trade';
import { walk } from './commands/walk';
import { fail, STDIO, type Io } from './output';

const USAGE = `dclutch — the dClutch trader loop from a terminal

usage: dclutch [global flags] <command> [args]

commands:
  markets ls                       enumerate and decode markets under the Core program
  markets show <address>           one market, in full, at one finalized floor
  portfolio [owner]                indexer-free position rollup (owner defaults to --keypair)
  intent sell|buy                  sign one Direct compact intent to a file (--out)
  buy                              cross a sell intent (--take, or --counter-keypair) and submit
  sell                             cross a buy intent (--take, or --counter-keypair) and submit
  spine                            is this market Direct-tradable now, and which walls stand (--market)
  redeem                           create the Claims-role Custody replay; state the payout honestly
  found                            drive the run-spec founding producer (--spec; --demo to preview)
  walk                             the funded failure walk: commit a passed deadline, collect the bounty
  refusal <code...>                name any custom program error via the band registry

global flags:
  --rpc <url>            JSON-RPC endpoint (default $DCLUTCH_RPC, then the session file, then http://127.0.0.1:20890/)
  --session <json>       a run spec, run evidence, or dclutch session file carrying program ids + markets
  --keypair <path>       Solana JSON keypair; also $DCLUTCH_KEYPAIR (never a default wallet path)
  --json                 machine-readable output where a command supports it
  --dry-run              build and print, sign and submit nothing

program ids come from --session or explicit --core-program/--claims-program/... flags.
refusal codes: band = code >> 12; codes below 0x1000 are provably not dClutch's. See docs/guides/client-developers.md.`;

const FLAG_OPTIONS = {
  rpc: { type: 'string' },
  session: { type: 'string' },
  keypair: { type: 'string' },
  payer: { type: 'string' },
  json: { type: 'boolean' },
  'dry-run': { type: 'boolean' },
  demo: { type: 'boolean' },
  spec: { type: 'string' },
  'keypair-seed': { type: 'string' },
  'session-out': { type: 'string' },
  'bootstrap-bin': { type: 'string' },
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
  fill: { type: 'string' },
  price: { type: 'string' },
  lifecycle: { type: 'string' },
  nonce: { type: 'string' },
  'valid-from': { type: 'string' },
  'valid-through': { type: 'string' },
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

export async function run(argv: ReadonlyArray<string>, env: NodeJS.ProcessEnv, io: Io): Promise<number> {
  let parsed: ReturnType<typeof parseArgs<{ options: typeof FLAG_OPTIONS; allowPositionals: true }>>;
  try {
    parsed = parseArgs({ args: [...argv], options: FLAG_OPTIONS, allowPositionals: true, strict: true });
  } catch (error) {
    io.err(error instanceof Error ? error.message : String(error));
    io.err('run `dclutch --help` for usage');
    return 2;
  }
  const [command, ...rest] = parsed.positionals;
  if (parsed.values.help === true || command === undefined || command === 'help') {
    io.out(USAGE);
    return command === undefined && parsed.values.help !== true ? 2 : 0;
  }
  const context = resolveContext(parsed.values, env);
  try {
    switch (command) {
      case 'markets': {
        const sub = rest[0];
        if (sub === 'ls') return await marketsLs(context, io);
        if (sub === 'show' && rest[1] !== undefined) return await marketsShow(context, io, rest[1]);
        io.err('usage: dclutch markets ls | dclutch markets show <address>');
        return 2;
      }
      case 'portfolio':
        return await portfolio(context, io, rest[0], env);
      case 'intent':
        return await intentCommand(context, io, rest[0], env);
      case 'buy':
        return await tradeCommand(context, io, 'buy', env);
      case 'sell':
        return await tradeCommand(context, io, 'sell', env);
      case 'spine':
        return await spine(context, io, rest[0], env);
      case 'redeem':
        return await redeem(context, io, env);
      case 'found':
        return await found(context, io, env);
      case 'walk':
        return await walk(context, io, env);
      case 'refusal':
        return refusal(io, rest);
      default:
        io.err(`unknown command: ${command}`);
        io.err(USAGE);
        return 2;
    }
  } catch (error) {
    return fail(io, error);
  }
}

const entry = process.argv[1];
if (entry !== undefined && (entry.endsWith('dclutch.mjs') || entry.endsWith('main.ts') || entry.endsWith('dclutch'))) {
  run(process.argv.slice(2), process.env, STDIO).then((code) => {
    process.exitCode = code;
  });
}
