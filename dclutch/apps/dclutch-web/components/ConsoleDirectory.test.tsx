import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import ConsoleDirectory from './ConsoleDirectory';
import { browserActPrerequisitesV1, BROWSER_CAPABILITY_STANDINGS_V1 } from '@/lib/capabilitySurface';

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

  it('says what a reader must already hold, in the card that asks for it', () => {
    // The venue line says what an act DOES. It never said what an act cannot
    // be STARTED without, and that is how a redemption whose second step
    // opens a file picker for a Rust-authored payout plan came to be
    // advertised as one wallet signature sent from here.
    const needing = listed.filter((standing) =>
      browserActPrerequisitesV1(standing).some((entry) => entry.id === 'external-file'));
    expect(needing.length, 'no listed act reads a file, so this assertion proves nothing').toBeGreaterThan(0);
    for (const standing of needing) {
      const outcome = html.indexOf(standing.action.action);
      expect(outcome, `${standing.action.id} has no outcome line`).toBeGreaterThanOrEqual(0);
      const statement = browserActPrerequisitesV1(standing)
        .filter((entry) => entry.id === 'external-file')
        .map((entry) => entry.statement)[0]!;
      expect(
        html.indexOf(statement, outcome),
        `${standing.action.id} needs a file this browser cannot produce and its card does not say so`,
      ).toBeGreaterThan(outcome);
    }
  });

  it('states no prerequisite on an act that does not have it', () => {
    // The mirror-image failure, and the cheaper one to ship: a line every card
    // carries says nothing, and a reader stops reading it.
    const free = listed.filter((standing) => browserActPrerequisitesV1(standing).length === 0);
    expect(free.length, 'every listed act has a prerequisite, so this assertion proves nothing').toBeGreaterThan(0);
    const stated = (html.match(/Before you start/g) ?? []).length;
    expect(stated).toBe(listed.length - free.length);
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
