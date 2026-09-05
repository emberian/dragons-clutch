import { Keypair } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import * as HotAbi from './generated/directInlineV3';
import { deriveFinalizedRecordAddressesV1 } from './releaseRegistry';

const REGISTRY = Keypair.fromSeed(new Uint8Array(32).fill(52)).publicKey.toBase58();

// Transcribed by hand from the publishing authority's own source, so this
// vector is independent of the generator that emits the TS constant:
// crates/dclutch-vm/src/account_profile/lifecycle_v3.rs:44-47,
// `pub const CURRENT_RENT_QUOTE_SCHEMA_RELEASE_ID_V5`. That is the constant
// crates/dclutch-operator/src/rational_lifecycle_hot/selected_bundle_v6.rs:5 imports
// as LIFECYCLE_SCHEMA_ID_V5 and registers the dealer lifecycle artifact under
// at :149, re-validating it at :242. If this reds, the Rust authority moved
// and a human must re-read it -- the generated constant alone cannot say so.
const RUST_AUTHORITY_LIFECYCLE_SCHEMA_V5 = Uint8Array.from([
  0x10, 0xfb, 0xed, 0x6c, 0x13, 0x26, 0x12, 0x7c, 0xf7, 0xe5, 0x47, 0x83, 0xb1, 0xa5, 0x97, 0xd7,
  0x7c, 0xa3, 0xe7, 0x6b, 0x53, 0xde, 0x97, 0xc0, 0x8f, 0x27, 0x3f, 0x5e, 0x67, 0xe3, 0x98, 0x3b,
]);

// Likewise from lifecycle_v3.rs:29-32, `pub const SCHEMA_RELEASE_ID` -- the V3
// generation the dealer route used to key on. Qualified Rust binders of
// `lifecycle_v3::SCHEMA_RELEASE_ID` repo-wide: zero.
const RUST_AUTHORITY_LIFECYCLE_SCHEMA_V3 = Uint8Array.from([
  0xad, 0xfe, 0x22, 0x40, 0x22, 0xdf, 0xb6, 0xff, 0xb2, 0x14, 0xd7, 0xd4, 0x24, 0x83, 0xf9, 0x64,
  0xc9, 0xe0, 0x8b, 0x7f, 0xb1, 0xa2, 0x80, 0x1e, 0x2e, 0x8c, 0x73, 0x8a, 0x34, 0xad, 0x03, 0x0a,
]);

describe('dealer lifecycle Registry key', () => {
  it('keys on the generation the dealer bundle author actually registers', () => {
    // Not a self-comparison: the left side is what the generator scraped, the
    // right side is what a human read in the Rust authority's source.
    expect(HotAbi.SELECTED_LIFECYCLE_SCHEMA_RELEASE_ID_V5)
      .toEqual(RUST_AUTHORITY_LIFECYCLE_SCHEMA_V5);
    expect(HotAbi.LIFECYCLE_SCHEMA_RELEASE_ID)
      .toEqual(RUST_AUTHORITY_LIFECYCLE_SCHEMA_V3);
  });

  it('holds the two lifecycle generations apart', () => {
    // Two independently emitted constants from two different Rust items. Reds
    // if the generator ever collapses them into one value, which would make
    // the fix above silently meaningless.
    expect(HotAbi.SELECTED_LIFECYCLE_SCHEMA_RELEASE_ID_V5)
      .not.toEqual(HotAbi.LIFECYCLE_SCHEMA_RELEASE_ID);
  });

  it('derives a different Registry PDA under each generation', () => {
    // The schema is a PDA seed, so keying on the wrong generation does not
    // merely mislabel the record -- it addresses a different account that no
    // publisher ever writes. This proves the one-line fix is load-bearing.
    const digest = new Uint8Array(32).fill(7);
    const v5 = deriveFinalizedRecordAddressesV1(
      REGISTRY, HotAbi.SELECTED_LIFECYCLE_SCHEMA_RELEASE_ID_V5, digest);
    const v3 = deriveFinalizedRecordAddressesV1(
      REGISTRY, HotAbi.LIFECYCLE_SCHEMA_RELEASE_ID, digest);
    expect(v5.record).not.toEqual(v3.record);
    expect(v5.staging).not.toEqual(v3.staging);
  });
});
