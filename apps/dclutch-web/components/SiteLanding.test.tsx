import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import SiteLanding from './SiteLanding';

// The front door against the real public cut. Its companion,
// SiteLanding.opened.test.tsx, renders the same page with a market named.

describe('the front door', () => {
  const html = renderToStaticMarkup(<SiteLanding />);

  it('says plainly where this stands before it says anything else', () => {
    expect(html).toContain('On devnet — nothing for sale');
    expect(html).toContain('the first markets are being set up');
    expect(html).toContain('no value at risk anywhere');
  });

  it('does not promise the reader a view of activity that is not there', () => {
    // It used to say "you can watch it all happen live below", above a strip
    // of three numbers.
    expect(html).not.toContain('watch it all happen live');
    expect(html).toContain('read live from the chain every time you open this page');
  });

  it('describes what needs an open market without pretending there is one', () => {
    expect(html).toContain('The seven programs are deployed');
    expect(html).toContain('will tell you plainly that there is not one yet');
  });
});
