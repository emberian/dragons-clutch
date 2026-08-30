import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import CreateMarketWizard from './CreateMarketWizard';

describe('the public market design wizard', () => {
  const html = renderToStaticMarkup(<CreateMarketWizard />);

  it('states that the chain step is read-only and cannot strand a partial founding', () => {
    expect(html).toContain('Inspect the chain');
    expect(html).toContain('Read-only opening preview');
    expect(html).toContain('without signing or spending');
    expect(html).not.toContain('DCLTGMF3');
    expect(html).not.toContain('κ');
    expect(html).not.toContain('ManipulationFloorV1');
    expect(html).not.toContain('Sign &amp; submit');
  });

  it('does not render a mutation boundary before the complete campaign is available', () => {
    expect(html).not.toContain('Sign and submit the rungs a browser can drive');
    expect(html).not.toContain('Sign &amp; submit this transaction');
    expect(html).not.toContain('Submission boundary');
  });
});
