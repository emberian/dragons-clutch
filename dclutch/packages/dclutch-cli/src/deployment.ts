/**
 * Which deployment this invocation means, and the proof that the endpoint is it.
 *
 * `@dclutch/sdk/deployments` is the semantic owner of dClutch's deployment
 * coordinates, and this module is only the CLI's spelling of them. No program
 * id, endpoint, or genesis hash is written down here: a second copy of an
 * address the SDK already owns would drift the first time a cohort redeploys,
 * and the copy would be the one a reader trusted.
 *
 * WHY THIS EXISTS. Without it the terminal client could not name a single
 * program on the substrate it ships against. `programId` resolved a role from
 * an explicit `--core-program` flag or from a session file that only a founding
 * run produces, so `dclutch-terminal markets ls` — the first command the trader guide
 * teaches — refused with "the core program id is not known" for anyone who had
 * not already founded a market themselves. The browser had the answer baked in
 * from this same SDK module the whole time.
 *
 * WHY IT IS A FLAG AND NOT A DEFAULT. `--cluster` is a user-visible
 * authorization, in the same family as `--i-mean-devnet`. The CLI still refuses
 * to guess which chain a bare endpoint is. And when the caller does name one,
 * the CLI owes them a check before it prints a sentence like "markets under
 * Core H… at finalized slot N": the endpoint must actually report that chain's
 * identity. That check is `SolanaRpcClient.assertMutationCluster`, which is the
 * tree's existing authority on admitting an endpoint as devnet or as an
 * explicitly addressed local validator, and which caches nothing.
 */
import {
  DEVNET_DEPLOYMENT_V1,
  LOCAL_DEPLOYMENT_V1,
  type DeploymentV1,
  type ProtocolRoleV1,
} from '@dclutch/sdk/deployments';
import type { MutationClusterAdmissionV1, SolanaRpcClient } from '@dclutch/sdk/rpc';

import type { ProgramRoleV1 } from './context';

/** The clusters `--cluster` names, in the order the usage text lists them. */
export const CLUSTER_DEPLOYMENTS_V1: ReadonlyArray<DeploymentV1> = Object.freeze([
  DEVNET_DEPLOYMENT_V1,
  LOCAL_DEPLOYMENT_V1,
]);

/**
 * The CLI calls the rent-credit role `rentCredit`; the SDK manifest calls it
 * `rent`. The two spellings meet here and nowhere else.
 */
const SDK_ROLE_BY_CLI_ROLE_V1: Readonly<Record<ProgramRoleV1, ProtocolRoleV1>> = Object.freeze({
  registry: 'registry',
  core: 'core',
  claims: 'claims',
  trading: 'trading',
  resolution: 'resolution',
  custody: 'custody',
  rentCredit: 'rent',
});

/**
 * Resolve `--cluster <name>` to one SDK deployment, or to null when absent.
 *
 * An unrecognized name is a refusal that lists what is recognized, never a
 * silent fall back to a different chain than the caller typed.
 */
export function resolveClusterDeploymentV1(value: unknown): DeploymentV1 | null {
  if (value === undefined) return null;
  if (typeof value !== 'string' || value.length === 0) {
    throw new Error(`pass --cluster ${CLUSTER_DEPLOYMENTS_V1.map((entry) => entry.cluster).join('|')}`);
  }
  const deployment = CLUSTER_DEPLOYMENTS_V1.find((entry) => entry.cluster === value);
  if (deployment === undefined) {
    throw new Error(
      `--cluster ${value} names no deployment this client ships: ${CLUSTER_DEPLOYMENTS_V1.map((entry) => entry.cluster).join(', ')}`
        + ' (an operator-run chain is named by --rpc plus the explicit --core-program/--claims-program/... flags, or by --session)',
    );
  }
  return deployment;
}

/** One role's program id in a named deployment, through the SDK manifest. */
export function deploymentProgramIdV1(deployment: DeploymentV1, role: ProgramRoleV1): string {
  return deployment.programs[SDK_ROLE_BY_CLI_ROLE_V1[role]];
}

/**
 * The sentence that says where a manifest-resolved program id came from.
 *
 * The manifest's `provenance` is already a full sentence with its own period,
 * so this trims one rather than printing `…§2).;`.
 */
export function deploymentProvenanceLineV1(deployment: DeploymentV1): string {
  const provenance = deployment.provenance.replace(/\.$/, '');
  return `program ids from the ${deployment.label} deployment manifest — ${provenance}`;
}

export type DeploymentIdentityClientV1 = Pick<SolanaRpcClient, 'assertMutationCluster'>;

/**
 * Prove the endpoint is the chain `--cluster` named, before any id from that
 * manifest is used to state a fact about it.
 *
 * Devnet must report devnet's exact genesis. A local deployment's genesis hash
 * is unpredictable by construction (a fresh ledger each campaign), so its
 * admission is the SDK's: an explicitly addressed, credential-free HTTP
 * loopback origin, with the known public chains refused even through loopback.
 * Neither admission is cached; an endpoint substituted after this call is
 * caught by the next boundary that reacquires it.
 */
export async function assertDeploymentIdentityV1(
  client: DeploymentIdentityClientV1,
  deployment: DeploymentV1,
  boundary: string,
): Promise<MutationClusterAdmissionV1> {
  let admission: MutationClusterAdmissionV1;
  try {
    admission = await client.assertMutationCluster();
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`--cluster ${deployment.cluster} refused at ${boundary}: ${reason}`);
  }
  const expectedKind = deployment.genesisHash === null ? 'loopback-local-validator' : 'devnet';
  if (admission.kind !== expectedKind) {
    throw new Error(
      `--cluster ${deployment.cluster} refused at ${boundary}: the endpoint is admitted as ${admission.kind},`
        + ` not as the ${deployment.label} deployment this invocation named`,
    );
  }
  if (deployment.genesisHash !== null && admission.genesisHash !== deployment.genesisHash) {
    throw new Error(
      `--cluster ${deployment.cluster} refused at ${boundary}: the endpoint reports genesis ${admission.genesisHash},`
        + ` not the ${deployment.label} deployment's ${deployment.genesisHash}`,
    );
  }
  return admission;
}
