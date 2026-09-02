import { parseQuantityV1, type DenominationV1 } from '@/lib/quantity';
import { type FlowStepIndexV1 } from '@/lib/tradeFlowRefusals';

/**
 * The shape of the trade, derived: which of the seven steps is done, which one
 * you are on, which are open to you, and which are shut with a reason.
 *
 * Pure on purpose. The stepper's whole claim is that a reader can see the
 * WHOLE shape of what they are about to do before they start, and that claim
 * is only as good as the state assignment behind it. Assignment done inline in
 * JSX is assignment nobody can test; here it is one function over one plain
 * record, and its table of cases is the specification.
 *
 * The state machine itself lives in `lib/tradeFlowMachine.ts` and is not
 * touched by this file. This module reads the machine's states; it never
 * decides anything the machine decides.
 */

/**
 * Five states, and `blocked` always carries its reason.
 *
 * `available` and `upcoming` are the distinction that keeps the rail honest:
 * steps 1 and 2 are genuinely independent, so an unstarted flow shows step 2
 * as reachable rather than pretending it is queued behind a wallet connection.
 */
export type StepStatusV1 = 'done' | 'current' | 'available' | 'blocked' | 'upcoming';

export type FlowStepKeyV1 =
  | 'connect' | 'outcome' | 'other-half' | 'size' | 'preview' | 'sign' | 'send';

export type FlowStepV1 = Readonly<{
  index: FlowStepIndexV1;
  key: FlowStepKeyV1;
  /** The step's name on the rail. */
  title: string;
  /** The question the step answers, in the reader's words. */
  question: string;
  status: StepStatusV1;
  /** Present exactly when `status` is `blocked`. */
  blockedReason: string | null;
}>;

/** What the machine's states say about progress, flattened for assignment. */
export type FlowProgressV1 = Readonly<{
  /** `participant.status === 'ready'`. */
  participantReady: boolean;
  /** An outcome index is picked. */
  outcomePicked: boolean;
  /** `inspected.outcomeCount !== null`. */
  outcomeCountKnown: boolean;
  /** `ticketState.kind === 'ready'`. */
  ticketReady: boolean;
  /** The typed size parses, or is blank (which means "all of it"). */
  sizeAccepted: boolean;
  /** `execution.kind === 'ready'`. */
  previewReady: boolean;
  /** Signature A exists: the detached intent was signed. */
  intentSigned: boolean;
  /** Signature B exists: the v0 packet was signed. Still not sent. */
  packetSigned: boolean;
  /** The packet was submitted once and read back finalized. */
  executed: boolean;
  /** The route's payer is somebody else. A first-class outcome, not an error. */
  operatorRequired: boolean;
  /** The `packet` wall's detail, when the measured geometry exceeds the limit. */
  packetWallDetail: string | null;
}>;

const STEP_NAMES_V1: ReadonlyArray<Readonly<{
  index: FlowStepIndexV1; key: FlowStepKeyV1; title: string; question: string;
}>> = Object.freeze([
  Object.freeze({ index: 1 as const, key: 'connect' as const, title: 'Connect', question: 'can you trade here at all?' }),
  Object.freeze({ index: 2 as const, key: 'outcome' as const, title: 'Outcome', question: 'which claim?' }),
  Object.freeze({ index: 3 as const, key: 'other-half' as const, title: 'The other half', question: 'who is on the other side?' }),
  Object.freeze({ index: 4 as const, key: 'size' as const, title: 'Size', question: 'how much?' }),
  Object.freeze({ index: 5 as const, key: 'preview' as const, title: 'Preview', question: 'what exactly happens?' }),
  Object.freeze({ index: 6 as const, key: 'sign' as const, title: 'Sign', question: 'two signatures, and neither one sends' }),
  Object.freeze({ index: 7 as const, key: 'send' as const, title: 'Send', question: 'once, and only once' }),
]);

/**
 * Is this step finished?
 *
 * Step 4 is done when a size DECISION stands, which a blank box already is --
 * blank means "take the ticket in full" and is the commonest correct answer.
 * Step 6 is done when the packet is signed OR when the route named another
 * payer: `operator-required` is a completed signing, not a failed one.
 */
function stepDoneV1(index: FlowStepIndexV1, progress: FlowProgressV1): boolean {
  switch (index) {
    case 1: return progress.participantReady;
    case 2: return progress.outcomePicked;
    case 3: return progress.ticketReady;
    case 4: return progress.ticketReady && progress.sizeAccepted;
    case 5: return progress.previewReady;
    case 6: return progress.packetSigned || progress.operatorRequired;
    case 7: return progress.executed;
  }
}

/**
 * Why this step cannot be worked yet, in the reader's terms and remedy first.
 *
 * A blocked step names the thing that would unblock it. It never says
 * "complete the previous step", which tells a reader only that they are not
 * where they wanted to be.
 */
function stepBlockedReasonV1(index: FlowStepIndexV1, progress: FlowProgressV1): string | null {
  switch (index) {
    case 1:
      return null;
    case 2:
      return progress.outcomeCountKnown
        ? null
        : 'This Market does not expose the Trading program and Product width needed for an exact crossing.';
    case 3:
      return progress.outcomePicked
        ? null
        : 'Pick a claim first — offers are shown one claim at a time.';
    case 4:
      return progress.ticketReady
        ? null
        : 'Hold an offer first — its maker sets the price and the most you can take.';
    case 5:
      if (!progress.ticketReady) return 'Hold an offer first — a preview is priced against one signed ticket.';
      if (!progress.participantReady) return 'Ask the chain about your accounts first — a preview is checked against what you actually hold.';
      return null;
    case 6:
      if (progress.packetWallDetail !== null) return progress.packetWallDetail;
      return progress.previewReady ? null : 'Preview the crossing first — you sign what the preview showed.';
    case 7:
      if (progress.operatorRequired) return null;
      return progress.packetSigned ? null : 'Sign the packet first. Signing is not sending, and this step is the send.';
  }
}

/**
 * The seven steps with their states.
 *
 * `current` is the first step that is neither done nor blocked. Every other
 * unfinished step is `available` when nothing stands in its way and `upcoming`
 * when something does but that something is not a named refusal -- the
 * difference between "you could do this now" and "this comes later", which a
 * single greyed-out treatment collapses and a reader cannot recover.
 */
export function tradeFlowStepsV1(progress: FlowProgressV1): ReadonlyArray<FlowStepV1> {
  const assigned = STEP_NAMES_V1.map((step) => {
    const done = stepDoneV1(step.index, progress);
    const blockedReason = done ? null : stepBlockedReasonV1(step.index, progress);
    return { step, done, blockedReason };
  });
  const currentIndex = assigned.find((entry) => !entry.done && entry.blockedReason === null)?.step.index ?? null;
  return Object.freeze(assigned.map(({ step, done, blockedReason }) => {
    const status: StepStatusV1 = done
      ? 'done'
      : blockedReason !== null
        ? 'blocked'
        : step.index === currentIndex ? 'current' : 'available';
    return Object.freeze({ ...step, status, blockedReason });
  }));
}

/**
 * The three market-level walls, resolved BEFORE a stepper is meaningful.
 *
 * `phase`, `activation` and `release` are facts about the Market, not about
 * the reader or their trade, and no step can move them. Rendering six greyed
 * steps under "this market can never trade" is the flat console in a new
 * costume, so the stepper does not render at all: one card says what is true,
 * and each wall's last clause is the remedy and is kept.
 *
 * The ORDER is the order a reader can act on. `phase` first because a market
 * that is not Open makes the other two moot; `activation` next because it is
 * the one with a deadline; `release` last because it is the furthest from the
 * market itself -- a fact about which execution release has been checked, not
 * about this Market at all.
 *
 * `release` joined them on 2026-09-02, and it is why this gate is worth
 * having: it was reaching a reader at step 5's preview button, after they had
 * picked an outcome, taken a ticket and chosen a size, and it is the one wall
 * of the three that a full-redeploy cohort can never clear.
 */
export type MarketGateV1 =
  | Readonly<{ kind: 'open' }>
  | Readonly<{ kind: 'closed'; wall: string; heading: string; detail: string }>;

const MARKET_GATE_HEADINGS_V1: Readonly<Record<string, string>> = Object.freeze({
  phase: 'This market is not open for trading.',
  activation: 'This market’s Direct trading was founded, but never switched on.',
  release: 'Trading here waits on a checked execution release.',
});

/** The market-level wall names, in the order a reader can act on them. */
export const MARKET_GATE_WALL_ORDER_V1: ReadonlyArray<string> = Object.freeze(['phase', 'activation', 'release']);

export function marketGateV1(
  walls: ReadonlyArray<Readonly<{ name: string; detail: string }>>,
): MarketGateV1 {
  for (const name of MARKET_GATE_WALL_ORDER_V1) {
    const wall = walls.find((candidate) => candidate.name === name);
    if (wall !== undefined) {
      return Object.freeze({
        kind: 'closed' as const,
        wall: wall.name,
        heading: MARKET_GATE_HEADINGS_V1[wall.name]!,
        detail: wall.detail,
      });
    }
  }
  return Object.freeze({ kind: 'open' as const });
}

/**
 * One outcome's share of every claim this Market has issued.
 *
 * Exact and float-free, like everything else that touches a quantity here: the
 * quotient is taken in tenths of a percent as one BigInt division and then
 * split, so no intermediate ever becomes a Number. Truncated rather than
 * rounded, because a share that rounds up to `100.0%` while another outcome
 * still holds claims would be a arithmetic lie in the one place a reader is
 * comparing outcomes against each other.
 *
 * Null when nothing has been issued yet. A share of an empty supply is not
 * zero percent, it is undefined, and rendering `0.0%` against a market that
 * has issued nothing would invent a fact about its outcomes.
 */
export function outcomeShareV1(
  issuedAtoms: bigint | string,
  allIssuedAtoms: ReadonlyArray<bigint | string>,
): string | null {
  const issued = typeof issuedAtoms === 'bigint' ? issuedAtoms : BigInt(issuedAtoms);
  let total = 0n;
  for (const entry of allIssuedAtoms) total += typeof entry === 'bigint' ? entry : BigInt(entry);
  if (total <= 0n) return null;
  const tenths = (issued * 1_000n) / total;
  return `${(tenths / 10n).toString()}.${(tenths % 10n).toString()}%`;
}

/**
 * Whether the size the reader typed is a size at all, checked as they type.
 *
 * Blank is accepted because blank is the ticket's own `maximumFill`, which is
 * what the placeholder promises. This runs the SAME parser the machine runs --
 * `parseQuantityV1` -- so the answer here and the answer at preview time can
 * never disagree, and the reader learns about a bad size at step 4 instead of
 * discovering it behind a preview button at step 5.
 */
export function sizeDecisionV1(
  text: string,
  denomination: DenominationV1,
): Readonly<{ ok: true }> | Readonly<{ ok: false; reason: string }> {
  if (text.trim() === '') return Object.freeze({ ok: true as const });
  try {
    parseQuantityV1(text, denomination);
    return Object.freeze({ ok: true as const });
  } catch (error) {
    return Object.freeze({
      ok: false as const,
      reason: error instanceof Error ? error.message : 'your size must be one positive amount of claims',
    });
  }
}
