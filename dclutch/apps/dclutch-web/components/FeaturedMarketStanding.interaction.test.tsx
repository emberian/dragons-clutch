// @vitest-environment jsdom
import { act } from 'react';
import { createRoot } from 'react-dom/client';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { DEVNET_DEPLOYMENT_V1, LOCAL_DEPLOYMENT_V1, type DeploymentV1 } from '@dclutch/sdk/deployments';
import FeaturedMarketStanding from './FeaturedMarketStanding';

type Account = Readonly<{ owner: string; data: Uint8Array }>;
const state = vi.hoisted(() => ({
  deployment: null as DeploymentV1 | null,
  listeners: new Set<() => void>(),
  pending: [] as Array<(value: { account: Account | null }) => void>,
}));
vi.mock('@/lib/deploymentStore', async () => {
  const { useSyncExternalStore } = await import('react');
  return { useDeploymentV1: () => useSyncExternalStore(
    (listener) => { state.listeners.add(listener); return () => state.listeners.delete(listener); },
    () => state.deployment,
    () => state.deployment,
  ) };
});
vi.mock('@dclutch/sdk/publicCutStaging', async () => {
  const actual = await vi.importActual<typeof import('@dclutch/sdk/publicCutStaging')>('@dclutch/sdk/publicCutStaging');
  return { ...actual, PUBLIC_DEVNET_CUT_V1: { ...actual.PUBLIC_DEVNET_CUT_V1, market: '11111111111111111111111111111111' } };
});
vi.mock('@dclutch/sdk/rpc', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@dclutch/sdk/rpc')>();
  return { ...actual, SolanaRpcClient: class {
    async finalizedSlot() { return '5'; }
    accountInfo() { return new Promise((resolve) => state.pending.push(resolve)); }
  } };
});
vi.mock('@dclutch/sdk/marketCoreV2', () => ({
  decodeMarketCoreStateV2: (_address: string, bytes: Uint8Array) => ({ phase: bytes[0] === 0 ? 'Open' : 'Terminal' }),
}));
vi.mock('@/components/Anchor', () => ({
  default: ({ children, href }: Readonly<{ children: React.ReactNode; href: string }>) => <a href={href}>{children}</a>,
}));
Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true });
const roots: Array<ReturnType<typeof createRoot>> = [];
afterEach(async () => {
  await act(async () => { for (const root of roots.splice(0)) root.unmount(); });
  document.body.replaceChildren();
  state.listeners.clear();
  state.pending.length = 0;
});

async function choose(deployment: DeploymentV1) {
  await act(async () => {
    state.deployment = deployment;
    for (const listener of state.listeners) listener();
  });
}

for (const changed of ['endpoint', 'core'] as const) {
  describe(`featured market after changing ${changed}`, () => {
    it('clears the previous phase immediately and ignores late reads after switching back', async () => {
      const first = DEVNET_DEPLOYMENT_V1;
      const second: DeploymentV1 = {
        ...first,
        ...(changed === 'endpoint' ? { endpoint: 'http://127.0.0.1:23456/' } : {
          programs: { ...first.programs, core: LOCAL_DEPLOYMENT_V1.programs.core },
        }),
      };
      state.deployment = first;
      const container = document.createElement('div');
      document.body.appendChild(container);
      const root = createRoot(container);
      roots.push(root);
      await act(async () => root.render(<FeaturedMarketStanding />));
      expect(state.pending).toHaveLength(1);
      await act(async () => state.pending[0]({ account: { owner: first.programs.core, data: new Uint8Array([0]) } }));
      expect(container.querySelector('strong')?.textContent).toBe('open');

      await choose(second);
      expect(state.pending).toHaveLength(2);
      expect(container.querySelector('strong')).toBeNull();
      await choose(first);
      expect(state.pending).toHaveLength(3);
      expect(container.querySelector('strong')).toBeNull();
      await act(async () => state.pending[1]({ account: { owner: second.programs.core, data: new Uint8Array([1]) } }));
      expect(container.querySelector('strong')).toBeNull();
      await act(async () => state.pending[2]({ account: { owner: first.programs.core, data: new Uint8Array([0]) } }));
      expect(container.querySelector('strong')?.textContent).toBe('open');
    });
  });
}
