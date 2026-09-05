import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

import { describe, expect, it } from 'vitest';

import { renderedRecords, magicText, specForMagic } from './explorer/accountRecords';
import { STATE_MACHINE_RECORDS_V1 } from '@dclutch/sdk/generated/stateMachinesV1';

/**
 * Reservations are declared, not conserved — and the browser must say so.
 *
 * THE MEASURED FACT. A registered SELL escrows nothing. `register_intent_v2`
 * writes `reserved_claims = maximum_fill` into the record, and the Sell
 * `EffectProgramV4` has **no routes at all** — its own emitter says so:
 * "No routes: a Sell invokes no child program at all"
 * (`crates/dclutch-trading/src/registered_effect_artifacts_v4.rs`). The
 * Sell profile's coordinates name neither a Claims Position nor the aggregate,
 * so nothing is moved and nothing is checked against a balance. A registered
 * BUY is genuinely different: it moves `reserved_collateral` into a
 * record-keyed vault through real Custody routes. The asymmetry is real and
 * load-bearing.
 *
 * Three consequences a client can get badly wrong:
 *
 *   - one maker may register any number of Sell records each naming the whole
 *     supply, because the Position is not in the frame to stop them;
 *   - a resting Sell can become unfillable for free, at the taker's compute
 *     expense;
 *   - `sum(reserved_claims)` over live records is NOT bounded by supply and is
 *     NOT a conservation quantity. Totalling it and rendering it as "reserved",
 *     "locked" or "backed" states something false.
 *
 * Conservation is enforced at FILL, where `claim_custody_debit: fill` moves
 * real claims and rolls the whole transaction back if they are gone
 * (`successor.rs`, `DirectSideV2::Sell`). That is the fact a surface may lean
 * on. The reservation is not.
 *
 * SO THIS FILE IS A VOCABULARY GATE, in the same shape as the one that forbids
 * `verified` on a signature chip the browser only shape-checked. A status model
 * cannot catch this: a capability whose venue and authority are perfectly
 * honest can still sit above a NUMBER that lies. What is conserved and what is
 * merely declared is a second axis, and it needs its own guard.
 */

const webRoot = fileURLToPath(new URL('..', import.meta.url));

/**
 * The surfaces that can show a maker's declared size.
 *
 * Scoped rather than swept: `Reserved` is a legitimate field label for a
 * zero-enforced byte span across the explorer's whole render map, and a gate
 * that failed on those would be deleted within a week.
 */
const TRADE_SURFACES = [
  'components/MarketTradePanel.tsx',
  'components/DirectTradeWorkspace.tsx',
  'components/trade/TicketCard.tsx',
  'components/trade/TicketBoard.tsx',
  'components/trade/MakerOfferComposer.tsx',
  'components/trade/PreviewReceipt.tsx',
  // The ticket vocabulary lives in the SDK now; the browser imports it whole.
  '../../packages/dclutch-sdk/lib/directTicket.ts',
  'lib/tradeFlowSteps.ts',
];

/**
 * Words that assert someone has set something aside. None of them is true here.
 *
 * Matched on word boundaries, because `blocked` contains `locked` and a gate
 * that fails on the trade flow's own step vocabulary is a gate somebody turns
 * off rather than a gate that works.
 */
const SOLVENCY_WORDS = [
  'reserved',
  'locked',
  'backed by',
  'escrowed',
  'set aside for',
  'collateralised',
  'collateralized',
  'guaranteed',
  'held for you',
];

/** The names a declared ceiling travels under in this client. */
const DECLARED_QUANTITIES = ['maximumFill', 'reservedClaims', 'reserved_claims'];

function sourceFiles(directory: string): ReadonlyArray<string> {
  const found: string[] = [];
  const walk = (absolute: string) => {
    for (const entry of readdirSync(absolute)) {
      if (entry === 'node_modules' || entry === 'dist' || entry === 'generated' || entry.startsWith('.')) continue;
      const child = join(absolute, entry);
      if (statSync(child).isDirectory()) walk(child);
      else if (/\.tsx?$/.test(entry) && !entry.includes('.test.')) found.push(child);
    }
  };
  walk(join(webRoot, directory));
  return found;
}

/** Prose a reader can actually see: string literals and JSX text, never identifiers. */
function proseOf(source: string): string {
  const strings = [...source.matchAll(/'((?:[^'\\\n]|\\.)*)'|"((?:[^"\\\n]|\\.)*)"/g)]
    .map((match) => match[1] ?? match[2] ?? '');
  const jsxText = [...source.matchAll(/>([^<>{}]{3,})</g)].map((match) => match[1]);
  return [...strings, ...jsxText].join('\n').toLowerCase();
}

describe('a declared ceiling is never rendered as solvency', () => {
  it('has surfaces to speak about at all', () => {
    for (const path of TRADE_SURFACES) {
      expect(() => readFileSync(join(webRoot, path), 'utf8'), `${path} is gated here and does not exist`).not.toThrow();
    }
  });

  it('uses no word that says a maker set claims aside', () => {
    for (const path of TRADE_SURFACES) {
      const prose = proseOf(readFileSync(join(webRoot, path), 'utf8'));
      for (const word of SOLVENCY_WORDS) {
        expect(
          new RegExp(`\\b${word}\\b`).test(prose),
          `${path} says "${word}" to a reader. A registered sell escrows nothing — its effect program has no routes and its profile names no Claims Position — so the maximum fill is a ceiling on what the offer may ever trade, not a quantity anyone has set aside. Conservation happens at the fill, in claim_custody_debit.`,
        ).toBe(false);
      }
    }
  });

  it('says out loud that the ceiling is not a balance', () => {
    // The positive half. Forbidding the false sentence is not the same as
    // telling the truth, and this line is the one a taker reads before sizing.
    const panel = readFileSync(join(webRoot, 'components/MarketTradePanel.tsx'), 'utf8');
    expect(panel).toContain('ceiling, not a balance');
    expect(panel).toContain('Nothing');
    expect(panel).toContain('the chain moves the maker');
  });

  it('totals no declared ceiling anywhere in the client', () => {
    // `sum(reserved_claims)` is not bounded by supply, so no total of it means
    // anything. The regex looks for the shapes an accumulation takes rather
    // than for a particular helper, because the next one will be written by
    // hand in a component.
    const offenders: string[] = [];
    for (const directory of ['lib', 'components', 'app']) {
      for (const absolute of sourceFiles(directory)) {
        const source = readFileSync(absolute, 'utf8');
        for (const quantity of DECLARED_QUANTITIES) {
          const accumulation = new RegExp(`(reduce\\([^)]*${quantity}|${quantity}[^\\n]*\\+=|\\+=[^\\n]*${quantity}|total[A-Za-z]*\\s*[+]?=[^\\n]*${quantity})`);
          if (accumulation.test(source)) offenders.push(`${relative(webRoot, absolute)} totals ${quantity}`);
        }
      }
    }
    expect(
      offenders,
      'A sum of declared reservations is not a conservation quantity and is not bounded by supply. Show the per-record ceiling, or show what the chain actually holds.',
    ).toEqual([]);
  });

  it('describes the registered records without borrowing the word', () => {
    let checked = 0;
    for (const spec of renderedRecords()) {
      if (spec.family !== 'Direct') continue;
      const prose = `${spec.summary} ${spec.note ?? ''}`.toLowerCase();
      for (const word of SOLVENCY_WORDS) {
        expect(
          new RegExp(`\\b${word}\\b`).test(prose),
          `the explorer's ${magicText(spec.magic)} spec says "${word}"; the record declares a ceiling and holds nothing.`,
        ).toBe(false);
      }
      checked += 1;
    }
    // A family filter that stopped matching would pass this silently, which is
    // exactly how the arm below came to be needed.
    expect(checked).toBeGreaterThan(0);
  });

  /**
   * The same vocabulary, over the eight persisted state machines.
   *
   * The arm above filters `family === 'Direct'`, and the machine records carry
   * their MACHINE label as their family (`direct-root`, `projected-custody`),
   * so every one of their summaries fell outside it the moment they were
   * added. That was checked by eye once, which is not a gate; this is.
   *
   * The exemption is derived rather than listed: a word that is one of this
   * machine's own state NAMES is the machine speaking, not a claim about
   * solvency. `projected-custody` genuinely has a `HoardLocked` and
   * `dealer-checkpoint` a `Reserved`, and a summary that cannot name its own
   * states is not a summary. The exemption is per machine and reaches no
   * further, so a Source summary still may not say `locked`.
   */
  it('describes every persisted state machine without borrowing the word', () => {
    let checked = 0;
    for (const row of STATE_MACHINE_RECORDS_V1) {
      const spec = specForMagic(row.magic);
      expect(spec, `${row.machine} is not rendered`).not.toBeNull();
      if (spec === null) continue;
      const names = row.states.map((state) => state.state.toLowerCase());
      const prose = `${spec.summary} ${spec.note ?? ''}`.toLowerCase();
      for (const word of SOLVENCY_WORDS) {
        if (names.some((name) => name.includes(word))) continue;
        expect(
          new RegExp(`\\b${word}\\b`).test(prose),
          `the explorer's ${row.machine} spec says "${word}", and no state of that machine is named for it. A discriminant records where a lifecycle has got to; it holds nothing.`,
        ).toBe(false);
      }
      checked += 1;
    }
    expect(checked).toBe(STATE_MACHINE_RECORDS_V1.length);
  });
});
