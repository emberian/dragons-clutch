import { appendFileSync } from 'node:fs';

import { describe, expect, it } from 'vitest';

import { DEVNET_DEPLOYMENT_V1 } from './deployments';
import { inspectDirectTradeSpineV1 } from './directTradeSpine';
import { checkedReleaseSetIdsV1, PUBLIC_DEVNET_CUT_V1 } from './publicCutStaging';
import { SolanaRpcClient } from './rpc';

const live = process.env.DCLUTCH_LIVE_DEVNET === '1' ? it : it.skip;

/**
 * The panel's ask-the-chain path, run against the public cluster.
 *
 * `inspectDirectTradeSpineV1` is what the trade panel calls before any operator
 * route manifest exists, so this is the reader's first contact with the chain
 * and the place a drifted pin surfaces as a refusal.
 *
 * THE SUBJECT IS THE MARKET THIS SITE POINTS AT, and it is read out of the
 * public cut rather than typed here. Two cohort-8 addresses were pinned in
 * this file, and by 2026-09-02 both were owned by a Core program that had been
 * closed: the case asserted `spine.status === 'inspected'` about markets no
 * deployment could decode any more, so the only live coverage of the reader's
 * first chain contact was aimed at accounts nobody can reach. A live case that
 * names its own subject is a live case that goes stale with the fixture it
 * should have been reading.
 *
 * Gated on `DCLUTCH_LIVE_DEVNET=1` because it performs real network IO against
 * the configured devnet endpoint. Reads only; it signs and sends nothing.
 */
const FEATURED = PUBLIC_DEVNET_CUT_V1.market;

const report = (line: string) => {
  const out = process.env.DCLUTCH_LIVE_REPORT;
  if (out !== undefined) appendFileSync(out, `${line}\n`);
};

describe('live devnet Direct trade spine', () => {
  const featured = FEATURED === null ? [] : [Object.freeze({ name: 'the featured market', address: FEATURED })];
  for (const market of featured) {
    live(`reaches inspection for ${market.name} instead of refusing its descriptor`, async () => {
      const client = new SolanaRpcClient(process.env.DCLUTCH_LIVE_ENDPOINT ?? DEVNET_DEPLOYMENT_V1.endpoint);
      const spine = await inspectDirectTradeSpineV1(client, {
        marketAddress: market.address,
        coreProgramId: DEVNET_DEPLOYMENT_V1.programs.core,
        registryProgramId: DEVNET_DEPLOYMENT_V1.programs.registry,
        tradingProgramId: DEVNET_DEPLOYMENT_V1.programs.trading,
        claimsProgramId: DEVNET_DEPLOYMENT_V1.programs.claims,
        checkedReleaseSetIds: checkedReleaseSetIdsV1(),
      });

      if (spine.status === 'refused') {
        report(`${market.name} REFUSED: ${spine.reason}`);
        throw new Error(`${market.name} refused: ${spine.reason}`);
      }

      report(`\n===== ${market.name} ${market.address} =====`);
      report(`slot ${spine.observedSlot} phase ${spine.phase} generation ${spine.generation}`);
      report(`descriptor ${spine.descriptorId}`);
      report(`programSet ${spine.programSetId}  config ${spine.configId}`);
      report(`outcomes ${spine.outcomeCount} priceScale ${spine.priceScale} fee ${spine.feeBasisPoints}bps`);
      report(`root ${spine.rootAddress} exists=${spine.rootExists}`);
      report(`tradable=${spine.tradable} walls=${spine.walls.length}`);
      for (const wall of spine.walls) report(`  WALL ${wall.name}: ${wall.detail}`);
      report(`reason: ${spine.reason}`);

      expect(spine.status).toBe('inspected');
      expect(spine.marketAddress).toBe(market.address);
      // The descriptor conjunct is the thing that refused a reader. Reaching an
      // inspection at all means it decoded; these are the facts it yielded.
      expect(spine.descriptorId).toMatch(/^[0-9a-f]{64}$/);
      expect(spine.outcomeCount).toBeGreaterThan(0);
      expect(spine.priceScale).toBeGreaterThan(0n);
      // The featured market's Direct capability is founded AND switched on, so
      // its activation root stands and the panel's gate opens. A cut that
      // headlines a market whose trading was never activated would be pointing
      // every reader at a stepper they cannot use.
      expect(spine.rootExists).toBe(true);
      expect(spine.walls.map((wall) => wall.name)).not.toContain('phase');
      expect(spine.walls.map((wall) => wall.name)).not.toContain('activation');

      // The wall a reader used to meet at the preview button. The public cut
      // is this site's own deployment record and names the execution release
      // sets with a checked release; cohort-12 is a full redeploy and can
      // produce none, so the featured market's set is absent from it and the
      // fill is what waits. The assertion is conditional on the cut's own
      // answer rather than on a pinned expectation, so a cohort that DOES seal
      // one turns the wall off here without editing this case.
      const sealed = (PUBLIC_DEVNET_CUT_V1.checkedReleases[spine.releaseSetId] ?? null) !== null;
      report(`checked release for ${spine.releaseSetId}: ${sealed ? 'on file' : 'none'}`);
      expect(spine.walls.some((wall) => wall.name === 'release')).toBe(!sealed);
      expect(spine.tradable).toBe(sealed);
    }, 120_000);
  }
});
