/**
 * The landing number strip: a row of stat tiles, nothing more.
 *
 * A handful of headline numbers is not a chart (per the dataviz form
 * heuristic), so this is the one chart component with no plot and no hover
 * layer. It is also deliberately hook-free so a server component can render
 * it. Values arrive as display-ready exact strings; a tile whose number has
 * not been read carries an em dash and the strip's provenance sentence says
 * why — an unread value is never shown as zero, and a read zero is shown as
 * the zero it is.
 *
 * A tile may also carry PARTS instead of one value. Some quantities genuinely
 * have no single number: collateral held in two different tokens is two totals
 * in two different units, and adding them is not a rounding compromise but a
 * category error. The old answer was an em dash, which said "we could not read
 * this" about something we had read perfectly well. Parts let the tile show
 * what it actually holds — one exact figure per unit, each labelled with the
 * unit it is in.
 */

export type NumberStripPartV1 = Readonly<{
  /** Display-ready exact value for this one part. */
  value: string;
  /** The unit, source, or subject this part's value is in. */
  label: string;
}>;

export type NumberStripStatV1 = Readonly<{
  label: string;
  /** Display-ready exact value, or null when nothing has been read. */
  value: string | null;
  detail: string;
  /**
   * Per-unit figures shown in place of `value`, when one number would have to
   * span units that cannot be added. An empty or absent list falls back to
   * `value`, so a tile that acquires parts only when it needs them is fine.
   */
  parts?: ReadonlyArray<NumberStripPartV1>;
}>;

export type NumberStripPropsV1 = Readonly<{
  stats: ReadonlyArray<NumberStripStatV1>;
  /** One plain sentence: where these numbers come from, or why there are none. */
  provenance: string;
}>;

export default function NumberStrip({ stats, provenance }: NumberStripPropsV1) {
  return <figure className="viz-figure">
    <div className="viz-strip">
      {stats.map((stat) => <article key={stat.label}>
        <span>{stat.label}</span>
        {stat.parts === undefined || stat.parts.length === 0
          ? <strong>{stat.value ?? '—'}</strong>
          : <ul className="viz-strip-parts">
            {stat.parts.map((part) => <li key={part.label}>
              <strong>{part.value}</strong>
              <small>{part.label}</small>
            </li>)}
          </ul>}
        <small>{stat.detail}</small>
      </article>)}
    </div>
    <figcaption className="viz-strip-note">{provenance}</figcaption>
  </figure>;
}
