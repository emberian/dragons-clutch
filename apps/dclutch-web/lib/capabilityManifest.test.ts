import { describe, expect, it } from 'vitest';

import {
  CAPABILITY_MANIFEST_ENTRY_BYTES,
  CAPABILITY_MANIFEST_HEADER_BYTES,
  decodeCapabilityManifestV1,
  recognizeCapabilityKindV1,
} from './capabilityManifest';

type Entry = Readonly<{ kind: number; policy?: 0 | 1; deadline?: bigint; dependencies?: ReadonlyArray<number> }>;

function manifest(entries: ReadonlyArray<Entry>): Uint8Array {
  const bytes = new Uint8Array(CAPABILITY_MANIFEST_HEADER_BYTES + CAPABILITY_MANIFEST_ENTRY_BYTES * entries.length);
  bytes.set(new TextEncoder().encode('DCLTCAP1'));
  const view = new DataView(bytes.buffer);
  view.setUint16(8, 1, true);
  view.setUint16(10, 1, true);
  view.setUint16(12, entries.length, true);
  entries.forEach((entry, index) => {
    const offset = CAPABILITY_MANIFEST_HEADER_BYTES + CAPABILITY_MANIFEST_ENTRY_BYTES * index;
    for (let identity = 0; identity < 6; identity += 1) {
      bytes.fill(entry.kind + identity, offset + identity * 32, offset + identity * 32 + 32);
    }
    const policy = entry.policy ?? 0;
    bytes[offset + 192] = policy;
    const dependencies = entry.dependencies ?? [];
    bytes[offset + 193] = dependencies.length;
    view.setBigUint64(offset + 200, entry.deadline ?? BigInt(0), true);
    dependencies.forEach((dependency, position) => { bytes[offset + 208 + position] = dependency; });
  });
  return bytes;
}

describe('immutable capability manifest', () => {
  it('decodes every entry of one ordered manifest, not only a selected one', () => {
    const entries = decodeCapabilityManifestV1(manifest([{ kind: 1 }, { kind: 9, policy: 1, deadline: BigInt(4242), dependencies: [0] }]));
    expect(entries).toHaveLength(2);
    expect(entries[0]).toMatchObject({ index: 0, activation: 'immediate', deadline: BigInt(0), dependencies: [] });
    expect(entries[1]).toMatchObject({ index: 1, activation: 'deadline', deadline: BigInt(4242), dependencies: [1 - 1] });
    expect(entries[1].kind.every((byte) => byte === 9)).toBe(true);
    expect(entries[1].derivation.every((byte) => byte === 14)).toBe(true);
  });

  it('refuses a header, width, ordering, policy, or dependency list that is not canonical', () => {
    const wrongMagic = manifest([{ kind: 1 }]);
    wrongMagic[0] = 0x44 + 1;
    expect(() => decodeCapabilityManifestV1(wrongMagic)).toThrow(/wrong exact header/);

    const trailing = new Uint8Array(manifest([{ kind: 1 }]).length + 1);
    trailing.set(manifest([{ kind: 1 }]));
    expect(() => decodeCapabilityManifestV1(trailing)).toThrow(/width is invalid/);

    expect(() => decodeCapabilityManifestV1(manifest([{ kind: 9 }, { kind: 1 }]))).toThrow(/not strictly ordered/);
    expect(() => decodeCapabilityManifestV1(manifest([{ kind: 1 }, { kind: 9 }]))).not.toThrow();
    expect(() => decodeCapabilityManifestV1(manifest([{ kind: 1, policy: 1, deadline: BigInt(0) }]))).toThrow(/activation policy is noncanonical/);
    expect(() => decodeCapabilityManifestV1(manifest([{ kind: 1, deadline: BigInt(5) }]))).toThrow(/activation policy is noncanonical/);
    expect(() => decodeCapabilityManifestV1(manifest([{ kind: 1, dependencies: [0] }]))).toThrow(/dependency list is noncanonical/);
    expect(() => decodeCapabilityManifestV1(manifest([{ kind: 1 }, { kind: 9, dependencies: [3] }]))).toThrow(/dependency list is noncanonical/);

    const zeroIdentity = manifest([{ kind: 1 }]);
    zeroIdentity.fill(0, CAPABILITY_MANIFEST_HEADER_BYTES + 64, CAPABILITY_MANIFEST_HEADER_BYTES + 96);
    expect(() => decodeCapabilityManifestV1(zeroIdentity)).toThrow(/identity 2/);

    const stray = manifest([{ kind: 1 }]);
    stray[CAPABILITY_MANIFEST_HEADER_BYTES + 208] = 1;
    expect(() => decodeCapabilityManifestV1(stray)).toThrow(/inactive dependency is nonzero/);
  });

  it('names only kinds it recognizes and never invents a meaning', () => {
    const known = Uint8Array.from((
      '8e8a063932339a7eb910608e76b1e70ad0f41b999b6252eeab890ffb733b5474'.match(/../g) ?? []
    ), (pair) => Number.parseInt(pair, 16));
    expect(recognizeCapabilityKindV1(known)).toBe('Product payoff admission');
    expect(recognizeCapabilityKindV1(new Uint8Array(32).fill(3))).toBeNull();
  });
});
