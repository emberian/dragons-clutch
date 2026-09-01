import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';

import CommandRunbook, { copyCommandV1 } from './CommandRunbook';

describe('operator command runbook', () => {
  it('renders the exact command with a utility action and quiet initial status', () => {
    const command = 'dclutch route direct \\\n+  --output "$DIRECT_ROUTE"';
    const html = renderToStaticMarkup(<CommandRunbook label="Preview and execute" command={command} />);
    expect(html).toContain('Preview and execute');
    expect(html).toContain('Copy commands');
    expect(html).toContain('dclutch route direct');
    expect(html).toContain('--output &quot;$DIRECT_ROUTE&quot;');
    expect(html).toContain('aria-live="polite">Not copied.');
  });

  it('copies exactly once and does not imply execution', async () => {
    const write = vi.fn(async () => undefined);
    await expect(copyCommandV1('dclutch join --execute', write)).resolves.toBe(
      'Copied the exact displayed commands. Nothing was executed.',
    );
    expect(write).toHaveBeenCalledOnce();
    expect(write).toHaveBeenCalledWith('dclutch join --execute');
  });

  it('refuses an empty block before touching the clipboard boundary', async () => {
    const write = vi.fn(async () => undefined);
    await expect(copyCommandV1('', write)).rejects.toThrow('command block is empty');
    expect(write).not.toHaveBeenCalled();
  });
});
