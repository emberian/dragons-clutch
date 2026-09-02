import { PublicKey } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import liveOpenMarket from '../fixtures/live-open-market.json';
import {
  DEVNET_DEPLOYMENT_V1,
  DEVNET_PROGRAM_EVIDENCE_V1,
  LOCAL_DEPLOYMENT_V1,
  PROTOCOL_ROLES_V1,
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
  it('bakes the seven devnet addresses of the cohort that is actually running', () => {
    // NOT "permanent". They were called that -- DEPLOY_1.md §2 says
    // "these are the durable protocol addresses" -- and then devnet was ruled
    // disposable, and cohorts 9, 10, 11 and 12 each replaced the whole set and
    // CLOSED the one before. This table shipped cohort-8's ids for a day after
    // cohort-8 was closed, which is what a word like "permanent" buys.
    expect(DEVNET_DEPLOYMENT_V1.programs).toEqual({
      registry: '5c4CfHXHaLoJRtVSZFURp6Qhub8P4x8Hk4yZ3KJNrK53',
      rent: 'HD72aKvtRzBrVdmDGn8UrcocVA6g4NuG9Bt94GRLMYcW',
      custody: '2MHNgYoCtDzqRryjgAxzFwLVPztSN6NTUr7RmjiMrcLc',
      resolution: '9vs7atqDTAZTMo2a9iMZXD6Nf39jQZ7sZFf2X4pGDDvs',
      claims: 'GwduZB13AgqLxsoxi8wZEQndYBsQERea35dhuYKJzCvc',
      trading: 'Ahzug4zYhG8sc4t6tXjaSjnqbv7bTkgNYRc4kWUxYGJe',
      core: 'G4Wz4fj4zqBPFWYFF9CeYeJtTK5UqSZUu2fyCr9ANjYG',
    });
    // Generated, not typed: `scripts/derive-activation-hint.mjs --write` moves
    // this and the manifest together. Pinned here so the value cannot change
    // without a reviewer seeing it — but it is a HINT the session follows past,
    // so a cohort making it stale costs a reader accuracy, not a session.
    expect(DEVNET_DEPLOYMENT_V1.activationCache).toBe('9H5Jy4kjXJFiLtj8uGrCwhzYVw3nYsxRMAHA7s86p8cE');
    expect(DEVNET_DEPLOYMENT_V1.genesisHash).toBe('EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG');
    expect(DEVNET_DEPLOYMENT_V1.endpoint).toBe('https://api.devnet.solana.com');
  });

  it('carries the Loader-derived ProgramData and deployment slot for every devnet role', () => {
    expect(DEVNET_PROGRAM_EVIDENCE_V1.registry).toEqual({ programData: 'FbfJYjjyULLJYX8wzXqv5M5U2fxWLhCwwhvWaxCfWAsu', deploymentSlot: '491871867' });
    expect(DEVNET_PROGRAM_EVIDENCE_V1.core).toEqual({ programData: '6wzSk1ip5uuiiqQoCDUuJPNjDZdHtdaJZL5DSQHG4kcy', deploymentSlot: '491872487' });
    for (const role of PROTOCOL_ROLES_V1) {
      const evidence = DEVNET_PROGRAM_EVIDENCE_V1[role];
      expect(new PublicKey(evidence.programData).toBase58()).toBe(evidence.programData);
      expect(BigInt(evidence.deploymentSlot)).toBeGreaterThan(489_100_000n);
      // DERIVED, not transcribed. A ProgramData address is a pure function of
      // its Program id under the upgradeable loader, so a table row that does
      // not derive is a typo or a row belonging to another cohort -- which is
      // exactly the shape a hand-copied evidence table fails in. This makes the
      // whole table checkable offline against `programs` beside it.
      const [derived] = PublicKey.findProgramAddressSync(
        [new PublicKey(DEVNET_DEPLOYMENT_V1.programs[role]).toBytes()],
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
    expect(LOCAL_DEPLOYMENT_V1.endpoint).toBe('http://127.0.0.1:8899');
    expect(LOCAL_DEPLOYMENT_V1.genesisHash).toBeNull();
  });

  it('keeps every address canonical and every role distinct, in both clusters', () => {
    for (const deployment of [DEVNET_DEPLOYMENT_V1, LOCAL_DEPLOYMENT_V1]) {
      const addresses = PROTOCOL_ROLES_V1.map((role) => deployment.programs[role]);
      expect(new Set(addresses).size).toBe(PROTOCOL_ROLES_V1.length);
      for (const address of addresses) expect(new PublicKey(address).toBase58()).toBe(address);
    }
  });

  it('labels all seven role programs of a deployment by role and cluster', () => {
    const labels = deploymentProgramLabelsV1(DEVNET_DEPLOYMENT_V1);
    expect(Object.keys(labels)).toHaveLength(7);
    expect(labels[DEVNET_DEPLOYMENT_V1.programs.core]).toBe('dClutch Core · Devnet');
    expect(labels[DEVNET_DEPLOYMENT_V1.programs.registry]).toBe('dClutch Registry · Devnet');
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
