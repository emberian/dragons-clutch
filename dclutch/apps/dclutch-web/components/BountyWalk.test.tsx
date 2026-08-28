import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import BountyWalk from './BountyWalk';
import SmokeStory from './SmokeStory';

describe('the smoke story and bounty pages speak to the reader', () => {
  const bounty = renderToStaticMarkup(<BountyWalk />);
  const smoke = renderToStaticMarkup(<SmokeStory />);

  it('addresses the reader in second person with concrete verbs', () => {
    expect(bounty).toContain('Get paid to close');
    expect(bounty).toContain('any wallet can push it to the fallback outcome');
    expect(bounty).toContain('You need a wallet and a little SOL for the fee');
    expect(smoke).toContain('yours — can send one ordinary transaction');
  });

  it('distinguishes the deployed devnet programs from smoke markets that are not live yet', () => {
    for (const html of [bounty, smoke]) {
      expect(html).toContain('Not live yet');
      for (const jargon of ['census', 'ProgramTest', 'fail-closed', 'hostile-decode', 'provenance chip', 'evidence level', 'finalized floor']) {
        expect(html).not.toContain(jargon);
      }
    }
    expect(smoke).toContain('seven protocol programs are deployed at permanent addresses on Solana devnet');
    expect(smoke).toContain('None of these three smoke markets exists yet');
    expect(smoke).not.toContain('nothing is deployed to any network');
    expect(bounty).toContain('No such market is live on any public network today');
  });

  it('translates every refusal into a human sentence with the code beside it', () => {
    expect(bounty).toContain('Too early — the deadline has not passed yet');
    expect(bounty).toContain('Someone beat you to it');
    expect(bounty).toContain('chain code 0x800C');
    expect(bounty).toContain('chain code 0x800E');
    // The code never stands alone as the message.
    expect(bounty).not.toContain('Transition (12)');
  });

  it('labels measured numbers as measurements of the rehearsal, never as constants', () => {
    expect(bounty).toContain('each market posts its own number before opening');
    expect(bounty).toContain('895 bytes measured');
    expect(bounty).toContain('local test network');
  });

  it('keeps handwritten instruction bytes off the reader page', () => {
    expect(bounty).toContain('Open the generated route reference');
    expect(bounty).not.toContain('DCLTRIX1');
    expect(bounty).not.toContain('dclutch/resolution-cert/v3');
    expect(bounty).not.toContain('Accounts, in order');
  });

  it('states the trust honestly in one human sentence each', () => {
    expect(smoke).toContain('You are trusting that messenger not to lie, and the market says so up front');
    expect(bounty).toContain('the walk pays once');
    expect(bounty).toContain('No public devnet bounty exists today');
  });
});
