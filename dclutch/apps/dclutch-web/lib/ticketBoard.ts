import {
  boardHealthV1,
  listBoardOffersV1,
  postBoardOfferV1,
  ticketBoardConfigV1,
  TicketBoardError,
  type BoardOfferV1,
  type BoardRefusalV1,
  type TicketBoardConfigV1,
  type TicketBoardHealthV1,
  type TicketBoardListingV1,
  type TicketBoardPostV1,
  type TicketBoardQueryV1,
} from '@dclutch/sdk/ticketBoard';

/**
 * This deployment's offer board — the configuration half, and only that.
 *
 * The transport lives in `@dclutch/sdk/ticketBoard` and takes its board URL as
 * an argument, because an SDK that reached for `process.env` would be a second
 * place a deployment is decided. This module is where the deployment is
 * decided, once, and it is the app's own file for the same reason
 * `rpcDefault.ts` is: the Pages build is a static export, so `NEXT_PUBLIC_*` is
 * the one mechanism that survives it.
 *
 * **An unset board URL is the default and is fully supported.** No board is
 * configured for a plain checkout, and that deployment is not degraded: the
 * paste box needs no relay and carries the entire flow by itself. Callers get
 * `null` from {@link configuredTicketBoardV1} and hide every board affordance —
 * they must never render the absence as a failure, and must never remove the
 * paste box, which is the no-relay path and the proof that the board is a
 * convenience rather than an authority.
 *
 * Nothing this module returns is verified. A decoded offer is WELL-FORMED. The
 * chain re-derives the signing message and verifies natively at execution, and
 * it is the only thing that does.
 */

/**
 * The board this build points at, or `null` for the deployments with none.
 *
 * A static export BAKES this value into the public bundle, so it must only ever
 * carry a URL that is deliberately public. A board holds no keys, takes no
 * custody, and has no authority, so its URL is not a secret — but that is a
 * property of *this* service, not a licence to inline any endpoint here.
 */
export function configuredTicketBoardV1(): TicketBoardConfigV1 | null {
  return ticketBoardConfigV1(process.env.NEXT_PUBLIC_DCLUTCH_TICKET_BOARD);
}

/**
 * The standing line every board state shows, verbatim.
 *
 * It is exported rather than written into a component so that one sentence
 * cannot become three, each slightly weaker than the last. A board's honesty is
 * a fixed string or it is decoration.
 */
export const TICKET_BOARD_HONESTY_LINE_V1 =
  'Offers are collected by a relay, not by the chain. The chain checks every '
  + 'signature when the trade executes — a relay can hide an offer from you, but '
  + 'it cannot change one.';

/** What the reader is told when this deployment names no board. */
export const TICKET_BOARD_ABSENT_MESSAGE_V1 =
  'No offer board is configured for this deployment. You can still take an '
  + 'offer someone sends you directly.';

/** What the reader is told when the board is configured but silent. */
export const TICKET_BOARD_UNREACHABLE_MESSAGE_V1 =
  'The offer board did not answer. Nothing is wrong with this market — you can '
  + 'still paste a ticket.';

/** List this deployment's board, through the browser's own `fetch`. */
export function listConfiguredBoardOffersV1(
  config: TicketBoardConfigV1,
  query: TicketBoardQueryV1,
): Promise<TicketBoardListingV1> {
  return listBoardOffersV1(config, query, globalThis.fetch);
}

/** Publish one authored ticket to this deployment's board. */
export function postConfiguredBoardOfferV1(
  config: TicketBoardConfigV1,
  ticketText: string,
): Promise<TicketBoardPostV1> {
  return postBoardOfferV1(config, ticketText, globalThis.fetch);
}

/** Ask this deployment's board whether it is there. */
export function configuredBoardHealthV1(
  config: TicketBoardConfigV1,
): Promise<TicketBoardHealthV1> {
  return boardHealthV1(config, globalThis.fetch);
}

export {
  TicketBoardError,
  type BoardOfferV1,
  type BoardRefusalV1,
  type TicketBoardConfigV1,
  type TicketBoardHealthV1,
  type TicketBoardListingV1,
  type TicketBoardPostV1,
  type TicketBoardQueryV1,
};
