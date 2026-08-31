import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import published from '@/public/campaign-series.json';
import example from '@/fixtures/campaign-series.example.json';
import { CAMPAIGN_LOCAL_CAVEAT_V1, parseSimulatorSeriesV1 } from '@/lib/simulatorSeries';

import CampaignWorkspace, { campaignSeriesOrRefusalV1 } from './CampaignWorkspace';

/**
 * The campaign surface, and the one rule it exists to enforce.
 *
 * Every figure on this page came off a private validator on 127.0.0.1. The
 * project's demo-vs-product rule is that nothing on this site may imply
 * trading that has not happened, and a chart is exactly the surface that
 * implies it — so the local caveat travels WITH each chart rather than sitting
 * once in a footer, and a record that does not say `local` is refused rather
 * than drawn under captions that would be false about it.
 */
/**
 * The RENDER tests draw a fixture, not the day's capture. What a chart says
 * must not depend on which campaign happened to be recorded when the suite
 * ran — a run that settled on a different cell, or never settled at all,
 * would silently turn an assertion into a no-op.
 *
 * The COMMITTED capture is pinned separately, because it is the thing readers
 * actually get: an artifact this decoder refuses would otherwise ship as a
 * silent "absent" and nobody would learn why the page went blank.
 */
const series = parseSimulatorSeriesV1(example);
const capture = parseSimulatorSeriesV1(published);

describe('the published campaign record', () => {
  it('decodes exactly as committed, as a local campaign', () => {
    expect(capture.cluster).toBe('local');
    expect(capture.campaign).not.toBeNull();
    expect(capture.campaign?.rpcOrigin.startsWith('http://127.0.0.1:')).toBe(true);
    expect(capture.campaign?.sourceRevision).toMatch(/^[0-9a-f]{40}$/);
    expect(capture.points.length).toBeGreaterThan(0);
    expect(capture.outcomeCount).toBeGreaterThan(0);
  });

  it('names every boundary it drew, so the x-axis is the campaign’s words', () => {
    for (const point of capture.points) expect(point.stage).not.toBeNull();
  });

  /** A broken law halts a campaign. One published here would be the single
   * most important fact on the page, and this pin is why it cannot arrive
   * quietly in a capture nobody re-read. */
  it('never publishes a broken conservation law', () => {
    const broken = capture.points.reduce((sum, point) => sum + point.checksBroken, 0);
    expect(broken).toBe(0);
  });

  /**
   * The label is where a reader learns WHOSE run this is. The site draws one
   * campaign record at a time and the runs are other people's as often as not,
   * so an artifact that will not name its run is one whose figures cannot be
   * traced back to the machine that produced them.
   */
  it('names the run in the label, and prints that label on the page', () => {
    expect(capture.campaign?.label.length).toBeGreaterThan(0);
    const html = renderToStaticMarkup(<CampaignWorkspace preloaded={{ kind: 'loaded', series: capture }} />);
    expect(html).toContain(capture.campaign?.label as string);
  });

  it('renders, which is the only thing that makes it an artifact at all', () => {
    const html = renderToStaticMarkup(<CampaignWorkspace preloaded={{ kind: 'loaded', series: capture }} />);
    expect(html).toContain(CAMPAIGN_LOCAL_CAVEAT_V1);
    expect(html).toContain('<polyline');
  });
});

describe('the campaign surface', () => {
  const html = renderToStaticMarkup(<CampaignWorkspace preloaded={{ kind: 'loaded', series }} />);

  it('says under every chart that this was a local rehearsal validator', () => {
    // One caveat per figure, not one per page. A reader who screenshots one
    // chart must still be told what chain it came off.
    const figures = html.split('<figure').length - 1;
    const caveats = html.split(CAMPAIGN_LOCAL_CAVEAT_V1).length - 1;
    expect(figures).toBeGreaterThan(0);
    expect(caveats).toBeGreaterThanOrEqual(figures);
  });

  it('never says devnet or mainnet about its own figures', () => {
    expect(html).toContain('Not devnet');
    expect(html).not.toMatch(/traded on devnet|on the public devnet[^,.]*price/);
  });

  it('draws the odds path and says it is a liability record and not a price anyone paid', () => {
    expect(html).toContain('Where the claims sit');
    expect(html).toContain('not a price anyone paid');
    expect(html).toContain('<polyline');
  });

  it('draws the vault against the tracked total', () => {
    expect(html).toContain('What the vault held');
    expect(html).toContain('in the market’s own Hoard');
    expect(html).toContain('tracked across every named account');
  });

  it('calls the work what it is, and never calls it traded volume', () => {
    expect(html).toContain('The work each stage took');
    expect(html).toContain('A market with nobody trading has no traded volume');
  });

  it('names the boundary each law column is, because a number is not a stage', () => {
    expect(html).toContain('The checks, after every stage');
    expect(html).toContain(series.points[0].stage as string);
  });

  it('states the terminal answer per cell, and never draws it as a path', () => {
    if (series.settlement === null) {
      expect(html).toContain('has not reached a terminal answer');
      return;
    }
    expect(html).toContain(`The terminal certificate selected cell ${series.settlement.selectedCell}`);
    expect(html).toContain('is worth nothing');
  });
});

describe('the guard between a devnet record and a page that calls everything local', () => {
  it('refuses to draw a record whose cluster is not local', () => {
    const state = campaignSeriesOrRefusalV1({
      kind: 'loaded',
      series: parseSimulatorSeriesV1({ ...(example as Record<string, unknown>), cluster: 'devnet' }),
    });
    expect(state.kind).toBe('refused');
    expect(state.kind === 'refused' ? state.reason : '').toContain('cluster is devnet');
  });

  it('refuses a record that names no campaign, because nothing would attribute the figures', () => {
    const state = campaignSeriesOrRefusalV1({
      kind: 'loaded',
      series: parseSimulatorSeriesV1({ ...(example as Record<string, unknown>), campaign: null }),
    });
    expect(state.kind).toBe('refused');
  });

  it('reads a missing artifact as the missing artifact it is', () => {
    expect(campaignSeriesOrRefusalV1({ kind: 'absent' }).kind).toBe('absent');
    expect(campaignSeriesOrRefusalV1(null).kind).toBe('waiting');
  });

  it('renders the empty state without a chart when nothing is published', () => {
    const html = renderToStaticMarkup(<CampaignWorkspace preloaded={{ kind: 'absent' }} />);
    expect(html).toContain('No campaign record is published beside this site right now');
    expect(html).not.toContain('<polyline');
  });
});
