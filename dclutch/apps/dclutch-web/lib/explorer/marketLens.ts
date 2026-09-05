/**
 * The Market lens: the record graph, as a navigable thing.
 *
 * A Market is not one account. It is a Core state whose nine identity seeds
 * name a Realm, a Product record, a product instance, a resolution policy, a
 * capability manifest and a release set; a Claims aggregate that holds the
 * liability those claims represent; a Hoard in a Custody namespace only the
 * aggregate records; a capability manifest with its funding quotes; and, once
 * it resolves, a terminal receipt. `lib/marketDiscovery.ts` already performs
 * that whole join and authenticates the parts it can. What it does not do is
 * make any of it navigable — every identity on the Market detail page today is
 * plain text.
 *
 * This module is the missing half. It takes the joined view and turns each
 * edge into a NODE with a target the explorer can open:
 *
 *   - an ADDRESS node opens the account view directly;
 *   - an IDENTITY node is a content digest, which is not an address. When the
 *     founding pairs that slot with a known record schema, the Registry
 *     raw-record PDA is derived so the node becomes openable — and the node
 *     says the address was DERIVED and not reacquired, because this client did
 *     not fetch it. Opening it re-hashes the bytes and settles the question,
 *     which is where that check belongs.
 *   - an identity with no known schema pairing stays a digest, and says so.
 *
 * The schema pairings come from `lib/coreFound.ts`'s founding construction —
 * the records a Found37 publishes, in the order the Market's seeds take
 * them — and every schema identity is imported from `lib/generated/coreFound`.
 */
import {
  CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
  EXECUTION_RELEASE_SET_SCHEMA_RELEASE_ID_V1,
  PRODUCT_RECORD_SCHEMA_ID_V2,
  REALM_SCHEMA_RELEASE_ID_V1,
  SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
} from '@dclutch/sdk/generated/coreFound';
import { inspectMarketDetailV1, type MarketDetailV1 } from '@dclutch/sdk/marketDetail';
import { deriveFinalizedRecordAddressesV1 } from '@dclutch/sdk/releaseRegistry';
import type { SolanaRpcClient } from '@dclutch/sdk/rpc';

/** Where a node's identity comes from, and how far it was checked. */
export type NodeProvenance =
  | Readonly<{ kind: 'observed'; slot: string }>
  | Readonly<{ kind: 'derived'; how: string }>
  | Readonly<{ kind: 'stated'; how: string }>
  | Readonly<{ kind: 'unavailable'; reason: string }>;

export type LensNode = Readonly<{
  id: string;
  /** Which band of the graph it sits in, for layout. */
  band: 'market' | 'identity' | 'liability' | 'collateral' | 'capability' | 'settlement';
  title: string;
  summary: string;
  /** The account this node opens, when it has one. */
  address: string | null;
  /** The content digest, for identity nodes. */
  contentId: string | null;
  provenance: NodeProvenance;
  facts: ReadonlyArray<Readonly<{ label: string; value: string }>>;
}>;

export type LensEdge = Readonly<{ from: string; to: string; label: string }>;

export type MarketLens = Readonly<{
  address: string;
  floorSlot: string;
  coreProgramId: string;
  registryProgramId: string | null;
  claimsProgramId: string | null;
  custodyProgramId: string | null;
  nodes: ReadonlyArray<LensNode>;
  edges: ReadonlyArray<LensEdge>;
  /**
   * The Market's own binding checks, carried through from the discovery join.
   *
   * `Extract`, not `extends ? :`. The card is a UNION -- decoded or refused --
   * and a conditional type distributes over it, so `card extends { bindings }`
   * asked whether the REFUSED arm has bindings, got no, and resolved the whole
   * field to `never`. Both projections below then cast real arrays to `never`
   * to satisfy it, which is how a field that is populated at runtime and
   * rendered on the page typechecked as a thing that cannot exist -- and why
   * `ChainExplorer` could not read its own `.length`.
   */
  bindings: Extract<MarketDetailV1['card'], Readonly<{ status: 'decoded' }>>['bindings'];
  /** What the lens could not show, and why. */
  gaps: ReadonlyArray<string>;
  detail: MarketDetailV1;
}>;

/**
 * The schema each Core identity seed is published under.
 *
 * A Market's seeds are digests, not addresses. These pairings are what makes
 * them openable, and they are exactly the seven records `prepareCoreFoundV2`
 * validates before it derives the Market — so the pairing is the founding's,
 * not this module's invention. The product INSTANCE identity is deliberately
 * absent: it is a digest read out of the product record's body, not a record of
 * its own, and deriving a record address for it would invent an account.
 */
const IDENTITY_SCHEMAS: ReadonlyArray<
  Readonly<{ field: string; schema: Uint8Array; title: string; summary: string }>
> = Object.freeze([
  {
    field: 'realmId',
    schema: REALM_SCHEMA_RELEASE_ID_V1,
    title: 'Realm record',
    summary: 'What this market pays out in: the exact token behind every claim it issues.',
  },
  {
    field: 'productRecordId',
    schema: PRODUCT_RECORD_SCHEMA_ID_V2,
    title: 'Product record',
    summary: 'The product this market grades its claims against.',
  },
  {
    field: 'resolutionPolicyId',
    schema: SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
    title: 'Source material',
    summary: 'How this market decides its outcome: the source material it settles on.',
  },
  {
    field: 'capabilityManifestId',
    schema: CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
    title: 'Capability manifest record',
    summary: 'The fixed list of what this market is allowed to do, set at founding.',
  },
  {
    field: 'selectedReleaseSetId',
    schema: EXECUTION_RELEASE_SET_SCHEMA_RELEASE_ID_V1,
    title: 'Execution release set',
    summary: 'The exact program builds every action on this market runs under.',
  },
]);

function fact(label: string, value: string): Readonly<{ label: string; value: string }> {
  return Object.freeze({ label, value });
}

function derivedRecordAddress(registryProgram: string | null, schema: Uint8Array, identityHex: string): string | null {
  if (registryProgram === null) return null;
  try {
    const digest = Uint8Array.from(identityHex.match(/../g) ?? [], (byte) => Number.parseInt(byte, 16));
    if (digest.length !== 32) return null;
    return deriveFinalizedRecordAddressesV1(registryProgram, schema, digest).record;
  } catch {
    return null;
  }
}

/** Build the navigable record graph for one Market. */
export function projectMarketLens(detail: MarketDetailV1): MarketLens {
  const nodes: LensNode[] = [];
  const edges: LensEdge[] = [];
  const gaps: string[] = [];
  const card = detail.card;

  if (card.status !== 'decoded') {
    nodes.push(
      Object.freeze({
        id: 'market',
        band: 'market',
        title: 'Market',
        summary: card.refusal,
        address: detail.address,
        contentId: null,
        provenance: Object.freeze({ kind: 'unavailable', reason: card.refusal }),
        facts: Object.freeze([]),
      }),
    );
    gaps.push('The Market itself did not decode.');
    return Object.freeze({
      address: detail.address,
      floorSlot: detail.floorSlot,
      coreProgramId: detail.coreProgramId,
      registryProgramId: detail.registryProgramId,
      claimsProgramId: detail.claimsProgramId,
      custodyProgramId: detail.custodyProgramId,
      nodes: Object.freeze(nodes),
      edges: Object.freeze(edges),
      bindings: Object.freeze([]),
      gaps: Object.freeze(gaps),
      detail,
    });
  }

  // ------------------------------------------------------------- the Market
  nodes.push(
    Object.freeze({
      id: 'market',
      band: 'market',
      title: 'Market · Core state',
      summary: detail.phaseMeaning ?? 'The market account itself.',
      address: card.address,
      contentId: null,
      provenance: Object.freeze({ kind: 'observed', slot: card.observedSlot }),
      facts: Object.freeze([
        fact('Phase', card.phase),
        fact('Opening readiness', card.readiness),
        fact('Generation', card.generation),
        fact('Outstanding capabilities', card.outstandingCapabilities),
        fact('Schema', `${card.identity.schemaMagic} v${card.identity.schemaVersion}`),
        fact('Account bytes', String(card.identity.accountBytes)),
        fact('Registry program', card.identity.registryProgram),
        fact('Rent beneficiary', card.identity.rentBeneficiary),
      ]),
    }),
  );

  // -------------------------------------------------- the six identity seeds
  const identityValues: Readonly<Record<string, string>> = {
    realmId: card.identity.realmId,
    productRecordId: card.identity.productRecordId,
    resolutionPolicyId: card.identity.resolutionPolicyId,
    capabilityManifestId: card.identity.capabilityManifestId,
    selectedReleaseSetId: card.identity.selectedReleaseSetId,
  };
  for (const entry of IDENTITY_SCHEMAS) {
    const identity = identityValues[entry.field];
    const address = derivedRecordAddress(detail.registryProgramId, entry.schema, identity);
    nodes.push(
      Object.freeze({
        id: entry.field,
        band: 'identity',
        title: entry.title,
        summary: entry.summary,
        address,
        contentId: identity,
        provenance:
          address === null
            ? Object.freeze({
                kind: 'stated',
                how: 'a content identity the Market carries; no Registry program is selected, so its record address is not derived',
              })
            : Object.freeze({
                kind: 'derived',
                how: 'raw-record PDA of this identity under the selected Registry program; not reacquired',
              }),
        facts: Object.freeze([fact('Content identity', identity)]),
      }),
    );
    edges.push(Object.freeze({ from: 'market', to: entry.field, label: 'Core seed' }));
  }

  nodes.push(
    Object.freeze({
      id: 'productInstance',
      band: 'identity',
      title: 'Product instance',
      summary: 'The product the payout is graded under. It lives inside the product record, not in an account of its own.',
      address: null,
      contentId: card.identity.productInstanceId,
      provenance: Object.freeze({
        kind: 'stated',
        how: 'read from the Market’s own seed; no record account holds it',
      }),
      facts: Object.freeze([fact('Content identity', card.identity.productInstanceId)]),
    }),
  );
  edges.push(Object.freeze({ from: 'productRecordId', to: 'productInstance', label: 'names' }));

  // ------------------------------------------------------------- collateral
  if (card.collateral.status === 'bound') {
    nodes.push(
      Object.freeze({
        id: 'realm',
        band: 'collateral',
        title: 'Realm, authenticated',
        summary: 'The record naming the token this market pays out in.',
        address: card.collateral.realmAddress,
        contentId: card.collateral.realmContentId,
        provenance: Object.freeze({ kind: 'observed', slot: card.collateral.observedSlot }),
        facts: Object.freeze([
          fact('Collateral mint', card.collateral.collateralMint),
          fact('Token program', card.collateral.tokenProgram),
          fact('Adapter release', card.collateral.adapterReleaseId),
          fact('Mint authority policy', card.collateral.mintAuthorityPolicy),
          fact('Freeze authority policy', card.collateral.freezeAuthorityPolicy),
        ]),
      }),
    );
    edges.push(Object.freeze({ from: 'realmId', to: 'realm', label: 'reacquired' }));
    nodes.push(
      Object.freeze({
        id: 'collateralMint',
        band: 'collateral',
        title: 'Collateral mint',
        summary: 'The token every claim in this market is backed by.',
        address: card.collateral.collateralMint,
        contentId: null,
        provenance: Object.freeze({ kind: 'observed', slot: card.collateral.observedSlot }),
        facts: Object.freeze([fact('Token program', card.collateral.tokenProgram)]),
      }),
    );
    edges.push(Object.freeze({ from: 'realm', to: 'collateralMint', label: 'binds' }));
  } else {
    gaps.push(`Realm: ${card.collateral.reason}`);
  }

  // -------------------------------------------------------------- liability
  if (card.liability.status === 'bound') {
    nodes.push(
      Object.freeze({
        id: 'aggregate',
        band: 'liability',
        title: 'Claims aggregate',
        summary: 'How many claims this market has issued in total, and where the collateral behind them sits.',
        address: card.liability.aggregateAddress,
        contentId: null,
        provenance: Object.freeze({ kind: 'observed', slot: card.liability.observedSlot }),
        facts: Object.freeze([
          fact('Claims program', card.liability.claimsProgramId),
          fact('Claim count', String(card.liability.claimCount)),
          fact('Revision', card.liability.revision),
          fact('Generation', card.liability.generation),
          fact('Liability basis', card.liability.liabilityBasisId),
          fact('Custody context', card.liability.custodyContext),
          fact('Required backing', `${card.liability.requiredBackingAtoms} atoms`),
          fact('Backing basis', card.liability.requiredBackingBasis),
        ]),
      }),
    );
    edges.push(Object.freeze({ from: 'market', to: 'aggregate', label: 'liability' }));
  } else {
    gaps.push(`Claims aggregate: ${card.liability.reason}`);
  }

  if (card.hoard.status === 'derived') {
    nodes.push(
      Object.freeze({
        id: 'hoard',
        band: 'liability',
        title: 'Hoard',
        summary: 'The vault holding the collateral behind every outstanding claim.',
        address: card.hoard.address,
        contentId: null,
        provenance: Object.freeze({ kind: 'observed', slot: card.hoard.observedSlot }),
        facts: Object.freeze([
          fact('Principal', `${card.hoard.principalAtoms} atoms`),
          fact('Custody program', card.hoard.custodyProgramId),
          fact('Custody authority', card.hoard.custodyAuthority),
          fact('Custody context', card.hoard.custodyContext),
          fact('Mint', card.hoard.collateralMint),
        ]),
      }),
    );
    edges.push(Object.freeze({ from: 'aggregate', to: 'hoard', label: 'backed by' }));
    nodes.push(
      Object.freeze({
        id: 'custodyAuthority',
        band: 'liability',
        title: 'Custody authority',
        summary: 'The account that owns the vault.',
        address: card.hoard.custodyAuthority,
        contentId: null,
        provenance: Object.freeze({
          kind: 'derived',
          how: 'dclutch:custody-authority:v1 over the Market and its release set, under the selected Custody program',
        }),
        facts: Object.freeze([]),
      }),
    );
    edges.push(Object.freeze({ from: 'hoard', to: 'custodyAuthority', label: 'owned by' }));
  } else {
    gaps.push(`Hoard: ${card.hoard.reason}`);
  }

  // ------------------------------------------------------------ capabilities
  if (card.capabilities.status === 'authenticated') {
    nodes.push(
      Object.freeze({
        id: 'manifest',
        band: 'capability',
        title: 'Capability manifest, authenticated',
        summary: `${card.capabilities.badges.length} things this market is allowed to do.`,
        address: card.capabilities.recordAddress,
        contentId: card.capabilities.manifestId,
        provenance: Object.freeze({ kind: 'observed', slot: card.capabilities.observedSlot }),
        facts: Object.freeze([fact('Entries', String(card.capabilities.badges.length))]),
      }),
    );
    edges.push(Object.freeze({ from: 'capabilityManifestId', to: 'manifest', label: 'reacquired' }));
    for (const badge of card.capabilities.badges) {
      const id = `capability-${badge.index}`;
      nodes.push(
        Object.freeze({
          id,
          band: 'capability',
          title: `Capability ${badge.index} · ${badge.label}`,
          summary: badge.recognized
            ? 'A named capability kind.'
            : 'An unnamed capability kind, identified by its fingerprint.',
          address: null,
          contentId: badge.kindId,
          provenance: Object.freeze({ kind: 'observed', slot: card.capabilities.observedSlot }),
          facts: Object.freeze([
            fact('Kind', badge.kindId),
            fact('Program set', badge.programSetId),
            fact('Config', badge.configId),
            fact('Activation', badge.activation),
            fact('Deadline', badge.deadline ?? 'immediate'),
            fact('Dependencies', badge.dependencies.length === 0 ? 'none' : badge.dependencies.join(', ')),
            fact('Native funding', `${badge.funding.nativeLamportsTotal.toString()} lamports`),
            fact('Realm funding', `${badge.funding.realmCollateralTotal.toString()} atoms`),
          ]),
        }),
      );
      edges.push(Object.freeze({ from: 'manifest', to: id, label: 'entry' }));
      for (const dependency of badge.dependencies) {
        edges.push(
          Object.freeze({ from: id, to: `capability-${dependency}`, label: 'depends on' }),
        );
      }
    }
  } else {
    gaps.push(`Capability manifest: ${card.capabilities.reason}`);
  }

  // -------------------------------------------------------------- settlement
  if (card.settlement.status === 'terminal') {
    nodes.push(
      Object.freeze({
        id: 'terminalReceipt',
        band: 'settlement',
        title: 'Terminal receipt',
        summary: `${card.settlement.label}. Winning claim index ${card.settlement.winner}.`,
        address: null,
        contentId: card.settlement.receiptId,
        provenance: Object.freeze({
          kind: 'stated',
          how: 'read from the Market’s own terminal receipt slot; no schema pairing is declared for it',
        }),
        facts: Object.freeze([
          fact('Winner', String(card.settlement.winner)),
          fact('Receipt identity', card.settlement.receiptId),
        ]),
      }),
    );
    edges.push(Object.freeze({ from: 'market', to: 'terminalReceipt', label: 'settled by' }));
  } else {
    nodes.push(
      Object.freeze({
        id: 'terminalReceipt',
        band: 'settlement',
        title: 'Terminal settlement',
        summary: card.settlement.label,
        address: null,
        contentId: null,
        provenance: Object.freeze({ kind: 'observed', slot: card.observedSlot }),
        facts: Object.freeze([]),
      }),
    );
    edges.push(Object.freeze({ from: 'market', to: 'terminalReceipt', label: 'settles into' }));
  }

  if (detail.registryProgramId === null) {
    gaps.push(
      'No Registry program is selected, so the five content identities the Market names could not be turned into record addresses. Select one to make them openable.',
    );
  }

  return Object.freeze({
    address: detail.address,
    floorSlot: detail.floorSlot,
    coreProgramId: detail.coreProgramId,
    registryProgramId: detail.registryProgramId,
    claimsProgramId: detail.claimsProgramId,
    custodyProgramId: detail.custodyProgramId,
    nodes: Object.freeze(nodes),
    edges: Object.freeze(edges),
    bindings: card.bindings,
    gaps: Object.freeze(gaps),
    detail,
  });
}

export type MarketLensRequest = Readonly<{
  coreProgramId: string;
  registryProgramId?: string | null;
  claimsProgramId?: string | null;
  custodyProgramId?: string | null;
  address: string;
}>;

/** Read one Market and project its record graph. */
export async function inspectMarketLens(
  client: Pick<SolanaRpcClient, 'finalizedSlot' | 'multipleAccounts'>,
  request: MarketLensRequest,
): Promise<MarketLens> {
  return projectMarketLens(await inspectMarketDetailV1(client, request));
}
