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
    summary: 'Apply a batch of affine claim-balance deltas under one authenticated plan.',
  },
  {
    routeId: 'claims/custody_replay_v1::process',
    summary: 'Create or advance the Custody replay record a Claims action settles through.',
  },
  {
    routeId: 'claims/founding_v5::process',
    summary: 'Found the Claims aggregate for a Market, atomically with its Custody composition.',
  },
  {
    routeId: 'claims/market_closure_v1::process',
    summary: 'Close a retired Market’s Claims state and release what it held.',
  },
  {
    routeId: 'claims/process_core_effect',
    summary: 'Apply a Core-authored effect to Claims state as a CPI child.',
  },
  {
    routeId: 'claims/protocol_position_v2::process',
    summary: 'Open, advance or close one owner’s Claims Position.',
  },
  {
    routeId: 'claims/rational_lifecycle_v2::process',
    summary: 'Drive the rational representation lifecycle: retire a receipt, redeem a coordinate.',
  },
  {
    routeId: 'claims/rational_representation_v2::process',
    summary: 'Issue, unwrap, denominate or reconstitute a rational representation of a claim.',
    bodyMagic: magicText(RATIONAL_REQUEST_MAGIC_V2),
  },
  {
    routeId: 'claims/signed_delta_v3::process',
    summary: 'Apply a signed position-delta plan produced by a Dealer or General settlement.',
  },
  {
    routeId: 'claims/sparse_native_transfer_v1::process',
    summary: 'Move native lamports between Claims-owned compartments without touching balances.',
  },
  {
    routeId: 'claims/terminal_settlement_v3::process',
    summary: 'Settle a terminal Market: redeem winning claims against the Hoard.',
  },

  // --------------------------------------------------------------------- core
  {
    routeId: 'core/found::project',
    summary: 'Project a founding without committing it, so the caller can see the Market it would create.',
  },
  {
    routeId: 'core/generic_founding_v1::process',
    summary: 'The Core half of the atomic generic founding: Found-and-permit, or Open.',
    bodyMagic: magicText(GENERIC_FOUNDING_REQUEST_MAGIC_V1),
  },
  {
    routeId: 'core/series_consume::process',
    summary: 'Consume a Series permit against a projected Custody lock receipt.',
  },
  {
    routeId: 'core/series_open::process',
    summary: 'Open the next Market in a Series against its Claims founding receipt.',
  },
  {
    routeId: 'core/series_permit_expiry::process',
    summary: 'Expire an unconsumed Series permit and return what it reserved.',
  },

  // ------------------------------------------------------------------ custody
  {
    routeId: 'custody/delegated::process',
    summary: 'Move collateral under a delegated Custody authority, against an exact replay revision.',
  },
  {
    routeId: 'custody/projected::process',
    summary: 'Lock, realize or abort a projected Custody transfer — the founding-time escrow.',
  },

  // ----------------------------------------------------------------- registry
  {
    routeId: 'registry/continuation_v1::process',
    summary: 'Continue a multi-transaction Registry publication.',
  },
  {
    routeId: 'registry/hot_continuation_v2::process',
    summary: 'The Registry’s side of a Hot execution: authenticate the release the transition runs under.',
    bodyMagic: magicText(HOT_EXECUTION_MAGIC_V3),
  },
  {
    routeId: 'registry/record_v1::dispatch',
    summary: 'Stage, finalize or close a content-addressed record account.',
  },

  // --------------------------------------------------------------------- rent
  {
    routeId: 'rent/process_create_v2#Create',
    summary: 'Create a Market’s lifecycle RentCredit, prepaying its rent to a named refund wallet.',
    bodyMagic: magicText(LIFECYCLE_RENT_INSTRUCTION_MAGIC_V2),
  },
  {
    routeId: 'rent/process_sweep_v2#Sweep',
    summary: 'Sweep accrued lamports out of a RentCredit to its beneficiary.',
    bodyMagic: magicText(LIFECYCLE_RENT_INSTRUCTION_MAGIC_V2),
  },
  {
    routeId: 'rent/process_close_v2#Close',
    summary: 'Close a spent RentCredit and refund its principal.',
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
    note = 'The leading eight bytes are not printable ASCII, so they are not a dClutch instruction magic.';
  } else if (routes.length === 0 && spec === null) {
    note = 'No route the census enumerates is selected by this magic, and no rendered record carries it.';
  } else if (routes.length === 0) {
    note = 'No census route is selected by this magic; the bytes are decoded against the record of the same name.';
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
