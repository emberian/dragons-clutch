import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import ConsoleDirectory from './ConsoleDirectory';

describe('the console index', () => {
  const html = renderToStaticMarkup(<ConsoleDirectory />);

  it('lists every operator console exactly once, each as a link', () => {
    for (const href of ['/workbench', '/found', '/product-v2', '/trade', '/liquidity', '/redeem', '/resolution', '/general', '/release', '/operate', '/local']) {
      expect((html.match(new RegExp(`href="${href}"`, 'g')) ?? []).length).toBe(1);
    }
  });

  it('states readiness boundaries and names the provenance answer key', () => {
    expect(html).toContain('does not update programs');
    expect(html).toContain('does not mean it can send a transaction');
    expect(html).toContain('For market authors');
    expect(html).toContain('Wallet redemption');
    // The blurb used to offer redemption as something a reader could do today.
    expect(html).toContain('Wallet redemption (not open yet)');
    expect(html).toContain('Paying out winning claims is not available yet');
    // Names the provenance answer key and its standard.
    expect(html).toContain('The artifacts, and where they come from');
    expect(html).toContain('a bug in the console');
  });

  it('sends readers who are not operators back to the product', () => {
    expect(html).toContain('start at');
    expect(html).toContain('href="/markets"');
  });
});
