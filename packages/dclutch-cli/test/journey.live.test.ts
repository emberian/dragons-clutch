import { readFileSync } from 'node:fs';

import { describe, expect, it } from 'vitest';

import { run } from '../src/main';

/**
 * The reader's CLI journey against a LIVE chain: discover → inspect →
 * tradability → portfolio, through the real `run()` dispatcher — the exact
 * code `dclutch` executes, with only stdout captured.
 *
 * Opt-in via DCLUTCH_JOURNEY_SESSION, the same session file the web journey
 * uses (dclutch-web-journey-session-v1). Without it this file skips; it never
 * invents a chain. Mutation verbs are deliberately absent here: `join` and
 * `redeem` mutate through their own journaled paths and `buy`/`sell` refuse
 * by design — this journey proves the READING surface tells one coherent
 * story about a real market.
 */

type JourneySessionV1 = Readonly<{
  schema: string;
  endpoint: string;
  programs: Readonly<{
    registry: string; core: string; trading: string; claims: string;
    custody: string; resolution: string; rent: string;
  }>;
  market?: string;
  wallet?: string;
}>;

const sessionPath = process.env.DCLUTCH_JOURNEY_SESSION;
const describeLive = sessionPath === undefined ? describe.skip : describe;

function session(): JourneySessionV1 {
  const value = JSON.parse(readFileSync(sessionPath!, 'utf8')) as JourneySessionV1;
  if (value.schema !== 'dclutch-web-journey-session-v1') throw new Error('journey session has another schema');
  return value;
}

async function dclutch(args: ReadonlyArray<string>): Promise<Readonly<{ code: number; out: string; err: string }>> {
  const { endpoint, programs } = session();
  const out: string[] = [];
  const err: string[] = [];
  const code = await run([
    '--rpc', endpoint,
    '--registry-program', programs.registry,
    '--core-program', programs.core,
    '--claims-program', programs.claims,
    '--trading-program', programs.trading,
    '--resolution-program', programs.resolution,
    '--custody-program', programs.custody,
    '--rent-credit-program', programs.rent,
    ...args,
  ], {}, { out: (line) => out.push(line), err: (line) => err.push(line) });
  return Object.freeze({ code, out: out.join('\n'), err: err.join('\n') });
}

describeLive('the CLI reading journey against the live chain', () => {
  it('discovers the market, inspects it, names its walls, and rolls up a portfolio', async () => {
    const { market, wallet } = session();

    const ls = await dclutch(['markets', 'ls']);
    expect(ls.code, ls.err).toBe(0);
    if (market !== undefined) expect(ls.out).toContain(market);

    const target = market ?? ls.out.match(/[1-9A-HJ-NP-Za-km-z]{32,44}/)?.[0];
    expect(target, 'no market to inspect: discovery printed none and the session names none').toBeDefined();

    const show = await dclutch(['markets', 'show', target!]);
    expect(show.code, show.err).toBe(0);
    expect(show.out).toContain(target!);

    const spine = await dclutch(['spine', '--market', target!]);
    expect(spine.code, spine.err).toBe(0);

    if (wallet !== undefined) {
      const portfolio = await dclutch(['portfolio', wallet]);
      expect(portfolio.code, portfolio.err).toBe(0);
    }

    console.log('--- markets ls ---\n' + ls.out);
    console.log('--- markets show ---\n' + show.out);
    console.log('--- spine ---\n' + spine.out);
  }, 120_000);
});
