import { describe, expect, it } from 'vitest';

import * as coverage from '../scripts/a11y-coverage.mjs';

type Row = Readonly<{ site: string; state: 'open' | 'exempt'; reason: string | null }>;

const report = coverage.coverage as () => Readonly<{
  scrollingClasses: ReadonlyArray<string>;
  unnamedControls: ReadonlyArray<Row>;
  unreachableScrollers: ReadonlyArray<Row>;
  lowContrastText: ReadonlyArray<Row & Readonly<{ ratio?: number }>>;
  unresolvedContrast: ReadonlyArray<Readonly<{ site: string; selector: string }>>;
  dimmedText: ReadonlyArray<Row & Readonly<{ selector?: string; alpha?: number }>>;
}>;
/** `[r, g, b]`, or `[r, g, b, a]` for a colour that is itself translucent. */
type Rgb = readonly [number, number, number] | readonly [number, number, number, number];
const effectiveContrastV1 = coverage.effectiveContrastV1 as (
  foreground: Rgb,
  background: Rgb,
  ground: Rgb,
  alpha: number,
) => number;
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
    // These rules are a named wall, not a pass. Closing them needs a resolved
    // cascade, and the test below records exactly which instrument cannot
    // supply one and why — measured, not assumed.
    //
    // 223 -> 196 on 2026-09-02, and the 27 that left were not resolved: 26 were
    // DEAD CSS, rules for classes no component renders, and one was a real
    // defect the refusal was hiding. The wall was carrying a legacy stylesheet
    // as if it were unmeasured surface. Every family was verified absent from
    // every `className` in app/ and components/ before deletion.
    //
    // TWO REAL FAILURES WERE FOUND INSIDE THE 196 by resolving them by hand,
    // which is what this number is for. `.trade-v3-preview p` sat on the
    // hairline-grid DIVIDER rather than a cell, 3.75:1 at 13px on /trade and
    // /market; it now paints the same tint over the same cell, opaque, and
    // resolves at 5.72:1. `.flow-rail a > small` was 4.24:1 before any state,
    // and `.flow-rail-upcoming a { opacity: .58 }` took the seven-step trade
    // rail to 2.16:1 for the step label and 3.18:1 for the number. THE SURVEY
    // MODELLED OPACITY NOWHERE, so that whole class of defect was invisible to
    // it even for rules it resolves. It does now; see the two cases below.
    //
    // 196 -> 194 with the creation-studio remnant, the last dead family left in
    // the sheet. 223 -> 194 in total, and THE DELETIONS ARE FINISHED: the 194
    // resolve to 94 distinct leading class stems and every one of them is
    // rendered by a component. Nothing further comes off this number by
    // deleting; what is left is the instrument's own edge.
    //
    // THE FOURTH DRAFT, REFUSED WITH A MEASUREMENT, like the third below.
    // The cheap way to shrink this is to composite the selector's PREFIX
    // ancestors -- provably ancestors, the same soundness that makes
    // `dimmerFor` safe -- over the ground and call that the background. Tried
    // on 2026-09-02: it takes 194 to 13 and produces 28 findings, and the
    // findings are WRONG. `.trade-v3-preview span` came back at 4.42:1,
    // measured against the hairline grid's 1px divider, because the element it
    // actually sits in is `.trade-v3-preview > div` -- an opaque cell that is
    // not in the span's selector and cannot be. Its true ratio is 6.13:1 and
    // it was never a failure. A prefix ancestor is a real ancestor, but it is
    // not the NEAREST painter, and only the nearest painter is the background.
    //
    // So the refusal stands, and for a sharper reason than before: it is not
    // that this file will not guess, it is that every cheaper model has now
    // been run and each invents failures on rules that are fine. The method
    // that works is still the one named below -- match each rule against the
    // RENDERED tree with `element.matches` and composite with this file's own
    // `tokens()` -- and it is a project, not a patch.
    //
    // 194 -> 196 on 2026-09-02 with `.market-answer-meaning` and its eyebrow,
    // the block that tells a reader of a resolved market what the answer means.
    // The block carries its own colour and its children inherit it, so it adds
    // TWO rules and not the four it started as; the two it adds are the same
    // pair `.phase-meaning` and `.phase-meaning strong` already contribute, for
    // the same reason -- a translucent tint over the page ground is a
    // background this instrument cannot resolve. That is the wall's own edge,
    // not a new dark corner, and the rules were checked by hand against the
    // rendered page at 1280 and 390 before the number moved.
    expect(report().unresolvedContrast.length).toBe(196);
  });

  it('composes element opacity into the ratio, on the two colours that were on the page', () => {
    /**
     * THE POSITIVE CONTROL, and this survey needs one badly.
     *
     * Both live `opacity` defects were fixed by receding in COLOUR instead, and
     * the one dimmer left in the sheet reaches no colour rule by selector
     * prefix -- so `alpha < 1` never fires in a real run today. A composition
     * path that is never taken is indistinguishable from one that is broken,
     * and "0 opacity rules dim text this survey cannot follow" would read as a
     * clean bill of health from an instrument that was disconnected.
     *
     * So this calls the composition directly with the colours that were
     * actually on `/market` and holds it to the ratios that were actually
     * measured. CSS renders the element and THEN composites it, so a dimmed
     * anchor's own wash dims too -- which is why the rail's step label is
     * 2.20:1 and not the 2.24:1 a hand calculation that dims only the ink
     * produces. Getting that wrong in the safe direction is still wrong.
     */
    const ground: Rgb = [0x07, 0x10, 0x0c];
    // `.flow-rail a`'s own `rgba(255,255,255,.02)`, composited over the ground
    // the way `surveyContrast` does before it measures anything.
    const railBackground: Rgb = [12, 21, 17];

    // `.flow-rail a > small` at #6e7c73 under `.flow-rail-upcoming a { opacity: .58 }`.
    expect(effectiveContrastV1([0x6e, 0x7c, 0x73, 1], railBackground, ground, 0.58)).toBeCloseTo(2.20, 2);
    // The same ink with no dimmer is above the floor, which is exactly why the
    // undimmed measurement said nothing was wrong.
    expect(effectiveContrastV1([0x6e, 0x7c, 0x73, 1], railBackground, ground, 1)).toBeCloseTo(4.24, 2);
    // `.flow-step > header p` at var(--muted) under `.flow-step-upcoming > header { opacity: .62 }`:
    // a SENTENCE at 15px, measured 7.72:1 and rendered 3.63:1.
    expect(effectiveContrastV1([0x9b, 0xa7, 0x9d, 1], ground, ground, 0.62)).toBeCloseTo(3.63, 2);
    expect(effectiveContrastV1([0x9b, 0xa7, 0x9d, 1], ground, ground, 1)).toBeCloseTo(7.72, 2);

    // And the colours that replaced them, all above the floor undimmed.
    expect(effectiveContrastV1([0x8a, 0x98, 0x8e, 1], railBackground, ground, 1)).toBeGreaterThanOrEqual(4.5);
    expect(effectiveContrastV1([0x7d, 0x8a, 0x81, 1], railBackground, ground, 1)).toBeGreaterThanOrEqual(4.5);
    expect(effectiveContrastV1([0x82, 0x8f, 0x86, 1], ground, ground, 1)).toBeGreaterThanOrEqual(4.5);
  });

  it('leaves no opacity rule dimming text without a composition or a written reason', () => {
    // The ratchet the two findings above earned. A dimmer this survey cannot
    // follow is the shape that hid a 2.20:1 rail for as long as the rail
    // existed, so it has to be cleared the way an unnamed control is: composed,
    // or excused in writing. `button:disabled` is the one exemption, and WCAG
    // 1.4.3 exempts inactive components by name.
    const open = report().dimmedText.filter((row) => row.state === 'open');
    expect(
      open.map((row) => `${row.site} (opacity ${row.alpha})`),
      'recede in colour instead, or exempt it with a reason',
    ).toEqual([]);
    expect(report().dimmedText.length, 'the dimmer census found nothing at all, so it is not reading the sheet').toBeGreaterThan(0);
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
