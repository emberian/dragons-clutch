import { Keypair, PublicKey } from '@solana/web3.js';
import { describe, expect, it, vi } from 'vitest';

import { SYSTEM_PROGRAM_ID } from './releaseRegistry';
import { type RpcAccount } from './rpc';
import {
  DIRECT_MAKER_REPLAY_BYTES_V1,
  deriveDirectMakerReplayAddressV1,
  inspectDirectMakerNonceV1,
  requireAuthenticatedDirectMakerNonceV1,
} from './directMakerReplay';

const TRADING = Keypair.fromSeed(new Uint8Array(32).fill(71)).publicKey.toBase58();
const MARKET = Keypair.fromSeed(new Uint8Array(32).fill(72)).publicKey.toBase58();
const MAKER = Keypair.fromSeed(new Uint8Array(32).fill(73)).publicKey.toBase58();
const FOREIGN = Keypair.fromSeed(new Uint8Array(32).fill(74)).publicKey.toBase58();
const RENT_OWNER = Keypair.fromSeed(new Uint8Array(32).fill(75)).publicKey.toBase58();
const GENERATION = 4n;
const U64_MAX = 0xffff_ffff_ffff_ffffn;

function putU16(bytes: Uint8Array, offset: number, value: number): void {
  new DataView(bytes.buffer).setUint16(offset, value, true);
}

function putU64(bytes: Uint8Array, offset: number, value: bigint): void {
  new DataView(bytes.buffer).setBigUint64(offset, value, true);
}

function replayAccount(overrides?: Readonly<{
  owner?: string;
  executable?: boolean;
  space?: number;
  lamports?: string;
  bytes?: (bytes: Uint8Array) => void;
}>): RpcAccount {
  const { bump } = deriveDirectMakerReplayAddressV1(TRADING, MARKET, GENERATION, MAKER);
  const data = new Uint8Array(DIRECT_MAKER_REPLAY_BYTES_V1);
  data.set(new TextEncoder().encode('DCLTDMR1'), 0);
  putU16(data, 8, 1);
  data[10] = bump;
  data.set(new PublicKey(MARKET).toBytes(), 16);
  putU64(data, 48, GENERATION);
  data.set(new PublicKey(MAKER).toBytes(), 56);
  putU64(data, 88, 9n);
  putU64(data, 96, 2n);
  putU64(data, 104, 5n);
  data.set(new PublicKey(RENT_OWNER).toBytes(), 112);
  putU64(data, 144, 2_000_000n);
  overrides?.bytes?.(data);
  return Object.freeze({
    data,
    executable: overrides?.executable ?? false,
    lamports: overrides?.lamports ?? '2000000',
    owner: overrides?.owner ?? TRADING,
    space: overrides?.space ?? data.length,
  });
}

function client(account: RpcAccount | null, observedSlot = '501') {
  return {
    finalizedSlot: vi.fn(async () => '500'),
    accountInfo: vi.fn(async () => Object.freeze({ slot: observedSlot, account })),
  };
}

describe('the Direct maker replay nonce reader', () => {
  it('derives one canonical PDA at one finalized floor and projects first use as nonce zero', async () => {
    const rpc = client(Object.freeze({
      data: new Uint8Array(0), executable: false, lamports: '37', owner: SYSTEM_PROGRAM_ID, space: 0,
    }));
    const observed = await inspectDirectMakerNonceV1(rpc, {
      tradingProgram: TRADING, market: MARKET, generation: GENERATION, maker: MAKER,
    });
    const derived = deriveDirectMakerReplayAddressV1(TRADING, MARKET, GENERATION, MAKER);
    expect(observed).toMatchObject({ address: derived.address, observedSlot: '501', nextNonce: 0n, state: 'vacant' });
    expect(rpc.finalizedSlot).toHaveBeenCalledOnce();
    expect(rpc.accountInfo).toHaveBeenCalledWith(derived.address, '500');
    expect(requireAuthenticatedDirectMakerNonceV1(observed, {
      tradingProgram: TRADING, market: MARKET, generation: GENERATION, maker: MAKER,
    })).toBe(0n);
  });

  it('accepts an absent PDA as the same canonical first-use state', async () => {
    const observed = await inspectDirectMakerNonceV1(client(null), {
      tradingProgram: TRADING, market: MARKET, generation: GENERATION, maker: MAKER,
    });
    expect(observed.state).toBe('vacant');
    expect(observed.nextNonce).toBe(0n);
  });

  it('hostile-decodes a progressed Trading-owned root and returns its exact next nonce', async () => {
    const observed = await inspectDirectMakerNonceV1(client(replayAccount()), {
      tradingProgram: TRADING, market: MARKET, generation: GENERATION, maker: MAKER,
    });
    expect(observed).toMatchObject({ state: 'existing', nextNonce: 9n, market: MARKET, generation: GENERATION, maker: MAKER });
  });

  it('refuses saturation and a finalized context regression', async () => {
    await expect(inspectDirectMakerNonceV1(client(replayAccount({ bytes: (bytes) => putU64(bytes, 88, U64_MAX) })), {
      tradingProgram: TRADING, market: MARKET, generation: GENERATION, maker: MAKER,
    })).rejects.toThrow('saturated');
    await expect(inspectDirectMakerNonceV1(client(replayAccount(), '499'), {
      tradingProgram: TRADING, market: MARKET, generation: GENERATION, maker: MAKER,
    })).rejects.toThrow('regressed below');
  });

  it('refuses owner, executable, space, bump, reserved-byte, and balance substitutions', async () => {
    const request = { tradingProgram: TRADING, market: MARKET, generation: GENERATION, maker: MAKER } as const;
    await expect(inspectDirectMakerNonceV1(client(replayAccount({ owner: FOREIGN })), request)).rejects.toThrow('not owned');
    await expect(inspectDirectMakerNonceV1(client(replayAccount({ executable: true })), request)).rejects.toThrow('executable');
    await expect(inspectDirectMakerNonceV1(client(replayAccount({ space: DIRECT_MAKER_REPLAY_BYTES_V1 + 1 })), request)).rejects.toThrow('exactly 152');
    await expect(inspectDirectMakerNonceV1(client(replayAccount({ bytes: (bytes) => { bytes[0] ^= 1; } })), request)).rejects.toThrow('magic or version');
    await expect(inspectDirectMakerNonceV1(client(replayAccount({ bytes: (bytes) => putU16(bytes, 8, 2) })), request)).rejects.toThrow('magic or version');
    await expect(inspectDirectMakerNonceV1(client(replayAccount({ bytes: (bytes) => { bytes[10] ^= 1; } })), request)).rejects.toThrow('PDA bump');
    await expect(inspectDirectMakerNonceV1(client(replayAccount({ bytes: (bytes) => { bytes[11] = 1; } })), request)).rejects.toThrow('reserved');
    await expect(inspectDirectMakerNonceV1(client(replayAccount({ lamports: '-1' })), request)).rejects.toThrow('canonical unsigned');
    await expect(inspectDirectMakerNonceV1(client(Object.freeze({
      data: Uint8Array.of(1), executable: false, lamports: '0', owner: SYSTEM_PROGRAM_ID, space: 1,
    })), request)).rejects.toThrow('data-free System-owned');
  });

  it('refuses Market, generation, maker, rent, and replay-invariant substitutions', async () => {
    const request = { tradingProgram: TRADING, market: MARKET, generation: GENERATION, maker: MAKER } as const;
    await expect(inspectDirectMakerNonceV1(client(replayAccount({ bytes: (bytes) => bytes.set(new PublicKey(FOREIGN).toBytes(), 16) })), request)).rejects.toThrow('substitutes');
    await expect(inspectDirectMakerNonceV1(client(replayAccount({ bytes: (bytes) => putU64(bytes, 48, GENERATION + 1n) })), request)).rejects.toThrow('substitutes');
    await expect(inspectDirectMakerNonceV1(client(replayAccount({ bytes: (bytes) => bytes.set(new PublicKey(FOREIGN).toBytes(), 56) })), request)).rejects.toThrow('substitutes');
    await expect(inspectDirectMakerNonceV1(client(replayAccount({ bytes: (bytes) => bytes.fill(0, 112, 144) })), request)).rejects.toThrow('RentCredit');
    await expect(inspectDirectMakerNonceV1(client(replayAccount({ bytes: (bytes) => putU64(bytes, 144, 0n) })), request)).rejects.toThrow('rent principal');
    await expect(inspectDirectMakerNonceV1(client(replayAccount({ bytes: (bytes) => putU64(bytes, 96, 10n) })), request)).rejects.toThrow('live count');
    await expect(inspectDirectMakerNonceV1(client(replayAccount({ bytes: (bytes) => putU64(bytes, 104, 10n) })), request)).rejects.toThrow('minimum-live');
  });

  it('refuses alias coordinates and cannot bind one authenticated observation to another crossing', async () => {
    expect(() => deriveDirectMakerReplayAddressV1(TRADING, MARKET, GENERATION, MARKET)).toThrow('must not alias');
    const observed = await inspectDirectMakerNonceV1(client(replayAccount()), {
      tradingProgram: TRADING, market: MARKET, generation: GENERATION, maker: MAKER,
    });
    expect(() => requireAuthenticatedDirectMakerNonceV1(observed, {
      tradingProgram: TRADING, market: MARKET, generation: GENERATION, maker: FOREIGN,
    })).toThrow('belongs to another');
    expect(() => requireAuthenticatedDirectMakerNonceV1({ ...observed } as typeof observed, {
      market: MARKET, generation: GENERATION, maker: MAKER,
    })).toThrow('not acquired');
  });
});
