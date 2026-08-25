import { AddressLookupTableAccount, PublicKey, SYSVAR_INSTRUCTIONS_PUBKEY } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import { decodeControllerInstructionV1 } from './directCodec';
import {
  CLAIM_PROGRAM_ID,
  CUSTODY_PROGRAM_ID,
  buildUnsignedDirectTransaction,
  deriveDirectAddresses,
  encodeIntentSigningPayload,
  type DirectMatchInputV1,
} from './directTransaction';

function key(byte: number): PublicKey {
  return new PublicKey(new Uint8Array(32).fill(byte));
}

function fixture(): DirectMatchInputV1 {
  const controllerProgram = key(67);
  const market = key(68);
  const sellerMaker = key(1);
  const buyerMaker = key(2);
  const sellerDestination = key(5);
  const buyerSource = key(6);
  const routing = {
    journal: key(10).toBase58(),
    realm: key(7).toBase58(),
    feePolicy: key(11).toBase58(),
    capabilityManifest: key(12).toBase58(),
    mint: key(8).toBase58(),
    buyerSource: buyerSource.toBase58(),
    sellerDestination: sellerDestination.toBase58(),
    feeDestination: key(9).toBase58(),
    tokenProgram: key(13).toBase58(),
  };
  const intent = (collateral: PublicKey, side: number) => ({
    side,
    outcome: 1,
    lifecycle: 0,
    market: market.toBytes(),
    generation: 3n,
    nonce: 0n,
    validFrom: 0n,
    validThrough: 18_446_744_073_709_551_615n,
    maximumFill: 2_000n,
    limitPrice: side === 0 ? 400_000n : 600_000n,
    feeBasisPoints: 25,
    collateralAccount: collateral.toBytes(),
  });
  const derived = deriveDirectAddresses(
    controllerProgram.toBase58(), market.toBase58(), sellerMaker.toBase58(), buyerMaker.toBase58(), 3n, 1, 1,
  );
  const lookupAddresses = [
    derived.controller, key(10), CLAIM_PROGRAM_ID, CUSTODY_PROGRAM_ID, market,
    key(7), key(11), key(12), key(8), key(9), key(13), SYSVAR_INSTRUCTIONS_PUBKEY,
  ];
  return {
    controllerProgram: controllerProgram.toBase58(),
    market: market.toBase58(),
    payer: key(93).toBase58(),
    recentBlockhash: key(94).toBase58(),
    fill: 2_000n,
    executionPrice: 500_000n,
    seller: { maker: sellerMaker.toBase58(), signature: new Uint8Array(64).fill(11), intent: intent(sellerDestination, 0) },
    buyer: { maker: buyerMaker.toBase58(), signature: new Uint8Array(64).fill(12), intent: intent(buyerSource, 1) },
    routing,
    lookupTable: new AddressLookupTableAccount({
      key: key(91),
      state: {
        deactivationSlot: 18_446_744_073_709_551_615n,
        lastExtendedSlot: 54,
        lastExtendedSlotStartIndex: 0,
        authority: key(92),
        addresses: lookupAddresses,
      },
    }),
  };
}

describe('chain-ready compiled Direct transaction construction', () => {
  it('builds the exact packet-safe native-signature/controller v0 batch', () => {
    const input = fixture();
    const report = buildUnsignedDirectTransaction(input);
    expect(report.wireBytes).toHaveLength(990);
    expect(report.instructions[0].data).toHaveLength(222);
    expect(report.instructions[1].keys).toHaveLength(18);
    expect(report.lookupAddressesUsed).toBe(12);
    const decoded = decodeControllerInstructionV1(report.controllerData);
    expect(decoded.seller).toEqual(input.seller.intent);
    expect(decoded.buyer).toEqual(input.buyer.intent);
    expect(decoded.fill).toBe(2_000n);
    expect(decoded.executionPrice).toBe(500_000n);
    expect(new DataView(report.instructions[0].data.buffer).getUint16(0, true)).toBe(2);
  });

  it('exports one exact signer payload and refuses routing substitution', () => {
    const input = fixture();
    const payload = encodeIntentSigningPayload(input.seller.intent);
    expect(payload.bytes).toHaveLength(136);
    expect(payload.hex).toHaveLength(272);
    expect(atob(payload.base64)).toHaveLength(136);

    const substituted = { ...input, routing: { ...input.routing, sellerDestination: key(99).toBase58() } };
    expect(() => buildUnsignedDirectTransaction(substituted)).toThrow(/bindings/);
  });

  it('refuses all-zero signatures, same-maker replay aliasing, and incomplete lookup tables', () => {
    const input = fixture();
    expect(() => buildUnsignedDirectTransaction({
      ...input,
      seller: { ...input.seller, signature: new Uint8Array(64) },
    })).toThrow(/all-zero/);
    expect(() => buildUnsignedDirectTransaction({
      ...input,
      buyer: { ...input.buyer, maker: input.seller.maker },
    })).toThrow(/makers must differ/);
    expect(() => buildUnsignedDirectTransaction({
      ...input,
      buyer: { ...input.buyer, intent: { ...input.buyer.intent, lifecycle: 2 } },
    })).toThrow(/FOK 0 or IOC 1/);
    expect(() => buildUnsignedDirectTransaction({
      ...input,
      lookupTable: new AddressLookupTableAccount({
        key: key(91),
        state: { ...input.lookupTable.state, addresses: input.lookupTable.state.addresses.slice(1) },
      }),
    })).toThrow(/routing set/);
  });
});
