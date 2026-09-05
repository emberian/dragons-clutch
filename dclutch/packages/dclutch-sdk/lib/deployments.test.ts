import { PublicKey } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import liveOpenMarket from '../fixtures/live-open-market.json';
import {
  DEPLOYED_PROGRAM_ROLES_V1,
  DEVNET_DEPLOYMENT_V1,
  DEVNET_PROGRAM_EVIDENCE_V1,
  LOCAL_DEPLOYMENT_V1,
  PROTOCOL_ROLES_V1,
  deployedProgramRolesV1,
  deploymentProgramLabelsV1,
  importDeploymentDocumentV1,
  parseCustomDeploymentV1,
} from './deployments';

/**
 * The manifest is a second copy of two published tables, ON PURPOSE: the
 * devnet rows must be exactly DEPLOY_1.md §2 ("The substrate — PERMANENT
 * ADDRESSES"), and the local rows must be exactly what a fixed-seed campaign
 * deploys — which `fixtures/live-open-market.json` captured off a real
 * campaign validator. These tests pin both copies to their sources, so a
 * "correction" to one table without the other is a red test, not a drift.
 */
describe('the deployment manifest', () => {
  it('bakes the eight devnet addresses of the cohort that is actually running', () => {
    // NOT "permanent". They were called that -- DEPLOY_1.md §2 says
    // "these are the durable protocol addresses" -- and then devnet was ruled
    // disposable, and cohorts 9 through 15 each replaced the whole set and
    // CLOSED the one before. This table shipped cohort-8's ids for a day after
    // cohort-8 was closed and cohort-14's for a morning after cohort-14 was,
    // which is what a word like "permanent" buys.
    //
    // Generated, not typed: `scripts/derive-deployment-manifest.mjs --write`
    // reads the sealed plan for the ids and the chain for what sits behind
    // them. These literals are the review copy, so the table cannot move
    // without a reviewer seeing which cohort it moved to.
    expect(DEVNET_DEPLOYMENT_V1.programs).toEqual({
      registry: '6gRRiB9BtQFN6AquyLXXjuiX1GYN2xyW8nqCTc3xJzkV',
      rent: '42xN9ULoMpULmeDbdGCtyAo82FRJved6sojUun6NSKdt',
      custody: '8UkoNCPD4JuWBiHWdc7WaM3j7Fj9jbf8Fe926Q1CDceo',
      resolution: 'jrjXw2Rph15VyJB3ztbRgoHUPJrcvMSHV6svRUYtUw3',
      claims: '8JfHfBBGaoUP1yV6VzXcvWwhQSZNV8eQmDAiYmCpNQJk',
      trading: 'ESQhDyV7obS4oNp7abjn7sSYChxtGrHru4TzvPuybJi3',
      core: '4wv7JxoAad6JMQi2vHJyByLXasWS8RzJSTdvEEmpCjpe',
      // The eighth, and the first cohort ever to deploy one from the runbook.
      // It is in `programs` and NOT in `PROTOCOL_ROLES_V1`, which is the whole
      // distinction: a program a reader can look up and a liveness gate must
      // ask about, and not a role any PDA derivation or owner check consults.
      accelerator: '6v1c2Go2h1rxkTN2EmzC5xGC35MTbaHPCHrKF6kTvg4y',
    });
    expect(deployedProgramRolesV1(DEVNET_DEPLOYMENT_V1)).toEqual([...DEPLOYED_PROGRAM_ROLES_V1]);
    expect(deployedProgramRolesV1(LOCAL_DEPLOYMENT_V1)).toEqual([...PROTOCOL_ROLES_V1]);
    // Generated, not typed: `scripts/derive-activation-hint.mjs --write` moves
    // this and the manifest together. Pinned here so the value cannot change
    // without a reviewer seeing it — but it is a HINT the session follows past,
    // so a cohort making it stale costs a reader accuracy, not a session.
    expect(DEVNET_DEPLOYMENT_V1.activationCache).toBe('2xVxMvfypJyo9bacGz1FFeK4L2qgqcsHaGoR9cbun6wV');
    expect(DEVNET_DEPLOYMENT_V1.genesisHash).toBe('EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG');
    expect(DEVNET_DEPLOYMENT_V1.endpoint).toBe('https://api.devnet.solana.com');
  });

  it('carries the Loader-derived ProgramData and deployment slot for every devnet program', () => {
    expect(DEVNET_PROGRAM_EVIDENCE_V1.registry).toEqual({ programData: '68Jh5pD42XWmYq5ViWoX3MKHMeENCRbgdxdGb8B7UY6k', deploymentSlot: '493638685' });
    expect(DEVNET_PROGRAM_EVIDENCE_V1.core).toEqual({ programData: 'BbyZZAwbz37VwLR6zMQMm2bJAhfqbJVFAxr9HbFRQ5AU', deploymentSlot: '493639301' });
    expect(DEVNET_PROGRAM_EVIDENCE_V1.accelerator).toEqual({ programData: 'DfJLGB1W12cUYGpw3doG2DmMDe6ubR2UkmrrUsqosa9g', deploymentSlot: '493639473' });
    for (const role of DEPLOYED_PROGRAM_ROLES_V1) {
      const evidence = DEVNET_PROGRAM_EVIDENCE_V1[role];
      expect(new PublicKey(evidence.programData).toBase58()).toBe(evidence.programData);
      expect(BigInt(evidence.deploymentSlot)).toBeGreaterThan(489_100_000n);
      // DERIVED, not transcribed. A ProgramData address is a pure function of
      // its Program id under the upgradeable loader, so a table row that does
      // not derive is a typo or a row belonging to another cohort -- which is
      // exactly the shape a hand-copied evidence table fails in. This makes the
      // whole table checkable offline against `programs` beside it.
      const [derived] = PublicKey.findProgramAddressSync(
        [new PublicKey(DEVNET_DEPLOYMENT_V1.programs[role] as string).toBytes()],
        new PublicKey('BPFLoaderUpgradeab1e11111111111111111111111'),
      );
      expect(derived.toBase58(), `${role} ProgramData`).toBe(evidence.programData);
    }
  });

  it('bakes the local layout the fixed-seed campaign actually deploys (fixture-pinned)', () => {
    const fixture = (liveOpenMarket as Readonly<{ programs: Readonly<Record<string, string>> }>).programs;
    for (const role of PROTOCOL_ROLES_V1) {
      expect(LOCAL_DEPLOYMENT_V1.programs[role], `local ${role} program`).toBe(fixture[role]);
    }
    // The gauntlet's tier-1 campaign deploys seven programs and no accelerator,
    // so the manifest says so rather than carrying an address nothing deploys.
    expect(LOCAL_DEPLOYMENT_V1.programs.accelerator).toBeUndefined();
    expect(LOCAL_DEPLOYMENT_V1.endpoint).toBe('http://127.0.0.1:8899');
    expect(LOCAL_DEPLOYMENT_V1.genesisHash).toBeNull();
  });

  it('keeps every address canonical and every program distinct, in both clusters', () => {
    for (const deployment of [DEVNET_DEPLOYMENT_V1, LOCAL_DEPLOYMENT_V1]) {
      const roles = deployedProgramRolesV1(deployment);
      const addresses = roles.map((role) => deployment.programs[role] as string);
      expect(new Set(addresses).size).toBe(roles.length);
      for (const address of addresses) expect(new PublicKey(address).toBase58()).toBe(address);
    }
  });

  it('labels every program of a deployment by role and cluster', () => {
    const labels = deploymentProgramLabelsV1(DEVNET_DEPLOYMENT_V1);
    expect(Object.keys(labels)).toHaveLength(8);
    expect(labels[DEVNET_DEPLOYMENT_V1.programs.core]).toBe('dClutch Core · Devnet');
    expect(labels[DEVNET_DEPLOYMENT_V1.programs.registry]).toBe('dClutch Registry · Devnet');
    expect(labels[DEVNET_DEPLOYMENT_V1.programs.accelerator as string]).toBe('dClutch Accelerator · Devnet');
    expect(Object.keys(deploymentProgramLabelsV1(LOCAL_DEPLOYMENT_V1))).toHaveLength(7);
  });
});

describe('custom deployment parsing', () => {
  const valid = Object.freeze({
    endpoint: 'http://127.0.0.1:21890/',
    programs: LOCAL_DEPLOYMENT_V1.programs,
    activationCache: null,
  });

  it('admits a complete custom deployment and marks its provenance as the reader’s', () => {
    const parsed = parseCustomDeploymentV1(valid);
    expect(parsed.cluster).toBe('custom');
    expect(parsed.programs.core).toBe(LOCAL_DEPLOYMENT_V1.programs.core);
    expect(parsed.genesisHash).toBeNull();
    expect(parsed.provenance).toContain('Your own deployment');
  });

  it('refuses a missing role, naming it', () => {
    const missingCore: Record<string, unknown> = { ...LOCAL_DEPLOYMENT_V1.programs };
    delete missingCore.core;
    expect(() => parseCustomDeploymentV1({ ...valid, programs: missingCore })).toThrow('core program is required');
  });

  it('admits an accelerator when one is named, and never demands one', () => {
    // An operator running a local successor has seven programs. A form that
    // required an eighth would refuse the deployment they actually have; a form
    // that silently dropped a named one would lose the program three route
    // families CPI into.
    expect(parseCustomDeploymentV1(valid).programs.accelerator).toBeUndefined();
    const withAccelerator = parseCustomDeploymentV1({
      ...valid,
      programs: { ...LOCAL_DEPLOYMENT_V1.programs, accelerator: DEVNET_DEPLOYMENT_V1.programs.accelerator },
    });
    expect(withAccelerator.programs.accelerator).toBe(DEVNET_DEPLOYMENT_V1.programs.accelerator);
    expect(Object.keys(deploymentProgramLabelsV1(withAccelerator))).toHaveLength(8);
    expect(() => parseCustomDeploymentV1({
      ...valid,
      programs: { ...LOCAL_DEPLOYMENT_V1.programs, accelerator: LOCAL_DEPLOYMENT_V1.programs.trading },
    })).toThrow('distinct from the seven');
    expect(() => parseCustomDeploymentV1({
      ...valid,
      programs: { ...LOCAL_DEPLOYMENT_V1.programs, accelerator: 'not-an-address' },
    })).toThrow('accelerator program is not a Solana address');
  });

  it('refuses a noncanonical address, an aliased role pair, and a non-http endpoint', () => {
    expect(() => parseCustomDeploymentV1({ ...valid, programs: { ...LOCAL_DEPLOYMENT_V1.programs, core: 'not-an-address' } })).toThrow('core program is not a Solana address');
    expect(() => parseCustomDeploymentV1({ ...valid, programs: { ...LOCAL_DEPLOYMENT_V1.programs, core: LOCAL_DEPLOYMENT_V1.programs.claims } })).toThrow('distinct');
    expect(() => parseCustomDeploymentV1({ ...valid, endpoint: 'ws://127.0.0.1:8900' })).toThrow('http or https');
  });
});

describe('operator document import', () => {
  const roles = {
    registry: { program_id: LOCAL_DEPLOYMENT_V1.programs.registry },
    core: { program_id: LOCAL_DEPLOYMENT_V1.programs.core },
    claims: { program_id: LOCAL_DEPLOYMENT_V1.programs.claims },
    trading: { program_id: LOCAL_DEPLOYMENT_V1.programs.trading },
    resolution: { program_id: LOCAL_DEPLOYMENT_V1.programs.resolution },
    custody: { program_id: LOCAL_DEPLOYMENT_V1.programs.custody },
    rent_credit: { program_id: LOCAL_DEPLOYMENT_V1.programs.rent },
  };

  it('fills all seven roles and the endpoint from a successor run spec', () => {
    const imported = importDeploymentDocumentV1(JSON.stringify({
      schema: 'dclutch-local-successor-run-spec-v2',
      rpc_url: 'http://127.0.0.1:20890/',
      ...roles,
    }));
    expect(imported.endpoint).toBe('http://127.0.0.1:20890/');
    expect(imported.programs.rent).toBe(LOCAL_DEPLOYMENT_V1.programs.rent);
    expect(imported.programs.core).toBe(LOCAL_DEPLOYMENT_V1.programs.core);
  });

  it('fills the roles from an infrastructure plan and says it has no endpoint', () => {
    const imported = importDeploymentDocumentV1(JSON.stringify({
      schema: 'dclutch-local-successor-infrastructure-plan-v3',
      ...roles,
    }));
    expect(imported.endpoint).toBeNull();
    expect(imported.programs.trading).toBe(LOCAL_DEPLOYMENT_V1.programs.trading);
  });

  it('carries the plan’s general_accelerator when it names one, and no eighth when it does not', () => {
    // `general_accelerator` is OPTIONAL in the plan schema and every genesis
    // plan carries none, so its absence is an absence and not a missing role.
    expect(importDeploymentDocumentV1(JSON.stringify({
      schema: 'dclutch-local-successor-infrastructure-plan-v3', ...roles,
    })).programs.accelerator).toBeUndefined();
    const imported = importDeploymentDocumentV1(JSON.stringify({
      schema: 'dclutch-local-successor-infrastructure-plan-v3',
      ...roles,
      general_accelerator: { program_id: DEVNET_DEPLOYMENT_V1.programs.accelerator },
    }));
    expect(imported.programs.accelerator).toBe(DEVNET_DEPLOYMENT_V1.programs.accelerator);
  });

  it('names a retired v2 plan rather than calling it a foreign document', () => {
    expect(() => importDeploymentDocumentV1(JSON.stringify({
      schema: 'dclutch-local-successor-infrastructure-plan-v2',
      ...roles,
    }))).toThrow('retired');
  });

  it('refuses non-JSON, foreign schemas, and missing roles by name', () => {
    expect(() => importDeploymentDocumentV1('not json')).toThrow('not JSON');
    expect(() => importDeploymentDocumentV1('{"schema":"something-else"}')).toThrow('neither a successor run spec');
    const missingRent: Record<string, unknown> = { ...roles };
    delete missingRent.rent_credit;
    expect(() => importDeploymentDocumentV1(JSON.stringify({
      schema: 'dclutch-local-successor-run-spec-v2', ...missingRent,
    }))).toThrow('rent_credit');
  });
});
