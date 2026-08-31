'use client';

import { useState, type PointerEvent } from 'react';

import { evaluateProductV2, type CompiledProductV2 } from '@/lib/productV2';

import { atomShareV1, maxAtomsV1, planRationalPositionsV1 } from './atomGeometry';
import { FIGURE_AXIS_PX, useFigureScale } from './useFigureScale';

/**
 * The payout shape: what one payoff pays at every point of its result domain.
 *
 * The record's terms are piecewise linear between its knots and constant
 * outside them, so the curve IS its control polygon: evaluate exactly at
 * every knot, join the points with straight lines, and extend both ends
 * flat. Nothing here is sampled — every plotted value is the exact bigint
 * evaluation the caller supplies, and only the projection to screen
 * coordinates rounds (in bigint, to one millionth of the plot).
 *
 * Presentational only: the knot values arrive as props. The adapter below
 * derives them from a compiled Product V2 record with the same exact
 * evaluator the byte-level tests pin.
 */

export type PayoutShapeKnotV1 = Readonly<{
  /** Exact knot coordinate numerator (decimal, i128 range). */
  numerator: string;
  /** Exact payout at this knot, in scaled payout atoms. */
  payoutAtoms: string;
}>;

export type PayoutShapePropsV1 = Readonly<{
  knots: ReadonlyArray<PayoutShapeKnotV1>;
  /** The shared positive denominator under every knot numerator. */
  knotDenominator: string;
  /** The full payout unit, drawn as the law hairline. */
  payoutScale: string;
  /** One plain sentence naming the curve. */
  caption: string;
  /** Your held atoms against this shape; shading deepens and the note joins the readout. */
  position?: Readonly<{ atoms: string; note: string }> | null;
  emptyReason?: string;
}>;

/** Exact knot evaluations of a compiled Product V2 payoff — no sampling. */
export function payoutShapeKnotsFromCompiledProductV2(compiled: CompiledProductV2): ReadonlyArray<PayoutShapeKnotV1> {
  return Object.freeze(compiled.input.knots.map((numerator) => Object.freeze({
    numerator: numerator.toString(),
    payoutAtoms: evaluateProductV2(compiled, numerator, compiled.input.knotDenominator).toString(),
  })));
}

const PLOT_WIDTH = 460;
const PLOT_HEIGHT = 130;
const TAIL = 34;
const TOP_PAD = 8;
/* The band under the baseline holds the two coordinate labels, and text is
   sized in real pixels now (see useFigureScale), so the room it needs depends
   on what the slot made one unit worth. This is the floor, kept so a full-width
   figure lays out exactly as it did before. */
const MIN_AXIS_BAND = 16;

function shortRational(numerator: string, denominator: string): string {
  const trim = (value: string) => (value.length > 12 ? `${value.slice(0, 7)}…${value.slice(-4)}` : value);
  return denominator === '1' ? trim(numerator) : `${trim(numerator)}/${trim(denominator)}`;
}

export default function PayoutShape({
  knots,
  knotDenominator,
  payoutScale,
  caption,
  position,
  emptyReason,
}: PayoutShapePropsV1) {
  const [active, setActive] = useState<number | null>(null);
  // The viewBox width is fixed by the plot and its two clamped tails, so it is
  // known before the empty check — which the measurement hook needs it to be.
  const width = TAIL + PLOT_WIDTH + TAIL;
  const { figureRef, units } = useFigureScale(width);

  if (knots.length < 2) {
    return <figure className="viz-figure">
      <p className="viz-caption">{emptyReason ?? 'Not enough points to draw a shape.'}</p>
    </figure>;
  }

  const ceiling = knots.reduce((most, knot) => maxAtomsV1(most, knot.payoutAtoms), payoutScale);
  const xs = planRationalPositionsV1(knots.map((knot) => knot.numerator));
  const axisSize = units(FIGURE_AXIS_PX);
  const axisBand = Math.max(MIN_AXIS_BAND, axisSize + units(5));
  const height = TOP_PAD + PLOT_HEIGHT + axisBand;
  const baselineY = TOP_PAD + PLOT_HEIGHT;
  const points = knots.map((knot, index) => Object.freeze({
    knot,
    index,
    x: TAIL + xs[index] * PLOT_WIDTH,
    y: baselineY - PLOT_HEIGHT * atomShareV1(knot.payoutAtoms, ceiling),
  }));
  const first = points[0];
  const last = points[points.length - 1];
  const scaleY = baselineY - PLOT_HEIGHT * atomShareV1(payoutScale, ceiling);

  // The extreme is the default readout subject.
  const peak = points.reduce((highest, point) => (point.y < highest.y ? point : highest), points[0]);
  const shown = points[active ?? peak.index];

  const curve = `0 ${first.y} ${points.map((point) => `${point.x} ${point.y}`).join(' ')} ${width} ${last.y}`;
  const wash = `0,${first.y} ${points.map((point) => `${point.x},${point.y}`).join(' ')} ${width},${last.y} ${width},${baselineY} 0,${baselineY}`;

  function nearestKnot(event: PointerEvent<SVGSVGElement>): number {
    const box = event.currentTarget.getBoundingClientRect();
    const x = ((event.clientX - box.left) / Math.max(box.width, 1)) * width;
    let best = 0;
    for (let index = 1; index < points.length; index += 1) {
      if (Math.abs(points[index].x - x) < Math.abs(points[best].x - x)) best = index;
    }
    return best;
  }

  return <figure className="viz-figure">
    <svg
      ref={figureRef}
      viewBox={`0 0 ${width} ${height}`}
      style={{ maxWidth: `${width}px` }}
      role="group"
      aria-label={caption}
      onPointerMove={(event) => setActive(nearestKnot(event))}
      onPointerLeave={() => setActive(null)}
    >
      <polygon points={wash} fill="var(--viz-mark)" fillOpacity={position == null ? 0.1 : 0.16} />
      <line x1={0} x2={width} y1={scaleY} y2={scaleY} stroke="var(--viz-law)" strokeWidth={1} />
      {active !== null && <line
        x1={points[active].x}
        x2={points[active].x}
        y1={TOP_PAD}
        y2={baselineY}
        stroke="var(--viz-grid)"
        strokeWidth={1}
      />}
      <polyline
        points={curve}
        fill="none"
        stroke="var(--viz-mark)"
        strokeWidth={2}
        strokeLinejoin="round"
        strokeLinecap="round"
      />
      {points.map((point) => {
        const label = `pays ${point.knot.payoutAtoms} of ${payoutScale} scaled payout atoms at ${point.knot.numerator}/${knotDenominator}`;
        return <circle
          key={point.index}
          cx={point.x}
          cy={point.y}
          r={active === point.index ? 5 : 4}
          fill="var(--viz-mark)"
          stroke="var(--viz-surface)"
          strokeWidth={2}
          tabIndex={0}
          aria-label={label}
          onFocus={() => setActive(point.index)}
          onBlur={() => setActive(null)}
        ><title>{label}</title></circle>;
      })}
      <line x1={0} x2={width} y1={baselineY} y2={baselineY} stroke="var(--viz-baseline)" strokeWidth={1} />
      <text x={first.x} y={height - units(5)} textAnchor="start" fontSize={axisSize} fill="var(--viz-muted)">{shortRational(knots[0].numerator, knotDenominator)}</text>
      <text x={last.x} y={height - units(5)} textAnchor="end" fontSize={axisSize} fill="var(--viz-muted)">{shortRational(knots[knots.length - 1].numerator, knotDenominator)}</text>
    </svg>
    <p className="viz-readout" aria-live="polite">
      <span className="viz-key" style={{ background: 'var(--viz-mark)' }} />
      <strong>{shown.knot.payoutAtoms} scaled payout atoms</strong> at {shown.knot.numerator}/{knotDenominator}
      {position != null && <> — {position.note}</>}
    </p>
    <p className="viz-readout">
      <span className="viz-key" style={{ background: 'var(--viz-law)' }} />
      <strong>payout scale · {payoutScale} atoms</strong> — one full unit; flat beyond the first and last knot (the clamped tails)
    </p>
    <figcaption className="viz-caption">{caption}</figcaption>
    <details className="viz-table">
      <summary>Exact numbers</summary>
      <div className="viz-table-scroll">
        <table>
          <thead><tr><th>Coordinate</th><th>Pays · scaled payout atoms</th></tr></thead>
          <tbody>
            <tr><td>below {knots[0].numerator}/{knotDenominator}</td><td>{knots[0].payoutAtoms} (clamped)</td></tr>
            {knots.map((knot) => <tr key={knot.numerator}><td>{knot.numerator}/{knotDenominator}</td><td>{knot.payoutAtoms}</td></tr>)}
            <tr><td>above {knots[knots.length - 1].numerator}/{knotDenominator}</td><td>{knots[knots.length - 1].payoutAtoms} (clamped)</td></tr>
          </tbody>
        </table>
      </div>
    </details>
  </figure>;
}
