import { DEVNET_DEPLOYMENT_V1, type DeploymentV1 } from '@dclutch/sdk/deployments';
import { inspectMarketDetailV1, type MarketDetailV1 } from '@dclutch/sdk/marketDetail';
import { type ConnectionFacts, type SignatureRecordObservation, type SolanaRpcClient, SOLANA_DEVNET_GENESIS_HASH_V1 } from '@dclutch/sdk/rpc';

import { publicMarketBindingsCohortV1 } from '@/lib/publicMarketBindings';
import { type AquariumStatusV1 } from '@/lib/aquariumStatus';

/** The panel reads one inventory market and this many recent finalized index rows. */
export const AQUARIUM_LIVE_SIGNATURE_LIMIT_V1 = 8;

export type AquariumLiveSelectionV1 =
  | Readonly<{ kind: 'unavailable'; reason: string }>
  | Readonly<{ kind: 'selected'; address: string; cohortNumber: number; manifestSha256: string; checkedReleaseSetIds: ReadonlyArray<string> }>;

export type AquariumLiveObservationV1 = Readonly<{
  address: string;
  detail: MarketDetailV1;
  facts: ConnectionFacts;
  signatures: ReadonlyArray<SignatureRecordObservation>;
  observedAt: string;
}>;

export type AquariumLiveMarketReaderV1 = (
  client: Pick<SolanaRpcClient, 'finalizedSlot' | 'multipleAccounts'>,
  request: Readonly<{ coreProgramId: string; registryProgramId: string; claimsProgramId: string; custodyProgramId: string; address: string }>,
) => Promise<MarketDetailV1>;

type AquariumLiveRpcV1 = Pick<SolanaRpcClient, 'probe' | 'signaturesForAddress' | 'finalizedSlot' | 'multipleAccounts'>;

function aborted(signal: AbortSignal | undefined): void {
  if (signal?.aborted) throw new Error('live chain observation cancelled');
}

/**
 * Select one address from the bounded published inventory only after its
 * cohort manifest agrees with the checked public inventory. The active list
 * is parsed by aquariumStatus, which caps it at 32 before this sees it.
 */
export function selectAquariumLiveMarketV1(
  status: AquariumStatusV1 | null,
  checkedReleaseSetIds: ReadonlyArray<string> | null,
): AquariumLiveSelectionV1 {
  if (status === null) return Object.freeze({ kind: 'unavailable', reason: 'Waiting for a published aquarium snapshot.' });
  const cohort = publicMarketBindingsCohortV1();
  if (cohort.manifestSha256 === null) {
    return Object.freeze({ kind: 'unavailable', reason: `The checked cohort-${cohort.number} inventory has not been published yet.` });
  }
  if (cohort.number !== status.cohort.number || cohort.manifestSha256 !== status.cohort.manifestSha256) {
    return Object.freeze({ kind: 'unavailable', reason: 'The snapshot and checked cohort inventory do not describe the same deployment.' });
  }
  if (checkedReleaseSetIds === null || checkedReleaseSetIds.length === 0) {
    return Object.freeze({ kind: 'unavailable', reason: 'This cohort has no checked release set published for a live market read.' });
  }
  const market = status.activity.activeMarkets.find((entry) => entry.address !== null);
  if (market?.address === undefined || market.address === null) {
    return Object.freeze({ kind: 'unavailable', reason: 'This snapshot names no materialized market address to read.' });
  }
  return Object.freeze({
    kind: 'selected', address: market.address, cohortNumber: cohort.number, manifestSha256: cohort.manifestSha256,
    checkedReleaseSetIds: Object.freeze([...checkedReleaseSetIds]),
  });
}

/** One finalized market detail read plus the node's bounded recent signature index. */
export async function observeAquariumLiveMarketV1(
  client: AquariumLiveRpcV1,
  selection: Extract<AquariumLiveSelectionV1, Readonly<{ kind: 'selected' }>>,
  options: Readonly<{
    signal?: AbortSignal;
    deployment?: DeploymentV1;
    now?: () => Date;
    readMarket?: AquariumLiveMarketReaderV1;
  }> = {},
): Promise<AquariumLiveObservationV1> {
  const deployment = options.deployment ?? DEVNET_DEPLOYMENT_V1;
  const readMarket = options.readMarket ?? inspectMarketDetailV1;
  aborted(options.signal);
  const facts = await client.probe();
  aborted(options.signal);
  if (facts.genesisHash !== SOLANA_DEVNET_GENESIS_HASH_V1 || facts.genesisHash !== deployment.genesisHash) {
    throw new Error('the configured public endpoint did not identify itself as Solana devnet');
  }
  const detail = await readMarket(client, {
    coreProgramId: deployment.programs.core,
    registryProgramId: deployment.programs.registry,
    claimsProgramId: deployment.programs.claims,
    custodyProgramId: deployment.programs.custody,
    address: selection.address,
  });
  aborted(options.signal);
  if (detail.card.status !== 'decoded') throw new Error(`the selected Market did not decode: ${detail.reason}`);
  if (!selection.checkedReleaseSetIds.includes(detail.card.identity.selectedReleaseSetId)) {
    throw new Error('the selected Market does not name a checked release set for this cohort');
  }
  const signatures = await client.signaturesForAddress(selection.address, AQUARIUM_LIVE_SIGNATURE_LIMIT_V1);
  aborted(options.signal);
  return Object.freeze({
    address: selection.address,
    detail,
    facts,
    signatures: Object.freeze([...signatures].slice(0, AQUARIUM_LIVE_SIGNATURE_LIMIT_V1)),
    observedAt: (options.now ?? (() => new Date()))().toISOString(),
  });
}
