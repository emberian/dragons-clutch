import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join as joinPath } from 'node:path';

import { SOLANA_DEVNET_GENESIS_HASH_V1, type MutationClusterAdmissionV1 } from '@dclutch/sdk/rpc';
import { Keypair, PublicKey } from '@solana/web3.js';
import { afterEach, describe, expect, it } from 'vitest';

import { EMPTY_SESSION, type CliContext } from '../src/context';
import {
  JOIN_DEVNET_COMMAND_V1,
  JOIN_OWNED_LOOPBACK_COMMAND_V1,
  join,
  joinClusterV1,
  type JoinDependenciesV1,
  type JoinRpcClientV1,
  type JoinSpawnResultV1,
} from '../src/commands/join';
import { run } from '../src/main';

const LOOPBACK_RPC = 'http://127.0.0.1:20890/';
const DEVNET_RPC = 'https://api.devnet.solana.com';

const roots: string[] = [];
afterEach(() => {
  while (roots.length > 0) rmSync(roots.pop() as string, { force: true, recursive: true });
});

function root(): string {
  const path = mkdtempSync(joinPath(tmpdir(), 'dclutch-join-'));
  roots.push(path);
  return path;
}

/**
 * A deterministic Solana JSON keypair. `Keypair.fromSecretKey` authenticates
 * that the trailing 32 bytes are the seed's real public key, so the fixture is
 * derived rather than invented.
 */
function writeKeypair(directory: string, name: string, seedByte: number): Readonly<{ path: string; address: string }> {
  const keypair = Keypair.fromSeed(new Uint8Array(32).fill(seedByte));
  const path = joinPath(directory, name);
  writeFileSync(path, `${JSON.stringify(Array.from(keypair.secretKey))}\n`, { mode: 0o600 });
  return Object.freeze({ path, address: keypair.publicKey.toBase58() });
}

function key(byte: number): string {
  return new PublicKey(new Uint8Array(32).fill(byte)).toBase58();
}

function context(flags: Readonly<Record<string, string | boolean | undefined>>, rpcUrl: string): CliContext {
  return Object.freeze({ rpcUrl, session: EMPTY_SESSION, flags: Object.freeze({ ...flags, rpc: rpcUrl }), json: flags.json === true });
}

type Recorder = Readonly<{
  calls: string[][];
  clients: number;
  dependencies: JoinDependenciesV1;
}>;

/**
 * A child that writes the report the real successor fsyncs, so the command's
 * own report authentication is exercised instead of stubbed away.
 */
function recorder(
  output: string,
  options: Readonly<{
    cluster?: 'devnet' | 'owned-loopback';
    slot?: string;
    admission?: MutationClusterAdmissionV1;
    result?: JoinSpawnResultV1;
    writeReport?: boolean;
    phase?: string;
  }> = {},
): Recorder {
  const calls: string[][] = [];
  const state = { clients: 0 };
  const cluster = options.cluster ?? 'devnet';
  const schema = cluster === 'devnet'
    ? 'dclutch-devnet-user-position-admission-execution-v1'
    : 'dclutch-owned-loopback-user-position-admission-execution-v1';
  const dependencies: JoinDependenciesV1 = Object.freeze({
    spawn(_binary, args) {
      calls.push([...args]);
      if (options.writeReport !== false) {
        writeFileSync(output, `${JSON.stringify({
          schema,
          cluster,
          rpcUrl: cluster === 'devnet' ? DEVNET_RPC : LOOPBACK_RPC,
          authorizedMutation: args.includes('--execute'),
          phase: options.phase ?? (args.includes('--execute') ? 'finalized' : 'planned'),
        })}\n`);
      }
      return options.result ?? { status: 0, signal: null, stdout: '{}', stderr: '' };
    },
    client(): JoinRpcClientV1 {
      state.clients += 1;
      return {
        assertMutationCluster: async (): Promise<MutationClusterAdmissionV1> => options.admission ?? Object.freeze({
          endpoint: cluster === 'devnet' ? DEVNET_RPC : LOOPBACK_RPC,
          genesisHash: cluster === 'devnet' ? SOLANA_DEVNET_GENESIS_HASH_V1 : key(33),
          kind: cluster === 'devnet' ? 'devnet' : 'loopback-local-validator',
        }),
        finalizedSlot: async (): Promise<string> => options.slot ?? '4242',
      };
    },
  });
  return Object.freeze({
    calls,
    get clients() { return state.clients; },
    dependencies,
  }) as Recorder;
}

function io(): Readonly<{ out: string[]; err: string[]; io: { out: (line: string) => void; err: (line: string) => void } }> {
  const out: string[] = [];
  const err: string[] = [];
  return Object.freeze({ out, err, io: { out: (line) => out.push(line), err: (line) => err.push(line) } });
}

type Fixture = Readonly<{
  directory: string;
  binary: string;
  plan: string;
  evidence: string;
  output: string;
  owner: Readonly<{ path: string; address: string }>;
  payer: Readonly<{ path: string; address: string }>;
  source: Readonly<{ path: string; address: string }>;
}>;

function fixture(): Fixture {
  const directory = root();
  return Object.freeze({
    directory,
    binary: joinPath(directory, 'dclutch-local-successor-bootstrap'),
    plan: joinPath(directory, 'plan.json'),
    evidence: joinPath(directory, 'campaign.json'),
    output: joinPath(directory, 'admission.json'),
    owner: writeKeypair(directory, 'owner.json', 3),
    payer: writeKeypair(directory, 'payer.json', 4),
    source: writeKeypair(directory, 'source.json', 5),
  });
}

function devnetFlags(files: Fixture): Record<string, string | boolean | undefined> {
  return {
    'bootstrap-bin': files.binary,
    'i-mean-devnet': SOLANA_DEVNET_GENESIS_HASH_V1,
    plan: files.plan,
    'campaign-evidence': files.evidence,
    output: files.output,
    keypair: files.owner.path,
  };
}

describe('join cluster selection', () => {
  it('selects the owned-loopback subcommand for the exact 127.0.0.1 origin shape', () => {
    expect(joinClusterV1(LOOPBACK_RPC)).toBe('owned-loopback');
  });

  it('selects devnet for an external origin', () => {
    expect(joinClusterV1(DEVNET_RPC)).toBe('devnet');
  });

  it('refuses a loopback host in a shape the successor does not answer, without advising an acknowledgment', () => {
    expect(() => joinClusterV1('http://localhost:20890/')).toThrow(/spelling to fix, not a cluster to acknowledge/);
    expect(() => joinClusterV1('https://127.0.0.1:20890/')).toThrow(/spelling to fix/);
    expect(() => joinClusterV1('http://127.0.0.1/')).toThrow(/spelling to fix/);
  });
});

describe('join argv assembly', () => {
  it('assembles the devnet admission argv with the acknowledgment and derived pubkeys', async () => {
    const files = fixture();
    const spy = recorder(files.output);
    const lines = io();
    const code = await join(context(devnetFlags(files), DEVNET_RPC), lines.io, {}, spy.dependencies);
    expect(code).toBe(0);
    expect(spy.calls).toEqual([[
      JOIN_DEVNET_COMMAND_V1,
      '--rpc-url', DEVNET_RPC,
      '--i-mean-devnet', SOLANA_DEVNET_GENESIS_HASH_V1,
      '--plan', files.plan,
      '--campaign-evidence', files.evidence,
      '--position-owner', files.owner.address,
      '--position-owner-keypair', files.owner.path,
      '--fee-payer', files.owner.address,
      '--fee-payer-keypair', files.owner.path,
      '--minimum-finalized-slot', '4242',
      '--output', files.output,
    ]]);
  });

  it('assembles the owned-loopback argv, which names no cluster acknowledgment at all', async () => {
    const files = fixture();
    const spy = recorder(files.output, { cluster: 'owned-loopback' });
    const flags = devnetFlags(files);
    delete flags['i-mean-devnet'];
    const code = await join(context(flags, LOOPBACK_RPC), io().io, {}, spy.dependencies);
    expect(code).toBe(0);
    expect(spy.calls[0]?.[0]).toBe(JOIN_OWNED_LOOPBACK_COMMAND_V1);
    expect(spy.calls[0]).not.toContain('--i-mean-devnet');
    expect(spy.calls[0]?.slice(0, 3)).toEqual([JOIN_OWNED_LOOPBACK_COMMAND_V1, '--rpc-url', LOOPBACK_RPC]);
  });

  it('reads the position owner keypair for its public key alone and passes the file path to the child', async () => {
    const files = fixture();
    const spy = recorder(files.output);
    await join(context(devnetFlags(files), DEVNET_RPC), io().io, {}, spy.dependencies);
    const args = spy.calls[0] as string[];
    expect(args[args.indexOf('--position-owner') + 1]).toBe(Keypair.fromSeed(new Uint8Array(32).fill(3)).publicKey.toBase58());
    expect(args[args.indexOf('--position-owner-keypair') + 1]).toBe(files.owner.path);
    const secret = JSON.parse(readFileSync(files.owner.path, 'utf8')) as number[];
    expect(args.some((value) => value.includes(String(secret.slice(0, 8))))).toBe(false);
  });

  it('takes the position owner keypair from $DCLUTCH_KEYPAIR when no flag names one', async () => {
    const files = fixture();
    const spy = recorder(files.output);
    const flags = devnetFlags(files);
    delete flags.keypair;
    const code = await join(context(flags, DEVNET_RPC), io().io, { DCLUTCH_KEYPAIR: files.owner.path }, spy.dependencies);
    expect(code).toBe(0);
    const args = spy.calls[0] as string[];
    expect(args[args.indexOf('--position-owner-keypair') + 1]).toBe(files.owner.path);
  });
});

describe('join fee payer', () => {
  it('defaults the fee payer to the position owner, key and address alike', async () => {
    const files = fixture();
    const spy = recorder(files.output);
    await join(context(devnetFlags(files), DEVNET_RPC), io().io, {}, spy.dependencies);
    const args = spy.calls[0] as string[];
    expect(args[args.indexOf('--fee-payer') + 1]).toBe(files.owner.address);
    expect(args[args.indexOf('--fee-payer-keypair') + 1]).toBe(files.owner.path);
  });

  it('uses a separate fee payer keypair and its derived address when one is named', async () => {
    const files = fixture();
    const spy = recorder(files.output);
    await join(
      context({ ...devnetFlags(files), 'fee-payer-keypair': files.payer.path }, DEVNET_RPC),
      io().io,
      {},
      spy.dependencies,
    );
    const args = spy.calls[0] as string[];
    expect(args[args.indexOf('--fee-payer') + 1]).toBe(files.payer.address);
    expect(args[args.indexOf('--fee-payer-keypair') + 1]).toBe(files.payer.path);
    expect(files.payer.address).not.toBe(files.owner.address);
  });
});

describe('join cluster acknowledgment', () => {
  it('requires --i-mean-devnet for an external origin', async () => {
    const files = fixture();
    const spy = recorder(files.output);
    const flags = devnetFlags(files);
    delete flags['i-mean-devnet'];
    await expect(join(context(flags, DEVNET_RPC), io().io, {}, spy.dependencies))
      .rejects.toThrow('pass --i-mean-devnet <full devnet genesis hash>');
    expect(spy.calls).toEqual([]);
  });

  it('refuses a wrong genesis hash before any key file is read', async () => {
    const files = fixture();
    const spy = recorder(files.output);
    await expect(join(context({ ...devnetFlags(files), 'i-mean-devnet': key(6) }, DEVNET_RPC), io().io, {}, spy.dependencies))
      .rejects.toThrow(SOLANA_DEVNET_GENESIS_HASH_V1);
    expect(spy.calls).toEqual([]);
  });

  it('refuses --i-mean-devnet against a loopback origin rather than guessing which is the typo', async () => {
    const files = fixture();
    const spy = recorder(files.output, { cluster: 'owned-loopback' });
    await expect(join(context(devnetFlags(files), LOOPBACK_RPC), io().io, {}, spy.dependencies))
      .rejects.toThrow(/loopback origin needs no acknowledgment/);
    expect(spy.calls).toEqual([]);
  });
});

describe('join collateral tuple', () => {
  it('passes the four child collateral flags, with the source owner derived from its keypair', async () => {
    const files = fixture();
    const spy = recorder(files.output);
    await join(
      context({
        ...devnetFlags(files),
        'collateral-source-owner-keypair': files.source.path,
        'collateral-source-account': key(12),
        'collateral-quantity-atoms': '7',
      }, DEVNET_RPC),
      io().io,
      {},
      spy.dependencies,
    );
    expect((spy.calls[0] as string[]).slice(-8)).toEqual([
      '--collateral-source-owner', files.source.address,
      '--collateral-source-owner-keypair', files.source.path,
      '--collateral-source-account', key(12),
      '--collateral-quantity-atoms', '7',
    ]);
  });

  it.each([
    ['collateral-source-owner-keypair'],
    ['collateral-source-account'],
    ['collateral-quantity-atoms'],
  ])('refuses %s on its own: the tuple is all three or none', async (flag) => {
    const files = fixture();
    const spy = recorder(files.output);
    const value = flag === 'collateral-source-owner-keypair' ? files.source.path : flag === 'collateral-source-account' ? key(12) : '7';
    await expect(join(context({ ...devnetFlags(files), [flag]: value }, DEVNET_RPC), io().io, {}, spy.dependencies))
      .rejects.toThrow(/requires all three of/);
    expect(spy.calls).toEqual([]);
    expect(spy.clients).toBe(0);
  });

  it('refuses a zero or noncanonical collateral quantity', async () => {
    const files = fixture();
    const spy = recorder(files.output);
    const stated = (quantity: string): CliContext => context({
      ...devnetFlags(files),
      'collateral-source-owner-keypair': files.source.path,
      'collateral-source-account': key(12),
      'collateral-quantity-atoms': quantity,
    }, DEVNET_RPC);
    await expect(join(stated('0'), io().io, {}, spy.dependencies)).rejects.toThrow('--collateral-quantity-atoms must be nonzero');
    await expect(join(stated('007'), io().io, {}, spy.dependencies)).rejects.toThrow('--collateral-quantity-atoms must be a canonical decimal u64');
    expect(spy.calls).toEqual([]);
  });
});

describe('join finalized floor', () => {
  it('reads the floor from the endpoint after binding its cluster identity', async () => {
    const files = fixture();
    const spy = recorder(files.output, { slot: '99001' });
    await join(context(devnetFlags(files), DEVNET_RPC), io().io, {}, spy.dependencies);
    const args = spy.calls[0] as string[];
    expect(args[args.indexOf('--minimum-finalized-slot') + 1]).toBe('99001');
    expect(spy.clients).toBe(1);
  });

  it('refuses a devnet floor read when the endpoint no longer reports the acknowledged genesis', async () => {
    const files = fixture();
    const spy = recorder(files.output, {
      admission: Object.freeze({ endpoint: DEVNET_RPC, genesisHash: key(21), kind: 'devnet' }),
    });
    await expect(join(context(devnetFlags(files), DEVNET_RPC), io().io, {}, spy.dependencies))
      .rejects.toThrow(/no longer reports the exact acknowledged devnet genesis/);
    expect(spy.calls).toEqual([]);
  });

  it('refuses a loopback floor read when the endpoint is not a local validator', async () => {
    const files = fixture();
    const spy = recorder(files.output, {
      cluster: 'owned-loopback',
      admission: Object.freeze({ endpoint: LOOPBACK_RPC, genesisHash: SOLANA_DEVNET_GENESIS_HASH_V1, kind: 'devnet' }),
    });
    const flags = devnetFlags(files);
    delete flags['i-mean-devnet'];
    await expect(join(context(flags, LOOPBACK_RPC), io().io, {}, spy.dependencies))
      .rejects.toThrow(/addressed as a loopback validator but the endpoint reports devnet/);
    expect(spy.calls).toEqual([]);
  });

  it('states the floor without touching the endpoint when --minimum-finalized-slot is given', async () => {
    const files = fixture();
    const spy = recorder(files.output);
    await join(
      context({ ...devnetFlags(files), 'minimum-finalized-slot': '77' }, DEVNET_RPC),
      io().io,
      {},
      spy.dependencies,
    );
    const args = spy.calls[0] as string[];
    expect(args[args.indexOf('--minimum-finalized-slot') + 1]).toBe('77');
    expect(spy.clients).toBe(0);
  });
});

describe('join execution boundary', () => {
  it('never passes --execute on the default preflight', async () => {
    const files = fixture();
    const spy = recorder(files.output);
    const lines = io();
    const code = await join(context(devnetFlags(files), DEVNET_RPC), lines.io, {}, spy.dependencies);
    expect(code).toBe(0);
    expect(spy.calls[0]).not.toContain('--execute');
    expect(lines.out.join('\n')).toContain('preflight only (pass --execute to admit)');
    expect(lines.out.join('\n')).toContain('planned');
  });

  it('passes --execute once, after every other flag, when the caller authorizes it', async () => {
    const files = fixture();
    const spy = recorder(files.output);
    const code = await join(context({ ...devnetFlags(files), execute: true }, DEVNET_RPC), io().io, {}, spy.dependencies);
    expect(code).toBe(0);
    const args = spy.calls[0] as string[];
    expect(args.filter((value) => value === '--execute')).toEqual(['--execute']);
    expect(args[args.length - 1]).toBe('--execute');
  });

  it('refuses a report that does not record the authorization --execute requested', async () => {
    const files = fixture();
    const spy = recorder(files.output);
    const flags = { ...devnetFlags(files), execute: true };
    // A child that writes an unauthorized report is a mixed-up producer, not a
    // completed admission.
    const dependencies: JoinDependenciesV1 = Object.freeze({
      client: spy.dependencies.client,
      spawn(binary, args, options) {
        const result = spy.dependencies.spawn(binary, args, options);
        writeFileSync(files.output, `${JSON.stringify({
          schema: 'dclutch-devnet-user-position-admission-execution-v1',
          cluster: 'devnet',
          authorizedMutation: false,
          phase: 'planned',
        })}\n`);
        return result;
      },
    });
    await expect(join(context(flags, DEVNET_RPC), io().io, {}, dependencies))
      .rejects.toThrow(/does not record the authorization --execute requested/);
  });
});

describe('join child failure', () => {
  it('surfaces the child exit code and its stderr', async () => {
    const files = fixture();
    const spy = recorder(files.output, {
      writeReport: false,
      result: { status: 3, signal: null, stdout: '', stderr: 'campaign evidence does not join this exact plan\n' },
    });
    const lines = io();
    await expect(join(context(devnetFlags(files), DEVNET_RPC), lines.io, {}, spy.dependencies))
      .rejects.toThrow('participant admission exited 3: campaign evidence does not join this exact plan');
    expect(lines.err).toContain('campaign evidence does not join this exact plan');
  });

  it('surfaces a signal death as a signal, not as a success', async () => {
    const files = fixture();
    const spy = recorder(files.output, {
      writeReport: false,
      result: { status: null, signal: 'SIGKILL', stdout: '', stderr: '' },
    });
    await expect(join(context(devnetFlags(files), DEVNET_RPC), io().io, {}, spy.dependencies))
      .rejects.toThrow('participant admission exited by signal SIGKILL');
  });

  it('surfaces a child that could never start', async () => {
    const files = fixture();
    const spy = recorder(files.output, {
      writeReport: false,
      result: { status: null, signal: null, stdout: null, stderr: null, error: new Error('ENOENT') },
    });
    await expect(join(context(devnetFlags(files), DEVNET_RPC), io().io, {}, spy.dependencies))
      .rejects.toThrow('the participant admission driver could not start: ENOENT');
  });

  it('refuses a zero exit that wrote no report', async () => {
    const files = fixture();
    const spy = recorder(files.output, { writeReport: false });
    await expect(join(context(devnetFlags(files), DEVNET_RPC), io().io, {}, spy.dependencies))
      .rejects.toThrow(/exited 0 without writing its report/);
  });

  it('refuses a report written for the other cluster', async () => {
    const files = fixture();
    const spy = recorder(files.output, { cluster: 'owned-loopback' });
    await expect(join(
      context({ ...devnetFlags(files), 'minimum-finalized-slot': '5' }, DEVNET_RPC),
      io().io,
      {},
      spy.dependencies,
    )).rejects.toThrow(/is not this cluster's dclutch-devnet-user-position-admission-execution-v1/);
  });
});

describe('join input discipline', () => {
  it.each([['plan'], ['campaign-evidence'], ['output']])('requires an absolute --%s', async (flag) => {
    const files = fixture();
    const spy = recorder(files.output);
    await expect(join(context({ ...devnetFlags(files), [flag]: 'relative.json' }, DEVNET_RPC), io().io, {}, spy.dependencies))
      .rejects.toThrow(`--${flag} must be an absolute path`);
    expect(spy.calls).toEqual([]);
  });

  it.each([['plan'], ['campaign-evidence'], ['output']])('names the missing --%s instead of guessing one', async (flag) => {
    const files = fixture();
    const spy = recorder(files.output);
    const flags = devnetFlags(files);
    delete flags[flag];
    await expect(join(context(flags, DEVNET_RPC), io().io, {}, spy.dependencies)).rejects.toThrow(`pass --${flag}`);
  });

  it.each([['keypair'], ['fee-payer-keypair'], ['collateral-source-owner-keypair']])('requires an absolute --%s, before the endpoint is touched', async (flag) => {
    const files = fixture();
    const spy = recorder(files.output);
    const flags = {
      ...devnetFlags(files),
      'collateral-source-account': key(12),
      'collateral-quantity-atoms': '7',
      'collateral-source-owner-keypair': files.source.path,
      [flag]: 'relative-key.json',
    };
    await expect(join(context(flags, DEVNET_RPC), io().io, {}, spy.dependencies))
      .rejects.toThrow(`--${flag} must be an absolute path`);
    expect(spy.calls).toEqual([]);
    expect(spy.clients).toBe(0);
  });

  it('refuses to guess a position owner keypair', async () => {
    const files = fixture();
    const spy = recorder(files.output);
    const flags = devnetFlags(files);
    delete flags.keypair;
    await expect(join(context(flags, DEVNET_RPC), io().io, {}, spy.dependencies))
      .rejects.toThrow(/never reads a default wallet path/);
    expect(spy.calls).toEqual([]);
  });
});

describe('join through the dispatcher', () => {
  it('is a named command in the usage text', async () => {
    const out: string[] = [];
    const code = await run(['--help'], {}, { out: (line) => out.push(line), err: () => undefined });
    expect(code).toBe(0);
    expect(out[0]).toContain('join                             admit one participant into a founded market (--plan, --campaign-evidence, --output; preflight unless --execute)');
  });

  it('dispatches join and refuses before reading any caller-named file', async () => {
    const out: string[] = [];
    const err: string[] = [];
    const missing = '/this/join-file-must-not-be-read.json';
    const code = await run([
      '--rpc', DEVNET_RPC,
      '--keypair', missing,
      '--campaign-evidence', missing,
      '--output', missing,
      'join',
    ], {}, { out: (line) => out.push(line), err: (line) => err.push(line) });
    expect(code).toBe(1);
    expect(out).toEqual([]);
    expect(err).toEqual(['refused: pass --plan <absolute successor plan json>']);
  });
});
