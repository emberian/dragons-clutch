import { describe, expect, it } from 'vitest';

import { UNGATE_LICENCE_V1, UNGATE_SHUT_V1, releaseUngateV1 } from './releaseUngate';
import { type RegistryActivationPlanV1 } from './releaseRegistry';

const PAYER = '4Nd1mBQtrMJVYVfKf2PJy9NZUZdTAsp7D4xWLs4gDB4T';
const OTHER = 'GDDMwNyyx8uB6zrqwBFHjLLG3TBYk2F8Az4yrQC5RzMp';

/// Only the field the gate reads is real; a plan reaching the gate has already
/// been produced by `prepareRegistryActivation`, whose own suite covers what it
/// demanded of the chain before returning one.
function plan(payer: string): RegistryActivationPlanV1 {
  return { payer } as unknown as RegistryActivationPlanV1;
}

describe('checked-release wallet un-gate', () => {
  it('stays shut without a plan, without a wallet, and for the wrong wallet', () => {
    for (const shut of [
      releaseUngateV1(null, null),
      releaseUngateV1(null, PAYER),
      releaseUngateV1(plan(PAYER), null),
      releaseUngateV1(plan(PAYER), ''),
      releaseUngateV1(plan(PAYER), OTHER),
    ]) {
      expect(shut.open).toBe(false);
      expect(shut.reason).toContain(UNGATE_SHUT_V1);
      // A closed gate must never carry the sentence a green plan licenses.
      expect(shut.reason).not.toContain(UNGATE_LICENCE_V1);
    }
  });

  it('names both identities when the connected wallet is not the declared payer', () => {
    const shut = releaseUngateV1(plan(PAYER), OTHER);
    expect(shut.reason).toContain(OTHER);
    expect(shut.reason).toContain(PAYER);
  });

  it('opens only for a green plan signed by exactly its declared fee payer', () => {
    const open = releaseUngateV1(plan(PAYER), PAYER);
    expect(open.open).toBe(true);
    expect(open.reason).toBe(UNGATE_LICENCE_V1);
    // The licence is bounded: it must state what it does NOT authorize.
    expect(open.reason).toContain('does not make these addresses official');
    expect(open.reason).toContain('does not make this frontend official');
    expect(open.reason).toContain('does not transfer to devnet or mainnet');
    // A near-miss on the payer is not a near-miss on the gate.
    expect(releaseUngateV1(plan(PAYER), `${PAYER} `).open).toBe(false);
    expect(releaseUngateV1(plan(PAYER), PAYER.toLowerCase()).open).toBe(false);
  });
});
