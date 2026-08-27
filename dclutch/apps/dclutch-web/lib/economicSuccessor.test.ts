import { PublicKey } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import {
  ECONOMIC_FOUNDING_BYTES,
  ECONOMIC_OPERATION_BYTES,
  ECONOMIC_PROJECTION_BYTES,
  buildEconomicOperationTransaction,
  decodeEconomicProjectionV1,
  decodeExecutionReleaseSetV1,
  encodeEconomicFoundingV1,
  encodeEconomicOperationV1,
  simulateEconomicOperationV1,
  type EconomicProjectionObservationV1,
} from './economicSuccessor';
import { LEGACY_TOKEN_PROGRAM_ID } from './registeredDirect';

function key(byte: number): PublicKey {
  return new PublicKey(new Uint8Array(32).fill(byte));
}

function putU64(bytes: Uint8Array, offset: number, value: bigint): void {
  new DataView(bytes.buffer, bytes.byteOffset + offset, 8).setBigUint64(0, value, true);
}

function projectionBytes(): Uint8Array {
  const bytes = new Uint8Array(ECONOMIC_PROJECTION_BYTES);
  bytes.set(new TextEncoder().encode('DCLTECO1'));
  bytes[8] = 1;
  for (const [index, value] of [8, 9, 5, 6, 7, 10].entries()) bytes.fill(value, 16 + index * 32, 48 + index * 32);
  const state = new Uint8Array(16 + 3 * 7 * 8);
  state.set(new TextEncoder().encode('DCES'));
  state[4] = 1;
  state[6] = 3;
  new DataView(bytes.buffer).setUint16(216, state.length, true);
  bytes.set(state, 224);
  return bytes;
}

function releaseAccount(economicProgram = key(2)) {
  const data = new Uint8Array(336);
  data.set(new TextEncoder().encode('DCLTRLS1'));
  new DataView(data.buffer).setUint16(8, 1, true);
  new DataView(data.buffer).setUint16(10, 1, true);
  const programs = [key(1), economicProgram, key(3), key(4), economicProgram];
  const releases = [11, 12, 13, 14, 12];
  programs.forEach((program, index) => {
    data.set(program.toBytes(), 16 + index * 64);
    data.fill(releases[index], 48 + index * 64, 80 + index * 64);
  });
  return Object.freeze({ data, executable: false, lamports: '1', owner: key(1).toBase58(), space: data.length });
}

describe('physical economic successor browser boundary', () => {
  it('strictly decodes the fixed projection and simulates exact split effects', () => {
    const projection = decodeEconomicProjectionV1(projectionBytes());
    const operation = { action: 'split', holder: 'source', representation: 'native', outcome: 0, quantity: 10n, expectedRevision: 0n } as const;
    const simulation = simulateEconomicOperationV1(projection, operation);
    expect(simulation.nextState.hoard).toBe(10n);
    expect(simulation.nextState.supply).toEqual([10n, 10n, 10n]);
    expect(simulation.nextState.nativeSupply).toEqual([10n, 10n, 10n]);
    expect(simulation.nextState.sourceNative).toEqual([10n, 10n, 10n]);
    expect(simulation.claims).toEqual([
      { operation: 'credit', holder: 'source', outcome: 0, amount: 10n },
      { operation: 'credit', holder: 'source', outcome: 1, amount: 10n },
      { operation: 'credit', holder: 'source', outcome: 2, amount: 10n },
    ]);
    expect(simulation.custody).toEqual({ source: 'source', destination: 'hoard', amount: 10n });
  });

  it('decodes release role authority and refuses inconsistent aliases', async () => {
    const release = await decodeExecutionReleaseSetV1(releaseAccount());
    expect(release.roles.core.program).toBe(key(1).toBase58());
    expect(release.roles.claims).toEqual(release.roles.custody);
    const hostile = releaseAccount();
    hostile.data.fill(99, 48 + 4 * 64, 80 + 4 * 64);
    await expect(decodeExecutionReleaseSetV1(hostile)).rejects.toThrow(/inconsistent role pairs/);
  });

  it('encodes exact founding and operation layouts without implicit account creation', () => {
    const founding = encodeEconomicFoundingV1({
      marketId: new Uint8Array(32).fill(8), releaseSetId: new Uint8Array(32).fill(9),
      sourceHolder: key(5).toBase58(), destinationHolder: key(6).toBase58(), collateralMint: key(7).toBase58(),
      hoardAccount: key(10).toBase58(), outcomeCount: 3,
    });
    expect(founding).toHaveLength(ECONOMIC_FOUNDING_BYTES);
    expect(new TextDecoder().decode(founding.slice(0, 8))).toBe('DCLTECI1');
    expect(founding[9]).toBe(0);
    expect(founding[13]).toBe(3);
    const operation = encodeEconomicOperationV1({ action: 'materialize', holder: 'destination', representation: 'materialized', outcome: 2, quantity: 4n, expectedRevision: 7n });
    expect(operation).toHaveLength(ECONOMIC_OPERATION_BYTES);
    expect([...operation.slice(9, 13)]).toEqual([4, 0, 0, 2]);
    expect(new DataView(operation.buffer).getBigUint64(16, true)).toBe(4n);
    expect(new DataView(operation.buffer).getBigUint64(24, true)).toBe(7n);
    expect(() => encodeEconomicOperationV1({ action: 'redeem', holder: 'source', representation: 'native', outcome: 1.5, quantity: 1n, expectedRevision: 0n })).toThrow(/not a byte/);
  });

  it('builds the exact custody frame and refuses stale or underfunded state', () => {
    const program = key(2);
    const projection = decodeEconomicProjectionV1(projectionBytes());
    const observation: EconomicProjectionObservationV1 = Object.freeze({ status: 'founded', address: key(20).toBase58(), observedSlot: '5', lamports: '1', projection });
    const plan = buildEconomicOperationTransaction({
      economicProgram: program.toBase58(), payer: key(90).toBase58(), recentBlockhash: key(91).toBase58(),
      authority: key(3).toBase58(), projection: observation, releaseSet: key(21).toBase58(),
      operation: { action: 'split', holder: 'source', representation: 'native', outcome: 0, quantity: 10n, expectedRevision: 0n },
      holderToken: key(22).toBase58(),
    });
    expect(plan.instruction.keys).toHaveLength(9);
    expect(plan.instruction.keys[3]).toMatchObject({ pubkey: key(5), isSigner: true, isWritable: false });
    expect(plan.instruction.keys[8].pubkey).toEqual(LEGACY_TOKEN_PROGRAM_ID);
    expect(plan.requiredSignerKeys).toEqual([key(90).toBase58(), key(3).toBase58(), key(5).toBase58()]);
    expect(plan.wireBytes.length).toBeLessThanOrEqual(1_232);
    expect(() => simulateEconomicOperationV1(projection, { action: 'merge', holder: 'source', representation: 'native', outcome: 0, quantity: 1n, expectedRevision: 0n })).toThrow(/insufficient balance/);
    expect(() => simulateEconomicOperationV1(projection, { action: 'split', holder: 'source', representation: 'native', outcome: 0, quantity: 1n, expectedRevision: 1n })).toThrow(/revision is stale/);
  });

  it('refuses malformed state padding and supply partitions', () => {
    const trailing = projectionBytes();
    trailing[ECONOMIC_PROJECTION_BYTES - 1] = 1;
    expect(() => decodeEconomicProjectionV1(trailing)).toThrow(/inactive capacity/);
    const partition = projectionBytes();
    putU64(partition, 224 + 16, 10n);
    expect(() => decodeEconomicProjectionV1(partition)).toThrow(/does not partition/);
  });
});
