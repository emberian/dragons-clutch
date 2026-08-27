import { PublicKey, SYSVAR_RENT_PUBKEY, TransactionInstruction, type AccountMeta } from '@solana/web3.js';

import { hex, isZero, requireZero, slice, u16 } from './bytes';
import {
  ADMISSION_ACCOUNT_COUNT_V2,
  ADMISSION_MAGIC_BYTES_V2,
  ADMISSION_MAGIC_OFFSET_V2,
  ADMISSION_RECEIPT_BYTES_V2,
  ADMISSION_RECEIPT_MAGIC_V2,
  ADMISSION_RECEIPT_PDA_DOMAIN_V2,
  ADMISSION_RECORD_COUNT_V2,
  ADMISSION_REQUEST_BYTES_V2,
  ADMISSION_REQUEST_MAGIC_V2,
  ADMISSION_VERSION_OFFSET_V2,
  ADMISSION_VERSION_V2,
  PORTFOLIO_SCHEMA_ID_V2,
  PRODUCT_DOMAIN_DIGEST_OFFSET_V2,
  PRODUCT_ID_OFFSET_V2,
  PRODUCT_PORTFOLIO_DIGEST_OFFSET_V2,
  PRODUCT_RECORD_BYTES_V2,
  PRODUCT_RECORD_MAGIC_V2,
  PRODUCT_RECORD_RESERVED_BYTES_V2,
  PRODUCT_RECORD_RESERVED_OFFSET_V2,
  PRODUCT_RECORD_SCHEMA_ID_V2,
  RECEIPT_COUNT_OFFSET_V2,
  RECEIPT_RECORDS_OFFSET_V2,
  RECEIPT_RESERVED_BYTES_V2,
  RECEIPT_RESERVED_OFFSET_V2,
  RECORD_COORDINATE_BYTES_V2,
  REQUEST_DOMAIN_DIGEST_OFFSET_V2,
  REQUEST_PORTFOLIO_DIGEST_OFFSET_V2,
  REQUEST_PRODUCT_DIGEST_OFFSET_V2,
  REQUEST_RESERVED_BYTES_V2,
  REQUEST_RESERVED_OFFSET_V2,
  RESULT_DOMAIN_SCHEMA_ID_V2,
} from './generated/productRuntimeV2Admission';

/**
 * The browser's Product Runtime V2 admission wire.
 *
 * AUTHORITY. `programs/dclutch-product-runtime-v2-sbf` is the deployed adapter;
 * `crates/dclutch-product-runtime-v2-admission` is the wire it decodes. Every
 * magic, width, offset, reserved span, account count and schema identity below
 * arrives through `lib/generated/productRuntimeV2Admission.ts`, which
 * `npm run abi:product-runtime-v2-admission` reads out of those two files. This
 * module states no protocol fact of its own.
 *
 * WHY THAT MATTERS MORE HERE THAN ANYWHERE ELSE. `DCLTPRQ2` is not one wire. It
 * names two incompatible 112-byte requests: the dead evaluator request that
 * belonged to `dclutch-product-payoff-v2-svm`, and this live admission request.
 * They share a magic and a width, so a diff cannot tell them apart -- but the
 * dead encoder wrote 1 at byte 10, and this decoder REQUIRES ZERO across bytes
 * 10..16. A browser that mirrored the dead layout would have produced a request
 * the deployed program refuses as `NonCanonical`, with a hex dump that looks
 * right. That collision is why the browser encoder was rebuilt from the live
 * decoder rather than renamed from the dead one, and why
 * `productRuntimeV2Admission.test.ts` witnesses each of the decoder's refusals
 * by name instead of asserting one happy path.
 *
 * WHAT THIS DOES NOT DO. It never claims a record is Registry-finalized. The
 * adapter authenticates owner, PDA, hash, rent exemption and staging vacancy
 * for all three records at its own trust boundary, and Core and Claims repeat
 * that work at theirs. This module composes the request, derives the receipt
 * address the program will itself recompute, and hostile-decodes what comes
 * back. It signs and submits nothing.
 */

export type AdmissionDigestsV2 = Readonly<{
  productDigest: Uint8Array;
  resultDomainDigest: Uint8Array;
  portfolioDigest: Uint8Array;
}>;

export type FinalizedRecordCoordinateV2 = Readonly<{
  schemaId: Uint8Array;
  contentDigest: Uint8Array;
  rawAccount: string;
  stagingAccount: string;
}>;

export type AdmissionReceiptV2 = Readonly<{
  product: FinalizedRecordCoordinateV2;
  resultDomain: FinalizedRecordCoordinateV2;
  portfolio: FinalizedRecordCoordinateV2;
}>;

export type ProductRecordV2 = Readonly<{
  productId: Uint8Array;
  resultDomainDigest: Uint8Array;
  portfolioDigest: Uint8Array;
}>;

export type AdmissionAccountsV2 = Readonly<{
  programId: string;
  registry: string;
  productRaw: string;
  productStaging: string;
  resultDomainRaw: string;
  resultDomainStaging: string;
  portfolioRaw: string;
  portfolioStaging: string;
}>;

export type AdmissionInstructionV2 = Readonly<{
  receipt: string;
  receiptBump: number;
  instruction: TransactionInstruction;
  requestBytes: Uint8Array;
}>;

const RENT_SYSVAR_OWNER = 'Sysvar1111111111111111111111111111111111111';

function ascii8(value: string): Uint8Array {
  return new TextEncoder().encode(value);
}

/**
 * `ContentId::new` refuses the all-zero identity. The browser refuses it at the
 * same boundary so a zero never reaches the PDA derivation, where it would
 * silently produce a plausible address for a record that cannot exist.
 */
function contentId(value: Uint8Array, field: string): Uint8Array {
  if (value.length !== 32) throw new Error(`${field} is not a 32-byte content identity`);
  if (isZero(value)) throw new Error(`${field} is the reserved all-zero content identity, which ContentId::new refuses`);
  return new Uint8Array(value);
}

function readContentId(bytes: Uint8Array, offset: number, field: string): Uint8Array {
  return contentId(slice(bytes, offset, 32), field);
}

function requireHeader(bytes: Uint8Array, expectedMagic: string, expectedLength: number, record: string): void {
  if (bytes.length !== expectedLength) {
    throw new Error(`${record} is ${bytes.length} bytes, not the exact ${expectedLength} the program decodes (InvalidLength)`);
  }
  const magic = new TextDecoder('ascii').decode(slice(bytes, ADMISSION_MAGIC_OFFSET_V2, ADMISSION_MAGIC_BYTES_V2));
  const version = u16(bytes, ADMISSION_VERSION_OFFSET_V2);
  if (magic !== expectedMagic || version !== ADMISSION_VERSION_V2) {
    throw new Error(`${record} selects ${magic} v${version}, not ${expectedMagic} v${ADMISSION_VERSION_V2} (UnsupportedSchema)`);
  }
}

/** Encode one exact `AdmissionRequestV2`. */
export function encodeAdmissionRequestV2(digests: AdmissionDigestsV2): Uint8Array {
  const product = contentId(digests.productDigest, 'Product record digest');
  const domain = contentId(digests.resultDomainDigest, 'result-domain record digest');
  const portfolio = contentId(digests.portfolioDigest, 'portfolio record digest');
  const bytes = new Uint8Array(ADMISSION_REQUEST_BYTES_V2);
  bytes.set(ascii8(ADMISSION_REQUEST_MAGIC_V2), ADMISSION_MAGIC_OFFSET_V2);
  new DataView(bytes.buffer).setUint16(ADMISSION_VERSION_OFFSET_V2, ADMISSION_VERSION_V2, true);
  // Bytes REQUEST_RESERVED_OFFSET_V2..+REQUEST_RESERVED_BYTES_V2 stay zero: the
  // live decoder's `require_zero`, and the one byte the dead DCLTPRQ2 encoder
  // set to 1.
  bytes.set(product, REQUEST_PRODUCT_DIGEST_OFFSET_V2);
  bytes.set(domain, REQUEST_DOMAIN_DIGEST_OFFSET_V2);
  bytes.set(portfolio, REQUEST_PORTFOLIO_DIGEST_OFFSET_V2);
  return bytes;
}

/** Hostile-decode one exact `AdmissionRequestV2`, refusing where the program refuses. */
export function decodeAdmissionRequestV2(bytes: Uint8Array): AdmissionDigestsV2 {
  requireHeader(bytes, ADMISSION_REQUEST_MAGIC_V2, ADMISSION_REQUEST_BYTES_V2, 'admission request');
  requireZero(bytes, REQUEST_RESERVED_OFFSET_V2, REQUEST_RESERVED_BYTES_V2, 'admission request (NonCanonical)');
  return Object.freeze({
    productDigest: readContentId(bytes, REQUEST_PRODUCT_DIGEST_OFFSET_V2, 'Product record digest'),
    resultDomainDigest: readContentId(bytes, REQUEST_DOMAIN_DIGEST_OFFSET_V2, 'result-domain record digest'),
    portfolioDigest: readContentId(bytes, REQUEST_PORTFOLIO_DIGEST_OFFSET_V2, 'portfolio record digest'),
  });
}

/** Hostile-decode one exact `ProductRecordV2` body. */
export function decodeProductRecordV2(bytes: Uint8Array): ProductRecordV2 {
  requireHeader(bytes, PRODUCT_RECORD_MAGIC_V2, PRODUCT_RECORD_BYTES_V2, 'Product record');
  requireZero(bytes, PRODUCT_RECORD_RESERVED_OFFSET_V2, PRODUCT_RECORD_RESERVED_BYTES_V2, 'Product record (NonCanonical)');
  return Object.freeze({
    productId: readContentId(bytes, PRODUCT_ID_OFFSET_V2, 'Product identity'),
    resultDomainDigest: readContentId(bytes, PRODUCT_DOMAIN_DIGEST_OFFSET_V2, 'Product result-domain digest'),
    portfolioDigest: readContentId(bytes, PRODUCT_PORTFOLIO_DIGEST_OFFSET_V2, 'Product portfolio digest'),
  });
}

function decodeCoordinate(bytes: Uint8Array, offset: number, expectedSchema: Uint8Array, role: string): FinalizedRecordCoordinateV2 {
  const schemaId = readContentId(bytes, offset, `${role} schema identity`);
  if (hex(schemaId) !== hex(expectedSchema)) {
    throw new Error(`${role} coordinate carries schema ${hex(schemaId)}, not the pinned ${hex(expectedSchema)} (NonCanonical)`);
  }
  return Object.freeze({
    schemaId,
    contentDigest: readContentId(bytes, offset + 32, `${role} content digest`),
    rawAccount: new PublicKey(readContentId(bytes, offset + 64, `${role} raw account`)).toBase58(),
    stagingAccount: new PublicKey(readContentId(bytes, offset + 96, `${role} staging account`)).toBase58(),
  });
}

/**
 * Hostile-decode the 400-byte reference-only receipt the adapter persists.
 *
 * The record ordering and the three schema identities are checked exactly as
 * `AdmissionReceiptV2::decode` checks them: a receipt whose coordinates are
 * permuted is refused, not silently reinterpreted.
 */
export function decodeAdmissionReceiptV2(bytes: Uint8Array): AdmissionReceiptV2 {
  requireHeader(bytes, ADMISSION_RECEIPT_MAGIC_V2, ADMISSION_RECEIPT_BYTES_V2, 'admission receipt');
  if (bytes[RECEIPT_COUNT_OFFSET_V2] !== ADMISSION_RECORD_COUNT_V2) {
    throw new Error(`admission receipt declares ${bytes[RECEIPT_COUNT_OFFSET_V2]} records, not the canonical ${ADMISSION_RECORD_COUNT_V2} (NonCanonical)`);
  }
  requireZero(bytes, RECEIPT_RESERVED_OFFSET_V2, RECEIPT_RESERVED_BYTES_V2, 'admission receipt (NonCanonical)');
  return Object.freeze({
    product: decodeCoordinate(bytes, RECEIPT_RECORDS_OFFSET_V2, PRODUCT_RECORD_SCHEMA_ID_V2, 'Product record'),
    resultDomain: decodeCoordinate(bytes, RECEIPT_RECORDS_OFFSET_V2 + RECORD_COORDINATE_BYTES_V2, RESULT_DOMAIN_SCHEMA_ID_V2, 'result-domain record'),
    portfolio: decodeCoordinate(bytes, RECEIPT_RECORDS_OFFSET_V2 + 2 * RECORD_COORDINATE_BYTES_V2, PORTFOLIO_SCHEMA_ID_V2, 'portfolio record'),
  });
}

/**
 * Derive the receipt address the adapter recomputes in `validate_frame`.
 *
 * The seeds are the PDA domain and the three request digests in request order,
 * so a caller cannot pass a receipt for a different admission graph: the
 * program derives this same address and compares.
 */
export function deriveAdmissionReceiptAddressV2(programId: string, digests: AdmissionDigestsV2): Readonly<{ address: string; bump: number }> {
  const [address, bump] = PublicKey.findProgramAddressSync(
    [
      ascii8(ADMISSION_RECEIPT_PDA_DOMAIN_V2),
      contentId(digests.productDigest, 'Product record digest'),
      contentId(digests.resultDomainDigest, 'result-domain record digest'),
      contentId(digests.portfolioDigest, 'portfolio record digest'),
    ],
    new PublicKey(programId),
  );
  return Object.freeze({ address: address.toBase58(), bump });
}

/**
 * Compose the exact 9-account admission instruction.
 *
 * The frame mirrors `validate_frame` and `require_distinct`: the receipt is the
 * only writable account, the Registry is the only executable one, the six
 * record accounts are read-only non-signers, the rent account is the rent
 * sysvar, and no two accounts may be the same key. Refusing here means the
 * browser never hands a wallet a transaction the program is going to reject
 * with `AccountFrame`.
 */
export function buildAdmissionInstructionV2(accounts: AdmissionAccountsV2, digests: AdmissionDigestsV2): AdmissionInstructionV2 {
  const requestBytes = encodeAdmissionRequestV2(digests);
  const receipt = deriveAdmissionReceiptAddressV2(accounts.programId, digests);
  const registry = new PublicKey(accounts.registry);
  const readOnlyRecords = [
    accounts.productRaw, accounts.productStaging,
    accounts.resultDomainRaw, accounts.resultDomainStaging,
    accounts.portfolioRaw, accounts.portfolioStaging,
  ].map((address) => new PublicKey(address));

  const keys: AccountMeta[] = [
    { pubkey: new PublicKey(receipt.address), isSigner: false, isWritable: true },
    { pubkey: registry, isSigner: false, isWritable: false },
    ...readOnlyRecords.map((pubkey) => ({ pubkey, isSigner: false, isWritable: false })),
    { pubkey: SYSVAR_RENT_PUBKEY, isSigner: false, isWritable: false },
  ];
  if (keys.length !== ADMISSION_ACCOUNT_COUNT_V2) {
    throw new Error(`composed ${keys.length} accounts, not the program's exact ${ADMISSION_ACCOUNT_COUNT_V2} (AccountFrame)`);
  }
  const seen = new Set<string>();
  for (const key of keys) {
    const address = key.pubkey.toBase58();
    if (seen.has(address)) throw new Error(`account ${address} appears twice; the program refuses any duplicate (AccountFrame)`);
    seen.add(address);
  }
  return Object.freeze({
    receipt: receipt.address,
    receiptBump: receipt.bump,
    instruction: new TransactionInstruction({ programId: new PublicKey(accounts.programId), keys, data: Buffer.from(requestBytes) }),
    requestBytes,
  });
}

/**
 * Check one observed receipt account against the frame the adapter requires
 * before it will write: program-owned, exactly `ADMISSION_RECEIPT_BYTES_V2`,
 * not executable, and wholly zero.
 *
 * A receipt that already holds bytes is refused by the program, so reporting
 * that here is the difference between "already admitted" and a wasted
 * signature.
 */
export function requireVacantReceiptAccountV2(
  programId: string,
  observed: Readonly<{ owner: string; executable: boolean; data: Uint8Array }>,
): void {
  if (observed.owner !== programId) throw new Error(`receipt account is owned by ${observed.owner}, not the admission program (AccountFrame)`);
  if (observed.executable) throw new Error('receipt account is executable (AccountFrame)');
  if (observed.data.length !== ADMISSION_RECEIPT_BYTES_V2) {
    throw new Error(`receipt account is ${observed.data.length} bytes, not the exact ${ADMISSION_RECEIPT_BYTES_V2} (AccountFrame)`);
  }
  if (!isZero(observed.data)) throw new Error('receipt account already holds bytes; the program refuses a non-vacant receipt (Receipt)');
}

/** The rent sysvar owner the adapter pins, exposed so a caller can pre-check an observation. */
export const RENT_SYSVAR_OWNER_V2 = RENT_SYSVAR_OWNER;
