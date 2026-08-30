'use client';

import { useState } from 'react';

/**
 * The time axis this app did not have.
 *
 * Every other chart here draws one finalized floor: bars for what the accounts
 * hold at a single instant, which is honest and is also the reason the site
 * reads as a museum. A quantity that changes deserves to be drawn changing.
 *
 * SMALL MULTIPLES, NOT A TANGLE. Several lines on one pair of axes need
 * several colors, and this app's chart palette deliberately has exactly one
 * mark hue plus an emphasis (app/charts.css) because no chart here carries two
 * co-equal categorical series. So lines are stacked in their own bands
 * instead, sharing one scale — comparable, individually readable, and still
 * one hue. Identity is position and label, never color.
 *
 * A FLAT LINE IS A RESULT. When nothing moved, this draws the flat line and
 * the caller's `flatNote` says so in words. It does not rescale to manufacture
 * a wiggle out of noise, and it does not hide the chart for being boring: that
 * a quantity has not moved across a hundred readings is a fact about the
 * market, and it is the fact most likely to change next.
 *
 * Presentational only. Every value arrives as an exact decimal string and
 * stays one; floats appear solely as pixel coordinates.
 */

export type SparklineLineV1 = Readonly<{
  /** What this line is, in the reader's words. */
  label: string;
  /** Exact values, oldest first, index-aligned with `xLabels`. */
  values: ReadonlyArray<string>;
}>;

export type SparklinePropsV1 = Readonly<{
  lines: ReadonlyArray<SparklineLineV1>;
  /** One label per point, oldest first: what the reader is hovering. */
  xLabels: ReadonlyArray<string>;
  /** One plain sentence naming what the lines are and where they came from. */
  caption: string;
  /** Shown instead of a plot when there is nothing to draw. */
  emptyReason?: string;
  /** Said in the readout when every value on every line is identical. */
  flatNote?: string;
  /** The unit the values are in, for the readout and the table header. */
  unit?: string;
}>;

const WIDTH = 1000;
const LABEL_BAND = 11;
const PLOT_BAND = 28;
const ROW_GAP = 9;
const AXIS_BAND = 14;
const ROW_HEIGHT = LABEL_BAND + PLOT_BAND;

/** Exact comparison over decimal strings of any width. */
function extremes(values: ReadonlyArray<string>): Readonly<{ low: bigint; high: bigint }> {
  let low = BigInt(values[0]);
  let high = low;
  for (const value of values) {
    const parsed = BigInt(value);
    if (parsed < low) low = parsed;
    if (parsed > high) high = parsed;
  }
  return { low, high };
}

export default function Sparkline({ lines, xLabels, caption, emptyReason, flatNote, unit }: SparklinePropsV1) {
  const [active, setActive] = useState<number | null>(null);

  const points = lines.length === 0 ? 0 : lines[0].values.length;
  const drawable = lines.length > 0
    && points > 0
    && lines.every((line) => line.values.length === points)
    && xLabels.length === points;
  if (!drawable) {
    return <figure className="viz-figure">
      <p className="viz-caption">{emptyReason ?? 'No run has been recorded for this market, so there is no line to draw.'}</p>
    </figure>;
  }

  // One scale for every band, so the stack is comparable rather than a set of
  // independently stretched pictures. A whole-series flat value is drawn on
  // the band's middle line, which is the only placement that does not imply a
  // rise or a fall that never happened.
  const all = lines.flatMap((line) => line.values);
  const { low, high } = extremes(all);
  const span = high - low;
  const flat = span === 0n;
  // Geometry only: a u64 difference exceeds float precision, and a pixel does
  // not. Every exact value stays a string and is printed from the string.
  const fraction = (value: string) => (flat ? 0.5 : Number(BigInt(value) - low) / Number(span));
  const xAt = (index: number) => (points === 1 ? WIDTH / 2 : (index * WIDTH) / (points - 1));

  const height = lines.length * ROW_HEIGHT + (lines.length - 1) * ROW_GAP + AXIS_BAND;
  const shown = active ?? points - 1;
  const readoutValues = lines
    .map((line) => `${line.label} ${line.values[shown]}`)
    .join(' · ');
  const columnLabel = (index: number) =>
    `${xLabels[index]} — ${lines.map((line) => `${line.label} ${line.values[index]}`).join(', ')}${unit === undefined ? '' : ` ${unit}`}`;

  return <figure className="viz-figure">
    <div className="viz-scroll"><svg
      viewBox={`0 0 ${WIDTH} ${height}`}
      role="group"
      aria-label={caption}
      style={{ width: '100%' }}
    >
      {lines.map((line, row) => {
        const top = row * (ROW_HEIGHT + ROW_GAP);
        const plotTop = top + LABEL_BAND;
        const plotBottom = plotTop + PLOT_BAND;
        const yAt = (value: string) => plotBottom - fraction(value) * PLOT_BAND;
        const path = line.values.map((value, index) => `${xAt(index)},${yAt(value)}`).join(' ');
        const lastX = xAt(points - 1);
        const lastY = yAt(line.values[points - 1]);
        return <g key={line.label}>
          <text x={0} y={top + 8} fontSize={8} fill="var(--viz-muted)">{line.label}</text>
          <text x={WIDTH} y={top + 8} fontSize={8} textAnchor="end" fill="var(--viz-ink)">{line.values[shown]}</text>
          <line x1={0} y1={plotBottom} x2={WIDTH} y2={plotBottom} stroke="var(--viz-baseline)" strokeWidth={0.5} />
          <polygon
            points={`0,${plotBottom} ${path} ${lastX},${plotBottom}`}
            fill="var(--viz-mark)"
            fillOpacity={0.1}
          />
          <polyline
            points={path}
            fill="none"
            stroke="var(--viz-mark)"
            strokeWidth={2}
            strokeLinejoin="round"
            strokeLinecap="round"
          />
          <circle cx={lastX} cy={lastY} r={3} fill="var(--viz-mark)" stroke="var(--viz-surface)" strokeWidth={1.5} />
          {active !== null && <circle cx={xAt(active)} cy={yAt(line.values[active])} r={3} fill="var(--viz-accent)" stroke="var(--viz-surface)" strokeWidth={1.5} />}
        </g>;
      })}

      {active !== null && <line
        x1={xAt(active)}
        y1={0}
        x2={xAt(active)}
        y2={height - AXIS_BAND}
        stroke="var(--viz-grid)"
        strokeWidth={1}
      />}

      {/* One hit column per point, spanning every band: a reader picking a
          moment gets every line's value at that moment, not one line's. */}
      {xLabels.map((label, index) => {
        const columnWidth = points === 1 ? WIDTH : WIDTH / points;
        return <rect
          key={label}
          className="viz-hit"
          x={Math.max(xAt(index) - columnWidth / 2, 0)}
          y={0}
          width={columnWidth}
          height={height - AXIS_BAND}
          tabIndex={0}
          aria-label={columnLabel(index)}
          onPointerEnter={() => setActive(index)}
          onPointerLeave={() => setActive(null)}
          onFocus={() => setActive(index)}
          onBlur={() => setActive(null)}
        ><title>{columnLabel(index)}</title></rect>;
      })}

      <text x={0} y={height - 3} fontSize={7} textAnchor="start" fill="var(--viz-muted)">{xLabels[0]}</text>
      {points > 1 && <text x={WIDTH} y={height - 3} fontSize={7} textAnchor="end" fill="var(--viz-muted)">{xLabels[points - 1]}</text>}
    </svg></div>

    <p className="viz-readout" aria-live="polite">
      <span className="viz-key" style={{ background: 'var(--viz-mark)' }} />
      <strong>{xLabels[shown]} · {readoutValues}{unit === undefined ? '' : ` ${unit}`}</strong>
      {flat && flatNote !== undefined && <> — {flatNote}</>}
    </p>

    <details className="viz-table">
      <summary>Exact ends and extremes of every line drawn</summary>
      <div className="viz-table-scroll">
        <table>
          <thead><tr><th>Line</th><th>Oldest{unit === undefined ? '' : ` · ${unit}`}</th><th>Newest</th><th>Lowest</th><th>Highest</th></tr></thead>
          <tbody>
            {lines.map((line) => {
              const bounds = extremes(line.values);
              return <tr key={line.label}>
                <td>{line.label}</td>
                <td>{line.values[0]}</td>
                <td>{line.values[points - 1]}</td>
                <td>{bounds.low.toString()}</td>
                <td>{bounds.high.toString()}</td>
              </tr>;
            })}
          </tbody>
        </table>
      </div>
      <p className="viz-caption">{points} point{points === 1 ? '' : 's'} drawn, from {xLabels[0]} to {xLabels[points - 1]}.</p>
    </details>

    <figcaption className="viz-caption">{caption}</figcaption>
  </figure>;
}
