import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import LaunchStory from './LaunchStory';

describe('launch story', () => {
  it('is a reader-facing route into the real devnet app', () => {
    const html = renderToStaticMarkup(<LaunchStory />);
    expect(html).toContain('Markets that');
    expect(html).toContain('resolve <em>in public.</em>');
    expect(html).toContain('These are real devnet transactions, not a replay rendered from fixtures.');
    expect(html).toContain('href="/markets"');
    expect(html).toContain('href="/explorer"');
    expect(html).toContain('href="/activity"');
    expect(html).toContain('0.50%');
    expect(html).toContain('Hies3…MD4Qj');
    expect(html).toContain('Test assets have no monetary value.');
  });
});
