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
 */

export type NumberStripStatV1 = Readonly<{
  label: string;
  /** Display-ready exact value, or null when nothing has been read. */
  value: string | null;
  detail: string;
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
        <strong>{stat.value ?? '—'}</strong>
        <small>{stat.detail}</small>
      </article>)}
    </div>
    <figcaption className="viz-strip-note">{provenance}</figcaption>
  </figure>;
}
