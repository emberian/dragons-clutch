import { PublicKey } from '@solana/web3.js';

import { ascii, fromHex, hex, pubkey, requireZero, sha256, slice, u16 } from './bytes';
import { type SolanaRpcClient } from './rpc';

const RAW_RECORD_SEED = new TextEncoder().encode('dclutch-raw-record-v1');
const FEE_POLICY_SCHEMA_RELEASE = fromHex(
  '281d896ec0ce69b52443420820bc580ef18ef297e139115df91cea91565c451d',
  'compiled Direct fee-policy release ID',
);

export type DirectFeePolicyObservation = Readonly<{
  address: string;
  observedSlot: string;
  contentDigest: string;
  feeBasisPoints: number;
  recipient: string;
}>;

/** Authenticate one immutable Direct fee-policy record from a real RPC read. */
export async function inspectDirectFeePolicy(
  client: SolanaRpcClient,
  protocolProgramText: string,
  addressText: string,
  minimumContextSlot: string,
): Promise<DirectFeePolicyObservation> {
  const protocolProgram = new PublicKey(protocolProgramText);
  const address = new PublicKey(addressText);
  if (protocolProgram.toBase58() !== protocolProgramText || address.toBase58() !== addressText) throw new Error('fee-policy inputs must be canonical base58 text');
  const observation = await client.accountInfo(addressText, minimumContextSlot);
  if (observation.account === null) throw new Error('fee-policy record is absent');
  const account = observation.account;
  if (account.owner !== protocolProgramText || account.executable) throw new Error('fee-policy record has the wrong owner or is executable');
  if (account.data.length !== 48 || ascii(account.data, 0, 8) !== 'DCLTFEE3' || u16(account.data, 8) !== 3) throw new Error('fee-policy record has an unsupported exact layout');
  requireZero(account.data, 12, 4, 'fee-policy reserved region');
  const feeBasisPoints = u16(account.data, 10);
  if (feeBasisPoints > 10_000) throw new Error('fee-policy basis points exceed the exact denominator');
  const recipient = pubkey(slice(account.data, 16, 32), 'fee recipient');
  const digest = await sha256(account.data);
  const [derived] = PublicKey.findProgramAddressSync([RAW_RECORD_SEED, FEE_POLICY_SCHEMA_RELEASE, digest], protocolProgram);
  if (!derived.equals(address)) throw new Error('fee-policy address is not the canonical content-derived raw-record PDA');
  return Object.freeze({
    address: addressText,
    observedSlot: observation.slot,
    contentDigest: hex(digest),
    feeBasisPoints,
    recipient,
  });
}
