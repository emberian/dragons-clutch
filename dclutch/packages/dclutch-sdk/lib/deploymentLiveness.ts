import { PublicKey } from '@solana/web3.js';

import { checkedReleaseSetIdsV1, PUBLIC_DEVNET_CUT_V1 } from './publicCutStaging';
import {
  DEVNET_DEPLOYMENT_V1,
  DEVNET_PROGRAM_EVIDENCE_V1,
  deployedProgramRolesV1,
  type DeployedProgramRoleV1,
  type DeploymentV1,
  type ProgramEvidenceV1,
} from './deployments';
import { decodeMarketCoreStateV2 } from './marketCoreV2';
import type { SolanaRpcClient } from './rpc';

/**
 * IS THE COHORT THIS SITE SHIPS STILL ALIVE, AND IS IT THE ONE IT TALKS ABOUT?
 *
 * Two questions, asked of the chain, because this browser has now published a
 * CLOSED cohort twice and nothing went red either time.
 *
 *   * 2026-09-02, `0f1d75b27`: the manifest named cohort-8 for a day after
 *     cohort-8 was closed. Every gate passed. `deployments.live.test.ts` checked
 *     that each Program account exists, is executable, is loader-owned, is 36
 *     bytes and names its recorded ProgramData -- all of which is true of a
 *     closed program -- and none of them asked for the account that holds
 *     the code.
 *   * 2026-09-04, the second C-16 walk: the manifest named cohort-14 the
 *     morning after cohort-15 landed and cohort-14 was closed in the same lane.
 *     All seven ProgramData accounts read `AccountNotFound`. The commit that
 *     fixed this exact defect one cohort earlier was still in the tree.
 *
 * The repair the first time was to teach ONE env-gated test to ask. That test
 * is skipped by default and runs in no tier, so what it bought was a check a
 * reader could run and nobody did. This module is the same question asked as a
 * gate: `scripts/deployment-liveness.mjs` runs it in `tools/ci/run.sh`'s `web`
 * tier, where a closed cohort is red rather than undiscovered.
 *
 * ## Why the second question is here too
 *
 * A live cohort is not enough. The site also PUBLISHES a featured market and a
 * table of checked execution releases, and those are the sentences a reader
 * actually acts on. `stageCheckedReleaseV1` proves at staging time that the row
 * is about the set the Market selects -- once, from an argument a human passed.
 * Nothing re-asked afterwards, so a cut that survived a cohort boundary would
 * keep saying a release was checked for a market that no longer selects it, or
 * for a market whose Core program no longer exists.
 *
 * So: the featured Market must be owned by THIS deployment's Core program, and
 * the release set it carries in its own bytes must be one the cut says was
 * checked. Both halves are read; neither is asserted from a document.
 *
 * ## Eight programs since cohort-16
 *
 * The reading asks whatever `deployedProgramRolesV1` says the deployment has,
 * which is the seven checked roles plus the accelerator wherever one was
 * deployed. That eighth is not a role any derivation here consults -- it owns
 * no account -- but General batches, the Dealer's first market and the whole
 * Series family CPI into it, so a cohort whose accelerator has been closed is a
 * cohort three route families cannot run, and this gate would have called it
 * alive.
 */

/** The upgradeable loader, which owns every Program and ProgramData account. */
export const UPGRADEABLE_LOADER_V1 = 'BPFLoaderUpgradeab1e11111111111111111111111';
/** Loader-v3 account state tags. 2 is the Program stub; 3 is the ProgramData. */
export const LOADER_STATE_PROGRAM_V1 = 2;
export const LOADER_STATE_PROGRAM_DATA_V1 = 3;
/** Tag, deployment slot, and the upgrade-authority option -- no ELF body. */
export const PROGRAM_DATA_HEADER_BYTES_V1 = 45;

export type ProgramLivenessRowV1 = Readonly<{
  role: DeployedProgramRoleV1;
  programId: string;
  /** What the Program stub names at offset 4, READ rather than derived. */
  programData: string;
  /** The u64 at offset 4 of the ProgramData header, or null when it is vacant. */
  deploymentSlot: string | null;
  /** True when the account that actually holds the code answered. */
  live: boolean;
}>;

export type DeploymentLivenessV1 =
  | Readonly<{
    status: 'alive';
    observedSlot: string;
    roles: ReadonlyArray<ProgramLivenessRowV1>;
    /** The featured Market's own bytes at offset 208. */
    market: string;
    marketReleaseSetId: string;
    marketPhase: string;
  }>
  | Readonly<{
    status: 'closed';
    observedSlot: string;
    roles: ReadonlyArray<ProgramLivenessRowV1>;
    /** Every program whose ProgramData is vacant, named. */
    closedRoles: ReadonlyArray<DeployedProgramRoleV1>;
    reason: string;
  }>
  | Readonly<{ status: 'refused'; reason: string }>;

/**
 * The whole gate, as one reading.
 *
 * `closed` and `refused` are deliberately distinct verdicts and neither is a
 * throw. A closed cohort is a FINDING about the deployment and must be reported
 * with the roles named; a market that will not decode, or an evidence row that
 * does not match what the stub says, is a refusal to state anything. A caller
 * that collapsed them would lose the sentence a reader needs.
 */
export async function readDeploymentLivenessV1(
  client: Pick<SolanaRpcClient, 'multipleAccounts' | 'multipleAccountDataSlices' | 'accountInfo'>,
  request: Readonly<{
    deployment: DeploymentV1;
    /**
     * The manifest's evidence rows. PARTIAL by type, because a deployment names
     * the programs it has and this table has to be able to say it carries no
     * row for one of them -- which is a refusal, not a silent skip.
     */
    evidence: Readonly<Partial<Record<DeployedProgramRoleV1, ProgramEvidenceV1>>>;
    /** The featured Market, and the release sets the cut says were checked. */
    market: string | null;
    checkedReleaseSetIds: ReadonlyArray<string> | null;
  }>,
): Promise<DeploymentLivenessV1> {
  const refuse = (reason: string): DeploymentLivenessV1 => Object.freeze({ status: 'refused' as const, reason });
  const { deployment, evidence } = request;
  // SEVEN OR EIGHT, ASKED OF THE DEPLOYMENT ITSELF. Cohort-16 put an
  // accelerator on a chain for the first time and three route families depend
  // on it, so a reading that stopped at the seven would call a cohort alive
  // with the program its General and Series routes need already closed.
  const roles = deployedProgramRolesV1(deployment);
  const programs = roles.map((role) => {
    const address = deployment.programs[role];
    if (address === undefined) throw new Error(`the deployment names no ${role} program`);
    return address;
  });

  let stubs;
  try {
    stubs = await client.multipleAccounts(programs);
  } catch (error) {
    return refuse(`The ${roles.length} Program accounts did not read: ${error instanceof Error ? error.message : String(error)}`);
  }
  const observedSlot = stubs.slot;
  const named: string[] = [];
  for (const [index, role] of roles.entries()) {
    const account = stubs.accounts[index].account;
    if (account === null) return refuse(`The ${role} Program account ${programs[index]} does not exist on ${deployment.endpoint}.`);
    if (account.owner !== UPGRADEABLE_LOADER_V1) return refuse(`The ${role} Program account is owned by ${account.owner}, not the upgradeable loader.`);
    if (!account.executable) return refuse(`The ${role} Program account is not executable.`);
    if (account.data.length !== 36) return refuse(`The ${role} Program account is ${account.data.length} bytes, not the 36 of a Loader-v3 Program.`);
    const tag = new DataView(account.data.buffer, account.data.byteOffset, account.data.byteLength).getUint32(0, true);
    if (tag !== LOADER_STATE_PROGRAM_V1) return refuse(`The ${role} Program account's Loader state tag is ${tag}, not ${LOADER_STATE_PROGRAM_V1}.`);
    const programData = new PublicKey(account.data.slice(4)).toBase58();
    const row = evidence[role];
    if (row === undefined) return refuse(`The manifest carries no ProgramData evidence row for the ${role} program, so nothing here can be checked against it.`);
    // The manifest's evidence row is a SECOND statement about the same fact, so
    // a disagreement is a refusal rather than something to paper over with the
    // chain's answer: one of the two is a row from another cohort.
    if (programData !== row.programData) {
      return refuse(`The ${role} Program account names ProgramData ${programData} and the manifest records ${row.programData}.`);
    }
    named.push(programData);
  }

  let headers;
  try {
    headers = await client.multipleAccountDataSlices(named, 0, PROGRAM_DATA_HEADER_BYTES_V1);
  } catch (error) {
    return refuse(`The ${roles.length} ProgramData headers did not read: ${error instanceof Error ? error.message : String(error)}`);
  }
  const rows: ProgramLivenessRowV1[] = [];
  const closedRoles: DeployedProgramRoleV1[] = [];
  for (const [index, role] of roles.entries()) {
    const account = headers.accounts[index].account;
    // THE ONE QUESTION A CLOSED COHORT ANSWERS DIFFERENTLY. `solana program
    // close` deletes this account and leaves everything checked above intact.
    if (account === null) {
      closedRoles.push(role);
      rows.push(Object.freeze({ role, programId: programs[index], programData: named[index], deploymentSlot: null, live: false }));
      continue;
    }
    if (account.owner !== UPGRADEABLE_LOADER_V1) return refuse(`The ${role} ProgramData account is owned by ${account.owner}, not the upgradeable loader.`);
    if (account.executable) return refuse(`The ${role} ProgramData account is executable.`);
    const view = new DataView(account.data.buffer, account.data.byteOffset, account.data.byteLength);
    const tag = view.getUint32(0, true);
    if (tag !== LOADER_STATE_PROGRAM_DATA_V1) return refuse(`The ${role} ProgramData Loader state tag is ${tag}, not ${LOADER_STATE_PROGRAM_DATA_V1}.`);
    if (account.space <= PROGRAM_DATA_HEADER_BYTES_V1) return refuse(`The ${role} ProgramData account is ${account.space} bytes and carries no ELF.`);
    rows.push(Object.freeze({
      role,
      programId: programs[index],
      programData: named[index],
      deploymentSlot: view.getBigUint64(4, true).toString(),
      live: true,
    }));
  }
  if (closedRoles.length > 0) {
    return Object.freeze({
      status: 'closed' as const,
      observedSlot,
      roles: Object.freeze(rows),
      closedRoles: Object.freeze(closedRoles),
      reason: `This deployment is CLOSED: ${closedRoles.join(', ')} ${closedRoles.length === 1 ? 'has' : 'have'} a vacant ProgramData account while ${closedRoles.length === 1 ? 'its' : 'their'} Program stub is still alive, executable and still naming it. The site is publishing dead addresses.`,
    });
  }

  // --- and then the market the site actually points a reader at. ---
  if (request.market === null) return refuse('The public cut names no featured market, so nothing joins these programs to a published sentence.');
  let observation;
  try {
    observation = await client.accountInfo(request.market, observedSlot);
  } catch (error) {
    return refuse(`The featured market did not read: ${error instanceof Error ? error.message : String(error)}`);
  }
  if (observation.account === null) return refuse(`The featured market ${request.market} does not exist at finalized slot ${observedSlot}.`);
  if (observation.account.owner !== deployment.programs.core) {
    return refuse(`The featured market ${request.market} is owned by ${observation.account.owner}, and this deployment's Core program is ${deployment.programs.core}. The cut features a market from another cohort.`);
  }
  let decoded;
  try {
    decoded = decodeMarketCoreStateV2(request.market, observation.account.data);
  } catch (error) {
    return refuse(`The featured market did not decode: ${error instanceof Error ? error.message : String(error)}`);
  }
  const marketReleaseSetId = decoded.identity.selectedReleaseSetId;
  if (request.checkedReleaseSetIds === null) return refuse('The public cut carries no checked-release rows, so nothing states which release this market was checked against.');
  if (!request.checkedReleaseSetIds.includes(marketReleaseSetId)) {
    return refuse(`The featured market selects execution release set ${marketReleaseSetId} and the cut carries checked rows for ${request.checkedReleaseSetIds.join(', ')}. The published table is about a release this market does not select.`);
  }
  return Object.freeze({
    status: 'alive' as const,
    observedSlot,
    roles: Object.freeze(rows),
    market: request.market,
    marketReleaseSetId,
    marketPhase: decoded.phase,
  });
}

/** The shipped manifest and the shipped cut, which is what a visitor gets. */
export async function readShippedDeploymentLivenessV1(
  client: Pick<SolanaRpcClient, 'multipleAccounts' | 'multipleAccountDataSlices' | 'accountInfo'>,
): Promise<DeploymentLivenessV1> {
  return readDeploymentLivenessV1(client, {
    deployment: DEVNET_DEPLOYMENT_V1,
    evidence: DEVNET_PROGRAM_EVIDENCE_V1,
    market: PUBLIC_DEVNET_CUT_V1.market,
    checkedReleaseSetIds: checkedReleaseSetIdsV1(PUBLIC_DEVNET_CUT_V1),
  });
}

/** One line per role plus the verdict, for a terminal that has to read it. */
export function describeDeploymentLivenessV1(liveness: DeploymentLivenessV1): string {
  if (liveness.status === 'refused') return `REFUSED  ${liveness.reason}`;
  const rows = liveness.roles.map((row) => `  ${row.role.padEnd(11)} ${row.programId}  ${row.live ? `slot ${row.deploymentSlot}` : 'ProgramData VACANT'}`);
  if (liveness.status === 'closed') return [`CLOSED   ${liveness.reason}`, ...rows].join('\n');
  return [
    `ALIVE    ${liveness.roles.length} programs, read finalized at slot ${liveness.observedSlot}`,
    ...rows,
    `  market      ${liveness.market}  ${liveness.marketPhase}`,
    `  release set ${liveness.marketReleaseSetId}  (checked, per the public cut)`,
  ].join('\n');
}
