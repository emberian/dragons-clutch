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

  it('keeps the live read unavailable while the cohort-18 binding manifest is still a placeholder', () => {
    expect(html).toContain('Live devnet market read');
    expect(html).toContain('The checked cohort-18 manifest has not been published in the public bindings yet.');
    expect(html).not.toContain('Recent finalized signatures');
  });

  it('links the existing market admission entry when its checked binding opens joining', () => {
    const open = JSON.parse(JSON.stringify(example));
    open.activity.active_markets[0].join_open = true;
    const status = parseAquariumStatusV1(open, {
      cohort: () => ({ number: 18, manifestSha256: example.cohort.manifest_sha256 }),
      bindingFor: (market) => market === example.activity.active_markets[0].address ? { checked: true } : undefined,
    });
    const openHtml = renderToStaticMarkup(<PulseWorkspace preloadedAquarium={{ kind: 'loaded', status }} />);
    expect(openHtml).toContain('Public joining');
    expect(openHtml).toContain('open');
    expect(openHtml).toContain(`/market?address=${example.activity.active_markets[0].address}#join`);
    expect(openHtml).toContain('Join this market');
  });
});
