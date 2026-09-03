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
  /*
    A SUITE THAT REGISTERS NOTHING IS NOT A SKIPPED SUITE, it is a file that
    fails to collect -- which is how this case behaved the moment the cut went
    pending between cohorts, reported as a red file with no failing test in it.
    "Failed" and "never ran" are different numbers and a suite has to be able to
    say which one it is, so the absence of a featured market registers an
    explicitly skipped case that names the reason.
  */
  if (FEATURED === null) {
    it.skip('has no featured market to inspect: the public cut is pending between cohorts', () => {});
  }
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
      // WHAT A CUT OWES A READER IS A MARKET THAT IS NOT PRE-OPEN, and this
      // case pinned `Open` exactly while ITS OWN REASONING said "below Open".
      //
      // The history: it first asserted `rootExists` and no `activation` wall,
      // on the reasoning that headlining an unactivated market points readers
      // at a stepper they cannot use -- which stopped being true when step 1
      // moved outside the gate, because the chain admits participants before it
      // admits a fill. It was rewritten to pin `Open`, "because below Open even
      // joining is refused". Founding is below Open. TERMINAL IS ABOVE IT, and
      // the literal was stronger than the sentence beside it.
      //
      // Cohort-14 is where the difference costs something. It has an Open market
      // whose Pyth Receiver pin was superseded before it was founded -- it can
      // never be captured, so its only reachable terminal is a failure walk this
      // project has already shipped once and refuses to ship twice -- and a
      // Terminal market that was captured inside its window, settled on a
      // success certificate and paid a real wallet. Headlining the first because
      // it is Open would point every reader at the market that cannot answer.
      //
      // So the rule is stated as the reasoning always meant it: never Founding,
      // and then each phase is held to what it actually owes.
      expect(spine.phase).not.toBe('Founding');
      const answered = spine.phase === 'Terminal' || spine.phase === 'Retiring' || spine.phase === 'Retired';
      // A market that has answered MUST carry the phase wall -- a spine that
      // reported a resolved market as joinable would be the same lie in the
      // other direction -- and it must not be tradable.
      if (answered) {
        expect(spine.walls.map((wall) => wall.name)).toContain('phase');
        expect(spine.tradable).toBe(false);
      } else {
        expect(spine.walls.map((wall) => wall.name)).not.toContain('phase');
      }
      // Every remaining wall is reported rather than assumed away, and each
      // must be one this browser has a card for: an unnamed wall would reach a
      // reader as a blank gate.
      for (const wall of spine.walls) {
        expect(['phase', 'activation', 'release', 'prestate', 'packet']).toContain(wall.name);
        report(`  wall ${wall.name}: ${wall.detail}`);
      }
      // Activation is the operator's move and has a deadline, so when it
      // stands the gate must carry the sentence that says whose move it is.
      const activation = spine.walls.find((wall) => wall.name === 'activation') ?? null;
      if (activation !== null) expect(activation.detail).toContain('operator');
      expect(spine.rootExists).toBe(activation === null);

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
      // Tradable is the CONJUNCTION, and this pinned it as though the checked
      // release were the only thing in the way -- true while the release wall
      // was the only market-level wall that could stand, and wrong the moment
      // a sealed market turned up unactivated, which is cohort-13 exactly.
      // Sealing says the fill would be admitted at the route boundary; it says
      // nothing about whether the capability is switched on.
      const marketWalls = spine.walls.filter((wall) => ['phase', 'activation', 'release'].includes(wall.name));
      expect(spine.tradable).toBe(marketWalls.length === 0);
    }, 120_000);
  }
});
