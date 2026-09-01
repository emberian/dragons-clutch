/**
 * `--cluster`: the deployment the invocation means, and the proof the endpoint
 * is it.
 *
 * Every expectation here compares against the SDK's own `deployments` export
 * rather than a base58 string typed into this file. A test that respelled the
 * addresses would pass while the manifest drifted, which is the exact failure
 * the flag exists to prevent.
 */
import {
  DEVNET_DEPLOYMENT_V1,
  LOCAL_DEPLOYMENT_V1,
  PROTOCOL_ROLES_V1,
} from '@dclutch/sdk/deployments';
import type { MutationClusterAdmissionV1 } from '@dclutch/sdk/rpc';
import { SOLANA_DEVNET_GENESIS_HASH_V1 } from '@dclutch/sdk/rpc';
import { PublicKey } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import {
  PROGRAM_ROLES_V1,
  bindDeploymentIdentity,
  programId,
  resolveContext,
  resolvesAnyProgramFromDeployment,
  statedProgramId,
  type CliContext,
  type ProgramRoleV1,
} from '../src/context';
import {
  assertDeploymentIdentityV1,
  deploymentProgramIdV1,
  resolveClusterDeploymentV1,
} from '../src/deployment';
import { RUST_READER_COMMANDS_V1, unknownCommandV1 } from '../src/main';

const NO_ENV: NodeJS.ProcessEnv = Object.freeze({});

function key(byte: number): string {
  return new PublicKey(new Uint8Array(32).fill(byte)).toBase58();
}

/** A stub that answers the one identity question and counts the round trips. */
function identityClient(admission: MutationClusterAdmissionV1 | Error): Readonly<{
  calls: () => number;
  assertMutationCluster: () => Promise<MutationClusterAdmissionV1>;
}> {
  let calls = 0;
  return Object.freeze({
    calls: () => calls,
    assertMutationCluster: async () => {
      calls += 1;
      if (admission instanceof Error) throw admission;
      return admission;
    },
  });
}

const DEVNET_ADMISSION: MutationClusterAdmissionV1 = Object.freeze({
  endpoint: DEVNET_DEPLOYMENT_V1.endpoint,
  genesisHash: SOLANA_DEVNET_GENESIS_HASH_V1,
  kind: 'devnet',
});

const LOOPBACK_ADMISSION: MutationClusterAdmissionV1 = Object.freeze({
  endpoint: LOCAL_DEPLOYMENT_V1.endpoint,
  genesisHash: key(200),
  kind: 'loopback-local-validator',
});

describe('--cluster resolution', () => {
  it('gives every role the SDK manifest\'s exact devnet id, none of them respelled here', () => {
    const context = resolveContext({ cluster: 'devnet' }, NO_ENV);
    for (const role of PROGRAM_ROLES_V1) {
      expect(programId(context, role)).toBe(deploymentProgramIdV1(DEVNET_DEPLOYMENT_V1, role));
    }
    // The seven CLI roles cover the seven SDK roles exactly: a role added to
    // one side and not the other would leave a program unnameable.
    const resolved = new Set(PROGRAM_ROLES_V1.map((role) => programId(context, role)));
    expect(resolved).toEqual(new Set(PROTOCOL_ROLES_V1.map((role) => DEVNET_DEPLOYMENT_V1.programs[role])));
  });

  it('maps the CLI\'s rentCredit role onto the manifest\'s rent role', () => {
    const context = resolveContext({ cluster: 'local' }, NO_ENV);
    expect(programId(context, 'rentCredit')).toBe(LOCAL_DEPLOYMENT_V1.programs.rent);
  });

  it('takes the deployment endpoint when the caller named no rpc', () => {
    expect(resolveContext({ cluster: 'devnet' }, NO_ENV).rpcUrl).toBe(DEVNET_DEPLOYMENT_V1.endpoint);
    expect(resolveContext({ cluster: 'local' }, NO_ENV).rpcUrl).toBe(LOCAL_DEPLOYMENT_V1.endpoint);
  });

  it('still lets an explicit endpoint, environment, or session url win over the manifest', () => {
    expect(resolveContext({ cluster: 'devnet', rpc: 'http://127.0.0.1:9/' }, NO_ENV).rpcUrl).toBe('http://127.0.0.1:9/');
    expect(resolveContext({ cluster: 'devnet' }, { DCLUTCH_RPC: 'http://127.0.0.1:8/' }).rpcUrl).toBe('http://127.0.0.1:8/');
  });

  it('lets an explicitly named program id beat the manifest for that role only', () => {
    const context = resolveContext({ cluster: 'devnet', 'core-program': key(3) }, NO_ENV);
    expect(programId(context, 'core')).toBe(key(3));
    expect(programId(context, 'claims')).toBe(DEVNET_DEPLOYMENT_V1.programs.claims);
    expect(statedProgramId(context, 'core')).toBe(key(3));
    expect(statedProgramId(context, 'claims')).toBeNull();
  });

  it('refuses a cluster it does not ship, naming the ones it does', () => {
    expect(() => resolveClusterDeploymentV1('mainnet')).toThrow(/mainnet names no deployment/);
    expect(() => resolveClusterDeploymentV1('mainnet')).toThrow(/devnet, local/);
    expect(() => resolveClusterDeploymentV1('')).toThrow(/pass --cluster devnet\|local/);
  });

  it('keeps refusing to guess when no cluster is named, and says the flag now exists', () => {
    const context = resolveContext({}, NO_ENV);
    expect(context.deployment).toBeNull();
    expect(() => programId(context, 'core')).toThrow(/the core program id is not known/);
    expect(() => programId(context, 'core')).toThrow(/--cluster <devnet\|local>/);
  });
});

describe('endpoint identity binding', () => {
  it('admits devnet when the endpoint proves devnet genesis', async () => {
    const client = identityClient(DEVNET_ADMISSION);
    await expect(assertDeploymentIdentityV1(client, DEVNET_DEPLOYMENT_V1, 'markets ls')).resolves.toMatchObject({ kind: 'devnet' });
    expect(client.calls()).toBe(1);
  });

  it('refuses devnet when the endpoint is only an admitted local validator', async () => {
    const client = identityClient(LOOPBACK_ADMISSION);
    await expect(assertDeploymentIdentityV1(client, DEVNET_DEPLOYMENT_V1, 'markets ls'))
      .rejects.toThrow(/admitted as loopback-local-validator, not as the Devnet deployment/);
  });

  it('refuses a local deployment when the endpoint is the public devnet', async () => {
    const client = identityClient(DEVNET_ADMISSION);
    await expect(assertDeploymentIdentityV1(client, LOCAL_DEPLOYMENT_V1, 'portfolio'))
      .rejects.toThrow(/admitted as devnet, not as the Local deployment/);
  });

  it('refuses a devnet admission whose genesis is not the manifest\'s', async () => {
    const client = identityClient(Object.freeze({ endpoint: 'https://x.invalid', genesisHash: key(7), kind: 'devnet' as const }));
    await expect(assertDeploymentIdentityV1(client, DEVNET_DEPLOYMENT_V1, 'spine'))
      .rejects.toThrow(/reports genesis/);
  });

  it('carries the endpoint\'s own refusal into the cluster refusal', async () => {
    const client = identityClient(new Error('mutation refused: the endpoint reports Solana mainnet-beta genesis'));
    await expect(assertDeploymentIdentityV1(client, DEVNET_DEPLOYMENT_V1, 'markets ls'))
      .rejects.toThrow(/mainnet-beta genesis/);
  });
});

describe('when the binding is owed', () => {
  function contextOf(flags: Readonly<Record<string, string | boolean | undefined>>): CliContext {
    return resolveContext(flags, NO_ENV);
  }

  it('is owed when any role would come from the manifest', () => {
    expect(resolvesAnyProgramFromDeployment(contextOf({ cluster: 'devnet' }))).toBe(true);
    expect(resolvesAnyProgramFromDeployment(contextOf({ cluster: 'devnet', 'core-program': key(3) }))).toBe(true);
  });

  it('is not owed when the caller named no cluster at all', async () => {
    const client = identityClient(DEVNET_ADMISSION);
    expect(resolvesAnyProgramFromDeployment(contextOf({}))).toBe(false);
    await expect(bindDeploymentIdentity(contextOf({}), client, 'markets ls')).resolves.toBeNull();
    expect(client.calls()).toBe(0);
  });

  it('is not owed when the caller named every role themselves, so it spends no round trip', async () => {
    const flags: Record<string, string> = { cluster: 'devnet' };
    const byRole: Readonly<Record<ProgramRoleV1, string>> = Object.freeze({
      registry: 'registry-program', core: 'core-program', claims: 'claims-program',
      trading: 'trading-program', resolution: 'resolution-program', custody: 'custody-program',
      rentCredit: 'rent-credit-program',
    });
    let byte = 10;
    for (const role of PROGRAM_ROLES_V1) flags[byRole[role]] = key(byte++);
    const context = contextOf(flags);
    const client = identityClient(DEVNET_ADMISSION);
    expect(resolvesAnyProgramFromDeployment(context)).toBe(false);
    await expect(bindDeploymentIdentity(context, client, 'markets ls')).resolves.toBeNull();
    expect(client.calls()).toBe(0);
  });

  it('dials exactly once per command boundary when it is owed', async () => {
    const client = identityClient(DEVNET_ADMISSION);
    await expect(bindDeploymentIdentity(contextOf({ cluster: 'devnet' }), client, 'markets ls')).resolves.toMatchObject({ kind: 'devnet' });
    expect(client.calls()).toBe(1);
  });
});

/**
 * The two programs named `dclutch`.
 *
 * `tools/dclutch-cli` (cargo) and `packages/dclutch-cli` (npm) both declare the
 * executable name `dclutch`; the docs teach both under that bare name and
 * whichever is first on PATH answers. Measured 2026-09-01, before this change:
 * this client answered `dclutch market` / `capability` / `ticket` / `general` /
 * `fractional-retirement-next` with "unknown command", and the Rust binary
 * answered `markets ls` / `portfolio` / `refusal` / `found` / `redeem` /
 * `walk` / `spine` / `intent buy` the same way — a PATH fact that read as a
 * broken runbook.
 */
describe('the other binary also named dclutch', () => {
  it('names the program that owns each of its verbs instead of calling it a typo', () => {
    for (const command of RUST_READER_COMMANDS_V1) {
      const text = unknownCommandV1(command);
      expect(text).toContain('tools/dclutch-cli');
      expect(text).toContain('@dclutch/cli');
      expect(text).not.toContain('unknown command');
    }
  });

  it('still refuses an ordinary typo plainly', () => {
    expect(unknownCommandV1('marketss')).toBe('unknown command: marketss');
  });

  it('claims no verb this client actually dispatches', () => {
    for (const command of ['markets', 'portfolio', 'offer', 'intent', 'route', 'product', 'spine', 'redeem', 'found', 'join', 'walk', 'refusal', 'buy', 'sell']) {
      expect(RUST_READER_COMMANDS_V1).not.toContain(command);
    }
  });
});
