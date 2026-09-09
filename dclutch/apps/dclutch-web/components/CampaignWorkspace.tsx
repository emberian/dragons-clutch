'use client';

import PageShell from '@/components/PageShell';
import { useEffect, useState, type ReactNode } from 'react';

import Nav from '@/components/Nav';
import LawBand from '@/components/charts/LawBand';
import Sparkline from '@/components/charts/Sparkline';
import { marketEditorialV1 } from '@dclutch/sdk/marketRegistry';
import {
  CAMPAIGN_SERIES_URL_V1,
  campaignReadingV1,
  campaignSpendLineV1,
  campaignStageLabelsV1,
  campaignVolumeV1,
  conservationLawRowsV1,
  conservationReadingV1,
  everyLineFlatV1,
  hoardCoverageLinesV1,
  impliedOddsLinesV1,
  issuedSupplyLinesV1,
  lawBandCyclesV1,
  NO_CAMPAIGN_SENTENCE_V1,
  readSimulatorSeriesV1,
  settlementCellsV1,
  type SimulatorSeriesReadV1,
  type SimulatorSeriesV1,
} from '@/lib/simulatorSeries';

/**
 * ONE MARKET'S WHOLE LIFE, on a chain this project started for the purpose.
 *
 * /pulse draws a poller: it watches one devnet market hold still and reports
 * the same quantities every cycle, which is an honest record whose honest
 * drawing is a flat line. This page draws the other kind of record. A campaign
 * founds a market from nothing, publishes the source graph it resolves
 * against, funds and activates its resolution, carries it to a terminal answer
 * through a real transport, and retires it — and every boundary between two of
 * its stages is a place where the numbers are allowed to move.
 *
 * WHAT THIS PAGE MAY NOT IMPLY, and the mechanism that stops it. The campaign
 * runs against a private validator on 127.0.0.1 with its own genesis. Nobody
 * outside the run can reach it, nobody traded against it, and no figure here
 * is a devnet or mainnet fact. So `CAMPAIGN_LOCAL_CAVEAT_V1` is printed beside
 * EVERY chart on this page rather than once at the top — a reader who lands
 * mid-page, or screenshots one figure, must still be told. The `cluster` field
 * on the artifact is what decides it, and this page refuses to draw a series
 * that does not say `local`, because a devnet series arriving at this URL
 * would inherit a caption that is false about it.
 *
 * WHAT THE CHARTS ARE, precisely:
 *
 * - the ODDS PATH is the market's own liability supply per cell, as a share.
 *   It is what the market says it owes on each outcome — not a price anybody
 *   paid, because in this record nobody has bought anything, and the caption
 *   says exactly that whether or not the line moves.
 * - the VAULT is what the market's Hoard held against every collateral atom
 *   the ledger could name anywhere. Two lines because the gap is the subject.
 * - the WORK is transactions, compute and fees per boundary. That is the only
 *   volume a market with no fills has, and calling it volume without saying so
 *   would be the lie this whole page is arranged to avoid.
 * - the SETTLEMENT is stated per cell and never drawn as a path. Two points —
 *   before the answer and after it — is a settlement, and a line through them
 *   would invent the shape in between.
 */

/** This site's editorial gloss on what each law is FOR. Not the census's words. */
const LAW_GLOSSES: Readonly<Record<string, string>> = Object.freeze({
  L1: 'all collateral atoms are held in tracked accounts',
  L2: 'vault movement matches the declared transfer',
  L3: 'position balances match issued claims by outcome',
  L4: 'the vault covers the largest unsettled outcome liability',
  L5: 'tracked collateral delta matches the declared amount',
  L6: 'lamports from closed protocol accounts are accounted for',
  L7: 'fee-payer delta matches transaction fees',
});

/**
 * The one guard between a devnet record and a page that calls everything local.
 *
 * Exported so a test can pin it: this is the surface where the demo-vs-product
 * rule is actually enforced, and enforcing it by convention was never going to
 * survive the first lane that repointed a URL.
 */
export function campaignSeriesOrRefusalV1(read: SimulatorSeriesReadV1 | null):
  | Readonly<{ kind: 'waiting' }>
  | Readonly<{ kind: 'absent' }>
  | Readonly<{ kind: 'refused'; reason: string }>
  | Readonly<{ kind: 'loaded'; series: SimulatorSeriesV1 }> {
  if (read === null) return Object.freeze({ kind: 'waiting' as const });
  if (read.kind === 'absent') return Object.freeze({ kind: 'absent' as const });
  if (read.kind === 'refused') return Object.freeze({ kind: 'refused' as const, reason: read.reason });
  if (read.series.cluster !== 'local') {
    return Object.freeze({
      kind: 'refused' as const,
      reason: `campaign cluster is ${read.series.cluster}; this page requires local`,
    });
  }
  if (read.series.campaign === null) {
    return Object.freeze({
      kind: 'refused' as const,
      reason: 'campaign name is missing',
    });
  }
  return Object.freeze({ kind: 'loaded' as const, series: read.series });
}

/** The odds path, and the sentence that says what it is and is not. */
export function OddsPath({ series }: Readonly<{ series: SimulatorSeriesV1 }>) {
  const editorial = series.market === null ? null : marketEditorialV1(series.market);
  const lines = impliedOddsLinesV1(series, editorial?.outcomes ?? null);
  const xLabels = campaignStageLabelsV1(series);
  const supply = issuedSupplyLinesV1(series, editorial?.outcomes ?? null);
  return <>
    {/* FE-CHART mount: each cell's share of the issued liability, in exact
        floored basis points. */}
    <Sparkline
      lines={lines}
      xLabels={xLabels}
      unit="basis points of the issued supply"
      caption="Issued claims by outcome · basis points · stage boundaries"
      flatNote={lines.length > 0 && everyLineFlatV1(lines)
        ? 'Unchanged across all boundaries.'
        : undefined}
      emptyReason="No share is available before claims are issued. Continue the campaign to an issued boundary."
    />
    <h3 className="detail-subhead">Issued claims</h3>
    {/* FE-CHART mount: the raw per-cell liability the shares are computed from. */}
    <Sparkline
      lines={supply}
      xLabels={xLabels}
      unit="claim atoms"
      caption="Issued claims by outcome · atoms · stage boundaries"
      flatNote={everyLineFlatV1(supply)
        ? 'Unchanged across all boundaries.'
        : undefined}
      emptyReason={NO_CAMPAIGN_SENTENCE_V1}
    />
  </>;
}

/** What the market's vault held, against every atom the ledger could name. */
export function VaultPath({ series }: Readonly<{ series: SimulatorSeriesV1 }>) {
  const lines = hoardCoverageLinesV1(series);
  return <>
    {/* FE-CHART mount: the Hoard against the tracked total and the Mint supply. */}
    <Sparkline
      lines={lines}
      xLabels={campaignStageLabelsV1(series)}
      unit="collateral atoms"
      caption="Hoard, tracked collateral, and Mint supply · atoms · stage boundaries"
      flatNote={lines.length > 0 && everyLineFlatV1(lines)
        ? 'Unchanged across all boundaries.'
        : undefined}
      emptyReason={NO_CAMPAIGN_SENTENCE_V1}
    />
  </>;
}

/** The work each boundary cost. Three dimensions, three figures. */
export function WorkPerStage({ series }: Readonly<{ series: SimulatorSeriesV1 }>) {
  const volume = campaignVolumeV1(series);
  const spend = campaignSpendLineV1(series);
  if (volume === null) {
    return <p className="market-empty">
      No stage-cost data is available. Publish a capture with transaction metrics.
    </p>;
  }
  return <>
    <p className="direct-status">
      {volume.totalTransactions === null ? null : <>{volume.totalTransactions} transactions across the drawn boundaries</>}
      {volume.totalComputeUnits === null ? null : <>, {volume.totalComputeUnits} compute units</>}
      {volume.totalFeeLamports === null ? null : <>, {volume.totalFeeLamports} lamports in fees</>}.
    </p>
    {volume.transactions === null ? null : <>
      {/* FE-CHART mount: transactions per boundary. */}
      <Sparkline
        lines={[volume.transactions]}
        xLabels={volume.xLabels}
        unit="transactions"
        caption="Transactions submitted · count · stage boundaries"
        emptyReason={NO_CAMPAIGN_SENTENCE_V1}
      />
    </>}
    {volume.computeUnits === null ? null : <>
      <h3 className="detail-subhead">Compute used</h3>
      {/* FE-CHART mount: compute units per boundary, its own figure because
          compute and transaction counts are different dimensions. */}
      <Sparkline
        lines={[volume.computeUnits]}
        xLabels={volume.xLabels}
        unit="compute units"
        caption="Compute consumed · CU · stage boundaries"
        emptyReason={NO_CAMPAIGN_SENTENCE_V1}
      />
    </>}
    {spend === null ? null : <>
      <h3 className="detail-subhead">Fees paid</h3>
      {/* FE-CHART mount: the fee payer's drawdown. A level, not an interval,
          so it never shares an axis with the counts above. */}
      <Sparkline
        lines={[spend]}
        xLabels={volume.xLabels}
        unit="lamports"
        caption="Fee-payer spend since first boundary · lamports · stage boundaries"
        emptyReason={NO_CAMPAIGN_SENTENCE_V1}
      />
    </>}
  </>;
}

/** What one claim on each cell turned out to be worth. */
export function Settlement({ series }: Readonly<{ series: SimulatorSeriesV1 }>) {
  const editorial = series.market === null ? null : marketEditorialV1(series.market);
  const cells = settlementCellsV1(series, editorial?.outcomes ?? null);
  if (cells.length === 0) {
    return <p className="market-empty">
      {series.settlement === null
        ? 'Settlement is pending. Continue the campaign to a terminal answer.'
        : 'Claim-unit data is missing. Publish a complete settlement capture.'}
    </p>;
  }
  return <>
    <p className="direct-status">
      Selected outcome {series.settlement?.selectedCell} pays {series.claimUnitAtoms} collateral atom{series.claimUnitAtoms === '1' ? '' : 's'} per claim; other outcomes pay 0.
    </p>
    <div className="viz-table-scroll" tabIndex={0} role="region" aria-label="What each outcome is owed at settlement">
      <table className="holders-table">
        <thead><tr><th>Outcome</th><th>Claims issued · raw u64</th><th>Collateral per claim</th><th>Owed in total</th></tr></thead>
        <tbody>
          {cells.map((cell) => <tr key={cell.cell}>
            <td>{cell.label}{cell.selected ? ' · selected' : ''}</td>
            <td>{cell.claimsIssued}</td>
            <td>{cell.realizedAtomsPerClaim}</td>
            <td>{cell.realizedAtoms}</td>
          </tr>)}
        </tbody>
      </table>
    </div>
    {series.settlement?.certificate === null || series.settlement === null
      ? null
      : <p className="market-editorial-note">Certificate: {series.settlement.certificate}</p>}
  </>;
}

/** Every law, at every boundary, with the boundaries named. */
export function CampaignLaws({ series }: Readonly<{ series: SimulatorSeriesV1 }>) {
  const rows = conservationLawRowsV1(series);
  const cycles = lawBandCyclesV1(series);
  const reading = conservationReadingV1(series);
  const labels = campaignStageLabelsV1(series);
  if (rows.length === 0) {
    return <p className="market-empty">Law identities are missing. Publish a capture with named law results.</p>;
  }
  return <>
    {reading === null ? null : <p className="direct-status">{reading}</p>}
    {/* FE-CHART mount: a status band, not a line — a verdict is a state, and a
        line through states invents an ordering between them. */}
    <LawBand
      rows={rows}
      cycles={cycles}
      glosses={LAW_GLOSSES}
      caption="Conservation result by law · status · stage boundaries"
      emptyReason={NO_CAMPAIGN_SENTENCE_V1}
    />
    <ol className="market-editorial-note">
      {labels.map((label, index) => <li key={`${index}-${label}`}>{index + 1} · {label}</li>)}
    </ol>
  </>;
}

export default function CampaignWorkspace({ preloaded }: Readonly<{
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
        CAMPAIGN_SERIES_URL_V1,
      );
      if (!cancelled) setRead(settled);
    })();
    return () => { cancelled = true; };
  }, [preloaded]);

  const state = campaignSeriesOrRefusalV1(read);
  const series = state.kind === 'loaded' ? state.series : null;
  const reading = series === null ? null : campaignReadingV1(series);

  const body = (inner: (series: SimulatorSeriesV1) => ReactNode) => {
    if (state.kind === 'waiting') return <p className="direct-status">Looking for a recorded campaign…</p>;
    if (state.kind === 'absent') return <p className="market-empty">{NO_CAMPAIGN_SENTENCE_V1}</p>;
    if (state.kind === 'refused') return <p className="market-refusal">Refused: {state.reason}</p>;
    return inner(state.series);
  };

  return <PageShell className="product-shell trade-v3-shell" header={<Nav current="/campaign" status="campaign" />}>

    <section className="trade-v3-hero">
      <div>
        <p className="eyebrow">Campaign lifecycle</p>
        <h1>Market lifecycle.<br /><em>From founding to settlement.</em></h1>
        <p>Track one local campaign across founding, activation, resolution, settlement, and retirement.</p>
      </div>
      <aside>
        <span>Run</span>
        <strong>{state.kind === 'loaded' ? 'One campaign recorded' : state.kind === 'refused' ? 'Refused' : state.kind === 'absent' ? 'Nothing published' : 'Reading…'}</strong>
        {reading === null
          ? <p>{NO_CAMPAIGN_SENTENCE_V1}</p>
          : <p>{reading}</p>}
      </aside>
    </section>

    <div className="local-status-strip">
      <i className={series === null ? undefined : 'online'} />
      <strong>{series === null ? 'No campaign record' : series.settlement === null ? 'Founded, not settled' : 'Founded and settled'}</strong>
      <span>{series === null
        ? NO_CAMPAIGN_SENTENCE_V1
        : `${series.points.length} stage boundar${series.points.length === 1 ? 'y' : 'ies'} · ${series.settlement === null ? 'settlement pending' : `settled cell ${series.settlement.selectedCell}`}`}</span>
    </div>

    <section className="trade-v3-card">
      <header><span>01</span><div><h2>Issued claims</h2><p>Claims by outcome across the campaign.</p></div></header>
      {body((loaded) => <OddsPath series={loaded} />)}
    </section>

    <section className="trade-v3-card">
      <header><span>02</span><div><h2>Collateral</h2><p>Hoard balance, tracked collateral, and Mint supply.</p></div></header>
      {body((loaded) => <VaultPath series={loaded} />)}
    </section>

    <section className="trade-v3-card">
      <header><span>03</span><div><h2>Stage costs</h2><p>Transactions, compute units, and fees by stage.</p></div></header>
      {body((loaded) => <WorkPerStage series={loaded} />)}
    </section>

    <section className="trade-v3-card">
      <header><span>04</span><div><h2>Settlement</h2><p>Selected outcome and collateral owed per claim.</p></div></header>
      {body((loaded) => <Settlement series={loaded} />)}
    </section>

    <section className="trade-v3-card">
      <header><span>05</span><div><h2>Conservation checks</h2><p>Law status at each stage boundary.</p></div></header>
      {body((loaded) => <CampaignLaws series={loaded} />)}
    </section>

    <footer className="product-footer">
      <span>Campaign metrics</span>
    </footer>
  </PageShell>;
}
