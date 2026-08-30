'use client';

import Sparkline from '@/components/charts/Sparkline';
import {
  everyLineFlatV1,
  issuedSupplyLinesV1,
  simulatorSeriesSpanV1,
  SERIES_RECORD_CAVEAT_V1,
  type SimulatorSeriesReadV1,
} from '@/lib/simulatorSeries';
import { useSimulatorSeriesV1 } from '@/lib/simulatorSeriesClient';

/**
 * A market's issued claims, drawn against the cycles somebody recorded.
 *
 * WHY IT CAN RENDER NOTHING. Exactly one market on this deployment has a
 * recorded run behind it, because exactly one is being exercised by the
 * simulator. Every other market gets no chart here — not an empty frame, not a
 * flat placeholder, nothing — because a market with no recorded history has no
 * history to show, and a row of identical empty figures across a listing would
 * say "we measured this and found nothing" about markets nobody measured.
 *
 * WHY IT IS NOT A SECOND SNAPSHOT. The issuance strip beside it draws the
 * market's supply at one finalized floor, read live. This draws the same
 * quantity across recorded time, read from a captured file. They can disagree,
 * and when they do the strip is the newer of the two — which is why the note
 * under this chart says when the recording stopped.
 */

export type MarketIssuanceHistoryPropsV1 = Readonly<{
  /** The market this chart must be about, or it draws nothing. */
  address: string;
  /** Editorial outcome names, index-aligned; the caller states their provenance. */
  outcomes?: ReadonlyArray<string> | null;
  /** Test seam: a settled read, so no fetch happens. */
  preloaded?: SimulatorSeriesReadV1;
}>;

export default function MarketIssuanceHistory({ address, outcomes, preloaded }: MarketIssuanceHistoryPropsV1) {
  const read = useSimulatorSeriesV1(preloaded);
  if (read === null || read.kind !== 'loaded') return null;
  const series = read.series;
  // The series names one market. Drawing it under any other market's heading
  // would attribute one market's history to another.
  if (series.market !== address) return null;
  const span = simulatorSeriesSpanV1(series);
  if (span === null) return null;

  const lines = issuedSupplyLinesV1(series, outcomes ?? null);
  const covered = span.minutesCovered === null
    ? `${span.cycles} recorded cycles`
    : `${span.cycles} recorded cycles over about ${span.minutesCovered} minute${span.minutesCovered === 1 ? '' : 's'}`;

  return <>
    {/* FE-CHART mount: the same claim atoms the strip above draws at one
        floor, drawn instead across every cycle a run recorded. */}
    <Sparkline
      lines={lines}
      xLabels={series.points.map((point) => `cycle ${point.cycle}`)}
      unit="atoms"
      caption={`Issued claims on each outcome across ${covered}, from a run's own records. Claim counts in raw atoms — not a forecast, and not a rate.`}
      flatNote={everyLineFlatV1(lines)
        ? 'unchanged at every recorded cycle: no trade has landed in this run yet'
        : undefined}
    />
    <p className="slot-clock-note">{SERIES_RECORD_CAVEAT_V1}</p>
  </>;
}
