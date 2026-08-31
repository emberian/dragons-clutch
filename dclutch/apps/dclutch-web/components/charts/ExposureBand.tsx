import { atomShareV1 } from './atomGeometry';

/**
 * What a set of positions can pay, drawn as bands on one axis.
 *
 * Every row is the interval one thing can pay, on a shared scale that runs
 * from nothing to the widest ceiling on the chart. Each band has two parts and
 * the split is the whole point: the head is what arrives whatever happens and
 * recedes accordingly, the tail is the part the outcomes decide and carries the
 * mark hue. A bundle row drawn above its own legs is visibly longer than any of
 * them, because with nothing shared between the Markets it is exactly their
 * sum — the reader can see the addition rather than take it on faith.
 *
 * One optional hairline marks a narrower ceiling that holds only under a stated
 * condition. It is drawn in the emphasis accent and always labelled in words,
 * never left as a line the reader has to interpret.
 *
 * Presentational and hook-free, so a server render reaches it. Positions are
 * projected by `atomShareV1`, which divides in bigint on a millionth grid, so a
 * u64 near its ceiling cannot lose its low bits on the way to a pixel. The
 * exact atom counts stay in the table twin below the plot.
 */

export type ExposureBandRowV1 = Readonly<{
  label: string;
  /** The least this row can pay, raw atoms. */
  floorAtoms: string;
  /** The most it can pay, raw atoms. */
  ceilingAtoms: string;
  /** Drawn heavier: the total, as against one of its parts. */
  emphasis: boolean;
  /** One short phrase for the row's readout and its table twin. */
  note: string;
}>;

export type ExposureBandPropsV1 = Readonly<{
  rows: ReadonlyArray<ExposureBandRowV1>;
  /** Atoms the full plot width represents; every row is drawn against it. */
  scaleAtoms: string;
  /** A narrower ceiling that holds only conditionally, or null. */
  conditionalCeilingAtoms?: string | null;
  /** The words that condition it. Required whenever the hairline is drawn. */
  conditionalLabel?: string | null;
  caption: string;
  emptyReason?: string;
}>;

const WIDTH = 1000;
const ROW = 18;
const GAP = 8;
const LABEL_BAND = 13;
const TOP_PAD = 4;

export default function ExposureBand({
  rows,
  scaleAtoms,
  conditionalCeilingAtoms,
  conditionalLabel,
  caption,
  emptyReason,
}: ExposureBandPropsV1) {
  if (rows.length === 0 || scaleAtoms === '0') {
    return <figure className="viz-figure">
      <p className="viz-caption">{emptyReason ?? 'Nothing in this read has a band to draw, so no plot is shown rather than an empty one.'}</p>
    </figure>;
  }

  const height = TOP_PAD + rows.length * (ROW + GAP) + LABEL_BAND;
  const hairline = conditionalCeilingAtoms == null || conditionalLabel == null
    ? null
    : Object.freeze({ x: atomShareV1(conditionalCeilingAtoms, scaleAtoms) * WIDTH, atoms: conditionalCeilingAtoms, label: conditionalLabel });

  const placed = rows.map((row, index) => {
    const floor = atomShareV1(row.floorAtoms, scaleAtoms) * WIDTH;
    const ceiling = atomShareV1(row.ceilingAtoms, scaleAtoms) * WIDTH;
    return Object.freeze({
      ...row,
      y: TOP_PAD + index * (ROW + GAP),
      floor,
      ceiling,
      // A band whose ends coincide stays visible as a 2-unit sliver: a position
      // that pays the same whatever happens is a fact, not an empty state.
      decided: Math.max(ceiling - floor, row.floorAtoms === row.ceilingAtoms ? 0 : 2),
    });
  });

  return <figure className="viz-figure">
    <div className="viz-scroll"><svg viewBox={`0 0 ${WIDTH} ${height}`} role="group" aria-label={caption} style={{ width: '100%' }}>
      {placed.map((row) => {
        const title = `${row.label} · pays at least ${row.floorAtoms} and at most ${row.ceilingAtoms} atoms · ${row.note}`;
        return <g key={row.label}>
          <rect x={0} y={row.y} width={WIDTH} height={ROW} fill="var(--viz-mark-wash)" rx={2} />
          {row.floor > 0 && <rect
            className="viz-bar"
            x={0}
            y={row.y}
            width={row.floor}
            height={ROW}
            rx={2}
            fill="var(--viz-deemph)"
            opacity={row.emphasis ? 1 : 0.7}
          />}
          {row.decided > 0 && <rect
            className="viz-bar"
            x={row.floor}
            y={row.y}
            width={row.decided}
            height={ROW}
            rx={2}
            fill="var(--viz-mark)"
            opacity={row.emphasis ? 1 : 0.65}
          />}
          <text x={4} y={row.y + ROW - 5} fontSize={9} fill="var(--viz-ink)">{row.label}</text>
          {/* Focusable and titled on top of the row, so the exact bounds reach
              a pointer and a keyboard alike; the table twin carries them for
              every reader regardless. */}
          <rect className="viz-hit" x={0} y={row.y} width={WIDTH} height={ROW} tabIndex={0} aria-label={title}><title>{title}</title></rect>
        </g>;
      })}
      {hairline !== null && <g>
        <line
          x1={hairline.x}
          x2={hairline.x}
          y1={TOP_PAD}
          y2={TOP_PAD + ROW}
          stroke="var(--viz-accent)"
          strokeWidth={1.5}
          strokeDasharray="3 2"
        ><title>{hairline.label}</title></line>
        <text
          x={Math.min(hairline.x + 4, WIDTH - 4)}
          y={height - 3}
          textAnchor={hairline.x > WIDTH - 220 ? 'end' : 'start'}
          fontSize={8}
          fill="var(--viz-accent)"
        >{hairline.atoms} · {hairline.label}</text>
      </g>}
      <line x1={0} x2={WIDTH} y1={height - LABEL_BAND} y2={height - LABEL_BAND} stroke="var(--viz-baseline)" strokeWidth={0.5} />
    </svg></div>
    <p className="viz-readout">
      <span className="viz-key" style={{ background: 'var(--viz-deemph)' }} />
      <strong>arrives whatever happens</strong>
      <span className="viz-key" style={{ background: 'var(--viz-mark)' }} />
      <strong>decided by the outcome</strong>
    </p>
    <details className="viz-table">
      <summary>Exact atoms behind every band</summary>
      <div className="viz-table-scroll">
        <table>
          <thead><tr><th>Band</th><th>At least · raw u64</th><th>At most · raw u64</th><th>Decided by the outcome</th></tr></thead>
          <tbody>
            {rows.map((row) => <tr key={row.label}>
              <td>{row.label}</td>
              <td>{row.floorAtoms}</td>
              <td>{row.ceilingAtoms}</td>
              <td>{(BigInt(row.ceilingAtoms) - BigInt(row.floorAtoms)).toString()}</td>
            </tr>)}
          </tbody>
        </table>
      </div>
    </details>
    <figcaption className="viz-caption">{caption}</figcaption>
  </figure>;
}
