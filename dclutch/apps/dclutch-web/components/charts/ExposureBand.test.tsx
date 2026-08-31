import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import ExposureBand, { type ExposureBandRowV1 } from './ExposureBand';

const ROWS: ReadonlyArray<ExposureBandRowV1> = Object.freeze([
  Object.freeze({ label: 'All 2 together', floorAtoms: '15', ceilingAtoms: '140', emphasis: true, note: 'the exact sum of the legs' }),
  Object.freeze({ label: 'Mark…One', floorAtoms: '10', ceilingAtoms: '40', emphasis: false, note: 'this Market has not settled' }),
  Object.freeze({ label: 'Mark…Two', floorAtoms: '5', ceilingAtoms: '100', emphasis: false, note: 'this Market has settled' }),
]);

describe('the exposure band', () => {
  const html = renderToStaticMarkup(<ExposureBand rows={ROWS} scaleAtoms="140" caption="one row per Position" />);

  it('keeps the exact atoms in a table twin rather than only in the plot', () => {
    expect(html).toContain('Exact atoms behind every band');
    for (const atoms of ['15', '140', '10', '40', '5', '100']) expect(html).toContain(`<td>${atoms}</td>`);
    // The decided column is computed in bigint from the two bounds.
    expect(html).toContain('<td>125</td>');
    expect(html).toContain('<td>95</td>');
  });

  it('names both parts of every band in words, never by colour alone', () => {
    expect(html).toContain('arrives whatever happens');
    expect(html).toContain('decided by the outcome');
  });

  it('draws no conditional hairline unless it is given the words that condition it', () => {
    expect(html).not.toContain('stroke-dasharray');
    const marked = renderToStaticMarkup(<ExposureBand
      rows={ROWS}
      scaleAtoms="140"
      conditionalCeilingAtoms="115"
      conditionalLabel="ceiling while every locked Market resolves"
      caption="one row per Position"
    />);
    expect(marked).toContain('stroke-dasharray');
    expect(marked).toContain('ceiling while every locked Market resolves');
    expect(marked).toContain('115');

    const unlabelled = renderToStaticMarkup(<ExposureBand rows={ROWS} scaleAtoms="140" conditionalCeilingAtoms="115" caption="x" />);
    expect(unlabelled).not.toContain('stroke-dasharray');
  });

  it('says why there is no plot instead of drawing an empty one', () => {
    const empty = renderToStaticMarkup(<ExposureBand rows={[]} scaleAtoms="0" caption="x" emptyReason="nothing is held here" />);
    expect(empty).toContain('nothing is held here');
    expect(empty).not.toContain('<svg');
  });

  it('still shows a band whose ends coincide, because a fixed payout is a fact', () => {
    const flat = renderToStaticMarkup(<ExposureBand
      rows={[Object.freeze({ label: 'complete set', floorAtoms: '500000000', ceilingAtoms: '500000000', emphasis: true, note: 'no outcome moves it' })]}
      scaleAtoms="500000000"
      caption="x"
    />);
    expect(flat).toContain('<svg');
    expect(flat).toContain('<td>0</td>');
  });

  it('projects a u64 near its ceiling without losing its low bits', () => {
    const huge = renderToStaticMarkup(<ExposureBand
      rows={[Object.freeze({ label: 'wide', floorAtoms: '9223372036854775809', ceilingAtoms: '18446744073709551615', emphasis: true, note: 'x' })]}
      scaleAtoms="18446744073709551615"
      caption="x"
    />);
    expect(huge).toContain('<td>18446744073709551615</td>');
    expect(huge).toContain('<td>9223372036854775806</td>');
  });
});
