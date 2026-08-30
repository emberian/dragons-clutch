'use client';

import { useState } from 'react';

import { formatBasisPointsV1, issuedSupplySharesV1 } from '@/lib/supplyShares';

/**
 * The issuance split: one horizontal parts-of-a-whole strip showing what
 * share of a Market's issued claims sits on each outcome cell.
 *
 * This is the surface the odds refusal always pointed at: the chain stores
 * no order book, no price, and no probability — but it stores exactly how
 * many claims of each outcome exist, and that split is honest to draw as
 * long as it is labelled as issuance and never as a forecast. The caller's
 * caption carries that labelling; this component adds the "evenly split"
 * reading for the founding state so an all-equal strip explains itself.
 *
 * Presentational and exact: shares arrive from `issuedSupplySharesV1` on a
 * 1/10000 grid summing to exactly 100.00%. One series, one hue (identity is
 * position + label, never color); a zero cell renders as a 2-unit sliver at
 * its position rather than vanishing; the exact-value table is built in.
 */

export type SupplyShareStripPropsV1 = Readonly<{
  /** Per-cell issued claim atoms, ordered by claim index. */
  supplies: ReadonlyArray<string>;
  /** Editorial outcome names, index-aligned; the caller states their provenance. */
  outcomes?: ReadonlyArray<string> | null;
  /** One plain sentence naming what the split is and is not. */
  caption: string;
  /** Shown instead of a plot when nothing has been issued. */
  emptyReason?: string;
}>;

const WIDTH = 1000;
const TOP_PAD = 4;
const BAND = 22;
const LABEL_BAND = 14;
const GAP = 2;

export default function SupplyShareStrip({ supplies, outcomes, caption, emptyReason }: SupplyShareStripPropsV1) {
  const [active, setActive] = useState<number | null>(null);
  const split = issuedSupplySharesV1(supplies);

  if (split === null) {
    return <figure className="viz-figure">
      <p className="viz-caption">{emptyReason ?? 'Nothing has been issued on this aggregate, so no split exists to draw.'}</p>
    </figure>;
  }

  const height = TOP_PAD + BAND + LABEL_BAND;
  const cells = split.shares.map((share) => ({
    ...share,
    name: outcomes?.[share.index] ?? null,
    percent: formatBasisPointsV1(share.basisPoints),
  }));

  // Exact-grid x positions: cumulative basis points scaled to the viewBox.
  const placed = cells.map((cell, position) => {
    const before = cells.slice(0, position).reduce((sum, prior) => sum + prior.basisPoints, 0);
    const x = (before * WIDTH) / 10_000;
    const xEnd = ((before + cell.basisPoints) * WIDTH) / 10_000;
    return { ...cell, x, width: xEnd - x };
  });

  const shownIndex = active ?? placed.reduce((widest, cell) => (cell.basisPoints > widest.basisPoints ? cell : widest), placed[0]).index;
  const shown = placed[shownIndex] ?? placed[0];
  const label = (cell: typeof placed[number]) =>
    `claim ${cell.index}${cell.name === null ? '' : ` · ${cell.name}`} · ${cell.percent} of issued claims · ${cell.atoms} atoms`;

  return <figure className="viz-figure">
    <div className="viz-scroll"><svg
      viewBox={`0 0 ${WIDTH} ${height}`}
      role="group"
      aria-label={caption}
      style={{ width: '100%' }}
    >
      {placed.map((cell, position) => {
        const gapBefore = position === 0 ? 0 : GAP / 2;
        const gapAfter = position === placed.length - 1 ? 0 : GAP / 2;
        // A zero share stays visible as a 2-unit sliver at its position.
        const drawnWidth = Math.max(cell.width - gapBefore - gapAfter, 2);
        return <g key={cell.index}>
          <rect
            className="viz-hit"
            x={cell.x}
            y={0}
            width={Math.max(cell.width, 2)}
            height={height}
            tabIndex={0}
            aria-label={label(cell)}
            onPointerEnter={() => setActive(cell.index)}
            onPointerLeave={() => setActive(null)}
            onFocus={() => setActive(cell.index)}
            onBlur={() => setActive(null)}
          ><title>{label(cell)}</title></rect>
          <rect
            className="viz-bar"
            x={cell.x + gapBefore}
            y={TOP_PAD}
            width={drawnWidth}
            height={BAND}
            rx={2}
            fill="var(--viz-mark)"
          />
          {cell.width >= 70 && <text
            x={cell.x + cell.width / 2}
            y={TOP_PAD + BAND + 11}
            textAnchor="middle"
            fontSize={8}
            fill="var(--viz-muted)"
          >{cell.index} · {cell.percent}</text>}
        </g>;
      })}
    </svg></div>
    <p className="viz-readout" aria-live="polite">
      <span className="viz-key" style={{ background: 'var(--viz-mark)' }} />
      <strong>{label(shown)}</strong>
      {split.even && <> — evenly split: issuance has not leaned toward any outcome yet</>}
    </p>
    <details className="viz-table">
      <summary>Exact issued supply behind every share</summary>
      <div className="viz-table-scroll">
        <table>
          <thead><tr><th>Claim</th><th>Share of issued claims</th><th>Issued atoms · raw u64</th></tr></thead>
          <tbody>
            {cells.map((cell) => <tr key={cell.index}>
              <td>{cell.index}{cell.name === null ? '' : ` · ${cell.name}`}</td>
              <td>{cell.percent}</td>
              <td>{cell.atoms}</td>
            </tr>)}
            <tr><td>total</td><td>100.00%</td><td>{split.totalAtoms}</td></tr>
          </tbody>
        </table>
      </div>
    </details>
    <figcaption className="viz-caption">{caption}</figcaption>
  </figure>;
}
