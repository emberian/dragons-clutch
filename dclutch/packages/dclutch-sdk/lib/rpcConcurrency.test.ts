import { describe, expect, it } from 'vitest';

import { MAX_IN_FLIGHT_READS_PER_ENDPOINT_V1, SolanaRpcClient } from './rpc';

/**
 * The public devnet endpoint refuses heavy reads issued at the same time, not
 * heavy reads as such: twelve sequential light reads pass, six sequential
 * program scans pass, and six of those same scans issued together return 429s.
 * Our reading surface fans out per market, so it sat on the wrong side of that
 * by construction. This pins the bound that keeps it on the right side.
 */
describe('the endpoint gate', () => {
  function trackingFetcher() {
    let inFlight = 0;
    let peak = 0;
    const release: Array<() => void> = [];
    const fetcher = (async () => {
      inFlight += 1;
      peak = Math.max(peak, inFlight);
      await new Promise<void>((resolve) => release.push(resolve));
      inFlight -= 1;
      return {
        ok: true,
        headers: { get: () => null },
        body: null,
        arrayBuffer: async () => new TextEncoder().encode(
          JSON.stringify({ jsonrpc: '2.0', id: 1, result: '1' }),
        ).buffer,
      } as unknown as Response;
    }) as unknown as typeof fetch;
    return { fetcher, peak: () => peak, releaseAll: () => { while (release.length > 0) release.shift()!(); } };
  }

  it('never lets more reads reach one endpoint at once than the measured bound', async () => {
    const tracker = trackingFetcher();
    const client = new SolanaRpcClient('https://gate.test/', tracker.fetcher);
    const reads = Array.from({ length: 8 }, () => client.finalizedSlot().catch(() => undefined));
    // Let the gate admit whatever it will before anything completes.
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(tracker.peak()).toBeLessThanOrEqual(MAX_IN_FLIGHT_READS_PER_ENDPOINT_V1);
    // Pump: each release lets the gate admit the next waiter, and the peak
    // must never rise above the bound no matter how many rounds it takes.
    for (let i = 0; i < 16; i += 1) {
      tracker.releaseAll();
      await new Promise((resolve) => setTimeout(resolve, 0));
    }
    await Promise.all(reads);
    // Everything still ran: the gate delays reads, it never drops them.
    expect(tracker.peak()).toBeLessThanOrEqual(MAX_IN_FLIGHT_READS_PER_ENDPOINT_V1);
  });

  it('bounds in-flight reads rather than total ones', async () => {
    const tracker = trackingFetcher();
    const client = new SolanaRpcClient('https://gate2.test/', tracker.fetcher);
    let settled = 0;
    const reads = Array.from({ length: 6 }, () => client.finalizedSlot().then(
      () => { settled += 1; },
      () => { settled += 1; },
    ));
    for (let i = 0; i < 12; i += 1) {
      tracker.releaseAll();
      await new Promise((resolve) => setTimeout(resolve, 0));
    }
    await Promise.all(reads);
    expect(settled).toBe(6);
  });

  it('keeps a slot count that cannot drift below zero across failures', async () => {
    // A refusing endpoint must still return its slot, or the gate would close
    // permanently after the first failure and the page would hang forever.
    const failing = (async () => { throw new Error('network down'); }) as unknown as typeof fetch;
    const client = new SolanaRpcClient('https://gate3.test/', failing);
    for (let i = 0; i < MAX_IN_FLIGHT_READS_PER_ENDPOINT_V1 + 3; i += 1) {
      await expect(client.finalizedSlot()).rejects.toThrow();
    }
    // Still serving after more failures than the gate is wide.
    await expect(client.finalizedSlot()).rejects.toThrow('network down');
  });
});
