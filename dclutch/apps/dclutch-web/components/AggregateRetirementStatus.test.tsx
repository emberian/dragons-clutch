import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import AggregateRetirementStatus from './AggregateRetirementStatus';

const html = renderToStaticMarkup(<AggregateRetirementStatus
  endpoint="https://api.devnet.solana.com"
  coreProgramId="4vJ9JU1bJJE96FWSJKvHsmmF7ujPKAy5SKpjXkLc6R1Q"
  claimsProgramId="8qbHbw2BbbTHBW1sbeqakYXV5ZZGczXJG2ajNeN3WFe"
  marketAddress="7CuJSi6uEyTFD7TUmyiUyszv51b5v1K4tXGXhvC5Y8DU"
  marketPhase="Retiring"
  marketGeneration="7"
  minimumContextSlot="100"
/>);

describe('reader-facing aggregate retirement status', () => {
  it('names the four ordered durable steps without offering a mutation', () => {
    for (const step of ['prepare', 'close-vault', 'close-replay', 'finish']) expect(html).toContain(step);
    expect(html).toContain('Retirement unavailable in this browser');
    expect(html).toContain('disabled=""');
  });

  it('states the exact authority and evidence boundary in second person', () => {
    expect(html).toContain('You can see whether this Market');
    expect(html).toContain('Rust-owned generated ABI');
    expect(html).toContain('You still need a checked release');
    expect(html).toContain('A local-validator execution is not devnet execution');
    expect(html).toContain('never reconstructs the original bundle, opens a wallet, signs, or submits');
  });

  it('does not claim an observed checkpoint before the injected read returns', () => {
    expect(html).toContain('Reading the derived Claims aggregate or Core retirement checkpoint');
    expect(html).not.toContain('Persisted phase');
    expect(html).not.toContain('Original bundle digest');
  });
});
