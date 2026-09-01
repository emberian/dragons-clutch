import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import DirectPage from './page';

describe('/direct canonical shell', () => {
  const html = renderToStaticMarkup(<DirectPage />);

  it('points once to the one Direct console without rendering a second form', () => {
    expect((html.match(/href="\/trade"/g) ?? []).length).toBeGreaterThanOrEqual(1);
    expect(html).toContain('Direct trading lives at /trade');
    expect(html).toContain('nothing is signed or sent');
    expect(html).not.toContain('Acquire the action-selected route');
    expect(html).not.toContain('<form');
  });
});
