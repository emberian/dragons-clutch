import { PublicKey, SYSVAR_RENT_PUBKEY } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import vector from '../fixtures/product-runtime-v2-admission-wire.json';
import { fromHex, hex, sha256 } from './bytes';
import {
  ADMISSION_ACCOUNT_COUNT_V2,
  ADMISSION_RECEIPT_PDA_DOMAIN_V2,
  ADMISSION_REQUEST_BYTES_V2,
  PORTFOLIO_SCHEMA_ID_V2,
  PORTFOLIO_SCHEMA_PREIMAGE_V2,
  PRODUCT_RECORD_SCHEMA_ID_V2,
  PRODUCT_RECORD_SCHEMA_PREIMAGE_V2,
  RESULT_DOMAIN_SCHEMA_ID_V2,
  RESULT_DOMAIN_SCHEMA_PREIMAGE_V2,
} from './generated/productRuntimeV2Admission';
import {
  buildAdmissionInstructionV2,
  decodeAdmissionReceiptV2,
  decodeAdmissionRequestV2,
  decodeProductRecordV2,
  deriveAdmissionReceiptAddressV2,
  encodeAdmissionRequestV2,
  requireVacantReceiptAccountV2,
} from './productRuntimeV2Admission';

/**
 * These are witness tests, not coverage. Each one reproduces a refusal that
 * `crates/dclutch-product-runtime-v2-admission` and
 * `programs/dclutch-product-runtime-v2-sbf` actually raise, so that the browser
 * refuses precisely what the deployed program refuses -- no wider, and above
 * all no narrower.
 */

const bytes = (value: string) => Uint8Array.from((value.match(/../g) ?? []), (pair) => Number.parseInt(pair, 16));
const REQUEST = bytes(vector.requestHex);
const PRODUCT_RECORD = bytes(vector.productRecordHex);
const RECEIPT = bytes(vector.receiptHex);

const DIGESTS = Object.freeze({
  productDigest: PRODUCT_RECORD_SCHEMA_ID_V2,
  resultDomainDigest: RESULT_DOMAIN_SCHEMA_ID_V2,
  portfolioDigest: PORTFOLIO_SCHEMA_ID_V2,
});

const PROGRAM_ID = new PublicKey(new Uint8Array(32).fill(7)).toBase58();
const ACCOUNTS = Object.freeze({
  programId: PROGRAM_ID,
  registry: new PublicKey(new Uint8Array(32).fill(11)).toBase58(),
  productRaw: new PublicKey(new Uint8Array(32).fill(21)).toBase58(),
  productStaging: new PublicKey(new Uint8Array(32).fill(22)).toBase58(),
  resultDomainRaw: new PublicKey(new Uint8Array(32).fill(23)).toBase58(),
  resultDomainStaging: new PublicKey(new Uint8Array(32).fill(24)).toBase58(),
  portfolioRaw: new PublicKey(new Uint8Array(32).fill(25)).toBase58(),
  portfolioStaging: new PublicKey(new Uint8Array(32).fill(26)).toBase58(),
});

function mutate(source: Uint8Array, offset: number, value: number): Uint8Array {
  const copy = new Uint8Array(source);
  copy[offset] = value;
  return copy;
}

describe('Product Runtime V2 admission wire — agreement with the live Rust encoders', () => {
  it('encodes the exact bytes the Rust crate encodes', () => {
    // The other side of this equality is
    // crates/dclutch-product-runtime-v2-admission/tests/browser_wire_vector.rs.
    // One vector, two independent producers; the crate is the authority.
    expect(hex(encodeAdmissionRequestV2(DIGESTS))).toBe(vector.requestHex);
    expect(REQUEST.length).toBe(ADMISSION_REQUEST_BYTES_V2);
  });

  it('leaves the reserved span zero — the one byte the dead DCLTPRQ2 encoder set', () => {
    // Two incompatible 112-byte requests share this magic. The dead evaluator
    // request wrote 1 at byte 10; the live decoder requires zero across 10..16.
    expect(Array.from(REQUEST.slice(10, 16))).toEqual([0, 0, 0, 0, 0, 0]);
    expect(() => decodeAdmissionRequestV2(mutate(REQUEST, 10, 1))).toThrow(/NonCanonical/);
  });

  it('round-trips its own request', () => {
    const decoded = decodeAdmissionRequestV2(REQUEST);
    expect(hex(decoded.productDigest)).toBe(hex(PRODUCT_RECORD_SCHEMA_ID_V2));
    expect(hex(decoded.resultDomainDigest)).toBe(hex(RESULT_DOMAIN_SCHEMA_ID_V2));
    expect(hex(decoded.portfolioDigest)).toBe(hex(PORTFOLIO_SCHEMA_ID_V2));
  });

  it('holds the schema identities the crate documents, as digests of their preimages', async () => {
    // Not hex blobs: each generated identity is re-derived here from the label
    // the crate hashes, so a copied-wrong byte cannot survive.
    for (const [preimage, identity] of [
      [PRODUCT_RECORD_SCHEMA_PREIMAGE_V2, PRODUCT_RECORD_SCHEMA_ID_V2],
      [RESULT_DOMAIN_SCHEMA_PREIMAGE_V2, RESULT_DOMAIN_SCHEMA_ID_V2],
      [PORTFOLIO_SCHEMA_PREIMAGE_V2, PORTFOLIO_SCHEMA_ID_V2],
    ] as const) {
      expect(hex(await sha256(new TextEncoder().encode(preimage)))).toBe(hex(identity));
    }
  });
});

describe('Product Runtime V2 admission wire — the decoder’s refusals, witnessed', () => {
  it('refuses any width but the exact one (InvalidLength)', () => {
    expect(() => decodeAdmissionRequestV2(REQUEST.slice(0, ADMISSION_REQUEST_BYTES_V2 - 1))).toThrow(/InvalidLength/);
    expect(() => decodeAdmissionRequestV2(new Uint8Array(ADMISSION_REQUEST_BYTES_V2 + 1))).toThrow(/InvalidLength/);
  });

  it('refuses another protocol’s magic or another schema version (UnsupportedSchema)', () => {
    expect(() => decodeAdmissionRequestV2(mutate(REQUEST, 7, 0x33))).toThrow(/UnsupportedSchema/);
    expect(() => decodeAdmissionRequestV2(mutate(REQUEST, 8, 1))).toThrow(/UnsupportedSchema/);
    expect(() => decodeAdmissionRequestV2(mutate(REQUEST, 8, 3))).toThrow(/UnsupportedSchema/);
  });

  it('refuses every byte of the reserved span, not just the first (NonCanonical)', () => {
    for (let offset = 10; offset < 16; offset += 1) {
      expect(() => decodeAdmissionRequestV2(mutate(REQUEST, offset, 0xff))).toThrow(/NonCanonical/);
    }
  });

  it('refuses the all-zero content identity on both sides (ContentId::new)', () => {
    expect(() => encodeAdmissionRequestV2({ ...DIGESTS, portfolioDigest: new Uint8Array(32) })).toThrow(/all-zero/);
    const zeroed = new Uint8Array(REQUEST);
    zeroed.fill(0, 48, 80);
    expect(() => decodeAdmissionRequestV2(zeroed)).toThrow(/all-zero/);
  });

  it('refuses a digest that is not 32 bytes', () => {
    expect(() => encodeAdmissionRequestV2({ ...DIGESTS, productDigest: new Uint8Array(31).fill(1) })).toThrow(/32-byte/);
  });
});

describe('Product Runtime V2 admission — the persisted records', () => {
  it('decodes the Product record the crate encodes', () => {
    const record = decodeProductRecordV2(PRODUCT_RECORD);
    expect(hex(record.productId)).toBe(vector.inputs.productRecordProductId);
    expect(hex(record.resultDomainDigest)).toBe(hex(RESULT_DOMAIN_SCHEMA_ID_V2));
    expect(hex(record.portfolioDigest)).toBe(hex(PORTFOLIO_SCHEMA_ID_V2));
    expect(() => decodeProductRecordV2(mutate(PRODUCT_RECORD, 10, 1))).toThrow(/NonCanonical/);
  });

  it('decodes the reference-only receipt in canonical record order', () => {
    const receipt = decodeAdmissionReceiptV2(RECEIPT);
    expect(hex(receipt.product.schemaId)).toBe(hex(PRODUCT_RECORD_SCHEMA_ID_V2));
    expect(hex(receipt.resultDomain.schemaId)).toBe(hex(RESULT_DOMAIN_SCHEMA_ID_V2));
    expect(hex(receipt.portfolio.schemaId)).toBe(hex(PORTFOLIO_SCHEMA_ID_V2));
    expect(hex(receipt.product.contentDigest)).toBe('21'.repeat(32));
    expect(hex(receipt.portfolio.contentDigest)).toBe('23'.repeat(32));
    expect(receipt.resultDomain.rawAccount).toBe(new PublicKey(new Uint8Array(32).fill(0x32)).toBase58());
  });

  it('refuses a receipt whose record count is not the canonical three (NonCanonical)', () => {
    expect(() => decodeAdmissionReceiptV2(mutate(RECEIPT, 10, 2))).toThrow(/NonCanonical/);
    expect(() => decodeAdmissionReceiptV2(mutate(RECEIPT, 11, 1))).toThrow(/NonCanonical/);
  });

  it('refuses a receipt whose coordinates are permuted rather than reinterpreting them', () => {
    // Swapping the Product and result-domain coordinates keeps every byte and
    // every width. Only the pinned schema identities catch it.
    const permuted = new Uint8Array(RECEIPT);
    permuted.set(RECEIPT.slice(144, 272), 16);
    permuted.set(RECEIPT.slice(16, 144), 144);
    expect(() => decodeAdmissionReceiptV2(permuted)).toThrow(/NonCanonical/);
  });
});

describe('Product Runtime V2 admission — the account frame the adapter validates', () => {
  it('derives the receipt address from the domain and the three digests, in request order', () => {
    const derived = deriveAdmissionReceiptAddressV2(PROGRAM_ID, DIGESTS);
    const expected = PublicKey.findProgramAddressSync(
      [
        new TextEncoder().encode(ADMISSION_RECEIPT_PDA_DOMAIN_V2),
        PRODUCT_RECORD_SCHEMA_ID_V2, RESULT_DOMAIN_SCHEMA_ID_V2, PORTFOLIO_SCHEMA_ID_V2,
      ],
      new PublicKey(PROGRAM_ID),
    );
    expect(derived.address).toBe(expected[0].toBase58());
    expect(derived.bump).toBe(expected[1]);
  });

  it('binds the receipt to its exact admission graph', () => {
    const other = deriveAdmissionReceiptAddressV2(PROGRAM_ID, { ...DIGESTS, portfolioDigest: fromHex('44'.repeat(32), 'other portfolio') });
    expect(other.address).not.toBe(deriveAdmissionReceiptAddressV2(PROGRAM_ID, DIGESTS).address);
  });

  it('composes exactly nine accounts with the adapter’s exact privileges', () => {
    const built = buildAdmissionInstructionV2(ACCOUNTS, DIGESTS);
    expect(built.instruction.keys.length).toBe(ADMISSION_ACCOUNT_COUNT_V2);
    expect(built.instruction.keys.map((key) => key.isWritable)).toEqual([true, false, false, false, false, false, false, false, false]);
    expect(built.instruction.keys.every((key) => !key.isSigner)).toBe(true);
    expect(built.instruction.keys[0].pubkey.toBase58()).toBe(built.receipt);
    expect(built.instruction.keys[1].pubkey.toBase58()).toBe(ACCOUNTS.registry);
    expect(built.instruction.keys[8].pubkey.toBase58()).toBe(SYSVAR_RENT_PUBKEY.toBase58());
    expect(hex(new Uint8Array(built.instruction.data))).toBe(vector.requestHex);
  });

  it('refuses a duplicate account before a wallet ever sees the transaction (AccountFrame)', () => {
    expect(() => buildAdmissionInstructionV2({ ...ACCOUNTS, portfolioStaging: ACCOUNTS.productRaw }, DIGESTS))
      .toThrow(/appears twice/);
  });

  it('refuses a receipt account that is not the program’s vacant exact-width account', () => {
    const vacant = { owner: PROGRAM_ID, executable: false, data: new Uint8Array(400) };
    expect(() => requireVacantReceiptAccountV2(PROGRAM_ID, vacant)).not.toThrow();
    expect(() => requireVacantReceiptAccountV2(PROGRAM_ID, { ...vacant, owner: ACCOUNTS.registry })).toThrow(/AccountFrame/);
    expect(() => requireVacantReceiptAccountV2(PROGRAM_ID, { ...vacant, executable: true })).toThrow(/AccountFrame/);
    expect(() => requireVacantReceiptAccountV2(PROGRAM_ID, { ...vacant, data: new Uint8Array(399) })).toThrow(/AccountFrame/);
    expect(() => requireVacantReceiptAccountV2(PROGRAM_ID, { ...vacant, data: mutate(new Uint8Array(400), 0, 0xee) })).toThrow(/non-vacant/);
  });
});
