// @vitest-environment jsdom
import { act } from 'react';
import { createRoot } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { DEVNET_DEPLOYMENT_V1, LOCAL_DEPLOYMENT_V1, type DeploymentV1 } from '@dclutch/sdk/deployments';
import ChainExplorer from './ChainExplorer';

type Deferred<T> = Readonly<{ promise: Promise<T>; resolve: (value: T) => void }>;

const state = vi.hoisted(() => ({
  deployment: null as DeploymentV1 | null,
  listeners: new Set<() => void>(),
  accountCalls: [] as string[],
  accountFactory: null as ((address: string) => Promise<unknown>) | null,
  marketCalls: 0,
  recordCalls: [] as string[],
  recordFactory: null as (() => Promise<unknown>) | null,
}));

vi.mock('@/lib/deploymentStore', async () => {
  const { useSyncExternalStore } = await import('react');
  return {
    useDeploymentV1: () => useSyncExternalStore(
      (onChange) => {
        state.listeners.add(onChange);
        return () => state.listeners.delete(onChange);
      },
      () => state.deployment,
      () => state.deployment,
    ),
  };
});

vi.mock('@/components/PageShell', () => ({
  default: ({ children, header, onClick }: Readonly<{ children: React.ReactNode; header: React.ReactNode; onClick?: React.MouseEventHandler }>) => (
    <div onClick={onClick}>{header}{children}</div>
  ),
}));
vi.mock('@/components/Nav', () => ({ default: () => null }));
vi.mock('@/components/Anchor', () => ({
  default: ({ children, ...props }: Readonly<{ children: React.ReactNode; href: string }>) => <a {...props}>{children}</a>,
}));
vi.mock('@/components/PublicDeploymentEvidence', () => ({ default: () => null }));

vi.mock('@/lib/explorer/account', () => ({
  inspectAccount: vi.fn((_client: unknown, request: Readonly<{ address: string }>) => {
    state.accountCalls.push(request.address);
    if (state.accountFactory === null) throw new Error('account factory not installed');
    return state.accountFactory(request.address);
  }),
}));
vi.mock('@/lib/explorer/transaction', () => ({
  inspectTransaction: vi.fn(async () => ({ status: 'absent', signature: 'sig', reason: 'test' })),
}));
vi.mock('@/lib/explorer/marketLens', () => ({
  inspectMarketLens: vi.fn(async () => {
    state.marketCalls += 1;
    return { address: 'market', floorSlot: '1', bindings: [], gaps: [], nodes: [] };
  }),
}));
vi.mock('@/lib/explorer/protocolHome', () => ({
  classifySearchV1: (value: string) => ({ kind: 'account', address: value.trim() }),
  inspectProtocolHomeV1: vi.fn(async () => ({
    facts: { endpoint: 'http://test', genesisHash: 'genesis', solanaCore: 'test', featureSet: 'test' },
    observedSlot: '1',
    clusterName: 'test',
    clusterCheck: 'match',
    cards: [],
    activity: [],
    activityNote: 'test',
  })),
}));
vi.mock('@dclutch/sdk/rpc', async () => {
  const actual = await vi.importActual<typeof import('@dclutch/sdk/rpc')>('@dclutch/sdk/rpc');
  return { ...actual, SolanaRpcClient: class { constructor(readonly endpoint: string) {} } };
});
vi.mock('@dclutch/sdk/records', async () => {
  const actual = await vi.importActual<typeof import('@dclutch/sdk/records')>('@dclutch/sdk/records');
  return {
    ...actual,
    inspectFinalizedRecord: vi.fn((_client: unknown, programId: string) => {
      state.recordCalls.push(programId);
      if (state.recordFactory === null) throw new Error('record factory not installed');
      return state.recordFactory();
    }),
  };
});

Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true });

const A = DEVNET_DEPLOYMENT_V1.programs.core;
const B = DEVNET_DEPLOYMENT_V1.programs.registry;
const DEPLOYMENT_B: DeploymentV1 = Object.freeze({
  ...DEVNET_DEPLOYMENT_V1,
  cluster: 'custom',
  label: 'Custom',
  genesisHash: null,
  programs: Object.freeze({ ...DEVNET_DEPLOYMENT_V1.programs, core: LOCAL_DEPLOYMENT_V1.programs.core }),
  activationCache: null,
  provenance: 'test deployment',
});

const mounted: Array<ReturnType<typeof createRoot>> = [];

function deferred<T>(): Deferred<T> {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((next) => { resolve = next; });
  return { promise, resolve };
}

function accountResult(address: string, label = 'Devnet') {
  return { status: 'empty', address, floorSlot: '1', observedSlot: '1', reason: `${label}:${address}` };
}

function recordResult(marker: string) {
  return {
    status: 'structurally-final',
    floorSlot: marker,
    rawObservedSlot: '1',
    stagingObservedSlot: '1',
    rawAddress: A,
    stagingAddress: B,
    contentBytes: marker,
    checks: [{ label: 'marker', ok: true, detail: marker }],
    semanticDisposition: 'schema-validator-not-present-in-browser',
  };
}

async function settle(): Promise<void> {
  await act(async () => {
    await Promise.resolve();
    await Promise.resolve();
  });
}

async function renderExplorer() {
  const container = document.createElement('div');
  document.body.appendChild(container);
  const root = createRoot(container);
  mounted.push(root);
  await act(async () => root.render(<ChainExplorer />));
  await settle();
  return container;
}

function changeDeployment(deployment: DeploymentV1): void {
  state.deployment = deployment;
  for (const listener of [...state.listeners]) listener();
}

beforeEach(() => {
  state.deployment = DEVNET_DEPLOYMENT_V1;
  state.listeners.clear();
  state.accountCalls.length = 0;
  state.marketCalls = 0;
  state.recordCalls.length = 0;
  state.accountFactory = async (address) => accountResult(address);
  state.recordFactory = async () => recordResult('default');
  window.history.replaceState(null, '', `/explorer?view=account&q=${encodeURIComponent(A)}`);
});

afterEach(async () => {
  await act(async () => { for (const root of mounted.splice(0)) root.unmount(); });
  state.listeners.clear();
  document.body.replaceChildren();
});

describe('ChainExplorer navigation and read lifecycle', () => {
  it('follows browser popstate and starts one read for the new URL', async () => {
    const container = await renderExplorer();
    expect(container.querySelector<HTMLInputElement>('input[aria-label="Search the chain"]')!.value).toBe(A);

    await act(async () => {
      window.history.pushState(null, '', `/explorer?view=account&q=${encodeURIComponent(B)}`);
      window.dispatchEvent(new PopStateEvent('popstate'));
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(container.querySelector<HTMLInputElement>('input[aria-label="Search the chain"]')!.value).toBe(B);
    expect(state.accountCalls).toEqual([A, B]);
  });

  it('starts the same query again after it is cleared and reopened', async () => {
    state.accountFactory = async () => new Promise(() => undefined);
    const container = await renderExplorer();
    expect(state.accountCalls).toEqual([A]);

    await act(async () => container.querySelector<HTMLButtonElement>('.xp-clear')!.click());
    await settle();
    expect(state.accountCalls).toEqual([A]);

    await act(async () => {
      window.history.pushState(null, '', `/explorer?view=account&q=${encodeURIComponent(A)}`);
      window.dispatchEvent(new PopStateEvent('popstate'));
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(state.accountCalls).toEqual([A, A]);
  });

  it('does not duplicate an in-page tab navigation', async () => {
    const container = await renderExplorer();
    const marketTab = [...container.querySelectorAll<HTMLButtonElement>('button[role="tab"]')]
      .find((button) => button.textContent === 'Market lens')!;
    await act(async () => marketTab.click());
    await settle();
    expect(state.marketCalls).toBe(1);
  });

  it('can explicitly retry the same query after an RPC failure', async () => {
    let attempts = 0;
    state.accountFactory = async (address) => {
      attempts += 1;
      if (attempts === 1) throw new Error('first refusal');
      return accountResult(address);
    };
    const container = await renderExplorer();
    expect(state.accountCalls).toEqual([A]);
    expect(container.textContent).toContain('Account read refused');

    await act(async () => {
      container.querySelector('form.xp-chain')!.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));
      await Promise.resolve();
      await Promise.resolve();
    });
    await settle();
    expect(state.accountCalls).toEqual([A, A]);
    expect(container.textContent).not.toContain('Account read refused');
  });

  it('ignores an older URL read when a newer URL read finishes first', async () => {
    const pending = new Map<string, Deferred<unknown>>();
    state.accountFactory = async (address) => {
      const next = deferred<unknown>();
      pending.set(address, next);
      return next.promise;
    };
    const container = await renderExplorer();
    await act(async () => {
      window.history.pushState(null, '', `/explorer?view=account&q=${encodeURIComponent(B)}`);
      window.dispatchEvent(new PopStateEvent('popstate'));
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(state.accountCalls).toEqual([A, B]);

    await act(async () => pending.get(B)!.resolve(accountResult(B))); await settle();
    expect(container.textContent).toContain(`${B}`);
    await act(async () => pending.get(A)!.resolve(accountResult(A))); await settle();
    expect(container.textContent).toContain(`${B}`);
    expect(container.textContent).not.toContain(`Devnet:${A}`);
  });

  it('does not render a pending read after the explorer unmounts', async () => {
    state.accountFactory = async () => new Promise(() => undefined);
    const container = await renderExplorer();
    const root = mounted.pop()!;
    await act(async () => root.unmount());
    await settle();
    expect(container.textContent).toBe('');
  });

  it('re-reads same-endpoint deployments and rejects the old deployment result', async () => {
    const pending: Array<Deferred<unknown>> = [];
    state.accountFactory = async () => {
      const next = deferred<unknown>();
      pending.push(next);
      return next.promise;
    };
    const container = await renderExplorer();
    expect(state.accountCalls).toEqual([A]);

    await act(async () => changeDeployment(DEPLOYMENT_B));
    await settle();
    expect(state.accountCalls).toEqual([A, A]);

    await act(async () => pending[1].resolve(accountResult(A, 'Custom'))); await settle();
    expect(container.textContent).toContain(`Custom:${A}`);
    await act(async () => pending[0].resolve(accountResult(A, 'Devnet'))); await settle();
    expect(container.textContent).toContain(`Custom:${A}`);
    expect(container.textContent).not.toContain(`Devnet:${A}`);
  });

  it('keeps the newest record submission when promises resolve out of order', async () => {
    const pending: Array<Deferred<unknown>> = [];
    state.recordFactory = async () => {
      const next = deferred<unknown>();
      pending.push(next);
      return next.promise;
    };
    window.history.replaceState(null, '', `/explorer?view=record&q=${encodeURIComponent(A)}`);
    const container = await renderExplorer();
    const form = container.querySelector('form.xp-chain')!;

    await act(async () => {
      form.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));
      form.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(state.recordCalls).toEqual([A, A]);

    await act(async () => pending[1].resolve(recordResult('newest'))); await settle();
    expect(container.textContent).toContain('Finalized floor newest');
    await act(async () => pending[0].resolve(recordResult('oldest'))); await settle();
    expect(container.textContent).toContain('Finalized floor newest');
    expect(container.textContent).not.toContain('Finalized floor oldest');
  });

  it('hides a record result when its deployment or route identity changes', async () => {
    let reads = 0;
    state.recordFactory = async () => {
      reads += 1;
      return recordResult(reads < 3 ? 'old deployment' : 'new deployment');
    };
    window.history.replaceState(null, '', `/explorer?view=record&q=${encodeURIComponent(A)}`);
    const container = await renderExplorer();
    const form = container.querySelector('form.xp-chain')!;
    await act(async () => {
      form.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));
      await Promise.resolve();
      await Promise.resolve();
    });
    await settle();
    expect(container.textContent).toContain('Finalized floor old deployment');

    const tab = (label: string) => [...container.querySelectorAll<HTMLButtonElement>('button[role="tab"]')]
      .find((button) => button.textContent === label)!;
    await act(async () => tab('Account').click());
    await settle();
    await act(async () => tab('Record pair').click());
    await settle();
    expect(container.textContent).not.toContain('Finalized floor old deployment');

    await act(async () => {
      form.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));
      await Promise.resolve();
      await Promise.resolve();
    });
    await settle();
    expect(container.textContent).toContain('Finalized floor old deployment');

    await act(async () => changeDeployment(DEPLOYMENT_B));
    await settle();
    expect(container.textContent).not.toContain('Finalized floor old deployment');

    await act(async () => {
      form.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));
      await Promise.resolve();
      await Promise.resolve();
    });
    await settle();
    expect(container.textContent).toContain('Finalized floor new deployment');
  });
});
