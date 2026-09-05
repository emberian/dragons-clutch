import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import MarketTradePanel from './MarketTradePanel';
import {
  DIRECT_PACKET_BUDGET_EVIDENCE_V1,
  DIRECT_PRESTATE_WALL_V1,
  directPacketWallV1,
} from '@dclutch/sdk/directTradeSpine';

describe('the market-detail trade panel', () => {
  const html = renderToStaticMarkup(<MarketTradePanel
    endpoint="http://127.0.0.1:8899"
    marketAddress="4fQNy8k7G7bZ9cak6pb2VnigV2F5fbhs7YnYFWQ2LQYH"
    coreProgramId=""
    registryProgramId={null}
    claimsProgramId={null}
    tradingProgramId={null}
    custodyProgramId={null}
    rentProgramId={null}
    liability={null}
    denomination={{ decimals: null, unit: null, mint: '' }}
    outcomes={null}
    clock={null}
    nowMs={null}
  />);

  it('treats the named refusal as the product surface, not a disabled button', () => {
    // Renegotiated 2026-08-31: the header used to promise that this panel
    // says why in one sentence and never shows a greyed-out button with no
    // reason. That is a promise about the panel; the panel keeps it below,
    // where the named refusals actually render, so the promise is deleted.
    expect(html).toContain('Pick an outcome, choose how much');
    expect(html).not.toContain('greyed-out');
  });

  it('carries the measured packet margin and the remaining prestate wall as exact facts', () => {
    expect(DIRECT_PACKET_BUDGET_EVIDENCE_V1).toEqual({
      wireBytes: 1_204, packetLimit: 1_232, marginBytes: 28, computeUnitLimit: 1_400_000,
    });
    expect(directPacketWallV1(DIRECT_PACKET_BUDGET_EVIDENCE_V1.wireBytes)).toBeNull();
    expect(directPacketWallV1(1_233)?.name).toBe('packet');
    expect(DIRECT_PRESTATE_WALL_V1.name).toBe('prestate');
  });

  it('starts from an honest empty state', () => {
    expect(html).toContain('The chain has not been asked about trading this Market yet.');
    expect(html).toContain('Ask the chain about trading here');
  });

  /**
   * The workbench link is RELOCATED, not weakened -- the same treatment this
   * file already gives the resumption promise below.
   *
   * It sat in this footer beside "See this market in the explorer", as though
   * driving a route by hand and looking at what a market is connected to were
   * the same kind of offer to the same reader. It now lives in the market
   * page's "For operators and auditors" region, and
   * `MarketDetailWorkspace.test.tsx` holds it there. This assertion exists so
   * that "it moved" can never quietly become "it went".
   */
  it('leaves the advanced workbench to the operator fold, and keeps the explorer', () => {
    expect(html).not.toContain('Advanced: full route workbench');
    expect(html).toContain('See this market in the explorer');
  });

  /**
   * THE VERDICT COMES FIRST. The re-read control is a control, not an answer:
   * it used to stand in the body above the wall it produces, so the first thing
   * a market that cannot trade offered was a button inviting a reader to ask
   * about trading it. It is in the header now, where every other section on
   * this page puts its re-read, and the body opens on what the chain said.
   */
  it('puts the re-read in the header and opens the body with what the chain said', () => {
    const header = html.slice(0, html.indexOf('</header>'));
    expect(header).toContain('Ask the chain about trading here');
    const body = html.slice(html.indexOf('</header>'));
    expect(body).not.toContain('<button');
  });

  it('never invents market-data metrics on a trading surface', () => {
    for (const forbidden of ['volume', 'Volume', 'odds', 'probability', 'TVL', 'APR', 'APY', '$', '24h', 'P&L']) {
      expect(html).not.toContain(forbidden);
    }
  });

  /**
   * FLOWFUL, 2026-08-31: the stepper does not exist until the chain has been
   * asked.
   *
   * This is not a loading nicety. Two of the four named walls are MARKET-level
   * -- `phase` and `activation` -- and until those have been read, rendering
   * seven steps would be promising a flow that may turn out to be impossible
   * on this market. The gate replaces the stepper in that case, so neither can
   * be drawn before the read that decides between them.
   *
   * The signing vocabulary follows from the same fact rather than being a
   * separate rule: every control that says `Sign` or `Submit` lives inside a
   * step, so an unread panel offers no signing surface at all.
   */
  it('shows no stepper, and no signing vocabulary, before the chain has been asked', () => {
    expect(html).not.toContain('flow-rail');
    expect(html).not.toContain('flow-step');
    expect(html).not.toContain('Sign');
    expect(html).not.toContain('Submit');
  });

  // This panel used to promise "There is no submit button here", and this test
  // pinned that sentence. Then submission shipped and the sentence stayed --
  // green, and untrue to every reader, on a public page. The assertions below
  // therefore pin the guarantees the panel actually keeps, and refuse the old
  // sentence by name so it cannot come back once it is no longer true.
  it('describes the submission it really performs, and the guarantees around it', () => {
    expect(html).not.toContain('There is no submit button here');
    expect(html).not.toContain('signed packet is never described as an executed trade');
    // Renegotiated 2026-08-31 (FLOWFUL). The resumption promise -- "Signing
    // sends nothing … rather than sending a second one" -- used to render
    // here, above everything, to a reader who did not yet know what signing or
    // sending were. It now renders in step 6's header, one step away from
    // being true, and it is pinned in `trade/steps/SignStep.test.tsx` whole
    // and unsplit. The promise is RELOCATED, not weakened: this assertion
    // exists so that "it moved" can never quietly become "it went".
    expect(html).not.toContain('Signing sends nothing.');
    // Renegotiated 2026-08-31. Three paragraphs of guarantees stood above the
    // controls: what gets re-read before signing, who pays, what "finalized"
    // means, and that everything on the page is a copy. What a reader needs
    // there is the one thing that changes what they should DO -- signing is
    // not sending, and sending happens once. The rest is deleted.
    expect(html).not.toContain('everything on this page is a copy');
    expect(html).not.toContain('re-reads both sides');
    expect(html).not.toContain('the programs on chain are what is true');
    expect(html).not.toContain('taker collateral account and Position derive under it');
    expect(html).not.toContain('Build, sign as payer, and submit');
  });
});
