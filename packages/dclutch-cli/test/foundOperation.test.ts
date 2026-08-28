import { createHash } from 'node:crypto';
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import { SOLANA_DEVNET_GENESIS_HASH_V1 } from '@dclutch/sdk/rpc';
import { PublicKey } from '@solana/web3.js';
import { afterEach, describe, expect, it } from 'vitest';

import { EMPTY_SESSION, type CliContext } from '../src/context';
import {
  FOUND_OPERATION_SCHEMA_V1,
  runFoundOperationV1,
  type FoundOperationDependenciesV1,
} from '../src/foundOperation';

const roots: string[] = [];
afterEach(() => {
  while (roots.length > 0) rmSync(roots.pop() as string, { force: true, recursive: true });
});

function root(): string {
  const path = mkdtempSync(join(tmpdir(), 'dclutch-found-operation-'));
  roots.push(path);
  return path;
}

function key(byte: number): string {
  return new PublicKey(new Uint8Array(32).fill(byte)).toBase58();
}

function sha(source: Uint8Array): string {
  return createHash('sha256').update(source).digest('hex');
}

function context(): CliContext {
  return Object.freeze({
    rpcUrl: 'https://api.devnet.solana.com',
    session: EMPTY_SESSION,
    flags: Object.freeze({
      rpc: 'https://api.devnet.solana.com',
      'i-mean-devnet': SOLANA_DEVNET_GENESIS_HASH_V1,
    }),
    json: false,
  });
}

function fixture(directory: string, collateral: boolean = false): {
  binary: string;
  operation: string;
  journal: string;
  market: string;
  evidence: string;
  participant: string;
  session: string;
  plan: string;
} {
  const binary = join(directory, 'successor');
  const plan = join(directory, 'plan.json');
  const operation = join(directory, 'operation.json');
  const journal = join(directory, 'journal.json');
  const market = join(directory, 'market.json');
  const evidence = join(directory, 'campaign.json');
  const participant = join(directory, 'participant.json');
  const session = join(directory, 'session.json');
  const price = join(directory, 'price-update.bin');
  writeFileSync(binary, '#!/bin/false\n', { mode: 0o700 });
  writeFileSync(price, 'public-price-update');
  writeFileSync(plan, `${JSON.stringify({
    schema: 'dclutch-local-successor-run-spec-v2',
    rpc_url: 'https://api.devnet.solana.com',
    registry: { program_id: key(1) },
    core: { program_id: key(2) },
    claims: { program_id: key(3) },
    trading: { program_id: key(4) },
    resolution: { program_id: key(5) },
    custody: { program_id: key(6) },
    rent_credit: { program_id: key(7) },
  })}\n`);
  const roles = [
    'core-upgrade-authority', 'collateral-mint', 'collateral-wallet',
    'founding-beneficiary', 'founding-founder', 'founding-projection-witness',
    'founding-source-funder', 'substituted-founder',
  ];
  writeFileSync(operation, `${JSON.stringify({
    schema: FOUND_OPERATION_SCHEMA_V1,
    plan,
    market: {
      kind: 'flagship',
      arguments: [
        '--registry-program-id', key(1),
        '--direct-fee-basis-points', '25',
        '--direct-fee-recipient', key(8),
        '--price-update', price,
        '--window-start', '1000',
      ],
      output: market,
    },
    campaign: {
      evidence,
      keypairs: roles.map((role, index) => ({ role, path: join(directory, `absent-key-${index}.json`) })),
    },
    participant: {
      output: participant,
      positionOwner: key(9),
      positionOwnerKeypair: join(directory, 'absent-position-key.json'),
      feePayer: key(10),
      feePayerKeypair: join(directory, 'absent-payer-key.json'),
      minimumFinalizedSlot: '91',
      collateral: collateral ? {
        sourceOwner: key(11),
        sourceOwnerKeypair: join(directory, 'absent-source-key.json'),
        sourceAccount: key(12),
        quantityAtoms: '7',
      } : null,
    },
  }, null, 2)}\n`);
  return { binary, operation, journal, market, evidence, participant, session, plan };
}

function flag(arguments_: ReadonlyArray<string>, name: string): string {
  const index = arguments_.indexOf(name);
  if (index < 0 || arguments_[index + 1] === undefined) throw new Error(`missing ${name}`);
  return arguments_[index + 1] as string;
}

function successfulDependencies(
  calls: string[][],
  failures: Readonly<{
    campaignAfterEvidence?: boolean;
    participantBeforeEvidence?: boolean;
  }> = Object.freeze({}),
): FoundOperationDependenciesV1 {
  return Object.freeze({
    invoke(_binary, arguments_) {
      calls.push([...arguments_]);
      const command = arguments_[0];
      if (command === 'devnet-market') {
        return { status: 0, stdout: Buffer.from('{"market":"exact-rust-authored"}\n'), stderr: new Uint8Array() };
      }
      if (command === 'campaign') {
        if (!arguments_.includes('--execute') && existsSync(flag(arguments_, '--evidence'))) {
          return { status: 0, stdout: new Uint8Array(), stderr: new Uint8Array() };
        }
        const plan = readFileSync(flag(arguments_, '--plan'));
        const market = readFileSync(flag(arguments_, '--market'));
        writeFileSync(flag(arguments_, '--evidence'), `${JSON.stringify({
          schema: 'dclutch-successor-campaign-report-v1',
          cluster: 'devnet',
          mode: 'execute',
          plan_sha256: sha(plan),
          market_sha256: sha(market),
          execution: {
            completed: true,
            market: { accounts: { founding_market: { address: key(13) } } },
          },
        })}\n`);
        return { status: failures.campaignAfterEvidence === true ? 1 : 0, stdout: new Uint8Array(), stderr: new Uint8Array() };
      }
      if (command === 'devnet-user-position-admission-v1') {
        if (failures.participantBeforeEvidence === true) {
          return { status: 1, stdout: new Uint8Array(), stderr: new Uint8Array() };
        }
        if (!arguments_.includes('--execute') && existsSync(flag(arguments_, '--output'))) {
          return { status: 0, stdout: new Uint8Array(), stderr: new Uint8Array() };
        }
        const plan = readFileSync(flag(arguments_, '--plan'));
        const evidence = readFileSync(flag(arguments_, '--campaign-evidence'));
        const collateral = arguments_.includes('--collateral-source-owner') ? {
          phase: 'finalized',
          intent: {
            sourceOwner: flag(arguments_, '--collateral-source-owner'),
            sourceAccount: flag(arguments_, '--collateral-source-account'),
            quantityAtoms: Number(flag(arguments_, '--collateral-quantity-atoms')),
          },
        } : undefined;
        writeFileSync(flag(arguments_, '--output'), `${JSON.stringify({
          schema: 'dclutch-devnet-user-position-admission-execution-v1',
          cluster: 'devnet',
          phase: 'finalized',
          intent: {
            planSha256: sha(plan),
            campaignEvidenceSha256: sha(evidence),
            positionOwner: flag(arguments_, '--position-owner'),
            feePayer: flag(arguments_, '--fee-payer'),
            minimumFinalizedSlot: Number(flag(arguments_, '--minimum-finalized-slot')),
          },
          collateral,
        })}\n`);
        return { status: 0, stdout: new Uint8Array(), stderr: new Uint8Array() };
      }
      throw new Error(`unexpected command ${command}`);
    },
  });
}

describe('permanent-devnet founding exterior', () => {
  it('prepares only the Rust-authored Market input without touching any key path', () => {
    const files = fixture(root());
    const calls: string[][] = [];
    const out: string[] = [];
    expect(runFoundOperationV1(
      context(), { out: (line) => out.push(line), err: () => undefined },
      files.binary, files.operation, files.journal, null, false,
      successfulDependencies(calls),
    )).toBe(0);
    expect(calls).toHaveLength(1);
    expect(calls[0]?.[0]).toBe('devnet-market');
    expect(readFileSync(files.market, 'utf8')).toBe('{"market":"exact-rust-authored"}\n');
    expect(JSON.parse(readFileSync(files.journal, 'utf8')).phase).toBe('market-prepared');
    expect(out.at(-1)).toMatch(/rerun.*--execute/);
  });

  it('runs the same dossier through founding and exact participant admission', () => {
    const files = fixture(root(), true);
    const calls: string[][] = [];
    const out: string[] = [];
    expect(runFoundOperationV1(
      context(), { out: (line) => out.push(line), err: () => undefined },
      files.binary, files.operation, files.journal, files.session, true,
      successfulDependencies(calls),
    )).toBe(0);
    expect(calls.map((call) => call[0])).toEqual([
      'devnet-market', 'campaign', 'devnet-user-position-admission-v1',
    ]);
    const campaign = calls[1] as string[];
    const participant = calls[2] as string[];
    expect(flag(campaign, '--plan')).toBe(files.plan);
    expect(flag(campaign, '--market')).toBe(files.market);
    expect(flag(participant, '--plan')).toBe(files.plan);
    expect(flag(participant, '--campaign-evidence')).toBe(files.evidence);
    expect(campaign).toContain('--execute');
    expect(participant).toContain('--execute');
    expect(flag(participant, '--collateral-quantity-atoms')).toBe('7');
    expect(JSON.parse(readFileSync(files.journal, 'utf8'))).toMatchObject({ phase: 'participant-complete', authorizedMutation: true });
    expect(JSON.parse(readFileSync(files.session, 'utf8')).markets).toEqual([key(13)]);
    expect(out.at(-1)).toContain(files.journal);
  });

  it('reconciles a completed child report after a crash without rerunning founding', () => {
    const files = fixture(root());
    const firstCalls: string[][] = [];
    expect(() => runFoundOperationV1(
      context(), { out: () => undefined, err: () => undefined },
      files.binary, files.operation, files.journal, null, true,
      successfulDependencies(firstCalls, { campaignAfterEvidence: true }),
    )).toThrow(/founding campaign exited 1/);
    expect(JSON.parse(readFileSync(files.journal, 'utf8')).phase).toBe('market-prepared');

    const resumedCalls: string[][] = [];
    expect(runFoundOperationV1(
      context(), { out: () => undefined, err: () => undefined },
      files.binary, files.operation, files.journal, null, true,
      successfulDependencies(resumedCalls),
    )).toBe(0);
    expect(resumedCalls.map((call) => call[0])).toEqual([
      'campaign', 'devnet-user-position-admission-v1',
    ]);
  });

  it('digest-checks a founding-complete resume before any canonical owner or key path', () => {
    const files = fixture(root());
    expect(() => runFoundOperationV1(
      context(), { out: () => undefined, err: () => undefined },
      files.binary, files.operation, files.journal, null, true,
      successfulDependencies([], { participantBeforeEvidence: true }),
    )).toThrow(/participant admission exited 1/);
    const journalBefore = readFileSync(files.journal);
    expect(JSON.parse(journalBefore.toString()).phase).toBe('founding-complete');
    writeFileSync(files.evidence, Buffer.concat([readFileSync(files.evidence), Buffer.from(' ')]));

    const resumedCalls: string[][] = [];
    expect(() => runFoundOperationV1(
      context(), { out: () => undefined, err: () => undefined },
      files.binary, files.operation, files.journal, null, true,
      successfulDependencies(resumedCalls),
    )).toThrow(/campaign evidence changed after it was durably joined/);
    expect(resumedCalls).toHaveLength(0);
    expect(readFileSync(files.journal)).toEqual(journalBefore);
  });

  it('authenticates both complete reports through their Rust owners and refuses either drift first', () => {
    const files = fixture(root());
    runFoundOperationV1(
      context(), { out: () => undefined, err: () => undefined },
      files.binary, files.operation, files.journal, null, true,
      successfulDependencies([]),
    );
    const campaign = readFileSync(files.evidence);
    const participant = readFileSync(files.participant);
    const journal = readFileSync(files.journal);

    writeFileSync(files.participant, Buffer.concat([participant, Buffer.from('\n')]));
    const participantDriftCalls: string[][] = [];
    expect(() => runFoundOperationV1(
      context(), { out: () => undefined, err: () => undefined },
      files.binary, files.operation, files.journal, null, false,
      successfulDependencies(participantDriftCalls),
    )).toThrow(/participant evidence changed after it was durably joined/);
    expect(participantDriftCalls).toHaveLength(0);
    expect(readFileSync(files.journal)).toEqual(journal);

    writeFileSync(files.participant, participant);
    writeFileSync(files.evidence, Buffer.concat([campaign, Buffer.from('\n')]));
    const campaignDriftCalls: string[][] = [];
    expect(() => runFoundOperationV1(
      context(), { out: () => undefined, err: () => undefined },
      files.binary, files.operation, files.journal, null, false,
      successfulDependencies(campaignDriftCalls),
    )).toThrow(/campaign evidence changed after it was durably joined/);
    expect(campaignDriftCalls).toHaveLength(0);
    expect(readFileSync(files.journal)).toEqual(journal);

    writeFileSync(files.evidence, campaign);
    const pollCalls: string[][] = [];
    expect(runFoundOperationV1(
      context(), { out: () => undefined, err: () => undefined },
      files.binary, files.operation, files.journal, null, false,
      successfulDependencies(pollCalls),
    )).toBe(0);
    expect(pollCalls.map((call) => call[0])).toEqual([
      'campaign', 'devnet-user-position-admission-v1',
    ]);
    expect(pollCalls.every((call) => !call.includes('--execute'))).toBe(true);
  });

  it('holds an exclusive durable operation lock across child dispatch and releases only its own inode', () => {
    const files = fixture(root());
    const calls: string[][] = [];
    const base = successfulDependencies(calls);
    let raced = false;
    const dependencies: FoundOperationDependenciesV1 = Object.freeze({
      invoke(binary, arguments_) {
        if (!raced && arguments_[0] === 'devnet-market') {
          raced = true;
          expect(() => runFoundOperationV1(
            context(), { out: () => undefined, err: () => undefined },
            files.binary, files.operation, files.journal, null, false,
            dependencies,
          )).toThrow(/is locked.*never removed automatically/i);
        }
        return base.invoke(binary, arguments_);
      },
    });
    expect(runFoundOperationV1(
      context(), { out: () => undefined, err: () => undefined },
      files.binary, files.operation, files.journal, null, false,
      dependencies,
    )).toBe(0);
    expect(raced).toBe(true);
    expect(existsSync(`${files.journal}.lock`)).toBe(false);

    expect(runFoundOperationV1(
      context(), { out: () => undefined, err: () => undefined },
      files.binary, files.operation, files.journal, null, false,
      successfulDependencies([]),
    )).toBe(0);
  });

  it('refuses wrong genesis, shared-flag injection, operation drift, and duplicate roles', () => {
    const files = fixture(root());
    const badContext = { ...context(), flags: Object.freeze({ rpc: 'https://api.devnet.solana.com', 'i-mean-devnet': key(1) }) };
    expect(() => runFoundOperationV1(
      badContext, { out: () => undefined, err: () => undefined }, files.binary,
      files.operation, files.journal, null, false, successfulDependencies([]),
    )).toThrow(/full genesis hash/);

    const operation = JSON.parse(readFileSync(files.operation, 'utf8')) as {
      market: { arguments: string[] };
    };
    operation.market.arguments.push('--rpc-url', 'https://mainnet.example');
    writeFileSync(files.operation, JSON.stringify(operation));
    expect(() => runFoundOperationV1(
      context(), { out: () => undefined, err: () => undefined }, files.binary,
      files.operation, files.journal, null, false, successfulDependencies([]),
    )).toThrow(/not an admitted flagship market argument/);

    const files2 = fixture(root());
    runFoundOperationV1(
      context(), { out: () => undefined, err: () => undefined }, files2.binary,
      files2.operation, files2.journal, null, false, successfulDependencies([]),
    );
    const changed = JSON.parse(readFileSync(files2.operation, 'utf8')) as {
      participant: { minimumFinalizedSlot: string };
    };
    changed.participant.minimumFinalizedSlot = '92';
    writeFileSync(files2.operation, JSON.stringify(changed));
    expect(() => runFoundOperationV1(
      context(), { out: () => undefined, err: () => undefined }, files2.binary,
      files2.operation, files2.journal, null, false, successfulDependencies([]),
    )).toThrow(/another operation/);

    const files3 = fixture(root());
    const duplicate = JSON.parse(readFileSync(files3.operation, 'utf8')) as {
      campaign: { keypairs: Array<{ role: string }> };
    };
    duplicate.campaign.keypairs[7].role = duplicate.campaign.keypairs[0].role;
    writeFileSync(files3.operation, JSON.stringify(duplicate));
    expect(() => runFoundOperationV1(
      context(), { out: () => undefined, err: () => undefined }, files3.binary,
      files3.operation, files3.journal, null, false, successfulDependencies([]),
    )).toThrow(/roles must be exactly/);

    const files4 = fixture(root());
    const exact = readFileSync(files4.operation, 'utf8');
    writeFileSync(files4.operation, exact.replace('{', `{"schema":"attacker",`));
    expect(() => runFoundOperationV1(
      context(), { out: () => undefined, err: () => undefined }, files4.binary,
      files4.operation, files4.journal, null, false, successfulDependencies([]),
    )).toThrow(/duplicate JSON object key "schema"/);

    const files5 = fixture(root());
    const keyAlias = JSON.parse(readFileSync(files5.operation, 'utf8')) as {
      campaign: { evidence: string; keypairs: Array<{ path: string }> };
    };
    keyAlias.campaign.evidence = keyAlias.campaign.keypairs[0]?.path as string;
    writeFileSync(files5.operation, JSON.stringify(keyAlias));
    const aliasCalls: string[][] = [];
    expect(() => runFoundOperationV1(
      context(), { out: () => undefined, err: () => undefined }, files5.binary,
      files5.operation, files5.journal, null, false, successfulDependencies(aliasCalls),
    )).toThrow(/must not overwrite a signer or Market input file/);
    expect(aliasCalls).toHaveLength(0);
  });
});
