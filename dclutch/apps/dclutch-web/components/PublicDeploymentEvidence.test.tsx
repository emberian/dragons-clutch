import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import PublicDeploymentEvidence, {
  PUBLIC_DEPLOYMENT_EVIDENCE_FILENAME_V1,
  publicDeploymentEvidenceDocumentV1,
  publicDeploymentEvidenceDownloadHrefV1,
} from './PublicDeploymentEvidence';
import {
  DEPLOYED_PROGRAM_ROLES_V1,
  DEVNET_DEPLOYMENT_V1,
  DEVNET_PROGRAM_EVIDENCE_V1,
  LOCAL_DEPLOYMENT_V1,
  DEVNET_RELEASE_EVIDENCE_V1,
} from '@dclutch/sdk/deployments';

describe('public deployment evidence', () => {
  it('projects every devnet Program and ProgramData coordinate the manifest names', () => {
    // EIGHT since cohort-16: the seven checked roles and the accelerator. A
    // reader who downloads this to look a refusal up by program would find no
    // address for the band the merged accelerator owns.
    const document = publicDeploymentEvidenceDocumentV1();
    const programs = document.programs as Record<string, Record<string, unknown>>;
    expect(Object.keys(programs)).toEqual(DEPLOYED_PROGRAM_ROLES_V1);
    for (const role of DEPLOYED_PROGRAM_ROLES_V1) {
      expect(programs[role]).toEqual({
        program: DEVNET_DEPLOYMENT_V1.programs[role],
        programData: DEVNET_PROGRAM_EVIDENCE_V1[role].programData,
        // Named for what it is. A static document cannot observe a chain, and
        // "observed" invited the reader to treat a build-time constant as
        // current state; a FIRST deployment slot is a historical fact that
        // never ages. The same care was already taken with the link text below
        // and had not reached the field name.
        firstDeploymentSlot: DEVNET_PROGRAM_EVIDENCE_V1[role].deploymentSlot,
      });
      expect(programs[role].observedDeploymentSlot).toBeUndefined();
    }
    expect(document.genesisHash).toBe(DEVNET_DEPLOYMENT_V1.genesisHash);
    expect(document.activationCache).toBe(DEVNET_DEPLOYMENT_V1.activationCache);
    expect(document.cohort).toBe(DEVNET_RELEASE_EVIDENCE_V1.cohort);
    expect(document.sourceCommit).toBe(DEVNET_RELEASE_EVIDENCE_V1.sourceCommit);
    expect(document.evidence).toBe(DEVNET_RELEASE_EVIDENCE_V1.evidencePath);
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
    expect(devnet).toContain('View deployment record');
    expect(devnet).toContain('Download program addresses');
    // The slots are the cohort's own deploy slots, read off its ProgramData
    // headers. The programs are upgradeable in place, so an unqualified
    // "observed slot" would read as current chain state and this document
    // cannot observe a chain.
    expect(devnet).not.toContain('and observed slots');
    expect(devnet).toContain('download="dclutch-devnet-deployment-evidence-v1.json"');

    const local = renderToStaticMarkup(
      <PublicDeploymentEvidence deployment={LOCAL_DEPLOYMENT_V1} />,
    );
    expect(local).toContain('Selected deployment: Local');
    expect(local).not.toContain('download=');
  });

  it('does not attach checked evidence to different programs carrying the devnet label', () => {
    const changed = { ...DEVNET_DEPLOYMENT_V1, programs: { ...DEVNET_DEPLOYMENT_V1.programs, core: LOCAL_DEPLOYMENT_V1.programs.core } };
    const html = renderToStaticMarkup(<PublicDeploymentEvidence deployment={changed} />);
    expect(html).not.toContain('Deployment details');
    expect(html).not.toContain('download=');
  });
});
