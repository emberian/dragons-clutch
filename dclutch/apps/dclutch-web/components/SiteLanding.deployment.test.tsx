import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';

vi.mock('@/lib/deploymentStore', async () => {
  const actual = await vi.importActual<typeof import('@/lib/deploymentStore')>('@/lib/deploymentStore');
  const { LOCAL_DEPLOYMENT_V1 } = await import('@dclutch/sdk/deployments');
  return { ...actual, useDeploymentV1: () => LOCAL_DEPLOYMENT_V1 };
});

const { default: SiteLanding } = await import('./SiteLanding');

describe('the homepage follows the selected deployment', () => {
  it('shows local counts and provenance without attaching the devnet evidence download', () => {
    const html = renderToStaticMarkup(<SiteLanding />);
    expect(html).toContain('Local deployment selected');
    expect(html).toContain('7 program addresses in the Local configuration');
    expect(html).not.toContain('On devnet');
    expect(html).not.toContain('dclutch-devnet-deployment-evidence-v1.json');
    expect(html).toContain('Selected deployment: Local');
  });
});
