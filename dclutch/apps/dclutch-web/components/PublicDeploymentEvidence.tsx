import {
  DEVNET_DEPLOYMENT_V1,
  DEVNET_PROGRAM_EVIDENCE_V1,
  deployedProgramRolesV1,
  type DeploymentV1,
} from '@dclutch/sdk/deployments';
import { docsHrefV1 } from '@/lib/flags';

export const PUBLIC_DEPLOYMENT_EVIDENCE_FILENAME_V1 =
  'dclutch-devnet-deployment-evidence-v1.json';

/**
 * One portable, reader-facing projection of the checked public deployment.
 * Every address comes from deployments.ts, the app's existing semantic owner;
 * this component states no parallel table.
 */
export function publicDeploymentEvidenceDocumentV1(): Readonly<Record<string, unknown>> {
  return Object.freeze({
    schema: 'dclutch-public-deployment-evidence-v1',
    network: 'solana-devnet',
    genesisHash: DEVNET_DEPLOYMENT_V1.genesisHash,
    endpoint: DEVNET_DEPLOYMENT_V1.endpoint,
    activationCache: DEVNET_DEPLOYMENT_V1.activationCache,
    programs: Object.freeze(Object.fromEntries(deployedProgramRolesV1(DEVNET_DEPLOYMENT_V1).map((role) => [
      role,
      Object.freeze({
        program: DEVNET_DEPLOYMENT_V1.programs[role],
        programData: DEVNET_PROGRAM_EVIDENCE_V1[role].programData,
        firstDeploymentSlot: DEVNET_PROGRAM_EVIDENCE_V1[role].deploymentSlot,
      }),
    ]))),
    evidence: 'docs/evidence/COHORT16_DEPLOYED_SEALED_2026_09_05.md §2',
    note: 'These are Solana devnet test programs, and they are NOT permanent. Devnet is disposable here by ruling: each cohort is a full redeploy at fresh addresses, and the cohort before it is closed, which returns its rent to pay for the next. This page named DEPLOY-1 — cohort-8 — for a day after cohort-8 was closed and all seven of its ProgramData accounts had been deleted, while its Program stubs stayed executable and kept naming them. There are eight programs now: the seven roles every account derivation and owner check names, and the accelerator, which owns no account and which the General, Dealer and Series routes call into. deploymentSlot is where each program was read to sit, not a historical first deployment. This document is a static projection and cannot observe a chain. For the current slot, read the ProgramData account — which the /operate deployment inspector now does live, reporting each role that has been upgraded since this app was built.',
  });
}

export function publicDeploymentEvidenceDownloadHrefV1(): string {
  const text = `${JSON.stringify(publicDeploymentEvidenceDocumentV1(), null, 2)}\n`;
  return `data:application/json;charset=utf-8,${encodeURIComponent(text)}`;
}

export default function PublicDeploymentEvidence({
  deployment,
}: Readonly<{ deployment: DeploymentV1 }>) {
  if (deployment.cluster !== 'devnet') {
    return <p className="direct-status">You selected {deployment.label}. Addresses came from your own configuration.</p>;
  }
  return <div className="direct-actions" aria-label="Checked deployment evidence">
    <a
      className="secondary-action"
      href={docsHrefV1('evidence/COHORT16_DEPLOYED_SEALED_2026_09_05.html', 'docs/evidence/COHORT16_DEPLOYED_SEALED_2026_09_05.md')}
    >Read the checked deployment record →</a>
    <a
      className="secondary-action"
      download={PUBLIC_DEPLOYMENT_EVIDENCE_FILENAME_V1}
      href={publicDeploymentEvidenceDownloadHrefV1()}
    >Download the eight addresses and the slots they were read at ↓</a>
  </div>;
}
