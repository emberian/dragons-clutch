import { PublicKey } from '@solana/web3.js';

import { ascii, hex, isZero, requireZero, slice, u16 } from './bytes';
import { REGISTRY_ROLES, type RegistryRole } from './releaseRegistry';
import { type SolanaRpcClient } from './rpc';
import {
  RELEASE_LINEAGE_MAGIC_V1,
  RELEASE_LINEAGE_PDA_DOMAIN_V1,
} from './generated/protocolConstantsV1';

/**
 * Release-set lineage, read side.
 *
 * A market's `selectedReleaseSetId` is its NAME — seed component 6 of 9 — so it
 * can never be rewritten, and every cut therefore leaves the markets founded
 * before it pinned to a set the world has moved off. `ReleaseLineageV1` is the
 * one record that says where a set went next: a 248-byte Registry-owned account
 * keyed by the PREDECESSOR, so a reader who knows only a market's founding pin
 * can derive its address and read the successor out.
 *
 * One record is one hop. This module walks them into a history.
 *
 * The layout below mirrors `crates/dclutch-registry/src/lineage.rs`
 * and the walk mirrors `lineage_walk.rs`, refusal for refusal. Neither is
 * generated: no emitter covers this record yet, which is why
 * `apps/dclutch-web/scripts/explorer-coverage.exempt.json` still exempts
 * `DCLRLND1`. Until one does, this file is a hand-written mirror and its tests
 * pin it to the Rust constants.
 */

/** Bytes in one complete release-set lineage record. */
export const RELEASE_LINEAGE_BYTES = 248;
/** Canonical release-set lineage magic. */
export const RELEASE_LINEAGE_MAGIC = RELEASE_LINEAGE_MAGIC_V1;
/** Implemented release-set lineage schema. */
export const RELEASE_LINEAGE_SCHEMA_VERSION = 1;
/** Implemented release-set lineage fixed-layout profile. */
export const RELEASE_LINEAGE_PROFILE = 1;

/**
 * The Registry's lineage PDA seed domain, and the only copy of it in TypeScript.
 *
 * A seed domain is a consensus coordinate. `releaseRegistry.ts` records what it
 * cost to keep three copies of the activation domain; this one starts with one.
 */
export const RELEASE_LINEAGE_PDA_SEED_V1 = RELEASE_LINEAGE_PDA_DOMAIN_V1;

const SCHEMA_OFFSET = 8;
const PROFILE_OFFSET = 10;
const HEADER_RESERVED_OFFSET = 12;
const HEADER_RESERVED_BYTES = 4;
const PREDECESSOR_OFFSET = 16;
const SUCCESSOR_OFFSET = 48;
const MOVED_ROLES_OFFSET = 80;
const MOVED_RESERVED_OFFSET = 85;
const MOVED_RESERVED_BYTES = 3;
const AUTHORITIES_OFFSET = 88;
const IDENTITY_BYTES = 32;

/**
 * Hops one walk will follow before refusing.
 *
 * Mirrors `LINEAGE_WALK_MAX_HOPS_V1`. A market migrates once per cut it is
 * behind, so this bounds cuts, not markets.
 */
export const MAX_LINEAGE_WALK_HOPS = 32;

/** One release set's declared successor, and who consented to the hop. */
export type ReleaseLineageV1 = Readonly<{
  predecessor: string;
  successor: string;
  /** The upgrade authority that signed for each role whose artifact moved. */
  consent: Readonly<Record<RegistryRole, string | null>>;
  /** The roles whose artifact release changed across this hop. */
  movedRoles: ReadonlyArray<RegistryRole>;
}>;

/**
 * Derive the lineage record's address for one predecessor release set.
 *
 * Seeds are exactly `[domain, predecessor]` under the Registry program — the
 * activation cache's two-seed shape, and no caller-selected seed.
 */
export function deriveReleaseLineageAddressV1(registryProgram: string, predecessorHex: string): string {
  return PublicKey.findProgramAddressSync(
    [RELEASE_LINEAGE_PDA_SEED_V1, contentIdBytes(predecessorHex, 'predecessor release set')],
    new PublicKey(registryProgram),
  )[0].toBase58();
}

/**
 * Hostile-decode one exact release-set lineage record.
 *
 * Refuses in the same order and for the same reasons as
 * `ReleaseLineageV1::decode`, including the two `require_zero` reserved runs and
 * the mask/consent coherence rule: a record claiming a role moved but recording
 * nobody's consent, or recording a key for a role that did not move, is not a
 * record this type can hold.
 */
export function decodeReleaseLineageV1(bytes: Uint8Array): ReleaseLineageV1 {
  if (bytes.length !== RELEASE_LINEAGE_BYTES) {
    throw new Error(`release lineage record must be exactly ${RELEASE_LINEAGE_BYTES} bytes, read ${bytes.length}`);
  }
  if (ascii(bytes, 0, 8) !== RELEASE_LINEAGE_MAGIC) {
    throw new Error('release lineage record does not carry the DCLTRLN1 magic');
  }
  if (u16(bytes, SCHEMA_OFFSET) !== RELEASE_LINEAGE_SCHEMA_VERSION) {
    throw new Error('release lineage record declares an unsupported schema');
  }
  if (u16(bytes, PROFILE_OFFSET) !== RELEASE_LINEAGE_PROFILE) {
    throw new Error('release lineage record declares an unsupported layout profile');
  }
  requireZero(bytes, HEADER_RESERVED_OFFSET, HEADER_RESERVED_BYTES, 'release lineage header reserved run');
  requireZero(bytes, MOVED_RESERVED_OFFSET, MOVED_RESERVED_BYTES, 'release lineage moved-role reserved run');

  const predecessorBytes = slice(bytes, PREDECESSOR_OFFSET, IDENTITY_BYTES);
  const successorBytes = slice(bytes, SUCCESSOR_OFFSET, IDENTITY_BYTES);
  if (isZero(predecessorBytes)) throw new Error('release lineage predecessor is the reserved all-zero identity');
  if (isZero(successorBytes)) throw new Error('release lineage successor is the reserved all-zero identity');
  const predecessor = hex(predecessorBytes);
  const successor = hex(successorBytes);
  if (predecessor === successor) {
    throw new Error('release lineage record names one release set as its own successor');
  }

  const consent: Record<string, string | null> = {};
  const movedRoles: RegistryRole[] = [];
  for (let index = 0; index < REGISTRY_ROLES.length; index += 1) {
    const role = REGISTRY_ROLES[index];
    const maskByte = bytes[MOVED_ROLES_OFFSET + index];
    if (maskByte !== 0 && maskByte !== 1) {
      throw new Error(`release lineage moved-role byte for ${role} is neither zero nor one`);
    }
    const authority = slice(bytes, AUTHORITIES_OFFSET + index * IDENTITY_BYTES, IDENTITY_BYTES);
    const present = !isZero(authority);
    // The mask and the key are one fact, so they are read as one.
    if ((maskByte === 1) !== present) {
      throw new Error(`release lineage consent for ${role} disagrees with its moved-role mask`);
    }
    consent[role] = present ? new PublicKey(authority).toBase58() : null;
    if (present) movedRoles.push(role);
  }
  if (movedRoles.length === 0) {
    throw new Error('release lineage record declares a hop in which no role moved');
  }

  return Object.freeze({
    predecessor,
    successor,
    consent: Object.freeze(consent) as Readonly<Record<RegistryRole, string | null>>,
    movedRoles: Object.freeze(movedRoles),
  });
}

/** What a caller found at the lineage address derived for one release set. */
export type LineageAtV1 =
  | Readonly<{ status: 'undeclared' }>
  | Readonly<{ status: 'declared'; record: ReleaseLineageV1 }>
  | Readonly<{ status: 'undecodable'; cause: string }>;

/** Why a lineage walk did not arrive. Mirrors `LineageWalkRefusal`. */
export type LineageWalkRefusalV1 = 'successor-undeclared' | 'misaddressed' | 'undecodable' | 'too-long';

/** Where a walk arrived, or why it did not. */
export type LineageWalkV1 =
  | Readonly<{
      status: 'arrived';
      origin: string;
      endpoint: string;
      hops: number;
      /** Every set traversed, origin first. This is the history itself. */
      path: ReadonlyArray<string>;
      /**
       * Whether a destination was named, and so what `endpoint` is evidence of.
       *
       * `true`: the walk was sent somewhere and got there. `false`: it ran to
       * the head of the DECLARED chain, which is the furthest anybody has
       * written down and not necessarily where the world is.
       */
      destinationChecked: boolean;
      /**
       * The origin was already the destination: nothing to migrate.
       *
       * Only ever `true` on a walk that named a destination, and that
       * restriction is the point. A destination-less walk on a set with no
       * declared successor also travels zero hops, but "nobody has declared a
       * successor for this set" and "this set is current" are different claims
       * and the walk cannot tell them apart without being told where current
       * is. Reporting them with one `true` is how a market two cuts behind the
       * world reads as up to date: market19 on devnet sits on cohort-7 with the
       * chain unwritten, and a to-head walk finds it trivially its own head.
       *
       * So this is `false` on every destination-less walk, including a
       * zero-hop one. A caller that wants to know whether a market is current
       * must say what current is, and then this answers it.
       */
      alreadyCurrent: boolean;
    }>
  | Readonly<{
      status: 'refused';
      refusal: LineageWalkRefusalV1;
      /** The set the walk was standing on when it refused. */
      at: string;
      sentence: string;
      path: ReadonlyArray<string>;
    }>;

/**
 * Follow a lineage from `origin`, optionally until it reaches `destination`.
 *
 * With no destination the walk runs to the head of the chain — the set nobody
 * has superseded, which is where the world is. With one, a chain that ends
 * short refuses `successor-undeclared` naming the set that still owes a
 * declaration, which is the repair instruction rather than a complaint.
 *
 * The walk reads no clock, because the record carries none: a hop declared long
 * after the fact encodes to exactly the bytes it would have encoded to at the
 * time, so a retroactively authored history walks like any other.
 */
export async function walkReleaseLineageV1(
  origin: string,
  lookup: (releaseSet: string) => LineageAtV1 | Promise<LineageAtV1>,
  options: Readonly<{ destination?: string }> = {},
): Promise<LineageWalkV1> {
  const destination = options.destination;
  const path: string[] = [origin];
  let standing = origin;
  let hops = 0;

  for (;;) {
    if (destination !== undefined && standing === destination) {
      return arrived(origin, standing, hops, path, true);
    }
    const found = await lookup(standing);
    if (found.status === 'undeclared') {
      // The head of the declared chain, which is not the same as the head of
      // the world while any hop is still unwritten.
      if (destination === undefined) return arrived(origin, standing, hops, path, false);
      return refused(
        'successor-undeclared',
        standing,
        path,
        `release set ${standing} has no declared successor, so the chain stops one or more cuts short of ${destination}; a lineage must be declared for ${standing} before this history is followable`,
      );
    }
    if (found.status === 'undecodable') {
      return refused('undecodable', standing, path, `the lineage record for ${standing} did not decode: ${found.cause}`);
    }
    // A record is evidence only about the set whose address derives it.
    if (found.record.predecessor !== standing) {
      return refused(
        'misaddressed',
        standing,
        path,
        `the record at ${standing}'s lineage address names ${found.record.predecessor} as its predecessor, so it is evidence about another set`,
      );
    }
    if (hops === MAX_LINEAGE_WALK_HOPS) {
      return refused(
        'too-long',
        standing,
        path,
        `the lineage from ${origin} runs past ${MAX_LINEAGE_WALK_HOPS} hops without arriving`,
      );
    }
    standing = found.record.successor;
    path.push(standing);
    hops += 1;
  }
}

function arrived(
  origin: string,
  endpoint: string,
  hops: number,
  path: ReadonlyArray<string>,
  destinationChecked: boolean,
): LineageWalkV1 {
  return Object.freeze({
    status: 'arrived' as const,
    origin,
    endpoint,
    hops,
    path: Object.freeze([...path]),
    destinationChecked,
    // Zero hops is only "already current" when there was a current to compare
    // against. See the field's own documentation.
    alreadyCurrent: destinationChecked && hops === 0,
  });
}

function refused(
  refusal: LineageWalkRefusalV1,
  at: string,
  path: ReadonlyArray<string>,
  sentence: string,
): LineageWalkV1 {
  return Object.freeze({ status: 'refused' as const, refusal, at, sentence, path: Object.freeze([...path]) });
}

/**
 * Follow a market's lineage against a live cluster.
 *
 * Reads one account per hop, deriving each address from the set the previous
 * record named — nothing about the destination is supplied on the wire, so
 * there is nothing to supply wrongly.
 */
export async function followReleaseLineageV1(
  client: Pick<SolanaRpcClient, 'multipleAccounts'>,
  request: Readonly<{ registryProgram: string; origin: string; destination?: string }>,
): Promise<LineageWalkV1> {
  return walkReleaseLineageV1(
    request.origin,
    async (releaseSet) => {
      const address = deriveReleaseLineageAddressV1(request.registryProgram, releaseSet);
      const observation = await client.multipleAccounts([address]);
      const account = observation.accounts[0]?.account ?? null;
      if (account === null || account.data.length === 0) return Object.freeze({ status: 'undeclared' as const });
      try {
        return Object.freeze({ status: 'declared' as const, record: decodeReleaseLineageV1(account.data) });
      } catch (error) {
        return Object.freeze({
          status: 'undecodable' as const,
          cause: error instanceof Error ? error.message : String(error),
        });
      }
    },
    request.destination === undefined ? {} : { destination: request.destination },
  );
}

function contentIdBytes(value: string, field: string): Uint8Array {
  if (!/^[0-9a-f]{64}$/.test(value)) {
    throw new Error(`${field} must be 64 lowercase hex characters`);
  }
  const bytes = new Uint8Array(IDENTITY_BYTES);
  for (let index = 0; index < IDENTITY_BYTES; index += 1) {
    bytes[index] = Number.parseInt(value.slice(index * 2, index * 2 + 2), 16);
  }
  if (isZero(bytes)) throw new Error(`${field} is the reserved all-zero identity`);
  return bytes;
}
