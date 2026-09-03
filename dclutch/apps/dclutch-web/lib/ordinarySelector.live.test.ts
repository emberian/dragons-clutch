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
/**
 * BOTH SETTLED COHORT-14 MARKETS, because one of them is the case that reached
 * a stranger.
 *
 * Market B’s mis-scaled settlement moved atoms between the founder and the
 * founder; market C was FILLED, and participant-2 bought 200 claims at index 1
 * — the cell the reading falls in — which the deployed program pays zero
 * (`docs/evidence/COHORT14_SEALED_FOUNDED_FILLED_2026_09_03.md`). Running both
 * is what makes this an assertion about the DEFECT rather than about one
 * market: they were founded by the same path, they declare the same identity
 * scale, and the same two readings differ on each.
 *
 * `DCLUTCH_RESOLVED_MARKET` still overrides, and overrides the whole list, so a
 * later cohort can be pointed at one market without editing this file.
 */
const COHORT14_SETTLED_MARKETS_V1: ReadonlyArray<Readonly<{ name: string; address: string }>> =
  process.env.DCLUTCH_RESOLVED_MARKET
    ? [{ name: 'the market named by DCLUTCH_RESOLVED_MARKET', address: process.env.DCLUTCH_RESOLVED_MARKET }]
    : [
      { name: 'cohort-14 market B', address: 'DUVcCGfjXzp1fBktTCjsAomgrn9S6sxSDziQHoyRiu8A' },
      { name: 'cohort-14 market C', address: 'BL8zsFokbz7aEdo3wjtcNffd5P1D8a9wVxwKq3mcMsMN' },
    ];

const live = process.env.DCLUTCH_LIVE_DEVNET === '1' ? it : it.skip;

describe('live devnet: the certificate-to-partition join', () => {
  for (const market of COHORT14_SETTLED_MARKETS_V1) live(`derives the very cell the chain committed for ${market.name}, from that market’s own records`, async () => {
    const client = new SolanaRpcClient(process.env.DCLUTCH_LIVE_ENDPOINT ?? DEVNET_DEPLOYMENT_V1.endpoint);
    const detail = await inspectMarketDetailV1(client, {
      coreProgramId: DEVNET_DEPLOYMENT_V1.programs.core,
      registryProgramId: DEVNET_DEPLOYMENT_V1.programs.registry,
      claimsProgramId: DEVNET_DEPLOYMENT_V1.programs.claims,
      custodyProgramId: DEVNET_DEPLOYMENT_V1.programs.custody,
      address: market.address,
    });
    expect(detail.card.status, detail.reason).toBe('decoded');
    if (detail.card.status !== 'decoded') return;
    expect(detail.card.settlement.status, 'this case needs a market that has settled').toBe('terminal');

    const question = await inspectMarketQuestionV1(client, {
      registryProgramId: DEVNET_DEPLOYMENT_V1.programs.registry,
      address: market.address,
      productRecordId: detail.card.identity.productRecordId,
      resolutionPolicyId: detail.card.identity.resolutionPolicyId,
    });
    const resolution = await inspectMarketResolutionV1(client, {
      card: detail.card,
      resolutionProgramId: DEVNET_DEPLOYMENT_V1.programs.resolution,
      floorSlot: detail.card.observedSlot,
      question,
      registryProgramId: DEVNET_DEPLOYMENT_V1.programs.registry,
    });
    expect(resolution.status).toBe('authenticated');
    if (resolution.status !== 'authenticated') return;
    expect(resolution.sourceReported, 'this case needs a success certificate, not a source failure').toBe(true);

    // THE SCALE, READ. The shift below is no longer a literal at this call
    // site: `inspectMarketDeclaredScaleV1` walked this market's own
    // `SourceMaterialV3` to its own `StatisticSpecV1` and read
    // `source_scale_exponent` out of the coordinate `DClutch.SourceStatisticSpecV1Abi`
    // emits. Market B declares the identity, and it declares it rather than
    // merely not contradicting it: the four bytes the factor occupies were
    // reserved and enforced zero before `4cd2b9cb5`, so a pre-factor record
    // decodes to a stated scale and not to an absent one.
    expect(resolution.scale.status, resolution.scale.status === 'unread' ? resolution.scale.reason : '').toBe('declared');
    if (resolution.scale.status !== 'declared') return;
    expect(resolution.scale.sourceScaleExponent, 'every cohort-14 market declares the identity').toBe(0);
    expect(resolution.scale.statisticRecord).not.toBe(resolution.scale.sourceMaterialRecord);

    const join = ordinarySelectorJoinV1(question, resolution.observation, resolution.selector, resolution.scale.sourceScaleExponent);
    expect(join.refusal).toBeNull();
    expect(join.derived).toBe(resolution.selector);
    expect(join.agrees).toBe(true);

    // AND WHAT THAT AGREEMENT IS AND IS NOT. The scale above came off the
    // chain, so the agreement is an exact statement of what the protocol DID,
    // and no statement at all about whether the cell is right about the world.
    //
    // It is not. This market's cuts are dollars authored in cents and its
    // observation is a raw Pyth mantissa at exponent -8, and read on the
    // cuts' own scale the price is inside the band rather than outside it.
    // Both cells below are honest arithmetic; they differ only in whether a
    // factor was declared, and that difference moved 500,000,000 atoms.
    const onTheCutsScale = ordinarySelectorJoinV1(question, resolution.observation, resolution.selector, -8);
    expect(onTheCutsScale.refusal).toBeNull();
    expect(onTheCutsScale.derived).not.toBe(join.derived);
    expect(onTheCutsScale.agrees).toBe(false);
    // The two cells NAMED rather than merely differing. Both markets committed
    // the top cell and both readings fall one cell lower, and asserting the
    // pair is what stops this from passing on any two numbers that happen not
    // to be equal.
    expect(join.derived, 'the chain paid the top ordinary cell').toBe(2);
    expect(onTheCutsScale.derived, 'the price is inside the band, which pays zero').toBe(1);

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
