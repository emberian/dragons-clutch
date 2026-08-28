import { useCallback, useState, useSyncExternalStore } from 'react';

import {
  DEFAULT_DEPLOYMENT_V1,
  DEVNET_DEPLOYMENT_V1,
  LOCAL_DEPLOYMENT_V1,
  parseCustomDeploymentV1,
  type ClusterIdV1,
  type DeploymentV1,
} from './deployments';

/**
 * The ACTIVE deployment — one browser-wide selection, read everywhere.
 *
 * The selection is the reader's, so it lives in `localStorage` and survives
 * navigation (every route change here is a full page load); the deployments
 * themselves are the baked manifest in `lib/deployments.ts`, except for the
 * one Custom slot the picker stores alongside the selection. A server render
 * — and the static export's prerender — always sees the default (devnet), and
 * the client corrects itself right after hydration; that is the documented
 * `useSyncExternalStore` contract, not a hydration mismatch.
 *
 * Nothing in this module performs IO. A surface that wants chain state builds
 * its own `SolanaRpcClient` from the deployment it reads here.
 */

const CLUSTER_KEY = 'dclutch.cluster.v1';
const CUSTOM_KEY = 'dclutch.customDeployment.v1';
// Not `dclutch:<name>`: that shape is a PDA seed domain, and the ABI coverage
// ratchet rightly refuses a hand-stated one. This is only a DOM event name.
const CHANGE_EVENT = 'dclutch-deployment-changed';

type StorageLike = Pick<Storage, 'getItem' | 'setItem' | 'removeItem'>;

/** `localStorage` where it exists and is permitted; null everywhere else. */
function storage(): StorageLike | null {
  try {
    if (typeof window === 'undefined') return null;
    return window.localStorage;
  } catch {
    return null;
  }
}

function readCluster(store: StorageLike | null): ClusterIdV1 {
  try {
    const stored = store?.getItem(CLUSTER_KEY);
    if (stored === 'devnet' || stored === 'local' || stored === 'custom') return stored;
  } catch {
    // an unreadable selection is the default selection
  }
  return DEFAULT_DEPLOYMENT_V1.cluster;
}

/** The stored Custom deployment, or null when none parses. */
export function storedCustomDeploymentV1(): DeploymentV1 | null {
  try {
    const raw = storage()?.getItem(CUSTOM_KEY);
    if (raw === null || raw === undefined) return null;
    return parseCustomDeploymentV1(JSON.parse(raw));
  } catch {
    return null;
  }
}

function resolve(): DeploymentV1 {
  const cluster = readCluster(storage());
  if (cluster === 'local') return LOCAL_DEPLOYMENT_V1;
  if (cluster === 'custom') {
    // A Custom selection whose stored deployment no longer parses falls back
    // to the default rather than presenting a half-deployment.
    return storedCustomDeploymentV1() ?? DEFAULT_DEPLOYMENT_V1;
  }
  return DEVNET_DEPLOYMENT_V1;
}

let cached: DeploymentV1 | null = null;

function snapshot(): DeploymentV1 {
  if (cached === null) cached = resolve();
  return cached;
}

function serverSnapshot(): DeploymentV1 {
  return DEFAULT_DEPLOYMENT_V1;
}

function announce(): void {
  cached = null;
  if (typeof window !== 'undefined') window.dispatchEvent(new Event(CHANGE_EVENT));
}

function subscribe(onChange: () => void): () => void {
  const handle = () => {
    cached = null;
    onChange();
  };
  window.addEventListener(CHANGE_EVENT, handle);
  window.addEventListener('storage', handle);
  return () => {
    window.removeEventListener(CHANGE_EVENT, handle);
    window.removeEventListener('storage', handle);
  };
}

/** The active deployment, outside React (event handlers, one-shot reads). */
export function activeDeploymentV1(): DeploymentV1 {
  return snapshot();
}

/** Select one of the named clusters. Selecting `custom` requires a stored Custom deployment. */
export function chooseClusterV1(cluster: ClusterIdV1): void {
  try {
    storage()?.setItem(CLUSTER_KEY, cluster);
  } catch {
    // nowhere to remember it; the in-page selection still applies until reload
  }
  announce();
}

/** Validate, store, and activate one Custom deployment. Throws with the field named. */
export function storeCustomDeploymentV1(raw: unknown): DeploymentV1 {
  const deployment = parseCustomDeploymentV1(raw);
  try {
    storage()?.setItem(CUSTOM_KEY, JSON.stringify({
      endpoint: deployment.endpoint,
      programs: deployment.programs,
      activationCache: deployment.activationCache,
    }));
    storage()?.setItem(CLUSTER_KEY, 'custom');
  } catch {
    // nowhere to remember it; the in-page selection still applies until reload
  }
  announce();
  return deployment;
}

/** The active deployment, as React state: re-renders when the picker changes it. */
export function useDeploymentV1(): DeploymentV1 {
  return useSyncExternalStore(subscribe, snapshot, serverSnapshot);
}

/**
 * One editable field seeded from the active deployment.
 *
 * The consoles keep their explicit infrastructure fields — an operator can
 * point any console anywhere — but the fields arrive FILLED from the active
 * deployment instead of empty. An untouched field follows a deployment
 * change; an edited one is the operator's and stays put.
 */
export function useDeploymentFieldV1(select: (deployment: DeploymentV1) => string): [string, (value: string) => void] {
  const deployment = useDeploymentV1();
  const [override, setOverride] = useState<string | null>(null);
  const set = useCallback((value: string) => setOverride(value), []);
  return [override ?? select(deployment), set];
}
