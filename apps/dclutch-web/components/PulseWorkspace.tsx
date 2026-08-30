'use client';

import { useEffect, useState } from 'react';

import Anchor from '@/components/Anchor';
import Nav from '@/components/Nav';
import LawBand from '@/components/charts/LawBand';
import NumberStrip, { type NumberStripStatV1 } from '@/components/charts/NumberStrip';
import Sparkline from '@/components/charts/Sparkline';
import { marketDetailHrefV1 } from '@/lib/marketHref';
import { marketEditorialV1 } from '@/lib/marketRegistry';
import {
  conservationLawRowsV1,
  conservationReadingV1,
  everyLineFlatV1,
  holdingsReadingV1,
  isCompleteSetV1,
  issuedSupplyLinesV1,
  lawBandCyclesV1,
  NO_SERIES_SENTENCE_V1,
  readSimulatorSeriesV1,
  SERIES_RECORD_CAVEAT_V1,
  simulatorHeartbeatV1,
  simulatorSeriesSpanV1,
  type SimulatorSeriesReadV1,
  type SimulatorSeriesV1,
} from '@/lib/simulatorSeries';
import {
  NO_SIMULATOR_SENTENCE_V1,
  readSimulatorStatusV1,
  simulatorBeatV1,
  type SimulatorBeatV1,
  type SimulatorReadV1,
  type SimulatorStatusV1,
} from '@/lib/simulatorStatus';

/**
 * The exchange's pulse: what the load simulator last wrote, or an honest
 * "nothing is running" when it wrote nothing.
 *
 * The simulator (tools/load-simulator/) is a small robot participant: every
 * cycle it trades through the same public routes a wallet would use, then
 * re-runs the ledger census and refuses to continue if one lamport is out of
 * place. Its status artifact may be published beside this site; this surface
 * reads that one file and renders exactly what it says. No artifact means no
 * pulse — dashes, never zeros, and no sample data standing in.
 *
 * This is the app's first runtime asset fetch. A static host answers a
 * missing path with its fallback page, so the guarded reader in
 * lib/simulatorStatus.ts treats a non-OK answer and an unparseable body as
 * "absent" and reserves "refused" for a real JSON document that failed the
 * decoder — those two must never be conflated, because one is normal and the
 * other is a defect worth showing.
 */

const READING_SENTENCE = 'Looking for a published pulse…';

type PulseSurfaceState = Readonly<{ read: SimulatorReadV1 | null }>;

const UNREAD_STATS: ReadonlyArray<NumberStripStatV1> = Object.freeze([
  Object.freeze({ label: 'Cycles completed', value: null, detail: 'one cycle is one trade round plus one full ledger check' }),
  Object.freeze({ label: 'Trades landed', value: null, detail: 'real transactions, each one confirmed on the chain it names' }),
  Object.freeze({ label: 'Wallets trading', value: null, detail: 'ordinary funded wallets, no special keys' }),
]);

function loadedStats(status: SimulatorStatusV1): ReadonlyArray<NumberStripStatV1> {
  return Object.freeze([
    Object.freeze({
      label: 'Cycles completed',
      value: String(status.cyclesRun),
      detail: status.cyclesTarget === null
        ? 'running until told to stop'
        : `of ${status.cyclesTarget} planned`,
    }),
    Object.freeze({ label: 'Trades landed', value: String(status.tradesLanded), detail: 'real transactions, each one confirmed on the chain it names' }),
    Object.freeze({ label: 'Wallets trading', value: String(status.wallets.length), detail: 'ordinary funded wallets, no special keys' }),
  ]);
}

function provenance(read: SimulatorReadV1 | null): string {
  if (read === null) return READING_SENTENCE;
  if (read.kind === 'absent') return NO_SIMULATOR_SENTENCE_V1;
  if (read.kind === 'refused') return `Refused: a status artifact was published here, and it did not decode — ${read.reason}`;
  const where = read.status.clusterLabel === 'local'
    ? 'a local rehearsal validator, not the public devnet'
    : 'the public Solana devnet';
  return `Read from the simulator's own status artifact, written ${read.status.updatedAt}. It is trading against ${where}.`;
}

function shortSignature(signature: string): string {
  return signature.length <= 20 ? signature : `${signature.slice(0, 10)}…${signature.slice(-10)}`;
}

function beatFor(read: SimulatorReadV1 | null): SimulatorBeatV1 | null {
  return read !== null && read.kind === 'loaded' ? simulatorBeatV1(read.status, Date.now()) : null;
}

/**
 * What each law is FOR, in this site's words rather than the census's.
 *
 * Kept apart from anything the census wrote and rendered as the gloss it is.
 * The census's own sentence about what a law FOUND travels beside it verbatim;
 * these say what the law is asking, which is the part a stranger needs and the
 * part no chain stores. Names follow tools/gauntlet/journey/src/ledger.rs.
 */
const LAW_GLOSSES: Readonly<Record<string, string>> = Object.freeze({
  L1: 'collateral closure — every collateral atom that exists is sitting in an account this census watches',
  L2: 'declared vault movement — the market’s vault moved by exactly what was declared, and by nothing else',
  L3: 'supply agreement — what the positions hold adds up, outcome by outcome, to what the market issued',
  L4: 'full collateralisation — the vault holds at least what the worst outcome could be asked to pay',
  L5: 'stage delta — tracked collateral changed between two readings by exactly the declared amount',
  L6: 'rent conservation — lamports leaving a closed protocol account are accounted for',
  L7: 'lamport accounting — the fee payer’s balance moved by exactly the fees paid',
});

/**
 * THE HEARTBEAT: the two quantities on this page that are actually moving.
 *
 * The census this page draws signs nothing, so it spends nothing, so every
 * quantity it observes about the MARKET holds still — and it should, because a
 * market nobody has traded is a market whose numbers have no business
 * changing. Drawing only those is how a truthful record ends up looking like a
 * dead one.
 *
 * These two are moving the whole time and neither belongs to the simulator:
 * the chain advanced between one reading and the next, and the run took real
 * wall-clock seconds to come back. Together they are the answer to the only
 * question a stranger is really asking here.
 *
 * Two figures, not one. Slots and seconds are different dimensions at
 * different magnitudes, and putting them on one pair of axes would be a
 * dual-axis chart with the scale chosen to make a shape.
 */
export function Heartbeat({ read }: Readonly<{ read: SimulatorSeriesReadV1 | null }>) {
  if (read === null) return <p className="direct-status">Looking for a recorded run…</p>;
  if (read.kind === 'absent') return <p className="market-empty">{NO_SERIES_SENTENCE_V1}</p>;
  if (read.kind === 'refused') {
    return <p className="market-refusal">Refused: a recorded run was published here, and it did not decode — {read.reason}</p>;
  }
  const series = read.series;
  const heartbeat = simulatorHeartbeatV1(series);
  const span = simulatorSeriesSpanV1(series);
  if (heartbeat === null || span === null) {
    return <p className="market-empty">Only one cycle was recorded, and a rate needs two readings to be measured between.</p>;
  }

  const stats: ReadonlyArray<NumberStripStatV1> = Object.freeze([
    Object.freeze({
      label: 'Chain slots covered',
      value: span.slotsCovered,
      detail: `slot ${span.firstSlot} to ${span.lastSlot}, read finalized at each end`,
    }),
    Object.freeze({
      label: 'Measured slot rate',
      value: heartbeat.measuredSlotRate === null ? null : `${heartbeat.measuredSlotRate}/s`,
      detail: heartbeat.measuredSlotRate === null
        ? 'the run did not record enough instants to divide by'
        : 'those slots over the run’s own recorded seconds — measured here, not a published constant',
    }),
    Object.freeze({
      label: 'Ledger checks held',
      value: String(span.checksHeld),
      detail: span.checksBroken === 0
        ? 'and none broke, across every cycle drawn below'
        : `and ${span.checksBroken} did not hold — see the band below`,
    }),
  ]);

  return <>
    {/* FE-CHART mount: three counts that are all derived, all exact, and all
        about the chain rather than about the robot. */}
    <NumberStrip
      stats={stats}
      provenance={`Derived from ${span.cycles} recorded cycles in ${series.censusFile}. Slots and seconds are the run's own readings; the rate is the one divided by the other.`}
    />

    {/* FE-CHART mount: how far the chain moved between two readings. */}
    <Sparkline
      lines={[heartbeat.slotAdvance]}
      xLabels={heartbeat.xLabels}
      unit="slots"
      caption="Slots the chain advanced between one reading and the next. This is the chain's own clock as this run observed it — a tall column is a long wait between readings, not a fast chain."
      emptyReason={NO_SERIES_SENTENCE_V1}
    />

    {heartbeat.cadence === null
      ? <p className="market-empty">Some cycles did not record when they happened, so the cadence between them cannot be drawn without guessing.</p>
      : <>
        <h3 className="detail-subhead">How often it came back</h3>
        {/* FE-CHART mount: the run's own rhythm, and its stalls. */}
        <Sparkline
          lines={[heartbeat.cadence]}
          xLabels={heartbeat.xLabels}
          unit="seconds"
          caption="Wall-clock seconds between consecutive readings. A spike is a reading that took longer to come back — a stall, and then a resume."
          emptyReason={NO_SERIES_SENTENCE_V1}
        />
        <p className="slot-clock-note">
          Shortest interval {heartbeat.shortestGapSeconds} seconds, longest {heartbeat.longestGapSeconds}, across {heartbeat.intervals} intervals.
        </p>
      </>}
  </>;
}

/**
 * Every named conservation law, cycle by cycle.
 *
 * This replaced a sparkline of how MANY checks held. That number was true and
 * it was the wrong shape: it drew a line through a count that sits at six, and
 * a reader learned neither which laws those were nor what any of them
 * compared. The census recorded both all along.
 */
export function ConservationLaws({ series }: Readonly<{ series: SimulatorSeriesV1 }>) {
  const rows = conservationLawRowsV1(series);
  const cycles = lawBandCyclesV1(series);
  const reading = conservationReadingV1(series);
  if (rows.length === 0) {
    return <p className="market-empty">
      This capture was taken before the laws were recorded by name, so it carries their counts and not their identities.
    </p>;
  }
  return <>
    {reading === null ? null : <p className="direct-status">{reading}</p>}
    {/* FE-CHART mount: a status band, not a line — a verdict is a state, and a
        line through states invents an ordering between them. */}
    <LawBand
      rows={rows}
      cycles={cycles}
      glosses={LAW_GLOSSES}
      caption="Each row is one conservation law; each column is one cycle boundary the run checked it at. Open the table for what every law asks and what it found at the newest cycle."
      emptyReason={NO_SERIES_SENTENCE_V1}
    />
  </>;
}

/**
 * The run drawn against time.
 *
 * The counts above are the run's present tense; this is its past, and it is
 * the only surface on this site with a time axis on it. Both lines come from
 * the same recorded censuses — one is what the market issued, the other is
 * whether the ledger still added up — so a reader can see the second holding
 * while the first does or does not move.
 *
 * Exported so its arrangement is pinned by a test rather than by a screenshot.
 */
export function RecordedCycles({ read }: Readonly<{ read: SimulatorSeriesReadV1 | null }>) {
  if (read === null) return <p className="direct-status">Looking for a recorded run…</p>;
  if (read.kind === 'absent') return <p className="market-empty">{NO_SERIES_SENTENCE_V1}</p>;
  if (read.kind === 'refused') {
    return <p className="market-refusal">Refused: a recorded run was published here, and it did not decode — {read.reason}</p>;
  }

  const series = read.series;
  const span = simulatorSeriesSpanV1(series);
  if (span === null) return <p className="market-empty">{NO_SERIES_SENTENCE_V1}</p>;

  // The chain stores no outcome names; the registry does. A market it has
  // never heard of keeps its claim indices and says nothing else.
  const editorial = series.market === null ? null : marketEditorialV1(series.market);
  const supplyLines = issuedSupplyLinesV1(series, editorial?.outcomes ?? null);
  const xLabels = series.points.map((point) => `cycle ${point.cycle}`);

  const covered = span.minutesCovered === null
    ? `${span.slotsCovered} slots of chain`
    : `${span.slotsCovered} slots of chain and about ${span.minutesCovered} minute${span.minutesCovered === 1 ? '' : 's'}`;

  return <>
    <p className="direct-status">
      {span.cycles} recorded cycle{span.cycles === 1 ? '' : 's'} covering {covered}, from the run&apos;s own
      census file {series.censusFile}.{' '}
      {series.pointsOmittedBefore === 0
        ? 'Every cycle the run has recorded is drawn.'
        : `${series.pointsOmittedBefore} earlier cycle${series.pointsOmittedBefore === 1 ? ' is' : 's are'} counted but not drawn.`}
      {' '}{span.checksBroken === 0
        ? `Across all of them the ledger was re-checked ${span.checksHeld} times and held every time.`
        : `Across all of them ${span.checksBroken} check${span.checksBroken === 1 ? '' : 's'} did not hold.`}
    </p>
    <p className="slot-clock-note">{SERIES_RECORD_CAVEAT_V1}</p>

    {/* FE-CHART mount: issued claims against the run's own cycle number. */}
    <Sparkline
      lines={supplyLines}
      xLabels={xLabels}
      unit="atoms"
      caption="Issued claims on each outcome, at every cycle the run recorded. These are claim counts in raw atoms, read from the market's Claims aggregate — not a forecast and not a rate."
      flatNote={everyLineFlatV1(supplyLines)
        ? 'unchanged at every recorded cycle: no trade has landed in this run yet'
        : undefined}
      emptyReason={NO_SERIES_SENTENCE_V1}
    />

  </>;
}

/**
 * Who is standing in the market, and what each of them holds.
 *
 * This is the surface a prediction market would normally call a leaderboard,
 * and calling it that today would be the lie: the record shows one founding
 * position and two funded participants who have not traded, so an ordering of
 * it ranks nothing. It is still worth showing — who is in a market before
 * anything happens is a real thing to know, and it is the exact table that
 * becomes a leaderboard on its own the first time somebody trades.
 *
 * The labels are the operator's, from the run's own configuration. The gloss
 * on what a label means is this site's editorial and is marked as such, the
 * same way market names are.
 */
export function WhoIsHolding({ series }: Readonly<{ series: SimulatorSeriesV1 }>) {
  const reading = holdingsReadingV1(series);
  if (reading.positionCount === 0 && series.collateralHolders.length === 0) {
    return <p className="market-empty">{reading.sentence}</p>;
  }
  return <>
    <p className="direct-status">{reading.sentence}</p>
    {reading.positionCount > 0 && <div className="viz-table-scroll">
      <table className="holders-table">
        <thead><tr><th>Position</th><th>Address</th><th>Claims held, per outcome · raw u64</th><th>Total claims</th></tr></thead>
        <tbody>
          {series.positions.map((position) => <tr key={position.label}>
            <td>{position.label}{isCompleteSetV1(position) ? ' · complete set' : ''}</td>
            <td title={position.address ?? undefined}>{position.address === null ? 'not recorded' : `${position.address.slice(0, 8)}…${position.address.slice(-4)}`}</td>
            <td>{position.claims.join(' · ')}</td>
            <td>{position.totalClaims}</td>
          </tr>)}
        </tbody>
      </table>
    </div>}
    {series.collateralHolders.length > 0 && <>
      <h3 className="detail-subhead">The collateral, and who is holding it</h3>
      <div className="viz-table-scroll">
        <table className="holders-table">
          <thead><tr><th>Account</th><th>Address</th><th>Collateral atoms · raw u64</th></tr></thead>
          <tbody>
            {series.collateralHolders.map((holder) => <tr key={holder.label}>
              <td>{holder.label}{holder.label === 'hoard' ? ' · the market’s own vault' : ''}</td>
              <td title={holder.address ?? undefined}>{holder.address === null ? 'not recorded' : `${holder.address.slice(0, 8)}…${holder.address.slice(-4)}`}</td>
              <td>{holder.atoms}</td>
            </tr>)}
          </tbody>
        </table>
      </div>
      <p className="market-editorial-note">The account names are the run operator&apos;s own labels, not anything the chain stores; the note that the hoard is the market&apos;s vault is this site&apos;s. Every address and every figure beside them is read from the chain. A holder of collateral is not a holder of claims: these accounts hold the token the market settles in, and only a position holds claims on the answer.</p>
    </>}
  </>;
}

export default function PulseWorkspace({ preloaded, preloadedSeries }: Readonly<{
  /** Test seam: a settled read. The page itself always fetches. */
  preloaded?: SimulatorReadV1;
  /** Test seam: a settled series read. The page itself always fetches. */
  preloadedSeries?: SimulatorSeriesReadV1;
}> = {}) {
  const [state, setState] = useState<PulseSurfaceState>({ read: preloaded ?? null });
  const [series, setSeries] = useState<SimulatorSeriesReadV1 | null>(preloadedSeries ?? null);

  useEffect(() => {
    if (preloaded !== undefined) return undefined;
    let cancelled = false;
    (async () => {
      const read = await readSimulatorStatusV1((url) => globalThis.fetch(url, { cache: 'no-store', redirect: 'error', credentials: 'omit' }));
      if (!cancelled) setState({ read });
    })();
    return () => { cancelled = true; };
  }, [preloaded]);

  // The recorded run is a second artifact and a second fetch on purpose: the
  // counts above must appear as soon as the status lands, whether or not any
  // history was ever captured, and a missing series must never keep the
  // present tense off the page.
  useEffect(() => {
    if (preloadedSeries !== undefined) return undefined;
    let cancelled = false;
    (async () => {
      const read = await readSimulatorSeriesV1((url) => globalThis.fetch(url, { cache: 'no-store', redirect: 'error', credentials: 'omit' }));
      if (!cancelled) setSeries(read);
    })();
    return () => { cancelled = true; };
  }, [preloadedSeries]);

  const read = state.read;
  const status = read !== null && read.kind === 'loaded' ? read.status : null;
  const beat = beatFor(read);

  return <main className="product-shell trade-v3-shell">
    <Nav current="/pulse" status={status === null ? 'no simulator running' : status.halted ? 'halted' : 'simulator publishing'} />

    <section className="trade-v3-hero">
      <div>
        <p className="eyebrow">The pulse · a robot trades here so you can watch</p>
        <h1>Is anybody home?<br /><em>Ask the robot.</em></h1>
        <p>We run a small automated trader against the protocol — the simulator. It sends the same transactions you would send, on a loop: fund a wallet, trade, then re-check that every unit of collateral is exactly where the ledger says it must be. If a single check ever fails, it stops loudly and this page shows the stop. This page reads the last thing it wrote, and nothing else.</p>
      </div>
      <aside>
        <span>Where this stands</span>
        <strong>{status === null ? 'No simulator running' : status.halted ? 'Halted — loudly, on purpose' : 'Publishing its pulse'}</strong>
        {status === null
          ? <p>No pulse artifact is published beside this site right now. The simulator exists and runs against local rehearsal validators; when a run publishes here, this page fills in by itself. Until then it stays empty rather than showing you a rehearsal as if it were live.</p>
          : <p>Everything below comes from one file the simulator rewrites after every cycle. It is a report by the robot about itself, checked against the chain it trades on — not an estimate, and not a marketing counter. This site is a set of files, so it carries the robot&apos;s last write before the site was published, not the one happening now; the heartbeat says how old that is.</p>}
      </aside>
    </section>

    <div className="local-status-strip">
      <i className={beat === null ? undefined : beat.state === 'running' ? 'online' : beat.state === 'halted' ? 'offline' : undefined} />
      <strong>
        {beat === null
          ? 'No heartbeat to show'
          : beat.state === 'running'
            ? 'Beating'
            : beat.state === 'stopping'
              ? 'Winding down'
              : beat.state === 'stale'
                ? 'Gone quiet'
                : 'Halted'}
      </strong>
      <span>{beat === null ? (read === null ? READING_SENTENCE : 'nothing published') : beat.sentence}</span>
    </div>

    <section className="trade-v3-card">
      <header><span>01</span><div><h2>The pulse, by the numbers</h2><p>Three counts from the simulator&apos;s last write. A dash is an unread value, never a zero; a read zero is shown as the zero it is.</p></div></header>
      {/* FE-CHART mount: the pulse feeds the presentational NumberStrip; the
          reader in lib/simulatorStatus.ts decides what may appear here. */}
      <NumberStrip stats={status === null ? UNREAD_STATS : loadedStats(status)} provenance={provenance(read)} />
    </section>

    <section className="trade-v3-card">
      <header><span>02</span><div><h2>The heartbeat</h2><p>Two things move between one reading and the next, and neither of them belongs to us: how far the chain got, and how long we took to come back. This is the part of the record that answers whether anything is on the other end of the line.</p></div></header>
      <Heartbeat read={series} />
    </section>

    <section className="trade-v3-card">
      <header><span>03</span><div><h2>Every law, at every boundary</h2><p>Seven conservation laws are re-checked after each cycle — that the collateral is all still somewhere we can name, that the positions add up to the issued supply, that the vault covers the worst outcome it could be asked to pay. Each row is one law; each column is one cycle.</p></div></header>
      {series !== null && series.kind === 'loaded'
        ? <ConservationLaws series={series.series} />
        : <p className="market-empty">{NO_SERIES_SENTENCE_V1}</p>}
    </section>

    <section className="trade-v3-card">
      <header><span>04</span><div><h2>What the run looked like over time</h2><p>Every other chart on this site draws one moment. This one draws the run: one point per cycle, from the run&apos;s own records.</p></div></header>
      <RecordedCycles read={series} />
    </section>

    <section className="trade-v3-card">
      <header><span>05</span><div><h2>The last ledger check</h2><p>After trading, the simulator re-reads the chain and proves conservation: every lamport and every collateral atom accounted for. This is the check that halts it.</p></div></header>
      {status === null || status.lastReconciliation === null
        ? <p className="market-empty">No check has been read. When a run publishes here, its most recent conservation verdict appears in this spot — pass or fail, with its timestamp.</p>
        : <p className="direct-status">
          <span className={`status-chip ${status.lastReconciliation.ok ? 'pass' : 'fail'}`}>{status.lastReconciliation.ok ? 'conserved' : 'violated'}</span>
          {' '}Checked at {status.lastReconciliation.checkedAt}.
          {status.lastReconciliation.detail === null ? '' : ` ${status.lastReconciliation.detail}.`}
          {status.halted && status.haltReason !== null ? ` The simulator halted itself: ${status.haltReason}` : ''}
        </p>}
    </section>

    <section className="trade-v3-card">
      <header><span>06</span><div><h2>Who is in this market</h2><p>Every position on the market and every account holding its collateral, as of the last recorded cycle. This is the table a prediction market usually calls a leaderboard; it is not called that here, because ordering one position ranks nothing. The first trade changes that by itself.</p></div></header>
      {series !== null && series.kind === 'loaded'
        ? <WhoIsHolding series={series.series} />
        : <p className="market-empty">{NO_SERIES_SENTENCE_V1}</p>}
    </section>

    <section className="trade-v3-card">
      <header><span>07</span><div><h2>The wallets and their trades</h2><p>The wallets are ordinary accounts, funded in the open; the signatures are real transactions you can look up yourself.</p></div></header>
      {status === null
        ? <p className="market-empty">{NO_SIMULATOR_SENTENCE_V1}</p>
        : <>
          <div className="trade-v3-evidence">
            {status.wallets.slice(0, 4).map((wallet) => <article key={wallet.address}>
              <span>{wallet.role} · {wallet.source}</span>
              <strong>{wallet.address}</strong>
              <small>{wallet.solLamports === null ? 'balance unread this cycle' : `${wallet.solLamports} lamports`}</small>
            </article>)}
          </div>
          {status.wallets.length > 4
            ? <p className="direct-status">{status.wallets.length - 4} more wallet{status.wallets.length - 4 === 1 ? '' : 's'} in the artifact, not shown here.</p>
            : null}
          {status.signatures.length === 0
            ? <p className="market-empty">No trades have landed yet in this run.</p>
            : <p className="direct-status">
              Latest signatures: {status.signatures.slice(-3).map(shortSignature).join(' · ')}
              {status.market === null
                ? ''
                : ' — traded on '}
              {status.market === null
                ? null
                : <Anchor className="secondary-action" href={marketDetailHrefV1(status.market)}>the market they name →</Anchor>}
            </p>}
        </>}
    </section>

    <footer className="product-footer">
      <span>The robot&apos;s own report · one file, rewritten every cycle</span>
      <span>No estimates · no synthesized runs · a missing file renders as the missing file it is</span>
    </footer>
  </main>;
}
