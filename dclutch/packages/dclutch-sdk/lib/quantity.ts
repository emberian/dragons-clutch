import { formatAtomsV1 } from './marketDiscovery';


/**
 * The units policy, in one module.
 *
 * Every quantity a human decides on is shown in the collateral's display
 * denomination, with thousands separators. The raw atoms are one hover or one
 * drawer away, ALWAYS: `formatQuantityV1` returns `atoms` beside `display` for
 * exactly that reason, and no caller may drop it. A humanized number is never
 * the only number on screen.
 *
 * This module WRAPS `formatAtomsV1`; it does not replace it. That function
 * stays the exact, float-free atom -> decimal-string converter it already is,
 * and it keeps doing the arithmetic. What is added here is grouping, the
 * unknown-decimals path, and the exact twin every render site must carry.
 *
 * Non-negotiable, and enforced by reading this file: no `Number`, no
 * `parseFloat`, no `toFixed`. `bigint` and string manipulation only, all the
 * way down. Grouping is applied to the integer part AFTER the atom/decimal
 * split, so it can never perturb the value. There is no "compact" mode --
 * `1.2M` is a rounded number wearing a decision's clothes.
 */

/** A resolved display denomination for one Market's collateral. */
export type DenominationV1 = Readonly<{
  /** The mint's own decimals byte, chain-read. Null when unread or unauthenticated. */
  decimals: number | null;
  /** Editorial unit label, or null. NEVER invented. */
  unit: string | null;
  /** The collateral mint, for provenance and the drawer. */
  mint: string;
}>;

/** What one quantity reads as, and the exact integer it never stops being. */
export type QuantityV1 = Readonly<{
  /** What a person reads. e.g. "1,250.5" or, decimals-unknown, "500,000,000". */
  display: string;
  /** The exact integer, always. e.g. "500000000". Never omitted. */
  atoms: string;
  /** True when `display` is scaled atoms; false when decimals were unknown. */
  humanized: boolean;
  /** One title/tooltip string carrying the whole truth. */
  title: string;
}>;

/**
 * The collateral token has no name anywhere: no symbol on chain, no metadata
 * read, nothing in the registry. So none is invented. Absent an editorial
 * entry, the unit is the word itself, and the mint address stays one click
 * away in every case.
 */
export const UNNAMED_COLLATERAL_UNIT_V1 = 'collateral';

/** The unit label to print beside a humanized quantity. Never a guessed ticker. */
export function denominationUnitV1(denomination: DenominationV1): string {
  return denomination.unit ?? UNNAMED_COLLATERAL_UNIT_V1;
}

/** The denomination for a Market whose collateral mint never authenticated. */
export function unreadDenominationV1(mint: string): DenominationV1 {
  return Object.freeze({ decimals: null, unit: null, mint });
}

/**
 * Group an integer's digits in threes, without touching its value.
 *
 * Operates on the digit string only, and only on the integer part -- the
 * fractional part is never grouped, never padded, and never rounded. A leading
 * sign is carried across rather than grouped into.
 */
function groupIntegerDigitsV1(digits: string): string {
  const negative = digits.startsWith('-');
  const magnitude = negative ? digits.slice(1) : digits;
  let grouped = '';
  for (let end = magnitude.length; end > 0; end -= 3) {
    const start = end - 3 > 0 ? end - 3 : 0;
    grouped = grouped === '' ? magnitude.slice(start, end) : `${magnitude.slice(start, end)},${grouped}`;
  }
  return negative ? `-${grouped}` : grouped;
}

/**
 * Humanize an exact atom count for a reader who is about to decide with it.
 *
 * Exact and float-free: delegates the atom -> decimal split to formatAtomsV1,
 * then groups the INTEGER part only.
 *
 * Fails OPEN to the truth: when `decimals` is null the atoms have no known
 * display scale, so the raw integer is returned, grouped, and the caller MUST
 * render it with the `atoms` suffix. A null decimals is never treated as 0 --
 * that would silently multiply every quantity on the page by a million.
 */
export function formatQuantityV1(
  atoms: bigint | string,
  denomination: DenominationV1,
): QuantityV1 {
  const exact = (typeof atoms === 'bigint' ? atoms : BigInt(atoms)).toString();
  const atomWord = exact === '1' ? 'atom' : 'atoms';
  const { decimals } = denomination;
  if (decimals === null) {
    return Object.freeze({
      display: groupIntegerDigitsV1(exact),
      atoms: exact,
      humanized: false,
      title: `${exact} ${atomWord} — this mint’s display precision was not read`,
    });
  }
  const scaled = formatAtomsV1(exact, decimals);
  const point = scaled.indexOf('.');
  const display = point === -1
    ? groupIntegerDigitsV1(scaled)
    : `${groupIntegerDigitsV1(scaled.slice(0, point))}${scaled.slice(point)}`;
  return Object.freeze({
    display,
    atoms: exact,
    humanized: true,
    title: `${exact} ${atomWord} at ${decimals} decimals`,
  });
}

/**
 * The exact twin of a humanized quantity, rendered VISIBLY rather than hidden
 * behind a hover: touch devices have no hover, and a humanized number must
 * never be the only number on screen. When the mint's precision was never
 * read the display is ALREADY the grouped raw integer, so this labels it as
 * atoms rather than restating it.
 *
 * Lives here rather than in one component because three surfaces now render
 * the twin -- the trade flow's steps, the ticket card, and the preview
 * receipt -- and a formatting rule that exists in three copies is a rule that
 * will shortly exist in three versions.
 */
export function exactTwinV1(quantity: QuantityV1, kind: string): string {
  return quantity.humanized
    ? `${quantity.atoms} ${kind} atoms`
    : `${kind} atoms, at a display precision this mint never published`;
}

/** The widest amount the protocol's u64 fields carry. */
const U64_MAX_V1 = 0xffff_ffff_ffff_ffffn;

/**
 * Read a quantity a person TYPED, in the display denomination, back to exact
 * atoms.
 *
 * This is the inverse of formatQuantityV1 and it exists because relabelling an
 * input without converting it is worse than never relabelling at all: a reader
 * who types 500 into a box marked "claims" and gets 500 atoms has been shown a
 * word and charged a different number. Every quantity the flow decides with
 * passes through here or through formatQuantityV1, and neither one uses a
 * float.
 *
 * Grouping separators are accepted on input because the site prints them on
 * output, and a reader who copies a value back in should not be punished for
 * it. Refusals are remedy-shaped and name the unit the reader is working in.
 */
export function parseQuantityV1(text: string, denomination: DenominationV1): bigint {
  const cleaned = text.trim().replace(/,/g, '');
  const { decimals } = denomination;
  const unit = denominationUnitV1(denomination);
  const match = /^([0-9]+)(?:\.([0-9]+))?$/.exec(cleaned);
  if (match === null) {
    throw new Error(decimals === null
      ? 'your size must be one positive whole number of claim atoms'
      : 'your size must be one positive amount of claims');
  }
  const whole = match[1]!;
  const fraction = match[2] ?? '';
  if (decimals === null) {
    if (fraction !== '') {
      throw new Error(`this mint never published a display precision, so a size here is counted in whole ${unit} atoms`);
    }
    return positiveAtomsV1(BigInt(whole));
  }
  if (fraction.length > decimals) {
    throw new Error(`your size is finer than one claim atom, this market's smallest tradeable unit (${decimals} decimals)`);
  }
  const scaled = BigInt(whole) * 10n ** BigInt(decimals) + BigInt(fraction.padEnd(decimals, '0') || '0');
  return positiveAtomsV1(scaled);
}

function positiveAtomsV1(atoms: bigint): bigint {
  if (atoms <= 0n) throw new Error('your size must be one positive amount of claims');
  // Byte-identical to the bound the panel has always enforced.
  if (atoms > U64_MAX_V1) throw new Error('your size exceeds the protocol’s u64 amount width');
  return atoms;
}

/** One claim's price, as the share of a full payout that claim costs. */
export type ClaimPriceV1 = Readonly<{
  /** What a person reads, cents on the unit. e.g. "35¢", or "≈33.3333¢". */
  display: string;
  /** The cents figure alone, ungarnished. e.g. "35". */
  cents: string;
  /** False when the ratio does not terminate at four decimals of a cent. */
  exact: boolean;
  /** The exact fraction, always. e.g. "350000 / 1000000". */
  fraction: string;
  /** One title/tooltip string carrying the whole truth. */
  title: string;
}>;

/**
 * Render `limitPrice / priceScale` as cents on one unit of collateral.
 *
 * `previewDirectInlineV3` refuses any `executionPrice > priceScale`, and
 * `gross = fill * price / priceScale`. So this ratio is ALWAYS in (0, 1]: the
 * share of one full payout that one claim costs. At a priceScale of 1000000, a
 * limitPrice of 350000 is 0.35 -- 35 cents on the unit, and equivalently this
 * market saying about 35% likely.
 *
 * `priceScale` itself is evidence, not a decision input, so it does not appear
 * in `display`; it stays exact in `fraction` and `title` for the drawer.
 *
 * Exact and float-free. Cents are computed as one BigInt quotient carrying
 * four decimal places of a cent; when the ratio does not terminate there the
 * display is marked approximate rather than being quietly rounded.
 */
export function formatClaimPriceV1(price: bigint | string, priceScale: bigint | string): ClaimPriceV1 {
  const numerator = typeof price === 'bigint' ? price : BigInt(price);
  const scale = typeof priceScale === 'bigint' ? priceScale : BigInt(priceScale);
  const fraction = `${numerator.toString()} / ${scale.toString()}`;
  if (scale <= 0n) {
    throw new Error('a claim price needs one positive price scale to be a share of anything');
  }
  // cents x 10^4 = 10^6 x price / priceScale. The quotient is exact when the
  // remainder is zero, and truncated -- never rounded -- when it is not.
  const tenThousandthsOfACent = (numerator * 1_000_000n) / scale;
  const exact = (numerator * 1_000_000n) % scale === 0n;
  const cents = formatAtomsV1(tenThousandthsOfACent, 4);
  return Object.freeze({
    display: `${exact ? '' : '≈'}${cents}¢`,
    cents,
    exact,
    fraction,
    title: exact
      ? `${cents}¢ on the unit — exactly ${fraction}`
      : `${fraction} on the unit, which does not terminate at four decimals of a cent`,
  });
}

/**
 * Read a price typed as cents on one full collateral unit into the Market's
 * exact scaled integer.
 *
 * This is deliberately stricter than a decimal parser. A displayed cents
 * value is a rational number, while Direct carries one integer `limitPrice`.
 * The conversion therefore succeeds only when
 *
 *     typed cents / 100 = limitPrice / priceScale
 *
 * exactly. No nearest tick, binary float, or hidden rounding boundary is
 * admitted. The remedy names the immutable price scale so a caller can show
 * the reader why a seemingly ordinary decimal was refused.
 */
export function parseClaimPriceV1(text: string, priceScale: bigint): bigint {
  const cleaned = text.trim();
  if (priceScale <= 0n || priceScale > U64_MAX_V1) {
    throw new Error('this market does not carry one positive u64 price scale');
  }
  const match = /^([0-9]+)(?:\.([0-9]+))?$/.exec(cleaned);
  if (match === null) {
    throw new Error('price must be one positive decimal number of cents, from more than 0 through 100');
  }
  const fraction = match[2] ?? '';
  // Keep the typed rational exactly. BigInt also lets a hostile, very wide
  // input fail at the protocol bound below rather than losing precision first.
  const decimalScale = 10n ** BigInt(fraction.length);
  const centsNumerator = BigInt(match[1]!) * decimalScale
    + BigInt(fraction === '' ? '0' : fraction);
  const denominator = 100n * decimalScale;
  const scaled = centsNumerator * priceScale;
  if (scaled % denominator !== 0n) {
    throw new Error(
      `that cents value is not exactly representable at this market's immutable ${priceScale.toString()} price scale; no rounding was applied`,
    );
  }
  const price = scaled / denominator;
  if (price <= 0n || price > priceScale) {
    throw new Error('price must be more than 0 and no more than 100 cents on one full collateral unit');
  }
  return price;
}

/**
 * The one inline gloss that explains the price scale, the first time a price
 * appears in the flow. It teaches the identity the numbers already carry:
 * cents on the unit and the market's implied percentage are the same figure.
 */
export function claimPriceGlossV1(price: ClaimPriceV1, denomination: DenominationV1): string {
  const unit = denominationUnitV1(denomination);
  return `Each claim pays 1 ${unit} if this outcome wins, nothing if it does not. So a price of ${price.display} is this market saying about ${price.cents}% likely.`;
}
