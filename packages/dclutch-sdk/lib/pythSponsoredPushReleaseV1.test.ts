import { PublicKey } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import { hex, sha256 } from './bytes';
import {
  buildPythSponsoredPushReleaseRecordV1,
  decodePythSponsoredPushReleaseV1,
  encodePythSponsoredPushReleaseV1,
  PYTH_SPONSORED_PUSH_RELEASE_BYTES_V1,
  PYTH_SPONSORED_PUSH_RELEASE_SCHEMA_ID_V1,
  type PythSponsoredPushReleaseInputV1,
} from '@dclutch/sdk/pythSponsoredPushReleaseV1';

function address(byte: number): string {
  return new PublicKey(new Uint8Array(32).fill(byte)).toBase58();
}

function id(byte: number): string {
  return byte.toString(16).padStart(2, '0').repeat(32);
}

function release(): PythSponsoredPushReleaseInputV1 {
  return Object.freeze({
    clusterId: id(1),
    receiverProgram: address(2),
    receiverProgramData: address(3),
    receiverAbiId: id(4),
    receiverUpgradeAuthority: address(5),
    pushOracleProgram: address(6),
    pushOracleProgramData: address(7),
    pushOracleAbiId: id(8),
    pushOracleUpgradeAuthority: address(9),
    receiverConfig: address(10),
    receiverConfigDigest: id(11),
    priceAccount: address(12),
    feedId: id(13),
    priceUpdateCodecId: id(14),
    adapterId: id(15),
    providerFamilyId: id(16),
    transportProfileId: id(17),
    receiverDeploymentSlot: 489_486_600n,
    pushOracleDeploymentSlot: 489_486_972n,
    shard: 0,
    feedAccountBump: 254,
    activationTime: 1_777_777_777n,
  });
}

describe('PythSponsoredPushReleaseV1', () => {
  it('round-trips the exact 592-byte Rust layout', () => {
    const input = release();
    const bytes = encodePythSponsoredPushReleaseV1(input);
    expect(bytes).toHaveLength(PYTH_SPONSORED_PUSH_RELEASE_BYTES_V1);
    expect(new TextDecoder().decode(bytes.slice(0, 8))).toBe('DCLTPSP1');
    expect(hex(bytes.slice(80, 112))).toBe(hex(new PublicKey(input.receiverProgramData).toBytes()));
    expect(hex(bytes.slice(176, 208))).toBe(hex(new PublicKey(input.pushOracleProgramData).toBytes()));
    expect(hex(bytes.slice(272, 304))).toBe(input.feedId);
    expect(new DataView(bytes.buffer).getBigUint64(560, true)).toBe(input.receiverDeploymentSlot);
    expect(new DataView(bytes.buffer).getBigUint64(568, true)).toBe(input.pushOracleDeploymentSlot);
    expect(decodePythSponsoredPushReleaseV1(bytes)).toMatchObject(input);
    expect(encodePythSponsoredPushReleaseV1(decodePythSponsoredPushReleaseV1(bytes))).toEqual(bytes);
  });

  it('builds the exact Registry identity and schema binding', async () => {
    const record = await buildPythSponsoredPushReleaseRecordV1(release());
    expect(record.schemaId).toBe(PYTH_SPONSORED_PUSH_RELEASE_SCHEMA_ID_V1);
    expect(record.recordId).toBe(hex(await sha256(record.body)));
    expect(record.release.bytes).toEqual(record.body);
  });

  it('refuses wrong headers, reserved bytes, zero identities, and zero slots', () => {
    const canonical = encodePythSponsoredPushReleaseV1(release());
    const wrongMagic = canonical.slice();
    wrongMagic[0] = 0;
    const reserved = canonical.slice();
    reserved[10] = 1;
    const zeroIdentity = canonical.slice();
    zeroIdentity.fill(0, 16, 48);
    for (const hostile of [canonical.slice(0, -1), wrongMagic, reserved, zeroIdentity]) {
      expect(() => decodePythSponsoredPushReleaseV1(hostile)).toThrow();
    }
    const zeroSlot = canonical.slice();
    new DataView(zeroSlot.buffer).setBigUint64(560, 0n, true);
    expect(() => decodePythSponsoredPushReleaseV1(zeroSlot)).toThrow(/zero deployment slot/);
  });

  it('changes the record identity for ProgramData, slot, and feed substitutions', async () => {
    const canonical = await buildPythSponsoredPushReleaseRecordV1(release());
    for (const [offset, label] of [[80, 'ProgramData'], [568, 'slot'], [272, 'feed']] as const) {
      const body = canonical.body.slice();
      body[offset] = (body[offset] ?? 0) ^ 1;
      expect(() => decodePythSponsoredPushReleaseV1(body), label).not.toThrow();
      expect(hex(await sha256(body)), label).not.toBe(canonical.recordId);
    }
  });
});
