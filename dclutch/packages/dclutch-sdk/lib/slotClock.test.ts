import { describe, expect, it } from 'vitest';

import {
  ASSUMED_MS_PER_SLOT_V1,
  SLOT_RATE_SPAN_SLOTS_V1,
  assumedSlotClockV1,
  deadlineMomentPhraseV1,
  describeSlotMomentV1,
  measuredSlotClockV1,
  readSlotClockV1,
  slotClockCaveatV1,
  slotMomentV1,
} from './slotClock';

/**
 * The wall-clock layer is allowed to be approximate; it is never allowed to
 * be quietly approximate. These pin the two halves of that bargain: the rate
 * is measured from the cluster's own block times (and falls back rather than
 * inventing), and every describable moment comes with the caveat sentence.
 */
describe('slot clock measurement', () => {
  const NOW = 1_788_100_000_000;

  it('measures the rate from two of the cluster’s own block times', () => {
    // 100,000 slots over 16,000 s = 160 ms/slot — the shape devnet actually
    // showed on 2026-08-30, 2.5× off the nominal 400.
    const clock = measuredSlotClockV1({
      floorSlot: '490450000',
      floorUnixSeconds: '1788100000',
      earlierSlot: '490350000',
      earlierUnixSeconds: '1788084000',
      observedAtMs: NOW,
    });
    expect(clock.basis).toBe('measured');
    expect(clock.msPerSlot).toBeCloseTo(160, 5);
    expect(clock.measuredOverSlots).toBe('100000');
  });

  it('falls back to the assumed clock rather than inventing a rate', () => {
    const base = { floorSlot: '1000', earlierSlot: '900', observedAtMs: NOW };
    for (const broken of [
      { ...base, floorUnixSeconds: null, earlierUnixSeconds: '100' },
      { ...base, floorUnixSeconds: '200', earlierUnixSeconds: null },
      // degenerate span
      { ...base, floorUnixSeconds: '200', earlierUnixSeconds: '200' },
      // clock running backwards
      { ...base, floorUnixSeconds: '100', earlierUnixSeconds: '200' },
      // implied 10 ms/slot: outside the sane band, so it is a broken reading
      { ...base, floorUnixSeconds: '101', earlierUnixSeconds: '100' },
    ]) {
      const clock = measuredSlotClockV1(broken);
      expect(clock.basis).toBe('assumed');
      expect(clock.msPerSlot).toBe(ASSUMED_MS_PER_SLOT_V1);
    }
  });

  it('places past and future slots on the reader’s clock', () => {
    const clock = measuredSlotClockV1({
      floorSlot: '490450000', floorUnixSeconds: '1788100000',
      earlierSlot: '490350000', earlierUnixSeconds: '1788084000',
      observedAtMs: NOW,
    });
    // 120,032 slots before the floor at 160 ms/slot ≈ 5.3 h earlier — the
    // flagship’s lapsed activation deadline, placed where it actually lapsed.
    const past = slotMomentV1(clock, '490329968');
    expect(past.deltaSlots).toBe('-120032');
    expect(past.estimatedMs).toBeCloseTo(NOW - 120_032 * 160, 0);
    expect(describeSlotMomentV1(clock, '490329968', NOW)).toBe('≈ 5 h ago');

    const future = slotMomentV1(clock, '490825000');
    expect(future.deltaSlots).toBe('375000');
    expect(describeSlotMomentV1(clock, '490825000', NOW)).toBe('≈ in 17 h');

    expect(describeSlotMomentV1(clock, clock.floorSlot, NOW)).toBe('about now');
  });

  it('speaks minutes near now, hours through two days, then days', () => {
    const clock = assumedSlotClockV1('1000000', NOW);
    // 400 ms/slot: 6,000 slots = 40 min; 450,000 slots = 50 h ≈ 2 d.
    expect(describeSlotMomentV1(clock, '994000', NOW)).toBe('≈ 40 min ago');
    expect(describeSlotMomentV1(clock, '1006000', NOW)).toBe('≈ in 40 min');
    expect(describeSlotMomentV1(clock, '550000', NOW)).toBe('≈ 2 d ago');
  });

  it('phrases a deadline as passed, counting down, or at the boundary', () => {
    const clock = assumedSlotClockV1('1000000', NOW);
    expect(deadlineMomentPhraseV1(clock, '994000', NOW)).toBe('passed ≈ 40 min ago');
    expect(deadlineMomentPhraseV1(clock, '1006000', NOW)).toBe('≈ in 40 min');
    expect(deadlineMomentPhraseV1(clock, '1000000', NOW)).toBe('at the deadline about now');
  });

  it('carries its caveat, and the caveat names its basis', () => {
    const measured = measuredSlotClockV1({
      floorSlot: '490450000', floorUnixSeconds: '1788100000',
      earlierSlot: '490350000', earlierUnixSeconds: '1788084000',
      observedAtMs: NOW,
    });
    // Renegotiated 2026-08-31: the caveat used to run two clauses past its own
    // point ("slot time wobbles, and the chain's own clock is slots"; "devnet
    // often runs far from nominal"). What a reader needs is that the time is
    // an estimate and at what rate -- and, when we could not measure the
    // cluster, that we could not. Both still pinned, both now one clause.
    expect(slotClockCaveatV1(measured)).toContain('estimated');
    expect(slotClockCaveatV1(measured)).toContain('160 ms per slot');
    expect(slotClockCaveatV1(measured).length).toBeLessThan(60);

    const assumed = assumedSlotClockV1('1', NOW);
    expect(slotClockCaveatV1(assumed)).toContain('nominal 400 ms');
    expect(slotClockCaveatV1(assumed)).toContain('could not be measured');
  });
});

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
