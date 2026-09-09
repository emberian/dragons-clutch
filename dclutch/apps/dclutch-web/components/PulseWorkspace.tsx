'use client';

import PageShell from '@/components/PageShell';
import { useEffect, useState } from 'react';

import Anchor from '@/components/Anchor';
import AquariumLiveChainPanel from '@/components/AquariumLiveChainPanel';
import Nav from '@/components/Nav';
import LawBand from '@/components/charts/LawBand';
import NumberStrip, { type NumberStripStatV1 } from '@/components/charts/NumberStrip';
import Sparkline from '@/components/charts/Sparkline';
import { marketDetailHrefV1 } from '@dclutch/sdk/marketHref';
import { marketEditorialV1 } from '@dclutch/sdk/marketRegistry';
import {
  campaignSpendLineV1,
  campaignStageLabelsV1,
  conservationLawRowsV1,
  conservationPhaseRuleV1,
  conservationReadingV1,
  hoardCoverageLinesV1,
  everyLineFlatV1,
  holdingsReadingV1,
  isCompleteSetV1,
  issuedSupplyLinesV1,
  lawBandCyclesV1,
  NO_SERIES_SENTENCE_V1,
  readSimulatorSeriesV1,
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
import {
  aquariumBeatV1,
  readAquariumStatusV1,
  type AquariumReadV1,
  type AquariumStatusV1,
} from '@/lib/aquariumStatus';

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

const READING_SENTENCE = 'Loading pulse…';

type PulseSurfaceState = Readonly<{ read: SimulatorReadV1 | null }>;
type AquariumSurfaceState = Readonly<{ read: AquariumReadV1 | null }>;

const UNREAD_STATS: ReadonlyArray<NumberStripStatV1> = Object.freeze([
  Object.freeze({ label: 'Cycles completed', value: null, detail: 'cycles' }),
  Object.freeze({ label: 'Trades landed', value: null, detail: 'transactions' }),
  Object.freeze({ label: 'Wallets trading', value: null, detail: 'wallets' }),
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
    Object.freeze({ label: 'Trades landed', value: String(status.tradesLanded), detail: 'transactions' }),
    Object.freeze({ label: 'Wallets trading', value: String(status.wallets.length), detail: 'wallets' }),
  ]);
}

function provenance(read: SimulatorReadV1 | null): string {
  if (read === null) return READING_SENTENCE;
  if (read.kind === 'absent') return NO_SIMULATOR_SENTENCE_V1;
  if (read.kind === 'refused') return `Status refused: ${read.reason}`;
  const where = read.status.clusterLabel === 'local' ? 'Local validator' : 'Solana devnet';
  return `${where} · updated ${read.status.updatedAt}`;
}

function shortSignature(signature: string): string {
  return signature.length <= 20 ? signature : `${signature.slice(0, 10)}…${signature.slice(-10)}`;
}

function beatFor(read: SimulatorReadV1 | null): SimulatorBeatV1 | null {
  return read !== null && read.kind === 'loaded' ? simulatorBeatV1(read.status, Date.now()) : null;
}

function aquariumStats(status: AquariumStatusV1 | null): ReadonlyArray<NumberStripStatV1> {
  if (status === null) return Object.freeze([
    Object.freeze({ label: 'Active markets observed', value: null, detail: 'not published' }),
    Object.freeze({ label: 'Synthetic fills observed', value: null, detail: 'not published' }),
    Object.freeze({ label: 'Public joining', value: null, detail: 'not published' }),
  ]);
  const openMarkets = status.activity.activeMarkets.filter((market) => market.joinOpen).length;
  return Object.freeze([
    Object.freeze({ label: 'Active markets observed', value: String(status.activity.activeMarkets.length), detail: `limit ${status.limits.maxActiveMarkets}` }),
    Object.freeze({ label: 'Synthetic fills observed', value: String(status.activity.counts.fill), detail: 'fills' }),
    Object.freeze({ label: 'Public joining', value: openMarkets > 0 ? 'open' : 'closed', detail: status.activity.joinNote }),
  ]);
}

function aquariumProvenance(read: AquariumReadV1 | null): string {
  if (read === null) return 'Loading aquarium…';
  if (read.kind === 'absent') return 'No aquarium data is available. Start a run to publish an observation.';
  if (read.kind === 'refused') return `Aquarium status refused: ${read.reason}`;
  return `Devnet · cohort ${read.status.cohort.number} · checked ${read.status.cohort.checkedAt}`;
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
  L1: 'all collateral atoms are held in tracked accounts',
  L2: 'vault movement matches the declared transfer',
  L3: 'position balances match issued claims by outcome',
  L4: 'the vault covers the largest unsettled outcome liability',
  L5: 'tracked collateral delta matches the declared amount',
  L6: 'lamports from closed protocol accounts are accounted for',
  L7: 'fee-payer delta matches transaction fees',
  L8: 'collateral compartment deltas match declared movements',
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
  if (read === null) return <p className="direct-status">Loading run…</p>;
  if (read.kind === 'absent') return <p className="market-empty">{NO_SERIES_SENTENCE_V1}</p>;
  if (read.kind === 'refused') {
    return <p className="market-refusal">Run data refused: {read.reason}</p>;
  }
  const series = read.series;
  const heartbeat = simulatorHeartbeatV1(series);
  const span = simulatorSeriesSpanV1(series);
  if (heartbeat === null || span === null) {
    return <p className="market-empty">Only one cycle recorded.</p>;
  }

  const stats: ReadonlyArray<NumberStripStatV1> = Object.freeze([
    Object.freeze({
      label: 'Chain slots covered',
      value: span.slotsCovered,
      detail: `slots ${span.firstSlot}–${span.lastSlot}`,
    }),
    Object.freeze({
      label: 'Measured slot rate',
      value: heartbeat.measuredSlotRate === null ? null : `${heartbeat.measuredSlotRate}/s`,
      detail: heartbeat.measuredSlotRate === null
        ? 'more timestamps required'
        : 'slots per second',
    }),
    Object.freeze({
      label: 'Ledger checks held',
      value: String(span.checksHeld),
      detail: span.checksBroken === 0
        ? '0 failed'
        : `${span.checksBroken} failed`,
    }),
  ]);

  return <>
    {/* FE-CHART mount: three counts that are all derived, all exact, and all
        about the chain rather than about the robot. */}
    <NumberStrip
      stats={stats}
      provenance={`${span.cycles} boundaries · ${series.censusFile}`}
    />

    {/* FE-CHART mount: how far the chain moved between two readings. */}
    <Sparkline
      lines={[heartbeat.slotAdvance]}
      xLabels={heartbeat.xLabels}
      unit="slots"
      caption="Slot advance · slots · recorded boundaries"
      emptyReason={NO_SERIES_SENTENCE_V1}
    />

    {heartbeat.cadence === null
      ? <p className="market-empty">Timestamps are missing. Publish a capture with recorded times.</p>
      : <>
        <h3 className="detail-subhead">Cadence</h3>
        {/* FE-CHART mount: the run's own rhythm, and its stalls. */}
        <Sparkline
          lines={[heartbeat.cadence]}
          xLabels={heartbeat.xLabels}
          unit="seconds"
          caption="Time between readings · seconds · recorded boundaries"
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
  // THE RULE, PER PHASE, published rather than left in a test. The last
  // boundary drawn is the state this page presents as where things stand, and
  // which laws even APPLY there depends on the Market's phase — so a reader is
  // told which ones retired at that boundary and which are still watching.
  const phaseRule = conservationPhaseRuleV1(series);
  if (rows.length === 0) {
    return <p className="market-empty">
      Law identities are missing. Publish a capture with named law results.
    </p>;
  }
  return <>
    {reading === null ? null : <p className="direct-status">{reading}</p>}
    {phaseRule === null ? null : <p className="slot-clock-note">{phaseRule}</p>}
    {/* FE-CHART mount: a status band, not a line — a verdict is a state, and a
        line through states invents an ordering between them. */}
    <LawBand
      rows={rows}
      cycles={cycles}
      glosses={LAW_GLOSSES}
      caption="Conservation result by law · status · recorded boundaries"
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
  if (read === null) return <p className="direct-status">Loading run…</p>;
  if (read.kind === 'absent') return <p className="market-empty">{NO_SERIES_SENTENCE_V1}</p>;
  if (read.kind === 'refused') {
    return <p className="market-refusal">Run data refused: {read.reason}</p>;
  }

  const series = read.series;
  const span = simulatorSeriesSpanV1(series);
  if (span === null) return <p className="market-empty">{NO_SERIES_SENTENCE_V1}</p>;

  // The chain stores no outcome names; the registry does. A market it has
  // never heard of keeps its claim indices and says nothing else.
  const editorial = series.market === null ? null : marketEditorialV1(series.market);
  const supplyLines = issuedSupplyLinesV1(series, editorial?.outcomes ?? null);
  // Both are null-safe by construction: each returns nothing when the record
  // did not carry the field, which is what a capture taken before the producer
  // wrote it looks like.
  const coverage = hoardCoverageLinesV1(series);
  const spend = campaignSpendLineV1(series);
  // The boundary's own name where the record has one. `campaignStageLabelsV1`
  // has existed for the campaign series since v3 and this page kept counting,
  // because the pulse producer never wrote `stage` — so a census that chains a
  // fee settlement onto a poller's cycles drew `cycle 4` for a boundary whose
  // name is `cohort13-post-fee-settlement`.
  const xLabels = campaignStageLabelsV1(series);

  const covered = span.minutesCovered === null
    ? `${span.slotsCovered} slots of chain`
    : `${span.slotsCovered} slots of chain and about ${span.minutesCovered} minute${span.minutesCovered === 1 ? '' : 's'}`;

  return <>
    <p className="direct-status">
      {span.cycles} boundar{span.cycles === 1 ? 'y' : 'ies'} · {covered} · {series.censusFile}.{' '}
      {/* BOUNDARY, not cycle. A census chains: cohort-13's runs two poller
          cycles from one run, one from a second, and one boundary that no
          poller drove at all — the permissionless fee settlement. Calling
          those four "cycles" says the poller ran four times, which it did
          not. Each one's own name is on the axis below. */}
      {series.pointsOmittedBefore === 0
        ? 'All recorded boundaries shown.'
        : `${series.pointsOmittedBefore} earlier boundar${series.pointsOmittedBefore === 1 ? 'y' : 'ies'} omitted.`}
      {' '}{span.checksHeld} checks passed · {span.checksBroken} failed.
    </p>

    {/* FE-CHART mount: issued claims against the run's own cycle number. */}
    <Sparkline
      lines={supplyLines}
      xLabels={xLabels}
      unit="atoms"
      caption="Issued claims by outcome · atoms · recorded boundaries"
      flatNote={everyLineFlatV1(supplyLines)
        ? 'Unchanged across all boundaries.'
        : undefined}
      emptyReason={NO_SERIES_SENTENCE_V1}
    />

    {/* FE-CHART mount: the collateral, three ways.
        `hoardCoverageLinesV1` and `campaignSpendLineV1` have existed since the
        v3 series and /campaign has drawn both since then, while /pulse could
        draw neither -- not because this page had no room for them but because
        THIS RUN'S PRODUCER dropped `mint_supply` and `payer_lamports` before
        they reached the artifact. They are carried now, so the two charts the
        library already had can finally be pointed at a poller's record.

        The Mint line is what L1 compares the tracked total against, which is
        the law that broke twice in this capture; the gap between the two IS
        the finding, drawn. */}
    {coverage.length > 1 && <>
      <h3 className="detail-subhead">Collateral</h3>
      <Sparkline
        lines={coverage}
        xLabels={xLabels}
        unit="atoms"
        caption="Hoard, tracked collateral, and Mint supply · atoms · recorded boundaries"
        flatNote={everyLineFlatV1(coverage)
          ? 'Unchanged across all boundaries.'
          : undefined}
        emptyReason={NO_SERIES_SENTENCE_V1}
      />
    </>}

    {spend !== null && <>
      <h3 className="detail-subhead">Fees</h3>
      {/* FE-CHART mount: the one quantity a census-only run moves by itself.
          A level, drawn as the drop from the first boundary, because the raw
          balance is eighteen digits and the interesting part is the last six. */}
      <Sparkline
        lines={[spend]}
        xLabels={xLabels}
        unit="lamports"
        caption="Fee-payer spend since first boundary · lamports · recorded boundaries"
        emptyReason={NO_SERIES_SENTENCE_V1}
      />
    </>}

  </>;
}

/**
 * Who is standing in the market, and what each of them holds.
 *
 * This said, in its own words, that calling it a leaderboard would be a lie —
 * "one founding position and two funded participants who have not traded, so
 * an ordering of it ranks nothing". That was true when it was written and it
 * stopped being true on 2026-09-02, when a real crossing moved 200 claims of
 * outcome 0 from the founder to participant-2. The ordering ranks something
 * now, and the sort was already correct: it has always been by total claims.
 *
 * It stays a RECORDED table, from the run's own census — the derived, live
 * ordering of the same market is on the market page, read from the Positions
 * themselves. Two surfaces, two provenances, and each says which it is.
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
    {reading.positionCount > 0 && <div className="viz-table-scroll" tabIndex={0} role="region" aria-label="Claims held, per position and outcome">
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
      <h3 className="detail-subhead">Collateral holders</h3>
      <div className="viz-table-scroll" tabIndex={0} role="region" aria-label="The collateral, and who is holding it">
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
      <p>Collateral atoms by account.</p>
    </>}
  </>;
}

export default function PulseWorkspace({ preloaded, preloadedSeries, preloadedAquarium }: Readonly<{
  /** Test seam: a settled read. The page itself always fetches. */
  preloaded?: SimulatorReadV1;
  /** Test seam: a settled series read. The page itself always fetches. */
  preloadedSeries?: SimulatorSeriesReadV1;
  /** Test seam for the separately published aquarium observation. */
  preloadedAquarium?: AquariumReadV1;
}> = {}) {
  const [state, setState] = useState<PulseSurfaceState>({ read: preloaded ?? null });
  const [series, setSeries] = useState<SimulatorSeriesReadV1 | null>(preloadedSeries ?? null);
  const [aquarium, setAquarium] = useState<AquariumSurfaceState>({ read: preloadedAquarium ?? null });
  // The static snapshot does not refetch itself, but its deadline still has to
  // age on screen. Otherwise a quiet writer can look alive forever.
  const [nowMs, setNowMs] = useState(() => Date.now());

  useEffect(() => {
    const tick = window.setInterval(() => setNowMs(Date.now()), 30_000);
    return () => window.clearInterval(tick);
  }, []);

  useEffect(() => {
    if (preloadedAquarium !== undefined) return undefined;
    let cancelled = false;
    (async () => {
      const read = await readAquariumStatusV1((url) => globalThis.fetch(url, { cache: 'no-store', redirect: 'error', credentials: 'omit' }));
      if (!cancelled) setAquarium({ read });
    })();
    return () => { cancelled = true; };
  }, [preloadedAquarium]);

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
  const aquariumRead = aquarium.read;
  const aquariumStatus = aquariumRead !== null && aquariumRead.kind === 'loaded' ? aquariumRead.status : null;
  const aquariumBeat = aquariumRead !== null && aquariumRead.kind === 'loaded' ? aquariumBeatV1(aquariumRead.status, nowMs) : null;
  // How many laws this record names. Derived, because the count moved: the
  // census gained L8 (compartment accounting) and this page said "seven" in
  // its own words, which would have been a wrong number the day the record
  // changed and no gate anywhere could have noticed.
  const lawCount = series !== null && series.kind === 'loaded' && series.series.lawIds.length > 0
    ? series.series.lawIds.length
    : null;

  /*
    THE PILL FOLLOWS THE BEAT, not the file.

    `status.halted` is a flag inside the last artifact that was written; the
    beat is that artifact judged against the clock. They disagree exactly when
    a run stops writing without halting, which is the common failure, and on
    2026-09-02 the nav said "simulator publishing" beside a strip that said
    "Gone quiet — overdue for its next write" about a file three days old. One
    of the two was reading the liveness question; it is the one that decides
    this pill now.
  */
  const pill = aquariumBeat !== null
    ? aquariumBeat.state === 'running'
      ? 'aquarium publishing'
      : `aquarium ${aquariumBeat.state}`
    : beat === null
    ? (read === null ? 'reading the pulse' : 'no simulator running')
    : beat.state === 'running'
      ? 'simulator publishing'
      : beat.state === 'stopping'
        ? 'simulator winding down'
        : beat.state === 'stale'
          ? 'simulator gone quiet'
          : 'simulator halted';

  return <PageShell className="product-shell trade-v3-shell" header={<Nav current="/pulse" status={pill} />}>

    <section className="trade-v3-hero">
      <div>
        <p className="eyebrow">Activity pulse</p>
        <h1>Synthetic market<br /><em>activity.</em></h1>
        <p>Track active markets, completed cycles, transactions, and conservation checks.</p>
      </div>
      <aside>
        <span>Where this stands</span>
        <strong>{aquariumRead === null
          ? status === null ? 'No simulator running' : status.halted ? 'Simulator halted' : 'Simulator publishing'
          : aquariumRead.kind === 'absent' ? 'No aquarium observation' : aquariumRead.kind === 'refused' ? 'Observation refused' : aquariumBeat?.state === 'running' ? 'Aquarium publishing' : `Aquarium ${aquariumBeat?.state}`}</strong>
        {aquariumRead === null
          ? status === null ? <p>Start the simulator to publish a run.</p> : <p>Last published simulator status.</p>
          : aquariumRead.kind === 'loaded'
          ? <p>{aquariumBeat?.sentence}</p>
          : <p>Publish an aquarium observation to update this view.</p>}
      </aside>
    </section>

    <section className="trade-v3-card">
      <header><span>01</span><div><h2>Aquarium activity</h2><p>Latest published synthetic activity.</p></div></header>
      {aquariumRead === null
        ? <p className="direct-status">Loading aquarium activity…</p>
        : <NumberStrip stats={aquariumStats(aquariumStatus)} provenance={aquariumProvenance(aquariumRead)} />}
      {aquariumStatus !== null && <>
        <div className="trade-v3-evidence">
          <article><span>Cohort record</span><strong>{aquariumStatus.cohort.number}</strong><small>commit {aquariumStatus.cohort.deploymentCommit}</small></article>
          <article><span>Last event</span><strong>{aquariumStatus.activity.lastEventKind ?? 'none'}</strong><small>{aquariumStatus.activity.lastEventAt ?? 'not recorded'}</small></article>
          <article><span>Observed payer outflow</span><strong>{aquariumStatus.run.lamportsSpentObserved}</strong><small>of {aquariumStatus.run.maxLamportsSpent} lamports bounded</small></article>
        </div>
        {aquariumStatus.failure !== null && <p className="market-refusal">Driver exit at {aquariumStatus.failure.at}: {aquariumStatus.failure.detail}</p>}
        {aquariumStatus.activity.activeMarkets.length === 0
          ? <p className="market-empty">No active markets are listed. Run the aquarium to add one.</p>
          : <div className="market-card-grid">
            {aquariumStatus.activity.activeMarkets.map((market) => <article className="market-discovery-card" key={market.marketId}>
              <div className="market-card-top"><span className="provenance-chip">synthetic observation</span><span className="phase-chip">{market.state}</span></div>
              <h3>{market.address === null ? market.marketId : <Anchor href={marketDetailHrefV1(market.address)}>{market.marketId}</Anchor>}</h3>
              <dl className="market-card-facts">
                <div><dt>Public joining</dt><dd>{market.joinOpen ? 'open' : 'closed'}</dd></div>
                <div><dt>Epochs completed</dt><dd>{market.epochsCompleted}</dd></div>
                <div><dt>Epochs precommitted</dt><dd>{market.epochsPrecommitted}</dd></div>
                <div><dt>Observed at</dt><dd>{market.observedAt ?? 'not recorded'}</dd></div>
              </dl>
              {market.joinOpen && market.address !== null
                && <div className="direct-actions"><Anchor className="secondary-action" href={`${marketDetailHrefV1(market.address)}#join`}>Join this market →</Anchor></div>}
            </article>)}</div>}
      </>}
    </section>

    <AquariumLiveChainPanel status={aquariumStatus} />

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
      <span>{beat === null ? (read === null ? READING_SENTENCE : 'Start the simulator to publish a heartbeat.') : beat.sentence}</span>
    </div>

    <section className="trade-v3-card">
      <header><span>03</span><div><h2>Simulator status</h2><p>Last published cycle totals.</p></div></header>
      {/* FE-CHART mount: the pulse feeds the presentational NumberStrip; the
          reader in lib/simulatorStatus.ts decides what may appear here. */}
      <NumberStrip stats={status === null ? UNREAD_STATS : loadedStats(status)} provenance={provenance(read)} />
    </section>

    <section className="trade-v3-card">
      <header><span>04</span><div><h2>Heartbeat</h2><p>Slot and time intervals between readings.</p></div></header>
      <Heartbeat read={series} />
    </section>

    <section className="trade-v3-card">
      <header><span>05</span><div><h2>{lawCount === null ? 'Conservation checks' : `${lawCount} conservation checks`}</h2><p>Result for each law at every recorded boundary.</p></div></header>
      {series !== null && series.kind === 'loaded'
        ? <ConservationLaws series={series.series} />
        : <p className="market-empty">{NO_SERIES_SENTENCE_V1}</p>}
    </section>

    <section className="trade-v3-card">
      <header><span>06</span><div><h2>Run history</h2><p>Claims, collateral, and fees by recorded boundary.</p></div></header>
      <RecordedCycles read={series} />
    </section>

    <section className="trade-v3-card">
      <header><span>07</span><div><h2>Last ledger check</h2><p>Latest reconciliation of lamports and collateral atoms.</p></div></header>
      {status === null || status.lastReconciliation === null
        ? <p className="market-empty">No reconciliation is available. Run a census to add one.</p>
        : <p className="direct-status">
          <span className={`status-chip ${status.lastReconciliation.ok ? 'pass' : 'fail'}`}>{status.lastReconciliation.ok ? 'conserved' : 'violated'}</span>
          {' '}Checked at {status.lastReconciliation.checkedAt}.
          {status.lastReconciliation.detail === null ? '' : ` ${status.lastReconciliation.detail}.`}
          {status.halted && status.haltReason !== null ? ` The simulator halted itself: ${status.haltReason}` : ''}
        </p>}
    </section>

    <section className="trade-v3-card">
      <header><span>08</span><div><h2>Market holdings</h2><p>Positions and collateral accounts at the last recorded boundary.</p></div></header>
      {series !== null && series.kind === 'loaded'
        ? <WhoIsHolding series={series.series} />
        : <p className="market-empty">{NO_SERIES_SENTENCE_V1}</p>}
    </section>

    <section className="trade-v3-card">
      <header><span>09</span><div><h2>Wallets and trades</h2><p>Participant balances and recent transaction signatures.</p></div></header>
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
            ? <p className="market-empty">No trades have landed. Run a funded cycle to add one.</p>
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
      <span>Latest published simulator run</span>
    </footer>
  </PageShell>;
}
