import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import RationalTerminalPanel from './RationalTerminalPanel';

describe('Rational terminal successor workbench', () => {
  it('shows independent K/N semantics and refuses browser-authored Custody authority', () => {
    const html = renderToStaticMarkup(<RationalTerminalPanel />);
    expect(html).toContain('Read a real terminal payout without forging Custody authority');
    expect(html).toContain('representation basis; need not equal N');
    expect(html).toContain('zero is valid');
    expect(html).toContain('Rust-emitter gated');
    expect(html).toContain('no parallel TypeScript digest authority');
    expect(html).not.toContain('Sign');
    expect(html).not.toContain('Submit');
  });
});
