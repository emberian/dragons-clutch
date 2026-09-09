import {
  DEVNET_DEPLOYMENT_V1,
  DEVNET_PROGRAM_EVIDENCE_V1,
  DEVNET_RELEASE_EVIDENCE_V1,
  deployedProgramRolesV1,
  isPublishedDevnetDeploymentV1,
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
    cohort: DEVNET_RELEASE_EVIDENCE_V1.cohort,
    sourceCommit: DEVNET_RELEASE_EVIDENCE_V1.sourceCommit,
    releaseGateSha256: DEVNET_RELEASE_EVIDENCE_V1.releaseGateSha256,
    evidence: DEVNET_RELEASE_EVIDENCE_V1.evidencePath,
    note: 'Solana devnet deployment record. Open the deployment inspector for current program versions.',
  });
}

export function publicDeploymentEvidenceDownloadHrefV1(): string {
  const text = `${JSON.stringify(publicDeploymentEvidenceDocumentV1(), null, 2)}\n`;
  return `data:application/json;charset=utf-8,${encodeURIComponent(text)}`;
}

export default function PublicDeploymentEvidence({
  deployment,
}: Readonly<{ deployment: DeploymentV1 }>) {
  if (!isPublishedDevnetDeploymentV1(deployment)) {
    return <p className="direct-status">Selected deployment: {deployment.label}. Deployment record unavailable.</p>;
  }
  return <div className="direct-actions" aria-label="Deployment details">
    <a
      className="secondary-action"
      href={docsHrefV1(DEVNET_RELEASE_EVIDENCE_V1.evidencePath.replace(/^docs\//, '').replace(/\.md$/, '.html'), DEVNET_RELEASE_EVIDENCE_V1.evidencePath)}
    >View deployment record →</a>
    <a
      className="secondary-action"
      download={PUBLIC_DEPLOYMENT_EVIDENCE_FILENAME_V1}
      href={publicDeploymentEvidenceDownloadHrefV1()}
    >Download program addresses ↓</a>
  </div>;
}
