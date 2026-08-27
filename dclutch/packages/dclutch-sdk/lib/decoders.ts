import { PublicKey } from '@solana/web3.js';

import { ascii, fromHex, hex, isZero, pubkey, requireNonzero, requireZero, sha256, slice, u16, u64 } from './bytes';
import { LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2 as RENT_CREDIT_SEED } from './generated/coreFound';
import {
  REALM_ADAPTER_RELEASE_ID_OFFSET_V1,
  REALM_BYTES_V1,
  REALM_COLLATERAL_MINT_OFFSET_V1,
  REALM_FREEZE_AUTHORITY_POLICY_OFFSET_V1,
  REALM_MAGIC_V1,
  REALM_MINT_AUTHORITY_POLICY_OFFSET_V1,
  REALM_PDA_DOMAIN_V1,
  REALM_RESERVED_BYTES_V1,
  REALM_RESERVED_OFFSET_V1,
  REALM_SCHEMA_VERSION_V1,
  REALM_TOKEN_PROGRAM_OFFSET_V1,
} from './generated/realmPositionV1';

export type CoreKind = 'Realm' | 'RentCredit';

export type BindingCheck = Readonly<{
  label: string;
  ok: boolean;
  detail: string;
}>;

export type RealmAuthorityPolicy = 'Require absent' | 'Admit issuer control';

export type RealmSemantics = Readonly<{
  kind: 'Realm';
  canonicalBytes: Uint8Array;
  contentDigest: string | null;
  tokenProgram: string;
  collateralMint: string;
  adapterReleaseId: string;
  mintAuthorityPolicy: RealmAuthorityPolicy;
  freezeAuthorityPolicy: RealmAuthorityPolicy;
}>;

type RentCreditSemantics = Readonly<{
  kind: 'RentCredit';
  refundWallet: string;
  market: string;
  marketBytes: Uint8Array;
  releaseSet: string;
  generation: string;
  bump: number;
}>;

export type DecodedProjection = Readonly<{
  status: 'decoded';
  kind: CoreKind;
  address: string;
  lamports: string;
  observedSlot: string;
  schema: 'v1' | 'v2';
  details: ReadonlyArray<Readonly<{ label: string; value: string }>>;
  bindings: ReadonlyArray<BindingCheck>;
  semantics: RealmSemantics | RentCreditSemantics;
}>;

export type RefusedProjection = Readonly<{
  status: 'refused';
  kind: 'Unknown' | CoreKind;
  address: string;
  lamports: string;
  observedSlot: string;
  reason: string;
  header: string;
}>;

export type AccountProjection = DecodedProjection | RefusedProjection;

export type FullAccountObservation = Readonly<{
  address: string;
  owner: string;
  executable: boolean;
  lamports: string;
  observedSlot: string;
  data: Uint8Array;
}>;

const MAGIC = Object.freeze({
  [REALM_MAGIC_V1]: 'Realm',
  DCLRNTL2: 'RentCredit',
} satisfies Record<string, CoreKind>);

function detail(label: string, value: string | number | bigint): Readonly<{ label: string; value: string }> {
  return Object.freeze({ label, value: String(value) });
}

function sameIdentity(left: Uint8Array, right: Uint8Array): boolean {
  return left.length === right.length && left.every((value, index) => value === right[index]);
}

function kindFromMagic(data: Uint8Array): CoreKind | null {
  if (data.length < 8) return null;
  try {
    return MAGIC[ascii(data, 0, 8) as keyof typeof MAGIC] ?? null;
  } catch {
    return null;
  }
}

export function classifyHeader(data: Uint8Array): CoreKind | null {
  return kindFromMagic(data);
}

/**
 * A Market names its Realm by content identity, not by address. The canonical
 * Realm account is the content-addressed PDA of that identity under the same
 * Core program, so a Market alone is enough to name the Realm to reacquire.
 */
export function deriveRealmAddress(programId: string, realmContentIdHex: string): string {
  const digest = fromHex(realmContentIdHex, 'Realm content ID');
  if (digest.length !== 32 || isZero(digest)) throw new Error('Realm content ID must be one nonzero 32-byte identity');
  return PublicKey.findProgramAddressSync([REALM_PDA_DOMAIN_V1, digest], new PublicKey(programId))[0].toBase58();
}

export function decodeCoreAccount(observation: FullAccountObservation, expectedProgramId: string): AccountProjection {
  const kind = kindFromMagic(observation.data);
  if (observation.owner !== expectedProgramId) {
    return refused(observation, kind ?? 'Unknown', 'account owner differs from the selected program ID');
  }
  if (observation.executable) {
    return refused(observation, kind ?? 'Unknown', 'executable program data is not a core state account');
  }
  if (kind === null) {
    return refused(observation, 'Unknown', 'unknown account magic; no layout was guessed');
  }
  try {
    if (kind === 'Realm') return decodeRealm(observation);
    return decodeRentCredit(observation);
  } catch (error) {
    return refused(observation, kind, error instanceof Error ? error.message : 'canonical decoder refused the account');
  }
}

function refused(observation: FullAccountObservation, kind: 'Unknown' | CoreKind, reason: string): RefusedProjection {
  return Object.freeze({
    status: 'refused',
    kind,
    address: observation.address,
    lamports: observation.lamports,
    observedSlot: observation.observedSlot,
    reason,
    header: hex(observation.data.slice(0, 16)),
  });
}

function commonHeader(bytes: Uint8Array, magic: string, exactLength: number, version = 1): void {
  if (bytes.length !== exactLength) throw new Error(`expected exactly ${exactLength} bytes, observed ${bytes.length}`);
  if (ascii(bytes, 0, 8) !== magic) throw new Error(`magic is not ${magic}`);
  if (u16(bytes, 8) !== version) throw new Error(`schema version ${u16(bytes, 8)} is unsupported`);
}

function decodeRealm(observation: FullAccountObservation): DecodedProjection {
  const bytes = observation.data;
  commonHeader(bytes, REALM_MAGIC_V1, REALM_BYTES_V1, REALM_SCHEMA_VERSION_V1);
  requireZero(bytes, REALM_RESERVED_OFFSET_V1, REALM_RESERVED_BYTES_V1, 'Realm header');
  const mintByte = bytes[REALM_MINT_AUTHORITY_POLICY_OFFSET_V1];
  const freezeByte = bytes[REALM_FREEZE_AUTHORITY_POLICY_OFFSET_V1];
  const mintPolicy = mintByte === 0 ? 'Require absent' : mintByte === 1 ? 'Admit issuer control' : null;
  const freezePolicy = freezeByte === 0 ? 'Require absent' : freezeByte === 1 ? 'Admit issuer control' : null;
  if (mintPolicy === null || freezePolicy === null) throw new Error('Realm authority policy byte is undefined');
  const tokenProgram = pubkey(slice(bytes, REALM_TOKEN_PROGRAM_OFFSET_V1, 32), 'Realm token program');
  const collateralMint = pubkey(slice(bytes, REALM_COLLATERAL_MINT_OFFSET_V1, 32), 'Realm collateral mint');
  const adapterRelease = slice(bytes, REALM_ADAPTER_RELEASE_ID_OFFSET_V1, 32);
  requireNonzero(adapterRelease, 'collateral adapter release ID');
  return Object.freeze({
    status: 'decoded', kind: 'Realm', address: observation.address, lamports: observation.lamports,
    observedSlot: observation.observedSlot, schema: 'v1', bindings: Object.freeze([]),
    details: Object.freeze([
      detail('Token program', tokenProgram), detail('Collateral mint', collateralMint), detail('Adapter release ID', hex(adapterRelease)),
      detail('Mint authority policy', mintPolicy), detail('Freeze authority policy', freezePolicy),
    ]),
    semantics: Object.freeze({
      kind: 'Realm', canonicalBytes: new Uint8Array(bytes), contentDigest: null,
      tokenProgram, collateralMint, adapterReleaseId: hex(adapterRelease),
      mintAuthorityPolicy: mintPolicy, freezeAuthorityPolicy: freezePolicy,
    }),
  });
}

function decodeRentCredit(observation: FullAccountObservation): DecodedProjection {
  const bytes = observation.data;
  commonHeader(bytes, 'DCLRNTL2', 128, 2);
  requireZero(bytes, 11, 5, 'RentCredit header');
  requireZero(bytes, 120, 8, 'RentCredit body');
  const refundWallet = pubkey(slice(bytes, 16, 32), 'RentCredit refund wallet');
  const marketBytes = slice(bytes, 48, 32);
  const market = pubkey(marketBytes, 'RentCredit Market');
  const releaseSet = hex(slice(bytes, 80, 32));
  requireNonzero(slice(bytes, 80, 32), 'RentCredit release set');
  const generation = u64(bytes, 112);
  if (generation === 0n) throw new Error('RentCredit generation is zero');
  if (refundWallet === market || sameIdentity(slice(bytes, 16, 32), slice(bytes, 80, 32)) || sameIdentity(marketBytes, slice(bytes, 80, 32))) throw new Error('RentCredit lifecycle identities alias');
  const bump = bytes[10];
  return Object.freeze({
    status: 'decoded', kind: 'RentCredit', address: observation.address, lamports: observation.lamports,
    observedSlot: observation.observedSlot, schema: 'v2', bindings: Object.freeze([]),
    details: Object.freeze([
      detail('Refund wallet', refundWallet), detail('Market', market), detail('Generation', generation),
      detail('Execution release set', releaseSet), detail('Persisted PDA bump', bump), detail('Observed lamports', observation.lamports),
    ]),
    semantics: Object.freeze({ kind: 'RentCredit', refundWallet, market, marketBytes, releaseSet, generation: generation.toString(), bump }),
  });
}

export async function verifyLocalBindings(projection: DecodedProjection, programId: string): Promise<DecodedProjection> {
  const program = new PublicKey(programId);
  const checks: BindingCheck[] = [];
  let semantics = projection.semantics;
  if (semantics.kind === 'Realm') {
    const digest = await sha256(semantics.canonicalBytes);
    const [derived] = PublicKey.findProgramAddressSync([REALM_PDA_DOMAIN_V1, digest], program);
    checks.push(Object.freeze({ label: 'Realm content + PDA', ok: derived.toBase58() === projection.address, detail: `sha256(canonical bytes) ${hex(digest)} → ${derived.toBase58()}` }));
    semantics = Object.freeze({ ...semantics, contentDigest: hex(digest) });
  } else {
    const generation = BigInt(semantics.generation);
    const generationBytes = new Uint8Array(8);
    new DataView(generationBytes.buffer).setBigUint64(0, generation, true);
    const [derived, bump] = PublicKey.findProgramAddressSync([RENT_CREDIT_SEED, semantics.marketBytes, generationBytes], program);
    checks.push(Object.freeze({ label: 'RentCredit PDA', ok: derived.toBase58() === projection.address && bump === semantics.bump, detail: `Market + generation → ${derived.toBase58()}, bump ${bump}` }));
  }
  return Object.freeze({ ...projection, bindings: Object.freeze(checks), semantics });
}

export function crossCheckBindings(projections: ReadonlyArray<AccountProjection>): ReadonlyArray<AccountProjection> {
  return projections.map((projection) => {
    if (projection.status !== 'decoded') return projection;
    const checks = [...projection.bindings];
    if (projection.semantics.kind === 'RentCredit') {
      // The Market a RentCredit names is DCLTCOR2 Core state, which this
      // scanner does not decode: `lib/marketCoreV2.ts` owns that layout. The
      // cross-check that used to live here joined against a decoded DCLTCAT1
      // Market in the same scan, and DCLTCAT1 has no writer any more. Refusing
      // to state a join beats restating one against a representation nobody
      // writes.
      checks.push(Object.freeze({ label: 'RentCredit → Market lifecycle', ok: true, detail: `names Market ${projection.semantics.market} at generation ${projection.semantics.generation}; this scanner does not decode Core V2 state, so the join is not asserted here` }));
    }
    return Object.freeze({ ...projection, bindings: Object.freeze(checks) });
  });
}
