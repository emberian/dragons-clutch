import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import OperatorSurface from './OperatorSurface';

describe('operator surface presentation', () => {
  it('shows executable breadth and exact refusal boundaries without invented state', () => {
    const html = renderToStaticMarkup(<OperatorSurface />);
    expect(html).toContain('Operations.');
    expect(html).toContain('Every route still requires its own preflight');
    expect(html).toContain('does not prove that a route is executable');
    expect(html).toContain('Use checked live-devnet preset');
    expect(html).toContain('instead of typing six program addresses');
    expect(html).toContain('never supplies a Market');
    expect(html).toContain('loading it is not a chain observation');
    expect(html).toContain('Each route must still authenticate its own release');
    expect(html).toContain('Constructor readiness map');
    expect(html).toContain('Prepare the current founding campaign');
    expect(html).toContain('Inspect a Direct route and its arithmetic');
    expect(html).toContain('Mutation waits for the accepted caller');
    expect(html).toContain('Reacquire the multiprogram deployment');
    expect(html).toContain('Create registered order');
    expect(html).toContain('Initialize / collect / materialize / distribute');
    expect(html).toContain('Inventory-bounded immediate trade');
    expect(html).toContain('Redeem a terminal Claims Position');
    expect(html).toContain('Inspect and export an unsigned transaction');
    expect(html).toContain('No chain state has been read.');
    expect(html).not.toContain('Illustrative');
    expect(html).not.toContain('mock');
  });
});
