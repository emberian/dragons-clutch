import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import published from '@/public/simulator-series.json';
import example from '@/fixtures/simulator-status.example.json';
import { marketEditorialV1 } from '@/lib/marketRegistry';
import { parseSimulatorSeriesV1 } from '@/lib/simulatorSeries';
import { parseSimulatorStatusV1 } from '@/lib/simulatorStatus';

import PulseWorkspace, { RecordedCycles } from './PulseWorkspace';

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
    expect(html).toContain('cycle 1');
  });

  it('says in numbers what the drawn window covers, instead of in adjectives', () => {
    expect(html).toContain(`${series.points.length} recorded cycles covering`);
    expect(html).toContain('slots of chain');
    expect(html).toContain(`census file ${series.censusFile}`);
  });

  it('tells the reader the line is a record, not a feed', () => {
    expect(html).toContain('The run continues past the last point; this page does not.');
    expect(html).toContain('last write before the site was published');
  });

  it('reports the ledger checks across every drawn cycle, and whether they held', () => {
    expect(html).toContain('the ledger was re-checked');
    expect(html).toContain('held every time');
    expect(html).toContain('The ledger check, cycle by cycle');
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
    expect(html).toContain('no trade has landed in this run yet');
  });

  /**
   * The market-data ban, applied where it actually means something.
   *
   * A market's own outcome names are allowed to contain a money threshold —
   * "Below $120" is what this market ASKS, and refusing the dollar sign there
   * would be refusing the question rather than refusing an invented metric.
   * So the editorial names are subtracted first, exactly the way the shipped
   * disclaimers are, and what remains is this page's own prose. That prose may
   * not carry a figure the chain does not store.
   */
  it('never dresses a recorded run in market-data vocabulary of its own', () => {
    const editorial = series.market === null ? null : marketEditorialV1(series.market);
    const subtract = ['not a forecast and not a rate', ...(editorial?.outcomes ?? [])];
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
    expect(html).toContain('7 earlier cycles are counted but not drawn');
  });
});
