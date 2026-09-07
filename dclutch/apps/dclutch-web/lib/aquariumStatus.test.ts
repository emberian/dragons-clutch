import { describe, expect, it } from 'vitest';

import example from '@/fixtures/aquarium-status-v1.example.json';
import {
  AQUARIUM_STATUS_URL_V1,
  aquariumBeatV1,
  parseAquariumStatusV1,
  readAquariumStatusV1,
} from './aquariumStatus';

function mutated(change: (copy: Record<string, unknown>) => void): unknown {
  const copy = JSON.parse(JSON.stringify(example)) as Record<string, unknown>;
  change(copy);
  return copy;
}

describe('the aquarium status decoder', () => {
  it('accepts a bounded synthetic observation and preserves no local paths', () => {
    const status = parseAquariumStatusV1(example);
    expect(status.cohort.number).toBe(18);
    expect(status.activity.activeMarkets[0]?.joinOpen).toBe(false);
    expect(status.activity.counts.fill).toBe(6);
    expect(JSON.stringify(status)).not.toContain('/Users/');
  });

  it('refuses a status that would advertise a public joining path', () => {
    expect(() => parseAquariumStatusV1(mutated((copy) => {
      ((copy.activity as Record<string, unknown>).active_markets as Array<Record<string, unknown>>)[0]!.join_open = true;
    }))).toThrow('keep public joining closed');
  });

  it('refuses duplicate active coordinates and an over-bound public list', () => {
    expect(() => parseAquariumStatusV1(mutated((copy) => {
      const activity = copy.activity as Record<string, unknown>;
      const markets = activity.active_markets as Array<Record<string, unknown>>;
      markets.push({ ...markets[0]!, market_id: 'other-market' });
    }))).toThrow('repeats address');
    expect(() => parseAquariumStatusV1(mutated((copy) => {
      const activity = copy.activity as Record<string, unknown>;
      const markets = activity.active_markets as Array<Record<string, unknown>>;
      for (let index = 0; index < 4; index += 1) markets.push({ ...markets[0]!, market_id: `other-market-${index}`, address: null });
    }))).toThrow('exceeds its published limit');
  });

  it('treats the producer deadline and driver exit as stronger liveness facts', () => {
    const status = parseAquariumStatusV1(example);
    expect(aquariumBeatV1(status, Date.parse(status.run.updatedAt) + 1_000).state).toBe('running');
    expect(aquariumBeatV1(status, Date.parse(status.run.expectedNextUpdateBy!) + 1).state).toBe('stale');
    const exited = parseAquariumStatusV1(mutated((copy) => {
      copy.failure = { kind: 'driver-exit', at: '2026-09-07T18:02:01Z', detail: 'census refused' };
    }));
    expect(aquariumBeatV1(exited, Date.parse(exited.run.updatedAt)).state).toBe('halted');
  });
});

describe('the guarded aquarium reader', () => {
  it('pins the one public artifact URL', () => {
    expect(AQUARIUM_STATUS_URL_V1).toBe('/aquarium-status-v1.json');
  });

  it('treats static-host fallbacks as absence and a bad JSON record as a refusal', async () => {
    expect((await readAquariumStatusV1(async () => ({ ok: true, text: async () => '<!doctype html>' }))).kind).toBe('absent');
    expect((await readAquariumStatusV1(async () => ({ ok: true, text: async () => JSON.stringify({ schema: 'other' }) }))).kind).toBe('refused');
  });
});
