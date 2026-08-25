import {
  AddressLookupTableAccount,
  Ed25519Program,
  PublicKey,
  SYSVAR_INSTRUCTIONS_PUBKEY,
  TransactionInstruction,
  TransactionMessage,
  VersionedTransaction,
} from '@solana/web3.js';

import {
  COMPACT_INTENT_BYTES,
  type CompactIntentV1,
  encodeCompactIntentV1,
  encodeControllerInstructionV1,
} from './directCodec';

export const CONTROLLER_SEED = new TextEncoder().encode('dclutch-controller-v1');
export const REPLAY_SEED = new TextEncoder().encode('dclutch/direct-replay/v3');
export const POSITION_SEED = new TextEncoder().encode('dclutch/position/v1');
export const CLAIM_PROGRAM_ID = new PublicKey(new Uint8Array(32).fill(81));
export const CUSTODY_PROGRAM_ID = new PublicKey(new Uint8Array(32).fill(75));
export const PACKET_DATA_SIZE = 1_232;

const ED_DESCRIPTOR_BYTES = 14;
const ED_PAYLOAD_OFFSET = 2 + 2 * ED_DESCRIPTOR_BYTES;
const SELLER_MESSAGE_OFFSET = 32;
const BUYER_MESSAGE_OFFSET = 168;

export type SignedCompactIntentV1 = Readonly<{
  maker: string;
  signature: Uint8Array;
  intent: CompactIntentV1;
}>;

export type DirectRoutingV1 = Readonly<{
  journal: string;
  realm: string;
  feePolicy: string;
  capabilityManifest: string;
  mint: string;
  buyerSource: string;
  sellerDestination: string;
  feeDestination: string;
  tokenProgram: string;
}>;

export type DirectMatchInputV1 = Readonly<{
  controllerProgram: string;
  market: string;
  payer: string;
  recentBlockhash: string;
  fill: bigint;
  executionPrice: bigint;
  seller: SignedCompactIntentV1;
  buyer: SignedCompactIntentV1;
  routing: DirectRoutingV1;
  lookupTable: AddressLookupTableAccount;
}>;

export type DirectDerivedAddressesV1 = Readonly<{
  controller: PublicKey;
  sellerReplay: PublicKey;
  buyerReplay: PublicKey;
  sellerPosition: PublicKey;
  buyerPosition: PublicKey;
  controllerBump: number;
  sellerReplayBump: number;
  buyerReplayBump: number;
  sellerPositionBump: number;
  buyerPositionBump: number;
}>;

export type UnsignedDirectTransactionV1 = Readonly<{
  transaction: VersionedTransaction;
  wireBytes: Uint8Array;
  controllerData: Uint8Array;
  instructions: readonly [TransactionInstruction, TransactionInstruction];
  derived: DirectDerivedAddressesV1;
  lookupAddressesUsed: number;
}>;

function exactKey(value: string, field: string): PublicKey {
  const key = new PublicKey(value);
  if (key.toBase58() !== value) throw new Error(`${field} is not canonical base58 text`);
  return key;
}

function exactSignature(value: Uint8Array, field: string): Uint8Array {
  if (value.length !== 64) throw new Error(`${field} must be exactly 64 bytes`);
  if (value.every((byte) => byte === 0)) throw new Error(`${field} must not be the all-zero signature`);
  return new Uint8Array(value);
}

function putU16(output: Uint8Array, offset: number, value: number): void {
  new DataView(output.buffer, output.byteOffset + offset, 2).setUint16(0, value, true);
}

function littleEndianU64(value: bigint): Uint8Array {
  if (value < 0n || value > 18_446_744_073_709_551_615n) throw new Error('generation is not a u64');
  const output = new Uint8Array(8);
  new DataView(output.buffer).setBigUint64(0, value, true);
  return output;
}

export function deriveDirectAddresses(
  controllerProgramText: string,
  marketText: string,
  sellerMakerText: string,
  buyerMakerText: string,
  generation: bigint,
  sellerOutcome: number,
  buyerOutcome: number,
): DirectDerivedAddressesV1 {
  if (!Number.isInteger(sellerOutcome) || sellerOutcome < 0 || sellerOutcome > 255) throw new Error('seller outcome is not a byte');
  if (!Number.isInteger(buyerOutcome) || buyerOutcome < 0 || buyerOutcome > 255) throw new Error('buyer outcome is not a byte');
  const controllerProgram = exactKey(controllerProgramText, 'controller program');
  const market = exactKey(marketText, 'Market');
  const sellerMaker = exactKey(sellerMakerText, 'seller maker');
  const buyerMaker = exactKey(buyerMakerText, 'buyer maker');
  if (sellerMaker.equals(buyerMaker)) throw new Error('seller and buyer makers must differ');
  const generationBytes = littleEndianU64(generation);
  const [controller, controllerBump] = PublicKey.findProgramAddressSync([CONTROLLER_SEED], controllerProgram);
  const [sellerReplay, sellerReplayBump] = PublicKey.findProgramAddressSync(
    [REPLAY_SEED, market.toBytes(), generationBytes, sellerMaker.toBytes()], controllerProgram,
  );
  const [buyerReplay, buyerReplayBump] = PublicKey.findProgramAddressSync(
    [REPLAY_SEED, market.toBytes(), generationBytes, buyerMaker.toBytes()], controllerProgram,
  );
  const [sellerPosition, sellerPositionBump] = PublicKey.findProgramAddressSync(
    [POSITION_SEED, market.toBytes(), sellerMaker.toBytes(), Uint8Array.of(sellerOutcome)], controllerProgram,
  );
  const [buyerPosition, buyerPositionBump] = PublicKey.findProgramAddressSync(
    [POSITION_SEED, market.toBytes(), buyerMaker.toBytes(), Uint8Array.of(buyerOutcome)], controllerProgram,
  );
  return Object.freeze({
    controller, sellerReplay, buyerReplay, sellerPosition, buyerPosition,
    controllerBump, sellerReplayBump, buyerReplayBump, sellerPositionBump, buyerPositionBump,
  });
}

function nativeEd25519Batch(
  seller: SignedCompactIntentV1,
  buyer: SignedCompactIntentV1,
): TransactionInstruction {
  const output = new Uint8Array(ED_PAYLOAD_OFFSET + 2 * 96);
  putU16(output, 0, 2);
  for (const [index, material, messageOffset] of [
    [0, seller, SELLER_MESSAGE_OFFSET],
    [1, buyer, BUYER_MESSAGE_OFFSET],
  ] as const) {
    const descriptor = 2 + index * ED_DESCRIPTOR_BYTES;
    const publicKeyOffset = ED_PAYLOAD_OFFSET + index * 96;
    const signatureOffset = publicKeyOffset + 32;
    putU16(output, descriptor, signatureOffset);
    putU16(output, descriptor + 2, 0xffff);
    putU16(output, descriptor + 4, publicKeyOffset);
    putU16(output, descriptor + 6, 0xffff);
    putU16(output, descriptor + 8, messageOffset);
    putU16(output, descriptor + 10, COMPACT_INTENT_BYTES);
    putU16(output, descriptor + 12, 1);
    output.set(exactKey(material.maker, `${index === 0 ? 'seller' : 'buyer'} maker`).toBytes(), publicKeyOffset);
    output.set(exactSignature(material.signature, `${index === 0 ? 'seller' : 'buyer'} signature`), signatureOffset);
  }
  return new TransactionInstruction({ programId: Ed25519Program.programId, keys: [], data: output as Buffer });
}

export function buildUnsignedDirectTransaction(input: DirectMatchInputV1): UnsignedDirectTransactionV1 {
  const controllerProgram = exactKey(input.controllerProgram, 'controller program');
  const market = exactKey(input.market, 'Market');
  const payer = exactKey(input.payer, 'payer');
  const sellerCollateral = exactKey(input.routing.sellerDestination, 'seller destination');
  const buyerCollateral = exactKey(input.routing.buyerSource, 'buyer source');
  if (input.seller.intent.side !== 0 || input.buyer.intent.side !== 1) throw new Error('compiled Direct requires seller side 0 and buyer side 1');
  if (input.seller.intent.lifecycle > 1 || input.buyer.intent.lifecycle > 1) throw new Error('compiled Direct lifecycle must be FOK 0 or IOC 1');
  if (input.fill === 0n || input.seller.intent.maximumFill === 0n || input.buyer.intent.maximumFill === 0n) throw new Error('compiled Direct fill capacity must be positive');
  if (!new PublicKey(input.seller.intent.market).equals(market)
      || !new PublicKey(input.buyer.intent.market).equals(market)
      || input.seller.intent.generation !== input.buyer.intent.generation
      || input.seller.intent.feeBasisPoints !== input.buyer.intent.feeBasisPoints
      || !new PublicKey(input.seller.intent.collateralAccount).equals(sellerCollateral)
      || !new PublicKey(input.buyer.intent.collateralAccount).equals(buyerCollateral)) {
    throw new Error('signed intents do not share the exact Market/generation/fee/routing bindings');
  }
  const derived = deriveDirectAddresses(
    input.controllerProgram, input.market, input.seller.maker, input.buyer.maker,
    input.seller.intent.generation, input.seller.intent.outcome, input.buyer.intent.outcome,
  );
  const controllerData = encodeControllerInstructionV1({
    controllerBump: derived.controllerBump,
    sellerReplayBump: derived.sellerReplayBump,
    buyerReplayBump: derived.buyerReplayBump,
    sellerPositionBump: derived.sellerPositionBump,
    buyerPositionBump: derived.buyerPositionBump,
    fill: input.fill,
    executionPrice: input.executionPrice,
    seller: input.seller.intent,
    buyer: input.buyer.intent,
  });
  // Parsing the blockhash as a 32-byte key is an exact base58-width check.
  exactKey(input.recentBlockhash, 'recent blockhash');
  const key = (value: string, field: string) => exactKey(value, field);
  const controllerInstruction = new TransactionInstruction({
    programId: controllerProgram,
    keys: [
      { pubkey: derived.controller, isSigner: false, isWritable: false },
      { pubkey: derived.sellerReplay, isSigner: false, isWritable: true },
      { pubkey: derived.buyerReplay, isSigner: false, isWritable: true },
      { pubkey: key(input.routing.journal, 'journal'), isSigner: false, isWritable: true },
      { pubkey: derived.sellerPosition, isSigner: false, isWritable: true },
      { pubkey: derived.buyerPosition, isSigner: false, isWritable: true },
      { pubkey: CLAIM_PROGRAM_ID, isSigner: false, isWritable: false },
      { pubkey: CUSTODY_PROGRAM_ID, isSigner: false, isWritable: false },
      { pubkey: market, isSigner: false, isWritable: false },
      { pubkey: key(input.routing.realm, 'Realm'), isSigner: false, isWritable: false },
      { pubkey: key(input.routing.feePolicy, 'fee policy'), isSigner: false, isWritable: false },
      { pubkey: key(input.routing.capabilityManifest, 'capability manifest'), isSigner: false, isWritable: false },
      { pubkey: key(input.routing.mint, 'mint'), isSigner: false, isWritable: false },
      { pubkey: buyerCollateral, isSigner: false, isWritable: true },
      { pubkey: sellerCollateral, isSigner: false, isWritable: true },
      { pubkey: key(input.routing.feeDestination, 'fee destination'), isSigner: false, isWritable: true },
      { pubkey: key(input.routing.tokenProgram, 'token program'), isSigner: false, isWritable: false },
      { pubkey: SYSVAR_INSTRUCTIONS_PUBKEY, isSigner: false, isWritable: false },
    ],
    data: controllerData as Buffer,
  });
  const signatureInstruction = nativeEd25519Batch(input.seller, input.buyer);
  const requiredLookupAddresses = [
    derived.controller,
    key(input.routing.journal, 'journal'),
    CLAIM_PROGRAM_ID,
    CUSTODY_PROGRAM_ID,
    market,
    key(input.routing.realm, 'Realm'),
    key(input.routing.feePolicy, 'fee policy'),
    key(input.routing.capabilityManifest, 'capability manifest'),
    key(input.routing.mint, 'mint'),
    key(input.routing.feeDestination, 'fee destination'),
    key(input.routing.tokenProgram, 'token program'),
    SYSVAR_INSTRUCTIONS_PUBKEY,
  ];
  const lookupSet = new Set(input.lookupTable.state.addresses.map((address) => address.toBase58()));
  if (!input.lookupTable.isActive()) throw new Error('lookup table is deactivated');
  if (requiredLookupAddresses.some((address) => !lookupSet.has(address.toBase58()))) {
    throw new Error('lookup table does not contain the exact reusable Market routing set');
  }
  const message = new TransactionMessage({
    payerKey: payer,
    recentBlockhash: input.recentBlockhash,
    instructions: [signatureInstruction, controllerInstruction],
  }).compileToV0Message([input.lookupTable]);
  const transaction = new VersionedTransaction(message);
  const wireBytes = transaction.serialize();
  if (wireBytes.length > PACKET_DATA_SIZE) throw new Error(`unsigned transaction is ${wireBytes.length} bytes, above the ${PACKET_DATA_SIZE}-byte packet bound`);
  return Object.freeze({
    transaction,
    wireBytes,
    controllerData,
    instructions: Object.freeze([signatureInstruction, controllerInstruction]) as readonly [TransactionInstruction, TransactionInstruction],
    derived,
    lookupAddressesUsed: message.addressTableLookups.reduce((sum, lookup) => sum + lookup.readonlyIndexes.length + lookup.writableIndexes.length, 0),
  });
}

export function encodeIntentSigningPayload(intent: CompactIntentV1): Readonly<{ bytes: Uint8Array; hex: string; base64: string }> {
  const bytes = encodeCompactIntentV1(intent);
  const hex = Array.from(bytes, (byte) => byte.toString(16).padStart(2, '0')).join('');
  let binary = '';
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return Object.freeze({ bytes, hex, base64: btoa(binary) });
}
