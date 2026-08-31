/**
 * The account view.
 *
 * Paste an address; get back an owner-aware decode. What "owner-aware" buys is
 * exact: the record's layout comes from its own leading magic, and every PDA
 * derivation is run under the account's ACTUAL owner program rather than under
 * a program the reader guessed — so a Claims position sitting under the wrong
 * program is visibly not derivable, instead of quietly annotated as if it were.
 *
 * The three states an account can be in here, and what each means:
 *
 *   `decoded`      — its magic is one `lib/generated/` declares, and it is
 *                    rendered against that schema. Fields that lie outside the
 *                    observed bytes refuse individually; the rest still show.
 *   `unrecognized` — it has a printable eight-byte header that no generated
 *                    module declares, or none at all. Shown as hex with the
 *                    header text if there is one. Never matched to a near-miss.
 *   `empty`        — no account at this address at the finalized floor.
 *
 * Rent is reported, not judged: an account below its exemption minimum is a
 * fact about the chain, and the explorer says the number rather than deciding
 * what it means.
 */
import type { SolanaRpcClient } from '../rpc';
import {
  decodeAgainstSpec,
  leadingMagic,
  specForData,
  type DecodedRecord,
} from './accountRecords';
import {
  derivationsForRecord,
  identifyFinalizedRecord,
  realmContentDerivation,
  type Derivation,
  type RecordIdentification,
} from './derivations';
import { programLabel } from './instructions';

export type RentStatus = Readonly<{
  lamports: string;
  exemptionMinimum: string | null;
  /** `null` when the exemption minimum could not be read. */
  exempt: boolean | null;
  note: string;
}>;

export type ExplorerAccount = Readonly<{
  address: string;
  observedSlot: string;
  floorSlot: string;
  owner: string;
  ownerLabel: string | null;
  executable: boolean;
  dataBytes: number;
  rent: RentStatus;
  /** The eight-byte header as text, when printable. */
  header: string | null;
  /** The first bytes in hex, always, so an unrecognized account still shows something. */
  headHex: string;
  decoded: DecodedRecord | null;
  derivations: ReadonlyArray<Derivation>;
  record: RecordIdentification | null;
  /** What could not be said, and why. */
  note: string | null;
}>;

export type ExplorerAccountResult =
  | Readonly<{ status: 'found'; account: ExplorerAccount }>
  | Readonly<{ status: 'empty'; address: string; floorSlot: string; observedSlot: string; reason: string }>;

export type ExplorerAccountRequest = Readonly<{
  address: string;
  programLabels?: Readonly<Record<string, string>>;
}>;

const HEAD_HEX_BYTES = 32;

function hexOf(bytes: Uint8Array): string {
  return Array.from(bytes, (byte) => byte.toString(16).padStart(2, '0')).join('');
}

async function rentStatus(
  client: Pick<SolanaRpcClient, 'minimumBalanceForRentExemption'>,
  lamports: string,
  dataBytes: number,
): Promise<RentStatus> {
  try {
    const observation = await client.minimumBalanceForRentExemption(dataBytes);
    const exempt = BigInt(lamports) >= BigInt(observation.lamports);
    return Object.freeze({
      lamports,
      exemptionMinimum: observation.lamports,
      exempt,
      note: exempt
        ? 'Its balance covers the cost of storing it, so the runtime will keep it.'
        : 'Its balance does not cover the cost of storing it. The runtime will reclaim it.',
    });
  } catch (error) {
    return Object.freeze({
      lamports,
      exemptionMinimum: null,
      exempt: null,
      note: `The minimum balance for an account this size could not be read: ${error instanceof Error ? error.message : 'the request failed'}`,
    });
  }
}

/** Read one account at a finalized floor and decode it against its own magic. */
export async function inspectAccount(
  client: Pick<SolanaRpcClient, 'accountInfo' | 'finalizedSlot' | 'minimumBalanceForRentExemption'>,
  request: ExplorerAccountRequest,
): Promise<ExplorerAccountResult> {
  const floorSlot = await client.finalizedSlot();
  const observation = await client.accountInfo(request.address, floorSlot);
  if (observation.account === null) {
    return Object.freeze({
      status: 'empty',
      address: request.address,
      floorSlot,
      observedSlot: observation.slot,
      reason: `No account exists at this address at finalized slot ${observation.slot}.`,
    });
  }

  const account = observation.account;
  const data = account.data;
  const spec = specForData(data);
  const decoded = spec === null ? null : decodeAgainstSpec(spec, data);

  const derivations: Derivation[] = [];
  if (decoded !== null) {
    derivations.push(...derivationsForRecord(decoded, data, request.address, account.owner));
    if (decoded.spec.name === 'Realm') {
      const realm = await realmContentDerivation(data, request.address, account.owner);
      if (realm !== null) derivations.push(realm);
    }
  }

  const record = await identifyFinalizedRecord(request.address, account.owner, data);

  let note: string | null = null;
  if (decoded === null) {
    const header = leadingMagic(data);
    note =
      header === null
        ? 'The first eight bytes are not readable text, so this account carries no dClutch record magic.'
        : `The protocol declares no record with the magic ${header}.`;
  } else if (derivations.length === 0) {
    note =
      'This record does not carry the seeds its address would be derived from.';
  }

  return Object.freeze({
    status: 'found',
    account: Object.freeze({
      address: request.address,
      observedSlot: observation.slot,
      floorSlot,
      owner: account.owner,
      ownerLabel: programLabel(account.owner, request.programLabels ?? {}),
      executable: account.executable,
      dataBytes: data.length,
      rent: await rentStatus(client, account.lamports, data.length),
      header: leadingMagic(data),
      headHex: hexOf(data.slice(0, HEAD_HEX_BYTES)),
      decoded,
      derivations: Object.freeze(derivations),
      record,
      note,
    }),
  });
}
