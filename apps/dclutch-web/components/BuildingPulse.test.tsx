import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import { BUILDING_PULSE_V1 } from '@/lib/buildingPulse';
import BuildingPulse from './BuildingPulse';

// The /building page renders the shipped fixture and nothing else, so these
// tests pin the properties that make it safe to screenshot and pass around:
// it is dated, it is honest about being hand-written, and it never slips into
// the project's internal shorthand.

/** What React's renderer does to text children — apostrophes and all. */
const esc = (text: string) =>
  text.replace(/&/g, '&amp;').replace(/'/g, '&#x27;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');

describe('the building page', () => {
  const html = renderToStaticMarkup(<BuildingPulse />);

  it('dates itself and names its own URL, so a screenshot carries its provenance', () => {
    expect(html).toContain('clutch.dregg.pro/building');
    expect(html).toContain('written by hand');
    expect(html).toContain(`updated ${BUILDING_PULSE_V1.updatedDate}, ${BUILDING_PULSE_V1.updatedTime}`);
  });

  it('says which way it can be wrong: reality ahead of the page, never behind', () => {
    expect(html).toContain('reality runs ahead of it, never behind');
    expect(html).toContain('may already have happened');
  });

  it('does not borrow the live strip\'s authority', () => {
    // The front page reads the chain on every load; this page is hand-written
    // and must say so beside its numbers, not in a footnote.
    expect(html).toContain('not read live from the chain');
  });

  it('renders every stat, in-flight item, first and wall from the fixture', () => {
    for (const stat of BUILDING_PULSE_V1.stats) {
      expect(html).toContain(esc(stat.value));
      expect(html).toContain(esc(stat.label));
    }
    for (const item of [...BUILDING_PULSE_V1.now, ...BUILDING_PULSE_V1.recent]) expect(html).toContain(esc(item.title));
    for (const wall of BUILDING_PULSE_V1.walls.entries) {
      expect(html).toContain(esc(wall.name));
      expect(html).toContain(esc(wall.epitaph));
    }
    for (const link of BUILDING_PULSE_V1.links) expect(html).toContain(`href="${link.href}"`);
  });

  it('speaks no internal vocabulary anywhere in the rendered page', () => {
    // Belt to the parser's suspenders: class names and component-authored copy
    // are outside the fixture, so the rendered artifact is checked whole.
    expect(html).not.toMatch(/\bcohorts?\b|\bswarms?\b|\bseams?\b|WAVE\.md/i);
  });

  it('keeps the devnet posture the rest of the site promises', () => {
    expect(html).toContain('devnet');
    expect(html).toContain('no value at risk anywhere');
    expect(html).not.toMatch(/open for trading/i);
  });
});
