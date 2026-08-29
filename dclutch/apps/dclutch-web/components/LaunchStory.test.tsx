import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import LaunchStory from './LaunchStory';

describe('launch story', () => {
  it('keeps the public cut honest until its checked Market manifest is updated', () => {
    const html = renderToStaticMarkup(<LaunchStory />);
    expect(html).toContain('Markets that');
    expect(html).toContain('resolve <em>in public.</em>');
    expect(html).toContain('No public market is named yet.');
    expect(html).toContain('no lifecycle activity is invented');
    expect(html).toContain('href="/markets"');
    expect(html).toContain('href="/explorer"');
    expect(html).toContain('href="/activity"');
    expect(html).toContain('0.50%');
    expect(html).toContain('Hies3…MD4Qj');
    expect(html).toContain('Test assets have no monetary value.');
  });
});
