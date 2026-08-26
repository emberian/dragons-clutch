import { PublicKey } from '@solana/web3.js';

import { ascii, hex, isZero, pubkey, requireNonzero, requireZero, sha256, slice, u16, u64 } from './bytes';

export type CoreKind = 'Market' | 'Realm' | 'Position' | 'RentCredit';

export type BindingCheck = Readonly<{
  label: string;
  ok: boolean;
  detail: string;
}>;

type MarketSemantics = Readonly<{
  kind: 'Market';
  realmId: string;
  generation: string;
  outcomeCount: number;
  phase: 'Founding' | 'Open' | 'Resolved' | 'Retiring' | 'Retired';
  identityBytes: Uint8Array;
}>;

type RealmSemantics = Readonly<{
  kind: 'Realm';
  canonicalBytes: Uint8Array;
  contentDigest: string | null;
}>;

type PositionSemantics = Readonly<{
  kind: 'Position';
  market: string;
  owner: string;
  generation: string;
}>;

type RentCreditSemantics = Readonly<{
  kind: 'RentCredit';
  refundWallet: string;
  market: string;
  marketBytes: Uint8Array;
  releaseSet: string;
  generation: string;
  bump: number;
}>;

export type DecodedProjection = Readonly<{
  status: 'decoded';
  kind: CoreKind;
  address: string;
  lamports: string;
  observedSlot: string;
  schema: 'v1' | 'v2';
  details: ReadonlyArray<Readonly<{ label: string; value: string }>>;
  bindings: ReadonlyArray<BindingCheck>;
  semantics: MarketSemantics | RealmSemantics | PositionSemantics | RentCreditSemantics;
}>;

export type RefusedProjection = Readonly<{
  status: 'refused';
  kind: 'Unknown' | CoreKind;
  address: string;
  lamports: string;
  observedSlot: string;
  reason: string;
  header: string;
}>;

export type AccountProjection = DecodedProjection | RefusedProjection;

export type FullAccountObservation = Readonly<{
  address: string;
  owner: string;
  executable: boolean;
  lamports: string;
  observedSlot: string;
  data: Uint8Array;
}>;

const MAGIC = Object.freeze({
  DCLTCAT1: 'Market',
  DCLTRLM1: 'Realm',
  DCLTPOS1: 'Position',
  DCLRNTL2: 'RentCredit',
} satisfies Record<string, CoreKind>);

const PHASES = Object.freeze(['Founding', 'Open', 'Resolved', 'Retiring', 'Retired']);
const RESOLUTION_KINDS = Object.freeze(['Occurrence', 'Failure', 'Recovery']);
const MARKET_SEED = new TextEncoder().encode('dclutch/market-root/v1');
const REALM_SEED = new TextEncoder().encode('dclutch/realm/v1');
const POSITION_SEED = new TextEncoder().encode('dclutch/position/v1');
const RENT_CREDIT_SEED = new TextEncoder().encode('dclutch/rent-market/v2');

function detail(label: string, value: string | number | bigint): Readonly<{ label: string; value: string }> {
  return Object.freeze({ label, value: String(value) });
}

function sameIdentity(left: Uint8Array, right: Uint8Array): boolean {
  return left.length === right.length && left.every((value, index) => value === right[index]);
}

function kindFromMagic(data: Uint8Array): CoreKind | null {
  if (data.length < 8) return null;
  try {
    return MAGIC[ascii(data, 0, 8) as keyof typeof MAGIC] ?? null;
  } catch {
    return null;
  }
}

export function classifyHeader(data: Uint8Array): CoreKind | null {
  return kindFromMagic(data);
}

export function decodeCoreAccount(observation: FullAccountObservation, expectedProgramId: string): AccountProjection {
  const kind = kindFromMagic(observation.data);
  if (observation.owner !== expectedProgramId) {
    return refused(observation, kind ?? 'Unknown', 'account owner differs from the selected program ID');
  }
  if (observation.executable) {
    return refused(observation, kind ?? 'Unknown', 'executable program data is not a core state account');
  }
  if (kind === null) {
    return refused(observation, 'Unknown', 'unknown account magic; no layout was guessed');
  }
  try {
    if (kind === 'Market') return decodeMarket(observation);
    if (kind === 'Realm') return decodeRealm(observation);
    if (kind === 'Position') return decodePosition(observation);
    return decodeRentCredit(observation);
  } catch (error) {
    return refused(observation, kind, error instanceof Error ? error.message : 'canonical decoder refused the account');
  }
}

function refused(observation: FullAccountObservation, kind: 'Unknown' | CoreKind, reason: string): RefusedProjection {
  return Object.freeze({
    status: 'refused',
    kind,
    address: observation.address,
    lamports: observation.lamports,
    observedSlot: observation.observedSlot,
    reason,
    header: hex(observation.data.slice(0, 16)),
  });
}

function commonHeader(bytes: Uint8Array, magic: string, exactLength: number, version = 1): void {
  if (bytes.length !== exactLength) throw new Error(`expected exactly ${exactLength} bytes, observed ${bytes.length}`);
  if (ascii(bytes, 0, 8) !== magic) throw new Error(`magic is not ${magic}`);
  if (u16(bytes, 8) !== version) throw new Error(`schema version ${u16(bytes, 8)} is unsupported`);
}

function decodeMarket(observation: FullAccountObservation): DecodedProjection {
  const bytes = observation.data;
  if (bytes.length < 16) throw new Error('Market header is truncated');
  if (u16(bytes, 8) !== 1) throw new Error(`Market schema version ${u16(bytes, 8)} is unsupported`);
  const outcomeCount = bytes[10];
  if (bytes[11] !== 1) throw new Error(`categorical profile ${bytes[11]} is unsupported`);
  if (outcomeCount < 2 || outcomeCount > 16) throw new Error(`outcome count ${outcomeCount} is outside provisional profile 2..16`);
  const expectedLength = 320 + outcomeCount * 8;
  commonHeader(bytes, 'DCLTCAT1', expectedLength);
  requireZero(bytes, 12, 4, 'Market header');
  if (ascii(bytes, 16, 8) !== 'DCLTROOT') throw new Error('embedded MarketRoot magic is invalid');
  if (u16(bytes, 24) !== 1) throw new Error(`embedded MarketRoot schema ${u16(bytes, 24)} is unsupported`);
  requireZero(bytes, 26, 6, 'MarketRoot header');
  requireZero(bytes, 201, 7, 'MarketRoot body');

  const realmId = slice(bytes, 32, 32);
  const productInstanceId = slice(bytes, 64, 32);
  const claimBasisId = slice(bytes, 96, 32);
  const resolutionPolicyId = slice(bytes, 128, 32);
  const capabilityManifestId = slice(bytes, 160, 32);
  for (const [name, value] of [
    ['Realm ID', realmId],
    ['Product instance ID', productInstanceId],
    ['claim basis ID', claimBasisId],
    ['resolution policy ID', resolutionPolicyId],
    ['capability manifest ID', capabilityManifestId],
  ] as const) requireNonzero(value, name);

  const generation = u64(bytes, 192);
  const phaseByte = bytes[200];
  const phase = PHASES[phaseByte];
  if (phase === undefined) throw new Error(`MarketRoot phase ${phaseByte} is undefined`);
  const outstandingChildren = u64(bytes, 208);
  if (phase === 'Retired' && outstandingChildren !== 0n) throw new Error('Retired Market retains outstanding direct children');
  const rentRefund = slice(bytes, 216, 32);
  requireNonzero(rentRefund, 'Market rent-refund authority');
  const hoardAtoms = u64(bytes, 248);
  const supply = Array.from({ length: outcomeCount }, (_, index) => u64(bytes, 256 + index * 8));
  const settlementOffset = 256 + outcomeCount * 8;
  const settlement = slice(bytes, settlementOffset, 64);
  const status = settlement[0];
  let settlementLabel = 'Empty';
  let winner: number | null = null;
  let resolutionDetails: ReadonlyArray<Readonly<{ label: string; value: string }>> = [];
  if (status === 0) {
    if (!isZero(settlement)) throw new Error('empty settlement summary contains nonzero bytes');
  } else if (status === 1) {
    const route = RESOLUTION_KINDS[settlement[1]];
    if (route === undefined) throw new Error(`resolution route ${settlement[1]} is undefined`);
    winner = settlement[2];
    if (winner >= outcomeCount) throw new Error(`winning outcome ${winner} exceeds the Market width`);
    requireZero(settlement, 3, 5, 'settlement header');
    requireZero(settlement, 48, 16, 'settlement tail');
    const terminalSequence = u64(settlement, 8);
    if (terminalSequence === 0n) throw new Error('resolved settlement has a zero terminal sequence');
    const evidence = slice(settlement, 16, 32);
    requireNonzero(evidence, 'resolution evidence ID');
    settlementLabel = `Resolved · ${route}`;
    resolutionDetails = [detail('Winning state', winner), detail('Terminal sequence', terminalSequence), detail('Evidence ID', hex(evidence))];
  } else {
    throw new Error(`settlement status ${status} is undefined`);
  }

  const emptyEconomics = hoardAtoms === 0n && supply.every((amount) => amount === 0n);
  if (phase === 'Founding' && (status !== 0 || !emptyEconomics)) throw new Error('Founding Market retained settlement or economic state');
  if (phase === 'Open' && status !== 0) throw new Error('Open Market retained terminal settlement truth');
  if (phase === 'Resolved' && status !== 1) throw new Error('Resolved Market lacks terminal settlement truth');
  if (phase === 'Retiring' && status === 0 && !emptyEconomics) throw new Error('unresolved Retiring Market retains economic state');
  if (phase === 'Retired' && !emptyEconomics) throw new Error('Retired Market retains economic state');
  const requiredBacking = winner === null ? supply.reduce((maximum, amount) => amount > maximum ? amount : maximum, 0n) : supply[winner];
  if (hoardAtoms < requiredBacking) throw new Error(`Hoard ${hoardAtoms} is below exact claimant backing ${requiredBacking}`);

  return Object.freeze({
    status: 'decoded', kind: 'Market', address: observation.address, lamports: observation.lamports,
    observedSlot: observation.observedSlot, schema: 'v1', bindings: Object.freeze([]),
    details: Object.freeze([
      detail('Phase', phase), detail('Generation', generation), detail('Outcomes', outcomeCount),
      detail('Hoard atoms', hoardAtoms), detail('Outstanding children', outstandingChildren),
      detail('Settlement', settlementLabel), detail('Supply', supply.join(' · ')),
      detail('Realm ID', hex(realmId)), detail('Product instance ID', hex(productInstanceId)),
      detail('Claim basis ID', hex(claimBasisId)), detail('Resolution policy ID', hex(resolutionPolicyId)),
      detail('Capability manifest ID', hex(capabilityManifestId)), detail('Rent refund authority', pubkey(rentRefund, 'Market rent-refund authority')),
      ...resolutionDetails,
    ]),
    semantics: Object.freeze({ kind: 'Market', realmId: hex(realmId), generation: generation.toString(), outcomeCount, phase, identityBytes: slice(bytes, 32, 168) }),
  });
}

function decodeRealm(observation: FullAccountObservation): DecodedProjection {
  const bytes = observation.data;
  commonHeader(bytes, 'DCLTRLM1', 112);
  requireZero(bytes, 12, 4, 'Realm header');
  const mintPolicy = bytes[10] === 0 ? 'Require absent' : bytes[10] === 1 ? 'Admit issuer control' : null;
  const freezePolicy = bytes[11] === 0 ? 'Require absent' : bytes[11] === 1 ? 'Admit issuer control' : null;
  if (mintPolicy === null || freezePolicy === null) throw new Error('Realm authority policy byte is undefined');
  const tokenProgram = pubkey(slice(bytes, 16, 32), 'Realm token program');
  const collateralMint = pubkey(slice(bytes, 48, 32), 'Realm collateral mint');
  const adapterRelease = slice(bytes, 80, 32);
  requireNonzero(adapterRelease, 'collateral adapter release ID');
  return Object.freeze({
    status: 'decoded', kind: 'Realm', address: observation.address, lamports: observation.lamports,
    observedSlot: observation.observedSlot, schema: 'v1', bindings: Object.freeze([]),
    details: Object.freeze([
      detail('Token program', tokenProgram), detail('Collateral mint', collateralMint), detail('Adapter release ID', hex(adapterRelease)),
      detail('Mint authority policy', mintPolicy), detail('Freeze authority policy', freezePolicy),
    ]),
    semantics: Object.freeze({ kind: 'Realm', canonicalBytes: new Uint8Array(bytes), contentDigest: null }),
  });
}

function decodePosition(observation: FullAccountObservation): DecodedProjection {
  const bytes = observation.data;
  if (bytes.length < 16) throw new Error('Position header is truncated');
  if (u16(bytes, 8) !== 1) throw new Error(`Position schema version ${u16(bytes, 8)} is unsupported`);
  const outcomeCount = bytes[10];
  if (outcomeCount < 2 || outcomeCount > 16) throw new Error(`Position outcome count ${outcomeCount} is outside 2..16`);
  commonHeader(bytes, 'DCLTPOS1', 88 + outcomeCount * 8);
  requireZero(bytes, 11, 5, 'Position header');
  const market = pubkey(slice(bytes, 16, 32), 'Position Market');
  const owner = pubkey(slice(bytes, 48, 32), 'Position owner');
  const generation = u64(bytes, 80);
  const balances = Array.from({ length: outcomeCount }, (_, index) => u64(bytes, 88 + index * 8));
  return Object.freeze({
    status: 'decoded', kind: 'Position', address: observation.address, lamports: observation.lamports,
    observedSlot: observation.observedSlot, schema: 'v1', bindings: Object.freeze([]),
    details: Object.freeze([
      detail('Market', market), detail('Owner', owner), detail('Generation', generation),
      detail('Outcomes', outcomeCount), detail('Owned balances', balances.join(' · ')),
    ]),
    semantics: Object.freeze({ kind: 'Position', market, owner, generation: generation.toString() }),
  });
}

function decodeRentCredit(observation: FullAccountObservation): DecodedProjection {
  const bytes = observation.data;
  commonHeader(bytes, 'DCLRNTL2', 128, 2);
  requireZero(bytes, 11, 5, 'RentCredit header');
  requireZero(bytes, 120, 8, 'RentCredit body');
  const refundWallet = pubkey(slice(bytes, 16, 32), 'RentCredit refund wallet');
  const marketBytes = slice(bytes, 48, 32);
  const market = pubkey(marketBytes, 'RentCredit Market');
  const releaseSet = hex(slice(bytes, 80, 32));
  requireNonzero(slice(bytes, 80, 32), 'RentCredit release set');
  const generation = u64(bytes, 112);
  if (generation === 0n) throw new Error('RentCredit generation is zero');
  if (refundWallet === market || sameIdentity(slice(bytes, 16, 32), slice(bytes, 80, 32)) || sameIdentity(marketBytes, slice(bytes, 80, 32))) throw new Error('RentCredit lifecycle identities alias');
  const bump = bytes[10];
  return Object.freeze({
    status: 'decoded', kind: 'RentCredit', address: observation.address, lamports: observation.lamports,
    observedSlot: observation.observedSlot, schema: 'v2', bindings: Object.freeze([]),
    details: Object.freeze([
      detail('Refund wallet', refundWallet), detail('Market', market), detail('Generation', generation),
      detail('Execution release set', releaseSet), detail('Persisted PDA bump', bump), detail('Observed lamports', observation.lamports),
    ]),
    semantics: Object.freeze({ kind: 'RentCredit', refundWallet, market, marketBytes, releaseSet, generation: generation.toString(), bump }),
  });
}

export async function verifyLocalBindings(projection: DecodedProjection, programId: string): Promise<DecodedProjection> {
  const program = new PublicKey(programId);
  const checks: BindingCheck[] = [];
  let semantics = projection.semantics;
  if (semantics.kind === 'Market') {
    const digest = await sha256(semantics.identityBytes);
    const [derived] = PublicKey.findProgramAddressSync([MARKET_SEED, digest], program);
    checks.push(Object.freeze({ label: 'Market PDA', ok: derived.toBase58() === projection.address, detail: `sha256(identity) → ${derived.toBase58()}` }));
  } else if (semantics.kind === 'Realm') {
    const digest = await sha256(semantics.canonicalBytes);
    const [derived] = PublicKey.findProgramAddressSync([REALM_SEED, digest], program);
    checks.push(Object.freeze({ label: 'Realm content + PDA', ok: derived.toBase58() === projection.address, detail: `sha256(canonical bytes) ${hex(digest)} → ${derived.toBase58()}` }));
    semantics = Object.freeze({ ...semantics, contentDigest: hex(digest) });
  } else if (semantics.kind === 'Position') {
    const [derived] = PublicKey.findProgramAddressSync([POSITION_SEED, new PublicKey(semantics.market).toBytes(), new PublicKey(semantics.owner).toBytes()], program);
    checks.push(Object.freeze({ label: 'Position PDA', ok: derived.toBase58() === projection.address, detail: `Market + owner → ${derived.toBase58()}` }));
  } else {
    const generation = BigInt(semantics.generation);
    const generationBytes = new Uint8Array(8);
    new DataView(generationBytes.buffer).setBigUint64(0, generation, true);
    const [derived, bump] = PublicKey.findProgramAddressSync([RENT_CREDIT_SEED, semantics.marketBytes, generationBytes], program);
    checks.push(Object.freeze({ label: 'RentCredit PDA', ok: derived.toBase58() === projection.address && bump === semantics.bump, detail: `Market + generation → ${derived.toBase58()}, bump ${bump}` }));
  }
  return Object.freeze({ ...projection, bindings: Object.freeze(checks), semantics });
}

export function crossCheckBindings(projections: ReadonlyArray<AccountProjection>): ReadonlyArray<AccountProjection> {
  const decoded = projections.filter((projection): projection is DecodedProjection => projection.status === 'decoded');
  const realms = new Map(decoded.filter((projection) => projection.semantics.kind === 'Realm' && projection.semantics.contentDigest !== null).map((projection) => [projection.semantics.kind === 'Realm' ? projection.semantics.contentDigest : '', projection]));
  const markets = new Map(decoded.filter((projection) => projection.kind === 'Market').map((projection) => [projection.address, projection]));
  return projections.map((projection) => {
    if (projection.status !== 'decoded') return projection;
    const checks = [...projection.bindings];
    if (projection.semantics.kind === 'Market') {
      const realm = realms.get(projection.semantics.realmId);
      checks.push(Object.freeze({ label: 'Market → Realm content', ok: realm !== undefined, detail: realm ? `joined decoded Realm ${realm.address}` : `no canonical Realm with content ID ${projection.semantics.realmId}` }));
    } else if (projection.semantics.kind === 'Position') {
      const market = markets.get(projection.semantics.market);
      const generation = market?.semantics.kind === 'Market' ? market.semantics.generation : null;
      checks.push(Object.freeze({ label: 'Position → Market generation', ok: generation === projection.semantics.generation, detail: market ? `Position ${projection.semantics.generation}; Market ${generation}` : 'named Market was not decoded in this finalized scan' }));
    } else if (projection.semantics.kind === 'RentCredit') {
      const market = markets.get(projection.semantics.market);
      const generation = market?.semantics.kind === 'Market' ? market.semantics.generation : null;
      checks.push(Object.freeze({ label: 'RentCredit → Market lifecycle', ok: generation === projection.semantics.generation, detail: market ? `RentCredit ${projection.semantics.generation}; Market ${generation}` : 'bound Market was not decoded in this finalized scan' }));
    }
    return Object.freeze({ ...projection, bindings: Object.freeze(checks) });
  });
}
