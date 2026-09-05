import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';

import { directTicketStateV1, type TicketState } from './tradeFlowMachine';
import { type DirectTradeSpineV1 } from '@dclutch/sdk/directTradeSpine';
import { type WalletDirectoryHandleV1 } from '@/components/WalletDirectory';

const VECTOR = JSON.parse(readFileSync(new URL('../../../packages/dclutch-sdk/fixtures/direct-intent-ticket.json', import.meta.url), 'utf8'));

/** Only `inspected === null` is read here, so the shape is all this needs. */
const INSPECTED = { status: 'inspected' } as unknown as Extract<DirectTradeSpineV1, { status: 'inspected' }>;
const CONNECTED = { address: '8bcRzB3v6PxbbtkVCiX9ceW2whwakA6gX7qvSYbeMHLq' } as unknown as WalletDirectoryHandleV1;
const DISCONNECTED = { address: null } as unknown as WalletDirectoryHandleV1;
const CLAIMS = 'Ho1dNL8bCcYo1zoTfQhrsAaTPTwFhXFRSXTGGwiHVGVE';

function state(overrides: Partial<Parameters<typeof directTicketStateV1>[0]>): TicketState {
  return directTicketStateV1({
    inspected: INSPECTED,
    ticketText: VECTOR.ticketText,
    wallets: CONNECTED,
    claimsProgramId: CLAIMS,
    ...overrides,
  });
}

// The ticket state used to be a useMemo inside an 800-line component, which
// meant its four branches could only be exercised by rendering the whole
// panel. Lifting it into the machine is what makes them reachable at all.
describe('the ticket the flow is holding', () => {
  it('decodes a real signed ticket into the ready state', () => {
    const ready = state({});
    expect(ready.kind).toBe('ready');
    if (ready.kind !== 'ready') throw new Error('expected the fixture ticket to decode');
    expect(ready.ticket.maker).toBe(VECTOR.maker);
    expect(ready.ticket.intent.outcome).toBe(3);
    expect(ready.ticket.intent.maximumFill).toBe(100_000_000n);
    expect(ready.ticket.intent.limitPrice).toBe(500_000n);
  });

  it('holds nothing until the chain has been asked, and nothing for an empty box', () => {
    expect(state({ inspected: null }).kind).toBe('none');
    expect(state({ ticketText: '' }).kind).toBe('none');
    expect(state({ ticketText: '   ' }).kind).toBe('none');
  });

  // Both refusals name what to do next, which is the property that has to
  // survive the move: a refusal is the protocol working, not an error state.
  it('names the two things it needs before it will read a ticket', () => {
    const noWallet = state({ wallets: DISCONNECTED });
    expect(noWallet.kind).toBe('refused');
    if (noWallet.kind !== 'refused') throw new Error('expected a refusal');
    expect(noWallet.reason).toBe('connect a browser wallet: the ticket is crossed against the connected identity');

    for (const claims of [null, '']) {
      const noClaims = state({ claimsProgramId: claims });
      expect(noClaims.kind).toBe('refused');
      if (noClaims.kind !== 'refused') throw new Error('expected a refusal');
      expect(noClaims.reason).toBe('select the Claims program before checking the participant admission evidence');
    }
  });

  // The decoder's own refusals are surfaced verbatim rather than replaced by
  // a generic message -- eleven distinct, remedy-shaped sentences survive.
  it('passes the decoder refusal through in the decoder own words', () => {
    const notJson = state({ ticketText: 'not json at all' });
    expect(notJson.kind).toBe('refused');
    if (notJson.kind !== 'refused') throw new Error('expected a refusal');
    expect(notJson.reason).toContain('ticket is not valid JSON');

    const wrongKind = state({ ticketText: JSON.stringify({ kind: 'something/else/v1' }) });
    expect(wrongKind.kind).toBe('refused');
    if (wrongKind.kind !== 'refused') throw new Error('expected a refusal');
    expect(wrongKind.reason).toContain('dclutch/direct-intent-ticket/v1');
  });
});
