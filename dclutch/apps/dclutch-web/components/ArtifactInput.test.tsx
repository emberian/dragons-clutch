import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import ArtifactInput from './ArtifactInput';

describe('local artifact input accessibility', () => {
  it('keeps file choice in the keyboard order and names both input paths', () => {
    const html = renderToStaticMarkup(<ArtifactInput
      label="Core checked release"
      provenance="Produced by the checked release pipeline."
      value=""
      onChange={() => undefined}
      required
    />);

    expect(html).toContain('type="file" aria-label="Choose Core checked release"');
    expect(html).not.toContain('tabindex="-1"');
    expect(html).toContain('Offline fallback · paste the same file as base64');
    expect(html).toContain('<textarea required=""');
    expect(html).toContain('aria-live="polite"');
  });
});
