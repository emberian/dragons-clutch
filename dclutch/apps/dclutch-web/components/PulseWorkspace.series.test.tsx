import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import published from '@/public/simulator-series.json';
import example from '@/fixtures/simulator-status.example.json';
import { marketEditorialV1 } from '@/lib/marketRegistry';
import { holdingsReadingV1, isCompleteSetV1, lawBandCyclesV1, parseSimulatorSeriesV1 } from '@/lib/simulatorSeries';
import { parseSimulatorStatusV1 } from '@/lib/simulatorStatus';

import PulseWorkspace, { RecordedCycles, WhoIsHolding } from './PulseWorkspace';

/**
 * The time axis, on the surface that owns it.
 *
 * The series is a SNAPSHOT: it is captured by hand and committed, because
 * `git archive` is the only path to the live host. Every claim made here is
 * therefore about a record, and the thing most worth pinning is that the page
 * says so — a line drawn from a captured file must never read as a live feed.
 */
const series = parseSimulatorSeriesV1(published);
const status = parseSimulatorStatusV1(example);

describe('the pulse surface, with a recorded run', () => {
  const html = renderToStaticMarkup(
    <PulseWorkspace preloaded={{ kind: 'loaded', status }} preloadedSeries={{ kind: 'loaded', series }} />,
  );

  it('draws the run against time, which no other chart on this site does', () => {
    expect(html).toContain('What the run looked like over time');
    expect(html).toContain('<polyline');
    expect(html).toContain(`cycle ${series.points[0].cycle}`);
  });

  it('says in numbers what the drawn window covers, instead of in adjectives', () => {
    expect(html).toContain(`${series.points.length} recorded boundaries covering`);
    expect(html).toContain('slots of chain');
    expect(html).toContain(`census file ${series.censusFile}`);
  });

  it('draws the collateral coverage and the spend, which the producer had starved', () => {
    // Both charts are library functions the /campaign surface has drawn since
    // v3. /pulse could draw neither, and the reason was upstream of this file:
    // scripts/simulator-series.mjs dropped `mint_supply` and `payer_lamports`
    // before they reached the artifact. Pinned on the SHIPPED capture, so the
    // day a producer stops carrying them this goes red here rather than the
    // charts quietly vanishing from the page.
    expect(html).toContain('The collateral, and everything the census could find of it');
    expect(html).toContain('the collateral Mint’s whole supply');
    expect(html).toContain('What the run has spent');
    expect(html).toContain('lamports the fee payer has spent since the first boundary');
  });

  it('tells the reader the line is a record, not a feed', () => {
    expect(html).toContain('The run continues past the last point; this page does not.');
    expect(html).toContain('the last write before publication');
  });

  it('reports the ledger checks across every drawn cycle, and whether they held', () => {
    // Which sentence is right is the record's to decide, not this case's: a
    // capture with no violation says the ledger held every time, and one with
    // a violation must say how many did not. Demanding the first outright made
    // this a case that only a spotless capture could pass, which is a case
    // that fails the day the census earns its keep.
    const broken = series.points.reduce((sum, point) => sum + point.checksBroken, 0);
    expect(html).toContain(broken === 0 ? 'the ledger was re-checked' : 'did not hold');
    if (broken === 0) expect(html).toContain('held every time');
    else expect(html).toContain(`${broken} check${broken === 1 ? '' : 's'} did not hold`);
  });

  /**
   * The heartbeat exists because everything ELSE on this page is honestly
   * still. A census-only run signs nothing, so the market's quantities have no
   * business moving — and a page that draws only those reads as a dead one
   * while the chain underneath it is plainly alive. These pin that the two
   * moving quantities are actually on the page, and that the rate beside them
   * is presented as measured rather than as a constant somebody looked up.
   */
  it('draws the two quantities that are actually moving', () => {
    expect(html).toContain('The heartbeat');
    expect(html).toContain('slots the chain advanced');
    expect(html).toContain('Chain slots covered');
    // The cadence is the half that depends on the record carrying instants,
    // and a chained census cannot attribute them (see simulatorSeries.test.ts).
    // So the page either draws the seconds or says why it has none — what it
    // may never do is leave the reader with an unexplained gap.
    const timed = series.points.every((point) => point.recordedAt !== null);
    expect(html).toContain(timed ? 'seconds between recordings' : 'Some cycles recorded no timestamp');
  });

  it('says the slot rate was measured here rather than looked up', () => {
    expect(html).toContain('Measured slot rate');
    const timed = series.points.every((point) => point.recordedAt !== null);
    expect(html).toContain(timed
      ? 'measured here, not a published constant'
      : 'the run did not record enough instants to divide by');
  });

  /**
   * The law band replaced a sparkline of how MANY checks held. The count was
   * true and shapeless; what a reader needs is which law and what it compared.
   */
  it('gives every conservation law its name, its verdict and its own sentence', () => {
    // The count in the heading is the RECORD's, not the page's: the census
    // gained L8 and the heading still said seven, which is the shape of wrong
    // that no decoder can catch. It is derived now, so this asserts the
    // derivation rather than a number.
    expect(html).toContain(`The ${series.lawIds.length} checks, after every boundary`);
    for (const id of series.lawIds) expect(html).toContain(`>${id}<`);
    // The census writes its sentences with real comparison operators in them
    // ("Hoard ... >= worst outcome ..."), which is exactly the phrasing worth
    // showing and exactly what markup escaping touches. Escape, never soften.
    const escaped = (text: string) => text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
    for (const law of series.laws) expect(html).toContain(escaped(law.detail));
  });

  it('carries the law verdicts in words and glyphs, never in color alone', () => {
    expect(html).toContain('did not apply');
    expect(html).toContain('did not hold');
    // Every column of the band says its own verdict in words, so a reader who
    // separates neither hue still gets the answer for every cycle drawn.
    for (const cycle of lawBandCyclesV1(series)) expect(html).toContain(`cycle ${cycle} — `);
  });

  /**
   * A law's gloss is this site's editorial about what the law ASKS; the
   * sentence beside it is the census's about what it FOUND. The page must
   * never be the author of the second one.
   */
  it('keeps this site’s gloss on a law apart from the census’s own finding', () => {
    expect(html).toContain('is this site&#x27;s gloss on what that law is for');
    expect(html).toContain('full collateralisation');
  });

  /**
   * The truth about this run today: no trade has landed, so every claim line
   * is flat. The chart must say that in words rather than let a reader read a
   * flat line as an absence of data — and it must NOT be hidden for being
   * uneventful, because "nothing has traded yet" is the most load-bearing fact
   * on the page.
   */
  it('names a flat line as flat instead of hiding the chart or implying a movement', () => {
    const flat = series.points.every((point) => point.supply.every((atoms, cell) => atoms === series.points[0].supply[cell]));
    if (!flat) return;
    // The note used to read "no trade has landed in this run yet", which was
    // an inference from a flat ISSUED-SUPPLY line and became false the day a
    // trade landed: a Direct fill moves claims between two positions and
    // issues none, so this line is flat across a real crossing. The note says
    // what the line means now, and points at the table that does move.
    expect(html).toContain('unchanged at every recorded boundary');
    expect(html).toContain('A Direct fill MOVES claims between two positions');
    expect(html).not.toContain('no trade has landed in this run yet');
  });

  /**
   * The market-data ban, applied where it actually means something.
   *
   * A market's own outcome names are allowed to contain a money threshold —
   * "Below $120" is what this market ASKS, and refusing the dollar sign there
   * would be refusing the question rather than refusing an invented metric.
   * So the editorial names are subtracted first, and what remains is this
   * page's own prose. That prose may not carry a figure the chain does not
   * store.
   *
   * Renegotiated 2026-08-31: the sparkline caption used to end "not a forecast
   * and not a rate", and this array subtracted that exact sentence before
   * scanning — a disclaimer written to be exempt from the scan that forbids
   * it. The caption is deleted, so the subtraction is too, and the scan now
   * runs over the whole caption. Stricter, not looser.
   */
  it('never dresses a recorded run in market-data vocabulary of its own', () => {
    const editorial = series.market === null ? null : marketEditorialV1(series.market);
    const subtract = [...(editorial?.outcomes ?? [])];
    let remainder = html;
    for (const phrase of subtract) {
      expect(remainder).toContain(phrase);
      remainder = remainder.split(phrase).join('');
    }
    for (const forbidden of ['volume', 'Volume', 'odds', 'probability', 'Probability', 'TVL', 'APR', 'APY', '$', 'price', 'Price']) {
      expect(remainder).not.toContain(forbidden);
    }
  });
});

/**
 * The table a prediction market usually calls a leaderboard.
 *
 * Today the record holds one founding position and two funded participants who
 * have not traded, so there is nothing to rank — and the risk is entirely that
 * the page implies a contest anyway. These pin the two honesty rules: a
 * ranking of one is never presented as a ranking, and holding the token a
 * market settles in is never presented as holding a claim on its answer.
 */
describe('who is in the market', () => {
  const html = renderToStaticMarkup(<WhoIsHolding series={series} />);

  it('lists every position with its exact claims, largest first', () => {
    for (const position of series.positions) {
      expect(html).toContain(position.label);
      expect(html).toContain(`<td>${position.totalClaims}</td>`);
    }
  });

  it('refuses to call a single position a ranking', () => {
    if (series.positions.length !== 1) return;
    expect(html).toContain('There is nothing here to rank yet.');
    expect(html).not.toContain('leaderboard');
    expect(html).not.toContain('rank 1');
  });

  it('names a complete set for what it is, because it is the same whatever happens', () => {
    if (!series.positions.every(isCompleteSetV1)) return;
    expect(html).toContain('complete set');
    expect(html).toContain('worth the same whatever the answer turns out to be');
  });

  it('never lets holding collateral read as holding a claim on the answer', () => {
    expect(html).toContain('only a position holds claims on the answer');
  });

  it('marks the operator’s labels and this site’s gloss as what they each are', () => {
    // Renegotiated 2026-08-31: the note used to open by attributing the
    // account names to the run operator and the vault gloss to this site.
    // Deleted; what survives is the one distinction a reader can act on.
    expect(html).not.toContain('not anything the chain stores');
    expect(html).toContain('Collateral holders hold the token the market settles in');
  });
});

describe('what may be said about who holds what', () => {
  const position = (label: string, claims: ReadonlyArray<string>) => ({
    label,
    address: null,
    lamports: null,
    claims,
    total_claims: claims.reduce((sum, atoms) => sum + BigInt(atoms), 0n).toString(),
  });
  const withPositions = (positions: ReadonlyArray<unknown>) => parseSimulatorSeriesV1({
    ...(published as Record<string, unknown>),
    positions,
  });

  it('says there is nobody to list when no position was recorded', () => {
    const reading = holdingsReadingV1(withPositions([]));
    expect(reading.rankable).toBe(false);
    expect(reading.sentence).toContain('nobody to list');
  });

  it('says one position cannot be ranked', () => {
    // Built here rather than taken from the published capture. This case is
    // about the RULE — one position is not an ordering — and reading it off
    // the shipped artifact made it a case about how many positions cohort-12
    // happened to record; it went red the hour a second holder appeared.
    const reading = holdingsReadingV1(withPositions([position('a', ['4', '4'])]));
    expect(reading.positionCount).toBe(1);
    expect(reading.rankable).toBe(false);
  });

  /**
   * The moment this becomes a leaderboard is the moment somebody trades. It
   * still refuses to call a claim count a score.
   */
  it('becomes an ordering once more than one position exists, and still is not a score', () => {
    const reading = holdingsReadingV1(withPositions([position('a', ['3', '1']), position('b', ['1', '1'])]));
    expect(reading.positionCount).toBe(2);
    expect(reading.rankable).toBe(true);
    expect(reading.sentence).toContain('not a score and not a return');
  });

  it('recognises a complete set, and an uneven position as not one', () => {
    expect(holdingsReadingV1(withPositions([position('a', ['5', '5', '5'])])).allComplete).toBe(true);
    expect(holdingsReadingV1(withPositions([position('a', ['5', '4', '5'])])).allComplete).toBe(false);
  });
});

describe('the recorded-run section on its own', () => {
  it('says nothing was published rather than drawing an empty frame', () => {
    const html = renderToStaticMarkup(<RecordedCycles read={{ kind: 'absent' }} />);
    expect(html).not.toContain('<svg');
    expect(html).toContain('there is no line to draw and nothing below is a zero');
  });

  it('shows a refusal as a refusal, with the field that failed', () => {
    const html = renderToStaticMarkup(<RecordedCycles read={{ kind: 'refused', reason: 'cluster must be local or devnet' }} />);
    expect(html).toContain('it did not decode');
    expect(html).toContain('cluster must be local or devnet');
  });

  it('says it is looking before the read settles, and claims nothing', () => {
    const html = renderToStaticMarkup(<RecordedCycles read={null} />);
    expect(html).toContain('Looking for a recorded run');
    expect(html).not.toContain('<svg');
  });

  it('counts cycles it left out rather than implying the run began at the first drawn point', () => {
    const trimmed = parseSimulatorSeriesV1({
      ...(published as Record<string, unknown>),
      points_omitted_before: 7,
      points: (published as { points: ReadonlyArray<unknown> }).points.slice(-3),
    });
    const html = renderToStaticMarkup(<RecordedCycles read={{ kind: 'loaded', series: trimmed }} />);
    expect(html).toContain('7 earlier boundaries are counted but not drawn');
  });
});
