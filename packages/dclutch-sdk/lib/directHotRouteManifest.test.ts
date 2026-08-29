import { PublicKey } from '@solana/web3.js';
import { describe, expect, it } from 'vitest';

import { sha256 } from './bytes';
import {
  DIRECT_INLINE_CURRENT_RUNTIME_TAIL_ACCOUNTS_V3,
  DIRECT_INLINE_NAMED_RUNTIME_FIXED_ALIASES_V3,
  DIRECT_INLINE_RUNTIME_TAIL_WRITABLE_V3,
} from './directInlineV3';
import {
  DIRECT_HOT_FIXED_ROLE_LABELS_V3,
  DIRECT_HOT_ROUTE_MANIFEST_FORMAT_V3,
  DIRECT_HOT_ROUTE_MANIFEST_MAX_BYTES_V3,
  inspectDirectHotRouteManifestJsonV3,
} from './directHotRouteManifest';
import { CHECKED_INFRASTRUCTURE_BYTES_V1 } from './infrastructure';
import { type SolanaRpcClient } from './rpc';

function key(seed: number): string {
  const bytes = new Uint8Array(32);
  new DataView(bytes.buffer).setUint32(0, seed, true);
  bytes[31] = 1;
  return new PublicKey(bytes).toBase58();
}

function hex(bytes: Uint8Array): string {
  return Array.from(bytes, (byte) => byte.toString(16).padStart(2, '0')).join('');
}

function base64(bytes: Uint8Array): string {
  return Buffer.from(bytes).toString('base64');
}

type ManifestValue = {
  format: string;
  payer: string;
  fixedAccounts: Array<{ role: string; address: string; isSigner: boolean; isWritable: boolean }>;
  strategyAccounts: Array<{ address: string; isSigner: boolean; isWritable: boolean }>;
  runtimeAccounts: Array<{ address: string; isSigner: boolean; isWritable: boolean }>;
  lookupTables: string[];
  lookupTableCreationSlot: string;
  checkedInfrastructure: string | null;
  checkedInfrastructureSha256?: string;
  [field: string]: unknown;
};

async function manifestValue(): Promise<ManifestValue> {
  const infrastructure = new Uint8Array(CHECKED_INFRASTRUCTURE_BYTES_V1).fill(0x5a);
  const fixedAccounts = DIRECT_HOT_FIXED_ROLE_LABELS_V3.map((role, index) => ({
    role,
    address: key(index + 2),
    isSigner: false,
    isWritable: index === 1,
  }));
  const joins = new Map<number, number>(DIRECT_INLINE_NAMED_RUNTIME_FIXED_ALIASES_V3);
  return {
    format: DIRECT_HOT_ROUTE_MANIFEST_FORMAT_V3,
    payer: key(1),
    fixedAccounts,
    strategyAccounts: [],
    runtimeAccounts: Array.from({ length: DIRECT_INLINE_CURRENT_RUNTIME_TAIL_ACCOUNTS_V3 }, (_, index) => ({
      address: index === 1 ? key(1) : joins.has(index) ? fixedAccounts[joins.get(index)!]!.address : key(100 + index),
      isSigner: index === 1,
      isWritable: DIRECT_INLINE_RUNTIME_TAIL_WRITABLE_V3.includes(index as never),
    })),
    lookupTables: [key(200)],
    lookupTableCreationSlot: '800',
    checkedInfrastructure: base64(infrastructure),
    checkedInfrastructureSha256: hex(await sha256(infrastructure)),
  };
}

function authenticatorSentinel(): Readonly<{ client: SolanaRpcClient; calls(): number }> {
  let count = 0;
  return Object.freeze({
    client: {
      finalizedSlot: async () => {
        count += 1;
        throw new Error('existing route authenticator reached');
      },
    } as unknown as SolanaRpcClient,
    calls: () => count,
  });
}

async function refuseBeforeRpc(source: string | Uint8Array, reason: RegExp): Promise<void> {
  const sentinel = authenticatorSentinel();
  await expect(inspectDirectHotRouteManifestJsonV3(sentinel.client, source)).rejects.toThrow(reason);
  expect(sentinel.calls()).toBe(0);
}

describe('strict Direct Hot route manifest admission', () => {
  it('passes a bounded exact document only into the existing chain authenticator', async () => {
    const sentinel = authenticatorSentinel();
    await expect(inspectDirectHotRouteManifestJsonV3(
      sentinel.client,
      JSON.stringify(await manifestValue()),
    )).rejects.toThrow('existing route authenticator reached');
    expect(sentinel.calls()).toBe(1);
  });

  it('refuses duplicate keys at the envelope and coordinate levels before JSON.parse erases them', async () => {
    const value = await manifestValue();
    const source = JSON.stringify(value);
    const payer = JSON.stringify(value.payer);
    await refuseBeforeRpc(source.replace(`"payer":${payer}`, `"payer":${payer},"payer":${payer}`), /duplicate object key "payer"/);
    const address = JSON.stringify(value.fixedAccounts[0]!.address);
    await refuseBeforeRpc(source.replace(`"address":${address}`, `"address":${address},"address":${address}`), /duplicate object key "address"/);
  });

  it('refuses missing, unknown, defaulted, or substituted envelope fields', async () => {
    const value = await manifestValue();
    const missing = { ...value };
    delete missing.checkedInfrastructureSha256;
    await refuseBeforeRpc(JSON.stringify(missing), /missing or unknown fields/);
    await refuseBeforeRpc(JSON.stringify({ ...value, unchecked: true }), /missing or unknown fields/);
    await refuseBeforeRpc(JSON.stringify({ ...value, format: 'dclutch-direct-hot-route-manifest-v2' }), /format must be/);
    await refuseBeforeRpc(JSON.stringify({ ...value, checkedInfrastructure: null }), /canonical base64/);
  });

  it('refuses trailing values and noncanonical string encoding', async () => {
    const value = await manifestValue();
    const source = JSON.stringify(value);
    await refuseBeforeRpc(`${source} {}`, /trailing data/);
    const payerToken = JSON.stringify(value.payer);
    const first = value.payer[0] as string;
    const escaped = `"\\u${first.charCodeAt(0).toString(16).padStart(4, '0')}${value.payer.slice(1)}"`;
    await refuseBeforeRpc(source.replace(payerToken, escaped), /canonical JSON encoding/);
  });

  it('bounds source bytes, UTF-8, nesting, arrays, strings, numbers, and total values', async () => {
    await refuseBeforeRpc(' '.repeat(DIRECT_HOT_ROUTE_MANIFEST_MAX_BYTES_V3 + 1), /outside 1\.\.65536 bytes/);
    await refuseBeforeRpc(Uint8Array.from([0xff]), /canonical UTF-8/);
    const value = await manifestValue();
    let nested: unknown = true;
    for (let depth = 0; depth < 10; depth += 1) nested = [nested];
    await refuseBeforeRpc(JSON.stringify({ ...value, hostile: nested }), /nesting exceeds 8/);
    await refuseBeforeRpc(JSON.stringify({ ...value, runtimeAccounts: Array.from({ length: 257 }, () => ({})) }), /array exceeds 256 entries/);
    await refuseBeforeRpc(JSON.stringify({ ...value, hostile: 'x'.repeat(4_097) }), /string exceeds 4096 bytes/);
    await refuseBeforeRpc(`${JSON.stringify(value).slice(0, -1)},"hostile":${'1'.repeat(25)}}`, /number exceeds 24 bytes/);
    await refuseBeforeRpc(JSON.stringify({ ...value, runtimeAccounts: Array.from({ length: 256 }, (_, index) => ({
      address: key(index + 100), isSigner: false, isWritable: false,
      nested: [true, false, null],
    })) }), /tree exceeds 2048 values/);
  });

  it('refuses noncanonical account rows, privileges, lengths, and role substitutions', async () => {
    const value = await manifestValue();
    await refuseBeforeRpc(JSON.stringify({ ...value, fixedAccounts: value.fixedAccounts.slice(0, -1) }), /fixedAccounts must contain exactly 39 entries/);
    const role = structuredClone(value);
    role.fixedAccounts[0]!.role = 'Another Market';
    await refuseBeforeRpc(JSON.stringify(role), /role must be exactly Market/);
    const privilege = structuredClone(value);
    privilege.fixedAccounts[1]!.isWritable = false;
    await refuseBeforeRpc(JSON.stringify(privilege), /noncanonical signer or writable privilege/);
    const unknown = structuredClone(value);
    Object.assign(unknown.runtimeAccounts[0]!, { role: 'invented' });
    await refuseBeforeRpc(JSON.stringify(unknown), /missing or unknown fields/);
    const missing = structuredClone(value);
    delete (missing.runtimeAccounts[0] as Partial<{ isSigner: boolean }>).isSigner;
    await refuseBeforeRpc(JSON.stringify(missing), /missing or unknown fields/);
    await refuseBeforeRpc(JSON.stringify({ ...value, strategyAccounts: [{ address: key(60), isSigner: false, isWritable: false }] }), /strategyAccounts must contain exactly 0 entries/);
    await refuseBeforeRpc(JSON.stringify({ ...value, lookupTables: [key(201), key(202)] }), /lookupTables must contain exactly 1 entries/);
    await refuseBeforeRpc(JSON.stringify({ ...value, lookupTableCreationSlot: '0800' }), /canonical decimal u64/);
  });

  it('refuses noncanonical base58 and every account alias class', async () => {
    const value = await manifestValue();
    await refuseBeforeRpc(JSON.stringify({ ...value, payer: `${value.payer} ` }), /canonical Solana address/);
    await refuseBeforeRpc(JSON.stringify({ ...value, payer: value.fixedAccounts[0]!.address }), /route payer must be runtimeAccounts\[1\]/);
    const fixedAlias = structuredClone(value);
    fixedAlias.fixedAccounts[1]!.address = fixedAlias.fixedAccounts[0]!.address;
    await refuseBeforeRpc(JSON.stringify(fixedAlias), /39 distinct named pre-seal roles/);
    const runtimeAlias = structuredClone(value);
    runtimeAlias.runtimeAccounts[0]!.address = value.fixedAccounts[0]!.address;
    await refuseBeforeRpc(JSON.stringify(runtimeAlias), /duplicate-free current 39-account physical tail/);
    await refuseBeforeRpc(JSON.stringify({ ...value, lookupTables: [value.fixedAccounts[0]!.address] }), /lookup table 0 aliases the payer or one Hot instruction account/);
  });

  it('refuses malformed checked evidence encodings, hex identities, and substitutions', async () => {
    const value = await manifestValue();
    await refuseBeforeRpc(JSON.stringify({ ...value, checkedInfrastructure: 'AA=' }), /canonical base64/);
    await refuseBeforeRpc(JSON.stringify({ ...value, checkedInfrastructure: 'AAAA' }), /exactly 2280 bytes/);
    await refuseBeforeRpc(JSON.stringify({ ...value, checkedInfrastructureSha256: 'AB'.repeat(32) }), /lowercase 32-byte hex identity/);
    await refuseBeforeRpc(JSON.stringify({ ...value, checkedInfrastructureSha256: '00'.repeat(32) }), /nonzero lowercase/);
    await refuseBeforeRpc(JSON.stringify({ ...value, checkedInfrastructureSha256: '11'.repeat(32) }), /bytes differ from their exact manifest digest/);
  });
});
