import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

import { PublicKey } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import { REGISTRY_ROLES, type RegistryRole } from './releaseRegistry';
import {
  MAX_LINEAGE_WALK_HOPS,
  RELEASE_LINEAGE_BYTES,
  RELEASE_LINEAGE_MAGIC,
  RELEASE_LINEAGE_PDA_SEED_V1,
  RELEASE_LINEAGE_PROFILE,
  RELEASE_LINEAGE_SCHEMA_VERSION,
  decodeReleaseLineageV1,
  deriveReleaseLineageAddressV1,
  followReleaseLineageV1,
  walkReleaseLineageV1,
  type LineageAtV1,
} from './releaseLineage';

const RUST_LINEAGE = fileURLToPath(
  new URL('../../../crates/dclutch-registry/src/lineage.rs', import.meta.url),
);
const RUST_WALK = fileURLToPath(
  new URL('../../../crates/dclutch-registry/src/lineage_walk.rs', import.meta.url),
);

const MOVED_ROLES_OFFSET = 80;
const AUTHORITIES_OFFSET = 88;

function setId(seed: number): string {
  return seed.toString(16).padStart(2, '0').repeat(32);
}

function authority(seed: number): Uint8Array {
  return new Uint8Array(32).fill(seed);
}

/** Build one canonical 248-byte record, the way the chain would hold it. */
function encodeLineage(
  predecessor: string,
  successor: string,
  moved: Readonly<Partial<Record<RegistryRole, Uint8Array>>>,
): Uint8Array {
  const bytes = new Uint8Array(RELEASE_LINEAGE_BYTES);
  bytes.set(new TextEncoder().encode(RELEASE_LINEAGE_MAGIC), 0);
  new DataView(bytes.buffer).setUint16(8, RELEASE_LINEAGE_SCHEMA_VERSION, true);
  new DataView(bytes.buffer).setUint16(10, RELEASE_LINEAGE_PROFILE, true);
  bytes.set(hexBytes(predecessor), 16);
  bytes.set(hexBytes(successor), 48);
  for (let index = 0; index < REGISTRY_ROLES.length; index += 1) {
    const key = moved[REGISTRY_ROLES[index]];
    if (key === undefined) continue;
    bytes[MOVED_ROLES_OFFSET + index] = 1;
    bytes.set(key, AUTHORITIES_OFFSET + index * 32);
  }
  return bytes;
}

function hexBytes(value: string): Uint8Array {
  const bytes = new Uint8Array(32);
  for (let index = 0; index < 32; index += 1) bytes[index] = Number.parseInt(value.slice(index * 2, index * 2 + 2), 16);
  return bytes;
}

const CANONICAL = encodeLineage(setId(0x11), setId(0x22), { core: authority(0xa0), trading: authority(0xa2) });

/** A lookup over explicit hops, in the shape a reader holds. */
function source(hops: ReadonlyArray<readonly [string, Uint8Array]>): (set: string) => LineageAtV1 {
  return (sought) => {
    const found = hops.find(([key]) => key === sought);
    if (found === undefined) return { status: 'undeclared' };
    return { status: 'declared', record: decodeReleaseLineageV1(found[1]) };
  };
}

function hop(from: number, to: number): readonly [string, Uint8Array] {
  return [setId(from), encodeLineage(setId(from), setId(to), { core: authority(0xa0) })] as const;
}

describe('release lineage record', () => {
  it('pins its layout to the Rust constants it mirrors', () => {
    const rust = readFileSync(RUST_LINEAGE, 'utf8');
    expect(rust).toContain('pub const RELEASE_LINEAGE_BYTES_V1: usize = 248;');
    expect(rust).toContain('b"dclutch:release-lineage:v1"');
    expect(rust).toContain('*b"DCLTRLN1"');
    expect(rust).toContain('const PREDECESSOR_OFFSET: usize = 16;');
    expect(rust).toContain('const SUCCESSOR_OFFSET: usize = 48;');
    expect(rust).toContain(`const MOVED_ROLES_OFFSET: usize = ${MOVED_ROLES_OFFSET};`);
    expect(rust).toContain(`const AUTHORITIES_OFFSET: usize = ${AUTHORITIES_OFFSET};`);

    expect(RELEASE_LINEAGE_BYTES).toBe(248);
    expect(RELEASE_LINEAGE_MAGIC).toBe('DCLTRLN1');
    expect(new TextDecoder().decode(RELEASE_LINEAGE_PDA_SEED_V1)).toBe('dclutch:release-lineage:v1');

    const walk = readFileSync(RUST_WALK, 'utf8');
    expect(walk).toContain(`pub const LINEAGE_WALK_MAX_HOPS_V1: u8 = ${MAX_LINEAGE_WALK_HOPS};`);
  });

  it('decodes a canonical record and reads consent back per role', () => {
    const record = decodeReleaseLineageV1(CANONICAL);
    expect(record.predecessor).toBe(setId(0x11));
    expect(record.successor).toBe(setId(0x22));
    expect(record.movedRoles).toEqual(['core', 'trading']);
    expect(record.consent.core).toBe(new PublicKey(authority(0xa0)).toBase58());
    expect(record.consent.trading).toBe(new PublicKey(authority(0xa2)).toBase58());
    expect(record.consent.claims).toBeNull();
    expect(record.consent.resolution).toBeNull();
    expect(record.consent.custody).toBeNull();
  });

  it('refuses every hostile mutation at its own field', () => {
    const mutate = (change: (bytes: Uint8Array) => void): (() => unknown) => {
      const bytes = Uint8Array.from(CANONICAL);
      change(bytes);
      return () => decodeReleaseLineageV1(bytes);
    };

    expect(() => decodeReleaseLineageV1(CANONICAL.slice(0, 247))).toThrow(/exactly 248 bytes/);
    expect(mutate((b) => { b[0] ^= 0xff; })).toThrow(/DCLTRLN1/);
    expect(mutate((b) => { b[8] = 2; })).toThrow(/unsupported schema/);
    expect(mutate((b) => { b[10] = 2; })).toThrow(/unsupported layout profile/);
    expect(mutate((b) => { b[12] = 1; })).toThrow(/header reserved run/);
    expect(mutate((b) => { b[85] = 1; })).toThrow(/moved-role reserved run/);
    expect(mutate((b) => { b.fill(0, 16, 48); })).toThrow(/predecessor is the reserved all-zero/);
    expect(mutate((b) => { b.fill(0, 48, 80); })).toThrow(/successor is the reserved all-zero/);
    expect(mutate((b) => { b.set(hexBytes(setId(0x11)), 48); })).toThrow(/its own successor/);
    expect(mutate((b) => { b[MOVED_ROLES_OFFSET] = 2; })).toThrow(/neither zero nor one/);
    // A role claimed as moved with nobody's consent recorded.
    expect(mutate((b) => { b.fill(0, AUTHORITIES_OFFSET, AUTHORITIES_OFFSET + 32); })).toThrow(/disagrees with its moved-role mask/);
    // Consent recorded for a role the mask says did not move.
    expect(mutate((b) => { b.set(authority(0xa1), AUTHORITIES_OFFSET + 32); })).toThrow(/disagrees with its moved-role mask/);
    // A hop in which nothing moved is not a hop.
    expect(
      mutate((b) => {
        b.fill(0, MOVED_ROLES_OFFSET, MOVED_ROLES_OFFSET + 5);
        b.fill(0, AUTHORITIES_OFFSET, RELEASE_LINEAGE_BYTES);
      }),
    ).toThrow(/no role moved/);
  });

  it('derives its address from the predecessor and nothing else', () => {
    const registry = new PublicKey(new Uint8Array(32).fill(7)).toBase58();
    const expected = PublicKey.findProgramAddressSync(
      [RELEASE_LINEAGE_PDA_SEED_V1, hexBytes(setId(0x11))],
      new PublicKey(registry),
    )[0].toBase58();
    expect(deriveReleaseLineageAddressV1(registry, setId(0x11))).toBe(expected);
    // Two different predecessors never name one record.
    expect(deriveReleaseLineageAddressV1(registry, setId(0x22))).not.toBe(expected);
    expect(() => deriveReleaseLineageAddressV1(registry, 'not-hex')).toThrow(/64 lowercase hex/);
  });
});

describe('walking a release lineage', () => {
  it('follows a chain to the set nobody has superseded', async () => {
    const walk = await walkReleaseLineageV1(setId(0x11), source([hop(0x11, 0x22), hop(0x22, 0x33)]));
    expect(walk.status).toBe('arrived');
    if (walk.status !== 'arrived') return;
    expect(walk.endpoint).toBe(setId(0x33));
    expect(walk.hops).toBe(2);
    expect(walk.path).toEqual([setId(0x11), setId(0x22), setId(0x33)]);
    expect(walk.destinationChecked).toBe(false);
    expect(walk.alreadyCurrent).toBe(false);
  });

  it('calls a market already on the destination already current', async () => {
    const walk = await walkReleaseLineageV1(setId(0x22), source([hop(0x11, 0x22)]), {
      destination: setId(0x22),
    });
    expect(walk.status).toBe('arrived');
    if (walk.status !== 'arrived') return;
    expect(walk.hops).toBe(0);
    expect(walk.destinationChecked).toBe(true);
    expect(walk.alreadyCurrent).toBe(true);
    expect(walk.path).toEqual([setId(0x22)]);
  });

  // The market19 trap, from the devnet recovery. A set two cuts behind the
  // world, with its chain unwritten, is trivially the head of its own declared
  // chain -- and used to report `alreadyCurrent: true` from there. "Nobody has
  // declared a successor for this set" and "this set is current" are different
  // claims, and a destination-less walk cannot tell them apart.
  it('does not call a stranded market current just because nothing points past it', async () => {
    const stranded = await walkReleaseLineageV1(setId(0x77), source([hop(0x11, 0x22)]));
    expect(stranded.status).toBe('arrived');
    if (stranded.status !== 'arrived') return;
    expect(stranded.hops).toBe(0);
    expect(stranded.endpoint).toBe(setId(0x77));
    expect(stranded.destinationChecked).toBe(false);
    expect(stranded.alreadyCurrent).toBe(false);

    // Asked the question that can actually be answered, it says the true thing
    // and names the set that owes a declaration.
    const asked = await walkReleaseLineageV1(setId(0x77), source([hop(0x11, 0x22)]), {
      destination: setId(0x22),
    });
    expect(asked.status).toBe('refused');
    if (asked.status !== 'refused') return;
    expect(asked.refusal).toBe('successor-undeclared');
    expect(asked.at).toBe(setId(0x77));
  });

  // A genuinely current market must still be able to say so, and can: it names
  // where current is.
  it('lets a market on the current set prove it by naming the destination', async () => {
    const walk = await walkReleaseLineageV1(setId(0x33), source([hop(0x11, 0x22), hop(0x22, 0x33)]), {
      destination: setId(0x33),
    });
    expect(walk.status).toBe('arrived');
    if (walk.status !== 'arrived') return;
    expect(walk.alreadyCurrent).toBe(true);
  });

  it('stops at a destination that sits mid-chain', async () => {
    const walk = await walkReleaseLineageV1(setId(0x11), source([hop(0x11, 0x22), hop(0x22, 0x33)]), {
      destination: setId(0x22),
    });
    expect(walk.status).toBe('arrived');
    if (walk.status !== 'arrived') return;
    expect(walk.endpoint).toBe(setId(0x22));
    expect(walk.hops).toBe(1);
  });

  it('names the set that still owes a successor when the chain stops short', async () => {
    const walk = await walkReleaseLineageV1(setId(0x11), source([hop(0x11, 0x22), hop(0x22, 0x33)]), {
      destination: setId(0x44),
    });
    expect(walk.status).toBe('refused');
    if (walk.status !== 'refused') return;
    expect(walk.refusal).toBe('successor-undeclared');
    expect(walk.at).toBe(setId(0x33));
    expect(walk.sentence).toContain('no declared successor');
    // The partial history is still returned: what IS followable stays visible.
    expect(walk.path).toEqual([setId(0x11), setId(0x22), setId(0x33)]);
  });

  it('refuses a record that is evidence about another set', async () => {
    // Red-proof by mutation: rewrite only the predecessor run, then serve the
    // record at the address it no longer describes.
    const forged = Uint8Array.from(CANONICAL);
    forged.set(hexBytes(setId(0x99)), 16);
    const walk = await walkReleaseLineageV1(setId(0x11), source([[setId(0x11), forged]]));
    expect(walk.status).toBe('refused');
    if (walk.status !== 'refused') return;
    expect(walk.refusal).toBe('misaddressed');
    expect(walk.at).toBe(setId(0x11));
    expect(walk.sentence).toContain(setId(0x99));
  });

  it('surfaces an undecodable record under the decoder\'s own complaint', async () => {
    const walk = await walkReleaseLineageV1(setId(0x11), () => ({ status: 'undecodable', cause: 'bad magic' }));
    expect(walk.status).toBe('refused');
    if (walk.status !== 'refused') return;
    expect(walk.refusal).toBe('undecodable');
    expect(walk.sentence).toContain('bad magic');
  });

  it('bounds a cycle instead of running forever', async () => {
    const walk = await walkReleaseLineageV1(setId(0x11), source([hop(0x11, 0x22), hop(0x22, 0x11)]), {
      destination: setId(0x44),
    });
    expect(walk.status).toBe('refused');
    if (walk.status !== 'refused') return;
    expect(walk.refusal).toBe('too-long');
  });

  it('arrives on a chain of exactly the bound and refuses the hop past it', async () => {
    const chain = Array.from({ length: MAX_LINEAGE_WALK_HOPS + 1 }, (_, step) => hop(0x11 + step, 0x12 + step));

    const exact = await walkReleaseLineageV1(setId(0x11), source(chain.slice(0, MAX_LINEAGE_WALK_HOPS)));
    expect(exact.status).toBe('arrived');
    if (exact.status === 'arrived') expect(exact.hops).toBe(MAX_LINEAGE_WALK_HOPS);

    const past = await walkReleaseLineageV1(setId(0x11), source(chain));
    expect(past.status).toBe('refused');
    if (past.status !== 'refused') return;
    expect(past.refusal).toBe('too-long');
    expect(past.at).toBe(setId(0x11 + MAX_LINEAGE_WALK_HOPS));
  });

  it('walks a retroactively authored history exactly like a timely one', async () => {
    // The honesty rests on an absence: the record has no field that could say
    // WHEN it was written, so a hop declared today for two cohorts that
    // superseded each other months ago is byte-identical to a timely one.
    const timely = encodeLineage(setId(0x11), setId(0x22), { core: authority(0xa0) });
    const late = encodeLineage(setId(0x11), setId(0x22), { core: authority(0xa0) });
    expect(Array.from(late)).toEqual(Array.from(timely));

    const walk = await walkReleaseLineageV1(setId(0x11), source([hop(0x11, 0x22), hop(0x22, 0x33)]), {
      destination: setId(0x33),
    });
    expect(walk.status).toBe('arrived');
    if (walk.status === 'arrived') expect(walk.hops).toBe(2);
  });
});

describe('following a lineage against a cluster', () => {
  it('derives each hop from the set the previous record named', async () => {
    const registry = new PublicKey(new Uint8Array(32).fill(7)).toBase58();
    const held = new Map(
      [hop(0x11, 0x22), hop(0x22, 0x33)].map(([set, bytes]) => [
        deriveReleaseLineageAddressV1(registry, set),
        bytes,
      ]),
    );
    const reads: string[] = [];

    const walk = await followReleaseLineageV1(
      {
        multipleAccounts: async (addresses: ReadonlyArray<string>) => {
          reads.push(addresses[0]);
          const data = held.get(addresses[0]);
          return {
            slot: '1',
            accounts: [
              {
                address: addresses[0],
                account:
                  data === undefined
                    ? null
                    : { data, executable: false, lamports: '1', owner: registry, space: data.length },
              },
            ],
          };
        },
      } as never,
      { registryProgram: registry, origin: setId(0x11) },
    );

    expect(walk.status).toBe('arrived');
    if (walk.status === 'arrived') expect(walk.endpoint).toBe(setId(0x33));
    // Three reads: two records, then the vacant address that ends the chain.
    expect(reads).toEqual([
      deriveReleaseLineageAddressV1(registry, setId(0x11)),
      deriveReleaseLineageAddressV1(registry, setId(0x22)),
      deriveReleaseLineageAddressV1(registry, setId(0x33)),
    ]);
  });
});

/**
 * The exact 248 bytes the compiled Registry wrote, read back off the bank.
 *
 * Produced by `programs/dclutch-registry-sbf/tests/lineage_program_test.rs`
 * (`the_compiled_registry_declares_a_successor_and_the_walk_follows_the_hop`),
 * which drives a real `DeclareSuccessor` through the real ELF and prints the
 * record it finds at the lineage address. Pasting the bytes here is the point:
 * the Rust program produced them and this decoder has never seen them, so
 * agreement is evidence the two implementations of the layout match rather
 * than evidence that one of them is self-consistent.
 */
const LANDED_RECORD_HEX =
  '44434c54524c4e310100010000000000d184a50d9e475ad43cb3c8ac0a3d8420d83b1fb32904948084ad6efd9b50555a22969e2508cf865cc1a00beacd35b8480b0a96a8fb2c582a053d76a2c39a59e60101010101000000d759793bbc13a2819a827c76adb6fba8a49aee007f49f2d0992d99b825ad2c48d759793bbc13a2819a827c76adb6fba8a49aee007f49f2d0992d99b825ad2c48d759793bbc13a2819a827c76adb6fba8a49aee007f49f2d0992d99b825ad2c48d759793bbc13a2819a827c76adb6fba8a49aee007f49f2d0992d99b825ad2c48d759793bbc13a2819a827c76adb6fba8a49aee007f49f2d0992d99b825ad2c48';

function fromHex(value: string): Uint8Array {
  const bytes = new Uint8Array(value.length / 2);
  for (let index = 0; index < bytes.length; index += 1) {
    bytes[index] = Number.parseInt(value.slice(index * 2, index * 2 + 2), 16);
  }
  return bytes;
}

describe('the record the compiled Registry actually wrote', () => {
  const landed = fromHex(LANDED_RECORD_HEX);

  it('is the exact width and decodes without being adjusted', () => {
    expect(landed).toHaveLength(RELEASE_LINEAGE_BYTES);
    const record = decodeReleaseLineageV1(landed);
    expect(record.predecessor).toBe(
      'd184a50d9e475ad43cb3c8ac0a3d8420d83b1fb32904948084ad6efd9b50555a',
    );
    expect(record.successor).toBe(
      '22969e2508cf865cc1a00beacd35b8480b0a96a8fb2c582a053d76a2c39a59e6',
    );
    // That hop moved all five roles, and one deployer consented for each.
    expect([...record.movedRoles].sort()).toEqual([...REGISTRY_ROLES].sort());
    for (const role of REGISTRY_ROLES) {
      // Base58, because a consenting authority is a KEY and the decoder says
      // so. The hex above is the raw record; this is what it means.
      expect(record.consent[role]).toBe('FVdnakemjhcemfWUgNR2AERbk5Pog7zJ1UF2LjbocBUj');
    }
  });

  it('is a chain the walk follows, one hop, arriving', async () => {
    const record = decodeReleaseLineageV1(landed);
    const lookup = (releaseSet: string): LineageAtV1 =>
      releaseSet === record.predecessor
        ? Object.freeze({ status: 'declared' as const, record })
        : Object.freeze({ status: 'undeclared' as const });

    const walk = await walkReleaseLineageV1(record.predecessor, lookup, {
      destination: record.successor,
    });
    expect(walk.status).toBe('arrived');
    if (walk.status !== 'arrived') return;
    expect(walk.hops).toBe(1);
    expect(walk.path).toEqual([record.predecessor, record.successor]);
    expect(walk.alreadyCurrent).toBe(false);

    // And a market already on the successor owes nothing.
    const settled = await walkReleaseLineageV1(record.successor, lookup, {
      destination: record.successor,
    });
    expect(settled.status).toBe('arrived');
    if (settled.status !== 'arrived') return;
    expect(settled.alreadyCurrent).toBe(true);
  });

  it('refuses a single flipped byte, so the agreement above is not vacuous', () => {
    const tampered = new Uint8Array(landed);
    // The moved-role mask now claims Core did not move, while its consent slot
    // still names a key: the mask and the key are one fact.
    tampered[80] = 0;
    expect(() => decodeReleaseLineageV1(tampered)).toThrow();
  });
});
