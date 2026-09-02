import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import type { ConservationLawRowV1 } from '@/lib/simulatorSeries';

import LawBand from './LawBand';

/**
 * The status band. Its risks are not a line chart's.
 *
 * A verdict is a STATE, and the two ways to get a status band wrong are to
 * carry the state in color alone — which loses every reader who cannot
 * separate the two hues, and every printed copy — and to let the columns drift
 * out of alignment with the cycles, which silently reports one cycle's verdict
 * under another cycle's number. Both are pinned here.
 */
const rows: ReadonlyArray<ConservationLawRowV1> = [
  { id: 'L1', statuses: ['holds', 'holds', 'holds'], held: 3, violated: 0, inapplicable: 0, detail: 'tracked 10 atoms across 2 accounts == Mint supply 10' },
  { id: 'L2', statuses: ['inapplicable', 'holds', 'holds'], held: 2, violated: 0, inapplicable: 1, detail: 'the Hoard moved 0 atoms, exactly as declared' },
];
const cycles = [7, 8, 9];

describe('LawBand', () => {
  const html = renderToStaticMarkup(
    <LawBand rows={rows} cycles={cycles} caption="Each row is one law." glosses={{ L1: 'collateral closure' }} />,
  );

  it('draws one row per law and one cell per cycle', () => {
    // Three columns are wide, so every cell keeps its 2px separator and stays
    // individually countable: 2 laws x 3 cycles, plus one hit column each.
    expect((html.match(/<rect/g) ?? []).length).toBe(rows.length * cycles.length + cycles.length);
    expect(html).toContain('>L1<');
    expect(html).toContain('>L2<');
  });

  it('names each verdict in words, so the state never rests on the color', () => {
    expect(html).toContain('cycle 9 — 2 laws held');
    expect(html).toContain('cycle 7 — 1 law held; L2 did not apply');
    expect(html).toContain('did not hold');
  });

  /**
   * A run of four hundred identical cells draws exactly the bar one rectangle
   * would, and the page pays for every one of them. This pins the collapse, and
   * pins that a differing cycle still splits the run where it differs.
   */
  it('collapses an unbroken run into one mark once the cells are too thin to separate', () => {
    // Typed at the binding rather than `as const` on the branch: a const
    // assertion may only be applied to a literal, and a conditional is not one.
    const long: ReadonlyArray<'violated' | 'holds'> = Array.from(
      { length: 300 }, (_unused, index) => (index === 150 ? 'violated' : 'holds'),
    );
    const dense = renderToStaticMarkup(<LawBand
      rows={[{ id: 'L1', statuses: long, held: 299, violated: 1, inapplicable: 0, detail: 'd' }]}
      cycles={long.map((_unused, index) => index + 1)}
      caption="dense"
    />);
    const marks = [...dense.matchAll(/<rect x="[\d.]+" y="0" width="[\d.]+" height="13"/g)];
    expect(marks).toHaveLength(3);
    expect(dense).toContain('var(--viz-law-violated)');
    expect(dense).toContain('cycle 151 — L1 did not hold');
  });

  it('reserves the status colors and does not reach for a series hue', () => {
    expect(html).toContain('var(--viz-law-holds)');
    expect(html).toContain('var(--viz-law-inapplicable)');
    expect(html).not.toContain('var(--viz-mark)');
  });

  it('carries the exact counts and the census’s own sentence in a table', () => {
    expect(html).toContain('<td>3</td>');
    expect(html).toContain('tracked 10 atoms across 2 accounts == Mint supply 10');
    expect(html).toContain('cycle 7 to cycle 9');
  });

  it('marks a gloss as this site’s and the sentence as the census’s', () => {
    expect(html).toContain('collateral closure');
    expect(html).toContain('is this site&#x27;s gloss on what that law is for');
  });

  /**
   * The alignment refusal. A row with a different number of verdicts than
   * there are cycles cannot be laid under those cycle numbers without
   * misreporting at least one of them, so nothing is drawn at all.
   */
  it('draws nothing rather than a band whose columns are not its cycles', () => {
    const ragged = renderToStaticMarkup(
      <LawBand rows={[{ ...rows[0], statuses: ['holds', 'holds'] }]} cycles={cycles} caption="ragged" />,
    );
    expect(ragged).not.toContain('<svg');
  });

  it('says why there is nothing rather than drawing an empty frame', () => {
    const empty = renderToStaticMarkup(<LawBand rows={[]} cycles={[]} caption="none" emptyReason="No laws were recorded." />);
    expect(empty).not.toContain('<svg');
    expect(empty).toContain('No laws were recorded.');
  });

  /**
   * The violated state is the only one on this band that is an emergency. It
   * must reach the reader as a word and a glyph, not only as a warmer color.
   */
  it('shows a broken law in words wherever it appears', () => {
    const broken = renderToStaticMarkup(
      <LawBand
        rows={[{ id: 'L4', statuses: ['holds', 'violated'], held: 1, violated: 1, inapplicable: 0, detail: 'Hoard 4 < worst outcome 5' }]}
        cycles={[1, 2]}
        caption="broken"
      />,
    );
    expect(broken).toContain('cycle 2 — L4 did not hold');
    expect(broken).toContain('cycle 1 — 1 law held');
    expect(broken).toContain('var(--viz-law-violated)');
    expect(broken).toContain('Hoard 4 &lt; worst outcome 5');
  });
});
