import { PublicKey } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import { capabilityActContractV1, evaluateCapabilityV1 } from './capabilityModel';
import { BROWSER_CAPABILITY_STANDINGS_V1, capabilityWorkspaceV1 } from './capabilitySurface';
import { DEVNET_DEPLOYMENT_V1, DEVNET_PROGRAM_EVIDENCE_V1 } from './deployments';
import {
  liveDevnetOperatorPresetV1,
  checkedLiveDevnetOperatorPresetV1,
  type OperatorSurfaceSnapshotV1,
} from './operatorSurface';
import * as operatorSurfaceModule from './operatorSurface';

function key(byte: number): string { return new PublicKey(new Uint8Array(32).fill(byte)).toBase58(); }

/**
 * What the BROWSER adds to the operator surface, now that it adds nothing else.
 *
 * `lib/operatorSurface.ts` was a 377-line fork of the SDK's owner and is now a
 * re-export of it, so every acquisition question -- Loader pairs, the activation
 * cache, upgrade authorities, the release join, the slot floor -- is asked and
 * answered in `packages/dclutch-sdk/lib/operatorSurface.test.ts`, once. What is
 * left here is the part that is genuinely the browser's: which route a
 * capability sends a reader to, and the module-shape regression that only bites
 * inside a bundler's import graph.
 */
describe('the browser half of the operator surface', () => {
  it('gives the capability verdict ladder the exact snapshot it decides on', () => {
    // The operator snapshot is the only chain input a capability status takes,
    // so the ladder is checked here, against this file's own snapshot shape.
    // Everything else about a capability -- venue, authority, walls -- is
    // derived from the browser's import graph and gated in
    // `lib/capabilityEvidence.test.ts`.
    const redeem = BROWSER_CAPABILITY_STANDINGS_V1.find((standing) => standing.action.id === 'claims.redeem');
    expect(redeem).toBeDefined();
    expect(redeem && evaluateCapabilityV1(redeem, null)).toMatchObject({ status: 'needs-chain' });
    const withoutMarket = { market: null } as unknown as OperatorSurfaceSnapshotV1;
    expect(redeem && evaluateCapabilityV1(redeem, withoutMarket)).toMatchObject({ status: 'needs-market' });
    const withMarket = { market: { address: key(44) } } as unknown as OperatorSurfaceSnapshotV1;
    expect(redeem && evaluateCapabilityV1(redeem, withMarket)).toMatchObject({ status: 'ready-to-preflight' });
    expect(redeem && capabilityWorkspaceV1(redeem.action, withMarket)).toBe('/redeem');

    // A market-bound act has no address until a Market is read, and an act
    // with no venue never reaches the chain questions at all.
    const author = BROWSER_CAPABILITY_STANDINGS_V1.find((standing) => standing.action.id === 'direct.author');
    expect(author && capabilityWorkspaceV1(author.action, null)).toBeNull();
    expect(author && capabilityWorkspaceV1(author.action, withMarket)).toBe(`/market?address=${key(44)}`);
    const walled = BROWSER_CAPABILITY_STANDINGS_V1.find((standing) => standing.action.id === 'dealer.trade');
    expect(walled && evaluateCapabilityV1(walled, withMarket)).toMatchObject({ status: 'no-venue' });
    expect(walled && capabilityActContractV1(walled).venue).toBe('Nothing here can build it yet');
  });
});

describe('the live preset is derived on demand, never at import', () => {
  /**
   * THE DEFECT THIS CLOSES is a latent bundle bug, not a test inconvenience.
   *
   * `liveDevnetOperatorPresetV1()` used to be a module-scope `const` whose
   * initializer calls `PublicKey.findProgramAddressSync` seven times. That
   * function SEARCHES — it walks 256 nonces and throws `Unable to find a
   * viable program address nonce` when none is off-curve — so the module could
   * throw while merely being imported. It did: past the eighteenth component
   * import in one module graph it threw during collection, while the same
   * module imported alone evaluated fine. Bisected to that exact boundary.
   *
   * A page that happens to import one more component would ship broken, and
   * the stack would name `operatorSurface` rather than whatever pushed the
   * graph over. Deriving on first use instead means the throw lands where a
   * caller can see and handle it, and importing a sibling can never take a
   * page down.
   *
   * Every check the eager version made still runs, unchanged, on first call.
   * This is laziness, not a relaxed guard.
   */
  it('exports a function, and no pre-derived constant', () => {
    // The shape check is the load-bearing one: re-adding the eager const is
    // exactly the regression, and it would otherwise look like a tidy-up.
    expect(typeof liveDevnetOperatorPresetV1).toBe('function');
    expect(Object.keys(operatorSurfaceModule)).not.toContain('liveDevnetOperatorPresetV1()');
  });

  it('derives once and hands back the same frozen preset', () => {
    const first = liveDevnetOperatorPresetV1();
    expect(liveDevnetOperatorPresetV1()).toBe(first);
    expect(Object.isFrozen(first)).toBe(true);
  });

  it('still refuses a preset whose ProgramData is not its Loader-v3 coordinate', () => {
    // The guard the eager derivation existed for, proven to still bite.
    const tampered = {
      ...DEVNET_PROGRAM_EVIDENCE_V1,
      core: { ...DEVNET_PROGRAM_EVIDENCE_V1.core, programData: '11111111111111111111111111111111' },
    };
    expect(() => checkedLiveDevnetOperatorPresetV1(DEVNET_DEPLOYMENT_V1, tampered))
      .toThrow(/is not the canonical Loader-v3 coordinate/);
  });
});
