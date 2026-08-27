import { ascii, hex, pubkey, requireNonzero, requireZero, slice, u16 } from './bytes';
import {
  REALM_ADAPTER_RELEASE_ID_OFFSET,
  REALM_BYTES,
  REALM_COLLATERAL_MINT_OFFSET,
  REALM_FREEZE_AUTHORITY_POLICY_OFFSET,
  REALM_MAGIC,
  REALM_MINT_AUTHORITY_POLICY_OFFSET,
  REALM_SCHEMA_VERSION,
  REALM_TOKEN_PROGRAM_OFFSET,
} from './generated/coreFound';

/**
 * The Realm body, as a finalized Registry record.
 *
 * A Market names its Realm by CONTENT IDENTITY. On a live chain the canonical
 * body is published as a finalized Registry record — a Core-owned Realm account
 * is not what a generic founding produces — so this decodes a record body and
 * says nothing about where it was found. The caller reacquires the record at its
 * derived PDA and re-hashes it against the identity the Market committed to;
 * this module owns only the layout, whose offsets are generated from
 * `crates/dclutch-realm-contract/src/lib.rs`.
 */

export type RealmAuthorityPolicy = 'Require absent' | 'Admit issuer control';

export type RealmRecordV1 = Readonly<{
  tokenProgram: string;
  collateralMint: string;
  adapterReleaseId: string;
  mintAuthorityPolicy: RealmAuthorityPolicy;
  freezeAuthorityPolicy: RealmAuthorityPolicy;
}>;

function policy(value: number, field: string): RealmAuthorityPolicy {
  if (value === 0) return 'Require absent';
  if (value === 1) return 'Admit issuer control';
  throw new Error(`${field} policy byte ${value} is undefined`);
}

/** Decode one canonical `DCLTRLM1` Realm body. */
export function decodeRealmRecordV1(bytes: Uint8Array): RealmRecordV1 {
  if (bytes.length !== REALM_BYTES) throw new Error(`Realm body is ${bytes.length} bytes; the exact width is ${REALM_BYTES}`);
  if (ascii(bytes, 0, 8) !== ascii(REALM_MAGIC, 0, 8)) throw new Error(`Realm magic is not ${ascii(REALM_MAGIC, 0, 8)}`);
  const version = u16(bytes, 8);
  if (version !== REALM_SCHEMA_VERSION) throw new Error(`Realm schema version ${version} is unsupported`);
  requireZero(bytes, 12, 4, 'Realm header');
  const adapterRelease = slice(bytes, REALM_ADAPTER_RELEASE_ID_OFFSET, 32);
  requireNonzero(adapterRelease, 'collateral adapter release identity');
  return Object.freeze({
    tokenProgram: pubkey(slice(bytes, REALM_TOKEN_PROGRAM_OFFSET, 32), 'Realm token program'),
    collateralMint: pubkey(slice(bytes, REALM_COLLATERAL_MINT_OFFSET, 32), 'Realm collateral mint'),
    adapterReleaseId: hex(adapterRelease),
    mintAuthorityPolicy: policy(bytes[REALM_MINT_AUTHORITY_POLICY_OFFSET], 'Realm mint authority'),
    freezeAuthorityPolicy: policy(bytes[REALM_FREEZE_AUTHORITY_POLICY_OFFSET], 'Realm freeze authority'),
  });
}
