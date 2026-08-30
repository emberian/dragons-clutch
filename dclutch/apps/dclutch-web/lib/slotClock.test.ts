import { describe, expect, it } from 'vitest';

import { readSlotClockV1, SLOT_RATE_SPAN_SLOTS_V1 } from './slotClock';

/**
 * The app half of the slot clock: two block-time reads against the real
 * client interface, one retry for a skipped slot, and degradation to the
 * labelled assumption instead of any failure surfacing to the page.
 */
describe('readSlotClockV1', () => {
  function stub(times: Record<string, string | null>) {
    const calls: string[] = [];
    return {
      calls,
      blockTime: (slot: string) => {
        calls.push(slot);
        return Promise.resolve(times[slot] ?? null);
      },
    };
  }

  it('measures from the floor and one span-earlier slot', async () => {
    const floor = '490450000';
    const earlier = (490_450_000n - SLOT_RATE_SPAN_SLOTS_V1).toString();
    const client = stub({ [floor]: '1788100000', [earlier]: '1788093088' });
    const clock = await readSlotClockV1(client, floor);
    expect(client.calls).toEqual([floor, earlier]);
    expect(clock.basis).toBe('measured');
    // 43,200 slots over 6,912 s = 160 ms/slot.
    expect(clock.msPerSlot).toBeCloseTo(160, 5);
  });

  it('retries one stride back when the earlier slot was skipped', async () => {
    const floor = '490450000';
    const earlier = (490_450_000n - SLOT_RATE_SPAN_SLOTS_V1).toString();
    const retried = (490_450_000n - SLOT_RATE_SPAN_SLOTS_V1 - 1_000n).toString();
    const client = stub({ [floor]: '1788100000', [retried]: '1788092928' });
    const clock = await readSlotClockV1(client, floor);
    expect(client.calls).toEqual([floor, earlier, retried]);
    expect(clock.basis).toBe('measured');
    // 44,200 slots over 7,072 s = 160 ms/slot.
    expect(clock.msPerSlot).toBeCloseTo(160, 5);
  });

  it('degrades to the assumed clock when the node keeps no times at all', async () => {
    const client = stub({});
    const clock = await readSlotClockV1(client, '490450000');
    expect(clock.basis).toBe('assumed');
    expect(clock.msPerSlot).toBe(400);
  });
});
