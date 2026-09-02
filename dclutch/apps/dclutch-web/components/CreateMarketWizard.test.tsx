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

  it('asks for the belief the gate is measured against, which it never used to collect', () => {
    // The three fields that make a partition measurable at all. Without a
    // belief there is nothing for `require_interesting_partition_v1` to
    // measure against, which is why the wizard could only run a unit-sanity
    // bound of its own before these existed.
    expect(html).toContain('What you believe the coordinate does');
    expect(html).toContain('Volatility · basis points of spot over the window');
    expect(html).toContain('Window · slots from founding to deadline');
    expect(html).toContain('Plausible half-widths');
    expect(html).toContain('Largest share one outcome may take · basis points');
  });

  it('claims no verdict before the compiled gate has loaded', () => {
    // A STATIC render has not loaded the WASM, so there is no measurement yet
    // and the page says exactly that. It must NOT name a refusal here: the
    // wizard used to print `DegenerateOutcomePartition` from a check of its
    // own, and a client that names the compiler's refusal without having asked
    // the compiler is the thing this whole unit removed.
    expect(html).toContain('Loading the compiled partition gate');
    expect(html).not.toContain('DegenerateOutcomePartition');
    expect(html).not.toContain('provisional unit-sanity bound');
  });
});
