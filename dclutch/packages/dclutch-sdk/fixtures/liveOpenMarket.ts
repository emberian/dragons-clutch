import raw from './live-open-market.json';

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
