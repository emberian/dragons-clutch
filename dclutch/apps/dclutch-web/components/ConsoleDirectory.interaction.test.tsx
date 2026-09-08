// @vitest-environment jsdom
import { act } from 'react';
import { createRoot } from 'react-dom/client';
import { afterEach, describe, expect, it, vi } from 'vitest';
import ConsoleDirectory from './ConsoleDirectory';
import { BROWSER_CAPABILITY_STANDINGS_V1, browserActPrerequisitesV1 } from '@/lib/capabilitySurface';

vi.mock('@/components/Nav', () => ({ default: () => <header /> }));
Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true });
const mounted: Array<ReturnType<typeof createRoot>> = [];
afterEach(async () => {
  await act(async () => { for (const root of mounted.splice(0)) root.unmount(); });
  document.body.replaceChildren();
});

describe('finding an existing console action', () => {
  it('filters the authoritative cards, retains prerequisites, and recovers from an empty result', async () => {
    const container = document.createElement('div');
    document.body.appendChild(container);
    const root = createRoot(container);
    mounted.push(root);
    await act(async () => root.render(<ConsoleDirectory />));
    const all = container.querySelectorAll('.console-stage .console-entry').length;
    expect(all).toBe(BROWSER_CAPABILITY_STANDINGS_V1.filter((entry) => entry.venue !== 'no-venue').length);
    const search = container.querySelector<HTMLInputElement>('#console-search')!;
    async function type(text: string) {
      await act(async () => {
        Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')!.set!.call(search, text);
        search.dispatchEvent(new Event('input', { bubbles: true }));
      });
    }
    await type('wallet');
    const matches = [...container.querySelectorAll('.console-stage .console-entry')];
    expect(matches.length).toBeGreaterThan(0);
    expect(matches.length).toBeLessThan(all);
    for (const card of matches) {
      expect(card.textContent?.toLowerCase()).toContain('wallet');
      const standing = BROWSER_CAPABILITY_STANDINGS_V1.find((entry) => entry.action.action === card.querySelector('strong')?.textContent)!;
      for (const needed of browserActPrerequisitesV1(standing)) expect(card.textContent).toContain(needed.statement);
      for (const wall of standing.walls) expect(card.textContent).toContain(wall.statement);
    }
    await type('no-matching-protocol-action-xyz');
    expect(container.querySelectorAll('.console-stage .console-entry')).toHaveLength(0);
    const reset = container.querySelector<HTMLButtonElement>('.console-empty button')!;
    await act(async () => reset.click());
    expect(search.value).toBe('');
    expect(container.querySelectorAll('.console-stage .console-entry')).toHaveLength(all);
  });
});
