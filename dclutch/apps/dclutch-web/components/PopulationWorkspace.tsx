'use client';

import PageShell from '@/components/PageShell';
import { useEffect, useState, type ReactNode } from 'react';

import Nav from '@/components/Nav';
import Sparkline from '@/components/charts/Sparkline';
import {
  archetypeCensusV1,
  eventTimelineLabelsV1,
  eventTimelineLinesV1,
  executedReadingV1,
  honestyRowsV1,
  marketOddsLinesV1,
  marketRowsV1,
  marketSlotLabelsV1,
  notDoneReadingV1,
  populationReadingV1,
  readSimulatorSeriesV1,
  SIMLIFE_SERIES_URL_V1,
  type SimulatorMarketSeriesV1,
  type SimulatorSeriesReadV1,
  type SimulatorSeriesV1,
} from '@/lib/simulatorSeries';

/**
 * A POPULATION of markets, drawn as contemporaries.
 *
 * `/campaign` draws one market's whole life. This draws many at once, which is
 * a different claim and needs its own page rather than a sixth card on that
 * one: the markets here were read at the SAME boundaries, so their lines share
 * an x-axis and can honestly be laid beside each other, and the interesting
 * quantity is how they differ.
 *
 * THE THING THIS PAGE MUST NOT DO is read as a trading record. A world plans
 * nine kinds of thing and a substrate may be able to do three of them; a page
 * that draws such a run without saying so is describing a market that does not
 * exist. So the honesty strip is not an appendix here — it is a section with a
 * number in it, and the hero says what the run mutated before it says anything
 * else.
 */

export const NO_POPULATION_SENTENCE_V1 =
  'No population data is available. Run simlife and publish its capture to view this page.';

export function populationOrRefusalV1(read: SimulatorSeriesReadV1 | null):
  | Readonly<{ kind: 'waiting' }>
  | Readonly<{ kind: 'absent' }>
  | Readonly<{ kind: 'refused'; reason: string }>
  | Readonly<{ kind: 'loaded'; series: SimulatorSeriesV1 }> {
  if (read === null) return Object.freeze({ kind: 'waiting' as const });
  if (read.kind === 'absent') return Object.freeze({ kind: 'absent' as const });
  if (read.kind === 'refused') return Object.freeze({ kind: 'refused' as const, reason: read.reason });
  // A v1/v2/v3 capture is a SINGLE market and decodes perfectly well; it just
  // is not a population, and every caption on this page would be false about
  // it. Refusing it by name is better than drawing one line under a heading
  // that promises twelve.
  if (read.series.world === null) {
    return Object.freeze({
      kind: 'refused' as const,
      reason: 'This capture contains one market and no population data. Open Campaign to view it.',
    });
  }
  return Object.freeze({ kind: 'loaded' as const, series: read.series });
}

/** The per-market odds paths, one small chart each. */
export function OddsPaths({ series }: Readonly<{ series: SimulatorSeriesV1 }>) {
  if (series.markets.length === 0) {
    return <p className="market-empty">No markets were observed. Run simlife with at least one market.</p>;
  }
  return <div className="population-grid">
    {series.markets.map((market) => <MarketOdds key={market.marketId} market={market} />)}
  </div>;
}

function MarketOdds({ market }: Readonly<{ market: SimulatorMarketSeriesV1 }>) {
  const lines = marketOddsLinesV1(market);
  const what = [market.archetype, `${market.outcomeCount} cells`, market.basis, market.destiny]
    .filter((part): part is string => part !== null)
    .join(' · ');
  return <article className="population-card">
    <header>
      <strong>{market.marketId}</strong>
      <span>{what.length === 0 ? 'market' : what}</span>
    </header>
    <Sparkline
      lines={lines}
      xLabels={marketSlotLabelsV1(market)}
      unit="basis points of issued supply"
      caption={`${market.marketId} issued share by cell · basis points · recorded boundaries`}
      emptyReason={lines.length === 0
        ? `${market.marketId} has no issued supply. Run a funded cycle to add an odds path.`
        : undefined}
      flatNote={`${market.marketId} was unchanged across all boundaries.`}
    />
  </article>;
}

/** The whole run over one tick axis. */
export function EventTimeline({ series }: Readonly<{ series: SimulatorSeriesV1 }>) {
  const lines = eventTimelineLinesV1(series);
  return <Sparkline
    lines={lines}
    xLabels={eventTimelineLabelsV1(series)}
    unit="events"
    caption="Landed mutations, refused mutations, and markets censused · events · run ticks"
    emptyReason={lines.length === 0
      ? 'No timeline is available. Publish a capture with tick events to view it.'
      : undefined}
    flatNote="Every tick did the same amount of work."
  />;
}

/** Every route, and what became of it. */
export function HonestyStrip({ series }: Readonly<{ series: SimulatorSeriesV1 }>) {
  const rows = honestyRowsV1(series);
  if (rows.length === 0) return <p className="market-empty">No route tally is available. Publish a capture with route results.</p>;
  return <>
    <div className="viz-table-scroll" tabIndex={0} role="region" aria-label="Planned against executed, per route">
      <table className="holders-table population-honesty">
        <thead>
          <tr>
            <th scope="col">route</th>
            <th scope="col">planned</th>
            <th scope="col">executed</th>
            <th scope="col">refused</th>
            <th scope="col">not attempted</th>
            <th scope="col">blocked</th>
          </tr>
        </thead>
        <tbody>
          {rows.map((row) => <tr key={row.route}>
            <th scope="row">{row.route}</th>
            <td>{row.planned}</td>
            <td className={row.executed > 0 ? 'population-executed' : undefined}>{row.executed}</td>
            <td>{row.refused}</td>
            <td>{row.unattempted}</td>
            <td>{row.blocked}</td>
          </tr>)}
        </tbody>
      </table>
    </div>
    <ul className="population-reasons">
      {rows.filter((row) => row.leadingReason !== null).map((row) => <li key={row.route}>
        <strong>{row.route}</strong>
        <span>{row.leadingOutcome}</span>
        <p>{row.leadingReason}</p>
      </li>)}
    </ul>
  </>;
}

/** What the world drew, whether or not anything could drive it. */
export function ArchetypeCensus({ series }: Readonly<{ series: SimulatorSeriesV1 }>) {
  const rows = archetypeCensusV1(series);
  const markets = marketRowsV1(series);
  return <>
    <div className="viz-table-scroll" tabIndex={0} role="region" aria-label="The population, by archetype">
      <table className="holders-table">
        <thead>
          <tr><th scope="col">archetype</th><th scope="col">basis</th><th scope="col">drawn</th><th scope="col">observed</th></tr>
        </thead>
        <tbody>
          {rows.map((row) => <tr key={row.archetype}>
            <th scope="row">{row.archetype}</th>
            <td>{row.basis}</td>
            <td>{row.planned}</td>
            <td>{row.observed}</td>
          </tr>)}
        </tbody>
      </table>
    </div>
    <div className="viz-table-scroll" tabIndex={0} role="region" aria-label="The markets in this population">
      <table className="holders-table">
        <thead>
          <tr>
            <th scope="col">market</th><th scope="col">cells</th><th scope="col">points</th>
            <th scope="col">slots covered</th><th scope="col">checks held</th><th scope="col">checks broken</th>
            <th scope="col">what moved</th>
          </tr>
        </thead>
        <tbody>
          {markets.map((row) => <tr key={row.marketId}>
            <th scope="row">{row.marketId}</th>
            <td>{row.outcomeCount}</td>
            <td>{row.points}</td>
            <td>{row.slotsCovered ?? '—'}</td>
            <td>{row.checksHeld}</td>
            <td className={row.checksBroken > 0 ? 'population-broken' : undefined}>{row.checksBroken}</td>
            <td>{row.moved.length === 0 ? 'nothing' : row.moved.join(', ')}</td>
          </tr>)}
        </tbody>
      </table>
    </div>
  </>;
}


/**
 * Where the world's answers landed.
 *
 * A population that settles every market into the same place has ONE
 * measurement copied as many times as it has markets, and until the bands were
 * drawn around the coordinate the substrate actually observes, every local
 * world did exactly that. Positions are tenths of the way through a market's
 * own ordinary cells, because cell 3 of four and cell 3 of eleven are not the
 * same answer.
 */
export function OutcomeSpread({ series }: Readonly<{ series: SimulatorSeriesV1 }>) {
  const spread = series.world?.outcomeSpread ?? null;
  if (spread === null) {
    return <p className="market-empty">No outcome spread is available. Publish a capture with settlement results.</p>;
  }
  if (spread.positionedMarkets === 0) {
    return <p className="market-empty">No resolved multi-cell markets are available. Complete one to add an outcome spread.</p>;
  }
  const buckets = Array.from({ length: 11 }, (_, tenths) => ({
    tenths,
    count: spread.positionCounts[String(tenths)] ?? 0,
  }));
  const tallest = Math.max(...buckets.map((bucket) => bucket.count), 1);
  return <>
    <dl className="population-facts">
      <div><dt>coordinate</dt><dd>{spread.coordinateAnchor}</dd></div>
      <div><dt>markets placed</dt><dd>{spread.positionedMarkets}</dd></div>
      <div><dt>distinct positions</dt><dd>{spread.distinctPositions}</dd></div>
      <div><dt>heaviest</dt><dd>{spread.heaviestSharePercent}%</dd></div>
    </dl>
    {spread.degenerate ? <p className="population-broken">
      One position takes {spread.heaviestSharePercent}% of this world, over the{' '}
      {spread.degenerateThresholdPercent}% threshold.
    </p> : null}
    <div className="viz-table-scroll" tabIndex={0} role="region" aria-label="Who holds this world, by position">
      <table className="holders-table">
        <thead>
          <tr>
            <th scope="col">position</th><th scope="col">markets</th><th scope="col">share</th>
          </tr>
        </thead>
        <tbody>
          {buckets.map((bucket) => <tr key={bucket.tenths}>
            <th scope="row">{bucket.tenths}/10</th>
            <td>{bucket.count}</td>
            <td>
              <span
                className="population-bar"
                style={{ width: `${Math.round((100 * bucket.count) / tallest)}%` }}
                aria-hidden="true"
              />
            </td>
          </tr>)}
        </tbody>
      </table>
    </div>
  </>;
}

export default function PopulationWorkspace({ preloaded }: Readonly<{
  /** Test seam: a settled read. The page itself always fetches. */
  preloaded?: SimulatorSeriesReadV1;
}> = {}) {
  const [read, setRead] = useState<SimulatorSeriesReadV1 | null>(preloaded ?? null);

  useEffect(() => {
    if (preloaded !== undefined) return undefined;
    let cancelled = false;
    (async () => {
      const settled = await readSimulatorSeriesV1(
        (url) => globalThis.fetch(url, { cache: 'no-store', redirect: 'error', credentials: 'omit' }),
        SIMLIFE_SERIES_URL_V1,
      );
      if (!cancelled) setRead(settled);
    })();
    return () => { cancelled = true; };
  }, [preloaded]);

  const state = populationOrRefusalV1(read);
  const series = state.kind === 'loaded' ? state.series : null;
  const drew = series === null ? null : populationReadingV1(series);
  const did = series === null ? null : executedReadingV1(series);
  const missed = series === null ? null : notDoneReadingV1(series);

  const body = (inner: (series: SimulatorSeriesV1) => ReactNode) => {
    if (state.kind === 'waiting') return <p className="direct-status">Looking for a population capture…</p>;
    if (state.kind === 'absent') return <p className="market-empty">{NO_POPULATION_SENTENCE_V1}</p>;
    if (state.kind === 'refused') return <p className="market-refusal">Refused: {state.reason}</p>;
    return inner(state.series);
  };

  return <PageShell className="product-shell trade-v3-shell" header={<Nav current="/population" status="population" />}>

    <section className="trade-v3-hero">
      <div>
        <p className="eyebrow">Market population</p>
        <h1>One run,<br /><em>many markets.</em></h1>
        <p>Compare market paths, route results, conservation checks, and settled outcomes across one seeded run.</p>
      </div>
      <aside>
        <span>Run status</span>
        <strong>{state.kind === 'loaded' ? 'One population recorded' : state.kind === 'refused' ? 'Refused' : state.kind === 'absent' ? 'Nothing published' : 'Reading…'}</strong>
        {drew === null ? <p>{NO_POPULATION_SENTENCE_V1}</p> : <p>{drew}</p>}
        {did === null ? null : <p className="market-editorial-note">{did}</p>}
        {missed === null ? null : <p className="market-editorial-note">{missed}</p>}
      </aside>
    </section>

    <section className="trade-v3-card">
      <header><span>01</span><div><h2>Market odds paths</h2><p>Issued share by cell at each recorded boundary.</p></div></header>
      {body((loaded) => <OddsPaths series={loaded} />)}
    </section>

    <section className="trade-v3-card">
      <header><span>02</span><div><h2>Run timeline</h2><p>Landed mutations, refusals, and censused markets by tick.</p></div></header>
      {body((loaded) => <EventTimeline series={loaded} />)}
    </section>

    <section className="trade-v3-card">
      <header><span>03</span><div><h2>Route results</h2><p>Planned, executed, refused, unattempted, and blocked counts by route.</p></div></header>
      {body((loaded) => <HonestyStrip series={loaded} />)}
    </section>

    <section className="trade-v3-card">
      <header><span>04</span><div><h2>Market census</h2><p>Archetypes, observed markets, slot coverage, and conservation results.</p></div></header>
      {body((loaded) => <ArchetypeCensus series={loaded} />)}
    </section>

    <section className="trade-v3-card">
      <header><span>05</span><div><h2>Outcome spread</h2><p>Settled cell positions · tenths of each market&apos;s ordinary cells</p></div></header>
      {body((loaded) => <OutcomeSpread series={loaded} />)}
    </section>

    <footer className="product-footer">
      <span>Population metrics</span>
    </footer>
  </PageShell>;
}
