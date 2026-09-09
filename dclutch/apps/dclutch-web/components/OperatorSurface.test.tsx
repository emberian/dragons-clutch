import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import OperatorSurface, { packetExportReadyV1, type PacketExportStateV1 } from './OperatorSurface';
import { browserActPrerequisitesV1, BROWSER_CAPABILITY_STANDINGS_V1 } from '@/lib/capabilitySurface';

describe('operator surface presentation', () => {
  it('shows executable breadth and exact refusal boundaries without invented state', () => {
    const html = renderToStaticMarkup(<OperatorSurface />);
    expect(html).toContain('Operations.');
    expect(html).toContain('Inspect a deployment, find an action');
    expect(html).toContain('Choose an action below');
    expect(html).toContain('Use devnet preset');
    expect(html).toContain('enter a custom deployment');
    expect(html).toContain('Market (optional)');
    expect(html).toContain('No chain state has been read');
    expect(html).toContain('open its wallet flow or CLI instructions');
    expect(html).toContain('Protocol actions');
    expect(html).toContain('Found a Market and admit its first participant');
    expect(html).toContain('Author a portable sell offer');
    expect(html).toContain('Export a portable Direct route');
    expect(html).toContain('Take and execute a signed offer');
    expect(html).toContain('This browser \u00b7 one detached message signature');
    expect(html).toContain('This browser \u00b7 one wallet signature, sent from here');
    expect(html).toContain('This browser \u00b7 one wallet signature, exported as a file');
    expect(html).toContain('Enter a Market above to open its actions');
    expect(html).toContain('Inspect a deployment');
    expect(html).toContain('Create a registered resting order');
    expect(html).toContain('Check a settlement plan and export its exact packet');
    expect(html).toContain('Take an inventory-bounded immediate trade');
    expect(html).toContain('Redeem a terminal Claims Position');
    expect(html).toContain('Inspect and export a transaction');
    expect(html).toContain('Export the portable Direct route');
    expect(html).toContain('Checked releases + frozen Direct session');
    expect(html).toContain('Solana devnet');
    expect(html).toContain('One route + one report');
    expect(html).toContain('Show the exact CLI invocation');
    expect(html).toContain('route release-set');
    expect(html).toContain('route direct');
    expect(html).toContain('expected-checked-execution-release-sha256');
    expect(html).not.toContain('--keypair');
    expect(html).toContain('Inspect unsigned packet');
    expect(html).toContain('Check transaction accounts');
    expect(html).toContain('Download unsigned transaction');
    expect(html).toContain('SHA-256');
    expect(html).toContain('Sign the downloaded transaction in your wallet client');
    expect(html).toContain('data-slot="button"');
    expect(html.indexOf('Inspect unsigned packet')).toBeLessThan(html.indexOf('Check transaction accounts'));
    expect(html.indexOf('Check transaction accounts')).toBeLessThan(html.indexOf('Download unsigned transaction'));
    expect(html.indexOf('Export the portable Direct route')).toBeLessThan(html.indexOf('Protocol actions'));
    expect(html).not.toContain('External identity boundary');
    expect(html).not.toContain('Optional. This surface never signs or submits.');
    expect(html).toContain('No chain state has been read.');
    expect(html).not.toContain('Illustrative');
    expect(html).not.toContain('mock');
  });

  /**
   * The census renders what the code can do, and says so about what it cannot.
   *
   * These are the two failures this surface has actually had. It used to print
   * a hand-typed implementation word beside every act -- so it could say
   * `rust unsigned` above a page that signs -- and it used to answer an act it
   * could not open with a disabled button, which says no and cannot say why.
   */
  it('names each act by where it runs, and names a wall where it runs nowhere', () => {
    const html = renderToStaticMarkup(<OperatorSurface />);
    expect(html).toContain('Each entry lists its requirements');
    expect(html).toContain('browser actions');
    expect(html).toContain('unavailable actions');
    expect(html).toContain('Unavailable:');
    expect(html).toContain('crates/dclutch-trading');
    expect(html).toContain('WAVE.md');
    // The vocabulary of a roadmap, in every spelling this surface has used.
    for (const word of ['awaiting production', 'coming soon', 'greyed-out', 'rust unsigned']) {
      expect(html.toLowerCase()).not.toContain(word);
    }
  });

  it('names what an act cannot be started without, beside what it does', () => {
    // The census says where each act runs and what it promises. Neither
    // sentence could ever say that `claims.redeem` opens a file picker for a
    // payout plan only a Rust binary authors, so a reader planning a session
    // read "one wallet signature, sent from here" and planned a session that
    // stops at step two. Derived in `browserActPrerequisitesV1`, rendered
    // here beside the guarantee it qualifies.
    const html = renderToStaticMarkup(<OperatorSurface />);
    const needing = BROWSER_CAPABILITY_STANDINGS_V1
      .filter((standing) => browserActPrerequisitesV1(standing).some((entry) => entry.id === 'external-file'));
    expect(needing.length, 'no act reads a file, so this assertion proves nothing').toBeGreaterThan(0);
    for (const standing of needing) {
      const outcome = html.indexOf(standing.action.action);
      expect(outcome, `${standing.action.id} is not on the census`).toBeGreaterThanOrEqual(0);
      expect(
        html.indexOf('a file this browser cannot produce', outcome),
        `${standing.action.id} needs a file this browser cannot produce and the census does not say so`,
      ).toBeGreaterThan(outcome);
    }
  });

  it('closes packet export when the endpoint, artifact, or reacquisition state changes', () => {
    const packet = {
      endpoint: 'https://rpc.example',
      sourceText: 'AAAA',
      report: { missing: [], nonExecutablePrograms: [] },
    } as PacketExportStateV1;
    expect(packetExportReadyV1(packet, 'https://rpc.example', 'AAAA')).toBe(true);
    expect(packetExportReadyV1(packet, 'https://other.example', 'AAAA')).toBe(false);
    expect(packetExportReadyV1(packet, 'https://rpc.example', 'BBBB')).toBe(false);
    expect(packetExportReadyV1({ ...packet, report: null }, 'https://rpc.example', 'AAAA')).toBe(false);
  });
});
