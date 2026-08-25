import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import RegisteredOrdersWorkspace from './RegisteredOrdersWorkspace';

describe('registered order workspace presentation', () => {
  it('renders the real-state discovery and signing boundaries before any action', () => {
    const html = renderToStaticMarkup(<RegisteredOrdersWorkspace
      endpoint="http://127.0.0.1:8899"
      protocolProgram="11111111111111111111111111111111"
      controllerProgram="11111111111111111111111111111111"
    />);
    expect(html).toContain('Registered orders on chain');
    expect(html).toContain('Discover registered Direct states');
    expect(html).toContain('No claim-owner scan has run.');
    expect(html).toContain('Fill, cancellation, and expiry start only from reacquired persisted authority.');
    expect(html).not.toContain('illustrative');
    expect(html).not.toContain('mock');
  });
});
