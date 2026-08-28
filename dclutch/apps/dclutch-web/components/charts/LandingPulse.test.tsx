import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import LandingPulse from './LandingPulse';

describe('LandingPulse', () => {
  it('renders every count as unread while nothing has been read, never as zero', () => {
    const html = renderToStaticMarkup(<LandingPulse />);
    expect(html).toContain('Markets founded');
    expect(html).toContain('Collateral locked');
    expect(html).toContain('Resolutions run');
    expect(html.split('>—</strong>').length - 1).toBe(3);
    expect(html).toContain('Reading finalized state from the active deployment…');
    expect(html).not.toContain('>0</strong>');
  });
});
