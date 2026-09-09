import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';

// The closed face, pinned against a mock pending cut now that the published
// fixture names a Market. Its companion, LaunchStory.opened.test.tsx, renders
// the open face the same way; between them both states stay guarded whatever
// the fixture currently says.
vi.mock('@dclutch/sdk/publicCutStaging', async () => {
  const actual = await vi.importActual<typeof import('@dclutch/sdk/publicCutStaging')>('@dclutch/sdk/publicCutStaging');
  const cut = actual.parsePublicDevnetCutV1({
    schema: 'dclutch-public-cut-v1',
    cluster: 'devnet',
    market: null,
    activity: { found: null, trade: null, resolve: null, redeem: null },
    checkedReleases: {},
  });
  return { ...actual, PUBLIC_DEVNET_CUT_V1: cut };
});

const { DEVNET_DEPLOYMENT_V1, deployedProgramRolesV1 } = await import('@dclutch/sdk/deployments');
const { default: LaunchStory } = await import('./LaunchStory');

describe('launch story', () => {
  it('links discovery when no featured market is configured', () => {
    const html = renderToStaticMarkup(<LaunchStory />);
    expect(html).toContain('No featured market is linked yet.');
    expect(html).toContain('href="/markets"');
    expect(html).toContain('href="/explorer"');
    expect(html).toContain('href="/activity"');
    expect(html).toContain('<strong>NONE</strong><span>featured market</span>');
    expect(html).toContain('href="/campaign"');
    expect(html).toContain('href="/population"');
  });

  it('renders the configured deployment programs', () => {
    const html = renderToStaticMarkup(<LaunchStory />);
    expect(html).toContain(`<strong>${deployedProgramRolesV1(DEVNET_DEPLOYMENT_V1).length}</strong><span>programs</span>`);
    for (const role of deployedProgramRolesV1(DEVNET_DEPLOYMENT_V1)) {
      const address = DEVNET_DEPLOYMENT_V1.programs[role];
      expect(html).toContain(`${address.slice(0, 5)}…${address.slice(-5)}`);
    }
    expect(html).toContain('Solana devnet');
  });
});
