import { describe, expect, it } from 'vitest';

import { DEVNET_DEPLOYMENT_V1 } from '@dclutch/sdk/deployments';
import { PUBLIC_DEVNET_CUT_V1 } from '@dclutch/sdk/publicCutStaging';
import { inspectMarketDetailV1 } from '@dclutch/sdk/marketDetail';
import { derivedOutcomeLabelsV1, inspectMarketQuestionV1 } from '@dclutch/sdk/marketQuestion';
import { inspectMarketResolutionV1 } from '@dclutch/sdk/marketResolution';
import { ordinarySelectorJoinV1 } from '@dclutch/sdk/ordinarySelectorV1';
import { SolanaRpcClient } from '@dclutch/sdk/rpc';

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
 * THE SETTLED MARKET THE SITE FEATURES, READ OUT OF THE PUBLIC CUT.
 *
 * This named two cohort-14 markets as literals, and both went unreadable the
 * hour their programs were closed -- a pin beside the fixture it should have
 * been reading, which is the staleness the registry itself was built to stop.
 * The address comes from the cut now, so a cohort boundary moves it.
 *
 * AND THE ASSERTION TURNED OVER WITH THE COHORT. Cohort-14's markets declared
 * the identity scale while comparing a raw Pyth mantissa at exponent -8 against
 * cuts in dollars, so their two readings DISAGREED and the chain paid the cell
 * its own boundaries do not imply. Cohort-15's featured market is the first
 * founded after `4cd2b9cb5` gave `StatisticSpecV1` a `source_scale_exponent`,
 * and its two readings AGREE. Both directions are asserted below, because
 * "they agree" alone passes on any market whose defect happens to be invisible:
 * the counterfactual reading at exponent 0 -- exactly what cohort-14 declared --
 * must land on a DIFFERENT cell, or this market's factor is doing no work and
 * the agreement proves nothing.
 *
 * `DCLUTCH_RESOLVED_MARKET` still overrides, so a later cohort can be pointed
 * at one market without editing this file.
 */
const SETTLED_MARKETS_V1: ReadonlyArray<Readonly<{ name: string; address: string }>> =
  process.env.DCLUTCH_RESOLVED_MARKET
    ? [{ name: 'the market named by DCLUTCH_RESOLVED_MARKET', address: process.env.DCLUTCH_RESOLVED_MARKET }]
    : PUBLIC_DEVNET_CUT_V1.market === null
      ? []
      : [{ name: 'the featured market', address: PUBLIC_DEVNET_CUT_V1.market }];

const live = process.env.DCLUTCH_LIVE_DEVNET === '1' ? it : it.skip;

describe('live devnet: the certificate-to-partition join', () => {
  it('has a settled market to read', () => { expect(SETTLED_MARKETS_V1.length, 'the public cut names no market, so this whole file asserts nothing').toBeGreaterThan(0); });

  for (const market of SETTLED_MARKETS_V1) live(`derives the very cell the chain committed for ${market.name}, from that market’s own records`, async () => {
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
    // A DECLARED FACTOR, not the identity. This pinned 0 and said "every
    // cohort-14 market declares the identity", which was true and is the
    // defect: those four bytes were a reserved span enforced zero, so a
    // pre-factor record decodes to a STATED identity rather than to an absent
    // scale, and the founder had no way to say the feed and the cuts are
    // written in different units.
    expect(resolution.scale.sourceScaleExponent, 'the featured market declares a source-to-result factor').not.toBe(0);
    expect(resolution.scale.statisticRecord).not.toBe(resolution.scale.sourceMaterialRecord);

    const join = ordinarySelectorJoinV1(question, resolution.observation, resolution.selector, resolution.scale.sourceScaleExponent);
    expect(join.refusal).toBeNull();
    expect(join.derived).toBe(resolution.selector);
    expect(join.agrees).toBe(true);

    // AND WHAT THAT AGREEMENT IS AND IS NOT. The scale above came off the
    // chain, so the agreement is an exact statement of what the protocol DID,
    // and no statement at all about whether the cell is right about the world.
    // A founding that declared the wrong shift is reproduced faithfully here
    // and is still wrong.
    //
    // THE COUNTERFACTUAL IS THE OTHER HALF OF THE ASSERTION. Read at exponent
    // 0 -- what every cohort-14 market declared, and what a reader that assumes
    // the identity computes -- the same observation and the same cuts land on a
    // DIFFERENT cell. So the factor is load-bearing on this market rather than
    // decorative, and the agreement above is not the accident of a market whose
    // reading would fall in the committed cell either way.
    const asCohort14Declared = ordinarySelectorJoinV1(question, resolution.observation, resolution.selector, 0);
    expect(asCohort14Declared.refusal).toBeNull();
    expect(asCohort14Declared.derived).not.toBe(join.derived);
    expect(asCohort14Declared.agrees).toBe(false);

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
