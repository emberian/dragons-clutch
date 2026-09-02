import { describe, expect, it } from 'vitest';

import {
  MARKET_GATE_WALL_ORDER_V1, marketGateV1, outcomeShareV1, sizeDecisionV1, tradeFlowStepsV1, type FlowProgressV1,
} from './tradeFlowSteps';
import { type DenominationV1 } from './quantity';

const NOTHING_DONE_V1: FlowProgressV1 = Object.freeze({
  participantReady: false,
  outcomePicked: false,
  outcomeCountKnown: true,
  ticketReady: false,
  sizeAccepted: true,
  previewReady: false,
  intentSigned: false,
  packetSigned: false,
  executed: false,
  operatorRequired: false,
  packetWallDetail: null,
});

const SIX_DECIMALS_V1: DenominationV1 = Object.freeze({ decimals: 6, unit: 'USDC', mint: 'mint' });
const UNREAD_V1: DenominationV1 = Object.freeze({ decimals: null, unit: null, mint: 'mint' });

const statuses = (progress: FlowProgressV1) => tradeFlowStepsV1(progress).map((step) => step.status);

describe('the seven steps, and which one you are on', () => {
  it('always renders all seven, in order, whatever the state', () => {
    expect(tradeFlowStepsV1(NOTHING_DONE_V1).map((step) => step.key)).toEqual([
      'connect', 'outcome', 'other-half', 'size', 'preview', 'sign', 'send',
    ]);
  });

  /**
   * Steps 1 and 2 are genuinely independent -- you can pick a claim before you
   * connect anything. A rail that showed step 2 as queued behind step 1 would
   * be inventing an order the flow does not have.
   */
  it('opens step 2 alongside step 1 rather than queueing it behind a wallet', () => {
    const [connect, outcome] = statuses(NOTHING_DONE_V1);
    expect(connect).toBe('current');
    expect(outcome).toBe('available');
  });

  it('blocks step 3 on a claim, because the board shows one claim at a time', () => {
    const steps = tradeFlowStepsV1(NOTHING_DONE_V1);
    expect(steps[2]!.status).toBe('blocked');
    expect(steps[2]!.blockedReason).toContain('Pick a claim first');
  });

  /**
   * `current` is the FIRST unfinished step that nothing blocks, so it stays on
   * step 1 while a wallet is unconnected even after a claim is picked. That is
   * the honest reading: an unconnected reader who picked a claim really does
   * still have step 1 in front of them, and step 3 is open rather than next.
   */
  it('moves the current step forward as each one is finished', () => {
    const connected = { ...NOTHING_DONE_V1, participantReady: true };
    expect(statuses(connected)[1]).toBe('current');
    const picked = { ...connected, outcomePicked: true };
    expect(statuses(picked)[2]).toBe('current');
    const held = { ...picked, ticketReady: true };
    // Step 4 is done the moment a ticket is held, because a blank size box
    // already IS a decision: it means "take the offer in full".
    expect(statuses(held)[3]).toBe('done');
    expect(statuses(held)[4]).toBe('current');
    expect(statuses({ ...held, previewReady: true })[5]).toBe('current');
  });

  it('keeps step 3 open, not next, while step 1 is still unfinished', () => {
    const picked = { ...NOTHING_DONE_V1, outcomePicked: true };
    expect(statuses(picked)[0]).toBe('current');
    expect(statuses(picked)[2]).toBe('available');
  });

  it('un-does step 4 when the typed size stops being a size', () => {
    const held = {
      ...NOTHING_DONE_V1,
      participantReady: true, outcomePicked: true, ticketReady: true, sizeAccepted: false,
    };
    expect(statuses(held)[3]).toBe('current');
  });

  /**
   * `operator-required` finishes step 6. The trader signed their intent and
   * holds a real portable ticket; the route's payer is somebody else. Marking
   * that step unfinished would tell a reader their signature did not count.
   */
  it('treats operator-required as a finished signing, and opens the send step', () => {
    const steps = tradeFlowStepsV1({
      ...NOTHING_DONE_V1,
      participantReady: true, outcomePicked: true, ticketReady: true,
      previewReady: true, intentSigned: true, operatorRequired: true,
    });
    expect(steps[5]!.status).toBe('done');
    expect(steps[6]!.status).toBe('current');
    expect(steps[6]!.blockedReason).toBeNull();
  });

  it('blocks the send step on a signature, and says signing is not sending', () => {
    const steps = tradeFlowStepsV1({
      ...NOTHING_DONE_V1,
      participantReady: true, outcomePicked: true, ticketReady: true, previewReady: true,
    });
    expect(steps[6]!.status).toBe('blocked');
    expect(steps[6]!.blockedReason).toContain('Signing is not sending');
  });

  it('blocks the sign step with the packet wall when the geometry is too big', () => {
    const steps = tradeFlowStepsV1({
      ...NOTHING_DONE_V1,
      participantReady: true, outcomePicked: true, ticketReady: true, previewReady: true,
      packetWallDetail: 'Your measured Direct transaction is 1,400 bytes, above the network’s 1,232-byte limit. Reduce its account or instruction geometry before signing.',
    });
    expect(steps[5]!.status).toBe('blocked');
    expect(steps[5]!.blockedReason).toContain('1,232-byte limit');
  });

  it('names one current step at most, and never one that is blocked', () => {
    for (const progress of [
      NOTHING_DONE_V1,
      { ...NOTHING_DONE_V1, outcomePicked: true },
      { ...NOTHING_DONE_V1, outcomeCountKnown: false },
      { ...NOTHING_DONE_V1, participantReady: true, outcomePicked: true, ticketReady: true, previewReady: true, packetSigned: true, intentSigned: true },
    ]) {
      const steps = tradeFlowStepsV1(progress);
      expect(steps.filter((step) => step.status === 'current').length).toBeLessThanOrEqual(1);
      for (const step of steps) {
        expect(step.blockedReason === null).toBe(step.status !== 'blocked');
      }
    }
  });
});

describe('the gate that stands instead of the stepper', () => {
  it('closes on a market that is not Open, and names the phase wall', () => {
    const gate = marketGateV1([{ name: 'phase', detail: 'this Market is Retired — trading is only open while a Market is Open' }]);
    expect(gate.kind).toBe('closed');
    expect(gate.kind === 'closed' && gate.wall).toBe('phase');
    expect(gate.kind === 'closed' && gate.detail).toContain('only open while a Market is Open');
  });

  it('closes on an unactivated capability and keeps the operator clause', () => {
    const detail = 'this Market founded a Direct trading capability but never switched it on — no activation root exists at Root111. Activation is the operator’s move, not yours.';
    const gate = marketGateV1([{ name: 'activation', detail }]);
    expect(gate.kind === 'closed' && gate.detail).toBe(detail);
    // The last clause IS the remedy: it tells a reader the wait is not theirs
    // to end, which is the whole reason this is a card and not a shrug.
    expect(gate.kind === 'closed' && gate.detail).toContain('Activation is the operator’s move, not yours.');
  });

  it('closes on a missing checked execution release, and says joining is unaffected', () => {
    const detail = 'no checked execution release is on file for this Market’s execution release set aa11 — so a Direct fill refuses at the route admission boundary.';
    const gate = marketGateV1([{ name: 'release', detail }]);
    expect(gate.kind === 'closed' && gate.wall).toBe('release');
    expect(gate.kind === 'closed' && gate.heading).toContain('checked execution release');
    expect(gate.kind === 'closed' && gate.detail).toBe(detail);
  });

  /**
   * The order is the order a reader can act on, and it is pinned because the
   * three walls answer different questions and the gate shows exactly one.
   * A market that is Retired AND has no checked release must lead with the
   * phase: the release is moot on a market that cannot trade at all.
   */
  it('shows the wall a reader can act on first, when more than one stands', () => {
    const walls = [
      { name: 'release', detail: 'no checked execution release is on file' },
      { name: 'activation', detail: 'never switched on' },
      { name: 'phase', detail: 'this Market is Retired' },
    ];
    const first = marketGateV1(walls);
    expect(first.kind === 'closed' && first.wall).toBe('phase');
    const second = marketGateV1(walls.slice(0, 2));
    expect(second.kind === 'closed' && second.wall).toBe('activation');
    expect(MARKET_GATE_WALL_ORDER_V1).toEqual(['phase', 'activation', 'release']);
  });

  /**
   * `prestate` and `packet` belong to steps 1 and 6. If the gate swallowed
   * them, a reader whose only problem was their own Position would be told the
   * market cannot trade -- which is false, and unrecoverable from.
   */
  it('stays open for the two walls that belong to steps', () => {
    expect(marketGateV1([{ name: 'prestate', detail: 'no Position' }]).kind).toBe('open');
    expect(marketGateV1([{ name: 'packet', detail: 'too big' }]).kind).toBe('open');
    expect(marketGateV1([]).kind).toBe('open');
  });
});

describe('one outcome’s share of every claim issued', () => {
  it('divides exactly, in tenths of a percent, with no float anywhere', () => {
    expect(outcomeShareV1(1n, [1n, 1n])).toBe('50.0%');
    expect(outcomeShareV1(1n, [1n, 2n])).toBe('33.3%');
    expect(outcomeShareV1(2n, [1n, 2n])).toBe('66.6%');
    expect(outcomeShareV1('750', ['750', '250'])).toBe('75.0%');
  });

  /**
   * Truncated, never rounded. Two thirds is 66.6%, not 66.7%: a share that
   * rounds up while another outcome still holds claims is an arithmetic lie
   * in the one place a reader compares outcomes against each other.
   */
  it('truncates rather than rounding, so shares never overstate', () => {
    expect(outcomeShareV1(2n, [1n, 2n])).toBe('66.6%');
    expect(outcomeShareV1(999_999n, [999_999n, 1n])).toBe('99.9%');
  });

  it('stays exact across quantities far beyond a float’s reach', () => {
    const huge = 10n ** 30n;
    expect(outcomeShareV1(huge, [huge, huge, huge, huge])).toBe('25.0%');
  });

  /**
   * A share of nothing is undefined, not zero. Rendering `0.0%` against a
   * market that has issued nothing would invent a fact about its outcomes.
   */
  it('refuses to name a share when nothing has been issued', () => {
    expect(outcomeShareV1(0n, [0n, 0n])).toBeNull();
    expect(outcomeShareV1(0n, [])).toBeNull();
  });

  it('says zero for an outcome nobody holds, when others do', () => {
    expect(outcomeShareV1(0n, [0n, 100n])).toBe('0.0%');
  });
});

describe('the size decision, checked as it is typed', () => {
  it('accepts a blank box, which is what "all of it" is', () => {
    expect(sizeDecisionV1('', SIX_DECIMALS_V1).ok).toBe(true);
    expect(sizeDecisionV1('   ', SIX_DECIMALS_V1).ok).toBe(true);
  });

  it('accepts a display quantity and its grouping separators', () => {
    expect(sizeDecisionV1('500', SIX_DECIMALS_V1).ok).toBe(true);
    expect(sizeDecisionV1('1,250.5', SIX_DECIMALS_V1).ok).toBe(true);
  });

  /**
   * The same parser the machine runs, so step 4 and step 5 can never disagree
   * about whether a size is a size.
   */
  it('refuses in the parser’s own words, before a preview is ever asked for', () => {
    const negative = sizeDecisionV1('-1', SIX_DECIMALS_V1);
    expect(negative.ok).toBe(false);
    expect(!negative.ok && negative.reason).toContain('one positive amount of claims');

    const tooFine = sizeDecisionV1('1.0000001', SIX_DECIMALS_V1);
    expect(!tooFine.ok && tooFine.reason).toContain('finer than one claim atom');

    const fractionalUnread = sizeDecisionV1('1.5', UNREAD_V1);
    expect(!fractionalUnread.ok && fractionalUnread.reason).toContain('never published a display precision');
  });
});
