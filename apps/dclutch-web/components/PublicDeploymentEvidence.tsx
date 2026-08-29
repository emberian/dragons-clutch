import {
  DEVNET_DEPLOYMENT_V1,
  DEVNET_PROGRAM_EVIDENCE_V1,
  PROTOCOL_ROLES_V1,
  type DeploymentV1,
} from '@/lib/deployments';
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
    programs: Object.freeze(Object.fromEntries(PROTOCOL_ROLES_V1.map((role) => [
      role,
      Object.freeze({
        program: DEVNET_DEPLOYMENT_V1.programs[role],
        programData: DEVNET_PROGRAM_EVIDENCE_V1[role].programData,
        observedDeploymentSlot: DEVNET_PROGRAM_EVIDENCE_V1[role].deploymentSlot,
      }),
    ]))),
    evidence: 'docs/evidence/DEPLOY_1.md §2',
    note: 'These are Solana devnet test programs. The addresses are permanent; the programs are mutable and have been upgraded in place since, keeping those addresses, so each observedDeploymentSlot is the slot of the ORIGINAL DEPLOY-1 deployment recorded in the evidence below and not the slot the program sits at today. Read the ProgramData account for the current one. The app reads current chain state before it describes an action as available.',
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
    return <p className="direct-status">You selected {deployment.label}. This selection has no checked public deployment record; its addresses came from your local or custom configuration.</p>;
  }
  return <div className="direct-actions" aria-label="Checked deployment evidence">
    <a
      className="secondary-action"
      href={docsHrefV1('evidence/DEPLOY_1.html', 'docs/evidence/DEPLOY_1.md')}
    >Read the checked deployment record →</a>
    <a
      className="secondary-action"
      download={PUBLIC_DEPLOYMENT_EVIDENCE_FILENAME_V1}
      href={publicDeploymentEvidenceDownloadHrefV1()}
    >Download the seven addresses and their first deployment slots ↓</a>
  </div>;
}
