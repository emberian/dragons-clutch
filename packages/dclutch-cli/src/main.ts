/**
 * dclutch — the dClutch terminal client.
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

const USAGE = `dclutch — the dClutch terminal client

usage: dclutch [global flags] <command> [args]

commands:
  markets ls                       enumerate and decode markets under the Core program
  markets show <address>           one market, in full, at one finalized floor
  portfolio [owner]                indexer-free position rollup (owner defaults to --keypair)
  intent sell|buy                  authenticate a route and sign one off-chain Direct intent (--out; never submits)
  buy                              disabled: refuses before context, keys, signing, or RPC access
  sell                             disabled: refuses before context, keys, signing, or RPC access
  spine                            is this market Direct-tradable now, and which walls stand (--market)
  redeem                           resume or finalize one exact wallet payout
  found                            drive the run-spec founding producer (--spec; --demo to preview)
  walk                             preview the funded failure walk (--dry-run required; submission disabled)
  refusal <code...>                name any custom program error via the band registry

global flags:
  --rpc <url>            JSON-RPC endpoint (default $DCLUTCH_RPC, then the session file, then http://127.0.0.1:20890/)
  --session <json>       a run spec, run evidence, or dclutch session file carrying program ids + markets
  --keypair <path>       Solana JSON keypair; also $DCLUTCH_KEYPAIR (never a default wallet path)
  --json                 machine-readable output where a command supports it
  --dry-run              where supported, build and print without signing or submitting; never enables buy/sell
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
  --i-mean-devnet <hash> name devnet by its full genesis hash for redeem and walk

program ids come from --session or explicit --core-program/--claims-program/... flags.
refusal codes: band = code >> 12; codes below 0x1000 are provably not dClutch's. See docs/guides/client-developers.md.`;

const FLAG_OPTIONS = {
  rpc: { type: 'string' },
  session: { type: 'string' },
  keypair: { type: 'string' },
  payer: { type: 'string' },
  recipient: { type: 'string' },
  json: { type: 'boolean' },
  'dry-run': { type: 'boolean' },
  demo: { type: 'boolean' },
  spec: { type: 'string' },
  'keypair-seed': { type: 'string' },
  'session-out': { type: 'string' },
  'bootstrap-bin': { type: 'string' },
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
        io.err('usage: dclutch markets ls | dclutch markets show <address>');
        return 2;
      }
      case 'portfolio':
        return await portfolio(context, io, rest[0], env);
      case 'intent':
        return await intentCommand(context, io, rest[0], env);
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
