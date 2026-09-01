import { describe, expect, it } from 'vitest';

import * as coverage from '../scripts/a11y-coverage.mjs';

type Row = Readonly<{ site: string; state: 'open' | 'exempt'; reason: string | null }>;

const report = coverage.coverage as () => Readonly<{
  scrollingClasses: ReadonlyArray<string>;
  unnamedControls: ReadonlyArray<Row>;
  unreachableScrollers: ReadonlyArray<Row>;
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
 * not claim the app is accessible. It claims two exact defect classes are at
 * zero and cannot come back quietly, and that every excuse is a sentence
 * someone had to write.
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

  it('carries a written reason for every exemption', () => {
    for (const row of [...report().unnamedControls, ...report().unreachableScrollers]) {
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
