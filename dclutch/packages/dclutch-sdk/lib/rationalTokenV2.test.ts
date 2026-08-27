import { AddressLookupTableAccount, PublicKey } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import { type RpcAccount } from './rpc';
import {
  TOKEN_2022_BEHAVIOR_PROFILE_ID_V2,
  TOKEN_2022_PROGRAM_ID,
  buildUnsignedBearerTransferV2,
  decodeToken2022BehaviorAccountV2,
  decodeToken2022BehaviorMintV2,
  decodeTokenBehaviorSelectionV2,
  encodeTokenBehaviorSelectionV2,
  type BearerTransferInspectionV2,
} from './rationalTokenV2';

const MAX_U64 = 18_446_744_073_709_551_615n;

function key(seed: number): string {
  return new PublicKey(new Uint8Array(32).fill(seed)).toBase58();
}

function putU16(bytes: Uint8Array, offset: number, value: number): void {
  new DataView(bytes.buffer, bytes.byteOffset + offset, 2).setUint16(0, value, true);
}

function putU32(bytes: Uint8Array, offset: number, value: number): void {
  new DataView(bytes.buffer, bytes.byteOffset + offset, 4).setUint32(0, value, true);
}

function putU64(bytes: Uint8Array, offset: number, value: bigint): void {
  new DataView(bytes.buffer, bytes.byteOffset + offset, 8).setBigUint64(0, value, true);
}

function account(owner: string, data: Uint8Array): RpcAccount {
  return Object.freeze({ owner, data, executable: false, lamports: '1000000', space: data.length });
}

function mintFixture(mint: string, controller: string, decimals = 255, metadata = false): RpcAccount {
  const base = new Uint8Array(166);
  putU32(base, 0, 1); base.set(new PublicKey(controller).toBytes(), 4);
  putU64(base, 36, 900n); base[44] = decimals; base[45] = 1; base[165] = 1;
  const entries: Uint8Array[] = [];
  const entry = (kind: number, value: Uint8Array) => {
    const bytes = new Uint8Array(4 + value.length); putU16(bytes, 0, kind); putU16(bytes, 2, value.length); bytes.set(value, 4); entries.push(bytes);
  };
  entry(3, new PublicKey(controller).toBytes());
  if (metadata) {
    const pointer = new Uint8Array(64); pointer.set(new PublicKey(mint).toBytes(), 32); entry(18, pointer);
    const value = new Uint8Array(80); value.set(new PublicKey(mint).toBytes(), 32); entry(19, value);
  }
  entry(28, new PublicKey(controller).toBytes());
  const bytes = new Uint8Array(base.length + entries.reduce((sum, value) => sum + value.length, 0));
  bytes.set(base); let offset = base.length; for (const value of entries) { bytes.set(value, offset); offset += value.length; }
  return account(TOKEN_2022_PROGRAM_ID, bytes);
}

function tokenAccountFixture(mint: string, owner: string, amount: bigint): RpcAccount {
  const bytes = new Uint8Array(165); bytes.set(new PublicKey(mint).toBytes(), 0); bytes.set(new PublicKey(owner).toBytes(), 32);
  putU64(bytes, 64, amount); bytes[108] = 1;
  return account(TOKEN_2022_PROGRAM_ID, bytes);
}

describe('TokenBehaviorSelectionV2 and ordinary Bearer transfer', () => {
  it('binds exact Market Realm/release bytes and refuses profile, program, reserved, and authority substitutions', () => {
    const realm = new Uint8Array(32).fill(1); const release = new Uint8Array(32).fill(2);
    const bytes = encodeTokenBehaviorSelectionV2(realm, release);
    const decoded = decodeTokenBehaviorSelectionV2(bytes, realm, release);
    expect(decoded.bytes).toHaveLength(144);
    expect(decoded.profileId).toEqual(TOKEN_2022_BEHAVIOR_PROFILE_ID_V2);
    expect(decoded.tokenProgram).toBe(TOKEN_2022_PROGRAM_ID);
    for (const offset of [10, 80, 112]) {
      const hostile = bytes.slice(); hostile[offset] ^= 0xff;
      expect(() => decodeTokenBehaviorSelectionV2(hostile, realm, release)).toThrow();
    }
    const anotherRealm = realm.slice(); anotherRealm[0] = 9;
    expect(() => decodeTokenBehaviorSelectionV2(bytes, anotherRealm, release)).toThrow('authenticated Market');
    expect(() => encodeTokenBehaviorSelectionV2(realm, realm)).toThrow('alias');
  });

  it('accepts full-u8 display decimals without converting raw-u64 supply or balances', () => {
    const mint = key(3); const controller = key(4); const holder = key(5);
    const mintView = decodeToken2022BehaviorMintV2(mint, mintFixture(mint, controller, 255, true));
    const holderView = decodeToken2022BehaviorAccountV2(key(6), tokenAccountFixture(mint, holder, MAX_U64));
    expect(mintView.displayDecimals).toBe(255);
    expect(mintView.rawSupply).toBe(900n);
    expect(mintView.metadata).toBe('immutable-self-hosted');
    expect(holderView.rawAmount).toBe(MAX_U64);
    expect(holderView.owner).toBe(holder);
  });

  it('refuses behavior-changing Mint and Account authority scars', () => {
    const mint = key(7); const controller = key(8); const owner = key(9);
    const unknownExtension = mintFixture(mint, controller).data.slice();
    putU16(unknownExtension, 166, 99);
    expect(() => decodeToken2022BehaviorMintV2(mint, account(TOKEN_2022_PROGRAM_ID, unknownExtension))).toThrow('extension 99');
    const frozen = tokenAccountFixture(mint, owner, 1n).data.slice(); frozen[108] = 2;
    expect(() => decodeToken2022BehaviorAccountV2(key(10), account(TOKEN_2022_PROGRAM_ID, frozen))).toThrow('frozen');
    const delegated = tokenAccountFixture(mint, owner, 1n).data.slice(); putU32(delegated, 72, 1); delegated.set(new PublicKey(key(11)).toBytes(), 76);
    expect(() => decodeToken2022BehaviorAccountV2(key(10), account(TOKEN_2022_PROGRAM_ID, delegated))).toThrow('delegated');
    const extended = new Uint8Array(166); extended.set(tokenAccountFixture(mint, owner, 1n).data);
    expect(() => decodeToken2022BehaviorAccountV2(key(10), account(TOKEN_2022_PROGRAM_ID, extended))).toThrow('extension-free');
  });

  it('compiles exact raw atoms into one unsigned v0 TransferChecked packet using the selected ALT', () => {
    const payer = key(12); const mintAddress = key(13); const sourceAddress = key(14); const destinationAddress = key(15);
    const realm = new Uint8Array(32).fill(16); const release = new Uint8Array(32).fill(17);
    const selection = decodeTokenBehaviorSelectionV2(encodeTokenBehaviorSelectionV2(realm, release), realm, release);
    const lookupTable = new AddressLookupTableAccount({
      key: new PublicKey(key(18)),
      state: {
        deactivationSlot: MAX_U64, lastExtendedSlot: 44, lastExtendedSlotStartIndex: 0, authority: undefined,
        addresses: [sourceAddress, mintAddress, destinationAddress, TOKEN_2022_PROGRAM_ID].map((value) => new PublicKey(value)),
      },
    });
    const inspection: BearerTransferInspectionV2 = Object.freeze({
      observedSlot: '45', payer, authority: payer, coreProgram: key(19), market: key(20), marketPhase: 'Open', generation: 7n,
      registryProgram: key(21), selectionRecord: key(22), selectionDigest: new Uint8Array(32).fill(23), selection,
      mint: Object.freeze({ mint: mintAddress, controller: key(24), rawSupply: 900n, displayDecimals: 255, metadata: 'absent' }),
      source: Object.freeze({ address: sourceAddress, mint: mintAddress, owner: payer, rawAmount: 77n }),
      destination: Object.freeze({ address: destinationAddress, mint: mintAddress, owner: key(25), rawAmount: 2n }), lookupTable,
    });
    const plan = buildUnsignedBearerTransferV2(inspection, key(26), 71n);
    expect(Array.from(plan.instructionBytes)).toEqual([12, 71, 0, 0, 0, 0, 0, 0, 0, 255]);
    expect(plan.rawAmount).toBe(71n);
    expect(plan.displayDecimals).toBe(255);
    expect(plan.requiredSigners).toEqual([payer]);
    expect(plan.loadedAddresses).toBeGreaterThan(0);
    expect(plan.wireBytes.length).toBeLessThanOrEqual(1_232);
    expect(() => buildUnsignedBearerTransferV2(inspection, key(26), 78n)).toThrow('raw balance');
    expect(() => buildUnsignedBearerTransferV2(inspection, key(26), 0n)).toThrow('1..u64');
  });
});
