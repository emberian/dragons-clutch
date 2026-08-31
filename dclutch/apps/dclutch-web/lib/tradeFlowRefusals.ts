/**
 * Every named refusal in the Direct trade flow, assigned to the step that owns
 * it, remedy first.
 *
 * The flow's refusals are produced in three places -- the ticket decoder, the
 * quantity parser, and the eight orchestration functions in
 * `lib/tradeFlowMachine.ts` -- and the machine reports them through exactly two
 * state slots, `execution.refused` and `walletPreparation.refused`. Those two
 * slots are NOT two steps. `previewIntent` alone can refuse because the wallet
 * has no Position (step 1), because no claim is picked (step 2), because the
 * ticket expired (step 3), because the size is not representable (step 4), or
 * because the picked claim is not the ticket's claim (step 5). Rendering all of
 * them under "preview" is the flat console that this flow exists to replace: it
 * tells a reader that something is wrong somewhere behind them.
 *
 * So this module is a routing table from a refusal's own words to the step that
 * can act on it. It is deliberately pure and deliberately data: a refusal is
 * matched on a fragment of the message the machine already produces, and the
 * machine's conditions are not touched to carry a code. Adding a code would
 * mean editing `tradeFlowMachine.ts`, and a diff there means the extraction
 * went wrong.
 *
 * **The remedy comes first and the refusal survives whole.** `remedy` is one
 * imperative sentence saying what the reader can do next; `detail` is the
 * message exactly as it was thrown, never paraphrased, never truncated, and
 * never split across two elements. A refusal that loses its own words has been
 * turned into a mood.
 */

/** The seven steps, by their position in the flow. */
export type FlowStepIndexV1 = 1 | 2 | 3 | 4 | 5 | 6 | 7;

/** One refusal, routed to its owner and given a remedy. */
export type StepRefusalV1 = Readonly<{
  /** The step that can act on this refusal. */
  step: FlowStepIndexV1;
  /** What the reader can do, in one imperative sentence. Rendered first. */
  remedy: string;
  /** The refusal, exactly as it was produced. Rendered whole, in one element. */
  detail: string;
  /** True when a fragment matched; false when the raising step kept it. */
  routed: boolean;
}>;

/**
 * The routing table.
 *
 * Fragments are matched with `includes`, in order, so the more specific entry
 * is listed first. Each fragment is chosen to be stable against the parts of a
 * message that interpolate (slots, byte counts, addresses, claim indices) and
 * to avoid the typographic apostrophes the source strings carry -- matching on
 * `protocol's` would silently never fire against `protocol’s`.
 */
const REFUSAL_OWNERS_V1: ReadonlyArray<Readonly<{
  fragment: string;
  step: FlowStepIndexV1;
  remedy: string;
}>> = Object.freeze([
  // ---- Step 1, Connect: who you are, and whether the chain knows it. -------
  Object.freeze({
    fragment: 'does not have a Claims Position on this Market yet',
    step: 1 as const,
    remedy: 'This wallet needs a Claims Position on this Market before it can trade. This page cannot create or sign one for you.',
  }),
  Object.freeze({
    fragment: 'authenticate your participant accounts before previewing',
    step: 1 as const,
    remedy: 'Ask the chain about your accounts first, then come back to the preview.',
  }),
  Object.freeze({
    fragment: 'name every program needed to authenticate your participant accounts',
    step: 1 as const,
    remedy: 'Pick a cluster whose deployment names the full Direct program set.',
  }),
  Object.freeze({
    fragment: 'Your wallet changed',
    step: 1 as const,
    remedy: 'Ask the chain again so it can read the accounts of the wallet that is connected now.',
  }),
  Object.freeze({
    fragment: 'connect a browser wallet',
    step: 1 as const,
    remedy: 'Connect a wallet: a crossing is signed against one connected identity.',
  }),
  Object.freeze({
    fragment: 'select the Claims program',
    step: 1 as const,
    remedy: 'Pick a cluster that names the Claims program, then ask the chain again.',
  }),
  Object.freeze({
    fragment: 'the Registry program is required',
    step: 1 as const,
    remedy: 'Pick a cluster that names the Registry program: the Direct capability is a Registry-finalized record.',
  }),

  // ---- Step 2, Outcome: which claim. --------------------------------------
  Object.freeze({
    fragment: 'Product width needed for an exact crossing',
    step: 2 as const,
    remedy: 'No claim can be chosen here: this Market never exposed the Trading program and Product width a crossing needs.',
  }),
  Object.freeze({
    fragment: 'Pick the claim you intend to trade',
    step: 2 as const,
    remedy: 'Choose one claim above before previewing.',
  }),

  // ---- Step 3, The other half: the counterparty's ticket. -----------------
  Object.freeze({
    fragment: 'finalized Position does not cover this fill',
    step: 3 as const,
    remedy: 'Take a different offer: this maker no longer holds enough claims to settle the one you picked.',
  }),
  Object.freeze({
    fragment: 'a Direct fill needs two distinct makers',
    step: 3 as const,
    remedy: 'Take an offer from a different maker: this ticket is signed by the wallet you are connected with.',
  }),
  Object.freeze({
    fragment: 'ticket expired at slot',
    step: 3 as const,
    remedy: 'Take a different offer: this one passed its own deadline before you got here.',
  }),
  Object.freeze({
    fragment: 'ticket becomes valid at slot',
    step: 3 as const,
    remedy: 'Wait for this offer to open, or take a different one.',
  }),
  Object.freeze({
    fragment: 'will not silently reverse its participant roles',
    step: 3 as const,
    remedy: 'Take a sell offer instead: this build can buy from a maker, and it will not quietly swap the two sides to make a buy ticket fit.',
  }),
  Object.freeze({
    fragment: 'ticket is not valid JSON',
    step: 3 as const,
    remedy: 'Paste the whole ticket file, including its outermost braces.',
  }),
  Object.freeze({
    fragment: 'ticket kind is not',
    step: 3 as const,
    remedy: 'Paste a Direct intent ticket: this text is a different kind of document.',
  }),
  Object.freeze({
    fragment: 'ticket signature must be one nonzero',
    step: 3 as const,
    remedy: 'Ask the maker for the ticket again: its signature field is not one 64-byte Ed25519 signature.',
  }),
  Object.freeze({
    fragment: 'ticket text is empty or above',
    step: 3 as const,
    remedy: 'Paste one ticket on its own: this text is empty, or larger than a ticket can be.',
  }),

  // ---- Step 4, Size: how much. -------------------------------------------
  Object.freeze({
    fragment: 'no admissible fill exists at or below the requested size',
    step: 4 as const,
    remedy: 'Ask for a size the price scale can settle exactly -- the nearest admissible one is shown beside the input.',
  }),
  Object.freeze({
    fragment: 'fill-or-kill for exactly',
    step: 4 as const,
    remedy: 'Take this offer in full: its maker signed it all-or-nothing.',
  }),
  Object.freeze({
    fragment: 'u64 amount width',
    step: 4 as const,
    remedy: 'Ask for a smaller size: this one is wider than the protocol amount field.',
  }),
  Object.freeze({
    fragment: 'finer than one claim atom',
    step: 4 as const,
    remedy: 'Round your size up to this market’s smallest tradeable unit.',
  }),
  Object.freeze({
    fragment: 'never published a display precision',
    step: 4 as const,
    remedy: 'Type a whole number: this collateral mint never published the precision a fractional size would need.',
  }),
  Object.freeze({
    fragment: 'your size must be one positive',
    step: 4 as const,
    remedy: 'Type one positive number of claims, or leave the box blank to take the offer in full.',
  }),

  // ---- Step 5, Preview: what exactly happens. -----------------------------
  Object.freeze({
    fragment: 'but this ticket is signed for claim',
    step: 5 as const,
    remedy: 'Pick the claim this ticket is signed for, or take an offer on the claim you picked.',
  }),

  // ---- Step 6, Sign: the route, both parties, and the two signatures. -----
  Object.freeze({
    fragment: 'Paste the operator-published Direct Hot route manifest',
    step: 6 as const,
    remedy: 'Supply the route manifest this market operator published, below.',
  }),
  Object.freeze({
    fragment: 'route manifest authenticates another Market',
    step: 6 as const,
    remedy: 'Supply the route manifest for THIS market: the one given belongs to another Market or Trading program.',
  }),
  Object.freeze({
    fragment: 'Reduce its account or instruction geometry',
    step: 6 as const,
    remedy: 'Nothing here can be signed: the transaction this route builds is larger than the network will carry.',
  }),
  Object.freeze({
    fragment: 'both participants must be ready before signing',
    step: 6 as const,
    remedy: 'Wait until both sides hold the accounts a crossing settles into, then prepare again.',
  }),
  Object.freeze({
    fragment: 'blockhash expired at block height',
    step: 6 as const,
    remedy: 'Prepare the packet again: its blockhash aged out before it was signed.',
  }),
  Object.freeze({
    fragment: 'name every program needed to authenticate both participants',
    step: 6 as const,
    remedy: 'Pick a cluster whose deployment names the full Direct program set.',
  }),
  Object.freeze({
    fragment: 'genesis changed while the Direct route was being authenticated',
    step: 6 as const,
    remedy: 'Prepare again on one cluster: the endpoint moved while the route was being read.',
  }),
  Object.freeze({
    fragment: 'genesis changed after Direct preparation',
    step: 6 as const,
    remedy: 'Prepare again: the packet was built against a different chain than the one connected now.',
  }),
  Object.freeze({
    fragment: 'genesis changed while the wallet signed',
    step: 6 as const,
    remedy: 'Prepare again: the endpoint moved while your wallet held the packet.',
  }),
  Object.freeze({
    fragment: 'connected wallet changed after Direct preparation',
    step: 6 as const,
    remedy: 'Prepare again with the wallet you mean to trade from.',
  }),
  Object.freeze({
    fragment: 'sole required payer signature',
    step: 6 as const,
    remedy: 'Sign again and let the wallet return the exact bytes it was given.',
  }),
  Object.freeze({
    fragment: 'omitted its authenticated lookup table',
    step: 6 as const,
    remedy: 'Prepare again: the built packet carried no authenticated lookup table to sign against.',
  }),
  Object.freeze({
    fragment: 'local recovery storage',
    step: 6 as const,
    remedy: 'Allow this site to store data, or use a window that can: the packet is saved here before your key is ever asked for it.',
  }),
  Object.freeze({
    fragment: 'not ready to trade, so a packet will not be prepared',
    step: 6 as const,
    remedy: 'Ask the chain about your accounts again before preparing a packet.',
  }),
  // Specific, because step 7 refuses with a message that also ends "it must
  // not be submitted here" -- a looser fragment here would capture it.
  Object.freeze({
    fragment: 'signed Direct packet expired at block height',
    step: 6 as const,
    remedy: 'Prepare and sign again: this packet aged out while it was being signed, and must not be sent.',
  }),
  // The deployment check `participantReadRequest` raises during the send
  // poll: it is a cluster choice, which is step 1's business, not step 7's.
  Object.freeze({
    fragment: 'name every program needed to authenticate participant accounts',
    step: 1 as const,
    remedy: 'Pick a cluster whose deployment names the full Direct program set.',
  }),

  // ---- Step 7, Send: the one submission. ----------------------------------
  Object.freeze({
    fragment: 'genesis changed after the packet was signed',
    step: 7 as const,
    remedy: 'Reconnect to the chain this packet was signed for, or prepare a new one.',
  }),
  Object.freeze({
    fragment: 'the chain can no longer include it',
    step: 7 as const,
    remedy: 'Prepare and sign a new packet: this one expired before it was sent.',
  }),
]);

/**
 * Route one refusal to the step that owns it.
 *
 * `fallback` is the step that raised it, used when no fragment matches -- an
 * unrecognised refusal is shown where it happened rather than swallowed, and
 * `routed` records which of the two it was so a test can prove the table is
 * doing work rather than the fallback carrying everything.
 */
export function assignRefusalV1(detail: string, fallback: FlowStepIndexV1): StepRefusalV1 {
  for (const owner of REFUSAL_OWNERS_V1) {
    if (detail.includes(owner.fragment)) {
      return Object.freeze({ step: owner.step, remedy: owner.remedy, detail, routed: true });
    }
  }
  return Object.freeze({
    step: fallback,
    remedy: 'This step refused. Its own words are below.',
    detail,
    routed: false,
  });
}

/** Every fragment the table routes on, for the tests that pin the coverage. */
export function routedRefusalFragmentsV1(): ReadonlyArray<string> {
  return REFUSAL_OWNERS_V1.map((owner) => owner.fragment);
}
