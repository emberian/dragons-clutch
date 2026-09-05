import { describe, expect, it } from 'vitest';

import * as coverage from '../../../tools/abi-coverage/abi-coverage.mjs';

/** What the survey reports for one category of source file. */
type Inventory = Readonly<{
  magics: ReadonlyArray<string>;
  domains: ReadonlyArray<string>;
  offsets: Readonly<Record<string, number>>;
}>;

const survey = coverage.surveyHandMirrors as () => Readonly<{ gating: Inventory; pins: Inventory }>;
const audit = coverage.auditAgainstBaseline as (gating: Inventory, baseline: Inventory) => ReadonlyArray<string>;
const baseline = coverage.readBaseline as () => Inventory;
/** One generated module and the `abi:*:verify` that byte-checks it, if any. */
type GeneratedAuthority = Readonly<{ module: string; verify: string | null; tree: string | null }>;

const authorities = coverage.surveyGeneratedAuthorities as () => ReadonlyArray<GeneratedAuthority>;
const auditAuthorities = coverage.auditGeneratedAuthorities as (
  rows: ReadonlyArray<GeneratedAuthority>,
) => ReadonlyArray<string>;

/**
 * The done-criterion for the hand-mirror genus.
 *
 * A hand-mirror is a record magic, a PDA seed domain, or a byte offset this
 * browser states in its own words rather than importing from `lib/generated/`.
 * Every one is a second authority for something a Lean schema already owns, and
 * the failure it produces is not a crash: it is a page that confidently shows
 * the wrong thing, or a transaction built against a layout that has moved.
 *
 * These tests hold the inventory to a ratchet. They do not claim the browser is
 * free of hand-mirrors — `scripts/abi-coverage.baseline.json` is a list of the
 * ones that remain, and it is long. They claim that the list cannot grow
 * without someone deciding to grow it, and that a surface converted to a
 * Lean-emitted module leaves the list for good.
 *
 * Run `npm run abi:coverage` to read the inventory, and
 * `npm run abi:coverage -- --write` to record a baseline that shrank.
 */
describe('ABI coverage', () => {
  it('states no magic, domain or offset the baseline does not already record', () => {
    const { gating } = survey();
    expect(audit(gating, baseline())).toEqual([]);
  });

  it('keeps every converted surface out of the browser source', () => {
    const { gating } = survey();
    const stated = [...gating.magics, ...gating.domains];
    // Surfaces with a Lean-emitted TypeScript module: nothing outside
    // lib/generated/ may name them again.
    for (const converted of [
      'DCLTCAP1', 'DCLTFQ01', 'DCLTCFS1', 'DCLTMOR1', 'DCLTRLM1', 'DCLTPOS1',
      'dclutch/position/v1', 'dclutch/realm/v1', 'dclutch/rent-market/v2',
      'dclutch/cap-funding/v1', 'dclutch/cap-fund-auth/v1', 'dclutch/cap-fund-vault/v1',
      'dclutch/open-readiness/v1', 'dclutch:lbv2:market', 'dclutch:lbv2:position',
    ]) {
      const offenders = stated.filter((entry) => entry.endsWith(`\t${converted}`));
      expect(offenders, `${converted} is emitted; import it instead of restating it`).toEqual([]);
    }
  });
});

/**
 * The other half of the coverage question.
 *
 * The survey above asks what this package still states in its OWN words. It
 * skips `lib/generated/` on purpose, which leaves the question it cannot ask:
 * whether a generated module has an authority behind it at all. AGENTS.md:
 * "every generated module carries an `abi:*:verify` that `npm test` runs, so a
 * surface with neither is a surface with no authority behind it."
 *
 * `packages/dclutch-sdk/lib/generated/routeCensus.ts` was exactly that surface
 * until 2026-09-02: no generator, no verify, held only by the web tree's
 * byte-identity test -- and the two copies drifted apart through a program edit
 * with just that one gate noticing (2fe2b9f84). Neither tree's coverage census
 * named the file. This is the arm that would have.
 */
describe('generated modules have an authority', () => {
  it('walks lib/generated/ and finds the modules it is meant to speak about', () => {
    // A walk that silently matched nothing would make the assertion below
    // vacuous, which is the one way this test could lie.
    const rows = authorities();
    expect(rows.length).toBeGreaterThanOrEqual(20);
    expect(rows.map((row) => row.module)).toContain('lib/generated/routeCensus.ts');
  });

  it('pairs every generated module with a verify script', () => {
    expect(auditAuthorities(authorities())).toEqual([]);
  });
});
