import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import JoinPanel, { JoinStanding, joiningClosedForPhaseV1 } from '@/components/JoinPanel';
import { type DirectParticipantReadinessV1 } from '@/lib/directParticipant';

const WALLET = '5oGySWQAKZ3fLmAwUbG6WifP7dCF6FRtriawtgxoCZXf';

const COORDINATES = Object.freeze({
  aggregate: 'GcE6LWbduoATDgK8jsGyj2i8ywV37fcAYABKCmKgttDz',
  position: 'E9buaTm2SAovWXsaRBMPyfk5uhdryDVA744CfAogpoRR',
  admission: 'E71d4qisbiQbt8UGb2PPmmJanqUeUxyPcjLjXX14ooZX',
  collateral: '4JbuXbcAnVi95itMiZFu6sAhb7AgvYsv34hwpuweVhFQ',
  custodyAuthority: '4sDPhUBKLCbFxBc45XWBuRwcNG8tw78tct9MgxeVZFTY',
});

const READY: DirectParticipantReadinessV1 = Object.freeze({
  status: 'ready' as const,
  observedSlot: '4242',
  market: '57i7c6zwEEzySrt7a94FAbY6AWnEdK4jDZEYkTej4PrP',
  generation: 1n,
  owner: WALLET,
  coordinates: COORDINATES,
  collateralMint: '57i7c6zwEEzySrt7a94FAbY6AWnEdK4jDZEYkTej4PrP',
  tokenProgram: 'TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb',
  positionRevision: 3n,
  positionBalances: Object.freeze([100n, 0n]),
  collateralAtoms: 5_000_000n,
  delegatedCollateralAtoms: 0n,
  spendableCollateralAtoms: 5_000_000n,
  reason: 'participant accounts authenticated',
});

const INCOMPLETE: DirectParticipantReadinessV1 = Object.freeze({
  status: 'incomplete' as const,
  observedSlot: '4242',
  market: '57i7c6zwEEzySrt7a94FAbY6AWnEdK4jDZEYkTej4PrP',
  generation: 1n,
  owner: WALLET,
  coordinates: COORDINATES,
  missing: Object.freeze(['Position and admission' as const, 'collateral account' as const]),
  reason: 'no Position exists for this owner',
});

describe('the join surface', () => {
  it('opens with an honest no-wallet state and no signing implication', () => {
    const html = renderToStaticMarkup(<JoinPanel
      endpoint="http://127.0.0.1:20890/"
      marketAddress="57i7c6zwEEzySrt7a94FAbY6AWnEdK4jDZEYkTej4PrP"
      marketPhase="Open"
      coreProgramId="GcE6LWbduoATDgK8jsGyj2i8ywV37fcAYABKCmKgttDz"
      registryProgramId="E9buaTm2SAovWXsaRBMPyfk5uhdryDVA744CfAogpoRR"
      claimsProgramId="E71d4qisbiQbt8UGb2PPmmJanqUeUxyPcjLjXX14ooZX"
      tradingProgramId="4JbuXbcAnVi95itMiZFu6sAhb7AgvYsv34hwpuweVhFQ"
      custodyProgramId="4sDPhUBKLCbFxBc45XWBuRwcNG8tw78tct9MgxeVZFTY"
      rentProgramId="DXCBPpfxhJfLrXEuhUwNt8TgM3atocHomrKLUa17rzvp"
    />);
    expect(html).toContain('Join this market');
    expect(html).toContain('id="join"');
    expect(html).toContain('No wallet connected.');
    // The idle shell promises nothing quantitative and never fakes an action.
    for (const forbidden of ['probability', 'odds', 'APY', 'APR', 'instantly', 'one click']) {
      expect(html).not.toContain(forbidden);
    }
  });

  it('shows a participant their real accounts and balances in atoms', () => {
    const html = renderToStaticMarkup(<JoinStanding readiness={READY} marketPhase="Open" walletAddress={WALLET} />);
    expect(html).toContain('You are a participant on this market.');
    expect(html).toContain('finalized slot 4242');
    expect(html).toContain(COORDINATES.position);
    expect(html).toContain(COORDINATES.collateral);
    expect(html).toContain('100 · 0');
    expect(html).toContain('5000000');
    expect(html).toContain('The trade panel below trades against exactly these accounts.');
  });

  it('tells a non-participant exactly what joining creates and how, without a fake button', () => {
    const html = renderToStaticMarkup(<JoinStanding readiness={INCOMPLETE} marketPhase="Open" walletAddress={WALLET} />);
    expect(html).toContain('This wallet is not a participant here yet.');
    expect(html).toContain('Position and admission');
    expect(html).toContain('collateral account');
    expect(html).toContain(COORDINATES.position);
    expect(html).toContain('dclutch join');
    expect(html).toContain('--execute');
    expect(html).toContain(WALLET);
    // The wall is named as a gap, never dressed up as a virtue.
    expect(html).toContain('cannot yet build the admission transaction itself');
    // Renegotiated 2026-08-31: "that is a gap we intend to close, not a
    // policy" is us managing the reader's opinion of us. Deleted. The gap is
    // still named in the sentence above, which is the part that matters.
    expect(html).not.toContain('not a policy');
    expect(html).not.toContain('<button');
  });

  it('refuses joining a terminal market in reader language instead of offering it', () => {
    const html = renderToStaticMarkup(<JoinStanding readiness={INCOMPLETE} marketPhase="Terminal" walletAddress={WALLET} />);
    expect(html).toContain('This market has already resolved');
    expect(html).not.toContain('How to join');
    expect(html).not.toContain('dclutch join');
  });

  it('passes a refusal through verbatim', () => {
    const refused: DirectParticipantReadinessV1 = Object.freeze({ status: 'refused' as const, reason: 'the Market root is not CoreState v2' });
    const html = renderToStaticMarkup(<JoinStanding readiness={refused} marketPhase="Open" walletAddress={WALLET} />);
    expect(html).toContain('Refused: the Market root is not CoreState v2');
  });

  it('treats terminal and retiring phases as closed and every other phase as joinable', () => {
    expect(joiningClosedForPhaseV1('Terminal')).toBe(true);
    expect(joiningClosedForPhaseV1('Retiring')).toBe(true);
    expect(joiningClosedForPhaseV1('Open')).toBe(false);
    expect(joiningClosedForPhaseV1('Founding')).toBe(false);
  });
});
