import { PublicKey } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import liveOpenMarket from '../fixtures/live-open-market.json';
import {
  DEVNET_DEPLOYMENT_V1,
  DEVNET_PROGRAM_EVIDENCE_V1,
  LOCAL_DEPLOYMENT_V1,
  PROTOCOL_ROLES_V1,
  deploymentProgramLabelsV1,
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
  it('bakes the seven permanent devnet addresses from DEPLOY_1.md §2, verbatim', () => {
    expect(DEVNET_DEPLOYMENT_V1.programs).toEqual({
      registry: 'Hies39GBowHUMZw9rVCfaDTAXNorkQqMGKnukY2MD4Qj',
      rent: 'DgfYeuorJUmnktxgCmUXy65f6MFBGcc1aMQoauxoJCY3',
      custody: '34dhZkSUUhhFPL98KpWXaoG9aMs3EinZo5xN5epJEgGH',
      resolution: '2GHmxBawHTmwDRzqXuqdeC9A9Gj2HzucRd29wGpfgzmd',
      claims: '85hwTeQGabwFRs71Hafvngb1UmHb6dQoumBv3VV4epNN',
      trading: '5ywjTNdo6DGTe7bC8p9CgFYWFrBNePx61xeXp8Cdhbkk',
      core: 'HezRkcMGTZ5EY2LZk3i4uJbrAjUSDcamAw9B5v68z33N',
    });
    expect(DEVNET_DEPLOYMENT_V1.activationCache).toBe('Hz6BXyxyf66teABb6Pr6ev9jCZBJJpP5Q9p4sYJwJSkj');
    expect(DEVNET_DEPLOYMENT_V1.genesisHash).toBe('EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG');
    expect(DEVNET_DEPLOYMENT_V1.endpoint).toBe('https://api.devnet.solana.com');
  });

  it('carries DEPLOY_1.md §2 ProgramData and deployment slots for every devnet role', () => {
    expect(DEVNET_PROGRAM_EVIDENCE_V1.registry).toEqual({ programData: 'ENRSwrUEymWaXyrNtyD4QXXXk3tsTmcTGPTUFvnpsRVz', deploymentSlot: '489100383' });
    expect(DEVNET_PROGRAM_EVIDENCE_V1.core).toEqual({ programData: 'AD6mb5SP6yqc5GFexf3xhpr1wKaZQhS7Hrt41iZhKxaN', deploymentSlot: '489100672' });
    for (const role of PROTOCOL_ROLES_V1) {
      const evidence = DEVNET_PROGRAM_EVIDENCE_V1[role];
      expect(new PublicKey(evidence.programData).toBase58()).toBe(evidence.programData);
      expect(BigInt(evidence.deploymentSlot)).toBeGreaterThan(489_100_000n);
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
