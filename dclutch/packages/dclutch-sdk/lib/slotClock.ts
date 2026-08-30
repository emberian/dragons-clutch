/**
 * Slot → wall-clock arithmetic, with its honesty carried in the type.
 *
 * The chain's own clock is slots. A reader's clock is not, and every slot
 * number this site shows costs the reader a division they cannot do — devnet
 * slot time is nominally 400 ms but actually wobbles hard (measured 2026-08-30:
 * ~160 ms/slot over the prior hours, 2.5× off nominal). So a wall-clock
 * estimate is only honest when the rate is MEASURED against the cluster's own
 * block times, and every rendered time says it is an estimate.
 *
 * This module is pure: callers observe (slot, blockTime) pairs however they
 * like and get back a clock, moments, and the caveat sentence that must
 * accompany them. Nothing here reads the chain and nothing here is exact —
 * which is the point, and which is why the caveat is part of the API.
 */

/** The nominal devnet slot time, used only when measurement was refused. */
export const ASSUMED_MS_PER_SLOT_V1 = 400;

/** Measured rates outside this band read as a broken measurement, not a fact. */
export const MEASURED_MS_PER_SLOT_FLOOR_V1 = 50;
export const MEASURED_MS_PER_SLOT_CEILING_V1 = 2_000;

export type SlotClockV1 = Readonly<{
  /** The finalized floor slot this clock is anchored at. */
  floorSlot: string;
  /** Reader wall-clock milliseconds (Date.now) when the floor was observed. */
  observedAtMs: number;
  /** Estimated milliseconds per slot. */
  msPerSlot: number;
  /** 'measured' against the cluster's block times, or 'assumed' nominal. */
  basis: 'measured' | 'assumed';
  /** For a measured clock, the slot span it was measured over. */
  measuredOverSlots: string | null;
}>;

export type SlotMomentV1 = Readonly<{
  /** Estimated reader wall-clock milliseconds for the slot. */
  estimatedMs: number;
  /** slot − floorSlot; negative when the slot is in the past. */
  deltaSlots: string;
}>;

/** A clock that admits it measured nothing and assumes the nominal rate. */
export function assumedSlotClockV1(floorSlot: string, observedAtMs: number): SlotClockV1 {
  return Object.freeze({
    floorSlot,
    observedAtMs,
    msPerSlot: ASSUMED_MS_PER_SLOT_V1,
    basis: 'assumed' as const,
    measuredOverSlots: null,
  });
}

/**
 * A clock measured from two of the cluster's own (slot, blockTime) readings.
 *
 * Falls back to the assumed clock — it never throws — when either block time
 * is missing, the span is degenerate, or the implied rate is outside the sane
 * band: a wrong measurement rendered as fact is exactly what this module
 * exists to avoid.
 */
export function measuredSlotClockV1(args: Readonly<{
  floorSlot: string;
  floorUnixSeconds: string | null;
  earlierSlot: string;
  earlierUnixSeconds: string | null;
  observedAtMs: number;
}>): SlotClockV1 {
  const fallback = assumedSlotClockV1(args.floorSlot, args.observedAtMs);
  if (args.floorUnixSeconds === null || args.earlierUnixSeconds === null) return fallback;
  const slotSpan = BigInt(args.floorSlot) - BigInt(args.earlierSlot);
  const secondsSpan = BigInt(args.floorUnixSeconds) - BigInt(args.earlierUnixSeconds);
  if (slotSpan <= 0n || secondsSpan <= 0n) return fallback;
  const msPerSlot = Number(secondsSpan * 1000n) / Number(slotSpan);
  if (!Number.isFinite(msPerSlot) || msPerSlot < MEASURED_MS_PER_SLOT_FLOOR_V1 || msPerSlot > MEASURED_MS_PER_SLOT_CEILING_V1) return fallback;
  return Object.freeze({
    floorSlot: args.floorSlot,
    observedAtMs: args.observedAtMs,
    msPerSlot,
    basis: 'measured' as const,
    measuredOverSlots: slotSpan.toString(),
  });
}

/** Where one slot lands on the reader's wall clock, by this clock's estimate. */
export function slotMomentV1(clock: SlotClockV1, slot: string): SlotMomentV1 {
  const deltaSlots = BigInt(slot) - BigInt(clock.floorSlot);
  const estimatedMs = clock.observedAtMs + Number(deltaSlots) * clock.msPerSlot;
  return Object.freeze({ estimatedMs, deltaSlots: deltaSlots.toString() });
}

function coarse(ms: number): string {
  const minutes = ms / 60_000;
  if (minutes < 1) return 'under a minute';
  if (minutes < 90) return `${Math.round(minutes)} min`;
  const hours = minutes / 60;
  if (hours < 48) return `${Math.round(hours)} h`;
  return `${Math.round(hours / 24)} d`;
}

/**
 * One coarse, approximate phrase for a slot: "≈ 6 h ago" / "≈ in 41 min" /
 * "about now". Deliberately coarse — the precision a division would fake is
 * precision the estimate does not have.
 */
export function describeSlotMomentV1(clock: SlotClockV1, slot: string, nowMs: number): string {
  const moment = slotMomentV1(clock, slot);
  const offset = moment.estimatedMs - nowMs;
  if (Math.abs(offset) < 60_000) return 'about now';
  return offset < 0 ? `≈ ${coarse(-offset)} ago` : `≈ in ${coarse(offset)}`;
}

/**
 * The phrase for a deadline slot specifically: a past deadline says it
 * passed, a future one reads as a countdown, and the boundary says so.
 */
export function deadlineMomentPhraseV1(clock: SlotClockV1, deadlineSlot: string, nowMs: number): string {
  const description = describeSlotMomentV1(clock, deadlineSlot, nowMs);
  if (description === 'about now') return 'at the deadline about now';
  return description.endsWith('ago') ? `passed ${description}` : description;
}

/** The sentence that must render on any surface showing these estimates. */
export function slotClockCaveatV1(clock: SlotClockV1): string {
  if (clock.basis === 'measured') {
    return `Times are estimates from this cluster's own recent block times (≈ ${Math.round(clock.msPerSlot)} ms per slot over the last ${clock.measuredOverSlots} slots); slot time wobbles, and the chain's own clock is slots.`;
  }
  return `Times are estimates assuming the nominal ${ASSUMED_MS_PER_SLOT_V1} ms per slot — the cluster's actual rate could not be measured, and devnet often runs far from nominal.`;
}
