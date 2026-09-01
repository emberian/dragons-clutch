import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import ConsoleDirectory from './ConsoleDirectory';
import { BROWSER_CAPABILITY_STANDINGS_V1 } from '@/lib/capabilitySurface';

/**
 * The directory, held to the model it is generated from.
 *
 * The old version of this test asserted the words the old board typed --
 * `'Compile an admitted degree-2/3 Product graph'`, `'Rust operator tooling
 * produces the checked unsigned transaction'` -- which meant it passed for as
 * long as the board and the page agreed with each other, and said nothing at
 * all about whether either agreed with the code. That is the whole defect this
 * page was rebuilt to close, so these assertions are about the JOIN instead:
 * every act the model says has a venue is on the page, every act it says has
 * none is not, and no card describes a workspace in words of its own.
 */
describe('the console index', () => {
  const html = renderToStaticMarkup(<ConsoleDirectory />);
  const listed = BROWSER_CAPABILITY_STANDINGS_V1.filter((standing) => standing.venue !== 'no-venue');
  const walled = BROWSER_CAPABILITY_STANDINGS_V1.filter((standing) => standing.venue === 'no-venue');

  it('has both kinds of act to speak about at all', () => {
    // A model that lost its acts would make every assertion below vacuous.
    expect(listed.length).toBeGreaterThanOrEqual(10);
    expect(walled.length).toBeGreaterThan(0);
  });

  it('lists exactly the acts the model says have a venue', () => {
    for (const standing of listed) {
      expect(html, `${standing.action.id} has a venue and is not on the directory`).toContain(standing.action.action);
    }
    for (const standing of walled) {
      expect(html, `${standing.action.id} has no venue and is advertised anyway`).not.toContain(standing.action.action);
    }
  });

  it('says the outcome, then the venue, then one guarantee — in that order', () => {
    for (const standing of listed) {
      const outcome = html.indexOf(standing.action.action);
      expect(outcome, `${standing.action.id} has no outcome line`).toBeGreaterThanOrEqual(0);
      // Searched forward from the outcome: three General acts legitimately
      // keep the same guarantee, so its first appearance on the page is not
      // necessarily the one belonging to this card.
      const guarantee = html.indexOf(standing.action.guarantee, outcome);
      expect(guarantee, `${standing.action.id} has no guarantee line after its outcome`).toBeGreaterThan(outcome);
    }
    expect(html).toContain('This browser · one wallet signature, sent from here');
    expect(html).toContain('This browser · one wallet signature, exported as a file');
    expect(html).toContain('This browser · one detached message signature');
    expect(html).toContain('This browser · no key, no signature');
    expect(html).toContain('Published command · your own key, after an explicit authorization');
  });

  it('carries a known wall in the same card as the outcome it qualifies', () => {
    // An act can be executable and still walk into a wall on chain. A reader
    // deciding whether to start needs both facts at once, not one of them on
    // another page.
    const walls = listed.flatMap((standing) => standing.walls);
    expect(walls.length).toBeGreaterThan(0);
    for (const held of walls) expect(html).toContain(held.statement);
  });

  it('keeps product journeys on the product and names the provenance answer key', () => {
    expect(html).toContain('Market-participant acts stay on the selected');
    expect(html).toContain('href="/markets"');
    expect(html).toContain('The artifacts, and where they come from');
    for (const productJourney of ['/campaign', '/population', '/trade']) {
      expect(html).not.toContain(`href="${productJourney}"`);
    }
  });

  it('never grows an awaiting-production category, in any of its spellings', () => {
    for (const word of ['awaiting production', 'coming soon', 'unavailable', 'not yet available', 'greyed-out', 'rust unsigned']) {
      expect(html.toLowerCase(), `the directory says "${word}"`).not.toContain(word);
    }
    // Nor a disabled control: a button that says no and cannot say why is the
    // failure this whole surface was rebuilt away from.
    expect(html).not.toContain('disabled');
  });
});
