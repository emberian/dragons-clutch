'use client';

import { useState } from 'react';

import type { ConservationLawRowV1, ConservationLawStatusV1 } from '@/lib/simulatorSeries';

import { FIGURE_LABEL_PX, useFigureScale } from './useFigureScale';

/**
 * Every named conservation law, cycle by cycle.
 *
 * WHY THIS IS NOT A SPARKLINE. A law's verdict is a STATE, not a quantity —
 * held, broken, did not apply — and a line drawn through states invents an
 * ordering between them that does not exist. So this is a band of cells: one
 * row per law, one cell per cycle, position carrying identity and a reserved
 * status color carrying the state.
 *
 * NEVER COLOR ALONE. The status palette is the site's own reserved trio
 * (app/charts.css), and every cell also carries its verdict in words in an
 * accessible label, every row a glyph beside its name, and the whole band an
 * exact table underneath. A reader who sees no color at all still gets the
 * answer.
 *
 * WHAT THE ROWS ARE FOR. A count of checks that held says a number stayed at
 * six. This says which law, and — from the census's own sentence at the newest
 * cycle — what it actually compared. `L4: Hoard 500000000 >= worst outcome
 * 500000000 x unit 1` is the protocol demonstrating solvency in one line, and
 * it was in the data all along.
 *
 * Presentational only: it computes no verdicts and reads no chain.
 */

export type LawBandPropsV1 = Readonly<{
  rows: ReadonlyArray<ConservationLawRowV1>;
  /** The cycle number each column is, oldest first, index-aligned with the rows. */
  cycles: ReadonlyArray<number>;
  /** One plain sentence naming what this band is and where it came from. */
  caption: string;
  /** Shown instead of the band when there is nothing to draw. */
  emptyReason?: string;
  /** This site's editorial gloss on what a law is FOR, by law id. Optional. */
  glosses?: Readonly<Record<string, string>>;
}>;

const WIDTH = 1000;
const ROW_GAP = 3;
/* A row carries its law's id inside its own height, in the gutter reserved to
   the left of the cells. Both are room for text, and text is sized in real
   pixels now (see useFigureScale), so how much room it takes depends on what
   the slot made one unit worth. These are the floors, kept so a full-width band
   lays out exactly as it did before. */
const MIN_ROW_HEIGHT = 13;
const MIN_LABEL_WIDTH = 26;
/** The gutter was 26 units for 8-unit ids; the ids are no longer 8 units. */
const LABEL_ROOM_PER_UNIT = 26 / 8;

/** The reserved status trio. Never used for a series hue, here or anywhere. */
const FILL: Readonly<Record<ConservationLawStatusV1, string>> = Object.freeze({
  holds: 'var(--viz-law-holds)',
  violated: 'var(--viz-law-violated)',
  inapplicable: 'var(--viz-law-inapplicable)',
});

/** The secondary encoding, so the state is never carried by color alone. */
const GLYPH: Readonly<Record<ConservationLawStatusV1, string>> = Object.freeze({
  holds: '█',
  violated: '✕',
  inapplicable: '·',
});

const WORD: Readonly<Record<ConservationLawStatusV1, string>> = Object.freeze({
  holds: 'held',
  violated: 'did not hold',
  inapplicable: 'did not apply',
});

/** What a row is, at a glance, without reading its 400 cells. */
function rowVerdict(row: ConservationLawRowV1): ConservationLawStatusV1 {
  if (row.violated > 0) return 'violated';
  if (row.held > 0) return 'holds';
  return 'inapplicable';
}

/**
 * Consecutive cycles with the same verdict, as one mark.
 *
 * A run of four hundred identical cells is four hundred rectangles that draw
 * exactly the bar one rectangle would, and the page pays for every one of them
 * — the first version of this band shipped 294 KB of markup for seven laws.
 * Collapsing them is a pure rendering change: an unbroken law IS an unbroken
 * run, and the moment one cycle differs the run splits there by itself, which
 * is the shape a reader is scanning for anyway.
 *
 * Only used when the cells are too thin to carry the 2px separator. Wide cells
 * keep their gaps and stay individually countable.
 */
function runsOf(statuses: ReadonlyArray<ConservationLawStatusV1>): ReadonlyArray<Readonly<{ start: number; length: number; status: ConservationLawStatusV1 }>> {
  const runs: Array<{ start: number; length: number; status: ConservationLawStatusV1 }> = [];
  for (const [index, status] of statuses.entries()) {
    const open = runs[runs.length - 1];
    if (open !== undefined && open.status === status) open.length += 1;
    else runs.push({ start: index, length: 1, status });
  }
  return runs;
}

/**
 * One cycle's whole verdict set, short enough to repeat on every column.
 *
 * The long form — every law named with its own verdict — is what the readout
 * under the band says once. Saying it again in four hundred tooltips costs
 * more than it tells, so the columns name only the laws that are NOT simply
 * holding, and count the rest.
 */
function columnSummary(rows: ReadonlyArray<ConservationLawRowV1>, index: number): string {
  const broken = rows.filter((row) => row.statuses[index] === 'violated');
  const skipped = rows.filter((row) => row.statuses[index] === 'inapplicable');
  const held = rows.length - broken.length - skipped.length;
  const parts: string[] = [];
  // The violation leads, always. It is the only state on this band that means
  // something has gone wrong with somebody's collateral.
  if (broken.length > 0) parts.push(`${broken.map((row) => row.id).join(', ')} did not hold`);
  if (held > 0) parts.push(`${held} law${held === 1 ? '' : 's'} held`);
  if (skipped.length > 0) parts.push(`${skipped.map((row) => row.id).join(', ')} did not apply`);
  return parts.join('; ');
}

export default function LawBand({ rows, cycles, caption, emptyReason, glosses }: LawBandPropsV1) {
  const [active, setActive] = useState<number | null>(null);
  const { figureRef, units } = useFigureScale(WIDTH);

  const columns = cycles.length;
  const drawable = rows.length > 0 && columns > 0 && rows.every((row) => row.statuses.length === columns);
  if (!drawable) {
    return <figure className="viz-figure">
      <p className="viz-caption">{emptyReason ?? 'No checks recorded.'}</p>
    </figure>;
  }

  const labelSize = units(FIGURE_LABEL_PX);
  // The id sits inside its row and inside the gutter, so both grow with it —
  // and a wide band, where the text is worth less than a unit each, keeps the
  // geometry it always had.
  const rowHeight = Math.max(MIN_ROW_HEIGHT, labelSize + units(2));
  const labelWidth = Math.max(MIN_LABEL_WIDTH, labelSize * LABEL_ROOM_PER_UNIT);
  const height = rows.length * rowHeight + (rows.length - 1) * ROW_GAP;
  const plotWidth = WIDTH - labelWidth;
  // A 2px surface gap between cells, per the mark spec — but only while the
  // cells are wide enough that the gap reads as separation. Below that the
  // gaps become a stripe TEXTURE covering the whole band, and a reader sees a
  // dashed row whether the law held or not; a solid run is both prettier and
  // more truthful, since an unbroken law IS an unbroken run.
  const cellWidth = plotWidth / columns;
  const gap = cellWidth >= 6 ? 2 : 0;
  const shown = active ?? columns - 1;

  const columnLabel = (index: number) => `cycle ${cycles[index]} — ${columnSummary(rows, index)}`;

  return <figure className="viz-figure">
    <div className="viz-scroll"><svg
      ref={figureRef}
      viewBox={`0 0 ${WIDTH} ${height}`}
      role="group"
      aria-label={caption}
      style={{ width: '100%' }}
    >
      {rows.map((row, index) => {
        const top = index * (rowHeight + ROW_GAP);
        // Gapped cells stay one mark each so they remain countable; gapless
        // ones collapse into runs, which draws the identical picture.
        const marks = gap > 0
          ? row.statuses.map((status, column) => ({ start: column, length: 1, status }))
          : runsOf(row.statuses);
        return <g key={row.id}>
          {/* Centred on the row rather than offset from its bottom edge, so the
              id stays centred at whatever size the slot asks for. */}
          <text x={0} y={top + rowHeight / 2 + labelSize * 0.35} fontSize={labelSize} fill="var(--viz-muted)">{row.id}</text>
          {marks.map((mark) => <rect
            key={cycles[mark.start]}
            x={labelWidth + mark.start * cellWidth}
            y={top}
            width={Math.max(mark.length * cellWidth - gap, 0.5)}
            height={rowHeight}
            rx={gap > 0 ? 2 : 0}
            fill={FILL[mark.status]}
          />)}
        </g>;
      })}

      {/* The column a reader is on, drawn once over the whole band instead of
          as an opacity change on every mark underneath it. */}
      {active !== null && <rect
        x={labelWidth + active * cellWidth}
        y={0}
        width={Math.max(cellWidth, 1)}
        height={height}
        fill="none"
        stroke="var(--viz-law)"
        strokeWidth={1}
      />}

      {/* One hit column spanning every row: picking a moment gives every law's
          verdict at that moment, not one law's. */}
      {cycles.map((cycle, index) => <rect
        key={cycle}
        className="viz-hit"
        x={labelWidth + index * cellWidth}
        y={0}
        width={cellWidth}
        height={height}
        tabIndex={0}
        aria-label={columnLabel(index)}
        onPointerEnter={() => setActive(index)}
        onPointerLeave={() => setActive(null)}
        onFocus={() => setActive(index)}
        onBlur={() => setActive(null)}
      ><title>{columnLabel(index)}</title></rect>)}
    </svg></div>

    <p className="viz-readout" aria-live="polite">
      <strong>cycle {cycles[shown]}</strong>
      {' · '}
      {rows.map((row) => `${row.id} ${WORD[row.statuses[shown]]}`).join(' · ')}
    </p>

    <ul className="law-legend">
      {(['holds', 'inapplicable', 'violated'] as const).map((status) => <li key={status}>
        <i style={{ color: FILL[status] }} aria-hidden="true">{GLYPH[status]}</i>
        {WORD[status]}
      </li>)}
    </ul>

    <details className="viz-table">
      <summary>Each check, and its latest result</summary>
      <div className="viz-table-scroll">
        <table>
          <thead><tr><th>Law</th><th>Held</th><th>Did not apply</th><th>Broke</th><th>What it checked, at the newest cycle</th></tr></thead>
          <tbody>
            {rows.map((row) => {
              const verdict = rowVerdict(row);
              return <tr key={row.id}>
                <td>
                  <i style={{ color: FILL[verdict] }} aria-hidden="true">{GLYPH[verdict]}</i>{' '}
                  {row.id}
                  {glosses?.[row.id] === undefined ? null : <><br /><small>{glosses[row.id]}</small></>}
                </td>
                <td>{row.held}</td>
                <td>{row.inapplicable}</td>
                <td>{row.violated}</td>
                <td>{row.detail ?? 'not recorded'}</td>
              </tr>;
            })}
          </tbody>
        </table>
      </div>
      <p className="viz-caption">
        {columns} cycle{columns === 1 ? '' : 's'} drawn, from cycle {cycles[0]} to cycle {cycles[columns - 1]}.
        The sentence in the last column is the census&apos;s own, verbatim; any short note under a law&apos;s name
        is this site&apos;s gloss on what that law is for.
      </p>
    </details>

    <figcaption className="viz-caption">{caption}</figcaption>
  </figure>;
}
