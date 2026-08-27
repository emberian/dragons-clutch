import { PublicKey } from '@solana/web3.js';

import { fromHex, hex, sha256 } from './bytes';
import { BindingCheck } from './decoders';
import { SolanaRpcClient } from './rpc';

const RAW_RECORD_SEED = new TextEncoder().encode('dclutch-raw-record-v1');
const STAGING_RECORD_SEED = new TextEncoder().encode('dclutch-record-stage-v1');

export type RecordObservation = Readonly<{
  status: 'structurally-final' | 'refused';
  floorSlot: string;
  rawObservedSlot: string;
  stagingObservedSlot: string;
  rawAddress: string;
  stagingAddress: string;
  contentBytes: string | null;
  checks: ReadonlyArray<BindingCheck>;
  semanticDisposition: 'schema-validator-not-present-in-browser';
}>;

export function deriveRecordAddresses(programId: string, schemaReleaseIdHex: string, contentDigestHex: string): Readonly<{ raw: string; staging: string }> {
  const program = new PublicKey(programId);
  const schema = fromHex(schemaReleaseIdHex, 'schema/release ID');
  const digest = fromHex(contentDigestHex, 'content digest');
  const [raw] = PublicKey.findProgramAddressSync([RAW_RECORD_SEED, schema, digest], program);
  const [staging] = PublicKey.findProgramAddressSync([STAGING_RECORD_SEED, schema, digest], program);
  return Object.freeze({ raw: raw.toBase58(), staging: staging.toBase58() });
}

export async function inspectFinalizedRecord(
  client: SolanaRpcClient,
  programId: string,
  schemaReleaseIdHex: string,
  contentDigestHex: string,
): Promise<RecordObservation> {
  const canonicalProgram = new PublicKey(programId).toBase58();
  if (canonicalProgram !== programId) throw new Error('program ID must be canonical base58 text');
  const addresses = deriveRecordAddresses(programId, schemaReleaseIdHex, contentDigestHex);
  const floorSlot = await client.finalizedSlot();
  const [raw, staging] = await Promise.all([
    client.accountInfo(addresses.raw, floorSlot),
    client.accountInfo(addresses.staging, floorSlot),
  ]);
  const checks: BindingCheck[] = [
    Object.freeze({ label: 'Raw-record PDA', ok: true, detail: addresses.raw }),
    Object.freeze({ label: 'Staging-cursor PDA', ok: true, detail: addresses.staging }),
  ];
  let contentBytes: string | null = null;
  if (raw.account === null) {
    checks.push(Object.freeze({ label: 'Raw record present', ok: false, detail: `absent at finalized context ${raw.slot}` }));
  } else {
    contentBytes = String(raw.account.data.length);
    checks.push(Object.freeze({ label: 'Raw record owner', ok: raw.account.owner === canonicalProgram && !raw.account.executable, detail: `${raw.account.owner}; executable=${raw.account.executable}` }));
    const observedDigest = hex(await sha256(raw.account.data));
    checks.push(Object.freeze({ label: 'Exact content digest', ok: observedDigest === contentDigestHex, detail: observedDigest }));
  }
  checks.push(Object.freeze({
    label: 'Canonical staging cursor absent',
    ok: staging.account === null,
    detail: staging.account === null ? `absent at finalized context ${staging.slot}` : `present with ${staging.account.data.length} bytes at finalized context ${staging.slot}`,
  }));
  const ok = checks.every((check) => check.ok);
  return Object.freeze({
    status: ok ? 'structurally-final' : 'refused',
    floorSlot,
    rawObservedSlot: raw.slot,
    stagingObservedSlot: staging.slot,
    rawAddress: addresses.raw,
    stagingAddress: addresses.staging,
    contentBytes,
    checks: Object.freeze(checks),
    semanticDisposition: 'schema-validator-not-present-in-browser',
  });
}
