'use client';

import { useState } from 'react';

import { atomBarPathV1, planAtomBarsV1 } from './atomGeometry';

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
  | Readonly<{ kind: 'mergeable'; completeSetsAtoms: string }>
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
const AXIS_BAND = 16;

export default function PositionBars({ balances, claim, caption, emptyReason }: PositionBarsPropsV1) {
  const [active, setActive] = useState<number | null>(null);

  if (balances.length === 0) {
    return <figure className="viz-figure">
      <p className="viz-caption">{emptyReason ?? 'This Position lists no claim balances, so there is nothing to draw.'}</p>
    </figure>;
  }

  const floor = claim.kind === 'mergeable' ? claim.completeSetsAtoms : null;
  const winner = claim.kind === 'redeemable' ? claim.winningClaim : null;
  const plan = planAtomBarsV1(balances, { referenceAtoms: floor });
  const width = plan.plotWidth + SIDE_PAD * 2;
  const height = TOP_PAD + PLOT_HEIGHT + AXIS_BAND;
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
    <svg
      viewBox={`0 0 ${width} ${height}`}
      style={{ maxWidth: `${width}px` }}
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
        y={baselineY + 11}
        textAnchor="middle"
        fontSize={7}
        fill="var(--viz-muted)"
      >{index}</text>)}
    </svg>
    <p className="viz-readout" aria-live="polite">
      <span className="viz-key" style={{ background: winner !== null ? (shownBar.index === winner ? 'var(--viz-accent)' : 'var(--viz-deemph)') : 'var(--viz-mark)' }} />
      <strong>claim {shownBar.index} · {shownBar.atoms} atoms</strong>
      {noteFor(shownBar.index) !== null && <> — {noteFor(shownBar.index)}</>}
    </p>
    {claim.kind === 'mergeable' && <p className="viz-readout">
      <span className="viz-key" style={{ background: 'var(--viz-law)' }} />
      <strong>merge floor · {claim.completeSetsAtoms} complete sets</strong> — {claim.completeSetsAtoms === '0'
        ? 'one claim balance is zero, so no complete set exists to merge'
        : 'the smallest owned balance; each set merges back into exactly one collateral atom'}
    </p>}
    {claim.kind === 'redeemable' && <p className="viz-readout">
      <span className="viz-key" style={{ background: 'var(--viz-accent)' }} />
      <strong>redeemable · {claim.redeemableAtoms} atoms</strong> — winning claim {claim.winningClaim}; every losing claim pays zero
    </p>}
    <figcaption className="viz-caption">{caption}</figcaption>
  </figure>;
}
