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

describe('the wizard says where the coordinate actually falls', () => {
  const html = renderToStaticMarkup(<CreateMarketWizard />);

  it('asks for the founding observation in the product step', () => {
    // The property the composer never had. Cuts strictly increasing, regions
    // = cuts + 1, a gcd-normalized portfolio: every shape invariant passed on
    // a market that resolves into its top cell every time, because none of
    // them says where the coordinate is.
    expect(html).toContain('Founding observation · ticks');
    expect(html).toContain('the same ticks as the band');
  });

  it('names the compiler refusal a band the coordinate cannot reach would meet', () => {
    // Rendered against the wizard's own shipped default, which is the demo
    // market's cuts and is the convicted case.
    expect(html).toContain('DegenerateOutcomePartition');
  });

  it('says its bound is provisional and names the gate that lifts it', () => {
    expect(html).toContain('require_interesting_partition_v1');
  });
});
