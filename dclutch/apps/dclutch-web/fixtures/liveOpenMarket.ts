import raw from './live-open-market.json';

import {
  CORE_STATE_BYTES,
  CORE_STATE_MAGIC,
  CORE_STATE_PRINCIPAL_CAP_SETS_OFFSET,
  CORE_STATE_RENT_BENEFICIARY_OFFSET,
  CORE_STATE_TERMINAL_RECEIPT_OFFSET,
  CORE_STATE_VERSION_OFFSET,
  CORE_VERSION,
} from '../lib/generated/coreFound';

/**
 * The first locally OPEN dClutch Market, as bytes.
 *
 * Test support, not shipped code. `live-open-market.json` holds finalized
 * account bytes copied verbatim off a successor-campaign validator by
 * `tools/gauntlet/frontend/expect.mjs --fixture-out`; this module turns them
 * into the shapes a browser read produces so surface tests can run against real
 * chain state instead of buffers this repository built to agree with itself.
 *
 * Adversarial cases mutate a copy of the real bytes, which is stronger than
 * constructing a plausible-looking forgery: the only thing wrong with the input
 * is the one field the case is about.
 */

export type LiveAccount = Readonly<{
  address: string;
  owner: string;
  lamports: string;
  data: Uint8Array;
}>;

function bytes(value: string, field: string): Uint8Array {
  if (value.length % 2 !== 0 || !/^[0-9a-f]*$/.test(value)) throw new Error(`${field} is not whole lowercase hexadecimal bytes`);
  return Uint8Array.from(value.match(/../g) ?? [], (pair) => Number.parseInt(pair, 16));
}

function account(name: keyof typeof raw.accounts): LiveAccount {
  const entry = raw.accounts[name];
  if (entry === null || entry === undefined) throw new Error(`the live fixture omits ${name}`);
  return Object.freeze({
    address: entry.address,
    owner: entry.owner,
    lamports: entry.lamports,
    data: bytes(entry.dataHex, `${name} bytes`),
  });
}

export const LIVE = Object.freeze({
  sourceRevision: raw.sourceRevision,
  observedFloorSlot: raw.observedFloorSlot,
  programs: raw.programs,
  founder: raw.founder,
  market: account('market'),
  claimsAggregate: account('claimsAggregate'),
  founderPosition: account('founderPosition'),
  realmRecord: account('realmRecord'),
  hoardVault: account('hoardVault'),
});

/** One RPC account projection over live bytes, with optional substitutions. */
export function liveRpcAccount(
  source: LiveAccount,
  overrides: Readonly<{ owner?: string; data?: Uint8Array }> = {},
): Readonly<{ data: Uint8Array; executable: boolean; lamports: string; owner: string; space: number }> {
  const data = overrides.data ?? source.data;
  return Object.freeze({
    data,
    executable: false,
    lamports: source.lamports,
    owner: overrides.owner ?? source.owner,
    space: data.length,
  });
}

/** A copy of live bytes with exactly one field replaced. */
export function mutate(source: Uint8Array, offset: number, replacement: Uint8Array | number): Uint8Array {
  const copy = new Uint8Array(source);
  if (typeof replacement === 'number') copy[offset] = replacement;
  else copy.set(replacement, offset);
  return copy;
}

/** Exact width of the recorded superseded Core V2 account. */
const LEGACY_CORE_V2_STATE_BYTES = 352;

/**
 * Test-only current Core envelope over the recorded Market's unchanged seeds.
 *
 * The captured chain account is the superseded 352-byte Core V2 generation and
 * stays byte-for-byte intact as historical evidence. Current reader tests need
 * a current-width Core V3 account, but no current public Market has been
 * recorded yet. This helper performs the exact V2-to-V3 physical migration:
 * everything before the new principal cap is preserved, the cap is inserted,
 * and the existing rent beneficiary and terminal receipt move eight bytes
 * later. Every byte past the terminal receipt is left zero, which is what the
 * current state means by an unrecorded bump and a canonical reserved span.
 * It is synthetic parser support, never presented as chain evidence.
 */
export function currentCoreMarketV3(principalCapSets: bigint = 500_000_000n): Uint8Array {
  const source = LIVE.market.data;
  if (source.length !== LEGACY_CORE_V2_STATE_BYTES) throw new Error('the recorded Core V2 fixture has an unexpected width');
  if (principalCapSets <= 0n || principalCapSets > 0xffff_ffff_ffff_ffffn) {
    throw new Error('the current Core fixture principal cap must be a nonzero u64');
  }
  const legacyRentBeneficiaryOffset = source.length - 64;
  const legacyTerminalReceiptOffset = source.length - 32;
  if (legacyRentBeneficiaryOffset !== CORE_STATE_PRINCIPAL_CAP_SETS_OFFSET) {
    throw new Error('the recorded Core V2 fixture does not meet the V3 insertion boundary');
  }

  const current = new Uint8Array(CORE_STATE_BYTES);
  current.set(source.slice(0, legacyRentBeneficiaryOffset));
  current.set(CORE_STATE_MAGIC, 0);
  const view = new DataView(current.buffer);
  view.setUint16(CORE_STATE_VERSION_OFFSET, CORE_VERSION, true);
  view.setBigUint64(CORE_STATE_PRINCIPAL_CAP_SETS_OFFSET, principalCapSets, true);
  current.set(source.slice(legacyRentBeneficiaryOffset, legacyTerminalReceiptOffset), CORE_STATE_RENT_BENEFICIARY_OFFSET);
  current.set(source.slice(legacyTerminalReceiptOffset), CORE_STATE_TERMINAL_RECEIPT_OFFSET);
  return current;
}
