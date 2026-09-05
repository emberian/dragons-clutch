import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import JoinPanel, { JoinStanding, joiningClosedForPhaseV1 } from '@/components/JoinPanel';
import { type DirectParticipantReadinessV1 } from '@dclutch/sdk/directParticipant';

const WALLET = '5oGySWQAKZ3fLmAwUbG6WifP7dCF6FRtriawtgxoCZXf';
const DEVNET = 'https://api.devnet.solana.com';

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
    const html = renderToStaticMarkup(<JoinStanding readiness={READY} marketPhase="Open" walletAddress={WALLET} endpoint={DEVNET} />);
    expect(html).toContain('You are a participant on this market.');
    expect(html).toContain('finalized slot 4242');
    expect(html).toContain(COORDINATES.position);
    expect(html).toContain(COORDINATES.collateral);
    expect(html).toContain('100 · 0');
    expect(html).toContain('5000000');
    expect(html).toContain('The trade panel below trades against exactly these accounts.');
  });

  it('tells a non-participant exactly what joining creates, and now offers it', () => {
    // WAS: this test pinned the published `dclutch --rpc … --execute` runbook
    // and the sentence "cannot yet build the admission transaction itself".
    // Both were honest while the browser could not compose the frame. It can
    // now — the compiled planner does — so the assertions move with the
    // behaviour rather than the wall being quietly deleted from under them.
    const html = renderToStaticMarkup(
      <JoinStanding readiness={INCOMPLETE} marketPhase="Open" walletAddress={WALLET} endpoint={DEVNET} admission={{
        market: 'EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG', owner: WALLET,
        coreProgramId: '11111111111111111111111111111112', claimsProgramId: '11111111111111111111111111111113',
        tradingProgramId: '11111111111111111111111111111114', registryProgramId: '11111111111111111111111111111115',
        rentProgramId: '11111111111111111111111111111117', activationCache: '11111111111111111111111111111118',
      }} />);
    expect(html).toContain('This wallet is not a participant here yet.');
    expect(html).toContain('Position and admission');
    expect(html).toContain('collateral account');
    expect(html).toContain(COORDINATES.position);
    // The act, not a command.
    expect(html).toContain('Join this market');
    expect(html).toContain('Nothing is signed');
    // The old wall, and the old apology for it, are both gone.
    expect(html).not.toContain('cannot yet build the admission transaction itself');
    expect(html).not.toContain('not a policy');
    // Still no button that cannot tell the truth: planning happens first, and
    // signing is a separate, explicit step after the reader sees the frame.
    expect(html).not.toContain('>Sign and join<');
  });

  it('refuses joining a terminal market in reader language instead of offering it', () => {
    const html = renderToStaticMarkup(<JoinStanding readiness={INCOMPLETE} marketPhase="Terminal" walletAddress={WALLET} endpoint={DEVNET} />);
    expect(html).toContain('This market has already resolved');
    expect(html).not.toContain('How to join');
    expect(html).not.toContain('dclutch-terminal join');
  });

  it('passes a refusal through verbatim', () => {
    const refused: DirectParticipantReadinessV1 = Object.freeze({ status: 'refused' as const, reason: 'the Market root is not CoreState v2' });
    const html = renderToStaticMarkup(<JoinStanding readiness={refused} marketPhase="Open" walletAddress={WALLET} endpoint={DEVNET} />);
    expect(html).toContain('Refused: the Market root is not CoreState v2');
  });

  it('treats terminal and retiring phases as closed and every other phase as joinable', () => {
    expect(joiningClosedForPhaseV1('Terminal')).toBe(true);
    expect(joiningClosedForPhaseV1('Retiring')).toBe(true);
    expect(joiningClosedForPhaseV1('Open')).toBe(false);
    expect(joiningClosedForPhaseV1('Founding')).toBe(false);
  });

  const ADMISSION = Object.freeze({
    market: 'EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG',
    owner: WALLET,
    coreProgramId: '11111111111111111111111111111112',
    claimsProgramId: '11111111111111111111111111111113',
    tradingProgramId: '11111111111111111111111111111114',
    registryProgramId: '11111111111111111111111111111115',
    rentProgramId: '11111111111111111111111111111117',
    activationCache: '11111111111111111111111111111118',
  });
  // Rendered in the exact state that used to publish the command: a connected
  // wallet that is not yet a participant.
  const admitted = renderToStaticMarkup(
    <JoinStanding readiness={INCOMPLETE} marketPhase="Open" walletAddress={WALLET} endpoint={DEVNET} admission={ADMISSION} />);

  it('no longer says the browser cannot build the transaction', () => {
    // The sentence this whole campaign was aimed at. It was true, and it was
    // why maker/taker trade was unreachable for a stranger: you cannot trade
    // in a market you cannot join.
    expect(admitted).not.toContain('cannot yet build the admission transaction itself');
    expect(admitted).not.toContain('Joining runs through the');
  });

  it('offers admission as an act in this browser, and names its authority', () => {
    expect(admitted).toContain('Join this market');
    expect(admitted).toContain('compiled Rust planner');
    expect(admitted).toContain('checked against');
  });

  it('publishes no CLI command for admission', () => {
    // A published `--execute` line is what a console offers when it cannot do
    // the thing. This one can.
    expect(admitted).not.toContain('--execute');
    expect(admitted).not.toContain('$POSITION_KEYPAIR');
    expect(admitted).not.toContain('dclutch --rpc');
  });

  it('does not offer the act when the deployment cannot derive the frame', () => {
    // Offering it and refusing after a reader commits is the worse failure.
    const partial = renderToStaticMarkup(
      <JoinStanding readiness={INCOMPLETE} marketPhase="Open" walletAddress={WALLET} endpoint={DEVNET} />);
    expect(partial).toContain('does not name every program the admission frame needs');
    expect(partial).not.toContain('Join this market');
  });
});
