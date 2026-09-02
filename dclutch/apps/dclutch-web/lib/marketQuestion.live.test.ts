import { describe, expect, it } from 'vitest';

import { DEVNET_DEPLOYMENT_V1 } from './deployments';
import { inspectMarketDetailV1 } from './marketDetail';
import { formatWindowInstantV1, inspectMarketQuestionV1 } from './marketQuestion';
import { marketEditorialV1, marketNarrativeV1 } from './marketRegistry';
import { PUBLIC_DEVNET_CUT_V1 } from './publicCutStaging';
import { SolanaRpcClient } from './rpc';

const featured = PUBLIC_DEVNET_CUT_V1.market;
const live = process.env.DCLUTCH_LIVE_DEVNET === '1' && featured !== null ? it : it.skip;

/**
 * The market page's question, read off the live market rather than typed.
 *
 * The case this exists for is the one the editorial registry kept losing: a
 * market on the live deployment that the registry does not know. Every redeploy
 * has produced one, and until this derivation existed such a market rendered as
 * `Unnamed · <address>`, "Outcomes 4", no question and "No settlement time is
 * published" — while the accounts the same page had already read carried the
 * cuts, the denominator, the outcome width and the window.
 *
 * So the assertion is deliberately made with the registry SWITCHED OFF: the
 * narrative is built from a null editorial entry, and it must still produce a
 * title, a question and named outcomes. What it may not produce is a coordinate
 * name, because the chain genuinely does not carry one.
 *
 * Gated on `DCLUTCH_LIVE_DEVNET=1`. Reads only.
 */
describe('live devnet market question', () => {
  live('derives the featured market’s partition and window with no registry row at all', async () => {
    const client = new SolanaRpcClient(process.env.DCLUTCH_LIVE_ENDPOINT ?? DEVNET_DEPLOYMENT_V1.endpoint);
    const detail = await inspectMarketDetailV1(client, {
      coreProgramId: DEVNET_DEPLOYMENT_V1.programs.core,
      registryProgramId: DEVNET_DEPLOYMENT_V1.programs.registry,
      claimsProgramId: DEVNET_DEPLOYMENT_V1.programs.claims,
      custodyProgramId: DEVNET_DEPLOYMENT_V1.programs.custody,
      address: featured!,
    });
    expect(detail.card.status, detail.reason).toBe('decoded');
    if (detail.card.status !== 'decoded') return;

    const derived = await inspectMarketQuestionV1(client, {
      registryProgramId: DEVNET_DEPLOYMENT_V1.programs.registry,
      address: featured!,
      productRecordId: detail.card.identity.productRecordId,
      resolutionPolicyId: detail.card.identity.resolutionPolicyId,
    });

    // Shape, not values: the cuts are this market's own and a later cohort's
    // will differ, so what is pinned is that a real partition came back and
    // that its three widths agree with each other and with the Market account.
    expect(derived.cuts.length).toBeGreaterThan(0);
    expect(derived.cutDenominator).toBeGreaterThan(0n);
    expect(derived.regionCount).toBe(derived.cuts.length + 1);
    expect(derived.outcomeCount).toBe(derived.regionCount + 1);
    expect(derived.outcomeCount).toBe(detail.card.liability.status === 'bound'
      ? detail.card.liability.supplyAtoms.length
      : derived.outcomeCount);
    for (let index = 1; index < derived.cuts.length; index += 1) {
      expect(derived.cuts[index]! > derived.cuts[index - 1]!).toBe(true);
    }

    // The window: the fact the page told every reader was not published.
    expect(derived.windowRefusal).toBeNull();
    expect(derived.window).not.toBeNull();
    expect(derived.window!.endUnixSeconds >= derived.window!.startUnixSeconds).toBe(true);
    expect(formatWindowInstantV1(derived.window!.endUnixSeconds)).toMatch(/^\d{4}-\d{2}-\d{2} \d{2}:\d{2} UTC$/);

    // With no registry row, the page still says what the market asks.
    const silent = marketNarrativeV1(featured!, detail.card.phase, null, derived);
    expect(silent.titleSource).toBe('chain');
    expect(silent.questionSource).toBe('chain');
    expect(silent.outcomeSource).toBe('chain');
    expect(silent.title).not.toContain('Unnamed');
    expect(silent.question).toContain('Where does');
    expect(silent.outcomes).toHaveLength(derived.outcomeCount);
    expect(silent.outcomes![derived.outcomeCount - 1]).toBe('The source failed to report');
    // Every boundary in the derived text is a decimal off the chain, so a raw
    // tick count reaching the page would mean the denominator was ignored.
    expect(silent.question).not.toContain(derived.cuts[0]!.toString());

    // And the shipped row supplies exactly the missing half: a coordinate name
    // the wire has no word for, and nothing that restates a number.
    const shipped = marketEditorialV1(featured!);
    expect(shipped?.coordinate?.label, 'the featured market names its coordinate').toBeTruthy();
    expect(shipped?.question, 'the featured row must not restate a derivable question').toBeNull();
    expect(shipped?.outcomes, 'the featured row must not restate derivable outcome names').toBeNull();
    const named = marketNarrativeV1(featured!, detail.card.phase, shipped, derived);
    expect(named.question).toContain(shipped!.coordinate!.label);
    expect(named.outcomes![0]).toContain(shipped!.coordinate!.unitPrefix!);
  }, 120_000);
});
