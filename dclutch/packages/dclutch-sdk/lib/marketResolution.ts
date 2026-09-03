import { PublicKey } from '@solana/web3.js';

import {
  bindTerminalResolutionCertificateV2,
  decodeResolutionCertificateV2,
  type ResolutionCertificateKindV2,
  type ResolutionCertificateV2,
} from './resolutionCertificateV2';
import { type MarketQuestionV1 } from './marketQuestion';
import { type DecodedMarketDiscoveryCardV1 } from './marketDiscovery';
import { type SolanaRpcClient } from './rpc';

/**
 * HOW A RESOLVED MARKET GOT ITS ANSWER, read rather than told.
 *
 * A Terminal Market page already said WHICH claim won and what that leaves
 * each holder holding. It could not say the one thing a reader of a prediction
 * market actually asks: *on what*. Cohort-13 resolved with no observation at
 * all -- a source-failure certificate, the founder keeping the pot -- and
 * cohort-14b resolved on a price read inside its own window. Those two are
 * opposite outcomes and the page rendered them with the same four words,
 * because everything that distinguishes them lives in the certificate and
 * nothing was reading it.
 *
 * IT IS ALREADY ADDRESSABLE. The Market's `terminal_receipt` slot -- offset
 * 328, surfaced as `settlement.receiptId` -- holds THE CERTIFICATE ACCOUNT'S
 * OWN ADDRESS, not a content digest. The explorer's lens says as much and then
 * files it under `address: null` with the note "no schema pairing is declared
 * for it", which is true of the RECORD layer and had the side effect that
 * nothing anywhere followed the 32 bytes. Following them is one account read.
 *
 * NOTHING HERE DECIDES ANYTHING. `decodeResolutionCertificateV2` is the
 * hostile decoder that already owns the ABI and
 * `bindTerminalResolutionCertificateV2` is the join that already owns the
 * agreement between a certificate and Core's own terminal authority. This
 * module supplies the address, the owner check, and the projection a page can
 * render; every refusal it can produce comes from one of those two.
 */

/** Whether the observation the certificate carries fell inside the window. */
export type ObservationWindowStandingV1 = 'inside' | 'before' | 'after' | 'unwindowed';

export type MarketObservationV1 = Readonly<{
  /** The provider's own publish time, seconds, as the certificate recorded it. */
  atUnixSeconds: bigint;
  /** The observed value as an exact integer ratio; never a float here. */
  numerator: bigint;
  denominator: bigint;
  /** The ratio rendered exactly in decimal, or null when it does not terminate. */
  decimal: string | null;
  /** Where it fell relative to the market's own settlement window. */
  standing: ObservationWindowStandingV1;
}>;

export type MarketResolutionV1 =
  | Readonly<{ status: 'not-terminal'; reason: string }>
  | Readonly<{ status: 'refused'; certificate: string | null; reason: string }>
  | Readonly<{
    status: 'authenticated';
    observedSlot: string;
    /** The certificate account the Market itself names. */
    certificate: string;
    kind: ResolutionCertificateKindV2;
    /**
     * Whether a data source actually reported.
     *
     * `resolution-success` is kind 1 and means the observation below exists.
     * `resolution-failure` is kind 4 and means it does not, and that the
     * market settled on the fallback outcome it named and prepaid for before
     * it opened. The runbook's sentence for the second is that shipping it
     * twice would make an oracle outage into founder revenue a second time,
     * so the difference is not decoration.
     */
    sourceReported: boolean;
    /** The cut the chain committed to, equal to Core's own `terminal_winner`. */
    selector: number;
    /** Present exactly when `sourceReported`; a failure certificate has none. */
    observation: MarketObservationV1 | null;
    /** The provider evidence the certificate pins, hex, or null on a failure. */
    providerEvidenceId: string | null;
    /** The source material the certificate pins, hex; always present. */
    sourceMaterialId: string;
    /** How many attempts the resolution took, from the certificate itself. */
    attemptIndex: number;
  }>;

/** The 32 bytes at the Market's terminal-receipt slot, as an address. */
export function terminalCertificateAddressV1(receiptId: string): string {
  if (!/^[0-9a-f]{64}$/.test(receiptId)) throw new Error('terminal receipt is not 32 bytes of hex');
  const bytes = Uint8Array.from(receiptId.match(/../g)!.map((pair) => Number.parseInt(pair, 16)));
  return new PublicKey(bytes).toBase58();
}

function hex(bytes: Uint8Array): string {
  return Array.from(bytes, (byte) => byte.toString(16).padStart(2, '0')).join('');
}

/**
 * An exact decimal for a ratio, or null when it does not terminate.
 *
 * A price is not a float here for the same reason atoms are not: the pair the
 * chain committed to is two integers, and a page that renders `100.36` should
 * be rendering a division that CLOSED, not a rounding. So the long division
 * runs to at most twelve places and answers null rather than truncating -- and
 * the caller still has the numerator and the denominator to show instead.
 */
export function exactRatioDecimalV1(numerator: bigint, denominator: bigint, maxPlaces = 12): string | null {
  if (denominator <= 0n) return null;
  const negative = numerator < 0n;
  let remainder = negative ? -numerator : numerator;
  const whole = remainder / denominator;
  remainder %= denominator;
  let places = '';
  for (let index = 0; index < maxPlaces && remainder !== 0n; index += 1) {
    remainder *= 10n;
    places += (remainder / denominator).toString();
    remainder %= denominator;
  }
  if (remainder !== 0n) return null;
  const sign = negative ? '-' : '';
  return places === '' ? `${sign}${whole}` : `${sign}${whole}.${places}`;
}

function standing(at: bigint, window: MarketQuestionV1['window']): ObservationWindowStandingV1 {
  if (window === null) return 'unwindowed';
  if (at < window.startUnixSeconds) return 'before';
  if (at > window.endUnixSeconds) return 'after';
  return 'inside';
}

/**
 * Read and authenticate the certificate a Terminal Market names.
 *
 * `question` is optional and only ever supplies the WINDOW: without it the
 * observation still reads and its standing is `unwindowed`, which is the
 * honest word for "this reader was not given the market's window", never a
 * claim that the market has none.
 */
export async function inspectMarketResolutionV1(
  client: Pick<SolanaRpcClient, 'accountInfo'>,
  request: Readonly<{
    card: DecodedMarketDiscoveryCardV1;
    resolutionProgramId: string;
    floorSlot: string;
    question?: MarketQuestionV1 | null;
  }>,
): Promise<MarketResolutionV1> {
  const { card, resolutionProgramId, floorSlot } = request;
  if (card.settlement.status !== 'terminal') {
    return Object.freeze({ status: 'not-terminal', reason: 'This market has not reached a terminal answer, so no resolution certificate exists to read.' });
  }
  let address: string;
  try {
    address = terminalCertificateAddressV1(card.settlement.receiptId);
  } catch {
    return Object.freeze({ status: 'refused', certificate: null, reason: 'The Market names a terminal receipt this reader cannot read as an account address.' });
  }
  let observation;
  try {
    observation = await client.accountInfo(address, floorSlot);
  } catch (error) {
    return Object.freeze({ status: 'refused', certificate: address, reason: `The certificate account did not read: ${error instanceof Error ? error.message : String(error)}` });
  }
  const account = observation.account;
  if (account === null) {
    return Object.freeze({ status: 'refused', certificate: address, reason: 'The Market names a certificate account that does not exist at this finalized floor.' });
  }
  // OWNER FIRST. A certificate is a Resolution-owned account, and decoding
  // bytes from an account some other program wrote would be authenticating a
  // shape rather than an authority.
  if (account.owner !== resolutionProgramId) {
    return Object.freeze({ status: 'refused', certificate: address, reason: `The certificate account is owned by ${account.owner}, not this deployment's Resolution program.` });
  }
  let certificate: ResolutionCertificateV2;
  try {
    certificate = decodeResolutionCertificateV2(account.data);
  } catch (error) {
    return Object.freeze({ status: 'refused', certificate: address, reason: `The certificate did not decode: ${error instanceof Error ? error.message : String(error)}` });
  }
  // And then the JOIN, which is the check that matters: a certificate that
  // decodes is still only a certificate until it agrees with the Market that
  // named it about the market, the generation, the selector and the receipt.
  const outcomeCount = card.liability.status === 'bound' ? card.liability.supplyAtoms.length : 0;
  if (outcomeCount < 2) {
    return Object.freeze({ status: 'refused', certificate: address, reason: 'The Claims aggregate did not read, so the certificate cannot be joined to an outcome count and nothing about the answer may be stated.' });
  }
  try {
    bindTerminalResolutionCertificateV2(certificate, {
      receiptAccount: new PublicKey(address).toBytes(),
      market: new PublicKey(card.address).toBytes(),
      sourceMaterial: certificate.sourceMaterial,
      productRecordDigest: certificate.productRecordDigest,
      generation: BigInt(card.generation),
      selector: card.settlement.winner,
      outcomeCount,
    });
  } catch (error) {
    return Object.freeze({ status: 'refused', certificate: address, reason: `The certificate does not join this Market's own terminal authority: ${error instanceof Error ? error.message : String(error)}` });
  }
  const sourceReported = certificate.kind === 'resolution-success';
  return Object.freeze({
    status: 'authenticated',
    observedSlot: observation.slot,
    certificate: address,
    kind: certificate.kind,
    sourceReported,
    selector: certificate.selector,
    observation: sourceReported
      ? Object.freeze({
        atUnixSeconds: certificate.observedAt,
        numerator: certificate.resultNumerator,
        denominator: certificate.resultDenominator,
        decimal: exactRatioDecimalV1(certificate.resultNumerator, certificate.resultDenominator),
        standing: standing(certificate.observedAt, request.question?.window ?? null),
      })
      : null,
    providerEvidenceId: sourceReported ? hex(certificate.providerEvidence) : null,
    sourceMaterialId: hex(certificate.sourceMaterial),
    attemptIndex: certificate.attemptIndex,
  });
}

/**
 * What is left in the vault against what the winning claim is owed.
 *
 * THE REDEMPTION STATE IS ARITHMETIC THE PAGE ALREADY HAD THE TERMS FOR and
 * was not doing. A Terminal Market's required backing is measured against the
 * WINNING claim supply (`requiredBackingBasis`), and the Hoard's principal is
 * read off the vault token account in the same finalized round. Their
 * difference is exactly how much of the answer has been cashed in.
 *
 * `paid` is therefore not a claim that any particular wallet was paid -- the
 * Market records no recipient and this reader invents none. It says the vault
 * no longer holds what the winners are owed, which is the same fact from the
 * side the chain actually publishes.
 */
export type MarketRedemptionStateV1 =
  | Readonly<{ status: 'unread'; reason: string }>
  | Readonly<{
    status: 'read';
    owedAtoms: string;
    heldAtoms: string;
    redeemedAtoms: string;
    /** `none`, `partial` or `complete` -- never a guess between them. */
    progress: 'none' | 'partial' | 'complete';
  }>;

export function marketRedemptionStateV1(card: DecodedMarketDiscoveryCardV1): MarketRedemptionStateV1 {
  if (card.settlement.status !== 'terminal') {
    return Object.freeze({ status: 'unread', reason: 'Nothing can be cashed in until this market has an answer.' });
  }
  if (card.liability.status !== 'bound') {
    return Object.freeze({ status: 'unread', reason: 'The Claims aggregate did not read, so what the winners are owed is unknown.' });
  }
  if (card.hoard.status !== 'derived') {
    return Object.freeze({ status: 'unread', reason: 'The vault did not read, so how much is left in it is unknown.' });
  }
  const owed = BigInt(card.liability.requiredBackingAtoms);
  const held = BigInt(card.hoard.principalAtoms);
  // A vault holding MORE than the winners are owed is not a redemption at all
  // and must not read as a negative one.
  const redeemed = owed > held ? owed - held : 0n;
  return Object.freeze({
    status: 'read',
    owedAtoms: owed.toString(),
    heldAtoms: held.toString(),
    redeemedAtoms: redeemed.toString(),
    progress: redeemed === 0n ? 'none' : held === 0n ? 'complete' : 'partial',
  });
}
