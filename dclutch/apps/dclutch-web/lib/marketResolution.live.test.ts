import { describe, expect, it } from 'vitest';

import { DEVNET_DEPLOYMENT_V1 } from './deployments';
import { inspectMarketDiscoveryV1 } from './marketDiscovery';
import { inspectMarketQuestionV1 } from './marketQuestion';
import { inspectMarketResolutionV1, marketRedemptionStateV1 } from './marketResolution';
import { PUBLIC_DEVNET_CUT_V1 } from './publicCutStaging';
import { SolanaRpcClient } from './rpc';

const live = process.env.DCLUTCH_LIVE_DEVNET === '1' ? it : it.skip;

/**
 * The featured market's own answer, read off the chain the way the page reads
 * it. Gated on `DCLUTCH_LIVE_DEVNET=1`; reads only.
 *
 * IT TAKES THE MARKET FROM THE CUT, never from a literal beside it -- the
 * lesson the registry case paid for when it pinned a market and went red the
 * hour that cohort closed. So this file states nothing about WHICH market is
 * featured; it states what must be true of whatever one is, and branches on
 * the phase the chain reports rather than on a phase written down here.
 */
describe('live devnet market resolution', () => {
  live('reads the featured market, and joins any certificate it names to Core', async () => {
    const featured = PUBLIC_DEVNET_CUT_V1.market;
    expect(featured, 'the public cut names no market').not.toBeNull();
    const client = new SolanaRpcClient(process.env.DCLUTCH_LIVE_ENDPOINT ?? DEVNET_DEPLOYMENT_V1.endpoint);
    const discovery = await inspectMarketDiscoveryV1(client, {
      coreProgramId: DEVNET_DEPLOYMENT_V1.programs.core,
      registryProgramId: DEVNET_DEPLOYMENT_V1.programs.registry,
      claimsProgramId: DEVNET_DEPLOYMENT_V1.programs.claims,
      custodyProgramId: DEVNET_DEPLOYMENT_V1.programs.custody,
      addresses: [featured!],
    });
    const card = discovery.cards[0];
    expect(card?.status, card?.refusal ?? '').toBe('decoded');
    if (card === undefined || card.status !== 'decoded') return;

    const question = await inspectMarketQuestionV1(client, {
      registryProgramId: DEVNET_DEPLOYMENT_V1.programs.registry,
      address: featured!,
      productRecordId: card.identity.productRecordId,
      resolutionPolicyId: card.identity.resolutionPolicyId,
    });
    const resolution = await inspectMarketResolutionV1(client, {
      card,
      resolutionProgramId: DEVNET_DEPLOYMENT_V1.programs.resolution,
      floorSlot: discovery.floorSlot,
      question,
      // Supplied so the reader walks this market's own SourceMaterialV3 to its
      // own StatisticSpecV1 and states the scale it DECLARES. Omitting it would
      // leave the scale `unread`, which is a truthful status and a weaker test.
      registryProgramId: DEVNET_DEPLOYMENT_V1.programs.registry,
    });

    if (card.settlement.status !== 'terminal') {
      // An Open market has no certificate and the reader must SAY so rather
      // than refusing, because "not yet" and "it did not read" are different
      // facts and a page that conflates them tells a reader to wait forever.
      //
      // THE BRANCH USED TO ASK `card.phase !== 'Terminal'`, AND THAT IS A
      // DIFFERENT QUESTION. A settled Market keeps its terminal receipt through
      // Retiring and Retired -- `decodeMarketCoreStateV2` admits all three and
      // refuses a receipt without one of them -- so the moment cohort-15's
      // featured market ran its payouts and began retiring, this case asserted
      // that a market with an authenticated certificate had none. The receipt
      // is what decides whether a certificate exists; the phase decides only
      // how far past settlement the market has walked.
      expect(resolution.status).toBe('not-terminal');
      return;
    }

    // A Terminal market's certificate must AUTHENTICATE. A refusal here is the
    // finding, so it is reported with its own reason rather than swallowed.
    expect(resolution.status, resolution.status === 'refused' ? resolution.reason : '').toBe('authenticated');
    if (resolution.status !== 'authenticated') return;

    // The join already proved the selector equals Core's own terminal winner;
    // this asserts the two independently so a weakened join goes red here too.
    expect(resolution.selector).toBe(card.settlement.status === 'terminal' ? card.settlement.winner : -1);
    expect(resolution.kind === 'resolution-success' || resolution.kind === 'resolution-failure').toBe(true);
    expect(resolution.sourceReported).toBe(resolution.kind === 'resolution-success');

    if (resolution.sourceReported) {
      const observation = resolution.observation;
      expect(observation, 'a success certificate carries an observation').not.toBeNull();
      expect(observation!.denominator > 0n).toBe(true);
      expect(observation!.atUnixSeconds > 1_700_000_000n).toBe(true);
      // THE SENTENCE THE PAGE MAKES. An honest resolution is one whose
      // observation fell inside the window the market published before it
      // opened, and both halves are read: the instant off the certificate, the
      // window off the market's own WindowSpec record.
      expect(observation!.standing).toBe('inside');
      expect(question.window).not.toBeNull();
      expect(observation!.atUnixSeconds >= question.window!.startUnixSeconds).toBe(true);
      expect(observation!.atUnixSeconds <= question.window!.endUnixSeconds).toBe(true);
      expect(resolution.providerEvidenceId).toMatch(/^[0-9a-f]{64}$/);
      // AND ON WHAT SCALE. The market's own `StatisticSpecV1`, reached through
      // its own `SourceMaterialV3`, both authenticated at the Registry PDAs
      // their content digests derive. An observation with no declared scale
      // beside it is a number a reader cannot compare to anything -- which is
      // the whole of `docs/design/OBSERVATION_SCALE_AUTHORITY.md` -- so a
      // certificate that authenticates and a scale that does not read is a
      // finding, and it is reported with its own reason rather than tolerated.
      expect(resolution.scale.status, resolution.scale.status === 'unread' ? resolution.scale.reason : '').toBe('declared');
      if (resolution.scale.status !== 'declared') return;
      expect(Number.isInteger(resolution.scale.sourceScaleExponent)).toBe(true);
      // A conversion declared with no factor is the cohort-14 shape exactly.
      // It is not asserted absent -- these are real markets and one of them is
      // that shape -- but the pair is read, so a later market that declares a
      // factor changes this reading instead of going unnoticed.
      expect(typeof resolution.scale.declaresConversion).toBe('boolean');
    } else {
      expect(resolution.observation).toBeNull();
      expect(resolution.providerEvidenceId).toBeNull();
    }

    const redemption = marketRedemptionStateV1(card);
    expect(redemption.status, redemption.status === 'unread' ? redemption.reason : '').toBe('read');
    if (redemption.status !== 'read') return;
    // Owed, held and redeemed must close, and the word must match the numbers.
    expect(BigInt(redemption.redeemedAtoms)).toBe(
      BigInt(redemption.owedAtoms) > BigInt(redemption.heldAtoms)
        ? BigInt(redemption.owedAtoms) - BigInt(redemption.heldAtoms)
        : 0n,
    );
    expect(redemption.progress).toBe(
      BigInt(redemption.redeemedAtoms) === 0n ? 'none' : BigInt(redemption.heldAtoms) === 0n ? 'complete' : 'partial',
    );
  }, 90_000);
});
