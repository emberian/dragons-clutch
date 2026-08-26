import { PublicKey } from '@solana/web3.js';

import {
  decodeCoreAccount,
  derivePositionAddressV1,
  verifyLocalBindings,
  type BindingCheck,
  type FullAccountObservation,
} from './decoders';
import {
  inspectMarketDiscoveryV1,
  MARKET_DISCOVERY_MAX_ADDRESSES,
  type MarketDiscoveryCardV1,
  type MarketProvenanceV1,
} from './marketDiscovery';
import { type RpcAccount, type SolanaRpcClient } from './rpc';

/**
 * A portfolio without an indexer.
 *
 * dClutch publishes no position index and this browser will not invent one. It
 * does not need one: a Position lives at the program-derived address of the
 * Position seed domain plus the exact Market and owner keys, so an owner plus a
 * set of Market addresses is enough to ask the chain directly. Every balance
 * below is a raw u64 atom count decoded from a finalized account this browser
 * read. A derived address that holds no account is reported as "no Position at
 * the derived address", which is the honest chain state, not an error and not a
 * zero balance invented on the reader's behalf.
 *
 * What this cannot do is find a Market the reader has not named. That gap is
 * real and is stated on the surface rather than papered over.
 */

export const PORTFOLIO_MAX_MARKETS = MARKET_DISCOVERY_MAX_ADDRESSES;
const RPC_ACCOUNT_BATCH = 32;

/** What the reader can still do with these exact balances, and why. */
export type PortfolioClaimV1 =
  | Readonly<{ kind: 'mergeable'; completeSetsAtoms: string; note: string }>
  | Readonly<{ kind: 'redeemable'; winningOutcome: number; redeemableAtoms: string; perOutcomeAtoms: ReadonlyArray<string>; note: string }>
  | Readonly<{ kind: 'unavailable'; note: string }>;

export type PortfolioPositionV1 =
  | Readonly<{
    status: 'held';
    address: string;
    provenance: MarketProvenanceV1;
    observedSlot: string;
    generation: string;
    outcomeCount: number;
    balances: ReadonlyArray<string>;
    claim: PortfolioClaimV1;
    bindings: ReadonlyArray<BindingCheck>;
  }>
  | Readonly<{ status: 'absent'; address: string; provenance: MarketProvenanceV1; observedSlot: string; note: string }>
  | Readonly<{ status: 'refused'; address: string; provenance: MarketProvenanceV1; observedSlot: string; reason: string }>;

export type PortfolioEntryV1 = Readonly<{
  marketAddress: string;
  positionAddress: string;
  market: MarketDiscoveryCardV1;
  position: PortfolioPositionV1;
}>;

export type PortfolioV1 = Readonly<{
  owner: string;
  coreProgramId: string;
  floorSlot: string;
  entries: ReadonlyArray<PortfolioEntryV1>;
  reason: string;
}>;

export type PortfolioRequestV1 = Readonly<{
  coreProgramId: string;
  owner: string;
  marketAddresses: ReadonlyArray<string>;
}>;

function canonical(value: string, field: string): string {
  let key: string;
  try {
    key = new PublicKey(value).toBase58();
  } catch {
    throw new Error(`${field} is not one canonical Solana address`);
  }
  if (key !== value) throw new Error(`${field} must be canonical base58 text`);
  return key;
}

/** Accept one owner identity from a wallet or from pasted text, identically. */
export function parsePortfolioOwnerV1(text: string): string {
  const owner = text.trim();
  if (owner.length === 0) throw new Error('an owner address is required; connect a browser wallet or paste one canonical address');
  return canonical(owner, 'owner address');
}

function chunks<T>(values: ReadonlyArray<T>, width: number): T[][] {
  const output: T[][] = [];
  for (let index = 0; index < values.length; index += width) output.push(values.slice(index, index + width));
  return output;
}

function observation(address: string, account: RpcAccount, observedSlot: string): FullAccountObservation {
  return Object.freeze({
    address,
    owner: account.owner,
    executable: account.executable,
    lamports: account.lamports,
    observedSlot,
    data: account.data,
  });
}

function minimum(values: ReadonlyArray<string>): string {
  return values.reduce((smallest, value) => (BigInt(value) < BigInt(smallest) ? value : smallest), values[0] ?? '0');
}

/**
 * What these exact balances still admit, decided by the Market's own phase and
 * settlement rather than by what would be convenient to display.
 */
export function portfolioClaimV1(market: MarketDiscoveryCardV1, balances: ReadonlyArray<string>): PortfolioClaimV1 {
  if (market.status !== 'decoded') {
    return Object.freeze({ kind: 'unavailable', note: 'The Market did not decode at this finalized floor, so nothing may be claimed about what these balances admit.' });
  }
  if (market.settlement.status === 'resolved') {
    if (market.phase !== 'Resolved' && market.phase !== 'Retiring') {
      return Object.freeze({ kind: 'unavailable', note: `Settlement is terminal but the Market is ${market.phase}, which admits no redemption.` });
    }
    const winner = market.settlement.winner;
    const redeemable = balances[winner] ?? '0';
    return Object.freeze({
      kind: 'redeemable',
      winningOutcome: winner,
      redeemableAtoms: redeemable,
      perOutcomeAtoms: Object.freeze(balances.map((amount, index) => (index === winner ? amount : '0'))),
      note: `Outcome ${winner} is frozen as winning. A winning claim pays exactly one collateral atom per claim atom; every losing claim pays zero, and those atoms are shown as zero rather than hidden.`,
    });
  }
  if (market.phase === 'Open') {
    return Object.freeze({
      kind: 'mergeable',
      completeSetsAtoms: minimum(balances),
      note: 'A complete set is one atom of every outcome. Merging burns one complete set and withdraws exactly one Hoard atom, so the count that can be merged is the smallest owned outcome balance. This is arithmetic on these balances, not an offer.',
    });
  }
  return Object.freeze({
    kind: 'unavailable',
    note: `Phase ${market.phase} admits neither complete-set merge nor redemption, so these balances admit no transition right now.`,
  });
}

function refusedPosition(address: string, observedSlot: string, reason: string): PortfolioPositionV1 {
  return Object.freeze({
    status: 'refused',
    address,
    provenance: Object.freeze({ kind: 'refused', reason }),
    observedSlot,
    reason,
  });
}

function projectPosition(
  address: string,
  account: RpcAccount | null,
  observedSlot: string,
  coreProgramId: string,
  owner: string,
  market: MarketDiscoveryCardV1,
): Promise<PortfolioPositionV1> | PortfolioPositionV1 {
  if (account === null) {
    return Object.freeze({
      status: 'absent',
      address,
      provenance: Object.freeze({ kind: 'chain', observedSlot }),
      observedSlot,
      note: 'No Position exists at the derived address. That is the chain state at this finalized floor, not a lookup failure: this owner has never held a claim in this Market, or the Position was closed.',
    });
  }
  const projection = decodeCoreAccount(observation(address, account, observedSlot), coreProgramId);
  if (projection.status !== 'decoded' || projection.semantics.kind !== 'Position') {
    return refusedPosition(address, observedSlot, projection.status === 'refused' ? projection.reason : 'the derived address holds a Core account that is not a Position');
  }
  const semantics = projection.semantics;
  if (semantics.owner !== owner) {
    return refusedPosition(address, observedSlot, `the account at the derived address names owner ${semantics.owner}, not ${owner}`);
  }
  if (semantics.market !== market.address) {
    return refusedPosition(address, observedSlot, `the account at the derived address names Market ${semantics.market}, not ${market.address}`);
  }
  if (market.status === 'decoded') {
    if (semantics.generation !== market.generation) {
      return refusedPosition(address, observedSlot, `the Position names generation ${semantics.generation} while the Market is at generation ${market.generation}; these are claims of a different Market incarnation and are not shown as current balances`);
    }
    if (semantics.outcomeCount !== market.outcomeCount) {
      return refusedPosition(address, observedSlot, `the Position is ${semantics.outcomeCount} outcomes wide while the Market is ${market.outcomeCount} wide`);
    }
  }
  return verifyLocalBindings(projection, coreProgramId).then((bound) => Object.freeze({
    status: 'held' as const,
    address,
    provenance: Object.freeze({ kind: 'chain' as const, observedSlot }),
    observedSlot,
    generation: semantics.generation,
    outcomeCount: semantics.outcomeCount,
    balances: semantics.balances,
    claim: portfolioClaimV1(market, semantics.balances),
    bindings: bound.bindings,
  }));
}

/**
 * Read one owner's Positions across the Markets they named.
 *
 * Markets and Positions are read behind one finalized floor so no entry mixes
 * observation epochs, and a Position whose generation or width disagrees with
 * its Market is refused rather than shown as a current balance.
 */
export async function inspectPortfolioV1(
  client: Pick<SolanaRpcClient, 'finalizedSlot' | 'multipleAccounts'>,
  request: PortfolioRequestV1,
): Promise<PortfolioV1> {
  const coreProgramId = canonical(request.coreProgramId, 'Core program');
  const owner = canonical(request.owner, 'owner address');
  const addresses = Object.freeze([...new Set(request.marketAddresses.map((address, index) => canonical(address, `Market address ${index + 1}`)))]);
  if (addresses.length > PORTFOLIO_MAX_MARKETS) {
    throw new Error(`portfolio requested ${addresses.length} Markets, above the explicit ${PORTFOLIO_MAX_MARKETS}-Market browser bound`);
  }
  const discovery = await inspectMarketDiscoveryV1(client, { coreProgramId, registryProgramId: null, addresses });
  if (addresses.length === 0) {
    return Object.freeze({
      owner,
      coreProgramId,
      floorSlot: discovery.floorSlot,
      entries: Object.freeze([]),
      reason: 'No Market has been named. A Position address is derived from a Market and an owner, so without a Market address there is nothing to derive and nothing to read.',
    });
  }

  const derived = new Map(addresses.map((address) => [address, derivePositionAddressV1(coreProgramId, address, owner)]));
  const positions = new Map<string, Readonly<{ account: RpcAccount | null; slot: string }>>();
  for (const group of chunks([...new Set(derived.values())], RPC_ACCOUNT_BATCH)) {
    const batch = await client.multipleAccounts(group, discovery.floorSlot);
    for (const entry of batch.accounts) positions.set(entry.address, Object.freeze({ account: entry.account, slot: batch.slot }));
  }

  const cards = new Map(discovery.cards.map((card) => [card.address, card]));
  const entries: PortfolioEntryV1[] = [];
  for (const marketAddress of addresses) {
    const positionAddress = derived.get(marketAddress) as string;
    const card = cards.get(marketAddress);
    const observed = positions.get(positionAddress);
    if (card === undefined) {
      entries.push(Object.freeze({
        marketAddress,
        positionAddress,
        market: Object.freeze({
          status: 'refused', address: marketAddress,
          provenance: Object.freeze({ kind: 'refused', reason: 'the Market was not returned by this finalized listing' }),
          observedSlot: discovery.floorSlot,
          refusal: 'the Market was not returned by this finalized listing',
        }),
        position: refusedPosition(positionAddress, discovery.floorSlot, 'the Market was not returned by this finalized listing, so its Position was not interpreted'),
      }));
      continue;
    }
    const position = observed === undefined
      ? refusedPosition(positionAddress, discovery.floorSlot, 'the derived Position address was not returned by this finalized read')
      : await projectPosition(positionAddress, observed.account, observed.slot, coreProgramId, owner, card);
    entries.push(Object.freeze({ marketAddress, positionAddress, market: card, position }));
  }

  const held = entries.filter((entry) => entry.position.status === 'held').length;
  const absent = entries.filter((entry) => entry.position.status === 'absent').length;
  return Object.freeze({
    owner,
    coreProgramId,
    floorSlot: discovery.floorSlot,
    entries: Object.freeze(entries),
    reason: `${held} of ${entries.length} derived Position${entries.length === 1 ? '' : 's'} hold state at finalized floor ${discovery.floorSlot}; ${absent} derived address${absent === 1 ? ' holds' : 'es hold'} no Position at all.`,
  });
}
