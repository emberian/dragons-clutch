/**
 * Refusal rendering: from a bare `custom program error` u32 to the program,
 * name, and meaning the protocol registered for it.
 *
 * The scheme (decision 0007; `crates/dclutch-refusal-registry` is the
 * authority): `band = code >> 12`, each band is 0x1000 codes wide, and band 0
 * is never allocated — so a custom code below 0x1000 is, by construction, not
 * a dClutch refusal, and this module says so rather than guessing. A refusal
 * is the protocol working: fail-closed means an input that does not
 * authenticate exactly is refused with a code that names the program and the
 * reason, and no partial effect survives.
 */
import { REFUSAL_BANDS_V1, REFUSAL_BAND_SPAN, REFUSAL_CODES_V1 } from './generated/refusalRegistryV1';
import type { RefusalBandV1, RefusalCodeV1 } from './generated/refusalRegistryV1';

export type { RefusalBandV1, RefusalCodeV1 };

const codeIndex: ReadonlyMap<number, RefusalCodeV1> = new Map(REFUSAL_CODES_V1.map((entry) => [entry.code, entry]));

/** The band that owns a code, or null when the code is not first-party. */
export function refusalBand(code: number): RefusalBandV1 | null {
  if (!Number.isInteger(code) || code < 0 || code > 0xffff_ffff) return null;
  return REFUSAL_BANDS_V1.find((band) => code >= band.base && code < band.base + REFUSAL_BAND_SPAN) ?? null;
}

/** The registered record for one exact code, or null when unregistered. */
export function refusalCode(code: number): RefusalCodeV1 | null {
  return codeIndex.get(code) ?? null;
}

/**
 * The one meaning every reader program registered for a superseded release.
 *
 * Decision 0012 gave eight programs a `ReleaseSuperseded` discriminant — one
 * per band, `registry 0x100D` through `resolution 0x8014` — and they are eight
 * names for a single event: the release's pinned deployment slot moved, so the
 * substrate was upgraded.
 *
 * A client reaches that verdict on its own, off chain, from the same slot
 * comparison the chain makes: it reads the ProgramData, sees a slot strictly
 * later than the one the artifact release pins, and knows the transaction it
 * was about to build would be refused. What it must NOT do is invent its own
 * words for that event — a browser that explains a supersession differently
 * from the program that raises it is a second authority on what the protocol
 * means. So the sentence is read out of the generated registry, whose chain of
 * custody is enum doc comment -> census -> `docs/reference/refusals.md` ->
 * this module, with a verify gate at every arrow.
 *
 * Fail-closed in both directions: no rows at all, or rows that disagree with
 * each other, throws rather than picking one.
 */
export function releaseSupersededMeaningV1(): string {
  const rows = REFUSAL_CODES_V1.filter((entry) => entry.name.endsWith('::ReleaseSuperseded'));
  if (rows.length === 0) throw new Error('the generated refusal registry carries no ReleaseSuperseded row; regenerate it');
  const meanings = new Set(rows.map((entry) => entry.meaning));
  if (meanings.size !== 1) throw new Error(`the ${rows.length} registered ReleaseSuperseded rows carry ${meanings.size} different meanings; the refusal registry disagrees with itself about decision 0012`);
  const [meaning] = meanings;
  if (meaning === undefined) throw new Error('unreachable');
  return meaning;
}

export interface RenderedRefusalV1 {
  /** `first-party` when a registered band owns the code; `foreign` below 0x1000 or in no band. */
  readonly origin: 'first-party' | 'foreign';
  /** One line naming what is known: program, enum variant, and meaning when registered. */
  readonly text: string;
}

/**
 * Render one custom code the way a terminal should show it.
 *
 * A code inside a registered band but without a row in the reference is
 * rendered as the program plus the bare code — that combination names the
 * owner honestly while saying the reference does not know the variant, which
 * is exactly the state of the world when a program adds a refusal before the
 * reference regenerates.
 */
export function renderRefusal(code: number): RenderedRefusalV1 {
  const hex = `0x${code.toString(16).toUpperCase()}`;
  const band = refusalBand(code);
  if (band === null) {
    const reason = code < 0x1000 ? 'band 0 is never allocated' : 'no registered band owns it';
    return Object.freeze({ origin: 'foreign', text: `${hex} is not a dClutch refusal (${reason}); it was raised by a foreign program in the transaction` });
  }
  const entry = refusalCode(code);
  if (entry === null) {
    return Object.freeze({ origin: 'first-party', text: `${band.label} (${band.package}) refused with ${hex}; the code sits in its band but the generated reference has no row for it — regenerate the reference` });
  }
  return Object.freeze({ origin: 'first-party', text: `${entry.band} refused: ${entry.name} (${hex}) — ${entry.meaning}` });
}

/**
 * Pull the custom code out of a JSON-RPC transaction error, when there is one.
 *
 * The shape the RPC returns is `{"InstructionError":[index,{"Custom":code}]}`.
 * Anything else — a non-custom instruction error, a different failure kind —
 * returns null, and the caller should show the raw error text instead of a
 * name this module cannot stand behind.
 */
export function customCodeFromTransactionError(error: unknown): number | null {
  if (typeof error !== 'object' || error === null || Array.isArray(error)) return null;
  const instruction = (error as Record<string, unknown>).InstructionError;
  if (!Array.isArray(instruction) || instruction.length !== 2) return null;
  const detail = instruction[1];
  if (typeof detail !== 'object' || detail === null || Array.isArray(detail)) return null;
  const custom = (detail as Record<string, unknown>).Custom;
  if (typeof custom !== 'number' || !Number.isInteger(custom) || custom < 0) return null;
  return custom;
}
