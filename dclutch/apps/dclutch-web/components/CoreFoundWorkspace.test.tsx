import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import CoreFoundWorkspace from './CoreFoundWorkspace';

describe('Core Found workspace', () => {
  it('renders the legacy Found37 boundary without presenting it as current founding', () => {
    const html = renderToStaticMarkup(<CoreFoundWorkspace />);
    expect(html).toContain('Legacy founding inspector');
    expect(html).toContain('cannot open a current devnet market');
    expect(html).toContain('older, partial packet pair');
    expect(html).toContain('Construct unsigned lifecycle + Found transactions');
    expect(html).toContain('Immutable rent refund wallet');
    expect(html).toContain('No transaction has been constructed');
    expect(html).toContain('No signing or submission occurs in this UI');
    expect(html).toContain('Product Runtime V2 raw');
    expect(html).toContain('SourceMaterialV3 raw');
    expect(html).toContain('Linked basis raw');
    expect(html).not.toContain('Execution release set raw');
    expect(html).not.toContain('Illustrative');
    expect(html).not.toContain('sample balance');
  });
});
