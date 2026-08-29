import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import RefusedMarketStory from '@/components/RefusedMarketStory';

describe('the refused market story', () => {
  const html = renderToStaticMarkup(<RefusedMarketStory
    refusal="This older devnet Market generation is incompatible with the current reader."
    observedSlot="489826258"
    address="57i7c6zwEEzySrt7a94FAbY6AWnEdK4jDZEYkTej4PrP"
  />);

  it('tells the verdict verbatim and where it was observed', () => {
    expect(html).toContain('This older devnet Market generation is incompatible');
    expect(html).toContain('489826258');
    expect(html).toContain('57i7c6zwEEzySrt7a94FAbY6AWnEdK4jDZEYkTej4PrP');
  });

  it('refuses to invent structure and offers real ways onward', () => {
    expect(html).toContain('does not pretend to a structure it could not read');
    expect(html).toContain('Browse the current markets');
    expect(html).toContain('See the raw account in the explorer');
    expect(html).toContain('/explorer?view=market');
  });
});
