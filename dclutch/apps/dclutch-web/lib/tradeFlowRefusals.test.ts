import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';

import { assignRefusalV1, routedRefusalFragmentsV1 } from './tradeFlowRefusals';
import { DIRECT_PRESTATE_WALL_V1, directPacketWallV1 } from './directTradeSpine';

/**
 * The routing table's job is to send a refusal to the step that can act on it.
 * There are two ways for that to be quietly wrong, and this file exists to
 * catch both.
 *
 * The first is DRIFT: the table matches on fragments of strings that live in
 * other modules, so a reworded refusal silently stops matching and starts
 * falling back. Every fragment is therefore checked against the actual source
 * of the module that raises it -- if a message is reworded, this test fails
 * where the rewording happened rather than a reader finding out.
 *
 * The second is a table that LOOKS full and does nothing, because everything
 * routes to the step that raised it anyway. So the cases below assert the
 * specific step, and the ones that matter most are the refusals raised by one
 * step that belong to another.
 */

/** The modules that actually produce the refusals this table routes. */
const REFUSAL_SOURCES_V1 = [
  'tradeFlowMachine.ts',
  'quantity.ts',
  'directTradeSpine.ts',
  'directTicket.ts',
  'directParticipant.ts',
].map((name) => readFileSync(new URL(`./${name}`, import.meta.url), 'utf8')).join('\n');

describe('the refusal routing table', () => {
  it('matches on fragments that really exist in the modules that raise them', () => {
    const missing = routedRefusalFragmentsV1().filter((fragment) => !REFUSAL_SOURCES_V1.includes(fragment));
    // A fragment that matches nothing is a route to nowhere: it can never
    // fire, and the refusal it was meant to catch falls back forever.
    expect(missing).toEqual([]);
  });

  it('routes each of the seven steps at least one refusal it owns', () => {
    const owned = new Set(routedRefusalFragmentsV1().map(
      (fragment) => assignRefusalV1(fragment, 5).step,
    ));
    expect([...owned].sort()).toEqual([1, 2, 3, 4, 5, 6, 7]);
  });

  it('always leads with a remedy and never edits the refusal it carries', () => {
    for (const fragment of routedRefusalFragmentsV1()) {
      const routed = assignRefusalV1(fragment, 5);
      expect(routed.detail).toBe(fragment);
      expect(routed.remedy.length).toBeGreaterThan(0);
      expect(routed.remedy).not.toBe(routed.detail);
    }
  });

  /**
   * The cases the whole module is for. Every one of these is raised inside
   * `previewIntent` or `prepareWalletIntent` -- two functions, two state
   * slots -- and every one of them belongs to a different step than the one
   * whose button was pressed.
   */
  it('sends a refusal to the step that can act on it, not the one that raised it', () => {
    // previewIntent raises all four of these. None of them is step 5's.
    expect(assignRefusalV1('Ask the chain to authenticate your participant accounts before previewing a crossing.', 5).step).toBe(1);
    expect(assignRefusalV1('Pick the claim you intend to trade before previewing the ticket.', 5).step).toBe(2);
    expect(assignRefusalV1('ticket expired at slot 490712003', 5).step).toBe(3);
    expect(assignRefusalV1('no admissible fill exists at or below the requested size at this exact price scale', 5).step).toBe(4);
    // And the one that IS step 5's: the claim you picked is not the claim the
    // maker signed for, which only the preview can notice.
    expect(assignRefusalV1('You picked claim 0, but this ticket is signed for claim 1.', 5).step).toBe(5);

    // prepareWalletIntent raises these. The buy-side one is step 3's: the
    // route is fine, the ticket is the wrong kind, and telling a reader to
    // fix their route manifest would send them to repair something correct.
    expect(assignRefusalV1('Wallet preparation V1 accepts a portable sell ticket and your connected wallet as buyer. This buy ticket remains a valid read-only preview, but this caller will not silently reverse its participant roles.', 6).step).toBe(3);
    expect(assignRefusalV1('the ticket seller’s finalized Position does not cover this fill', 6).step).toBe(3);
    expect(assignRefusalV1('the connected wallet is the ticket maker; a Direct fill needs two distinct makers', 6).step).toBe(3);
    // These really are step 6's.
    expect(assignRefusalV1('Paste the operator-published Direct Hot route manifest before asking your wallet to sign.', 6).step).toBe(6);
    expect(assignRefusalV1('route manifest authenticates another Market or Trading program', 6).step).toBe(6);
    expect(assignRefusalV1('both participants must be ready before signing: seller is incomplete; you are ready', 6).step).toBe(6);
  });

  /**
   * Two refusals that both end in "it must not be submitted" and belong to
   * different steps. An earlier draft of the table matched the shared tail and
   * sent the send-time one to step 6, which would have told a reader to
   * re-sign a packet whose problem was the endpoint they were connected to.
   */
  it('separates the two expiry refusals that share a tail', () => {
    expect(assignRefusalV1('signed Direct packet expired at block height 4001; it must not be submitted', 6).step).toBe(6);
    expect(assignRefusalV1('RPC genesis changed after the packet was signed; it must not be submitted here', 7).step).toBe(7);
    expect(assignRefusalV1('the signed packet expired at block height 4001; the chain can no longer include it', 7).step).toBe(7);
  });

  it('routes both named walls the flow owns, and keeps their own words whole', () => {
    const prestate = assignRefusalV1(DIRECT_PRESTATE_WALL_V1.detail, 5);
    expect(prestate.step).toBe(1);
    expect(prestate.detail).toBe(DIRECT_PRESTATE_WALL_V1.detail);
    // The remedy in the wall's own second sentence survives verbatim: a devnet
    // admission command exists and this page will not pretend to run it.
    expect(prestate.detail).toContain('this public page does not create or sign one');

    const packet = directPacketWallV1(1_400)!;
    const routedPacket = assignRefusalV1(packet.detail, 1);
    expect(routedPacket.step).toBe(6);
    expect(routedPacket.detail).toBe(packet.detail);
  });

  it('shows an unrecognised refusal where it happened rather than swallowing it', () => {
    const unknown = assignRefusalV1('a refusal nothing in the table has ever seen', 4);
    expect(unknown.step).toBe(4);
    expect(unknown.routed).toBe(false);
    expect(unknown.detail).toBe('a refusal nothing in the table has ever seen');
  });
});
