/**
 * The share cards' TEXT, split out of `og-cards.sh` so something can run it.
 *
 * WHY THIS IS A MODULE AND NOT A `node -e`. The card generator is a shell
 * script `npm test` does not run, and its row derivation lived inside a `node
 * -e` string. On 2026-09-03 the registry stopped writing titles for live
 * markets, `entry.title.split` threw on the first of them, and the loop
 * produced NO cards at all — not a bad card, none. Nothing was red, because
 * nothing ran it. The fix was one `??`; the DEFECT was that a derivation with
 * no test sat inside a quoted string.
 *
 * So the derivation is here, pure, and `lib/ogCards.test.ts` runs it against
 * the shipped registry on every `npm test`. The shell script calls
 * `node scripts/og-cards.mjs --rows` and composes what comes back.
 *
 * THE SECOND LINE. A registry title splits on `" — "` into a large first line
 * and a quieter second, and that is still what an editorial title does. But
 * five live markets carry no title at all — the pages derive better ones off
 * each market's own partition — so all five fell back to the COORDINATE, and
 * five cards read `SOL/USD` with nothing under them and nothing between them.
 * A share card that cannot be told from four others is a share card that has
 * failed at the one job it has.
 *
 * The second line for those is DERIVED, from the two facts that actually
 * separate them: the market's phase, and the instant it settles or settled.
 * Neither is editorial and neither is in the registry, so they come from
 * `fixtures/og-card-facts.devnet.json`, staged from a finalized chain read. A market with no facts row gets no second line rather than an
 * invented one, exactly as before.
 *
 * @typedef {Readonly<{ phase: string, settledAtUnixSeconds?: string | null, windowEndUnixSeconds?: string | null }>} OgMarketFactsV1
 * @typedef {Readonly<{ markets?: Readonly<Record<string, OgMarketFactsV1>> }>} OgCardFactsV1
 * @typedef {Readonly<{ markets?: Readonly<Record<string, Readonly<{ title?: string | null, coordinate?: Readonly<{ label?: string | null }> | null }>>> }>} OgRegistryV1
 *
 * The fixture is STAGED BY THE LIVE TEST'S OWN READER, not by this file:
 * `lib/ogCards.live.test.ts` asserts every committed fact still agrees with a
 * finalized chain read, and rewrites the fixture under
 * `DCLUTCH_OG_FACTS_WRITE=1`. So the thing that authors the fact and the thing
 * that checks it are one reader, and a card whose phase has moved on goes red
 * instead of quietly shipping last week's answer.
 *
 *   node scripts/og-cards.mjs --rows            # TSV: address, lead, second
 */
import { readFileSync } from 'node:fs';

export const OG_CARD_FACTS_SCHEMA_V1 = 'dclutch-og-card-facts-v1';

const APP = new URL('../', import.meta.url);
const REGISTRY_URL = new URL('../../packages/dclutch-sdk/fixtures/market-registry.devnet.json', APP);
const FACTS_URL = new URL('fixtures/og-card-facts.devnet.json', APP);

/**
 * `YYYY-MM-DD HH:MM UTC`, the same instant format every market page prints.
 *
 * @param {string | number} unixSeconds
 * @returns {string}
 */
export function ogInstantV1(unixSeconds) {
  const seconds = Number(unixSeconds);
  if (!Number.isSafeInteger(seconds) || seconds <= 0) throw new Error(`og card instant is not a whole positive second count: ${unixSeconds}`);
  return `${new Date(seconds * 1000).toISOString().slice(0, 16).replace('T', ' ')} UTC`;
}

/**
 * The quieter second line for one market, or `''` when there is nothing true
 * to put there.
 *
 * Editorial first: a title that carries its own subtitle keeps it, because a
 * human wrote it about this market. Chain second. Nothing third — an empty
 * second line is the honest output for a market this cut has not read.
 */
export function ogSecondLineV1(titleRest, facts) {
  if (typeof titleRest === 'string' && titleRest.trim() !== '') return titleRest.trim();
  if (facts === undefined || facts === null) return '';
  const phase = facts.phase;
  if (phase === 'Terminal' || phase === 'Retiring' || phase === 'Retired') {
    return facts.settledAtUnixSeconds === null || facts.settledAtUnixSeconds === undefined
      ? `${phase === 'Terminal' ? 'Resolved' : 'Closed'} · no observation recorded`
      : `${phase === 'Terminal' ? 'Resolved' : 'Closed'} · ${ogInstantV1(facts.settledAtUnixSeconds)}`;
  }
  if (facts.windowEndUnixSeconds === null || facts.windowEndUnixSeconds === undefined) return phase === 'Open' ? 'Open' : String(phase);
  return `${phase === 'Open' ? 'Open' : phase} · settles ${ogInstantV1(facts.windowEndUnixSeconds)}`;
}

/**
 * One row per registry market: address, the large line, the quiet line.
 *
 * It throws on a row it cannot name at all rather than skipping it, because
 * skipping is what turned a broken row into an empty output directory.
 *
 * @param {OgRegistryV1} registry
 * @param {OgCardFactsV1 | null} [facts]
 * @returns {ReadonlyArray<Readonly<{ address: string, lead: string, second: string }>>}
 */
export function ogCardRowsV1(registry, facts = null) {
  const markets = registry?.markets;
  if (markets === null || typeof markets !== 'object') throw new Error('the market registry carries no markets object');
  const rows = Object.entries(markets).map(([address, entry]) => {
    const named = entry.title ?? entry.coordinate?.label ?? null;
    if (named === null || named === '') throw new Error(`${address} has neither a title nor a coordinate name, so it can carry no card`);
    const [lead, ...rest] = String(named).split(' — ');
    return Object.freeze({
      address,
      lead,
      second: ogSecondLineV1(rest.join(' — '), facts?.markets?.[address] ?? null),
    });
  });
  if (rows.length === 0) throw new Error('the market registry named no markets, so this run would write no cards');
  return Object.freeze(rows);
}

/** @returns {OgRegistryV1} */
export function readOgRegistryV1() {
  return JSON.parse(readFileSync(REGISTRY_URL, 'utf8'));
}

/**
 * The committed chain facts, or null where the fixture has not been staged.
 *
 * @returns {OgCardFactsV1 | null}
 */
export function readOgCardFactsV1() {
  let body;
  try { body = readFileSync(FACTS_URL, 'utf8'); } catch { return null; }
  const facts = JSON.parse(body);
  if (facts.schema !== OG_CARD_FACTS_SCHEMA_V1) throw new Error(`og card facts carry another schema: ${facts.schema}`);
  if (facts.cluster !== 'devnet') throw new Error(`og card facts are not devnet: ${facts.cluster}`);
  return facts;
}

if (process.argv[1] !== undefined && import.meta.url === new URL(`file://${process.argv[1]}`).href) {
  if (process.argv.includes('--rows')) {
    for (const row of ogCardRowsV1(readOgRegistryV1(), readOgCardFactsV1())) {
      process.stdout.write(`${row.address}\t${row.lead}\t${row.second}\n`);
    }
  } else {
    process.stderr.write('usage: node scripts/og-cards.mjs --rows\n');
    process.exit(2);
  }
}
