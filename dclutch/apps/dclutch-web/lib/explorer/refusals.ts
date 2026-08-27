/**
 * Refusals, rendered by name.
 *
 * A Solana `custom program error` is a bare `u32`. The runtime reports it
 * without saying which program produced it, and a CPI chain can carry several
 * programs' refusals through one transaction — so `Custom(6)` on its own is an
 * ambiguous number that a reader resolves by assuming which frame it came from.
 * Decision 0007 removed the ambiguity at the source: every dClutch program owns
 * a disjoint `0x1000`-wide band, `band = code >> 12`, and band 0 is never
 * allocated, so a code below `0x1000` is by construction NOT one of ours.
 *
 * This module is the browser twin of the gauntlet census's crediting logic
 * (`tools/gauntlet/census/src/ledger.rs`). It reads the same two facts off the
 * same log lines, by the same two rules:
 *
 *   - the CODE is the LAST `custom program error` in the log, because that is
 *     what the transaction error carries and a frame that catches a child's
 *     refusal and raises its own has the last word;
 *   - the PROGRAM is the FIRST frame to report that code, because a propagated
 *     refusal is re-reported by every frame it unwinds through and only the
 *     innermost one originated it.
 *
 * Every name and meaning comes from `lib/generated/routeCensus.ts`, which is
 * emitted from the census enumeration and the refusal registry. Nothing here
 * restates a code, a band, or a meaning.
 */
import {
  PROTOCOL_REFUSALS,
  REFUSAL_BAND_SHIFT,
  REFUSAL_BANDS,
  type ProtocolRefusal,
  type RefusalBand,
} from '../generated/routeCensus';

const BY_CODE: ReadonlyMap<number, ProtocolRefusal> = new Map(
  PROTOCOL_REFUSALS.map((refusal) => [refusal.code, refusal]),
);

/** The band that owns `code`, or `null` when no allocation covers it. */
export function bandForCode(code: number): RefusalBand | null {
  return REFUSAL_BANDS.find((band) => code >= band.base && code < band.base + band.span) ?? null;
}

/** The band index `code` falls in. Band 0 is never allocated to us. */
export function bandIndex(code: number): number {
  return code >>> REFUSAL_BAND_SHIFT;
}

/**
 * What a custom code means, as far as the protocol's own authorities say.
 *
 * The four dispositions are exhaustive and each says something different:
 *
 *   `named`    — the census enumerates this exact code; name and meaning known.
 *   `banded`   — the band is ours, the code is not enumerated. The program is
 *                known and the meaning is not. This is a real state: a program
 *                can raise a code the enumerator did not resolve.
 *   `foreign`  — band 0. By construction not a dClutch refusal; it belongs to
 *                SPL Token, the loader, the System program, or another
 *                third-party program, and this client will not guess which.
 *   `unbanded` — above band 0 but inside no allocated band. Not ours either,
 *                and worth saying so distinctly from `foreign`.
 */
export type RefusalAttribution =
  | Readonly<{ disposition: 'named'; code: number; band: RefusalBand; refusal: ProtocolRefusal }>
  | Readonly<{ disposition: 'banded'; code: number; band: RefusalBand }>
  | Readonly<{ disposition: 'foreign'; code: number }>
  | Readonly<{ disposition: 'unbanded'; code: number }>;

/** Attribute one custom program error code. */
export function attributeCustomCode(code: number): RefusalAttribution {
  if (!Number.isSafeInteger(code) || code < 0) {
    throw new Error('a custom program error code is a non-negative integer');
  }
  const band = bandForCode(code);
  if (band === null) {
    return Object.freeze(
      bandIndex(code) === 0
        ? ({ disposition: 'foreign', code } as const)
        : ({ disposition: 'unbanded', code } as const),
    );
  }
  const refusal = BY_CODE.get(code);
  return Object.freeze(
    refusal === undefined
      ? ({ disposition: 'banded', code, band } as const)
      : ({ disposition: 'named', code, band, refusal } as const),
  );
}

/** One line of prose for an attribution, honest about what is not known. */
export function describeAttribution(attribution: RefusalAttribution): string {
  switch (attribution.disposition) {
    case 'named':
      return attribution.refusal.meaning ?? `${attribution.refusal.id} — the enum declares no doc comment.`;
    case 'banded':
      return `Band ${hexCode(attribution.band.base)} belongs to ${attribution.band.package}, but the census enumerates no refusal at this code. The program is known; the meaning is not.`;
    case 'foreign':
      return 'Below 0x1000, which no dClutch band covers. This refusal came from a program outside the protocol — SPL Token, the loader, the System program, or another third party. This client will not guess which.';
    case 'unbanded':
      return `Band ${hexCode(bandIndex(attribution.code) << REFUSAL_BAND_SHIFT)} is not allocated to any dClutch program. This is not a first-party refusal.`;
  }
}

/** The short label a chip shows: the refusal's own name, or an honest stand-in. */
export function attributionTitle(attribution: RefusalAttribution): string {
  switch (attribution.disposition) {
    case 'named':
      return `${attribution.refusal.enumName}::${attribution.refusal.variant}`;
    case 'banded':
      return `${attribution.band.label} · unnamed code`;
    case 'foreign':
      return 'not a dClutch refusal';
    case 'unbanded':
      return 'unallocated band';
  }
}

/** Lowercase `0x`-prefixed hexadecimal, the form validator logs use. */
export function hexCode(code: number): string {
  return `0x${code.toString(16).toUpperCase()}`;
}

// --------------------------------------------------------------- log reading

/** A `Program <address> failed: custom program error: 0xN` line, parsed. */
type FailedFrame = Readonly<{ program: string; code: number }>;

const FAILED = /^Program (\S+) failed: custom program error: 0x([0-9a-fA-F]+)/;
const BARE = /custom program error: 0x([0-9a-fA-F]+)/;
const INVOKE = /^Program (\S+) invoke \[(\d+)\]$/;

function failedFrames(logs: ReadonlyArray<string>): ReadonlyArray<FailedFrame> {
  const frames: FailedFrame[] = [];
  for (const line of logs) {
    const match = FAILED.exec(line);
    if (match === null) continue;
    const code = Number.parseInt(match[2], 16);
    if (!Number.isSafeInteger(code)) continue;
    frames.push(Object.freeze({ program: match[1], code }));
  }
  return frames;
}

/** The custom code the chain reported, and the frame that originated it. */
export type ReportedRefusal = Readonly<{
  code: number;
  /** The program whose own `failed:` line first carried this code, if named. */
  program: string | null;
  attribution: RefusalAttribution;
  /** How the code was recovered, so a reader can weigh it. */
  source: 'log-frame' | 'log-line' | 'transaction-error';
}>;

/**
 * Read the reported refusal out of a transaction's logs and structured error.
 *
 * `error` is `meta.err` as the RPC returns it, e.g.
 * `{"InstructionError":[0,{"Custom":12294}]}`. It is the last resort: the logs
 * carry the frame attribution and the error does not.
 */
export function readReportedRefusal(
  logs: ReadonlyArray<string>,
  error: unknown,
): ReportedRefusal | null {
  const frames = failedFrames(logs);
  const last = frames.at(-1);
  if (last !== undefined) {
    const originator = frames.find((frame) => frame.code === last.code);
    return Object.freeze({
      code: last.code,
      program: originator?.program ?? null,
      attribution: attributeCustomCode(last.code),
      source: 'log-frame',
    });
  }

  for (let index = logs.length - 1; index >= 0; index -= 1) {
    const match = BARE.exec(logs[index]);
    if (match === null) continue;
    const code = Number.parseInt(match[1], 16);
    if (!Number.isSafeInteger(code)) continue;
    return Object.freeze({
      code,
      program: null,
      attribution: attributeCustomCode(code),
      source: 'log-line',
    });
  }

  const code = customCodeFromError(error);
  if (code === null) return null;
  return Object.freeze({
    code,
    program: null,
    attribution: attributeCustomCode(code),
    source: 'transaction-error',
  });
}

/** The `Custom` code inside a `meta.err`, or `null` when it carries none. */
export function customCodeFromError(error: unknown): number | null {
  const found = findCustom(error, 0);
  return found;
}

function findCustom(value: unknown, depth: number): number | null {
  if (depth > 8 || value === null || typeof value !== 'object') return null;
  if (Array.isArray(value)) {
    for (const entry of value) {
      const found = findCustom(entry, depth + 1);
      if (found !== null) return found;
    }
    return null;
  }
  const record = value as Record<string, unknown>;
  const custom = record.Custom;
  if (typeof custom === 'number' && Number.isSafeInteger(custom) && custom >= 0) return custom;
  for (const entry of Object.values(record)) {
    const found = findCustom(entry, depth + 1);
    if (found !== null) return found;
  }
  return null;
}

/**
 * A non-custom transaction error, rendered as the runtime's own words.
 *
 * The runtime's own refusals — a privilege escalation, a missing signature, an
 * account already in use — are not custom codes and have no band. Naming them
 * is the runtime's job, not this registry's, so this returns the error's own
 * discriminant verbatim rather than translating it.
 */
export function runtimeErrorLabel(error: unknown): string | null {
  if (error === null || error === undefined) return null;
  if (typeof error === 'string') return error;
  if (typeof error !== 'object') return null;
  if (Array.isArray(error)) {
    for (const entry of error) {
      const label = runtimeErrorLabel(entry);
      if (label !== null) return label;
    }
    return null;
  }
  const keys = Object.keys(error as Record<string, unknown>);
  if (keys.length === 0) return null;
  const [key] = keys;
  const inner = (error as Record<string, unknown>)[key];
  if (key === 'InstructionError' && Array.isArray(inner)) {
    const detail = runtimeErrorLabel(inner[1]);
    return detail === null ? key : `${key} #${String(inner[0])}: ${detail}`;
  }
  if (typeof inner === 'object' && inner !== null) {
    const detail = runtimeErrorLabel(inner);
    return detail === null ? key : `${key}: ${detail}`;
  }
  return key;
}

// ------------------------------------------------------------- invoked frames

/** One `Program <address> invoke [depth]` frame the chain's logs report. */
export type InvokedFrame = Readonly<{ program: string; depth: number; index: number }>;

/**
 * The invocation frames the chain's own logs report, in order.
 *
 * This is the chain's account of what ran, not the client's reconstruction of
 * it: a CPI the client did not predict still shows up here.
 */
export function invokedFrames(logs: ReadonlyArray<string>): ReadonlyArray<InvokedFrame> {
  const frames: InvokedFrame[] = [];
  logs.forEach((line, index) => {
    const match = INVOKE.exec(line);
    if (match === null) return;
    const depth = Number.parseInt(match[2], 10);
    if (!Number.isSafeInteger(depth)) return;
    frames.push(Object.freeze({ program: match[1], depth, index }));
  });
  return Object.freeze(frames);
}
