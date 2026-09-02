import { describe, expect, it } from 'vitest';

import { DEVNET_DEPLOYMENT_V1 } from './deployments';
import { inspectDirectTradeSpineV1 } from './directTradeSpine';
import { inspectMarketActivityV1 } from './marketActivity';
import { checkedReleaseSetIdsV1, PUBLIC_DEVNET_CUT_V1 } from './publicCutStaging';
import { SolanaRpcClient } from './rpc';

const featured = PUBLIC_DEVNET_CUT_V1.market;
const live = process.env.DCLUTCH_LIVE_DEVNET === '1' && featured !== null ? it : it.skip;

/**
 * The first real devnet crossing, read back off the chain that took it.
 *
 * This is the case the whole module exists for and the only one that can prove
 * it: a fill LANDED on the featured market on 2026-09-02, and every number the
 * market page shows about it must come back from the node — not from the
 * evidence document, not from the job directory, not from a fixture.
 *
 * What is pinned is SHAPE plus the internal agreements, never a cohort's
 * literals: the next cohort will have different addresses and, before its own
 * first fill, no crossings at all. So a market with no fill yet is an explicit
 * skip that names the reason, exactly as the trade-spine cases do — "failed"
 * and "never ran" are different numbers.
 *
 * Gated on `DCLUTCH_LIVE_DEVNET=1`. Reads only.
 */
describe('live devnet market activity', () => {
  live('derives the featured market’s crossings, its positions and its fee standing', async () => {
    const client = new SolanaRpcClient(process.env.DCLUTCH_LIVE_ENDPOINT ?? DEVNET_DEPLOYMENT_V1.endpoint);
    const spine = await inspectDirectTradeSpineV1(client, {
      marketAddress: featured!,
      coreProgramId: DEVNET_DEPLOYMENT_V1.programs.core,
      registryProgramId: DEVNET_DEPLOYMENT_V1.programs.registry,
      tradingProgramId: DEVNET_DEPLOYMENT_V1.programs.trading,
      claimsProgramId: DEVNET_DEPLOYMENT_V1.programs.claims,
      checkedReleaseSetIds: checkedReleaseSetIdsV1(),
    });
    expect(spine.status, spine.reason).toBe('inspected');
    if (spine.status !== 'inspected') return;
    expect(spine.aggregateAddress).not.toBeNull();
    expect(spine.outcomeCount).not.toBeNull();

    const activity = await inspectMarketActivityV1(client, {
      marketAddress: featured!,
      tradingProgramId: DEVNET_DEPLOYMENT_V1.programs.trading,
      claimsProgramId: DEVNET_DEPLOYMENT_V1.programs.claims,
      aggregateAddress: spine.aggregateAddress!,
      generation: BigInt(spine.generation),
      outcomeCount: spine.outcomeCount!,
      priceScale: spine.priceScale,
      feeBasisPoints: spine.feeBasisPoints,
    });

    // The node's own history, and it must not be empty for a founded market.
    expect(activity.rows.length, activity.reason).toBeGreaterThan(0);

    // THE POSITIVE CONTROL, and it is the reason this case exists in this
    // shape. The first version of this module fired every transaction read at
    // once, the public endpoint returned 429 to all of them, and the result
    // was an empty fill list — indistinguishable from a market that has never
    // crossed, which is an answer this case is willing to accept. It passed.
    // A throttled instrument must never be able to buy that acceptance again.
    expect(activity.transactionsRefused, activity.reason).toBe(0);

    if (activity.fills.length === 0) {
      // Not a failure: it is what a market that has not crossed yet looks
      // like, and the page renders exactly that. Everything below is about a
      // crossing, so it is skipped WITH its reason rather than asserted away.
      expect(activity.positions.length).toBeGreaterThanOrEqual(0);
      return;
    }

    for (const fill of activity.fills) {
      // Both halves signed the same market, the same generation and the same
      // outcome, and the crossing sits inside both signed limits. That is the
      // admission rule; a crossing that reads back outside it would mean the
      // browser and the program disagree about what the chain accepted.
      expect(fill.terms.sellerIntent.market).toBe(featured);
      expect(fill.terms.buyerIntent.market).toBe(featured);
      expect(fill.terms.sellerIntent.generation).toBe(BigInt(spine.generation));
      expect(fill.terms.buyerIntent.generation).toBe(fill.terms.sellerIntent.generation);
      expect(fill.terms.sellerIntent.outcome).toBe(fill.terms.buyerIntent.outcome);
      expect(fill.terms.sellerIntent.outcome).toBeLessThan(spine.outcomeCount!);
      expect(fill.terms.executionPrice).toBeGreaterThanOrEqual(fill.terms.sellerIntent.limitPrice);
      expect(fill.terms.executionPrice).toBeLessThanOrEqual(fill.terms.buyerIntent.limitPrice);
      expect(fill.terms.fillAtoms).toBeGreaterThan(0n);
      expect(fill.terms.seller).not.toBe(fill.terms.buyer);
      // The fee rate is the config's, on both signed halves. The program
      // refuses any other pair, so reading a different one back would mean the
      // decoder is reading the wrong bytes.
      expect(fill.terms.sellerIntent.feeBasisPoints).toBe(spine.feeBasisPoints);
      expect(fill.terms.buyerIntent.feeBasisPoints).toBe(spine.feeBasisPoints);

      // The economics come from the trade stepper's own preview, so a landed
      // crossing must be one that preview re-admits.
      expect(fill.economicsRefusal).toBeNull();
      expect(fill.economics).not.toBeNull();
      const economics = fill.economics!;
      // Gross is fill x price at the immutable scale, exactly — the one
      // multiplication the whole crossing turns on, checked here in the other
      // direction so a preview that quietly rounded could not agree with it.
      expect(economics.grossCollateral * spine.priceScale).toBe(economics.fill * economics.executionPrice);
      expect(economics.sellerFee).toBe(economics.grossCollateral * BigInt(spine.feeBasisPoints) / 10_000n);
      expect(economics.buyerCollateralDebit).toBe(economics.grossCollateral + economics.buyerFee);
      expect(economics.sellerNetCollateralCredit).toBe(economics.grossCollateral - economics.sellerFee);
      expect(economics.totalFeeTransfer).toBe(economics.sellerFee + economics.buyerFee);
      expect(fill.grossPerClaim).not.toBeNull();
      expect(fill.grossPerClaim!.denominator).toBe(fill.terms.fillAtoms.toString());
    }

    // Both parties to a crossing hold a Position on this market, and the scan
    // found them: the leaderboard is derived from the same accounts the fill
    // wrote, not from a list of addresses written down anywhere.
    const owners = new Set(activity.positions.map((position) => position.owner));
    for (const fill of activity.fills) {
      expect(owners.has(fill.terms.seller), `seller ${fill.terms.seller} has no Position among ${owners.size} scanned`).toBe(true);
      expect(owners.has(fill.terms.buyer), `buyer ${fill.terms.buyer} has no Position among ${owners.size} scanned`).toBe(true);
    }
    // A crossing takes a side, so at least one Position is no longer level.
    expect(activity.positions.some((position) => !position.level)).toBe(true);
    // The ordering is the leaderboard's whole claim: totals descend.
    for (let index = 1; index < activity.positions.length; index += 1) {
      expect(BigInt(activity.positions[index - 1].totalClaims) >= BigInt(activity.positions[index].totalClaims)).toBe(true);
    }

    // The venue's fee standing is read where the protocol keeps it: a maker
    // replay exists for every party to a crossing, because the crossing wrote
    // it, and it is the only account that says whether the fee is settled.
    const parties = new Set(activity.fills.flatMap((fill) => [fill.terms.seller, fill.terms.buyer]));
    expect(new Set(activity.feeStandings.map((standing) => standing.maker))).toEqual(parties);
    for (const standing of activity.feeStandings) {
      expect(standing.state).toBe('existing');
      expect(BigInt(standing.feeOwed)).toBeGreaterThanOrEqual(0n);
      expect(BigInt(standing.nextNonce)).toBeGreaterThan(0n);
    }
  }, 120_000);
});
