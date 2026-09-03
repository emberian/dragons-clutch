import { describe, expect, it } from 'vitest';

import { DEVNET_DEPLOYMENT_V1 } from './deployments';
import { inspectMarketDetailV1 } from './marketDetail';
import { derivedOutcomeLabelsV1, inspectMarketQuestionV1 } from './marketQuestion';
import { inspectMarketResolutionV1 } from './marketResolution';
import { ordinarySelectorJoinV1 } from './ordinarySelectorV1';
import { SolanaRpcClient } from './rpc';

/**
 * THE JOIN THAT WAS CALLED UNDECIDABLE, decided against the live chain.
 *
 * `MarketDetailWorkspace.tsx` refused to name any ordinary cell because the
 * certificate carries the observation as `10062091764/1` while the cuts sit at
 * `9900, 10300` over `100`, with no exponent published anywhere. The refusal
 * was right that those are not one scale and wrong that the join needed one:
 * the Resolution program applies no exponent either, so the selector is a
 * function of exactly the numbers already on this page.
 *
 * This reads market B end to end — the Market, its Product records, its
 * certificate — and asserts that the cell derived from the market's OWN
 * partition is the cell the chain committed. Nothing here is typed: the cuts,
 * the denominator, the observation and the selector all arrive from finalized
 * accounts, so a re-founding on different cuts changes every number and not
 * whether this passes.
 *
 * Devnet evidence. Not mainnet evidence.
 */
const COHORT14_MARKET_B_V1 = process.env.DCLUTCH_RESOLVED_MARKET ?? 'DUVcCGfjXzp1fBktTCjsAomgrn9S6sxSDziQHoyRiu8A';

const live = process.env.DCLUTCH_LIVE_DEVNET === '1' ? it : it.skip;

describe('live devnet: the certificate-to-partition join', () => {
  live('derives the very cell the chain committed, from the market’s own records', async () => {
    const client = new SolanaRpcClient(process.env.DCLUTCH_LIVE_ENDPOINT ?? DEVNET_DEPLOYMENT_V1.endpoint);
    const detail = await inspectMarketDetailV1(client, {
      coreProgramId: DEVNET_DEPLOYMENT_V1.programs.core,
      registryProgramId: DEVNET_DEPLOYMENT_V1.programs.registry,
      claimsProgramId: DEVNET_DEPLOYMENT_V1.programs.claims,
      custodyProgramId: DEVNET_DEPLOYMENT_V1.programs.custody,
      address: COHORT14_MARKET_B_V1,
    });
    expect(detail.card.status, detail.reason).toBe('decoded');
    if (detail.card.status !== 'decoded') return;
    expect(detail.card.settlement.status, 'this case needs a market that has settled').toBe('terminal');

    const question = await inspectMarketQuestionV1(client, {
      registryProgramId: DEVNET_DEPLOYMENT_V1.programs.registry,
      address: COHORT14_MARKET_B_V1,
      productRecordId: detail.card.identity.productRecordId,
      resolutionPolicyId: detail.card.identity.resolutionPolicyId,
    });
    const resolution = await inspectMarketResolutionV1(client, {
      card: detail.card,
      resolutionProgramId: DEVNET_DEPLOYMENT_V1.programs.resolution,
      floorSlot: detail.card.observedSlot,
      question,
    });
    expect(resolution.status).toBe('authenticated');
    if (resolution.status !== 'authenticated') return;
    expect(resolution.sourceReported, 'this case needs a success certificate, not a source failure').toBe(true);

    const join = ordinarySelectorJoinV1(question, resolution.observation, resolution.selector);
    expect(join.refusal).toBeNull();
    expect(join.derived).toBe(resolution.selector);
    expect(join.agrees).toBe(true);

    // The chain's own agreement, read twice: Core's `terminal_winner` and the
    // certificate's selector are separate bytes in separate accounts, and the
    // derived cell equals both or this join means nothing.
    expect(detail.card.settlement.status === 'terminal' ? detail.card.settlement.winner : null).toBe(resolution.selector);

    // The cell is now nameable, so name it — and prove the name is an ORDINARY
    // one rather than the source-failure cell the certificate kind already
    // pins.
    const labels = derivedOutcomeLabelsV1(question, null);
    expect(join.derived).toBeLessThan(question.regionCount);
    expect(labels[join.derived!]).toBeTruthy();
    expect(labels[join.derived!]).not.toBe('The source failed to report');
  });
});
