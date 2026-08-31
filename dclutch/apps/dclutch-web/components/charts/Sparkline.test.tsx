import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import Sparkline from './Sparkline';

/**
 * The time-axis chart. Its risks are not the ones a bar chart has: a line
 * invites a reader to see a trend, so the things pinned here are the ones that
 * would let it draw a trend that did not happen — a rescaled flat series, a
 * line drawn from ragged inputs, or a number rounded on its way to the page.
 */
describe('Sparkline', () => {
  const lines = [
    { label: 'claim 0', values: ['10', '12', '11'] },
    { label: 'claim 1', values: ['90', '88', '89'] },
  ];
  const xLabels = ['cycle 1', 'cycle 2', 'cycle 3'];
  const html = renderToStaticMarkup(
    <Sparkline lines={lines} xLabels={xLabels} caption="Issued claims at every recorded cycle." unit="atoms" />,
  );

  it('draws one band per line and labels both ends of the axis', () => {
    expect(html).toContain('<polyline');
    expect((html.match(/<polyline/g) ?? []).length).toBe(2);
    expect(html).toContain('claim 0');
    expect(html).toContain('claim 1');
    expect(html).toContain('cycle 1');
    expect(html).toContain('cycle 3');
  });

  it('shows the newest point by default, because a line is read at its end', () => {
    // The readout leads with the last x label and the last value of each line.
    expect(html).toContain('cycle 3 · claim 0 11 · claim 1 89 atoms');
  });

  it('carries every drawn value exactly, in a table, never rounded', () => {
    expect(html).toContain('Exact numbers');
    for (const value of ['10', '12', '11', '90', '88', '89']) {
      expect(html).toContain(`<td>${value}</td>`);
    }
    expect(html).toContain('3 points drawn, from cycle 1 to cycle 3');
  });

  /**
   * The failure this guards against: a chart with a shared scale can be made
   * to look eventful by rescaling each band to its own range. These two lines
   * are far apart, and the low line must not be redrawn as if it were the high
   * one — so both must sit on the SAME domain.
   */
  it('draws every line on one shared scale', () => {
    const points = [...html.matchAll(/<polyline points="([^"]+)"/g)].map((m) => m[1]);
    expect(points).toHaveLength(2);
    const ys = points.map((series) => series.split(' ').map((pair) => Number(pair.split(',')[1])));
    // Line 1's values (88..90) are all above line 0's (10..12) on a shared
    // scale: every y of the second band, measured from its own band top, must
    // be higher on the plot than every y of the first.
    const bandOffset = 37 + 9; // ROW_HEIGHT + ROW_GAP
    const normalized = ys.map((band, index) => band.map((y) => y - index * bandOffset));
    expect(Math.max(...normalized[1])).toBeLessThan(Math.min(...normalized[0]));
  });

  it('says a flat series is flat instead of manufacturing a wiggle', () => {
    const flat = renderToStaticMarkup(<Sparkline
      lines={[{ label: 'claim 0', values: ['500', '500', '500'] }]}
      xLabels={xLabels}
      caption="Issued claims at every recorded cycle."
      flatNote="unchanged across every recorded cycle"
    />);
    expect(flat).toContain('unchanged across every recorded cycle');
    // Every drawn y is identical: a flat line, sitting on its band's middle.
    const drawn = /<polyline points="([^"]+)"/.exec(flat)?.[1] ?? '';
    const ys = new Set(drawn.split(' ').map((pair) => pair.split(',')[1]));
    expect(ys.size).toBe(1);
  });

  /**
   * The empty state is a caption and nothing else — no axes, no zero line, no
   * empty frame that reads as "we measured nothing". This is the same contract
   * CellStrip keeps, and it is pinned the same way.
   */
  it('draws no chart at all when there is nothing to draw', () => {
    const empty = renderToStaticMarkup(
      <Sparkline lines={[]} xLabels={[]} caption="c" emptyReason="No run has been recorded for this market." />,
    );
    expect(empty).not.toContain('<svg');
    expect(empty).toContain('No run has been recorded for this market.');
  });

  it('refuses ragged input rather than drawing a line through a gap', () => {
    const ragged = renderToStaticMarkup(<Sparkline
      lines={[{ label: 'claim 0', values: ['1', '2'] }, { label: 'claim 1', values: ['1'] }]}
      xLabels={['cycle 1', 'cycle 2']}
      caption="c"
      emptyReason="These points do not line up."
    />);
    expect(ragged).not.toContain('<svg');
    expect(ragged).toContain('These points do not line up.');
  });

  it('speaks in exact quantities, never in market-data vocabulary', () => {
    for (const forbidden of ['price', 'Price', 'odds', 'probability', 'Probability', 'volume', 'liquidity', 'trend', '$']) {
      expect(html).not.toContain(forbidden);
    }
  });
});
