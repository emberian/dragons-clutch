import { createHash } from 'node:crypto';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';

import {
  parseWalletTerminalPayoutManifestV3,
  type WalletTerminalPayoutManifestV3,
} from '@dclutch/sdk/walletTerminalPayoutV3';
import { AddressLookupTableProgram, PublicKey, type TransactionInstruction } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import { EMPTY_SESSION, type CliContext } from '../src/context';
import {
  assertWalletTerminalPayoutMatchesPortfolioV3,
  produceWalletTerminalPayoutAltPlanV1,
  produceWalletTerminalPayoutManifestV3,
  type SuccessorSpawn,
} from '../src/commands/redeem';

const DEVNET = 'EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG';

function key(byte: number): string {
  return new PublicKey(new Uint8Array(32).fill(byte)).toBase58();
}

function identity(byte: number): string {
  return byte.toString(16).padStart(2, '0').repeat(32);
}

function manifestValue(): Record<string, unknown> {
  const market = key(1);
  const owner = key(2);
  const position = key(3);
  const claimsProgram = key(4);
  const custodyProgram = key(5);
  const collateralMint = key(6);
  const tokenProgram = key(7);
  const recipient = key(8);
  const routeFields = [
    'aggregate', 'linkedBasisRaw', 'linkedBasisStaging', 'productRaw', 'productStaging',
    'resultDomainRaw', 'resultDomainStaging', 'portfolioRaw', 'portfolioStaging',
    'market', 'activationCache', 'registryProgram', 'claimsProgram', 'claimsProgramData',
    'coreProgram', 'coreProgramData', 'position', 'exposureRaw', 'exposureStaging',
    'custodyProgram', 'terminalCoordinateRaw', 'terminalCoordinateStaging', 'realmRaw',
    'realmStaging', 'custodyReplay', 'collateralMint', 'hoard', 'recipient',
    'custodyAuthority', 'tokenProgram',
  ] as const;
  const route = Object.fromEntries(routeFields.map((field, index) => [field, key(20 + index)]));
  Object.assign(route, { market, position, claimsProgram, custodyProgram, collateralMint, tokenProgram, recipient });
  return {
    format: 'dclutch-wallet-terminal-payout-v3',
    route,
    custodyContext: identity(9),
    request: {
      releaseSet: identity(10), market, realm: identity(11), parentContext: identity(12),
      productRecordDigest: identity(13), exposureId: identity(14), exposureDigest: identity(15),
      terminalRecordDigest: identity(16), owner, position, recipientOwner: owner, recipient,
      claimsProgram, custodyProgram, collateralMint, tokenProgram, semanticBasisId: identity(17),
      linkedBasisRecordDigest: identity(18), generation: '1', expectedMarketRevision: '2',
      expectedPositionRevision: '3', expectedCustodyRevision: '4', quantity: '7',
      claimIndex: 1, transferIndex: 0,
    },
    signedPacketBase64: 'AA==',
    payout: '7',
    lookupTable: key(60),
  };
}

function context(): CliContext {
  return Object.freeze({
    rpcUrl: 'https://api.devnet.solana.com/',
    session: EMPTY_SESSION,
    flags: Object.freeze({ 'bootstrap-bin': '/mock/successor' }),
    json: true,
  });
}

function successfulSpawn(stdout: string, calls: Array<Readonly<{ binary: string; args: ReadonlyArray<string> }>>): SuccessorSpawn {
  return (binary, args) => {
    calls.push(Object.freeze({ binary, args: Object.freeze([...args]) }));
    return Object.freeze({ status: 0, signal: null, stdout, stderr: '' });
  };
}

function instruction(value: TransactionInstruction) {
  return {
    programId: value.programId.toBase58(),
    accounts: value.keys.map((meta) => ({ address: meta.pubkey.toBase58(), signer: meta.isSigner, writable: meta.isWritable })),
    dataBase64: Buffer.from(value.data).toString('base64'),
  };
}

describe('wallet terminal payout producer consumer', () => {
  it('invokes the distinct read-only ALT planner and pins its source file byte-for-byte', () => {
    const directory = mkdtempSync(resolve(tmpdir(), 'dclutch-alt-test-'));
    try {
      const owner = key(2);
      const source = {
        format: 'dclutch-wallet-terminal-payout-plan-input-v1', market: key(1), owner,
        recipientOwner: owner, recipient: key(3), collateralMint: key(4), tokenProgram: key(5),
        quantity: '7', claimIndex: 1, transferIndex: 0, parentContext: identity(6),
        custodyContext: identity(7), releaseSet: identity(8),
        programs: { registry: key(10), core: key(11), claims: key(12), custody: key(13) },
        records: {
          realm: identity(20), product: identity(21), resultDomain: identity(22), portfolio: identity(23),
          productBasis: identity(24), executionDescriptor: identity(25), compositionDescriptor: identity(26),
          compositionGraph: identity(27), compositionTranslation: identity(28), compositionExposure: identity(29),
          terminalRecord: identity(30),
        },
      };
      const sourceBytes = Buffer.from(JSON.stringify(source));
      const inputPath = resolve(directory, 'input.json');
      writeFileSync(inputPath, sourceBytes);
      const [create, lookupTable] = AddressLookupTableProgram.createLookupTable({
        authority: new PublicKey(owner), payer: new PublicKey(owner), recentSlot: 44,
      });
      const addresses = [key(40)];
      const extension = AddressLookupTableProgram.extendLookupTable({
        lookupTable, authority: new PublicKey(owner), payer: new PublicKey(owner),
        addresses: addresses.map((address) => new PublicKey(address)),
      });
      const output = {
        format: 'dclutch-wallet-terminal-payout-alt-plan-v1',
        sourceInputSha256: createHash('sha256').update(sourceBytes).digest('hex'), observationSlot: '44',
        payer: owner, authority: owner, lookupTable: lookupTable.toBase58(), addresses,
        create: instruction(create), extensions: [instruction(extension)],
        payoutInput: { ...source, lookupTable: lookupTable.toBase58() },
      };
      const calls: Array<Readonly<{ binary: string; args: ReadonlyArray<string> }>> = [];
      const produced = produceWalletTerminalPayoutAltPlanV1(
        context(), {}, inputPath, DEVNET, successfulSpawn(JSON.stringify(output), calls),
      );
      expect(produced.plan.lookupTable).toBe(lookupTable.toBase58());
      expect(calls[0]?.args).toEqual([
        'wallet-terminal-payout-alt-plan', '--rpc-url', 'https://api.devnet.solana.com/',
        '--i-mean-devnet', DEVNET, '--input', inputPath,
      ]);
    } finally {
      rmSync(directory, { recursive: true, force: true });
    }
  });

  it('invokes only the read-only successor command with the explicit RPC, devnet identity, and absolute input', () => {
    const calls: Array<Readonly<{ binary: string; args: ReadonlyArray<string> }>> = [];
    const expected = parseWalletTerminalPayoutManifestV3(JSON.stringify(manifestValue()));
    const observed = produceWalletTerminalPayoutManifestV3(
      context(),
      Object.freeze({}),
      'inputs/payout.json',
      DEVNET,
      successfulSpawn(`${JSON.stringify(expected)}\n`, calls),
    );
    expect(calls).toEqual([{
      binary: '/mock/successor',
      args: [
        'wallet-terminal-payout-plan',
        '--rpc-url', 'https://api.devnet.solana.com/',
        '--i-mean-devnet', DEVNET,
        '--input', resolve(process.cwd(), 'inputs/payout.json'),
      ],
    }]);
    expect(JSON.stringify(observed)).toBe(JSON.stringify(expected));
    expect(Object.keys(observed)).toEqual([
      'format', 'route', 'request', 'custodyContext', 'signedPacketBase64', 'payout', 'lookupTable',
    ]);
  });

  it('hostile-parses stdout and refuses unknown fields, process failure, and a missing cluster acknowledgment', () => {
    const hostile = { ...manifestValue(), silentlyIgnored: true };
    expect(() => produceWalletTerminalPayoutManifestV3(
      context(), {}, '/tmp/input.json', DEVNET,
      () => ({ status: 0, signal: null, stdout: JSON.stringify(hostile), stderr: '' }),
    )).toThrow(/missing or unknown fields/);
    expect(() => produceWalletTerminalPayoutManifestV3(
      context(), {}, '/tmp/input.json', DEVNET,
      () => ({ status: 2, signal: null, stdout: '', stderr: 'read-only cluster check refused' }),
    )).toThrow(/exited 2: read-only cluster check refused/);
    expect(() => produceWalletTerminalPayoutManifestV3(
      context(), {}, '/tmp/input.json', '',
      () => { throw new Error('spawn must not run'); },
    )).toThrow(/i-mean-devnet/);
    expect(() => produceWalletTerminalPayoutManifestV3(
      context(), {}, '/tmp/input.json', DEVNET,
      () => ({ status: 0, signal: null, stdout: `${' '.repeat(32_769)}{}`, stderr: '' }),
    )).toThrow(/character bound/);
  });

  it('refuses every portfolio-coordinate substitution after the manifest parser accepts it', () => {
    const manifest = parseWalletTerminalPayoutManifestV3(JSON.stringify(manifestValue()));
    const expected = Object.freeze({
      market: manifest.request.market,
      owner: manifest.request.owner,
      position: manifest.request.position,
      winningClaim: manifest.request.claimIndex,
      availableQuantity: manifest.request.quantity,
    });
    expect(() => assertWalletTerminalPayoutMatchesPortfolioV3(manifest, expected)).not.toThrow();

    const substituted = [
      { ...manifest, route: { ...manifest.route, market: key(70) } },
      { ...manifest, request: { ...manifest.request, owner: key(71) } },
      { ...manifest, request: { ...manifest.request, position: key(72) } },
      { ...manifest, request: { ...manifest.request, claimIndex: 2 } },
      { ...manifest, request: { ...manifest.request, quantity: '8' } },
    ] as ReadonlyArray<WalletTerminalPayoutManifestV3>;
    for (const candidate of substituted) {
      expect(() => assertWalletTerminalPayoutMatchesPortfolioV3(candidate, expected)).toThrow(/wallet payout manifest/);
    }
  });
});
