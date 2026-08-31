import { Keypair, PublicKey } from '@solana/web3.js';
import { describe, expect, it, vi } from 'vitest';

import { SYSTEM_PROGRAM_ID } from './releaseRegistry';
import { type RpcAccount } from './rpc';
import {
  DIRECT_MAKER_REPLAY_BYTES_V1,
  DIRECT_MAKER_REPLAY_LEGACY_BYTES_V1,
  deriveDirectMakerReplayAddressV1,
  inspectDirectMakerNoncePairV1,
  inspectDirectMakerNonceV1,
  requireAuthenticatedDirectMakerNoncePairV1,
  requireAuthenticatedDirectMakerNonceV1,
} from './directMakerReplay';

const TRADING = Keypair.fromSeed(new Uint8Array(32).fill(71)).publicKey.toBase58();
const MARKET = Keypair.fromSeed(new Uint8Array(32).fill(72)).publicKey.toBase58();
const MAKER = Keypair.fromSeed(new Uint8Array(32).fill(73)).publicKey.toBase58();
const MAKER_TWO = Keypair.fromSeed(new Uint8Array(32).fill(76)).publicKey.toBase58();
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
  maker?: string;
  owner?: string;
  executable?: boolean;
  space?: number;
  lamports?: string;
  legacyWidth?: boolean;
  bytes?: (bytes: Uint8Array) => void;
}>): RpcAccount {
  const maker = overrides?.maker ?? MAKER;
  const { bump } = deriveDirectMakerReplayAddressV1(TRADING, MARKET, GENERATION, maker);
  const data = new Uint8Array(DIRECT_MAKER_REPLAY_BYTES_V1);
  data.set(new TextEncoder().encode('DCLTDMR1'), 0);
  putU16(data, 8, 1);
  data[10] = bump;
  data.set(new PublicKey(MARKET).toBytes(), 16);
  putU64(data, 48, GENERATION);
  data.set(new PublicKey(maker).toBytes(), 56);
  putU64(data, 88, 9n);
  putU64(data, 96, 2n);
  putU64(data, 104, 5n);
  data.set(new PublicKey(RENT_OWNER).toBytes(), 112);
  putU64(data, 144, 2_000_000n);
  putU64(data, 152, 4n);
  overrides?.bytes?.(data);
  const body = overrides?.legacyWidth === true
    ? data.slice(0, DIRECT_MAKER_REPLAY_LEGACY_BYTES_V1)
    : data;
  return Object.freeze({
    data: body,
    executable: overrides?.executable ?? false,
    lamports: overrides?.lamports ?? '2000000',
    owner: overrides?.owner ?? TRADING,
    space: overrides?.space ?? body.length,
  });
}

function pairClient(accounts: readonly [RpcAccount | null, RpcAccount | null], observedSlot = '501') {
  const first = deriveDirectMakerReplayAddressV1(TRADING, MARKET, GENERATION, MAKER).address;
  const second = deriveDirectMakerReplayAddressV1(TRADING, MARKET, GENERATION, MAKER_TWO).address;
  return {
    finalizedSlot: vi.fn(async () => '500'),
    multipleAccounts: vi.fn(async (addresses: ReadonlyArray<string>) => Object.freeze({
      slot: observedSlot,
      accounts: Object.freeze(addresses.map((address, index) => Object.freeze({
        address,
        account: address === first ? accounts[0] : address === second ? accounts[1] : index === 0 ? accounts[0] : accounts[1],
      }))),
    })),
  };
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
    expect(observed.feeOwed).toBe(4n);
  });

  it('reads both declared widths, and a legacy record owes nothing', async () => {
    const legacy = await inspectDirectMakerNonceV1(client(replayAccount({ legacyWidth: true })), {
      tradingProgram: TRADING, market: MARKET, generation: GENERATION, maker: MAKER,
    });
    expect(legacy).toMatchObject({ state: 'existing', nextNonce: 9n });
    expect(legacy.feeOwed).toBe(0n);
    await expect(inspectDirectMakerNonceV1(
      client(replayAccount({ space: DIRECT_MAKER_REPLAY_LEGACY_BYTES_V1 - 1 })),
      { tradingProgram: TRADING, market: MARKET, generation: GENERATION, maker: MAKER },
    )).rejects.toThrow('160 or 152 bytes');
  });

  it('reads seller and buyer replay roots from one exact finalized snapshot', async () => {
    const rpc = pairClient([replayAccount(), replayAccount({ maker: MAKER_TWO })]);
    const observed = await inspectDirectMakerNoncePairV1(rpc, [
      { tradingProgram: TRADING, market: MARKET, generation: GENERATION, maker: MAKER },
      { tradingProgram: TRADING, market: MARKET, generation: GENERATION, maker: MAKER_TWO },
    ]);
    const addresses = [
      deriveDirectMakerReplayAddressV1(TRADING, MARKET, GENERATION, MAKER).address,
      deriveDirectMakerReplayAddressV1(TRADING, MARKET, GENERATION, MAKER_TWO).address,
    ];
    expect(rpc.finalizedSlot).toHaveBeenCalledOnce();
    expect(rpc.multipleAccounts).toHaveBeenCalledWith(addresses, '500');
    expect(observed.map((entry) => [entry.maker, entry.nextNonce, entry.observedSlot])).toEqual([
      [MAKER, 9n, '501'], [MAKER_TWO, 9n, '501'],
    ]);
    expect(requireAuthenticatedDirectMakerNoncePairV1(observed)).toBe(observed);
    expect(() => requireAuthenticatedDirectMakerNoncePairV1([...observed] as unknown as typeof observed))
      .toThrow('not acquired from the authenticated pair reader');
  });

  it('refuses a mixed route, same-maker replay, reordered RPC result, and slot regression', async () => {
    const requests = [
      { tradingProgram: TRADING, market: MARKET, generation: GENERATION, maker: MAKER },
      { tradingProgram: TRADING, market: MARKET, generation: GENERATION, maker: MAKER_TWO },
    ] as const;
    await expect(inspectDirectMakerNoncePairV1(pairClient([null, null]), [
      requests[0], { ...requests[1], market: FOREIGN },
    ])).rejects.toThrow('does not share');
    await expect(inspectDirectMakerNoncePairV1(pairClient([null, null]), [requests[0], { ...requests[1], maker: MAKER }]))
      .rejects.toThrow('two distinct makers');
    const reordered = pairClient([null, null]);
    reordered.multipleAccounts.mockImplementationOnce(async (addresses: ReadonlyArray<string>) => Object.freeze({
      slot: '501',
      accounts: Object.freeze([...addresses].reverse().map((address) => Object.freeze({ address, account: null }))),
    }));
    await expect(inspectDirectMakerNoncePairV1(reordered, requests)).rejects.toThrow('substituted or reordered');
    await expect(inspectDirectMakerNoncePairV1(pairClient([null, null], '499'), requests)).rejects.toThrow('regressed below');
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
    await expect(inspectDirectMakerNonceV1(client(replayAccount({ space: DIRECT_MAKER_REPLAY_BYTES_V1 + 1 })), request)).rejects.toThrow('160 or 152 bytes');
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
