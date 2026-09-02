'use client';

import { useState } from 'react';

import { atomBarPathV1, planAtomBarsV1 } from './atomGeometry';
import { FIGURE_AXIS_PX, useFigureScale } from './useFigureScale';

/**
 * One owner's claim balances in one Market, as bars — with the one line the
 * Market's own phase draws through them.
 *
 * While the Market is Open, the merge floor: the smallest owned balance is
 * exactly the number of complete sets these balances can merge back into
 * collateral, so a hairline at that height says at a glance how much of the
 * position is a round trip and how much is a stance. Once a terminal receipt
 * is written, the winning claim carries the accent and every losing claim
 * de-emphasizes — the same emphasis flip the cell strip makes, always paired
 * with words, never color alone.
 *
 * Presentational only; the mounting surface's ordered balance list stays as
 * the exact-value table twin.
 */

export type PositionBarsClaimV1 =
  | Readonly<{
    kind: 'mergeable';
    completeSetsAtoms: string;
    /**
     * Collateral all these sets return, or `null` when the caller has not
     * authenticated the Market's basis scale. Passed in rather than assumed:
     * this figure has no chain access and cannot read `ProductBasisV3`.
     */
    mergeableCollateralAtoms?: string | null;
  }>
  | Readonly<{ kind: 'redeemable'; winningClaim: number; redeemableAtoms: string }>
  | Readonly<{ kind: 'unavailable' }>;

export type PositionBarsPropsV1 = Readonly<{
  /** Owned claim atoms, ordered by claim index. */
  balances: ReadonlyArray<string>;
  claim: PositionBarsClaimV1;
  /** One plain sentence naming what the heights are. */
  caption: string;
  emptyReason?: string;
}>;

const PLOT_HEIGHT = 90;
const TOP_PAD = 8;
const SIDE_PAD = 6;
/* The band under the baseline holds text, and text is sized in real pixels now
   (see useFigureScale), so the room it needs depends on what the slot made one
   unit worth. This is the floor, kept so a full-width figure lays out exactly
   as it did before. */
const MIN_AXIS_BAND = 16;

export default function PositionBars({ balances, claim, caption, emptyReason }: PositionBarsPropsV1) {
  const [active, setActive] = useState<number | null>(null);

  const floor = claim.kind === 'mergeable' ? claim.completeSetsAtoms : null;
  const winner = claim.kind === 'redeemable' ? claim.winningClaim : null;
  // Planned before the empty check, because this figure's viewBox width is its
  // own bar plan and the measurement hook has to run on every render.
  const plan = balances.length === 0 ? null : planAtomBarsV1(balances, { referenceAtoms: floor });
  const width = plan === null ? 0 : plan.plotWidth + SIDE_PAD * 2;
  const { figureRef, units } = useFigureScale(width);

  if (plan === null) {
    return <figure className="viz-figure">
      <p className="viz-caption">{emptyReason ?? 'No claims held.'}</p>
    </figure>;
  }

  const axisSize = units(FIGURE_AXIS_PX);
  const axisBand = Math.max(MIN_AXIS_BAND, axisSize + units(5));
  const height = TOP_PAD + PLOT_HEIGHT + axisBand;
  const baselineY = TOP_PAD + PLOT_HEIGHT;
  const slot = plan.barWidth + plan.gap;

  const extreme = plan.bars.reduce((tallest, bar) => (bar.share > tallest.share ? bar : tallest), plan.bars[0]);
  const shownBar = plan.bars[active ?? (winner ?? extreme.index)] ?? extreme;
  const noteFor = (index: number): string | null => {
    if (winner !== null) return index === winner ? 'winning · admitted to redemption' : 'losing · pays zero';
    if (floor !== null) return 'merges only as part of a complete set';
    return null;
  };

  const indexLabels = plan.bars.length <= 12
    ? plan.bars.map((bar) => bar.index)
    : [0, Math.floor((plan.bars.length - 1) / 2), plan.bars.length - 1];

  return <figure className="viz-figure">
    <div className="viz-scroll"><svg
      ref={figureRef}
      viewBox={`0 0 ${width} ${height}`}
      style={{ maxWidth: `${width}px`, minWidth: plan.bars.length > 24 ? `${width}px` : undefined }}
      role="group"
      aria-label={caption}
    >
      {plan.referenceShare !== null && plan.referenceShare > 0 && <line
        x1={SIDE_PAD}
        x2={width - SIDE_PAD}
        y1={TOP_PAD + PLOT_HEIGHT * (1 - plan.referenceShare)}
        y2={TOP_PAD + PLOT_HEIGHT * (1 - plan.referenceShare)}
        stroke="var(--viz-law)"
        strokeWidth={1}
      />}
      {plan.bars.map((bar) => {
        const x = SIDE_PAD + bar.index * slot;
        // A nonzero count is never invisible: floor its bar at 2 units.
        const top = baselineY - (bar.zero ? 0 : Math.max(PLOT_HEIGHT * bar.share, 2));
        const fill = winner !== null
          ? (bar.index === winner ? 'var(--viz-accent)' : 'var(--viz-deemph)')
          : 'var(--viz-mark)';
        const note = noteFor(bar.index);
        const label = `claim ${bar.index} · ${bar.atoms} atoms${note === null ? '' : ` · ${note}`}`;
        return <g key={bar.index}>
          <rect
            className="viz-hit"
            x={x - plan.gap / 2}
            y={0}
            width={slot}
            height={height}
            tabIndex={0}
            aria-label={label}
            onPointerEnter={() => setActive(bar.index)}
            onPointerLeave={() => setActive(null)}
            onFocus={() => setActive(bar.index)}
            onBlur={() => setActive(null)}
          ><title>{label}</title></rect>
          {bar.zero
            ? <line className="viz-bar" x1={x} x2={x + plan.barWidth} y1={baselineY} y2={baselineY} stroke={fill} strokeWidth={2} />
            : <path className="viz-bar" d={atomBarPathV1(x, top, baselineY, plan.barWidth)} fill={fill} />}
        </g>;
      })}
      <line x1={SIDE_PAD} x2={width - SIDE_PAD} y1={baselineY} y2={baselineY} stroke="var(--viz-baseline)" strokeWidth={1} />
      {indexLabels.map((index) => <text
        key={index}
        x={SIDE_PAD + index * slot + plan.barWidth / 2}
        y={height - units(5)}
        textAnchor="middle"
        fontSize={axisSize}
        fill="var(--viz-muted)"
      >{index}</text>)}
    </svg></div>
    <p className="viz-readout" aria-live="polite">
      <span className="viz-key" style={{ background: winner !== null ? (shownBar.index === winner ? 'var(--viz-accent)' : 'var(--viz-deemph)') : 'var(--viz-mark)' }} />
      <strong>claim {shownBar.index} · {shownBar.atoms} atoms</strong>
      {noteFor(shownBar.index) !== null && <> — {noteFor(shownBar.index)}</>}
    </p>
    {claim.kind === 'mergeable' && <p className="viz-readout">
      <span className="viz-key" style={{ background: 'var(--viz-law)' }} />
      <strong>merge floor · {claim.completeSetsAtoms} complete sets</strong> — {claim.completeSetsAtoms === '0'
        ? 'one claim balance is zero, so no complete set exists to merge'
        : (claim.mergeableCollateralAtoms ?? null) === null
          // The set count is scale-free; what a set is WORTH is `basis_scale`
          // atoms, and no record carrying it has been read here. Saying "one
          // collateral atom" would be the scale-1 assumption drawn as a fact.
          ? 'the smallest owned balance; what each set is worth in collateral is this Market’s basis scale, which this figure has not read'
          : `the smallest owned balance; these sets merge back into ${claim.mergeableCollateralAtoms} collateral atoms`}
    </p>}
    {claim.kind === 'redeemable' && <p className="viz-readout">
      <span className="viz-key" style={{ background: 'var(--viz-accent)' }} />
      <strong>redeemable · {claim.redeemableAtoms} atoms</strong> — winning claim {claim.winningClaim}; every losing claim pays zero
    </p>}
    <figcaption className="viz-caption">{caption}</figcaption>
  </figure>;
}
