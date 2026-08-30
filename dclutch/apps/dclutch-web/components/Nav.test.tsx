import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import Nav from './Nav';

/**
 * THE nav. Every page renders this component, so this test pins the one
 * canonical item set — the regression it guards is the one that actually
 * happened: ~20 hand-rolled nav bars, no two alike.
 */
describe('the site nav', () => {
  it('renders the canonical product set plus one Console entry, nothing else', () => {
    const html = renderToStaticMarkup(<Nav current="/markets" />);
    for (const label of ['Live', 'Markets', 'Pulse', 'Activity', 'Design', 'Portfolio', 'Explorer', 'Docs', 'Console']) {
      expect(html).toContain(`>${label}</a>`);
    }
    // The retired per-page item sets must not creep back in.
    for (const retired of ['>Trade</a>', '>Liquidity</a>', '>Represent</a>', '>Release</a>', '>Direct</a>', '>General</a>', '>Workbench</a>', '>Operate</a>']) {
      expect(html).not.toContain(retired);
    }
    expect((html.match(/<nav aria-label="Primary navigation">/g) ?? []).length).toBe(1);
    expect(html).toContain('<span id="main-content" class="main-content-anchor" tabindex="-1"></span>');
  });

  it('marks the current product route active, and only it', () => {
    const html = renderToStaticMarkup(<Nav current="/portfolio" />);
    expect(html).toContain('href="/portfolio" class="active" aria-current="page"');
    expect((html.match(/class="active"/g) ?? []).length).toBe(1);
    expect((html.match(/aria-current="page"/g) ?? []).length).toBe(1);
  });

  it('lights the Console entry from any console route', () => {
    for (const path of ['/console', '/release', '/trade', '/workbench', '/local']) {
      const html = renderToStaticMarkup(<Nav current={path} />);
      expect(html).toContain('href="/console" class="active" aria-current="page"');
    }
  });

  it('states one honest status, defaulting to the devnet preview', () => {
    expect(renderToStaticMarkup(<Nav />)).toContain('devnet preview');
    expect(renderToStaticMarkup(<Nav status="operator tool" />)).toContain('operator tool');
  });
});
