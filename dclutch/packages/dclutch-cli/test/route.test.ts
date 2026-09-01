import { SOLANA_DEVNET_GENESIS_HASH_V1 } from '@dclutch/sdk/rpc';
import { describe, expect, it } from 'vitest';

import type { CliContext } from '../src/context';
import {
  CHECKED_EXECUTION_RELEASE_COMMAND_V1,
  DIRECT_HOT_ROUTE_COMMAND_V3,
  checkedExecutionReleaseArgumentsV1,
  directHotRouteArgumentsV3,
  routeCommand,
} from '../src/commands/route';

const SHA_A = 'a'.repeat(64);
const SHA_B = 'b'.repeat(64);

function checked(path: string, sha256 = SHA_A) {
  return Object.freeze({ path, sha256 });
}

function context(flags: Readonly<Record<string, string | boolean | undefined>>): CliContext {
  return Object.freeze({
    rpcUrl: 'https://api.devnet.solana.com/',
    session: Object.freeze({ rpcUrl: null, programs: Object.freeze({}), markets: Object.freeze([]) }),
    flags: Object.freeze(flags),
    json: false,
  });
}

describe('checked Direct route production', () => {
  it('passes every role in the Rust producer\'s exact order with no signer capability', () => {
    const args = checkedExecutionReleaseArgumentsV1({
      rpcUrl: 'https://api.devnet.solana.com/',
      acknowledgment: SOLANA_DEVNET_GENESIS_HASH_V1,
      plan: '/evidence/plan.json',
      planSha256: SHA_A,
      checked: Object.freeze({
        core: checked('/evidence/core.checked'),
        claims: checked('/evidence/claims.checked'),
        trading: checked('/evidence/trading.checked'),
        resolution: checked('/evidence/resolution.checked'),
        custody: checked('/evidence/custody.checked'),
      }),
      output: '/evidence/execution-release.bin',
    });
    expect(args).toEqual([
      CHECKED_EXECUTION_RELEASE_COMMAND_V1,
      '--rpc-url', 'https://api.devnet.solana.com/',
      '--i-mean-devnet', SOLANA_DEVNET_GENESIS_HASH_V1,
      '--plan', '/evidence/plan.json',
      '--expected-plan-sha256', SHA_A,
      '--core-checked', '/evidence/core.checked', '--expected-core-checked-sha256', SHA_A,
      '--claims-checked', '/evidence/claims.checked', '--expected-claims-checked-sha256', SHA_A,
      '--trading-checked', '/evidence/trading.checked', '--expected-trading-checked-sha256', SHA_A,
      '--resolution-checked', '/evidence/resolution.checked', '--expected-resolution-checked-sha256', SHA_A,
      '--custody-checked', '/evidence/custody.checked', '--expected-custody-checked-sha256', SHA_A,
      '--output', '/evidence/execution-release.bin',
    ]);
    expect(args).not.toContain('--keypair');
  });

  it('passes only pinned route evidence to the existing read-only producer', () => {
    const args = directHotRouteArgumentsV3({
      rpcUrl: 'https://api.devnet.solana.com/',
      acknowledgment: SOLANA_DEVNET_GENESIS_HASH_V1,
      session: '/evidence/direct-session.json',
      checkedExecutionRelease: checked('/evidence/execution-release.bin'),
      registryChecked: checked('/evidence/registry.checked', SHA_B),
      rentChecked: checked('/evidence/rent.checked', SHA_B),
      output: '/evidence/direct-route.json',
    });
    expect(args).toEqual([
      DIRECT_HOT_ROUTE_COMMAND_V3,
      '--rpc-url', 'https://api.devnet.solana.com/',
      '--i-mean-devnet', SOLANA_DEVNET_GENESIS_HASH_V1,
      '--session', '/evidence/direct-session.json',
      '--checked-execution-release', '/evidence/execution-release.bin',
      '--expected-checked-execution-release-sha256', SHA_A,
      '--registry-checked', '/evidence/registry.checked',
      '--expected-registry-checked-sha256', SHA_B,
      '--rent-checked', '/evidence/rent.checked',
      '--expected-rent-checked-sha256', SHA_B,
      '--output', '/evidence/direct-route.json',
    ]);
    expect(args).not.toContain('--keypair');
  });

  it('admits the producer report for the exact output and exposes no signing step', async () => {
    const calls: string[] = [];
    const out: string[] = [];
    const invocation = context({
      'i-mean-devnet': SOLANA_DEVNET_GENESIS_HASH_V1,
      output: '/evidence/direct-route.json',
      session: '/evidence/direct-session.json',
      'checked-execution-release': '/evidence/execution-release.bin',
      'expected-checked-execution-release-sha256': SHA_A,
      'registry-checked': '/evidence/registry.checked',
      'expected-registry-checked-sha256': SHA_A,
      'rent-checked': '/evidence/rent.checked',
      'expected-rent-checked-sha256': SHA_A,
    });
    const report = Object.freeze({
      schema: 'dclutch-devnet-direct-hot-route-manifest-report-v1',
      format: 'dclutch-direct-hot-route-manifest-v3',
      output: '/evidence/direct-route.json',
      bytes: 12_345,
      sha256: SHA_A,
      market: 'market-address',
      lookupTable: 'lookup-address',
      checkedInfrastructureSha256: SHA_B,
    });
    const code = await routeCommand(invocation, { out: (line) => out.push(line), err: () => undefined }, 'direct', {}, {
      binary: () => {
        calls.push('locate-binary');
        return '/bin/successor';
      },
      spawn: (binary, args) => {
        calls.push('spawn');
        expect(binary).toBe('/bin/successor');
        expect(args[0]).toBe(DIRECT_HOT_ROUTE_COMMAND_V3);
        expect(args).not.toContain('--keypair');
        return Object.freeze({ status: 0, signal: null, stdout: JSON.stringify(report), stderr: '' });
      },
    });
    expect(code).toBe(0);
    expect(calls).toEqual(['locate-binary', 'spawn']);
    expect(out.join('\n')).toContain('portable Direct route manifest written');
    expect(out.join('\n')).toContain('no key read; no transaction submitted');
  });

  it('refuses unpinned or relative inputs before locating or spawning the producer', async () => {
    let capabilities = 0;
    const dependencies = {
      binary: () => { capabilities += 1; return '/bin/successor'; },
      spawn: () => { capabilities += 1; return Object.freeze({ status: 0, signal: null, stdout: '{}', stderr: '' }); },
    };
    const base = {
      'i-mean-devnet': SOLANA_DEVNET_GENESIS_HASH_V1,
      output: '/evidence/direct-route.json',
      session: '/evidence/direct-session.json',
      'checked-execution-release': '/evidence/execution-release.bin',
      'expected-checked-execution-release-sha256': SHA_A,
      'registry-checked': '/evidence/registry.checked',
      'expected-registry-checked-sha256': SHA_A,
      'rent-checked': '/evidence/rent.checked',
      'expected-rent-checked-sha256': SHA_A,
    };
    await expect(routeCommand(context({ ...base, 'registry-checked': 'relative.checked' }), { out: () => undefined, err: () => undefined }, 'direct', {}, dependencies))
      .rejects.toThrow(/must be an absolute path/);
    await expect(routeCommand(context({ ...base, 'expected-rent-checked-sha256': SHA_A.toUpperCase() }), { out: () => undefined, err: () => undefined }, 'direct', {}, dependencies))
      .rejects.toThrow(/exact lowercase SHA-256/);
    expect(capabilities).toBe(0);
  });
});
