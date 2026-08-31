import { appendFileSync } from 'node:fs';

import { describe, expect, it } from 'vitest';

import { DEVNET_DEPLOYMENT_V1 } from './deployments';
import { inspectDirectTradeSpineV1 } from './directTradeSpine';
import { SolanaRpcClient } from './rpc';

const live = process.env.DCLUTCH_LIVE_DEVNET === '1' ? it : it.skip;

/**
 * The panel's ask-the-chain path, run against the public cluster.
 *
 * `inspectDirectTradeSpineV1` is what the trade panel calls before any operator
 * route manifest exists, so this is the reader's first contact with the chain
 * and the place a drifted pin surfaces as a refusal. Cohort-8's two markets are
 * the subjects because they are the ones a reader was turned away from: their
 * CapabilityProgramV4 record carries the effect kernel's V4 schema, which the
 * browser pinned at V3 until `EFFECT_SCHEMA_RELEASE_ID_V4` landed.
 *
 * Gated on `DCLUTCH_LIVE_DEVNET=1` because it performs real network IO against
 * api.devnet.solana.com. Reads only; it signs and sends nothing.
 */
const COHORT_8 = Object.freeze([
  Object.freeze({ name: 'market21', address: '5w24EmP7Q2Kkw9y9tjMPdixLPMdJHA1xsY7Wip3k5SDm' }),
  Object.freeze({ name: 'market22', address: '8Xky2yx3wBmDRXeNfKSuJigqiWDtwSvGvB75BSW6tPxK' }),
]);

const report = (line: string) => {
  const out = process.env.DCLUTCH_LIVE_REPORT;
  if (out !== undefined) appendFileSync(out, `${line}\n`);
};

describe('live devnet Direct trade spine', () => {
  for (const market of COHORT_8) {
    live(`reaches inspection for ${market.name} instead of refusing its descriptor`, async () => {
      const client = new SolanaRpcClient(DEVNET_DEPLOYMENT_V1.endpoint);
      const spine = await inspectDirectTradeSpineV1(client, {
        marketAddress: market.address,
        coreProgramId: DEVNET_DEPLOYMENT_V1.programs.core,
        registryProgramId: DEVNET_DEPLOYMENT_V1.programs.registry,
        tradingProgramId: DEVNET_DEPLOYMENT_V1.programs.trading,
        claimsProgramId: DEVNET_DEPLOYMENT_V1.programs.claims,
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
      // Cohort-8 was activated, so its capability root stands.
      expect(spine.rootExists).toBe(true);
    }, 120_000);
  }
});
