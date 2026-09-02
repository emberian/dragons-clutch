import { describe, expect, it } from 'vitest';

import * as coverage from '../scripts/a11y-coverage.mjs';

type Row = Readonly<{ site: string; state: 'open' | 'exempt'; reason: string | null }>;

const report = coverage.coverage as () => Readonly<{
  scrollingClasses: ReadonlyArray<string>;
  unnamedControls: ReadonlyArray<Row>;
  unreachableScrollers: ReadonlyArray<Row>;
  lowContrastText: ReadonlyArray<Row & Readonly<{ ratio?: number }>>;
  unresolvedContrast: ReadonlyArray<Readonly<{ site: string; selector: string }>>;
}>;
const survey = coverage.survey as () => Readonly<{
  scrollingClasses: ReadonlyArray<string>;
  unnamedControls: ReadonlyArray<Readonly<{ site: string; tag: string }>>;
  unreachableScrollers: ReadonlyArray<Readonly<{ site: string; classes: ReadonlyArray<string> }>>;
}>;

/**
 * The accessibility done-criterion, for a suite with no DOM.
 *
 * C-12 asks that "mobile and accessible interaction is complete," and before
 * this file the application could not answer at all. `vitest.config.ts` runs
 * `environment: 'node'`, so across 171 test files there are zero `getByRole`
 * queries, zero `getByLabelText` queries and zero axe runs; accessibility was
 * asserted the only way a DOM-less suite can assert it, by pinning attribute
 * strings somebody remembered to pin. That catches a deleted `aria-label` on
 * the nav. It cannot catch an input nobody named or a scroll box no keyboard
 * reaches, which is why both were live in shipped routes.
 *
 * So this is a source survey held to a ratchet, the third in the shape
 * `abiCoverage.test.ts` and `explorerCoverage.test.ts` already use. It does
 * not claim the app is accessible. It claims three exact defect classes are at
 * zero, that they cannot come back quietly, and that every excuse — including
 * the backgrounds it refuses to guess — is a sentence someone had to write.
 *
 * Run `node scripts/a11y-coverage.mjs` to read the inventory.
 */
describe('accessibility coverage', () => {
  it('names every form control it renders', () => {
    const open = report().unnamedControls.filter((row) => row.state === 'open');
    expect(
      open.map((row) => row.site),
      'wrap it in a <label>, give it aria-label, or pair its id with an htmlFor',
    ).toEqual([]);
  });

  it('lets a keyboard reach every region that scrolls', () => {
    // WCAG 2.1.1. A container with `overflow-x: auto` scrolls under a mouse
    // and is inert to a keyboard unless it, or something inside it, can take
    // focus. This is the defect the mobile work created: wide tables were
    // pushed into horizontal scrollers, and every column pushed off-screen
    // became unreachable to somebody navigating by keyboard.
    const open = report().unreachableScrollers.filter((row) => row.state === 'open');
    expect(
      open.map((row) => row.site),
      'give the container tabIndex={0} with role="region" and an aria-label, or put something focusable in it',
    ).toEqual([]);
  });

  it('keeps small text above 4.5:1 wherever its background is knowable', () => {
    // WCAG 1.4.3. Twenty-nine ad-hoc greys between #46534b and #6f7d74 sat
    // below the floor on the page ground, each invented separately, none of
    // them a token. They are one token now, and this is what stops a
    // thirtieth being invented.
    const open = report().lowContrastText.filter((row) => row.state === 'open');
    expect(
      open.map((row) => `${row.site} (${row.ratio}:1)`),
      'use var(--dim) or brighter, or exempt it with a reason',
    ).toEqual([]);
  });

  it('refuses to judge a background it would have to guess, and says how many', () => {
    // The honest edge of this check, pinned so it cannot quietly widen. A rule
    // whose background comes from an ancestor is NOT measured: an earlier draft
    // guessed the nearest painting ancestor, which is not what a cascade does,
    // and it invented a 3.22:1 finding for a rule nobody had touched. A
    // contrast number produced by a guess is worse than no number, because it
    // gets colours rewritten to satisfy it.
    //
    // These 223 rules are a named wall, not a pass. Closing them needs a
    // resolved cascade, and the test below records exactly which instrument
    // cannot supply one and why — measured, not assumed.
    expect(report().unresolvedContrast.length).toBe(223);
  });

  /**
   * THE THIRD DRAFT, refused before it was written.
   *
   * Two earlier drafts guessed the painting ancestor and produced wrong
   * findings. The obvious third move is the instrument this suite already uses
   * for landmark nesting: render the shells with jsdom as a library and ask
   * `getComputedStyle` what is actually behind the text. It does not work, and
   * the reason is worth a test rather than a sentence, because "we tried
   * rendering" is exactly the claim a later lane would re-spend an afternoon
   * on.
   *
   * jsdom resolves selector matching correctly and does NOT resolve custom
   * properties: `color: var(--x)` comes back as the literal `var(--x)`, and a
   * `background: var(--ground)` shorthand comes back fully transparent. This
   * application's stylesheet is var()-based by construction — the contrast
   * collapse made it more so — so a cascade computed that way would report
   * every background as transparent and every colour as an unparsable string.
   * That is a third wrong answer, not a resolution.
   *
   * The control is live in both directions: if jsdom ever resolves custom
   * properties, this goes red and tells whoever reads it that the road is now
   * open. The method that would work without one is not a browser either — it
   * is matching each rule against the RENDERED tree with `element.matches`,
   * which jsdom does implement, and doing the token expansion and compositing
   * with this survey's own `tokens()`.
   */
  it('records why a rendered cascade cannot close the 223, and reopens if that changes', async () => {
    const { JSDOM } = await import('jsdom');
    const dom = new JSDOM(
      '<!doctype html><html><head><style>'
      + ':root{--ground:#101010;--muted:#777777}'
      + 'body{background:var(--ground)}.card{background:#222222}.card p{color:var(--muted);font-size:12px}'
      + '</style></head><body><div class="card"><p id="probe">x</p></div></body></html>',
    );
    const { window } = dom;
    const probe = window.document.getElementById('probe');
    expect(probe).not.toBeNull();
    const computed = window.getComputedStyle(probe as Element);

    // The positive control: jsdom DOES apply a plain stylesheet rule, so the
    // instrument is connected and the two failures below are real.
    expect(computed.fontSize).toBe('12px');
    expect(window.getComputedStyle(window.document.querySelector('.card') as Element).backgroundColor)
      .toBe('rgb(34, 34, 34)');

    // And the two failures that close the road.
    expect(computed.color, 'jsdom now resolves custom properties; a rendered cascade may be worth building')
      .toBe('var(--muted)');
    expect(window.getComputedStyle(window.document.body).backgroundColor,
      'jsdom now resolves a var() background shorthand; a rendered cascade may be worth building')
      .toBe('rgba(0, 0, 0, 0)');
    window.close();
  });

  it('carries a written reason for every exemption', () => {
    for (const row of [...report().unnamedControls, ...report().unreachableScrollers, ...report().lowContrastText]) {
      if (row.state !== 'exempt') continue;
      expect(row.reason, `${row.site} is exempt with no reason`).toBeTruthy();
      expect((row.reason ?? '').length, `${row.site}'s exemption reason is too short to be one`).toBeGreaterThan(64);
    }
  });

  it('has a surface to survey at all', () => {
    // Every assertion above passes vacuously against an empty survey, which is
    // the one way this file could lie: a broken walker, a renamed directory,
    // or a stylesheet that stopped parsing all read as "nothing is wrong."
    const found = survey();
    expect(found.scrollingClasses.length, 'no class in either stylesheet scrolls, which cannot be true').toBeGreaterThan(0);
    expect(found.unnamedControls.length + found.unreachableScrollers.length,
      'the survey found no control and no scroller anywhere, so it is not reading the source').toBeGreaterThan(0);
  });

  it('reads code and not the prose around it', () => {
    // `MarketFilterBar`'s header comment argues at length about whether a
    // search box counts as an `<input>`, and the first run of this survey
    // reported that argument as two unlabelled controls. A survey that reads
    // comments as source produces findings that get it disbelieved.
    const sites = survey().unnamedControls.map((entry) => entry.site);
    for (const site of sites) expect(site.startsWith('components/MarketFilterBar.tsx')).toBe(false);
  });
});
