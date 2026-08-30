/**
 * The SDK owns slot→wall-clock arithmetic and its honesty labels; the app
 * re-exports it so browser and external clients cannot drift, and adds the
 * one chain-touching step: reading two of the cluster's own block times to
 * measure the actual slot rate.
 */
export * from '@dclutch/sdk/slotClock';

import { measuredSlotClockV1, type SlotClockV1 } from '@dclutch/sdk/slotClock';

/**
 * How far behind the floor the second rate sample is taken. ~2 h of devnet
 * at the fast rate observed 2026-08-30 (~160 ms/slot), ~4.8 h at the nominal
 * 400 — comfortably inside the history a public RPC node retains, and long
 * enough that short-term wobble averages out.
 */
export const SLOT_RATE_SPAN_SLOTS_V1 = 43_200n;

/**
 * Measure the cluster's slot rate around one finalized floor.
 *
 * Two `getBlockTime` reads (three when the earlier slot was skipped). Any
 * refusal degrades to the assumed-rate clock, whose caveat says so — this
 * read may make a page more legible, never less available.
 */
export async function readSlotClockV1(
  client: Readonly<{ blockTime(slot: string): Promise<string | null> }>,
  floorSlot: string,
): Promise<SlotClockV1> {
  const observedAtMs = Date.now();
  const floor = BigInt(floorSlot);
  const earlier = floor > SLOT_RATE_SPAN_SLOTS_V1 ? floor - SLOT_RATE_SPAN_SLOTS_V1 : 0n;
  const [floorUnixSeconds, firstEarlier] = await Promise.all([
    client.blockTime(floorSlot),
    client.blockTime(earlier.toString()),
  ]);
  let earlierSlot = earlier;
  let earlierUnixSeconds = firstEarlier;
  if (earlierUnixSeconds === null && earlier > 1_000n) {
    // One retry a stride away covers the common case of a skipped slot
    // without turning a display nicety into a read storm.
    earlierSlot = earlier - 1_000n;
    earlierUnixSeconds = await client.blockTime(earlierSlot.toString());
  }
  return measuredSlotClockV1({
    floorSlot,
    floorUnixSeconds,
    earlierSlot: earlierSlot.toString(),
    earlierUnixSeconds,
    observedAtMs,
  });
}
