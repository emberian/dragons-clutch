import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import example from '@/fixtures/aquarium-status-v1.example.json';
import { parseAquariumStatusV1 } from '@/lib/aquariumStatus';
import PulseWorkspace from './PulseWorkspace';

describe('the published aquarium observation', () => {
  const html = renderToStaticMarkup(<PulseWorkspace preloadedAquarium={{ kind: 'loaded', status: parseAquariumStatusV1(example) }} />);

  it('calls synthetic activity a projection and keeps joining visibly closed', () => {
    expect(html).toContain('synthetic actors');
    expect(html).toContain('Public joining');
    expect(html).toContain('closed');
    expect(html).not.toContain('Join this market');
  });

  it('links a reported address for a reader to authenticate from the market itself', () => {
    expect(html).toContain('/market?address=GtmpRvSL9y6RpqMth73VSdb9h1XRe7zqQZkhJkfgxKrA');
    expect(html).toContain('reported by the aquarium, not an exchange volume claim');
  });

  it('keeps the live read unavailable while cohort-18 inventory is still a checked placeholder', () => {
    expect(html).toContain('Live devnet market read');
    expect(html).toContain('The checked cohort-18 inventory has not been published yet.');
    expect(html).not.toContain('Recent finalized signatures');
  });
});
