/**
 * Instruction decoding, by route.
 *
 * Every dClutch program dispatches on the leading eight bytes of its
 * instruction data. `lib/generated/routeCensus.ts` carries every such magic,
 * emitted from the gauntlet census's own enumeration of the program sources —
 * so the browser learns which route a magic selects from the same authority the
 * reference documentation and the campaign bindings use, and never from a list
 * kept by hand.
 *
 * This module adds the two things the census cannot: a sentence of prose per
 * route, and the link from a route to the record spec that describes its
 * request body, so an instruction's fields render the same way an account's do.
 * It keys on census ROUTE IDS, which are census identifiers rather than
 * protocol facts — no magic, offset or seed is restated here.
 *
 * What it will not do: name a route the census does not select by magic. Seven
 * programs dispatch on a predicate, a decoded action tag, or an exact length
 * instead, and `UNSELECTED_ENTRY_ROUTES` is the list of them. An instruction to
 * one of those programs renders as "this program's dispatch is not selected by
 * a leading magic", which is true, rather than as a guess.
 */
import { LIFECYCLE_RENT_INSTRUCTION_MAGIC_V2 } from '../generated/coreFound';
import { HOT_EXECUTION_MAGIC_V3 } from '../generated/directInlineV3';
import { GENERIC_FOUNDING_REQUEST_MAGIC_V1 } from '../generated/genericFoundingV1';
import { REQUEST_MAGIC_V2 as RATIONAL_REQUEST_MAGIC_V2 } from '../generated/rationalTerminalHotV3';
import {
  INSTRUCTION_MAGICS,
  UNSELECTED_ENTRY_ROUTES,
  type InstructionMagic,
  type UnselectedEntryRoute,
} from '../generated/routeCensus';
import {
  decodeAgainstSpec,
  leadingMagic,
  magicText,
  specForMagic,
  type DecodedRecord,
  type RecordSpec,
} from './accountRecords';

/** Prose and body linkage for one census route. */
export type InstructionRenderer = Readonly<{
  /** A census route id, exactly as `routeCensus.ts` spells it. */
  routeId: string;
  summary: string;
  /**
   * When the request body has a record spec, the magic that names it. This is
   * the SAME magic the route selects unless the route wraps a body of another
   * shape, in which case it is the wrapped one.
   */
  bodyMagic?: string;
}>;

/**
 * Prose for every magic-selected route the census enumerates.
 *
 * The gate in `lib/explorerCoverage.test.ts` requires each distinct instruction
 * magic to have at least one of its routes named here. When a program grows a
 * route, the census picks it up and the gate fails until a sentence is written
 * for it.
 */
const INSTRUCTION_RENDERERS: ReadonlyArray<InstructionRenderer> = Object.freeze([
  // ------------------------------------------------------------------- claims
  {
    routeId: 'claims/affine_batch_v2::process',
    summary: 'Applies a batch of claim-balance changes under one signed plan.',
  },
  {
    routeId: 'claims/custody_replay_v1::process',
    summary: 'Creates or advances the record that stops a collateral move being replayed.',
  },
  {
    routeId: 'claims/founding_v5::process',
    summary: 'Sets up a market’s claim supply and the vault that backs it, in one step.',
  },
  {
    routeId: 'claims/market_closure_v1::process',
    summary: 'Closes a retired market’s claim state and releases what it held.',
  },
  {
    routeId: 'claims/process_core_effect',
    summary: 'Applies a change the Core program asked for to claim balances.',
  },
  {
    routeId: 'claims/protocol_position_v2::process',
    summary: 'Opens, updates or closes one owner’s claim balances.',
  },
  {
    routeId: 'claims/rational_lifecycle_v2::process',
    summary: 'Retires a wrapped-claim receipt, or redeems what it stands for.',
  },
  {
    routeId: 'claims/rational_representation_v2::process',
    summary: 'Wraps claims into a token that can be moved on its own, or unwraps them back.',
    bodyMagic: magicText(RATIONAL_REQUEST_MAGIC_V2),
  },
  {
    // `RationalReplayCloseRequestV1`: "One request to close a spent Rational
    // replay cursor and reclaim its rent." Wrapping creates one cursor per
    // (descriptor, actor) and the rent for it sits with the actor who paid it.
    routeId: 'claims/rational_representation_v2::process_replay_close',
    summary: 'Closes a spent wrap/unwrap replay record and returns its rent to whoever paid it.',
  },
  {
    routeId: 'claims/signed_delta_v3::process',
    summary: 'Applies the balance changes a pool or an auction settlement worked out.',
  },
  {
    routeId: 'claims/sparse_native_transfer_v1::process',
    summary: 'Moves lamports between the protocol’s own accounts without touching any claim balance.',
  },
  {
    routeId: 'claims/terminal_settlement_v3::process',
    summary: 'Pays out a settled market: winning claims are redeemed against the vault.',
  },

  // --------------------------------------------------------------------- core
  {
    routeId: 'core/found::process#Found',
    summary: 'Founds a Market from its finalized records: the first Core action, the one every later action names.',
  },
  {
    routeId: 'core/open_market::process#OpenMarket',
    summary: 'Opens a founded Market for trading once its readiness has been verified.',
  },
  {
    routeId: 'core/begin_retiring::process#BeginRetiring',
    summary: 'Moves a Terminal Market into retirement, the phase in which its claims are reclaimed.',
  },
  {
    routeId: 'core/retire_v1::process#Retire',
    summary: 'Advances a retiring Market one coordinate toward Retired, reclaiming what that coordinate held.',
  },
  {
    routeId: 'core/retire_v1::process_checkpoint_prepare#Retire',
    summary: 'Prepares the retirement checkpoint a Retire step will commit against.',
  },
  {
    routeId: 'core/resolution::process#Retire',
    summary: 'Retires the resolution side of a Market whose answer is in.',
  },
  {
    routeId: 'core/process_instruction#Retire',
    summary: 'The inline Retire arm of Core’s dispatcher: the same Action, routed by the request’s length.',
  },
  {
    routeId: 'core/capability::process#CloseCapability',
    summary: 'Closes a capability root once nothing on the Market still needs it.',
  },
  {
    routeId: 'core/process_instruction#CloseCapability',
    summary: 'The inline CloseCapability arm of Core’s dispatcher, routed by the request’s length.',
  },
  {
    routeId: 'core/execute_provider_v3::process#ExecuteProvider',
    summary: 'Executes a provider step on a Market’s source, the act that moves an observation toward a certificate.',
  },
  {
    routeId: 'core/process_instruction#else',
    summary: 'Core’s wildcard arm: a well-formed request whose length no Action admits, refused by name.',
  },
  {
    routeId: 'core/found::project',
    summary: 'Works out what a founding would create, without creating it.',
  },
  {
    routeId: 'core/infrastructure_v2::process_initialize_v2',
    summary:
      'Names the Registry and Rent builds this deployment trusts, succeeding the selection made before them. The key that moved a program\u2019s bytes has to consent here, on chain, before anything will accept them.',
  },
  {
    routeId: 'core/generic_founding_v1::process',
    summary: 'The Core program’s half of founding a market: create it, or open it for trading.',
    bodyMagic: magicText(GENERIC_FOUNDING_REQUEST_MAGIC_V1),
  },
  {
    routeId: 'core/series_consume::process',
    summary: 'Spends a series permit against collateral already locked for it.',
  },
  {
    routeId: 'core/series_open::process',
    summary: 'Opens the next market in a series.',
  },
  {
    routeId: 'core/series_permit_expiry::process',
    summary: 'Expires an unused series permit and returns what it reserved.',
  },

  // ------------------------------------------------------------------ custody
  {
    routeId: 'custody/delegated::process',
    summary: 'Moves collateral on behalf of an owner who authorized it.',
  },
  {
    routeId: 'custody/projected::process',
    summary: 'Locks collateral for a founding, then either completes or releases it.',
  },

  // ----------------------------------------------------------------- registry
  {
    routeId: 'registry/continuation_v1::process',
    summary: 'Continues publishing a record too large for one transaction.',
  },
  {
    routeId: 'registry/hot_continuation_v2::process',
    summary: 'Checks that a trade is running the exact program build it claims to.',
    bodyMagic: magicText(HOT_EXECUTION_MAGIC_V3),
  },
  {
    routeId: 'registry/record_v1::dispatch',
    summary: 'Stages, finalizes or closes a stored record.',
  },

  // --------------------------------------------------------------------- rent
  {
    routeId: 'rent/process_create_v2#Create',
    summary: 'Prepays a market’s rent, refundable to one named wallet.',
    bodyMagic: magicText(LIFECYCLE_RENT_INSTRUCTION_MAGIC_V2),
  },
  {
    routeId: 'rent/process_sweep_v2#Sweep',
    summary: 'Sweeps accrued lamports out to the wallet they are owed to.',
    bodyMagic: magicText(LIFECYCLE_RENT_INSTRUCTION_MAGIC_V2),
  },
  {
    routeId: 'rent/process_close_v2#Close',
    summary: 'Closes spent prepaid rent and refunds what is left.',
    bodyMagic: magicText(LIFECYCLE_RENT_INSTRUCTION_MAGIC_V2),
  },

  // --------------------------------------------------------------- resolution
  {
    routeId: 'resolution/process_submit#magic',
    summary: 'A resolution provider submits its observation of the world.',
  },
  {
    routeId: 'resolution/process_reclaim#magic',
    summary: 'A resolution provider reclaims the bond it posted.',
  },
  {
    // The register's own distinction, kept: `process_reclaim` "proves the
    // submission became truth", and this route "proves it never can" -- the
    // submitter's own deadline has passed AND the Source has left Primary or
    // has already been discharged. Both, not either. So the word this renders
    // is ABANDONED, which is what the magic, the request type and the handler
    // are all named for, and it is not the same act as reclaiming a bond that
    // was consumed.
    routeId: 'resolution/process_abandon#magic',
    summary:
      'Returns the bond behind a submission the market can no longer consume, once its own deadline has passed and the source has moved on.',
  },
  {
    // `provider_instruction_v3.rs:3` is explicit about the negative space, and
    // it is the part a reader most needs: "This instruction never submits or
    // reclaims a Pyth update." Posting the update and consuming it are
    // separate transactions by different parties, and only the second is this.
    routeId: 'resolution/provider_instruction_v3::process_provider_resolution_v3',
    summary: 'Resolves a market from a price update already posted on chain. It does not post that update and does not reclaim it.',
  },
  {
    routeId: 'resolution/sponsored_push_v1::process_sponsored_push_v1',
    summary:
      'Captures a sponsored price feed’s current value as a permanent candidate, and afterwards settles, closes or fails what it captured. The upstream feed is overwritten in place; this is what keeps a copy.',
  },
  {
    routeId: 'resolution/pre_market_funding_v1::process_pre_market_funding_v2',
    summary: 'Funds the market a founding is about to create, against a projection of exactly which market that will be.',
  },
  {
    routeId: 'resolution/pre_market_funding_abort_v1::process_pre_market_funding_abort_v1',
    summary: 'Rolls back a pre-market funding ledger whose checkpoint expired, returning what it held.',
  },
  {
    routeId: 'resolution/core_effect::process_direct_funding_activation_v1',
    summary: 'Activates one pending funding ledger and writes the receipt for it last. Anyone may submit it.',
  },
  {
    routeId: 'resolution/core_effect::process_direct_funding_close_v1',
    summary: 'Closes a finished market’s source and its funding ledger together, without asking the Core program to do it.',
  },

  // ------------------------------------------------------------------ trading
  {
    routeId: 'trading/generic_founding_stages_v1::process_generic_market_open_v1',
    summary:
      'Opens the market a founding permit already paid for. The submitter supplies no economic truth of its own: the permit carries all of it, and expires on its own schedule.',
  },
  {
    routeId: 'trading/projected_custody_bootstrap_v1::process_controller_funding_prepare_v1',
    summary: 'Creates the two pending funding ledgers a founding needs, and the checkpoint that binds them to it.',
  },
  {
    routeId: 'trading/projected_custody_bootstrap_v1::process_projected_custody_bootstrap_v2',
    summary:
      'Creates the collateral vault and replay record a market is founded on top of. Both are created together or neither is, so a market is never left with one and not the other.',
  },
  {
    routeId: 'trading/user_position_admission_v1::process_user_position_admission_v1',
    summary:
      'Opens or closes one wallet’s position in a market, on that wallet’s own signature. The Claims program stays the only writer of the balances; this route adds the signature and nothing else.',
  },
  {
    routeId: 'trading/hot_v3::process_capability_seal_v1',
    summary:
      'Records once, permanently, that this build accepts a trading artifact. Anyone may write it, because the answer is a function of public bytes; nobody may rewrite it.',
  },
  {
    routeId: 'trading/hot_v3::process_capability_seal_close_v1',
    summary: 'Reclaims the rent from a seal no live release addresses any more. It refuses to close one that is still reachable.',
  },
  {
    routeId: 'trading/direct_begin_retiring_v1::process_direct_begin_retiring_v1',
    summary: 'Moves a market’s Direct trading from open to retiring. Anyone may submit it once the market itself is retiring.',
  },
  {
    routeId: 'trading/direct_token_setup_v1::process_direct_token_setup_v1',
    summary: 'Creates the empty seller and fee token accounts a Direct market pays through, before any trade uses them.',
  },
  {
    routeId: 'trading/direct_replay_setup_v1::process_direct_replay_setup_v1',
    summary: 'Creates, on first use, the record that stops one maker’s trades being replayed.',
  },
  {
    routeId: 'trading/direct_fee_settlement_v1::process_direct_fee_settlement_v1',
    summary:
      'Pays the fee a fill recorded but did not move, from whoever owes it to the market’s configured recipient. The market’s phase is not checked: the debt outlives trading.',
  },
  {
    // The module names itself the ONLY route that ever lowers
    // `open_maker_root_count`, which is what `CloseCapability` gates on, so a
    // reader looking at a market stuck short of retirement is usually looking
    // at makers nobody has closed yet. The closer keeps nothing: the reward is
    // zero and the rent goes to the wallet the record names.
    routeId: 'trading/direct_close_maker_v1::process_direct_close_maker_v1',
    summary:
      'Closes one drained, settled maker and returns its rent to whoever paid it. This is the only route that lowers a market’s open-maker count, and it refuses while a fee is still owed.',
  },

  // The seven stages of one durable Dealer scenario, in the order they run.
  // Each is its own transaction because the work does not fit in one.
  {
    routeId: 'trading/dealer_scenario_checkpoint_v1::process_dealer_scenario_checkpoint_create_v1',
    summary: 'Starts one Dealer scenario: creates the checkpoint everything below is written into.',
  },
  {
    routeId: 'trading/dealer_scenario_checkpoint_v1::process_dealer_scenario_checkpoint_page_v1',
    summary: 'Appends one page of a scenario’s transcript. Six pages, in order, and each is read-only afterwards.',
  },
  {
    routeId: 'trading/dealer_scenario_checkpoint_v1::process_dealer_scenario_checkpoint_evaluate_v1',
    summary: 'Seals the producer’s evaluation of a scenario, once all six transcript pages exist.',
  },
  {
    routeId: 'trading/dealer_scenario_checkpoint_v1::process_dealer_scenario_checkpoint_reserve_v1',
    summary: 'Records the collateral reservation an evaluated scenario needs before it can commit.',
  },
  {
    routeId: 'trading/dealer_scenario_checkpoint_v1::process_dealer_scenario_checkpoint_rollback_v1',
    summary: 'Records the reverse of a reservation, for a scenario that expired instead of committing.',
  },
  {
    routeId: 'trading/dealer_scenario_checkpoint_v1::process_dealer_scenario_checkpoint_commit_v1',
    summary: 'Commits an evaluated scenario: applies its claim changes and writes its obligation, in one step.',
  },
  {
    routeId: 'trading/dealer_scenario_checkpoint_v1::process_dealer_scenario_checkpoint_cleanup_v1',
    summary: 'Closes an expired scenario checkpoint and returns its rent to the wallet named when it was created. Anyone may submit it.',
  },
]);

const RENDERER_BY_ROUTE: ReadonlyMap<string, InstructionRenderer> = new Map(
  INSTRUCTION_RENDERERS.map((entry) => [entry.routeId, entry]),
);

const ROUTES_BY_MAGIC: ReadonlyMap<string, ReadonlyArray<InstructionMagic>> = (() => {
  const found = new Map<string, InstructionMagic[]>();
  for (const entry of INSTRUCTION_MAGICS) {
    const held = found.get(entry.magic) ?? [];
    held.push(entry);
    found.set(entry.magic, held);
  }
  return found;
})();

/** Every route the census enumerates, for the coverage table. */
export function instructionRenderers(): ReadonlyArray<InstructionRenderer> {
  return INSTRUCTION_RENDERERS;
}

/** Entry routes no leading magic selects — the honest gap, carried from the census. */
export function unselectedEntryRoutes(): ReadonlyArray<UnselectedEntryRoute> {
  return UNSELECTED_ENTRY_ROUTES;
}

// -------------------------------------------------------------------- decoding

export type DecodedInstruction = Readonly<{
  /** The leading eight bytes as text, when printable. */
  magic: string | null;
  /** Every census route this magic selects. More than one is normal. */
  routes: ReadonlyArray<
    Readonly<{
      routeId: string;
      program: string;
      handler: string;
      provenance: string;
      summary: string | null;
    }>
  >;
  /** The request body, decoded against its record spec when one is rendered. */
  body: DecodedRecord | null;
  /**
   * When the instruction is a Hot envelope, the family request that travels
   * inside it, decoded in its own right. The envelope names its own length.
   */
  inner: DecodedInstruction | null;
  bytes: number;
  /** Why nothing more was decoded, when nothing more was. */
  note: string | null;
}>;

/** The offset the Hot envelope's family request starts at, read from its own spec. */
function hotEnvelopeBody(spec: RecordSpec, data: Uint8Array): Uint8Array | null {
  if (spec.width.kind !== 'header-only') return null;
  const start = spec.width.headerBytes;
  if (data.length <= start) return null;
  return data.slice(start);
}

const HOT_ENVELOPE_ROUTE = 'registry/hot_continuation_v2::process';

/**
 * Decode one instruction's data.
 *
 * Never throws and never guesses: an unrecognized magic returns its text (or
 * `null` when the bytes are not printable) with an empty route list.
 */
export function decodeInstructionData(data: Uint8Array, depth = 0): DecodedInstruction {
  const magic = leadingMagic(data);
  const censusRoutes = magic === null ? [] : (ROUTES_BY_MAGIC.get(magic) ?? []);
  const routes = censusRoutes.map((route) =>
    Object.freeze({
      routeId: route.routeId,
      program: route.program,
      handler: route.handler,
      provenance: route.provenance,
      summary: RENDERER_BY_ROUTE.get(route.routeId)?.summary ?? null,
    }),
  );

  const bodyMagic =
    censusRoutes
      .map((route) => RENDERER_BY_ROUTE.get(route.routeId)?.bodyMagic)
      .find((held) => held !== undefined) ?? magic;
  const spec = bodyMagic === null || bodyMagic === undefined ? null : specForMagic(bodyMagic);
  const body = spec === null ? null : decodeAgainstSpec(spec, data);

  let inner: DecodedInstruction | null = null;
  let note: string | null = null;
  if (spec !== null && routes.some((route) => route.routeId === HOT_ENVELOPE_ROUTE) && depth < 2) {
    const nested = hotEnvelopeBody(spec, data);
    if (nested !== null) inner = decodeInstructionData(nested, depth + 1);
  }
  if (magic === null) {
    note = 'The first eight bytes are not readable text, so they are not a dClutch instruction magic.';
  } else if (routes.length === 0 && spec === null) {
    note = 'No dClutch route and no declared record uses this magic.';
  } else if (routes.length === 0) {
    note = 'No route uses this magic. It belongs to the record that carries it.';
  }

  return Object.freeze({ magic, routes: Object.freeze(routes), body, inner, bytes: data.length, note });
}

// ------------------------------------------------------------- program naming

/**
 * Programs everyone shares, named by the runtime rather than by dClutch.
 *
 * Kept here rather than in a workspace component so the explorer and the
 * activity view resolve the same names. A program not in this map and not named
 * by the reader is rendered unnamed — never guessed from a prefix.
 */
export const WELL_KNOWN_PROGRAMS: ReadonlyMap<string, string> = new Map([
  ['11111111111111111111111111111111', 'System Program'],
  ['Ed25519SigVerify111111111111111111111111111', 'Ed25519 signature verification'],
  ['ComputeBudget111111111111111111111111111111', 'Compute budget'],
  ['AddressLookupTab1e1111111111111111111111111', 'Address lookup table'],
  ['BPFLoaderUpgradeab1e11111111111111111111111', 'Upgradeable loader'],
  ['TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb', 'Token-2022'],
  ['TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA', 'Token'],
  ['ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL', 'Associated token account'],
  ['Sysvar1nstructions1111111111111111111111111', 'Instructions sysvar'],
  ['SysvarRent111111111111111111111111111111111', 'Rent sysvar'],
]);

/**
 * A label for a program address: the reader's own, then the runtime's, then
 * none. A dClutch program has no fixed address — the reader selects it — so
 * this never asserts one.
 */
export function programLabel(
  address: string,
  readerLabels: Readonly<Record<string, string>> = {},
): string | null {
  return readerLabels[address] ?? WELL_KNOWN_PROGRAMS.get(address) ?? null;
}
