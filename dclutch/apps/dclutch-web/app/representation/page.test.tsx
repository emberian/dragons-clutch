import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import RepresentationPage from './page';

describe('/representation operator route', () => {
  it('reaches the existing authenticated bearer-transfer workspace', () => {
    const html = renderToStaticMarkup(<RepresentationPage />);
    expect(html).toContain('Authenticate exact transfer route');
    expect(html).toContain('Sign with each required wallet, then submit once');
    expect(html).toContain('Submit fully signed transfer');
    expect(html).toContain('href="/console" class="active" aria-current="page"');
  });
});
