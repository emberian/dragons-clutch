import {
  decodeDirectIntentTicketV1,
  DIRECT_TICKET_KIND_V1,
} from './directTicket';
import type { SignedDirectIntentV3 } from './directInlineV3';

/**
 * The ticket board: one relay's list of offers, and the client half that reads
 * it without ever believing it.
 *
 * A Direct ticket is bearer-signed self-authenticating data, so a board is a
 * permitted transport rather than a concession: it supplies *candidates*, which
 * is the category O-016 allows a caller to supply. **A relay can withhold. A
 * relay can never forge.** Its worst case is censorship and staleness — never a
 * wrong trade, never a stolen one.
 *
 * That invariant is what licenses this module to be as thin as it is, and it is
 * also what this module must not quietly spend. Two rules follow, and both are
 * enforced below rather than documented and hoped for:
 *
 * 1. **The board's parse is never trusted.** Every listed offer is re-decoded
 *    here through {@link decodeDirectIntentTicketV1} — the same call the paste
 *    box makes, on the same bytes — and an offer whose stored text this decoder
 *    refuses is reported as refused, never rendered. The board transports text;
 *    the decoder in this process is the only reader whose answer counts.
 * 2. **The board is never described as an authority.** Nothing here returns a
 *    "verified" flag. A decoded offer is WELL-FORMED. Only the chain verifies,
 *    at execution, by re-deriving the signing message natively.
 *
 * The consumer side is deliberately transport-blind. Step ③ of the trade flow
 * contracts for *"produce one `SignedDirectIntentV3`, by any means"*, so what
 * this module hands back is exactly that type — indistinguishable from a pasted
 * ticket, a URL fragment, or a resting order read from chain. The board is a
 * source plugged into a finished flow, not a flow of its own.
 */

/** Where one board lives. Absent config is the *supported* state, not an error. */
export type TicketBoardConfigV1 = Readonly<{
  /** Origin and optional base path, with no trailing slash. */
  baseUrl: string;
}>;

/** One offer as the board served it, re-decoded locally. */
export type BoardOfferV1 = Readonly<{
  /** The board's identifier for this offer: SHA-256 of `text`, lowercase hex. */
  digest: string;
  /**
   * The exact ticket text the board stored, byte for byte as its maker authored
   * it. This is the paste box's input, and it decodes identically.
   */
  text: string;
  /**
   * The signed intent, decoded HERE by the one decoder.
   *
   * WELL-FORMED, not verified: the fields are canonical and the shape is exact.
   * Whether the signature satisfies the chain is decided by the chain.
   */
  ticket: SignedDirectIntentV3;
  /**
   * The slot the board says it accepted this offer, or `null` when the board
   * knew no slot. Board-asserted and unverifiable — order a list by it, never
   * decide anything by it.
   */
  postedAtSlot: bigint | null;
}>;

/** One offer the board served that this process refused to decode. */
export type BoardRefusalV1 = Readonly<{
  digest: string;
  /** The local decoder's sentence, verbatim. */
  reason: string;
}>;

/** What one listing call learned. */
export type TicketBoardListingV1 = Readonly<{
  /** Decoded offers, newest first, as the board ordered them. */
  offers: readonly BoardOfferV1[];
  /**
   * The slot the board judged expiry against, or `null` if it judged none.
   *
   * A board has no chain of its own. When the caller supplies a finalized slot
   * the board filters against exactly that; otherwise it filters against the
   * highest slot any caller has shown it, which is a hint and is reported as
   * one. The flow re-checks expiry itself at ⑤ regardless.
   */
  slotBasis: bigint | null;
  /** Offers the board dropped as expired before answering. */
  droppedExpired: number;
  /** Offers the board served whose text this process refused. */
  refused: readonly BoardRefusalV1[];
}>;

/** What one post call learned. */
export type TicketBoardPostV1 = Readonly<{
  /** The board's identifier for the accepted offer. */
  digest: string;
  /** True when the board already held this exact ticket. */
  duplicate: boolean;
}>;

/**
 * A board refused, or could not be reached.
 *
 * Carries the board's own sentence when it gave one, because the service names
 * every refusal and a caller that flattens them to "failed" throws away the
 * only thing the maker can act on.
 */
export class TicketBoardError extends Error {
  /** HTTP status, or `null` when the request never got an answer. */
  readonly status: number | null;

  constructor(message: string, status: number | null) {
    super(message);
    this.name = 'TicketBoardError';
    this.status = status;
  }
}

/** The one query a listing takes. */
export type TicketBoardQueryV1 = Readonly<{
  /** Base58 Market address. Required — a board is read per market. */
  market: string;
  /** Restrict to one outcome coordinate. Omit for every outcome. */
  outcome?: number;
  /**
   * The finalized slot to judge expiry against.
   *
   * Supply it whenever the caller has one — the trade flow always does, since
   * step ⑤ needs the same slot — so that expiry is decided by the chain's clock
   * rather than by whatever the board last overheard.
   */
  slot?: bigint;
}>;

/** The fetch this module calls, injectable so tests need no network. */
export type TicketBoardFetchV1 = (
  input: string,
  init?: Readonly<{ method?: string; headers?: Record<string, string>; body?: string }>,
) => Promise<Readonly<{ ok: boolean; status: number; text: () => Promise<string> }>>;

/**
 * Read one board URL into a config, or `null`.
 *
 * `null` is the deployment that has no board, and it is a first-class state:
 * the site hides every board affordance and the paste box — which needs no
 * relay at all — carries the whole flow. A board is a convenience; the absence
 * of one is never an error and must never be rendered as one.
 */
export function ticketBoardConfigV1(baseUrl: string | undefined | null): TicketBoardConfigV1 | null {
  if (typeof baseUrl !== 'string') return null;
  const trimmed = baseUrl.trim().replace(/\/+$/, '');
  if (trimmed.length === 0) return null;
  if (!/^https?:\/\/[^\s]+$/.test(trimmed)) {
    throw new TicketBoardError(`ticket board URL is not one http(s) origin: ${baseUrl}`, null);
  }
  return Object.freeze({ baseUrl: trimmed });
}

function exactOutcome(value: unknown): number {
  if (typeof value !== 'number' || !Number.isSafeInteger(value) || value < 0 || value > 0xffff_ffff) {
    throw new TicketBoardError('board outcome filter is not an exact unsigned integer', null);
  }
  return value;
}

function boardJson(text: string, status: number): Record<string, unknown> {
  let value: unknown;
  try {
    value = JSON.parse(text);
  } catch {
    throw new TicketBoardError('the offer board answered with text that is not JSON', status);
  }
  if (value === null || typeof value !== 'object' || Array.isArray(value)) {
    throw new TicketBoardError('the offer board answered with something that is not one JSON object', status);
  }
  return value as Record<string, unknown>;
}

function boardRefusalSentence(body: Record<string, unknown>, status: number): string {
  const reason = body.reason;
  return typeof reason === 'string' && reason.length > 0
    ? reason
    : `the offer board refused with status ${status} and named no reason`;
}

function optionalSlot(value: unknown, field: string): bigint | null {
  if (value === null || value === undefined) return null;
  if (typeof value !== 'string' || !/^(0|[1-9][0-9]*)$/.test(value)) {
    throw new TicketBoardError(`the offer board's ${field} is not one canonical unsigned decimal string`, null);
  }
  return BigInt(value);
}

/**
 * List the offers one board holds for a market, decoding each one here.
 *
 * Ordering is the board's — newest first — and is deliberately NOT price
 * ordering: §4.3 sorts by price for the reader, but sorting is a rendering
 * decision made against a route the board does not have. The board reports
 * arrival order; the flow decides presentation.
 *
 * Client-side filtering the flow still owns after this call: wrong generation,
 * wrong fee rate, outcome ≥ width, self-authored, and buy-side. Those need the
 * route and the connected wallet, neither of which belongs in a transport.
 */
export async function listBoardOffersV1(
  config: TicketBoardConfigV1,
  query: TicketBoardQueryV1,
  fetchImpl: TicketBoardFetchV1,
): Promise<TicketBoardListingV1> {
  const parameters = new URLSearchParams({ market: query.market });
  if (query.outcome !== undefined) parameters.set('outcome', String(exactOutcome(query.outcome)));
  if (query.slot !== undefined) parameters.set('slot', query.slot.toString());

  let response: Awaited<ReturnType<TicketBoardFetchV1>>;
  try {
    response = await fetchImpl(`${config.baseUrl}/tickets?${parameters.toString()}`);
  } catch (error) {
    throw new TicketBoardError(
      `the offer board did not answer: ${error instanceof Error ? error.message : String(error)}`,
      null,
    );
  }
  const text = await response.text();
  const body = boardJson(text, response.status);
  if (!response.ok) throw new TicketBoardError(boardRefusalSentence(body, response.status), response.status);

  const raw = body.offers;
  if (!Array.isArray(raw)) {
    throw new TicketBoardError("the offer board's listing carries no offers array", response.status);
  }
  const offers: BoardOfferV1[] = [];
  const refused: BoardRefusalV1[] = [];
  for (const entry of raw) {
    if (entry === null || typeof entry !== 'object' || Array.isArray(entry)) {
      throw new TicketBoardError("an entry in the offer board's listing is not one JSON object", response.status);
    }
    const offer = entry as Record<string, unknown>;
    const digest = typeof offer.digest === 'string' ? offer.digest : '';
    if (typeof offer.text !== 'string') {
      refused.push(Object.freeze({ digest, reason: 'the board served an offer with no ticket text' }));
      continue;
    }
    // THE LOAD-BEARING LINE. Same decoder, same bytes, same answer as the paste
    // box: the board's own opinion of this text reaches no further than here.
    try {
      offers.push(Object.freeze({
        digest,
        text: offer.text,
        ticket: decodeDirectIntentTicketV1(offer.text),
        postedAtSlot: optionalSlot(offer.postedAtSlot, 'postedAtSlot'),
      }));
    } catch (error) {
      refused.push(Object.freeze({
        digest,
        reason: error instanceof Error ? error.message : String(error),
      }));
    }
  }
  const dropped = body.droppedExpired;
  return Object.freeze({
    offers: Object.freeze(offers),
    slotBasis: optionalSlot(body.slotBasis, 'slotBasis'),
    droppedExpired: typeof dropped === 'number' && Number.isSafeInteger(dropped) && dropped >= 0 ? dropped : 0,
    refused: Object.freeze(refused),
  });
}

/**
 * Publish one authored ticket to a board.
 *
 * The text is sent verbatim, because the ticket's bytes are canonical and a
 * re-serialization here would be a second writer of a shape that has exactly
 * one. It is decoded first so that a malformed ticket is refused in this
 * process, by this decoder, with this sentence — rather than travelling to a
 * relay to be refused by a stranger's.
 */
export async function postBoardOfferV1(
  config: TicketBoardConfigV1,
  ticketText: string,
  fetchImpl: TicketBoardFetchV1,
): Promise<TicketBoardPostV1> {
  decodeDirectIntentTicketV1(ticketText);

  let response: Awaited<ReturnType<TicketBoardFetchV1>>;
  try {
    response = await fetchImpl(`${config.baseUrl}/tickets`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: ticketText,
    });
  } catch (error) {
    throw new TicketBoardError(
      `the offer board did not answer: ${error instanceof Error ? error.message : String(error)}`,
      null,
    );
  }
  const text = await response.text();
  const body = boardJson(text, response.status);
  if (!response.ok) throw new TicketBoardError(boardRefusalSentence(body, response.status), response.status);
  if (typeof body.digest !== 'string' || !/^[0-9a-f]{64}$/.test(body.digest)) {
    throw new TicketBoardError('the offer board accepted an offer without naming its digest', response.status);
  }
  return Object.freeze({ digest: body.digest, duplicate: body.duplicate === true });
}

/** What one board says about itself. */
export type TicketBoardHealthV1 = Readonly<{
  offers: number;
  /** The highest slot any caller has shown the board. A hint, never a clock. */
  observedSlot: bigint | null;
}>;

/** Ask a board whether it is there. */
export async function boardHealthV1(
  config: TicketBoardConfigV1,
  fetchImpl: TicketBoardFetchV1,
): Promise<TicketBoardHealthV1> {
  let response: Awaited<ReturnType<TicketBoardFetchV1>>;
  try {
    response = await fetchImpl(`${config.baseUrl}/health`);
  } catch (error) {
    throw new TicketBoardError(
      `the offer board did not answer: ${error instanceof Error ? error.message : String(error)}`,
      null,
    );
  }
  const body = boardJson(await response.text(), response.status);
  if (!response.ok) throw new TicketBoardError(boardRefusalSentence(body, response.status), response.status);
  const offers = body.offers;
  return Object.freeze({
    offers: typeof offers === 'number' && Number.isSafeInteger(offers) && offers >= 0 ? offers : 0,
    observedSlot: optionalSlot(body.observedSlot, 'observedSlot'),
  });
}

/** The kind every offer on every board declares. Re-exported so a consumer of
 * the board needs no second import to name what it is reading. */
export { DIRECT_TICKET_KIND_V1 };
