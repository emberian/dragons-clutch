'use client';

import { useState } from 'react';

import { atomBarPathV1, planAtomBarsV1 } from './atomGeometry';

/**
 * The cell strip: one Market's claim cells across its outcome domain, each
 * bar an exact issued-supply atom count from the Claims aggregate.
 *
 * Presentational only — every number arrives as a prop, already read and
 * decoded by the surface that mounts this. The strip renders one series in
 * one hue; settlement flips it to the emphasis form (the winning cell in the
 * accent, every losing cell de-emphasized), which is a state change the
 * caller also names in words through `notes`, never color alone. The
 * required-backing hairline is the aggregate's own law: backing follows the
 * tallest cell while unsettled and the winning cell once terminal, so the
 * line always touches the top of exactly the cell the basis names.
 *
 * The mounting surfaces keep their ordered per-cell lists as the exact-value
 * table twin, so nothing here is reachable only by hover. Vocabulary note:
 * this component ships no market-data words; heights are issued atoms.
 */

export type CellStripPropsV1 = Readonly<{
  /** Per-cell issued claim atoms, ordered by claim index. */
  supplies: ReadonlyArray<string>;
  /** The winning cell once a terminal receipt is written, else null. */
  winner: number | null;
  /** The aggregate's exact required backing, drawn as the law hairline. */
  requiredBackingAtoms: string | null;
  /** One plain sentence naming the law line, from the aggregate's basis. */
  requiredBackingNote: string | null;
  /** One plain sentence naming what the heights are. */
  caption: string;
  /** Per-cell what-you-get, shown in the readout and on hover/focus. */
  notes?: ReadonlyArray<string>;
  /** The one plain sentence shown instead of a plot when there are no cells. */
  emptyReason?: string;
}>;

const PLOT_HEIGHT = 110;
const TOP_PAD = 8;
const SIDE_PAD = 6;
const AXIS_BAND = 16;

export default function CellStrip({
  supplies,
  winner,
  requiredBackingAtoms,
  requiredBackingNote,
  caption,
  notes,
  emptyReason,
}: CellStripPropsV1) {
  const [active, setActive] = useState<number | null>(null);

  if (supplies.length === 0) {
    return <figure className="viz-figure">
      <p className="viz-caption">{emptyReason ?? 'This aggregate lists no claim cells, so there is nothing to draw.'}</p>
    </figure>;
  }

  const plan = planAtomBarsV1(supplies, { referenceAtoms: requiredBackingAtoms });
  const width = plan.plotWidth + SIDE_PAD * 2;
  const height = TOP_PAD + PLOT_HEIGHT + AXIS_BAND;
  const baselineY = TOP_PAD + PLOT_HEIGHT;
  const terminal = winner !== null;

  // Default readout: the settled winner, else the tallest cell (the extreme).
  const extreme = plan.bars.reduce((tallest, bar) => (bar.share > tallest.share ? bar : tallest), plan.bars[0]);
  const shown = active ?? (terminal ? winner : extreme.index);
  const shownBar = plan.bars[shown] ?? extreme;
  const shownNote = notes?.[shownBar.index] ?? null;

  const slot = plan.barWidth + plan.gap;
  const indexLabels = plan.bars.length <= 12
    ? plan.bars.map((bar) => bar.index)
    : [0, Math.floor((plan.bars.length - 1) / 2), plan.bars.length - 1];

  return <figure className="viz-figure">
    <div className="viz-scroll"><svg
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
        const fill = terminal
          ? (bar.index === winner ? 'var(--viz-accent)' : 'var(--viz-deemph)')
          : 'var(--viz-mark)';
        const label = `claim ${bar.index} · ${bar.atoms} atoms${notes?.[bar.index] === undefined ? '' : ` · ${notes[bar.index]}`}`;
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
    </svg></div>
    <p className="viz-readout" aria-live="polite">
      <span className="viz-key" style={{ background: terminal ? (shownBar.index === winner ? 'var(--viz-accent)' : 'var(--viz-deemph)') : 'var(--viz-mark)' }} />
      <strong>claim {shownBar.index} · {shownBar.atoms} atoms</strong>
      {shownNote !== null && <> — {shownNote}</>}
    </p>
    {requiredBackingAtoms !== null && requiredBackingNote !== null && <p className="viz-readout">
      <span className="viz-key" style={{ background: 'var(--viz-law)' }} />
      <strong>required backing · {requiredBackingAtoms} atoms</strong> — {requiredBackingNote}
    </p>}
    <figcaption className="viz-caption">{caption}</figcaption>
  </figure>;
}
