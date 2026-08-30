import { Keypair } from '@solana/web3.js';
import { describe, expect, it, vi } from 'vitest';

import {
  decodeAggregateRetirementCheckpointV1,
  inspectAggregateRetirementV1,
} from './aggregateRetirement';
import * as RetirementAbi from './generated/aggregateRetirementV1';
import {
  LIABILITY_BASIS_MARKET_BASIS_OFFSET,
  LIABILITY_BASIS_MARKET_CLAIM_COUNT_OFFSET,
  LIABILITY_BASIS_MARKET_CUSTODY_CONTEXT_OFFSET,
  LIABILITY_BASIS_MARKET_GENERATION_OFFSET,
  LIABILITY_BASIS_MARKET_HEADER_BYTES_V2,
  LIABILITY_BASIS_MARKET_LOGICAL_ID_OFFSET,
  LIABILITY_BASIS_MARKET_MAGIC_V2,
  LIABILITY_BASIS_MARKET_PRODUCT_OFFSET,
  LIABILITY_BASIS_MARKET_REALM_OFFSET,
  LIABILITY_BASIS_MARKET_REGISTRY_OFFSET,
  LIABILITY_BASIS_MARKET_RELEASE_SET_OFFSET,
  LIABILITY_BASIS_MARKET_REVISION_OFFSET,
  LIABILITY_BASIS_STATE_VERSION_V2,
} from './generated/coreFound';
import { deriveClaimsAggregateAddressV2 } from './marketCoreV2';
import { type RpcAccount } from './rpc';

const address = (seed: number) => Keypair.fromSeed(new Uint8Array(32).fill(seed)).publicKey.toBase58();
const CORE = address(1);
const CLAIMS = address(2);
const REGISTRY = address(3);
const MARKET = address(4);

function putU16(bytes: Uint8Array, offset: number, value: number): void {
  new DataView(bytes.buffer, bytes.byteOffset + offset, 2).setUint16(0, value, true);
}

function putU32(bytes: Uint8Array, offset: number, value: number): void {
  new DataView(bytes.buffer, bytes.byteOffset + offset, 4).setUint32(0, value, true);
}

function putU64(bytes: Uint8Array, offset: number, value: bigint): void {
  new DataView(bytes.buffer, bytes.byteOffset + offset, 8).setBigUint64(0, value, true);
}

function checkpoint(phase = 1, generation = 7n): Uint8Array {
  const bytes = new Uint8Array(RetirementAbi.AGGREGATE_RETIREMENT_CHECKPOINT_BYTES_V1);
  bytes.set(new TextEncoder().encode(RetirementAbi.AGGREGATE_RETIREMENT_CHECKPOINT_MAGIC_V1), RetirementAbi.AGGREGATE_RETIREMENT_MAGIC_OFFSET_V1);
  putU16(bytes, RetirementAbi.AGGREGATE_RETIREMENT_VERSION_OFFSET_V1, RetirementAbi.AGGREGATE_RETIREMENT_CHECKPOINT_VERSION_V1);
  bytes[RetirementAbi.AGGREGATE_RETIREMENT_PHASE_OFFSET_V1] = phase;
  bytes.fill(1, RetirementAbi.CORE_PRESTATE_OFFSET, RetirementAbi.CORE_PRESTATE_OFFSET + 32);
  bytes.fill(2, RetirementAbi.BUNDLE_DIGEST_OFFSET, RetirementAbi.BUNDLE_DIGEST_OFFSET + 32);
  bytes.fill(3, RetirementAbi.CLAIMS_CONTEXT_OFFSET, RetirementAbi.CLAIMS_CONTEXT_OFFSET + 32);
  bytes.fill(4, RetirementAbi.CLAIMS_RECEIPT_OFFSET, RetirementAbi.CLAIMS_RECEIPT_OFFSET + 32);
  if (phase >= 2) bytes.fill(5, RetirementAbi.VAULT_RECEIPT_OFFSET, RetirementAbi.VAULT_RECEIPT_OFFSET + 32);
  if (phase >= 3) bytes.fill(6, RetirementAbi.REPLAY_RECEIPT_OFFSET, RetirementAbi.REPLAY_RECEIPT_OFFSET + 32);
  putU64(bytes, RetirementAbi.CLAIMS_REFUND_OFFSET, 91n);
  putU64(bytes, RetirementAbi.CUSTODY_REFUND_OFFSET, phase >= 2 ? BigInt(phase) * 17n : 0n);
  putU64(bytes, RetirementAbi.GENERATION_OFFSET, generation);
  putU64(bytes, RetirementAbi.CLAIMS_REVISION_OFFSET, 12n);
  putU64(bytes, RetirementAbi.CUSTODY_REVISION_OFFSET, 20n + BigInt(phase));
  putU64(bytes, RetirementAbi.PHASE_REVISION_OFFSET, BigInt(phase));
  return bytes;
}

function aggregate(supplies: readonly bigint[]): Uint8Array {
  const bytes = new Uint8Array(LIABILITY_BASIS_MARKET_HEADER_BYTES_V2 + supplies.length * 8);
  bytes.set(LIABILITY_BASIS_MARKET_MAGIC_V2, 0);
  putU16(bytes, 8, LIABILITY_BASIS_STATE_VERSION_V2);
  putU32(bytes, LIABILITY_BASIS_MARKET_CLAIM_COUNT_OFFSET, supplies.length);
  putU64(bytes, LIABILITY_BASIS_MARKET_REVISION_OFFSET, 12n);
  bytes.set(Keypair.fromSeed(new Uint8Array(32).fill(4)).publicKey.toBytes(), LIABILITY_BASIS_MARKET_LOGICAL_ID_OFFSET);
  bytes.fill(7, LIABILITY_BASIS_MARKET_RELEASE_SET_OFFSET, LIABILITY_BASIS_MARKET_RELEASE_SET_OFFSET + 32);
  bytes.set(Keypair.fromSeed(new Uint8Array(32).fill(3)).publicKey.toBytes(), LIABILITY_BASIS_MARKET_REGISTRY_OFFSET);
  bytes.fill(8, LIABILITY_BASIS_MARKET_PRODUCT_OFFSET, LIABILITY_BASIS_MARKET_PRODUCT_OFFSET + 32);
  bytes.fill(9, LIABILITY_BASIS_MARKET_BASIS_OFFSET, LIABILITY_BASIS_MARKET_BASIS_OFFSET + 32);
  bytes.fill(10, LIABILITY_BASIS_MARKET_REALM_OFFSET, LIABILITY_BASIS_MARKET_REALM_OFFSET + 32);
  bytes.fill(11, LIABILITY_BASIS_MARKET_CUSTODY_CONTEXT_OFFSET, LIABILITY_BASIS_MARKET_CUSTODY_CONTEXT_OFFSET + 32);
  putU64(bytes, LIABILITY_BASIS_MARKET_GENERATION_OFFSET, 7n);
  supplies.forEach((supply, index) => putU64(bytes, LIABILITY_BASIS_MARKET_HEADER_BYTES_V2 + index * 8, supply));
  return bytes;
}

function account(owner: string, data: Uint8Array): RpcAccount {
  return Object.freeze({ owner, data, executable: false, lamports: '91', space: data.length });
}

function client(value: RpcAccount | null, slot = '101') {
  const aggregateAddress = deriveClaimsAggregateAddressV2(CLAIMS, MARKET);
  return Object.freeze({
    multipleAccounts: vi.fn(async () => Object.freeze({
      slot,
      accounts: Object.freeze([Object.freeze({ address: aggregateAddress, account: value })]),
    })),
  });
}

const request = Object.freeze({
  coreProgramId: CORE,
  claimsProgramId: CLAIMS,
  marketAddress: MARKET,
  marketPhase: 'Retiring' as const,
  marketGeneration: '7',
  minimumContextSlot: '100',
});

describe('generated aggregate-retirement checkpoint decoder', () => {
  it('decodes the exhaustive ordered checkpoint phases and their active effects', () => {
    expect(decodeAggregateRetirementCheckpointV1(checkpoint(1))).toMatchObject({ phase: 'ClaimsClosed', closeVaultReceiptDigest: null, closeReplayReceiptDigest: null, custodyRefundLamports: '0', phaseRevision: '1' });
    expect(decodeAggregateRetirementCheckpointV1(checkpoint(2))).toMatchObject({ phase: 'HoardVaultClosed', closeVaultReceiptDigest: '05'.repeat(32), closeReplayReceiptDigest: null, phaseRevision: '2' });
    expect(decodeAggregateRetirementCheckpointV1(checkpoint(3))).toMatchObject({ phase: 'CustodyReplayClosed', closeVaultReceiptDigest: '05'.repeat(32), closeReplayReceiptDigest: '06'.repeat(32), phaseRevision: '3' });
  });

  it('refuses width, header, reserved bytes, phase skips, zero identities, and inactive effects', () => {
    const hostiles: Uint8Array[] = [];
    hostiles.push(checkpoint().slice(1));
    const magic = checkpoint(); magic[0] ^= 1; hostiles.push(magic);
    const reserved = checkpoint(); reserved[RetirementAbi.AGGREGATE_RETIREMENT_RESERVED_OFFSET_V1] = 1; hostiles.push(reserved);
    const phase = checkpoint(); phase[RetirementAbi.AGGREGATE_RETIREMENT_PHASE_OFFSET_V1] = 4; putU64(phase, RetirementAbi.PHASE_REVISION_OFFSET, 4n); hostiles.push(phase);
    const zero = checkpoint(); zero.fill(0, RetirementAbi.BUNDLE_DIGEST_OFFSET, RetirementAbi.BUNDLE_DIGEST_OFFSET + 32); hostiles.push(zero);
    const skip = checkpoint(); putU64(skip, RetirementAbi.PHASE_REVISION_OFFSET, 2n); hostiles.push(skip);
    const inactive = checkpoint(); inactive[RetirementAbi.VAULT_RECEIPT_OFFSET] = 1; hostiles.push(inactive);
    const missing = checkpoint(2); missing.fill(0, RetirementAbi.VAULT_RECEIPT_OFFSET, RetirementAbi.VAULT_RECEIPT_OFFSET + 32); hostiles.push(missing);
    for (const bytes of hostiles) expect(() => decodeAggregateRetirementCheckpointV1(bytes)).toThrow();
  });
});

describe('cold injected aggregate-retirement inspection', () => {
  it('reads a Core-owned checkpoint at or above the Market floor and names only the next durable step', async () => {
    const injected = client(account(CORE, checkpoint(2)));
    await expect(inspectAggregateRetirementV1(injected, request)).resolves.toMatchObject({
      status: 'in-progress',
      observedSlot: '101',
      nextStep: 'close-replay',
      browserAction: 'disabled',
      checkpoint: { phase: 'HoardVaultClosed', generation: '7' },
    });
    expect(injected.multipleAccounts).toHaveBeenCalledWith([deriveClaimsAggregateAddressV2(CLAIMS, MARKET)], '100');
  });

  it('distinguishes zero Claims liabilities from whole-route readiness', async () => {
    await expect(inspectAggregateRetirementV1(client(account(CLAIMS, aggregate([0n, 0n]))), request)).resolves.toMatchObject({
      status: 'operator-required', nextStep: 'prepare', nonzeroClaimCount: 0, browserAction: 'disabled',
    });
    await expect(inspectAggregateRetirementV1(client(account(CLAIMS, aggregate([2n, 0n]))), request)).resolves.toMatchObject({
      status: 'blocked-liabilities', nextStep: 'none', nonzeroClaimCount: 1, browserAction: 'disabled',
    });
  });

  it('does not read a retirement account before the Market reaches Retiring', async () => {
    const injected = client(null);
    await expect(inspectAggregateRetirementV1(injected, { ...request, marketPhase: 'Terminal' })).resolves.toMatchObject({
      status: 'not-admitted', nextStep: 'none', browserAction: 'disabled',
    });
    expect(injected.multipleAccounts).not.toHaveBeenCalled();
  });

  it('refuses absent, foreign, malformed, cross-generation, and regressed cold observations', async () => {
    await expect(inspectAggregateRetirementV1(client(null), request)).resolves.toMatchObject({ status: 'refused' });
    await expect(inspectAggregateRetirementV1(client(account(REGISTRY, checkpoint())), request)).resolves.toMatchObject({ status: 'refused' });
    const malformed = checkpoint(); malformed[RetirementAbi.AGGREGATE_RETIREMENT_RESERVED_OFFSET_V1] = 1;
    await expect(inspectAggregateRetirementV1(client(account(CORE, malformed)), request)).resolves.toMatchObject({ status: 'refused' });
    await expect(inspectAggregateRetirementV1(client(account(CORE, checkpoint(1, 8n))), request)).resolves.toMatchObject({ status: 'refused' });
    await expect(inspectAggregateRetirementV1(client({ ...account(CORE, checkpoint()), lamports: '90' }), request)).resolves.toMatchObject({ status: 'refused' });
    await expect(inspectAggregateRetirementV1(client(account(CORE, checkpoint()), '99'), request)).rejects.toThrow('regressed below the Market floor');
  });
});
