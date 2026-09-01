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
    expect(html).toContain('Constructor readiness map');
    expect(html).toContain('Found a current Market and first participant');
    expect(html).toContain('Author a portable sell offer');
    expect(html).toContain('Export an authenticated Direct route');
    expect(html).toContain('Take and execute a Direct offer');
    expect(html).toContain('Wallet signs one detached message');
    expect(html).toContain('portable signed artifact');
    expect(html).toContain('Reacquire one Market above to open its exact participant flow');
    expect(html).toContain('Reacquire the multiprogram deployment');
    expect(html).toContain('Create registered order');
    expect(html).toContain('Initialize / collect / materialize / distribute');
    expect(html).toContain('Inventory-bounded immediate trade');
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
    expect(html.indexOf('Export the portable Direct route')).toBeLessThan(html.indexOf('Constructor readiness map'));
    expect(html).not.toContain('External identity boundary');
    expect(html).not.toContain('Optional. This surface never signs or submits.');
    expect(html).toContain('No chain state has been read.');
    expect(html).not.toContain('Illustrative');
    expect(html).not.toContain('mock');
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
