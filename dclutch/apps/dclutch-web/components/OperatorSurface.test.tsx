import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import OperatorSurface from './OperatorSurface';

describe('operator surface presentation', () => {
  it('shows executable breadth and exact refusal boundaries without invented state', () => {
    const html = renderToStaticMarkup(<OperatorSurface />);
    expect(html).toContain('Operate what exists.');
    expect(html).toContain('Reacquire the multiprogram deployment');
    expect(html).toContain('Create registered order');
    expect(html).toContain('Initialize / collect / materialize / distribute');
    expect(html).toContain('Inventory-bounded immediate trade');
    expect(html).toContain('Redeem terminal Rational / Bearer representation');
    expect(html).toContain('Inspect and export an unsigned transaction');
    expect(html).toContain('No chain state has been read.');
    expect(html).not.toContain('Illustrative');
    expect(html).not.toContain('mock');
  });
});
