import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import ConsoleDirectory from './ConsoleDirectory';

describe('the console index', () => {
  const html = renderToStaticMarkup(<ConsoleDirectory />);

  it('groups each current operator workspace once by lifecycle job', () => {
    for (const heading of ['Author and open', 'Trade and resolve', 'Run the deployment', 'Verify the record']) {
      expect((html.match(new RegExp(`>${heading}<`, 'g')) ?? []).length).toBe(1);
    }
    for (const href of ['/product-v2#spline-product', '/found#current-founding', '/liquidity', '/general', '/resolution', '/release', '/operate', '/workbench', '/local']) {
      expect((html.match(new RegExp(`href="${href}"`, 'g')) ?? []).length).toBe(1);
    }
    for (const productJourney of ['/campaign', '/population', '/redeem', '/trade']) expect(html).not.toContain(`href="${productJourney}"`);
  });

  it('derives outcomes and authority contracts from executable capability truth', () => {
    expect(html).toContain('Activate checked multiprogram release');
    expect(html).toContain('Compile an admitted degree-2/3 Product graph');
    expect(html).toContain('Consider candidate / freeze selection');
    expect(html).toContain('Submit real provider evidence / reclaim');
    expect(html).toContain('Browser produces checked unsigned transaction bytes');
    expect(html).toContain('Rust operator tooling produces the checked unsigned transaction');
    expect(html).not.toContain('awaiting production');
    expect(html).not.toContain('not open yet');
    expect(html).not.toContain('unavailable');
  });

  it('names the provenance answer key and keeps product journeys on the product', () => {
    expect(html).toContain('The artifacts, and where they come from');
    expect(html).toContain('Artifact inputs name their producer');
    expect(html).toContain('Market-participant acts stay on the selected');
    expect(html).toContain('href="/markets"');
  });
});
