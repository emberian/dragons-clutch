import { PublicKey } from '@solana/web3.js';

import { denominationUnitV1, formatQuantityV1, type DenominationV1 } from '@/lib/quantity';

/**
 * What a console field knows about the text currently inside it.
 *
 * `ArtifactInput` already established this shape for exactly one field type --
 * its `fact` line reports the bytes it decoded, their SHA-256, and how far off
 * the expected width they are. Its doc comment states the rule the rest of the
 * consoles never got:
 *
 *   "if a console asks you to paste something and you don't know where it
 *    comes from, that's a bug in the console."
 *
 * A field that cannot say what it just read is the same bug wearing a
 * different hat. So every operator field reads its own text and reports one of
 * three states, and the reading is a TOTAL FUNCTION of the text -- not a side
 * effect of a blur event.
 *
 * That purity is deliberate and it is what makes the honesty testable. A
 * validator that only runs on `onBlur` can only be checked by simulating
 * events; this repo's component tests render to static markup and assert on
 * strings (`renderToStaticMarkup` + `toContain`, no DOM queries anywhere), so
 * an event-driven validator would be exactly the kind of behaviour that ships
 * unverified. `lib/tradeFlowSteps.ts` states the same rule for the flow:
 * "assignment done inline in JSX is assignment nobody can test."
 *
 * Purity also settles WHEN a refusal appears, without an event: a field
 * refuses only text that is BOTH non-empty and wrong. An empty field is not
 * yet wrong, and no field type here passes through an invalid spelling on the
 * way to a valid one -- "12" en route to "1200" is already a legal reading of
 * atoms, and addresses, digests and JSON arrive by paste. There is no refusal
 * to flash.
 *
 * The refusal shape is `lib/tradeFlowRefusals.ts`'s, deliberately: **the
 * remedy comes first and the detail survives whole**, each in its own element,
 * because "a refusal that loses its own words has been turned into a mood."
 */
export type FieldReadingV1 =
  | Readonly<{ state: 'empty' }>
  | Readonly<{ state: 'resolved'; identity: string }>
  | Readonly<{ state: 'refused'; remedy: string; detail: string }>;

/** base58 as Bitcoin and Solana spell it: no 0, no O, no I, no l. */
const BASE58_ALPHABET_V1 = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz';
const HEX_ALPHABET_V1 = '0123456789abcdefABCDEF';

/** The first character not drawn from `alphabet`, with its 1-based position. */
function strayCharacterV1(
  text: string,
  alphabet: string,
): Readonly<{ character: string; position: number }> | null {
  for (let index = 0; index < text.length; index += 1) {
    const character = text[index]!;
    if (!alphabet.includes(character)) return { character, position: index + 1 };
  }
  return null;
}

/** `7Yk4…m3pQ` -- enough of an address to recognise, never enough to retype. */
export function compactAddressV1(address: string): string {
  return address.length <= 12 ? address : `${address.slice(0, 4)}…${address.slice(-4)}`;
}

/**
 * A 32-byte account address, base58.
 *
 * The parse is the one `lib/localSuccessor.ts` already runs on every address
 * it decodes. The refusals are this module's own, because a decoder throwing
 * "Invalid public key input" tells a reader nothing they can act on -- and
 * that exact string is what `/workbench` shows today for a mistyped program
 * address, straight out of web3.js.
 *
 * `identify` lets the page name the address it just resolved -- "the
 * activation cache derived in step 02", "the wallet connected in step 03". A
 * field that can say which known account you pasted turns a 44-character
 * string back into a thing with a name.
 */
export function readPubkeyV1(
  text: string,
  identify?: (address: string) => string | null,
): FieldReadingV1 {
  const trimmed = text.trim();
  if (trimmed === '') return { state: 'empty' };

  const stray = strayCharacterV1(trimmed, BASE58_ALPHABET_V1);
  if (stray !== null) {
    return {
      state: 'refused',
      remedy: 'Paste the address as base58.',
      detail: `Character ${stray.position} is ${JSON.stringify(stray.character)}, which base58 does not use: it omits 0, O, I and l so they cannot be misread for each other.`,
    };
  }

  let decoded: PublicKey;
  try {
    decoded = new PublicKey(trimmed);
  } catch {
    return {
      state: 'refused',
      remedy: 'Paste a 32-byte account address.',
      detail: `This is ${trimmed.length} base58 characters and does not decode to 32 bytes. An address the chain prints is 43 or 44 characters, or 32 when it is mostly zeroes.`,
    };
  }

  // No canonical-spelling refusal follows, and that is a measured claim rather
  // than an omission: base58's encoding of a FIXED 32-byte width is injective,
  // so a string that decodes to 32 bytes is already the only spelling of them.
  // (`lib/localSuccessor.ts` and `lib/deployments.ts` do carry round-trip
  // checks, correctly -- they decode values out of JSON, where the input may
  // not be an address at all. A field bound to a text input has already been
  // narrowed.) The property is pinned in this module's test, so a future
  // decoder swap that weakens it fails loudly rather than silently admitting
  // two spellings of one account.
  const canonical = decoded.toBase58();
  const known = identify?.(canonical) ?? null;
  return {
    state: 'resolved',
    identity: known === null
      ? `32 bytes · ${compactAddressV1(canonical)}`
      : `32 bytes · ${compactAddressV1(canonical)} · ${known}`,
  };
}

/**
 * A 32-byte digest, hex.
 *
 * Roots, digests and commitments all arrive this way, and they are the field
 * type most often confused with an address -- both are "a long string naming
 * 32 bytes", and pasting one where the other belongs is a malformation these
 * consoles cannot currently tell you about. So a hex field handed base58 says
 * which of the two it is holding.
 */
export function readHex64V1(text: string): FieldReadingV1 {
  const trimmed = text.trim();
  if (trimmed === '') return { state: 'empty' };

  const stray = strayCharacterV1(trimmed, HEX_ALPHABET_V1);
  if (stray !== null) {
    if (strayCharacterV1(trimmed, BASE58_ALPHABET_V1) === null) {
      return {
        state: 'refused',
        remedy: 'Paste the 64-character hex digest.',
        detail: 'This is a base58 address, which names an account rather than a digest. The two are both 32 bytes and are not interchangeable.',
      };
    }
    return {
      state: 'refused',
      remedy: 'Paste the digest as hex.',
      detail: `Character ${stray.position} is ${JSON.stringify(stray.character)}, and hex uses only the digits 0-9 and the letters a-f.`,
    };
  }
  if (trimmed.length !== 64) {
    return {
      state: 'refused',
      remedy: 'Paste all 64 hex characters.',
      detail: `This is ${trimmed.length}, which is ${trimmed.length < 64 ? `${64 - trimmed.length} short of` : `${trimmed.length - 64} more than`} the 64 a 32-byte digest is written with.`,
    };
  }
  return { state: 'resolved', identity: `32 bytes · ${trimmed.slice(0, 8)}…${trimmed.slice(-8)}` };
}

/**
 * An amount in atoms -- the protocol's own integer, and on an operator console
 * the PRIMARY rendering rather than the glossed one.
 *
 * FLOWFUL_IA_V1 §5.4 draws that line: humanized amounts are for decision
 * surfaces, and "operator surfaces keep raw atoms as the primary rendering ...
 * their readers are checking arithmetic against a chain, and a thousands
 * separator in a byte offset is a hazard." So this field is typed in atoms and
 * the humanized amount is what it reports back, which is the exact inverse of
 * the trade panel and correct for the same reason.
 *
 * The scaling is `formatQuantityV1`'s, never this module's, and never a float.
 */
export function readAtomsV1(text: string, denomination: DenominationV1): FieldReadingV1 {
  const trimmed = text.trim();
  if (trimmed === '') return { state: 'empty' };

  const cleaned = trimmed.replace(/,/g, '');
  if (/^[0-9]*\.[0-9]*$/.test(cleaned)) {
    return {
      state: 'refused',
      remedy: 'Enter this amount in whole atoms.',
      detail: 'An atom is the smallest unit the protocol carries and it does not divide, so a decimal point here has nothing left to name.',
    };
  }
  if (cleaned.startsWith('-')) {
    return {
      state: 'refused',
      remedy: 'Enter a positive amount in atoms.',
      detail: 'The protocol carries these as unsigned integers, so a negative amount has no wire encoding at all.',
    };
  }
  if (!/^[0-9]+$/.test(cleaned)) {
    return {
      state: 'refused',
      remedy: 'Enter the amount as digits only.',
      detail: 'Atoms are one whole number, with no unit suffix, no exponent and no spaces.',
    };
  }

  const atoms = BigInt(cleaned);
  if (atoms === 0n) {
    return {
      state: 'refused',
      remedy: 'Enter an amount above zero.',
      detail: 'A zero-atom instruction still spends a slot and a signature, and moves nothing.',
    };
  }
  if (atoms > 0xffff_ffff_ffff_ffffn) {
    return {
      state: 'refused',
      remedy: 'Enter an amount inside u64.',
      detail: 'This exceeds 18,446,744,073,709,551,615, the widest value the protocol amount fields carry.',
    };
  }

  const quantity = formatQuantityV1(atoms, denomination);
  const unit = denominationUnitV1(denomination);
  return {
    state: 'resolved',
    identity: quantity.humanized
      ? `${quantity.atoms} atoms · ${quantity.display} ${unit} at ${denomination.decimals} decimals`
      : `${quantity.atoms} atoms · this mint never published a display precision, so there is no ${unit} amount to show`,
  };
}

/** One row of an evidence summary: what the console found, and where. */
export type EvidenceRowV1 = Readonly<{ term: string; detail: string }>;

export type EvidenceReadingV1 =
  | Readonly<{ state: 'empty' }>
  | Readonly<{ state: 'resolved'; identity: string; rows: ReadonlyArray<EvidenceRowV1> }>
  | Readonly<{ state: 'refused'; remedy: string; detail: string }>;

/**
 * A refusal a `summarize` callback raises, carrying its own remedy.
 *
 * A thrown `Error` has one message, and this module needs two parts. Rather
 * than split a caller's sentence on a delimiter -- which would guess -- the
 * caller says which half is which.
 */
export class FieldRefusalV1 extends Error {
  readonly remedy: string;
  readonly detail: string;
  constructor(remedy: string, detail: string) {
    super(`${remedy} ${detail}`);
    this.name = 'FieldRefusalV1';
    this.remedy = remedy;
    this.detail = detail;
  }
}

/**
 * A JSON artifact some producer wrote, pasted into a browser.
 *
 * Today every one of these is a bare textarea: paste a blob, press a button,
 * and find out from a thrown error whether you pasted the right file.
 * `components/trade/TicketCard.tsx` already knows better -- it renders a
 * pasted ticket AS a ticket, with a drawer holding all fourteen signed fields
 * -- and this generalises that: on valid JSON the field summarises what it is
 * holding, so a reader confirms the artifact before spending a signature on
 * it, exactly as `ArtifactInput` prints bytes and a SHA-256 on intake.
 *
 * `summarize` is the caller's, because only the console knows what shape it
 * asked for. It may throw `FieldRefusalV1` to refuse a well-formed JSON
 * document that is the wrong artifact.
 */
export function readEvidenceV1(
  text: string,
  summarize: (parsed: unknown) => Readonly<{ identity: string; rows: ReadonlyArray<EvidenceRowV1> }>,
): EvidenceReadingV1 {
  const trimmed = text.trim();
  if (trimmed === '') return { state: 'empty' };

  let parsed: unknown;
  try {
    parsed = JSON.parse(trimmed);
  } catch (error) {
    return {
      state: 'refused',
      remedy: 'Paste the whole file, from its opening brace to its closing one.',
      detail: `This is not complete JSON yet: ${error instanceof Error ? error.message : String(error)}.`,
    };
  }

  try {
    const summary = summarize(parsed);
    return { state: 'resolved', identity: summary.identity, rows: summary.rows };
  } catch (error) {
    if (error instanceof FieldRefusalV1) {
      return { state: 'refused', remedy: error.remedy, detail: error.detail };
    }
    return {
      state: 'refused',
      remedy: 'Paste the artifact this field asks for.',
      detail: error instanceof Error
        ? error.message
        : 'This parses as JSON but is not the shape this field reads.',
    };
  }
}

/**
 * One of a known set.
 *
 * Enumerable choices are the field type today's consoles get closest to right
 * -- several are already a `<select>` -- and the refusal exists for the ones
 * that are not, where a role or a market is typed as free text and the chain
 * is the first thing to notice the typo.
 */
export function readEnumV1(text: string, choices: ReadonlyArray<string>): FieldReadingV1 {
  const trimmed = text.trim();
  if (trimmed === '') return { state: 'empty' };
  if (!choices.includes(trimmed)) {
    return {
      state: 'refused',
      remedy: `Choose one of ${choices.join(', ')}.`,
      detail: `${JSON.stringify(trimmed)} is not one of them, and this field is the whole list.`,
    };
  }
  return { state: 'resolved', identity: trimmed };
}

/**
 * A plain unsigned integer -- the most common numeric type on these consoles
 * and, before this, the least served.
 *
 * Compute-unit limits, market generations, scalar identities, knot
 * denominators, outcome indices and slots are all this type, and all of them
 * today accept any keystroke and defer to a decoder. `inputMode="numeric"`
 * appears on eight of them and constrains nothing -- it is a keyboard hint.
 *
 * Distinct from `readAtomsV1`: atoms are a QUANTITY of something a mint
 * denominates, and carry a display precision. These are counts and
 * identifiers, which have a noun but no decimals, so the reading names the
 * noun rather than inventing a scale for it.
 */
export function readU64V1(
  text: string,
  bound: Readonly<{ noun: string; min?: bigint; max?: bigint }>,
): FieldReadingV1 {
  const trimmed = text.trim();
  if (trimmed === '') return { state: 'empty' };

  const cleaned = trimmed.replace(/,/g, '');
  if (cleaned.startsWith('-')) {
    return {
      state: 'refused',
      remedy: `Enter a positive ${bound.noun}.`,
      detail: 'The protocol carries this as an unsigned integer, so a negative value has no wire encoding.',
    };
  }
  if (!/^[0-9]+$/.test(cleaned)) {
    return {
      state: 'refused',
      remedy: `Enter the ${bound.noun} as digits only.`,
      detail: 'This is one whole number, with no decimal point, no suffix and no exponent.',
    };
  }

  const value = BigInt(cleaned);
  const min = bound.min ?? 0n;
  const max = bound.max ?? 0xffff_ffff_ffff_ffffn;
  if (value < min || value > max) {
    return {
      state: 'refused',
      remedy: `Enter a ${bound.noun} within ${min.toLocaleString('en-US')} to ${max.toLocaleString('en-US')}.`,
      detail: `This is ${value.toLocaleString('en-US')}, ${value < min ? 'below' : 'above'} the range the protocol accepts here.`,
    };
  }
  return { state: 'resolved', identity: `${value.toString()} · ${value.toLocaleString('en-US')} ${bound.noun}` };
}

/**
 * A JSON-RPC endpoint.
 *
 * Seven fields across the consoles read one of these, and every one of them
 * answers a malformed value with the URL constructor's own `Invalid URL` --
 * `new URL` is unguarded at `lib/rpc.ts` and again at `lib/deployments.ts`.
 * That string is the single most-reached unauthored refusal on the site, and
 * it tells a reader nothing about what an endpoint is supposed to look like.
 *
 * The scheme check is the one `lib/rpc.ts` already enforces; what is added is
 * that the field enforces it before a click rather than after a round trip.
 */
export function readEndpointV1(text: string): FieldReadingV1 {
  const trimmed = text.trim();
  if (trimmed === '') return { state: 'empty' };

  let url: URL;
  try {
    url = new URL(trimmed);
  } catch {
    return {
      state: 'refused',
      remedy: 'Enter the endpoint as a full URL, scheme included.',
      detail: `This is not one: a JSON-RPC endpoint looks like http://127.0.0.1:20890 or https://api.devnet.solana.com.`,
    };
  }
  if (url.protocol !== 'http:' && url.protocol !== 'https:') {
    return {
      state: 'refused',
      remedy: 'Enter an http or https endpoint.',
      detail: `This one is ${url.protocol.replace(':', '')}, and the RPC client speaks only http and https.`,
    };
  }
  return {
    state: 'resolved',
    identity: `${url.protocol.replace(':', '')} · ${url.host}${url.pathname === '/' ? '' : url.pathname}`,
  };
}
