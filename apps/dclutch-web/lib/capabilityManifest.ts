import { ascii, hex, requireNonzero, requireZero, slice, u16, u64 } from './bytes';

/**
 * The immutable `DCLTCAP1` capability manifest a Market root commits to by
 * content identity.
 *
 * A Market names capability children only through this manifest; nothing else
 * in the browser may assert that a Market "has" a capability. Every entry is
 * decoded exhaustively here — ordering, activation policy, and the dependency
 * list included — so a caller cannot select an entry out of a manifest whose
 * remaining entries were never checked.
 */

export const CAPABILITY_MANIFEST_MAGIC = 'DCLTCAP1';
export const CAPABILITY_MANIFEST_HEADER_BYTES = 16;
export const CAPABILITY_MANIFEST_ENTRY_BYTES = 528;
export const CAPABILITY_MANIFEST_MAX_ENTRIES = 16;

export type CapabilityActivationV1 = 'immediate' | 'deadline';

export type CapabilityManifestEntryV1 = Readonly<{
  index: number;
  kind: Uint8Array;
  programSet: Uint8Array;
  config: Uint8Array;
  capacity: Uint8Array;
  rootSchema: Uint8Array;
  derivation: Uint8Array;
  activation: CapabilityActivationV1;
  deadline: bigint;
  dependencies: ReadonlyArray<number>;
}>;

/** Decode and fully validate every entry of one capability manifest. */
export function decodeCapabilityManifestV1(bytes: Uint8Array): ReadonlyArray<CapabilityManifestEntryV1> {
  if (bytes.length < CAPABILITY_MANIFEST_HEADER_BYTES
      || ascii(bytes, 0, 8) !== CAPABILITY_MANIFEST_MAGIC
      || u16(bytes, 8) !== 1
      || u16(bytes, 10) !== 1) {
    throw new Error('capability manifest has the wrong exact header');
  }
  requireZero(bytes, 14, 2, 'capability manifest header');
  const count = u16(bytes, 12);
  if (count === 0 || count > CAPABILITY_MANIFEST_MAX_ENTRIES
      || bytes.length !== CAPABILITY_MANIFEST_HEADER_BYTES + CAPABILITY_MANIFEST_ENTRY_BYTES * count) {
    throw new Error('capability manifest width is invalid');
  }
  let priorKind: Uint8Array | null = null;
  const entries: CapabilityManifestEntryV1[] = [];
  for (let index = 0; index < count; index += 1) {
    const offset = CAPABILITY_MANIFEST_HEADER_BYTES + CAPABILITY_MANIFEST_ENTRY_BYTES * index;
    const identities = [0, 32, 64, 96, 128, 160].map((relative) => slice(bytes, offset + relative, 32));
    identities.forEach((identity, coordinate) => requireNonzero(identity, `capability manifest entry ${index} identity ${coordinate}`));
    if (priorKind !== null) {
      let order = 0;
      while (order < 32 && priorKind[order] === identities[0][order]) order += 1;
      if (order === 32 || (priorKind[order] ?? 0) > (identities[0][order] ?? 0)) throw new Error('capability manifest kinds are not strictly ordered');
    }
    priorKind = identities[0];
    requireZero(bytes, offset + 194, 6, `capability manifest entry ${index}`);
    const policy = bytes[offset + 192];
    const deadline = u64(bytes, offset + 200);
    if ((policy !== 0 && policy !== 1) || (policy === 0 && deadline !== BigInt(0)) || (policy === 1 && deadline === BigInt(0))) {
      throw new Error('capability manifest activation policy is noncanonical');
    }
    const dependencyCount = bytes[offset + 193] ?? 0;
    if (dependencyCount > CAPABILITY_MANIFEST_MAX_ENTRIES) throw new Error('capability manifest dependency count exceeds its bound');
    const dependencies: number[] = [];
    let priorDependency = -1;
    for (let position = 0; position < CAPABILITY_MANIFEST_MAX_ENTRIES; position += 1) {
      const dependency = bytes[offset + 208 + position] ?? 0;
      if (position < dependencyCount) {
        if (dependency >= count || dependency === index || dependency <= priorDependency) throw new Error('capability manifest dependency list is noncanonical');
        priorDependency = dependency;
        dependencies.push(dependency);
      } else if (dependency !== 0) {
        throw new Error('capability manifest inactive dependency is nonzero');
      }
    }
    entries.push(Object.freeze({
      index,
      kind: identities[0],
      programSet: identities[1],
      config: identities[2],
      capacity: identities[3],
      rootSchema: identities[4],
      derivation: identities[5],
      activation: policy === 0 ? 'immediate' : 'deadline',
      deadline,
      dependencies: Object.freeze(dependencies),
    }));
  }
  return Object.freeze(entries);
}

/**
 * Kinds this browser can name. A kind absent from this table is still listed
 * from the authenticated manifest; it is labelled as unrecognized rather than
 * given an invented meaning.
 */
export const RECOGNIZED_CAPABILITY_KINDS_V1: Readonly<Record<string, string>> = Object.freeze({
  '8e8a063932339a7eb910608e76b1e70ad0f41b999b6252eeab890ffb733b5474': 'Product payoff admission',
});

export function recognizeCapabilityKindV1(kind: Uint8Array): string | null {
  return RECOGNIZED_CAPABILITY_KINDS_V1[hex(kind)] ?? null;
}
