import { PublicKey } from '@solana/web3.js';

import {
  decodeClaimsPositionV2,
  deriveClaimsAggregateAddressV2,
  deriveClaimsPositionAddressV2,
} from './marketCoreV2';
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
 * does not need one: a claim balance lives at a program-derived address, so an
 * owner plus a set of Market addresses is enough to ask the chain directly.
 *
 * WHERE it lives is the part that has to be right. A generic founding puts the
 * founder's claims in a Claims-owned LiabilityBasisV2 Position at
 * `[dclutch:lbv2:position, aggregate, owner]`, where the aggregate is itself
 * `[dclutch:lbv2:market, market]` under the same Claims program. Deriving
 * instead from the Core Realm-Position domain — the Direct family's Position —
 * produces a perfectly plausible address that holds nothing, and reporting that
 * as "no Position" is a confident false negative about an owner who holds
 * claims. Measured on a live chain on 2026-08-27: it said exactly that about
 * the founder of the market.
 *
 * So the Claims program is REQUIRED here. Without one, this surface refuses to
 * derive anything rather than deriving the wrong thing.
 *
 * Every balance below is a raw u64 atom count decoded from a finalized account
 * this browser read. A derived address that holds no account is reported as "no
 * Position at the derived address", which is the honest chain state, not an
 * error and not a zero invented on the reader's behalf.
 *
 * What this cannot do is find a Market the reader has not named. That gap is
 * real and is stated on the surface rather than papered over.
 */

export const PORTFOLIO_MAX_MARKETS = MARKET_DISCOVERY_MAX_ADDRESSES;
const RPC_ACCOUNT_BATCH = 32;

/** What the reader can still do with these exact balances, and why. */
export type PortfolioClaimV1 =
  | Readonly<{ kind: 'mergeable'; completeSetsAtoms: string; note: string }>
  | Readonly<{ kind: 'redeemable'; winningClaim: number; redeemableAtoms: string; perClaimAtoms: ReadonlyArray<string>; note: string }>
  | Readonly<{ kind: 'unavailable'; note: string }>;

export type PortfolioPositionV1 =
  | Readonly<{
    status: 'held';
    address: string;
    provenance: MarketProvenanceV1;
    observedSlot: string;
    aggregateAddress: string;
    revision: string;
    claimCount: number;
    liabilityBasisId: string;
    balances: ReadonlyArray<string>;
    claim: PortfolioClaimV1;
  }>
  | Readonly<{ status: 'absent'; address: string; provenance: MarketProvenanceV1; observedSlot: string; note: string }>
  | Readonly<{ status: 'refused'; address: string | null; provenance: MarketProvenanceV1; observedSlot: string; reason: string }>;

export type PortfolioEntryV1 = Readonly<{
  marketAddress: string;
  positionAddress: string | null;
  aggregateAddress: string | null;
  market: MarketDiscoveryCardV1;
  position: PortfolioPositionV1;
}>;

export type PortfolioV1 = Readonly<{
  owner: string;
  coreProgramId: string;
  claimsProgramId: string | null;
  registryProgramId: string | null;
  floorSlot: string;
  entries: ReadonlyArray<PortfolioEntryV1>;
  reason: string;
}>;

export type PortfolioRequestV1 = Readonly<{
  coreProgramId: string;
  claimsProgramId?: string | null;
  registryProgramId?: string | null;
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

function optional(value: string | null | undefined, field: string): string | null {
  return value === undefined || value === null || value === '' ? null : canonical(value, field);
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
  if (market.settlement.status === 'terminal') {
    if (market.phase !== 'Terminal' && market.phase !== 'Retiring') {
      return Object.freeze({ kind: 'unavailable', note: `A terminal receipt is written but the Market is ${market.phase}, which admits no redemption.` });
    }
    const winner = market.settlement.winner;
    const redeemable = balances[winner] ?? '0';
    return Object.freeze({
      kind: 'redeemable',
      winningClaim: winner,
      redeemableAtoms: redeemable,
      perClaimAtoms: Object.freeze(balances.map((amount, index) => (index === winner ? amount : '0'))),
      note: `Claim ${winner} is frozen as winning. A winning claim pays exactly one collateral atom per claim atom; every losing claim pays zero, and those atoms are shown as zero rather than hidden.`,
    });
  }
  if (market.phase === 'Open') {
    return Object.freeze({
      kind: 'mergeable',
      completeSetsAtoms: minimum(balances),
      note: 'A complete set is one atom of every claim. Merging burns one complete set and withdraws exactly one collateral atom, so the count that can be merged is the smallest owned claim balance. This is arithmetic on these balances, not an offer.',
    });
  }
  return Object.freeze({
    kind: 'unavailable',
    note: `Phase ${market.phase} admits neither complete-set merge nor redemption, so these balances admit no transition right now.`,
  });
}

function refusedPosition(address: string | null, observedSlot: string, reason: string): PortfolioPositionV1 {
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
  aggregateAddress: string,
  account: RpcAccount | null,
  observedSlot: string,
  claimsProgramId: string,
  owner: string,
  market: MarketDiscoveryCardV1,
): PortfolioPositionV1 {
  if (account === null) {
    return Object.freeze({
      status: 'absent',
      address,
      provenance: Object.freeze({ kind: 'chain', observedSlot }),
      observedSlot,
      note: `No Claims Position exists at ${address}, the address this Market and owner derive under the selected Claims program. That is the chain state at this finalized floor, not a lookup failure: this owner has never been admitted to this Market's liability basis, or the Position was closed.`,
    });
  }
  if (account.owner !== claimsProgramId || account.executable) {
    return refusedPosition(address, observedSlot, `the derived address holds an account the selected Claims program does not own (owner ${account.owner})`);
  }
  let position;
  try {
    position = decodeClaimsPositionV2(address, account.data);
  } catch (error) {
    return refusedPosition(address, observedSlot, error instanceof Error ? error.message : 'the derived address did not decode as one canonical Claims Position');
  }
  if (position.owner !== owner) {
    return refusedPosition(address, observedSlot, `the account at the derived address names owner ${position.owner}, not ${owner}`);
  }
  if (position.aggregate !== aggregateAddress) {
    return refusedPosition(address, observedSlot, `the Position names Claims aggregate ${position.aggregate}, not the ${aggregateAddress} this Market derives`);
  }
  if (market.status === 'decoded' && market.liability.status === 'bound') {
    if (position.claimCount !== market.liability.claimCount) {
      return refusedPosition(address, observedSlot, `the Position is ${position.claimCount} claims wide while the Market's liability basis is ${market.liability.claimCount} wide`);
    }
    if (position.liabilityBasisId !== market.liability.liabilityBasisId) {
      return refusedPosition(address, observedSlot, 'the Position names a different liability basis than the Market aggregate; these are claims of a different basis and are not shown as current balances');
    }
  }
  return Object.freeze({
    status: 'held',
    address,
    provenance: Object.freeze({ kind: 'chain', observedSlot }),
    observedSlot,
    aggregateAddress,
    revision: position.revision,
    claimCount: position.claimCount,
    liabilityBasisId: position.liabilityBasisId,
    balances: position.balances,
    claim: portfolioClaimV1(market, position.balances),
  });
}

/**
 * Read one owner's Claims Positions across the Markets they named.
 *
 * Markets and Positions are read behind one finalized floor so no entry mixes
 * observation epochs, and a Position whose owner, aggregate, width or basis
 * disagrees with its Market is refused rather than shown as a current balance.
 */
export async function inspectPortfolioV1(
  client: Pick<SolanaRpcClient, 'finalizedSlot' | 'multipleAccounts'>,
  request: PortfolioRequestV1,
): Promise<PortfolioV1> {
  const coreProgramId = canonical(request.coreProgramId, 'Core program');
  const claimsProgramId = optional(request.claimsProgramId, 'Claims program');
  const registryProgramId = optional(request.registryProgramId, 'Registry program');
  const owner = canonical(request.owner, 'owner address');
  const addresses = Object.freeze([...new Set(request.marketAddresses.map((address, index) => canonical(address, `Market address ${index + 1}`)))]);
  if (addresses.length > PORTFOLIO_MAX_MARKETS) {
    throw new Error(`portfolio requested ${addresses.length} Markets, above the explicit ${PORTFOLIO_MAX_MARKETS}-Market browser bound`);
  }
  const discovery = await inspectMarketDiscoveryV1(client, { coreProgramId, registryProgramId, claimsProgramId, addresses });
  if (addresses.length === 0) {
    return Object.freeze({
      owner,
      coreProgramId,
      claimsProgramId,
      registryProgramId,
      floorSlot: discovery.floorSlot,
      entries: Object.freeze([]),
      reason: 'No Market has been named. A Position address is derived from a Market and an owner, so without a Market address there is nothing to derive and nothing to read.',
    });
  }

  const cards = new Map(discovery.cards.map((card) => [card.address, card]));
  const noClaims = 'No Claims program was selected. Claim balances live in Claims-owned LiabilityBasisV2 Positions, so without that program no Position address can be derived — and this browser will not derive a different family\'s Position address and report its emptiness as an answer.';

  if (claimsProgramId === null) {
    const entries = addresses.map((marketAddress) => Object.freeze({
      marketAddress,
      positionAddress: null,
      aggregateAddress: null,
      market: cards.get(marketAddress) ?? Object.freeze({
        status: 'refused' as const,
        address: marketAddress,
        provenance: Object.freeze({ kind: 'refused' as const, reason: 'the Market was not returned by this finalized listing' }),
        observedSlot: discovery.floorSlot,
        refusal: 'the Market was not returned by this finalized listing',
      }),
      position: refusedPosition(null, discovery.floorSlot, noClaims),
    }));
    return Object.freeze({
      owner, coreProgramId, claimsProgramId, registryProgramId,
      floorSlot: discovery.floorSlot,
      entries: Object.freeze(entries),
      reason: noClaims,
    });
  }

  const derived = new Map(addresses.map((marketAddress) => {
    const aggregate = deriveClaimsAggregateAddressV2(claimsProgramId, marketAddress);
    return [marketAddress, Object.freeze({ aggregate, position: deriveClaimsPositionAddressV2(claimsProgramId, aggregate, owner) })];
  }));
  const observed = new Map<string, Readonly<{ account: RpcAccount | null; slot: string }>>();
  for (const group of chunks([...new Set([...derived.values()].map((entry) => entry.position))], RPC_ACCOUNT_BATCH)) {
    const batch = await client.multipleAccounts(group, discovery.floorSlot);
    for (const entry of batch.accounts) observed.set(entry.address, Object.freeze({ account: entry.account, slot: batch.slot }));
  }

  const entries: PortfolioEntryV1[] = [];
  for (const marketAddress of addresses) {
    const coordinates = derived.get(marketAddress) as Readonly<{ aggregate: string; position: string }>;
    const card = cards.get(marketAddress);
    if (card === undefined) {
      const reason = 'the Market was not returned by this finalized listing';
      entries.push(Object.freeze({
        marketAddress,
        positionAddress: coordinates.position,
        aggregateAddress: coordinates.aggregate,
        market: Object.freeze({
          status: 'refused', address: marketAddress,
          provenance: Object.freeze({ kind: 'refused', reason }),
          observedSlot: discovery.floorSlot,
          refusal: reason,
        }),
        position: refusedPosition(coordinates.position, discovery.floorSlot, `${reason}, so its Position was not interpreted`),
      }));
      continue;
    }
    const entry = observed.get(coordinates.position);
    const position = entry === undefined
      ? refusedPosition(coordinates.position, discovery.floorSlot, 'the derived Position address was not returned by this finalized read')
      : projectPosition(coordinates.position, coordinates.aggregate, entry.account, entry.slot, claimsProgramId, owner, card);
    entries.push(Object.freeze({ marketAddress, positionAddress: coordinates.position, aggregateAddress: coordinates.aggregate, market: card, position }));
  }

  const held = entries.filter((entry) => entry.position.status === 'held').length;
  const absent = entries.filter((entry) => entry.position.status === 'absent').length;
  return Object.freeze({
    owner,
    coreProgramId,
    claimsProgramId,
    registryProgramId,
    floorSlot: discovery.floorSlot,
    entries: Object.freeze(entries),
    reason: `${held} of ${entries.length} derived Claims Position${entries.length === 1 ? '' : 's'} hold state at finalized floor ${discovery.floorSlot}; ${absent} derived address${absent === 1 ? ' holds' : 'es hold'} no Position at all.`,
  });
}
