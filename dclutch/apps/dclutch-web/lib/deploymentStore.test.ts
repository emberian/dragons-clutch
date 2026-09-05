import { afterEach, describe, expect, it } from 'vitest';

import { DEVNET_DEPLOYMENT_V1, LOCAL_DEPLOYMENT_V1 } from '@dclutch/sdk/deployments';
import {
  activeDeploymentV1,
  chooseClusterV1,
  storeCustomDeploymentV1,
  storedCustomDeploymentV1,
} from './deploymentStore';

/**
 * The store under a simulated browser: `window` with a real-enough
 * `localStorage`. The headless case (no `window` at all — the prerender, and
 * every node-side import) is exercised by removing the stub.
 */

type Stub = Readonly<{ values: Map<string, string> }>;

function installWindow(): Stub {
  const values = new Map<string, string>();
  const stub = {
    localStorage: {
      getItem: (key: string) => values.get(key) ?? null,
      setItem: (key: string, value: string) => void values.set(key, value),
      removeItem: (key: string) => void values.delete(key),
    },
    addEventListener: () => undefined,
    removeEventListener: () => undefined,
    dispatchEvent: () => true,
  };
  (globalThis as Record<string, unknown>).window = stub;
  return { values };
}

function removeWindow(): void {
  delete (globalThis as Record<string, unknown>).window;
}

afterEach(removeWindow);

describe('the deployment store', () => {
  it('defaults to devnet with no window at all (the prerender case)', () => {
    removeWindow();
    expect(activeDeploymentV1()).toBe(DEVNET_DEPLOYMENT_V1);
    // Choosing without storage does not throw; the selection just cannot persist.
    expect(() => chooseClusterV1('local')).not.toThrow();
  });

  it('defaults to devnet with an empty storage, and follows an explicit choice', () => {
    installWindow();
    chooseClusterV1('devnet');
    expect(activeDeploymentV1()).toBe(DEVNET_DEPLOYMENT_V1);
    chooseClusterV1('local');
    expect(activeDeploymentV1()).toBe(LOCAL_DEPLOYMENT_V1);
  });

  it('stores, activates, and round-trips a Custom deployment', () => {
    const stub = installWindow();
    const stored = storeCustomDeploymentV1({
      endpoint: 'http://127.0.0.1:21890',
      programs: LOCAL_DEPLOYMENT_V1.programs,
    });
    expect(stored.cluster).toBe('custom');
    expect(stub.values.get('dclutch.cluster.v1')).toBe('custom');
    expect(activeDeploymentV1().endpoint).toBe('http://127.0.0.1:21890/');
    expect(storedCustomDeploymentV1()?.programs.core).toBe(LOCAL_DEPLOYMENT_V1.programs.core);
  });

  it('falls back to the default when the Custom selection has no parseable deployment behind it', () => {
    const stub = installWindow();
    stub.values.set('dclutch.customDeployment.v1', '{"endpoint":"http://127.0.0.1:8899","programs":{}}');
    chooseClusterV1('custom');
    expect(activeDeploymentV1()).toBe(DEVNET_DEPLOYMENT_V1);
    stub.values.set('dclutch.customDeployment.v1', 'not json at all');
    chooseClusterV1('custom');
    expect(activeDeploymentV1()).toBe(DEVNET_DEPLOYMENT_V1);
  });

  it('refuses to store a malformed Custom deployment, naming the field', () => {
    installWindow();
    expect(() => storeCustomDeploymentV1({ endpoint: 'http://127.0.0.1:8899', programs: { ...LOCAL_DEPLOYMENT_V1.programs, trading: 'nope' } }))
      .toThrow('trading program is not a Solana address');
  });
});
