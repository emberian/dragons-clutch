import { PublicKey } from '@solana/web3.js';

/**
 * A cold reader's complete public journey, expressed without a browser driver.
 *
 * The live devnet campaign injects observations through `ColdClientAdapterV1`;
 * this contract owns their order and acceptance rules. It deliberately has no
 * wallet-signing or submission callback. A test can therefore exercise every
 * supported read/preview/redeem-preflight state from empty client storage, and
 * a later live run can supply real addresses and transaction observations
 * without creating a second journey implementation.
 */

export const COLD_CLIENT_CHAIN_STEPS_V1 = Object.freeze([
  'market.discover',
  'market.inspect',
  'participant.inspect',
  'direct.inspect',
  'direct.preview-unsigned',
  'resolution.inspect',
  'redeem.inspect',
  'redeem.prepare-unsigned',
] as const);

export type ColdClientChainStepV1 = typeof COLD_CLIENT_CHAIN_STEPS_V1[number];

export type ColdClientEvidenceV1 = Readonly<{
  deploymentKey: string;
  /** Select this chain-discovered Market; omit only when discovery must find exactly one. */
  marketAddress?: string;
  /** Public identity only. The journey never accepts a signing capability. */
  walletAddress?: string;
  /** Portable signed counterparty input. Its signature remains chain-untrusted here. */
  directTicket?: string;
  /** Rust-authored payout plan input for the browser's existing redeem preflight. */
  redeemPlan?: string;
  /** Optional finalized activity evidence captured by the later devnet run. */
  transactionIds?: ReadonlyArray<string>;
}>;

export type ColdClientDeploymentV1 = Readonly<{
  cluster: 'devnet' | 'localnet';
  endpoint: string;
  releaseSetId: string;
  programs: Readonly<{
    registry: string;
    core: string;
    trading: string;
    claims: string;
    custody: string;
    resolution: string;
    rent: string;
  }>;
}>;

export type ColdClientTruthV1 = Readonly<{
  subject: string;
  verdict: 'authenticated' | 'refused';
  detail: string;
}>;

export type ColdClientStepResultV1 = Readonly<{
  step: ColdClientChainStepV1;
  status: 'ready' | 'incomplete' | 'refused' | 'unavailable';
  reason: string;
  /** Required for every ready chain read or derived unsigned artifact. */
  observedSlot?: string;
  /** Addresses reacquired or derived by this step, never caller labels. */
  addresses?: ReadonlyArray<string>;
  truths?: ReadonlyArray<ColdClientTruthV1>;
  artifact?: Readonly<{
    kind: 'unsigned-preview' | 'unsigned-transaction';
    digest: string;
    byteLength?: number;
  }>;
}>;

export type ColdClientContextV1 = Readonly<{
  evidence: ColdClientEvidenceV1;
  deployment: ColdClientDeploymentV1;
  selectedMarket: string | null;
  prior: ReadonlyArray<ColdClientStepResultV1>;
}>;

export type ColdClientAdapterV1 = Readonly<{
  coldState(): Promise<Readonly<{
    localStorageKeys: ReadonlyArray<string>;
    sessionStorageKeys: ReadonlyArray<string>;
    cacheKeys: ReadonlyArray<string>;
  }>>;
  loadBakedDeployment(deploymentKey: string): Promise<ColdClientDeploymentV1>;
  runStep(step: ColdClientChainStepV1, context: ColdClientContextV1): Promise<ColdClientStepResultV1>;
}>;

export type ColdClientJourneyReportV1 = Readonly<{
  schema: 'dclutch/cold-client-journey/v1';
  deployment: ColdClientDeploymentV1;
  selectedMarket: string;
  steps: ReadonlyArray<ColdClientStepResultV1>;
  injectedTransactionIds: ReadonlyArray<string>;
  signingRequested: false;
  submissionRequested: false;
}>;

function canonicalAddress(value: string, field: string): string {
  const parsed = new PublicKey(value).toBase58();
  if (parsed !== value) throw new Error(`${field} must be canonical base58 text`);
  return parsed;
}

function canonicalSlot(value: string | undefined, field: string): bigint {
  if (value === undefined || !/^(0|[1-9][0-9]*)$/.test(value)) throw new Error(`${field} must carry one finalized slot`);
  return BigInt(value);
}

function validateDeployment(deployment: ColdClientDeploymentV1): void {
  if (deployment.cluster !== 'devnet' && deployment.cluster !== 'localnet') throw new Error('cold-client deployment is not a development cluster');
  const endpoint = new URL(deployment.endpoint);
  if (endpoint.protocol !== 'https:' && endpoint.protocol !== 'http:') throw new Error('cold-client deployment endpoint is not HTTP(S)');
  if (!/^[0-9a-f]{64}$/.test(deployment.releaseSetId)) throw new Error('cold-client release set is not one lowercase-hex identity');
  const addresses = Object.entries(deployment.programs).map(([role, address]) => canonicalAddress(address, `${role} program`));
  if (new Set(addresses).size !== addresses.length) throw new Error('cold-client deployment aliases two program roles');
}

function validateResult(result: ColdClientStepResultV1, step: ColdClientChainStepV1): void {
  if (result.step !== step) throw new Error(`cold-client adapter returned ${result.step} while ${step} was requested`);
  if (result.reason.trim() === '') throw new Error(`${step} omitted its reader-facing reason`);
  for (const address of result.addresses ?? []) canonicalAddress(address, `${step} observed address`);
  for (const truth of result.truths ?? []) {
    if (truth.subject.trim() === '' || truth.detail.trim() === '') throw new Error(`${step} emitted an unnamed capability truth`);
  }
  if (result.status === 'ready') {
    canonicalSlot(result.observedSlot, step);
    if ((result.truths ?? []).length === 0) throw new Error(`${step} is ready without one authenticated or refused chain truth`);
  }
  if (result.artifact !== undefined) {
    if (step !== 'direct.preview-unsigned' && step !== 'redeem.prepare-unsigned') throw new Error(`${step} returned an artifact outside an unsigned builder step`);
    if (result.status !== 'ready') throw new Error(`${step} returned an artifact while ${result.status}`);
    if (!/^[0-9a-f]{64}$/.test(result.artifact.digest)) throw new Error(`${step} artifact digest is not lowercase SHA-256 text`);
    if (result.artifact.byteLength !== undefined
        && (!Number.isSafeInteger(result.artifact.byteLength) || result.artifact.byteLength <= 0)) {
      throw new Error(`${step} artifact byte length is invalid`);
    }
  }
}

function ready(prior: ReadonlyArray<ColdClientStepResultV1>, step: ColdClientChainStepV1): boolean {
  return prior.find((result) => result.step === step)?.status === 'ready';
}

function enforceDependencies(
  result: ColdClientStepResultV1,
  prior: ReadonlyArray<ColdClientStepResultV1>,
  evidence: ColdClientEvidenceV1,
): void {
  if (result.status !== 'ready') return;
  switch (result.step) {
    case 'market.inspect':
      if (!ready(prior, 'market.discover')) throw new Error('Market inspection became ready after discovery refused');
      break;
    case 'participant.inspect':
    case 'direct.inspect':
    case 'resolution.inspect':
    case 'redeem.inspect':
      if (!ready(prior, 'market.inspect')) throw new Error(`${result.step} became ready without an authenticated Market`);
      break;
    case 'direct.preview-unsigned':
      if (!ready(prior, 'participant.inspect') || !ready(prior, 'direct.inspect')
          || evidence.walletAddress === undefined || evidence.directTicket === undefined) {
        throw new Error('Direct preview became ready without participant state, Direct state, wallet identity, and a ticket');
      }
      if (result.artifact?.kind !== 'unsigned-preview') throw new Error('Direct preview did not return an unsigned preview artifact');
      break;
    case 'redeem.prepare-unsigned':
      if (!ready(prior, 'redeem.inspect') || evidence.walletAddress === undefined || evidence.redeemPlan === undefined) {
        throw new Error('redeem preparation became ready without redeem state, wallet identity, and a payout plan');
      }
      if (result.artifact?.kind !== 'unsigned-transaction') throw new Error('redeem preparation did not return an unsigned transaction artifact');
      break;
    case 'market.discover':
      break;
  }
}

/** Run the complete unsigned public journey from a demonstrably empty client. */
export async function runColdClientJourneyV1(
  adapter: ColdClientAdapterV1,
  evidence: ColdClientEvidenceV1,
): Promise<ColdClientJourneyReportV1> {
  if (evidence.deploymentKey.trim() === '') throw new Error('cold-client deployment key is empty');
  if (evidence.walletAddress !== undefined) canonicalAddress(evidence.walletAddress, 'cold-client wallet');
  if (evidence.marketAddress !== undefined) canonicalAddress(evidence.marketAddress, 'cold-client selected Market');
  const state = await adapter.coldState();
  const residue = [...state.localStorageKeys, ...state.sessionStorageKeys, ...state.cacheKeys];
  if (residue.length > 0) throw new Error(`cold-client state is not empty: ${residue.join(', ')}`);

  const deployment = await adapter.loadBakedDeployment(evidence.deploymentKey);
  validateDeployment(deployment);
  const results: ColdClientStepResultV1[] = [];
  let selectedMarket = evidence.marketAddress ?? null;
  let lastReadySlot = -1n;
  for (const step of COLD_CLIENT_CHAIN_STEPS_V1) {
    const result = await adapter.runStep(step, Object.freeze({ evidence, deployment, selectedMarket, prior: Object.freeze([...results]) }));
    validateResult(result, step);
    enforceDependencies(result, results, evidence);
    if (result.status === 'ready') {
      const slot = canonicalSlot(result.observedSlot, step);
      if (slot < lastReadySlot) throw new Error(`${step} regressed below the prior finalized observation`);
      lastReadySlot = slot;
    }
    if (step === 'market.discover' && result.status === 'ready') {
      const discovered = result.addresses ?? [];
      if (selectedMarket === null) {
        if (discovered.length !== 1) throw new Error('cold discovery needs an injected Market or exactly one authenticated result');
        selectedMarket = discovered[0] ?? null;
      } else if (!discovered.includes(selectedMarket)) {
        throw new Error('injected Market was not present in cold discovery');
      }
    }
    results.push(Object.freeze(result));
  }
  if (selectedMarket === null) throw new Error('cold-client journey ended without a selected Market');
  return Object.freeze({
    schema: 'dclutch/cold-client-journey/v1',
    deployment,
    selectedMarket,
    steps: Object.freeze(results),
    injectedTransactionIds: Object.freeze([...(evidence.transactionIds ?? [])]),
    signingRequested: false,
    submissionRequested: false,
  });
}
