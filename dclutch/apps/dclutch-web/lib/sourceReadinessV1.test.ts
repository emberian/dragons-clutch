import { PublicKey, VersionedTransaction } from '@solana/web3.js';
import { describe, expect, it, vi } from 'vitest';

import {
  SOURCE_READINESS_MARKET_FORMAT_V1,
  SOURCE_READINESS_PLAN_FORMAT_V1,
  SOURCE_READINESS_RECORDS_FORMAT_V1,
  SOURCE_READINESS_SNAPSHOT_FORMAT_V1,
  SOURCE_READINESS_SOURCE_FORMAT_V1,
} from '@dclutch/sdk/generated/sourceReadinessWasmV1';
import {
  acquireSourceReadinessV1,
  buildSourceReadinessTransactionV1,
  parseSourceReadinessPlanV1,
  type SourceReadinessPlanV1,
  type SourceReadinessRouteWasmV1,
} from './sourceReadinessV1';

const address = (byte: number) => new PublicKey(new Uint8Array(32).fill(byte)).toBase58();
function plan(route: SourceReadinessPlanV1['route'] = 'create'): Record<string, unknown> {
  const executable = route === 'create' || route === 'activate' || route === 'accept';
  return {
    format: SOURCE_READINESS_PLAN_FORMAT_V1,
    route,
    observedSlot: '11',
    instruction: executable ? {
      program: address(3),
      accounts: [{ address: address(4), isSigner: false, isWritable: true }],
      dataBase64: 'AQIDBA==',
    } : null,
    prepay: route === 'create' ? { destination: address(5), lamports: '0' } : null,
    accounts: executable ? { protocolWritable: [address(4)], completion: [address(4)] } : null,
    geometry: executable ? {
      protocolAccountCount: 1,
      protocolUniqueAccountCount: 2,
      protocolWritableCount: 1,
      protocolSignerCount: 0,
      protocolDataLen: 4,
      transactionInstructionCountWithoutComputeBudget: 1,
      transactionLockCountWithoutPayer: 2,
    } : null,
    facts: { callerAuthority: address(6) },
  };
}

function fixture(slotMismatch = false) {
  const market = address(1);
  const core = address(2);
  const registry = address(3);
  const resolution = address(4);
  const sourceRaw = address(5);
  const sourceStaging = address(6);
  const manifestRaw = address(7);
  const manifestStaging = address(8);
  const frame = {
    coordinates: {
      market,
      sourceMaterial: { raw: sourceRaw, staging: sourceStaging },
      capabilityManifest: { raw: manifestRaw, staging: manifestStaging },
      recoveryPolicy: null,
      sourceState: address(9),
      fundingLedger: address(10),
      beneficiary: address(11),
      activationReceipt: address(12),
    },
    activationCache: address(13),
    registryProgram: registry,
    coreProgram: core,
    coreProgramdata: address(14),
    resolutionProgram: resolution,
    resolutionProgramdata: address(15),
  };
  const addresses = [
    market, frame.activationCache, registry, core, frame.coreProgramdata, resolution,
    frame.resolutionProgramdata, sourceRaw, sourceStaging, manifestRaw, manifestStaging,
    frame.coordinates.sourceState, frame.coordinates.fundingLedger, frame.coordinates.beneficiary,
    frame.coordinates.activationReceipt, address(16), address(17), address(18),
  ];
  const account = (owner: string, data = Uint8Array.of(1)) => Object.freeze({
    owner, executable: false, lamports: '1', data, space: data.length,
  });
  // FOUR exports, annotated as the four this route calls -- not as the whole
  // eleven-export module. The old annotation made this stub claim a boundary
  // it covered a third of.
  const wasm: SourceReadinessRouteWasmV1 = Object.freeze({
    derive_source_readiness_base_v1(source: string) {
      const value = JSON.parse(source) as Record<string, unknown>;
      expect(value.format).toBe(SOURCE_READINESS_MARKET_FORMAT_V1);
      return JSON.stringify({
        activationCache: frame.activationCache,
        coreProgramdata: frame.coreProgramdata,
        resolutionProgramdata: frame.resolutionProgramdata,
        sourceMaterial: sourceRaw,
        sourceMaterialStaging: sourceStaging,
        capabilityManifest: manifestRaw,
        capabilityManifestStaging: manifestStaging,
        sourceState: frame.coordinates.sourceState,
        activationReceipt: frame.coordinates.activationReceipt,
        beneficiary: frame.coordinates.beneficiary,
        generation: '1',
      });
    },
    derive_source_readiness_recovery_v1(source: string) {
      expect((JSON.parse(source) as Record<string, unknown>).format).toBe(SOURCE_READINESS_SOURCE_FORMAT_V1);
      return JSON.stringify({ recoveryPolicy: null, recoveryPolicyStaging: null });
    },
    derive_source_readiness_detail_v1(source: string) {
      expect((JSON.parse(source) as Record<string, unknown>).format).toBe(SOURCE_READINESS_RECORDS_FORMAT_V1);
      return JSON.stringify({
        recoveryPolicy: null,
        recoveryPolicyStaging: null,
        fundingLedger: frame.coordinates.fundingLedger,
        fundingEntryIndices: [0, 2, 1],
        frame,
        addresses,
      });
    },
    plan_source_readiness_v1(source: string) {
      const value = JSON.parse(source) as Record<string, unknown>;
      expect(value.format).toBe(SOURCE_READINESS_SNAPSHOT_FORMAT_V1);
      expect((value.accounts as Array<Record<string, unknown>>).map((entry) => entry.address)).toEqual(addresses);
      return JSON.stringify(plan());
    },
  });
  let finalizedReads = 0;
  const rpc = {
    finalizedSlot: vi.fn(async () => (++finalizedReads === 1 ? '10' : '11')),
    blockTime: vi.fn(async () => '1700000000'),
    accountInfo: vi.fn(async (candidate: string) => {
      if (candidate === market) return { slot: '10', account: account(core, Uint8Array.of(9)) };
      const slot = slotMismatch && candidate === frame.resolutionProgramdata ? '12' : '11';
      return { slot, account: account(address(99), Uint8Array.of(8)) };
    }),
    multipleAccounts: vi.fn(async (candidates: ReadonlyArray<string>) => {
      if (candidates.length === 4) {
        return {
          slot: '10',
          accounts: candidates.map((candidate) => ({
            address: candidate,
            account: candidate === sourceStaging || candidate === manifestStaging ? null : account(registry, Uint8Array.of(7)),
          })),
        };
      }
      return {
        slot: '11',
        accounts: candidates.map((candidate) => ({ address: candidate, account: account(address(99)) })),
      };
    }),
  };
  return { market, core, registry, resolution, addresses, wasm, rpc };
}

describe('Source readiness browser crossing', () => {
  it('hostile-decodes the Rust plan and refuses unknown fields, signer drift, and malformed routes', () => {
    expect(parseSourceReadinessPlanV1(JSON.stringify(plan())).route).toBe('create');
    expect(() => parseSourceReadinessPlanV1(JSON.stringify({ ...plan(), extra: true }))).toThrow(/unknown fields/);
    const signer = plan();
    (signer.instruction as { accounts: Array<{ isSigner: boolean }> }).accounts[0]!.isSigner = true;
    expect(() => parseSourceReadinessPlanV1(JSON.stringify(signer))).toThrow(/unexpectedly require a signer/);
    expect(() => parseSourceReadinessPlanV1(JSON.stringify({ ...plan('complete'), instruction: plan().instruction })))
      .toThrow(/disagrees with its executable/);
  });

  it('preserves Rust-derived address order across the split exact-slot RPC read', async () => {
    const value = fixture();
    const acquired = await acquireSourceReadinessV1(value.rpc, value.wasm, value.market, {
      coreProgram: value.core, registryProgram: value.registry, resolutionProgram: value.resolution,
    });
    expect(acquired.observationAddresses).toEqual(value.addresses);
    expect(acquired.plan.route).toBe('create');
    expect(value.rpc.accountInfo).toHaveBeenCalledWith(address(14), '11');
    expect(value.rpc.accountInfo).toHaveBeenCalledWith(address(15), '11');
  });

  it('refuses to relabel two ProgramData reads and ordinary accounts as one observation', async () => {
    const value = fixture(true);
    await expect(acquireSourceReadinessV1(value.rpc, value.wasm, value.market, {
      coreProgram: value.core, registryProgram: value.registry, resolutionProgram: value.resolution,
    })).rejects.toThrow(/slot advanced during the split ELF read/);
  });

  it('compiles only a sole-payer packet and refuses terminal routes', () => {
    const payer = address(20);
    const acquisition = Object.freeze({
      plan: parseSourceReadinessPlanV1(JSON.stringify(plan())),
      planJson: JSON.stringify(plan()), snapshotJson: '{}', observationAddresses: Object.freeze([]),
    });
    const built = buildSourceReadinessTransactionV1(acquisition, payer, {
      slot: '11', blockhash: address(21), lastValidBlockHeight: '99',
    });
    const decoded = VersionedTransaction.deserialize(built.wireBytes);
    expect(decoded.message.header.numRequiredSignatures).toBe(1);
    expect(decoded.message.staticAccountKeys[0]?.toBase58()).toBe(payer);
    const terminal = { ...acquisition, plan: parseSourceReadinessPlanV1(JSON.stringify(plan('complete'))) };
    expect(() => buildSourceReadinessTransactionV1(terminal, payer, {
      slot: '11', blockhash: address(21), lastValidBlockHeight: '99',
    })).toThrow(/has no wallet act/);
  });
});
