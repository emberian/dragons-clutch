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
  outcomeCount={4}
/>);

describe('reader-facing aggregate retirement status', () => {
  /**
   * RULING D1 item 2: the market's terms state the crank-first order.
   *
   * A vocabulary gate as much as a render test. The reversed order -- opener
   * before cranker -- is what the DESIGN originally stated and every total
   * still adds up under it, so nothing but the words distinguishes them. The
   * page must not be able to lose them quietly.
   */
  it('states the crank-first order and who it leaves short', () => {
    expect(html).toContain('costs the opener the first crank');
    expect(html).toContain('<strong>the cranker before the opener</strong>');
    expect(html).toContain('never repays its opener in full');
    // And it states no figure it has not read: the server render has made no
    // RPC call, so the loading line stands where a quoted number would be.
    expect(html).toContain('Reading this cluster');
    expect(html).not.toMatch(/0\.00\d{7} SOL/);
  });

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
