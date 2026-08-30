import { describe, expect, it } from 'vitest';

import manifest from '@/fixtures/building-pulse.json';
import { parseBuildingPulseV1 } from './buildingPulse';

// A minimal valid pulse, cloned per test and broken one field at a time. The
// shipped fixture itself is parsed in the first test, so the two can never
// drift apart silently.
const minimal = () => ({
  schema: 'dclutch-building-pulse-v1',
  updatedDate: '2026-08-30',
  updatedTime: '11:05 EDT',
  eyebrow: 'In development',
  headline: 'A headline.',
  lede: 'A lede.',
  stats: [{ value: '7', label: 'programs', detail: 'all of them' }],
  statsProvenance: 'Written by hand.',
  now: [{ title: 'A thing', detail: 'in flight' }],
  recent: [{ title: 'A first', detail: 'landed' }],
  walls: { intro: 'Walls fall.', entries: [{ name: 'Wall 1', epitaph: 'It fell.' }] },
  closing: 'Nothing is for sale.',
  links: [{ href: '/markets', label: 'Markets' }],
});

describe('the building pulse parser', () => {
  it('accepts the shipped fixture', () => {
    const pulse = parseBuildingPulseV1(manifest);
    expect(pulse.headline.length).toBeGreaterThan(0);
    expect(pulse.stats.length).toBeGreaterThan(0);
    expect(pulse.now.length).toBeGreaterThan(0);
    expect(pulse.walls.entries.length).toBeGreaterThan(0);
  });

  it('refuses internal vocabulary, naming the word', () => {
    // The page is written for strangers; a future edit pasting an internal
    // status line in verbatim must be refused, not quietly published.
    const doc = minimal();
    doc.now = [{ title: 'The cohort-7 upgrade', detail: 'is staged' }];
    expect(() => parseBuildingPulseV1(doc)).toThrow(/internal vocabulary \("cohort"\)/);
    const doc2 = minimal();
    doc2.recent = [{ title: 'A first', detail: 'the SEAM audit ran' }];
    expect(() => parseBuildingPulseV1(doc2)).toThrow(/internal vocabulary/);
  });

  it('refuses an undated pulse — "recently" is not a date', () => {
    const doc = minimal();
    doc.updatedDate = 'recently';
    expect(() => parseBuildingPulseV1(doc)).toThrow(/YYYY-MM-DD/);
  });

  it('refuses unknown fields instead of carrying them silently', () => {
    const doc = { ...minimal(), surprise: true };
    expect(() => parseBuildingPulseV1(doc)).toThrow(/missing or unknown fields/);
  });

  it('refuses empty sections — a pulse with nothing in flight is a lie of shape', () => {
    const doc = minimal();
    doc.now = [];
    expect(() => parseBuildingPulseV1(doc)).toThrow(/non-empty list/);
  });

  it('keeps data links site-relative', () => {
    const doc = minimal();
    doc.links = [{ href: 'https://elsewhere.example', label: 'Away' }];
    expect(() => parseBuildingPulseV1(doc)).toThrow(/site-relative/);
  });
});
