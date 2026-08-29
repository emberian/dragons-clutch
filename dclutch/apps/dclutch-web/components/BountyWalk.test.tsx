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
    // The offer used to land nine lines before its correction, with a
    // runnable-looking command in between; a skimmer took away "get paid".
    // The correction now sits with the offer on both pages.
    expect(bounty).toContain('You cannot do this today.');
    expect(smoke).toContain('none of these markets is open yet');
  });

  it('translates every refusal into a human sentence with the code beside it', () => {
    expect(bounty).toContain('Too early — the deadline has not passed yet');
    expect(bounty).toContain('Someone beat you to it');
    expect(bounty).toContain('chain code 0x800C');
    expect(bounty).toContain('chain code 0x800E');
    // The code never stands alone as the message.
    expect(bounty).not.toContain('Transition (12)');
  });

  it('labels measured numbers as measurements, and says what they were measured against', () => {
    expect(bounty).toContain('each market posts its own number before opening');
    expect(bounty).toContain('895 bytes measured');
    // The page used to rest its numbers on one past end-to-end run on a local
    // network. That run is real but its campaign is parked, so a reader cannot
    // reproduce it — while the stronger claim went unsaid. The walk executes
    // against the real compiled Resolution and Core programs on every test run
    // (crates/dclutch-svm-harness/tests/relayed_mainnet_state.rs,
    // a_silent_relayer_cannot_make_the_market_unresolvable), and 895 is still
    // the measured single-signer extent at HEAD: the harness measures 991 for
    // its two-signature form, less one signature (64) and one account key (32).
    expect(bounty).toContain('it runs against the real programs');
    expect(bounty).toContain('The numbers below are measured from those runs');
    expect(bounty).not.toContain('local test network');
  });

  it('keeps handwritten instruction bytes off the reader page', () => {
    expect(bounty).toContain('Open the generated route reference');
    expect(bounty).toContain('reference/abi/routeCensus.md');
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
