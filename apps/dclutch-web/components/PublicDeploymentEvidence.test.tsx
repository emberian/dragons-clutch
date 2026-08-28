import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import PublicDeploymentEvidence, {
  PUBLIC_DEPLOYMENT_EVIDENCE_FILENAME_V1,
  publicDeploymentEvidenceDocumentV1,
  publicDeploymentEvidenceDownloadHrefV1,
} from './PublicDeploymentEvidence';
import {
  DEVNET_DEPLOYMENT_V1,
  DEVNET_PROGRAM_EVIDENCE_V1,
  LOCAL_DEPLOYMENT_V1,
  PROTOCOL_ROLES_V1,
} from '@/lib/deployments';

describe('public deployment evidence', () => {
  it('projects exactly the seven checked devnet Program and ProgramData coordinates', () => {
    const document = publicDeploymentEvidenceDocumentV1();
    const programs = document.programs as Record<string, Record<string, unknown>>;
    expect(Object.keys(programs)).toEqual(PROTOCOL_ROLES_V1);
    for (const role of PROTOCOL_ROLES_V1) {
      expect(programs[role]).toEqual({
        program: DEVNET_DEPLOYMENT_V1.programs[role],
        programData: DEVNET_PROGRAM_EVIDENCE_V1[role].programData,
        observedDeploymentSlot: DEVNET_PROGRAM_EVIDENCE_V1[role].deploymentSlot,
      });
    }
    expect(document.genesisHash).toBe(DEVNET_DEPLOYMENT_V1.genesisHash);
    expect(document.activationCache).toBe(DEVNET_DEPLOYMENT_V1.activationCache);
  });

  it('downloads the exact projection as one bounded JSON document', () => {
    const href = publicDeploymentEvidenceDownloadHrefV1();
    expect(href.startsWith('data:application/json;charset=utf-8,')).toBe(true);
    const text = decodeURIComponent(href.slice(href.indexOf(',') + 1));
    expect(JSON.parse(text)).toEqual(publicDeploymentEvidenceDocumentV1());
    expect(text.endsWith('\n')).toBe(true);
    expect(PUBLIC_DEPLOYMENT_EVIDENCE_FILENAME_V1).toBe(
      'dclutch-devnet-deployment-evidence-v1.json',
    );
  });

  it('links the checked record only for the checked devnet deployment', () => {
    const devnet = renderToStaticMarkup(
      <PublicDeploymentEvidence deployment={DEVNET_DEPLOYMENT_V1} />,
    );
    expect(devnet).toContain('Read the checked deployment record');
    expect(devnet).toContain('Download the seven addresses and observed slots');
    expect(devnet).toContain('download="dclutch-devnet-deployment-evidence-v1.json"');

    const local = renderToStaticMarkup(
      <PublicDeploymentEvidence deployment={LOCAL_DEPLOYMENT_V1} />,
    );
    expect(local).toContain('has no checked public deployment record');
    expect(local).not.toContain('download=');
  });
});
