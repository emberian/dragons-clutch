'use client';

import PageShell from '@/components/PageShell';
import { useEffect, useState, type ReactNode } from 'react';

import Nav from '@/components/Nav';
import LawBand from '@/components/charts/LawBand';
import Sparkline from '@/components/charts/Sparkline';
import { marketEditorialV1 } from '@/lib/marketRegistry';
import {
  CAMPAIGN_LOCAL_CAVEAT_V1,
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
  SERIES_RECORD_CAVEAT_V1,
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
  L1: 'collateral closure — every collateral atom that exists is sitting in an account this census watches',
  L2: 'declared vault movement — the market’s vault moved by exactly what was declared, and by nothing else',
  L3: 'supply agreement — what the positions hold adds up, outcome by outcome, to what the market issued',
  L4: 'full collateralisation — the vault holds at least what the worst outcome could be asked to pay, which is a question about a market that still owes: settlement discharges that liability, so this law retires at a Terminal market rather than reading broken against one that paid',
  L5: 'stage delta — tracked collateral changed between two readings by exactly the declared amount',
  L6: 'rent conservation — lamports leaving a closed protocol account are accounted for',
  L7: 'lamport accounting — the fee payer’s balance moved by exactly the fees paid',
});

/** Said under every figure. A caption, not a disclaimer buried in a footer. */
function LocalNote() {
  return <p className="market-editorial-note">{CAMPAIGN_LOCAL_CAVEAT_V1}</p>;
}

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
      reason: `this page draws local rehearsal campaigns and the published record says its cluster is ${read.series.cluster}. Every caption here would be false about it, so it is not drawn.`,
    });
  }
  if (read.series.campaign === null) {
    return Object.freeze({
      kind: 'refused' as const,
      reason: 'the published record names no campaign, so there is no run to attribute these figures to.',
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
      caption="Each outcome’s share of the claims the market has issued, at every stage boundary the campaign censused. This is the market’s own liability record — what it says it owes on each outcome — not a price anyone paid, because nobody has bought or sold anything in this run. Shares are floored to the basis point, so they can sum to slightly under 10,000."
      flatNote={lines.length > 0 && everyLineFlatV1(lines)
        ? 'unchanged at every boundary: this is the distribution the founding set, and no fill has moved it'
        : undefined}
      emptyReason="At least one boundary of this run had no claims issued at all, and a share of nothing is undefined rather than zero — so no odds line is drawn."
    />
    <LocalNote />
    <h3 className="detail-subhead">The same thing in atoms</h3>
    {/* FE-CHART mount: the raw per-cell liability the shares are computed from. */}
    <Sparkline
      lines={supply}
      xLabels={xLabels}
      unit="claim atoms"
      caption="The claim atoms behind those shares, read from the market’s Claims aggregate. A share is a ratio and hides the size; this is the size."
      flatNote={everyLineFlatV1(supply)
        ? 'unchanged at every boundary: no claim was issued or retired between them'
        : undefined}
      emptyReason={NO_CAMPAIGN_SENTENCE_V1}
    />
    <LocalNote />
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
      caption="What the market’s own Hoard held at each boundary, against every collateral atom the conservation ledger could find in an account it names, against the Mint’s whole supply. When the first two move apart, collateral left the vault for somewhere still watched; when the tracked total itself moves, an atom went somewhere nobody named — and L1 is the law that says so."
      flatNote={lines.length > 0 && everyLineFlatV1(lines)
        ? 'unchanged at every boundary: this campaign moves the market’s phase and its resolution, and never its collateral'
        : undefined}
      emptyReason={NO_CAMPAIGN_SENTENCE_V1}
    />
    <LocalNote />
  </>;
}

/** The work each boundary cost. Three dimensions, three figures. */
export function WorkPerStage({ series }: Readonly<{ series: SimulatorSeriesV1 }>) {
  const volume = campaignVolumeV1(series);
  const spend = campaignSpendLineV1(series);
  if (volume === null) {
    return <p className="market-empty">
      This record does not carry per-boundary transaction counts, so there is nothing here to draw. It is not a zero.
    </p>;
  }
  return <>
    <p className="direct-status">
      {volume.totalTransactions === null ? null : <>{volume.totalTransactions} transactions across the drawn boundaries</>}
      {volume.totalComputeUnits === null ? null : <>, {volume.totalComputeUnits} compute units</>}
      {volume.totalFeeLamports === null ? null : <>, {volume.totalFeeLamports} lamports in fees</>}.
      {' '}A transaction belongs to the boundary that could have seen it: the one whose finalized slot it landed at or before, and after the previous boundary&apos;s. A boundary censused at the same slot as the one before it honestly gets none.
    </p>
    {volume.transactions === null ? null : <>
      {/* FE-CHART mount: transactions per boundary. */}
      <Sparkline
        lines={[volume.transactions]}
        xLabels={volume.xLabels}
        unit="transactions"
        caption="Transactions the campaign submitted between one boundary and the next. This is the only volume a market with no fills has, and it is work rather than trade."
        emptyReason={NO_CAMPAIGN_SENTENCE_V1}
      />
      <LocalNote />
    </>}
    {volume.computeUnits === null ? null : <>
      <h3 className="detail-subhead">And what that work cost the runtime</h3>
      {/* FE-CHART mount: compute units per boundary, its own figure because
          compute and transaction counts are different dimensions. */}
      <Sparkline
        lines={[volume.computeUnits]}
        xLabels={volume.xLabels}
        unit="compute units"
        caption="Compute units those same transactions consumed. Kept on its own axis: a count of transactions and a count of compute units are different dimensions, and one pair of axes for both would be a shape chosen rather than measured."
        emptyReason={NO_CAMPAIGN_SENTENCE_V1}
      />
      <LocalNote />
    </>}
    {spend === null ? null : <>
      <h3 className="detail-subhead">And what it cost the wallet paying for it</h3>
      {/* FE-CHART mount: the fee payer's drawdown. A level, not an interval,
          so it never shares an axis with the counts above. */}
      <Sparkline
        lines={[spend]}
        xLabels={volume.xLabels}
        unit="lamports"
        caption="What the campaign’s fee payer had spent by each boundary, measured as the drop from its balance at the first one. The raw balance is an eighteen-digit genesis figure and the interesting part is the last six digits of it, so the drop is what is drawn."
        emptyReason={NO_CAMPAIGN_SENTENCE_V1}
      />
      <LocalNote />
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
        ? 'This market has not reached a terminal answer in this record, so no claim has a realized value yet.'
        : 'This record names a terminal answer but no claim unit, so what a claim is worth in collateral cannot be stated exactly — and it is not going to be approximated here.'}
    </p>;
  }
  return <>
    <p className="direct-status">
      The terminal certificate selected cell {series.settlement?.selectedCell}. One claim on that cell is worth {series.claimUnitAtoms} collateral atom{series.claimUnitAtoms === '1' ? '' : 's'}; one claim on every other cell is worth nothing. That is the whole of a settlement, and it is the only price move a market without fills ever makes.
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
      : <p className="market-editorial-note">The certificate is at {series.settlement.certificate}, on the rehearsal chain this run started and then stopped. {CAMPAIGN_LOCAL_CAVEAT_V1}</p>}
  </>;
}

/** Every law, at every boundary, with the boundaries named. */
export function CampaignLaws({ series }: Readonly<{ series: SimulatorSeriesV1 }>) {
  const rows = conservationLawRowsV1(series);
  const cycles = lawBandCyclesV1(series);
  const reading = conservationReadingV1(series);
  const labels = campaignStageLabelsV1(series);
  if (rows.length === 0) {
    return <p className="market-empty">This record carries the laws&apos; counts and not their identities, so there is no band to draw.</p>;
  }
  return <>
    {reading === null ? null : <p className="direct-status">{reading}</p>}
    {/* FE-CHART mount: a status band, not a line — a verdict is a state, and a
        line through states invents an ordering between them. */}
    <LawBand
      rows={rows}
      cycles={cycles}
      glosses={LAW_GLOSSES}
      caption="Each row is one conservation law; each column is one stage boundary the campaign re-checked it at. The columns are numbered; the list below says which stage each number is."
      emptyReason={NO_CAMPAIGN_SENTENCE_V1}
    />
    <ol className="market-editorial-note">
      {labels.map((label, index) => <li key={`${index}-${label}`}>{index + 1} · {label}</li>)}
    </ol>
    <LocalNote />
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

  return <PageShell className="product-shell trade-v3-shell" header={<Nav current="/campaign" status="local rehearsal record" />}>

    <section className="trade-v3-hero">
      <div>
        <p className="eyebrow">A campaign · one market&apos;s whole life on a private chain</p>
        <h1>Founded, resolved, retired.<br /><em>On a chain we started for it.</em></h1>
        <p>Everything on this page came off a private validator running on 127.0.0.1 — its own genesis, its own seven programs, nobody else on it. A campaign founds one market there from nothing, publishes the source graph it will resolve against, funds and activates its resolution, carries it to a terminal answer through the real transport, and retires it. Then it re-reads the chain at every boundary and proves the collateral is all still where the ledger says it must be.</p>
        <p>None of this is devnet or mainnet. It is a rehearsal.</p>
      </div>
      <aside>
        <span>Where this stands</span>
        <strong>{state.kind === 'loaded' ? 'One campaign recorded' : state.kind === 'refused' ? 'Refused' : state.kind === 'absent' ? 'Nothing published' : 'Reading…'}</strong>
        {reading === null
          ? <p>{NO_CAMPAIGN_SENTENCE_V1}</p>
          : <p>{reading}</p>}
        {series === null ? null : <p className="market-editorial-note">
          Re-derive this file from the run&apos;s transcript with <code>scripts/campaign-series.mjs --check</code>.
        </p>}
      </aside>
    </section>

    <div className="local-status-strip">
      <i className={series === null ? undefined : 'online'} />
      <strong>{series === null ? 'No campaign record' : series.settlement === null ? 'Founded, not settled' : 'Founded and settled'}</strong>
      <span>{series === null ? NO_CAMPAIGN_SENTENCE_V1 : SERIES_RECORD_CAVEAT_V1}</span>
    </div>

    <section className="trade-v3-card">
      <header><span>01</span><div><h2>Where the claims sit</h2><p>Each outcome&apos;s share of the claims issued against it. Nobody has traded in this one.</p></div></header>
      {body((loaded) => <OddsPath series={loaded} />)}
    </section>

    <section className="trade-v3-card">
      <header><span>02</span><div><h2>What the vault held</h2><p>What was in the market&apos;s vault, against every unit of collateral the ledger could account for anywhere.</p></div></header>
      {body((loaded) => <VaultPath series={loaded} />)}
    </section>

    <section className="trade-v3-card">
      <header><span>03</span><div><h2>The work each stage took</h2><p>Transactions, and what they cost to run.</p></div></header>
      {body((loaded) => <WorkPerStage series={loaded} />)}
    </section>

    <section className="trade-v3-card">
      <header><span>04</span><div><h2>The answer, and what a claim turned out to be worth</h2><p>One outcome pays the full claim unit; every other one pays nothing.</p></div></header>
      {body((loaded) => <Settlement series={loaded} />)}
    </section>

    <section className="trade-v3-card">
      <header><span>05</span><div><h2>The checks, after every stage</h2><p>After each stage the run re-checks that the collateral is all still somewhere we can name, that the positions add up to the claims issued, and that the vault covers the worst outcome it could be asked to pay. Any one failing stops the campaign.</p></div></header>
      {body((loaded) => <CampaignLaws series={loaded} />)}
    </section>

    <footer className="product-footer">
      <span>One campaign&apos;s own transcript · a private validator on 127.0.0.1</span>
    </footer>
  </PageShell>;
}
