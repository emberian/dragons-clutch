import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import BountyWalk from './BountyWalk';
import SmokeStory from './SmokeStory';

describe('example markets and bounties', () => {
  const bounty = renderToStaticMarkup(<BountyWalk />);
  const smoke = renderToStaticMarkup(<SmokeStory />);

  it('shows availability for the currently unlisted example markets', () => {
    expect(bounty).toContain('Not live yet');
    expect(bounty).toContain('No bounty market is listed yet');
    expect(smoke).toContain('Not live yet');
    expect(smoke).toContain('These example markets are not listed on devnet yet');
  });

  it('shows the bounty, fee and transaction errors', () => {
    expect(bounty).toContain('each market posts its own number before opening');
    expect(bounty).toContain('The figures below are from a local test run');
    expect(bounty).toContain('895 bytes with your one signature');
    expect(bounty).toContain('The bounty pays once');
    expect(bounty).toContain('Too early — the deadline has not passed yet');
    expect(bounty).toContain('chain code 0x800C');
    expect(bounty).toContain('chain code 0x800E');
  });

  it('links the developer reference and explains the reporter dependency', () => {
    expect(bounty).toContain('reference/abi/routeCensus.md');
    expect(smoke).toContain('The market relies on that reporter for the observation');
    expect(smoke).toContain('href="/bounty"');
  });
});
