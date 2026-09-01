import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import OperatorSurface, { packetExportReadyV1, type PacketExportStateV1 } from './OperatorSurface';

describe('operator surface presentation', () => {
  it('shows executable breadth and exact refusal boundaries without invented state', () => {
    const html = renderToStaticMarkup(<OperatorSurface />);
    expect(html).toContain('Operations.');
    expect(html).toContain('Every route still requires its own preflight');
    expect(html).toContain('does not make a route executable');
    expect(html).toContain('Use checked live-devnet preset');
    expect(html).toContain('instead of typing six program addresses');
    expect(html).toContain('supplies no Market');
    expect(html).toContain('No chain state has been read');
    expect(html).toContain('each route still authenticates its own release');
    expect(html).toContain('The whole census, including what has no venue');
    expect(html).toContain('Found a Market and admit its first participant');
    expect(html).toContain('Author a portable sell offer');
    expect(html).toContain('Export a portable Direct route');
    expect(html).toContain('Take and execute a signed offer');
    expect(html).toContain('This browser \u00b7 one detached message signature');
    expect(html).toContain('This browser \u00b7 one wallet signature, sent from here');
    expect(html).toContain('This browser \u00b7 one wallet signature, exported as a file');
    expect(html).toContain('Reacquire one Market above to open its exact participant flow');
    expect(html).toContain('Reacquire the multiprogram deployment');
    expect(html).toContain('Create a registered resting order');
    expect(html).toContain('Check a settlement plan and export its exact packet');
    expect(html).toContain('Take an inventory-bounded immediate trade');
    expect(html).toContain('Redeem a terminal Claims Position');
    expect(html).toContain('Inspect, reacquire, then export');
    expect(html).toContain('Export the portable Direct route');
    expect(html).toContain('Checked releases + frozen Direct session');
    expect(html).toContain('Finalized devnet reads only');
    expect(html).toContain('One route + one report');
    expect(html).toContain('Show the exact CLI invocation');
    expect(html).toContain('route release-set');
    expect(html).toContain('route direct');
    expect(html).toContain('expected-checked-execution-release-sha256');
    expect(html).not.toContain('--keypair');
    expect(html).toContain('Inspect unsigned packet');
    expect(html).toContain('Reacquire packet dependencies');
    expect(html).toContain('Download exact unsigned bytes');
    expect(html).toContain('SHA-256');
    expect(html).toContain('No wallet is requested here');
    expect(html).toContain('data-slot="button"');
    expect(html.indexOf('Inspect unsigned packet')).toBeLessThan(html.indexOf('Reacquire packet dependencies'));
    expect(html.indexOf('Reacquire packet dependencies')).toBeLessThan(html.indexOf('Download exact unsigned bytes'));
    expect(html.indexOf('Export the portable Direct route')).toBeLessThan(html.indexOf('The whole census, including what has no venue'));
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
    expect(html).toContain('nothing here is a status anyone typed');
    expect(html).toContain('acts this browser builds');
    expect(html).toContain('acts with no venue and a named wall');
    expect(html).toContain('Known wall');
    expect(html).toContain('crates/dclutch-dealer-scenario-kernel');
    expect(html).toContain('WAVE.md');
    // The vocabulary of a roadmap, in every spelling this surface has used.
    for (const word of ['awaiting production', 'coming soon', 'unavailable', 'greyed-out', 'rust unsigned']) {
      expect(html.toLowerCase()).not.toContain(word);
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
