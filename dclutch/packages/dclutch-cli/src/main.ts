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
import { FLAG_OPTIONS, USAGE } from './usage';

/**
 * Re-exported at the name its importers already use.
 *
 * `test/cli.test.ts` holds the help page to this table, and the table now
 * lives beside the page it renders (`usage.ts`) rather than beside the
 * dispatcher that parses with it.
 */
export { FLAG_OPTIONS };

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
