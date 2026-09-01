import { execFileSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { describe, expect, it } from 'vitest';

import { CAPABILITY_ACTIONS_V1, capabilityStandingV1, type CapabilityActionV1 } from './capabilityModel';
import {
  browserActPrerequisitesV1,
  BROWSER_CAPABILITY_SURFACE_V1,
  BROWSER_CAPABILITY_STANDINGS_V1,
} from './capabilitySurface';

/**
 * The drift guard for capability claims.
 *
 * THE DEFECT THIS CLOSES was not any particular wrong status. It was that a
 * status was a STRING SOMEBODY TYPED. Each row of `capabilityModel.ts` carried
 * `implementation: 'browser-wallet' | 'rust-unsigned' | 'awaiting-production'`,
 * every surface rendered its whole story from it, and the test that guarded it
 * asserted the same string back. Four capabilities changed state in one night
 * and the board changed for none of them; it was wrong in both directions at
 * once, claiming `/liquidity` could not construct a Dealer transaction while
 * that page signed one, and claiming `/release` produced unsigned bytes while
 * that page asked a wallet to sign.
 *
 * So this file does not assert statuses. It asserts the three things that make
 * a status underivable by hand:
 *
 *   1. THERE IS NO STATUS FIELD. An act carries anchors, a guarantee and walls,
 *      and nothing else. The shape check below is the load-bearing one: if a
 *      lane re-adds `implementation`, this fails before anything renders.
 *   2. EVERY ANCHOR IS REAL AND SURVEYED. A named owner, builder or runbook
 *      that is not a file, or that the generated surface has never heard of,
 *      is a claim standing on nothing — and a missing surface row is worse
 *      than a wrong one, because the derivation would silently demote instead
 *      of failing.
 *   3. THE PROSE MAY NOT OUTRUN THE DERIVATION. A guarantee that promises one
 *      submission and a finalized poststate, on an act the code shows cannot
 *      send, is the same species of claim as `verified` on a signature chip
 *      the browser never checked. It is pinned here so it cannot drift back.
 *
 * The evidence itself is gated elsewhere and deliberately: the surface is
 * regenerated and byte-compared by `abi:capability-surface:verify`, which
 * `lib/abiVerification.test.ts` runs inside `npm test`. Delete a wallet path
 * from a workspace and that gate goes red; hand-edit the generated surface to
 * claim one and it goes red the same way. Between the two, a status can only
 * be changed by changing what the browser does.
 */

const repoRoot = fileURLToPath(new URL('../../..', import.meta.url));
const webRoot = fileURLToPath(new URL('..', import.meta.url));

/** The exact shape of one act. Anything else is a status someone can type. */
const ACT_FIELDS = ['id', 'stage', 'family', 'action', 'workspace', 'requiresMarket', 'anchors', 'guarantee', 'walls'];

/**
 * Vocabulary a capability claim may never use.
 *
 * The first four are the "awaiting production" product category by another
 * name — a promise dressed as a status, which is exactly what the directory
 * was told not to grow. The rest are words that borrow an authority the
 * browser does not have.
 */
const FORBIDDEN = [
  'coming soon',
  'awaiting production',
  'not yet available',
  'will be available',
  'roadmap',
  'for now',
  'temporarily',
  'greyed',
  'grayed',
  'unavailable',
];

/** Claims only an act that sends its own packet may make. */
const SUBMISSION_CLAIMS = ['sent once', 'submits once', 'submission', 'resubmit', 'sends a second', 'finalized poststate'];

/** Claims only an act that asks a wallet for something may make. */
const WALLET_CLAIMS = ['your wallet', 'wallet signature', 'wallet sees it'];

function anchorPaths(candidate: CapabilityActionV1): ReadonlyArray<string> {
  return [candidate.anchors.owner, candidate.anchors.builder, candidate.anchors.runbook]
    .filter((entry): entry is string => entry !== null);
}

/** Whether a citation names something this repository actually holds. */
function citationResolves(citation: string): boolean {
  if (existsSync(join(repoRoot, citation))) return true;
  try {
    // A wall may cite the commit that convicted it rather than a file.
    execFileSync('git', ['cat-file', '-e', `${citation}^{commit}`], { cwd: repoRoot, stdio: 'pipe' });
    return true;
  } catch {
    return false;
  }
}

describe('the capability model carries no status anyone can type', () => {
  it('gives an act anchors and a guarantee, and no implementation field', () => {
    for (const candidate of CAPABILITY_ACTIONS_V1) {
      expect(
        Object.keys(candidate).sort(),
        `${candidate.id} has fields this model does not define. A status field is the defect this whole mechanism replaced: derive it from anchors instead.`,
      ).toEqual([...ACT_FIELDS].sort());
      expect(Object.keys(candidate.anchors).sort()).toEqual(['builder', 'owner', 'runbook']);
    }
  });

  it('has acts to speak about at all', () => {
    // A catalogue that silently emptied would make every assertion below
    // vacuous, which is the one way this file could lie.
    expect(CAPABILITY_ACTIONS_V1.length).toBeGreaterThanOrEqual(20);
    expect(BROWSER_CAPABILITY_STANDINGS_V1.length).toBe(CAPABILITY_ACTIONS_V1.length);
    expect(BROWSER_CAPABILITY_SURFACE_V1.modules.length).toBeGreaterThanOrEqual(50);
    expect(BROWSER_CAPABILITY_STANDINGS_V1.some((standing) => standing.venue === 'browser')).toBe(true);
    expect(BROWSER_CAPABILITY_STANDINGS_V1.some((standing) => standing.venue === 'no-venue')).toBe(true);
  });

  it('names only anchors that exist and that the surface has surveyed', () => {
    const surveyed = new Set(BROWSER_CAPABILITY_SURFACE_V1.modules.map((entry) => entry.module));
    const publishing = new Set(BROWSER_CAPABILITY_SURFACE_V1.runbooks.map((entry) => entry.module));
    for (const candidate of CAPABILITY_ACTIONS_V1) {
      for (const path of anchorPaths(candidate)) {
        expect(existsSync(join(webRoot, path)), `${candidate.id} anchors ${path}, which is not a file`).toBe(true);
        expect(
          surveyed.has(path),
          `${candidate.id} anchors ${path}, which no route reaches. An anchor the surface has never seen makes this act silently lose its venue rather than fail; route the module or drop the anchor.`,
        ).toBe(true);
      }
      if (candidate.anchors.runbook !== null) {
        expect(
          publishing.has(candidate.anchors.runbook),
          `${candidate.id} says ${candidate.anchors.runbook} publishes an exact command, and that file publishes none.`,
        ).toBe(true);
      }
    }
  });

  it('lets no capability id appear twice', () => {
    const seen = new Set<string>();
    for (const candidate of CAPABILITY_ACTIONS_V1) {
      expect(seen.has(candidate.id), `${candidate.id} is catalogued twice`).toBe(false);
      seen.add(candidate.id);
    }
  });
});

describe('walls are named and cited, never softened', () => {
  it('gives every act without a venue at least one wall', () => {
    for (const standing of BROWSER_CAPABILITY_STANDINGS_V1) {
      if (standing.venue !== 'no-venue') continue;
      expect(
        standing.walls.length,
        `${standing.action.id} has no venue and says nothing about why. A capability with nothing behind it names its wall; it does not get a cheerful label.`,
      ).toBeGreaterThan(0);
    }
  });

  it('cites something this repository actually holds', () => {
    for (const candidate of CAPABILITY_ACTIONS_V1) {
      for (const held of candidate.walls) {
        expect(
          citationResolves(held.citation),
          `${candidate.id} cites ${held.citation}, which is neither a path in this repository nor a commit in it.`,
        ).toBe(true);
        expect(held.statement.length, `${candidate.id}'s wall is too short to be a statement`).toBeGreaterThan(40);
      }
    }
  });

  it('refuses the vocabulary of a roadmap anywhere in the catalogue', () => {
    for (const candidate of CAPABILITY_ACTIONS_V1) {
      const prose = [candidate.action, candidate.guarantee, ...candidate.walls.map((held) => held.statement)]
        .join(' ')
        .toLowerCase();
      for (const word of FORBIDDEN) {
        expect(prose.includes(word), `${candidate.id} says "${word}". A wall states what stops it; it does not promise.`).toBe(false);
      }
    }
  });
});

describe('a guarantee may not outrun the derivation', () => {
  it('promises one submission and a finalized poststate only where the browser sends', () => {
    for (const standing of BROWSER_CAPABILITY_STANDINGS_V1) {
      if (standing.submits) continue;
      const guarantee = standing.action.guarantee.toLowerCase();
      for (const claim of SUBMISSION_CLAIMS) {
        expect(
          guarantee.includes(claim),
          `${standing.action.id} promises "${claim}" and the code shows it cannot send: its owner never reaches submitSignedTransactionV1. This is the same species of claim as calling a signature "verified" that the browser only checked the shape of.`,
        ).toBe(false);
      }
    }
  });

  it('mentions a wallet only where one is asked for something', () => {
    for (const standing of BROWSER_CAPABILITY_STANDINGS_V1) {
      if (standing.authority !== 'none') continue;
      const guarantee = standing.action.guarantee.toLowerCase();
      for (const claim of WALLET_CLAIMS) {
        expect(
          guarantee.includes(claim),
          `${standing.action.id} speaks of a wallet and its owner requests no signature of any kind.`,
        ).toBe(false);
      }
    }
  });

  it('claims a venue only where a route or a published command reaches it', () => {
    for (const standing of BROWSER_CAPABILITY_STANDINGS_V1) {
      if (standing.venue === 'no-venue') {
        expect(standing.routes, `${standing.action.id} has no venue and yet names routes`).toEqual([]);
        expect(standing.authority).toBe('none');
        expect(standing.submits).toBe(false);
        continue;
      }
      expect(standing.routes.length, `${standing.action.id} claims a venue no route reaches`).toBeGreaterThan(0);
    }
  });

  it('stands every generated authority it decodes against on an abi:*:verify script', () => {
    for (const standing of BROWSER_CAPABILITY_STANDINGS_V1) {
      expect(
        standing.unverifiedAbis,
        `${standing.action.id} decodes against a generated module no verify script checks, which is a surface with no authority behind it.`,
      ).toEqual([]);
    }
  });
});

describe('what an act asks for before it can be started', () => {
  /**
   * THE DEFECT THIS CLOSES, and it is the first shape all over again.
   *
   * The derivation above answers what a workspace can DO — which wallet
   * request it reaches, whether it can send — and that made `claims.redeem`
   * a browser act with transaction authority that submits, which is all true.
   * What none of it measured is what the act cannot START without.
   * `RedeemFlow` opens with a file picker for a payout plan it says outright
   * it will never author: "This browser never creates or completes a payout
   * plan." The producer is `dclutch-local-successor-bootstrap
   * wallet-terminal-payout-input`, a Rust binary under
   * `tools/local-validator/`, and a reader holding a wallet and nothing else
   * cannot get past step two of redemption.
   *
   * So the directory said "This browser · one wallet signature, sent from
   * here" over an act a stranger cannot begin — a declaration standing on a
   * derivation that had never been run against the prerequisite. The fix is
   * the same species as the rest of this file: measure it, do not type it.
   * A workspace that renders a file input reads bytes this browser cannot
   * produce, and that is a syntactic fact about the module graph.
   *
   * Granularity is the workspace, deliberately and in both directions: an
   * anchor names a component, so a page carrying two acts states the
   * prerequisite for both. That is the same granularity the wallet authority
   * already has, and it is the honest one — the reader is being told what the
   * page in front of them needs.
   */
  const prerequisiteIds = (id: string): ReadonlyArray<string> => {
    const standing = BROWSER_CAPABILITY_STANDINGS_V1.find((candidate) => candidate.action.id === id);
    if (standing === undefined) throw new Error(`${id} is not catalogued`);
    return browserActPrerequisitesV1(standing).map((entry) => entry.id);
  };

  it('says a file is needed exactly where a workspace reads one', () => {
    // `/redeem` imports a Rust-authored payout plan; `/release` and `/operate`
    // import checked artifacts and an unsigned packet. All three are true.
    expect(prerequisiteIds('claims.redeem')).toContain('external-file');
    expect(prerequisiteIds('release.activate')).toContain('external-file');
    // `direct.author` composes an offer out of the market and the wallet, and
    // reads no file at all. Marking it would be the mirror-image lie.
    expect(prerequisiteIds('direct.author')).not.toContain('external-file');
    expect(prerequisiteIds('market.inspect')).not.toContain('external-file');
  });

  it('names a Market exactly where the catalogue binds one', () => {
    for (const standing of BROWSER_CAPABILITY_STANDINGS_V1) {
      const ids = browserActPrerequisitesV1(standing).map((entry) => entry.id);
      expect(ids.includes('market'), `${standing.action.id} disagrees with its own requiresMarket`)
        .toBe(standing.action.requiresMarket);
    }
  });

  it('writes every prerequisite as a sentence, never a bare label', () => {
    for (const standing of BROWSER_CAPABILITY_STANDINGS_V1) {
      for (const entry of browserActPrerequisitesV1(standing)) {
        expect(entry.statement.length, `${standing.action.id}'s ${entry.id} prerequisite is too short to be one`)
          .toBeGreaterThan(12);
      }
    }
  });

  it('has file-reading and file-free acts to tell apart at all', () => {
    // Both sides must be populated or the join above is vacuous in one
    // direction, which is exactly how a guard ends up agreeing with itself.
    const reading = BROWSER_CAPABILITY_STANDINGS_V1
      .filter((standing) => browserActPrerequisitesV1(standing).some((entry) => entry.id === 'external-file'));
    expect(reading.length).toBeGreaterThan(0);
    expect(reading.length).toBeLessThan(BROWSER_CAPABILITY_STANDINGS_V1.length);
  });
});

describe('the derivation refuses evidence that is not there', () => {
  /**
   * The adversarial half.
   *
   * Each case takes a real act and moves ONE anchor to a module whose evidence
   * does not support the claim. The point is not that these edits are likely;
   * it is that when someone makes one, the model answers from the code rather
   * than from what the row says about itself.
   */
  const act = (id: string): CapabilityActionV1 => {
    const found = CAPABILITY_ACTIONS_V1.find((candidate) => candidate.id === id);
    if (found === undefined) throw new Error(`${id} is not catalogued`);
    return found;
  };
  const rewrite = (id: string, next: Partial<CapabilityActionV1['anchors']>): CapabilityActionV1 => {
    const base = act(id);
    return Object.freeze({ ...base, anchors: Object.freeze({ ...base.anchors, ...next }) });
  };
  const standing = (candidate: CapabilityActionV1) => capabilityStandingV1(candidate, BROWSER_CAPABILITY_SURFACE_V1);

  it('does not let an act inherit a wallet it does not reach', () => {
    // `/general` authenticates a plan and hands the bytes back; it asks for no
    // key. Pointing a redemption at it does not make it a redemption.
    const borrowed = rewrite('claims.redeem', { owner: 'components/GeneralWorkspace.tsx' });
    expect(standing(borrowed).authority).toBe('none');
    expect(standing(borrowed).submits).toBe(false);
  });

  it('does not let an unrouted workspace become a capability', () => {
    // `RationalRepresentationWorkspace` builds a transfer and asks a wallet to
    // sign it. No page renders it, so no reader can perform it, so it is not
    // something this browser can do.
    const unrouted = rewrite('claims.represent', {
      owner: 'components/RationalRepresentationWorkspace.tsx',
      builder: 'lib/rationalTokenV2.ts',
    });
    expect(standing(unrouted).venue).toBe('no-venue');
    expect(standing(unrouted).routes).toEqual([]);
  });

  it('does not let a reachable page without a constructor become a capability', () => {
    // `/operate` renders a route export and decodes generated records, and has
    // no module that builds its transaction. Route reachability alone is the
    // route-magic mistake wearing a workspace's clothes.
    const magic = rewrite('direct.route', { runbook: null });
    expect(standing(magic).venue).toBe('no-venue');
  });

  it('reports a wallet act as exporting rather than sending when it cannot send', () => {
    // `/liquidity` signs and downloads; `/resolution` signs and submits. The
    // difference is one import, and the card must not blur it.
    expect(standing(act('dealer.liquidity')).authority).toBe('wallet-transaction');
    expect(standing(act('dealer.liquidity')).submits).toBe(false);
    expect(standing(act('source.close-fund')).submits).toBe(true);
  });
});
