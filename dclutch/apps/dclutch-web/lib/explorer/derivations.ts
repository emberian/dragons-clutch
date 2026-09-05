/**
 * PDA annotation, by reproduction.
 *
 * An explorer that labels an address "the Claims aggregate" because it looks
 * like one is asserting something it has not checked. This module only ever
 * says a derivation applies when re-deriving it REPRODUCES THE ADDRESS EXACTLY.
 * Every candidate is built from facts the account itself carries — its owner
 * program and its own decoded bytes — so an annotation is a verified statement
 * about the account in hand, not a guess keyed off its shape.
 *
 * Two consequences worth stating plainly:
 *
 *   - A record whose seeds are not recoverable from its own bytes gets NO
 *     annotation, and says so. The Direct Position is the standing example: its
 *     seeds include a maker and an outcome the record does not carry.
 *   - A near-miss is reported as a MISMATCH, not silence. An account that
 *     carries a Market's identity seeds but does not sit at the address those
 *     seeds derive is a finding, and burying it would be the worst outcome
 *     here.
 *
 * Every seed domain is imported from `lib/generated/`; none is written here.
 */
import { PublicKey } from '@solana/web3.js';

import { sha256 } from '@dclutch/sdk/bytes';
import {
  ARTIFACT_RELEASE_SCHEMA_ID_V1,
  CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
  EXECUTION_RELEASE_SET_SCHEMA_RELEASE_ID_V1,
  LIABILITY_BASIS_MARKET_SEED_V2,
  LIABILITY_BASIS_POSITION_SEED_V2,
  MARKET_CORE_STATE_PDA_DOMAIN_V2,
  PORTFOLIO_SCHEMA_ID_V2,
  PRODUCT_RECORD_SCHEMA_ID_V2,
  REALM_SCHEMA_RELEASE_ID_V1,
  RESULT_DOMAIN_SCHEMA_ID_V2,
  SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
} from '@dclutch/sdk/generated/coreFound';
import {
  CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V1,
  CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
  CAPABILITY_PROGRAM_V4_SCHEMA_RELEASE_ID,
  DESCRIPTORCONTRACT_SCHEMA_RELEASE_ID,
  DIRECT_EXECUTION_CONFIG_SCHEMA_ID_V1,
  DIRECT_EXECUTION_REQUEST_SCHEMA_ID_V3,
  DIRECT_ROOT_SCHEMA_ID_V1,
  EFFECT_SCHEMA_RELEASE_ID_V3,
  EFFECT_SCHEMA_RELEASE_ID_V4,
  EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2,
  GRADED_BASIS_RECORD_SCHEMA_ID_V3,
  LIFECYCLE_SCHEMA_RELEASE_ID,
  REQUEST_PROFILE_V2_SCHEMA_RELEASE_ID,
  SELECTED_LIFECYCLE_SCHEMA_RELEASE_ID_V5,
  TRANSITION_SCHEMA_RELEASE_ID,
} from '@dclutch/sdk/generated/directInlineV3';
import {
  PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1,
  PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V2,
} from '@dclutch/sdk/generated/protocolInfrastructure';
import { REALM_PDA_DOMAIN_V1 } from '@dclutch/sdk/generated/realmPositionV1';
import { deriveFinalizedRecordAddressesV1 } from '@dclutch/sdk/releaseRegistry';
import { magicText, type DecodedRecord } from './accountRecords';

/** One derivation the explorer checked against the address in hand. */
export type Derivation = Readonly<{
  /** What this address would be, if it derives. */
  name: string;
  /** The seeds, described the way a reader can re-run them. */
  seeds: ReadonlyArray<string>;
  /** The program the seeds derive under. */
  program: string;
  derived: string;
  /** Whether `derived` is the address in hand. */
  matches: boolean;
  /** The PDA bump the derivation found. */
  bump: number;
}>;

function derive(
  name: string,
  seedBytes: ReadonlyArray<Uint8Array>,
  seedLabels: ReadonlyArray<string>,
  program: string,
  address: string,
): Derivation | null {
  try {
    const [key, bump] = PublicKey.findProgramAddressSync([...seedBytes], new PublicKey(program));
    const derived = key.toBase58();
    return Object.freeze({
      name,
      seeds: Object.freeze([...seedLabels]),
      program,
      derived,
      matches: derived === address,
      bump,
    });
  } catch {
    return null;
  }
}

function hexOf(bytes: Uint8Array): string {
  return Array.from(bytes, (byte) => byte.toString(16).padStart(2, '0')).join('');
}

/**
 * A seed domain, as text, read back out of the emitted constant.
 *
 * The reader needs to see the domain to re-run a derivation by hand, and the
 * obvious way to show it is to type it into the label. That would make this
 * file a second authority for a string a Lean schema owns — the exact
 * hand-mirror `lib/abiCoverage.test.ts` ratchets down — and it would go on
 * agreeing with the schema right up until the schema moved. Decoding the
 * imported bytes shows the same text and cannot drift from them.
 */
function seedLabel(domain: Uint8Array): string {
  return new TextDecoder().decode(domain);
}

function short(value: string, edge = 6): string {
  return value.length <= edge * 2 + 1 ? value : `${value.slice(0, edge)}…${value.slice(-edge)}`;
}

function u64Seed(value: string): Uint8Array {
  const bytes = new Uint8Array(8);
  new DataView(bytes.buffer).setBigUint64(0, BigInt(value), true);
  return bytes;
}

function fieldBytes(decoded: DecodedRecord, label: string, data: Uint8Array): Uint8Array | null {
  const held = decoded.fields.find((entry) => entry.label === label);
  if (held === undefined) return null;
  if (held.offset + held.bytes > data.length) return null;
  return data.slice(held.offset, held.offset + held.bytes);
}

function fieldScalar(decoded: DecodedRecord, label: string): string | null {
  const held = decoded.fields.find((entry) => entry.label === label);
  return held !== undefined && held.value.form === 'scalar' ? held.value.text : null;
}

/**
 * Every schema identity the generated modules emit, so a finalized record
 * account can be told what schema it holds by reproduction rather than by
 * guessing from its bytes.
 */
const SCHEMA_IDS: ReadonlyArray<Readonly<{ name: string; id: Uint8Array }>> = Object.freeze([
  { name: 'Realm', id: REALM_SCHEMA_RELEASE_ID_V1 },
  { name: 'Product record', id: PRODUCT_RECORD_SCHEMA_ID_V2 },
  { name: 'Result domain', id: RESULT_DOMAIN_SCHEMA_ID_V2 },
  { name: 'Portfolio', id: PORTFOLIO_SCHEMA_ID_V2 },
  { name: 'Source material', id: SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3 },
  { name: 'Capability manifest', id: CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1 },
  { name: 'Execution release set', id: EXECUTION_RELEASE_SET_SCHEMA_RELEASE_ID_V1 },
  { name: 'Artifact release', id: ARTIFACT_RELEASE_SCHEMA_ID_V1 },
  { name: 'Graded basis', id: GRADED_BASIS_RECORD_SCHEMA_ID_V3 },
  { name: 'Direct execution request', id: DIRECT_EXECUTION_REQUEST_SCHEMA_ID_V3 },
  { name: 'Direct execution config', id: DIRECT_EXECUTION_CONFIG_SCHEMA_ID_V1 },
  { name: 'Direct root', id: DIRECT_ROOT_SCHEMA_ID_V1 },
  { name: 'Descriptor contract', id: DESCRIPTORCONTRACT_SCHEMA_RELEASE_ID },
  { name: 'Capability program set V1', id: CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V1 },
  { name: 'Capability program set V2', id: CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2 },
  { name: 'Capability program V4', id: CAPABILITY_PROGRAM_V4_SCHEMA_RELEASE_ID },
  { name: 'Request profile V2', id: REQUEST_PROFILE_V2_SCHEMA_RELEASE_ID },
  { name: 'Transition', id: TRANSITION_SCHEMA_RELEASE_ID },
  { name: 'Effect V3', id: EFFECT_SCHEMA_RELEASE_ID_V3 },
  { name: 'Effect V4', id: EFFECT_SCHEMA_RELEASE_ID_V4 },
  // Three generations of state-lifecycle policy exist; this table names the
  // one whose id it actually holds, so a V5 record is not mis-labelled by an
  // unversioned row sitting above it.
  { name: 'Lifecycle V3', id: LIFECYCLE_SCHEMA_RELEASE_ID },
  { name: 'Selected lifecycle V5', id: SELECTED_LIFECYCLE_SCHEMA_RELEASE_ID_V5 },
  { name: 'Execution strategy program', id: EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2 },
]);

export type RecordIdentification = Readonly<{
  /** SHA-256 of the account's exact bytes — its content identity if it is a record. */
  contentDigest: string;
  /** The schema whose raw-record PDA reproduces this address, when one does. */
  schema: string | null;
  /** The Registry program the reproduction was run under. */
  registryProgram: string;
  stagingAddress: string | null;
}>;

/**
 * Whether this account is a finalized content-addressed record, and of what.
 *
 * The test is exact: hash the account's own bytes, then for every schema the
 * generated modules emit, derive `[raw-record-v1, schema, digest]` under the
 * account's owner and see whether it lands on this address. One schema can
 * match; none can. Nothing is inferred from the content.
 */
export async function identifyFinalizedRecord(
  address: string,
  owner: string,
  data: Uint8Array,
): Promise<RecordIdentification | null> {
  if (data.length === 0) return null;
  let digest: Uint8Array;
  try {
    digest = await sha256(data);
  } catch {
    return null;
  }
  for (const schema of SCHEMA_IDS) {
    let addresses: Readonly<{ record: string; staging: string }>;
    try {
      addresses = deriveFinalizedRecordAddressesV1(owner, schema.id, digest);
    } catch {
      continue;
    }
    if (addresses.record === address) {
      return Object.freeze({
        contentDigest: hexOf(digest),
        schema: schema.name,
        registryProgram: owner,
        stagingAddress: addresses.staging,
      });
    }
  }
  return Object.freeze({
    contentDigest: hexOf(digest),
    schema: null,
    registryProgram: owner,
    stagingAddress: null,
  });
}

/**
 * Derivations reproducible from a decoded record's own bytes.
 *
 * Each entry is checked. `matches: false` is kept and shown, because an account
 * that carries seeds it does not sit at is a finding.
 */
export function derivationsForRecord(
  decoded: DecodedRecord,
  data: Uint8Array,
  address: string,
  owner: string,
): ReadonlyArray<Derivation> {
  const found: Array<Derivation | null> = [];
  const magic = decoded.magic;
  const spec = decoded.spec;

  // The Market Core state derives from its own nine identity seeds, so an
  // account claiming to be a Market can be checked against itself with nothing
  // else supplied.
  if (magic === magicText(spec.magic) && spec.name === 'Market Core state') {
    const seedLabels = [
      'Realm identity',
      'Product record identity',
      'Product instance identity',
      'Resolution policy identity',
      'Capability manifest identity',
      'Selected release set',
      'Registry program',
    ];
    const seeds = seedLabels.map((label) => fieldBytes(decoded, label, data));
    const generation = fieldScalar(decoded, 'Generation');
    if (seeds.every((seed) => seed !== null) && generation !== null) {
      found.push(
        derive(
          'Market Core state',
          [MARKET_CORE_STATE_PDA_DOMAIN_V2, ...(seeds as Uint8Array[]), u64Seed(generation)],
          [
            seedLabel(MARKET_CORE_STATE_PDA_DOMAIN_V2),
            ...seedLabels,
            `generation ${generation}`,
          ],
          owner,
          address,
        ),
      );
    }
  }

  if (spec.name === 'Claims aggregate') {
    const market = decoded.fields.find((entry) => entry.label === 'Logical Market');
    if (market?.value.form === 'address') {
      found.push(
        derive(
          'Claims aggregate',
          [LIABILITY_BASIS_MARKET_SEED_V2, new PublicKey(market.value.base58).toBytes()],
          [seedLabel(LIABILITY_BASIS_MARKET_SEED_V2), `Market ${short(market.value.base58)}`],
          owner,
          address,
        ),
      );
    }
  }

  if (spec.name === 'Claims position') {
    const aggregate = decoded.fields.find((entry) => entry.label === 'Claims aggregate');
    const holder = decoded.fields.find((entry) => entry.label === 'Owner');
    if (aggregate?.value.form === 'address' && holder?.value.form === 'address') {
      found.push(
        derive(
          'Claims position',
          [
            LIABILITY_BASIS_POSITION_SEED_V2,
            new PublicKey(aggregate.value.base58).toBytes(),
            new PublicKey(holder.value.base58).toBytes(),
          ],
          [
            seedLabel(LIABILITY_BASIS_POSITION_SEED_V2),
            `aggregate ${short(aggregate.value.base58)}`,
            `owner ${short(holder.value.base58)}`,
          ],
          owner,
          address,
        ),
      );
    }
  }

  // Both profile domains are offered for either record, because both accounts
  // live on chain at once: the succession profile is what every route reads,
  // and its predecessor stays written forever at its own address.
  if (spec.name === 'Protocol infrastructure profile' || spec.name === 'Protocol infrastructure succession profile') {
    found.push(
      derive(
        'Protocol infrastructure profile',
        [PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1],
        [seedLabel(PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1)],
        owner,
        address,
      ),
    );
    found.push(
      derive(
        'Protocol infrastructure succession profile',
        [PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V2],
        [seedLabel(PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V2)],
        owner,
        address,
      ),
    );
  }

  return Object.freeze(found.filter((entry): entry is Derivation => entry !== null));
}

/**
 * The content-addressed Realm PDA, when a Realm body is held as a Core account
 * rather than as a Registry record. Needs the digest, so it is async and
 * separate from the synchronous set above.
 */
export async function realmContentDerivation(
  data: Uint8Array,
  address: string,
  owner: string,
): Promise<Derivation | null> {
  try {
    const digest = await sha256(data);
    return derive(
      'Realm, as a Core-owned content-addressed account',
      [REALM_PDA_DOMAIN_V1, digest],
      [seedLabel(REALM_PDA_DOMAIN_V1), `sha256(account bytes) ${short(hexOf(digest), 8)}`],
      owner,
      address,
    );
  } catch {
    return null;
  }
}
