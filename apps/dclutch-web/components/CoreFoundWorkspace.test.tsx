import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import CoreFoundWorkspace from './CoreFoundWorkspace';

describe('Core Found workspace', () => {
  it('renders the real Found31 boundary without a fabricated Market or authority', () => {
    const html = renderToStaticMarkup(<CoreFoundWorkspace />);
    expect(html).toContain('Found one common');
    expect(html).toContain('Real 31-account frame');
    expect(html).toContain('Construct unsigned lifecycle + Found transactions');
    expect(html).toContain('Immutable rent refund wallet');
    expect(html).toContain('No transaction has been constructed');
    expect(html).toContain('No signing or submission occurs in this UI');
    expect(html).toContain('Product Runtime V2 raw');
    expect(html).not.toContain('Illustrative');
    expect(html).not.toContain('sample balance');
  });
});
