'use client';

import { TransactionMessage, VersionedTransaction, type AddressLookupTableAccount, type TransactionInstruction } from '@solana/web3.js';
import Nav from '@/components/Nav';
import { FormEvent, useEffect, useMemo, useState } from 'react';

import { PublicKey } from '@solana/web3.js';

import { prepareCoreFoundV2, type CoreFoundInputV2, type CoreFoundPlanV2 } from '@/lib/coreFound';
import {
  FOUNDING_LADDER_V1,
  summarizeFoundingLadderV1,
  type FoundingRungStatusV1,
} from '@/lib/founding/ladder';
import {
  BONDING_CURVE_FLOOR_DERIVATION_ID_V1,
  BONDING_CURVE_GRADUATION_FLOOR_LAMPORTS_V1,
  MANIPULATION_FLOOR_V1_BYTES,
  MANIPULATION_FLOOR_V1_MAGIC,
} from '@/lib/generated/principalCapacityV1';
import {
  DEFAULT_CHAIN_STATE_CAPACITY_V1,
  admitPrincipalCapacityV1,
  decodeManipulationFloorV1,
  formatCapacityV1,
  type PrincipalCapacityV1,
} from '@/lib/founding/principalCapacity';
import {
  encodeCapabilityManifestV1,
  nativeLamportsV1,
  summarizeManifestFundingV1,
  type CapabilityEntryInputV1,
  type FundingCompartmentNameV1,
} from '@/lib/founding/capabilityQuote';
import { FUNDING_COMPARTMENTS_V1 } from '@/lib/generated/capabilityManifestV1';
import {
  composeRangeProtectionV1,
  formatTicksV1,
  rangeProtectionBackingV1,
} from '@/lib/founding/rangeProtection';
import {
  PYTH_SOL_USD_MEASURED_P50_SECONDS_V1,
  TERMINAL_WINDOW_GUIDANCE_SECONDS_V1,
  TERMINAL_WINDOW_ROBUST_SECONDS_V1,
  WINDOW_CADENCE_TABLE_V1,
  assessWindowWidthV1,
  resolutionDeadlineV1,
} from '@/lib/founding/windowCadence';
import { hex, sha256 } from '@/lib/bytes';
import { planLookupTableV1, lookupTableAccountV1, type LookupTablePlanV1 } from '@/lib/founding/lookupTable';
import { SolanaRpcClient } from '@/lib/rpc';
import { requestWalletTransactionSignatureV1, submitSignedTransactionV1 } from '@/lib/walletHandoff';
import WalletDirectory, { useWalletDirectoryV1 } from '@/components/WalletDirectory';
import { useDeploymentFieldV1, useDeploymentV1 } from '@/lib/deploymentStore';

type StepId = 'product' | 'window' | 'funding' | 'review' | 'submit';

const STEPS: ReadonlyArray<Readonly<{ id: StepId; number: string; title: string; blurb: string }>> = Object.freeze([
  { id: 'product', number: '01', title: 'Product', blurb: 'Range protection on a Pyth source. The partition, the payoff, and the explicit failure outcome.' },
  { id: 'window', number: '02', title: 'Window width', blurb: '§12.3’s cadence table. The operator states a width; nothing here states one for them.' },
  { id: 'funding', number: '03', title: 'Principal & funding', blurb: 'Seven segregated compartments, and the κ capacity bound checked at the atom.' },
  { id: 'review', number: '04', title: 'Review the ladder', blurb: 'The exact transaction set, with every rung labelled by what actually builds it.' },
  { id: 'submit', number: '05', title: 'Sign & submit', blurb: 'The rungs a browser can drive, signed by a wallet against a finalized RPC.' },
]);

const STATUS_LABEL: Readonly<Record<FoundingRungStatusV1, string>> = Object.freeze({
  'browser-builder': 'browser builder',
  'browser-frame-borrowed-coordinates': 'browser frame · borrowed coordinates',
  'tooling-only': 'tooling only',
});

type AddressField = Exclude<keyof CoreFoundInputV2, 'generation' | 'lookupTable'>;
type AddressValues = Record<AddressField, string>;

const ADDRESS_FIELDS: ReadonlyArray<Readonly<{ field: AddressField; label: string }>> = Object.freeze([
  { field: 'payer', label: 'Payer (the connected wallet)' },
  { field: 'registryProgram', label: 'Registry program' },
  { field: 'activationCache', label: 'Release activation cache' },
  { field: 'refundWallet', label: 'Immutable rent-refund wallet' },
  { field: 'realmRecord', label: 'Realm raw record' },
  { field: 'productRecord', label: 'Product Runtime V2 raw' },
  { field: 'resultDomainRecord', label: 'Result domain raw' },
  { field: 'portfolioRecord', label: 'Portfolio raw' },
  { field: 'linkedBasisRecord', label: 'Linked basis raw' },
  { field: 'sourceMaterialRecord', label: 'SourceMaterialV3 raw' },
  { field: 'sourceSpecRecord', label: 'Source spec raw' },
  { field: 'capacityProfileRecord', label: 'Source capacity profile raw' },
  { field: 'manipulationFloorRecord', label: 'Manipulation floor raw' },
  { field: 'capabilityManifestRecord', label: 'Capability manifest raw' },
]);

type SubmitStageId = 'rent-credit' | 'routing-table' | 'found31';
type SubmitStage = Readonly<{ id: SubmitStageId; label: string; signature: string | null; status: 'pending' | 'signing' | 'submitted' | 'refused'; detail: string }>;

function reason(error: unknown): string {
  return error instanceof Error ? error.message : 'refused without a usable reason';
}

function bigintOrNull(value: string): bigint | null {
  if (!/^-?(0|[1-9][0-9]*)$/.test(value.trim())) return null;
  try { return BigInt(value.trim()); } catch { return null; }
}

function digitsOrNull(value: string): number | null {
  return /^(0|[1-9][0-9]*)$/.test(value.trim()) ? Number(value.trim()) : null;
}

/** Format raw atoms at a display precision, exactly, without floating point. */
function formatAtoms(atoms: bigint, decimals: number): string {
  const scale = 10n ** BigInt(decimals);
  const whole = atoms / scale;
  const fraction = (atoms % scale).toString().padStart(decimals, '0').replace(/0+$/, '');
  return decimals === 0 || fraction === '' ? whole.toString() : `${whole}.${fraction}`;
}

function id(byte: number): string {
  return byte.toString(16).padStart(2, '0').repeat(32);
}

export default function CreateMarketWizard() {
  const [step, setStep] = useState<StepId>('product');

  // 01 — Product
  const [coordinateLabel, setCoordinateLabel] = useState('SOL/USD');
  const [cutDenominator, setCutDenominator] = useState('100');
  const [lowerEdge, setLowerEdge] = useState('12000');
  const [upperEdge, setUpperEdge] = useState('18000');

  // 02 — Window
  const [windowWidth, setWindowWidth] = useState('1250');
  const [maxAge, setMaxAge] = useState('600');
  const [windowEnd, setWindowEnd] = useState('1790784000');

  // 03 — Principal and funding
  const [decimals, setDecimals] = useState('6');
  const [principal, setPrincipal] = useState('1000000000');
  const [kappaNumerator, setKappaNumerator] = useState(String(DEFAULT_CHAIN_STATE_CAPACITY_V1.kind === 'bounded' ? DEFAULT_CHAIN_STATE_CAPACITY_V1.numerator : 1n));
  const [kappaDenominator, setKappaDenominator] = useState(String(DEFAULT_CHAIN_STATE_CAPACITY_V1.kind === 'bounded' ? DEFAULT_CHAIN_STATE_CAPACITY_V1.denominator : 4n));
  const [venueFloor, setVenueFloor] = useState(BONDING_CURVE_GRADUATION_FLOOR_LAMPORTS_V1.toString());
  const [floorRecordHex, setFloorRecordHex] = useState('');
  const [compartments, setCompartments] = useState<Record<FundingCompartmentNameV1, string>>(() => ({
    Rent: '1', Creation: '1', Work: '0', Provider: '0', Bounty: '1', Liquidity: '0', Service: '0',
  }));

  // 04/05 — chain. The endpoint and the deployment-derivable coordinates
  // arrive filled from the active deployment; the author's edits override.
  const deployment = useDeploymentV1();
  const [endpoint, setEndpoint] = useDeploymentFieldV1((d) => d.endpoint);
  const [generation, setGeneration] = useState('2');
  const [addresses, setAddresses] = useState<AddressValues>(() => Object.fromEntries(ADDRESS_FIELDS.map(({ field }) => [field, ''])) as AddressValues);
  const effectiveAddresses: AddressValues = {
    ...addresses,
    registryProgram: addresses.registryProgram !== '' ? addresses.registryProgram : deployment.programs.registry,
    activationCache: addresses.activationCache !== '' ? addresses.activationCache : deployment.activationCache ?? '',
  };
  const [plan, setPlan] = useState<CoreFoundPlanV2 | null>(null);
  const [planStatus, setPlanStatus] = useState('No transaction has been constructed. The ladder below is the plan, not a promise.');
  const [stages, setStages] = useState<ReadonlyArray<SubmitStage>>([]);
  const [table, setTable] = useState<AddressLookupTableAccount | null>(null);
  const [walletStatus, setWalletStatus] = useState('No wallet connected.');
  const wallets = useWalletDirectoryV1();

  const product = useMemo(() => {
    const denominator = bigintOrNull(cutDenominator);
    const low = bigintOrNull(lowerEdge);
    const high = bigintOrNull(upperEdge);
    if (denominator === null || low === null || high === null) return { ok: false as const, message: 'Band edges and the denominator must be whole numbers.' };
    try {
      return { ok: true as const, value: composeRangeProtectionV1({ coordinateLabel, cutDenominator: denominator, lowerEdgeTicks: low, upperEdgeTicks: high }) };
    } catch (error) { return { ok: false as const, message: reason(error) }; }
  }, [coordinateLabel, cutDenominator, lowerEdge, upperEdge]);

  const window = useMemo(() => {
    const width = digitsOrNull(windowWidth);
    const age = digitsOrNull(maxAge);
    const end = digitsOrNull(windowEnd);
    if (width === null || age === null || end === null) return { ok: false as const, message: 'Width, max age and the window end must be whole seconds.' };
    try {
      return {
        ok: true as const,
        assessment: assessWindowWidthV1(width),
        deadline: resolutionDeadlineV1(end - width, end, age),
      };
    } catch (error) { return { ok: false as const, message: reason(error) }; }
  }, [windowWidth, maxAge, windowEnd]);

  const capacity = useMemo<PrincipalCapacityV1>(() => {
    const numerator = bigintOrNull(kappaNumerator);
    const denominator = bigintOrNull(kappaDenominator);
    if (numerator === null || denominator === null || numerator < 0n || denominator < 0n) return { kind: 'unstated' };
    return { kind: 'bounded', numerator, denominator };
  }, [kappaNumerator, kappaDenominator]);

  /**
   * The floor, from an authenticated record when one is pasted.
   *
   * A typed number is a claim about a venue; a `ManipulationFloorV1` record is
   * the venue's own derivation, and it names the Source, adapter configuration
   * and collateral unit it was derived for. The typed field remains, because a
   * wizard has to be usable before an operator has a record in hand -- but the
   * two are never silently interchangeable, and the copy says which is in play.
   */
  const floorRecord = useMemo(() => {
    const text = floorRecordHex.trim().replace(/^0x/, '');
    if (text === '') return { kind: 'stated' as const };
    if (!/^[0-9a-fA-F]*$/.test(text) || text.length % 2 !== 0) return { kind: 'refused' as const, message: 'A floor record is hexadecimal bytes.' };
    try {
      return { kind: 'decoded' as const, floor: decodeManipulationFloorV1(Uint8Array.from(text.toLowerCase().match(/../g) ?? [], (byte) => Number.parseInt(byte, 16))) };
    } catch (error) { return { kind: 'refused' as const, message: reason(error) }; }
  }, [floorRecordHex]);

  const kappa = useMemo(() => {
    const principalAtoms = bigintOrNull(principal);
    if (floorRecord.kind === 'refused') return { ok: false as const, message: floorRecord.message };
    const floor = floorRecord.kind === 'decoded' ? floorRecord.floor.floorAtoms : bigintOrNull(venueFloor);
    if (principalAtoms === null || floor === null || principalAtoms < 0n || floor < 0n) return { ok: false as const, message: 'Principal and the venue floor must be whole numbers of atoms.' };
    try { return { ok: true as const, verdict: admitPrincipalCapacityV1(capacity, floor, principalAtoms) }; }
    catch (error) { return { ok: false as const, message: reason(error) }; }
  }, [capacity, principal, venueFloor, floorRecord]);

  const backing = useMemo(() => {
    const principalAtoms = bigintOrNull(principal);
    if (!product.ok || principalAtoms === null) return null;
    try { return rangeProtectionBackingV1(product.value, principalAtoms); } catch { return null; }
  }, [product, principal]);

  const manifest = useMemo(() => {
    // Three capability entries, in the canonical kind order, quoted from the
    // compartment amounts the operator states. The identities here are wizard
    // placeholders and the copy says so: a real founding names released kinds.
    const quoteCompartments = Object.fromEntries(
      FUNDING_COMPARTMENTS_V1.flatMap((compartment) => {
        const amount = bigintOrNull(compartments[compartment.name]);
        return amount === null || amount <= 0n ? [] : [[compartment.name, nativeLamportsV1(amount)]];
      }),
    );
    const entries: CapabilityEntryInputV1[] = [1, 2, 3].map((index) => ({
      kindId: id(index),
      releaseId: id(0x20),
      configId: id(0x30 + index),
      capacityProfileId: id(0x40 + index),
      childSchemaId: id(0x50),
      childDerivationId: id(0x60),
      activation: 'RequiredAtFounding',
      activationDeadlineSlot: 0n,
      dependencies: [],
      quote: { compartments: quoteCompartments, realmCollateral: null },
    }));
    try {
      const bytes = encodeCapabilityManifestV1(entries);
      return { ok: true as const, bytes, totals: summarizeManifestFundingV1(entries), entries: entries.length };
    } catch (error) { return { ok: false as const, message: reason(error) }; }
  }, [compartments]);

  // The manifest's content identity is what the Market PDA is derived from, so
  // it is worth showing -- but SHA-256 in the browser is async, which makes it
  // the one derived value here that cannot be a `useMemo`. The cancel flag
  // keeps a slow digest of an old manifest from overwriting a newer one.
  const [manifestDigest, setManifestDigest] = useState<string | null>(null);
  useEffect(() => {
    let live = true;
    void (async () => {
      let next: string | null = null;
      try { if (manifest.ok) next = hex(await sha256(manifest.bytes)); } catch { next = null; }
      if (live) setManifestDigest(next);
    })();
    return () => { live = false; };
  }, [manifest]);

  const ladder = summarizeFoundingLadderV1();
  const displayDecimals = digitsOrNull(decimals) ?? 0;

  async function buildChainPlan(event: FormEvent<HTMLFormElement>): Promise<void> {
    event.preventDefault();
    setPlan(null);
    setPlanStatus('Reacquiring immutable releases, Product semantics, and all 37 accounts at finalized commitment…');
    try {
      const generationValue = bigintOrNull(generation);
      if (generationValue === null || generationValue <= 0n) throw new Error('generation must be a positive integer');
      const next = await prepareCoreFoundV2(new SolanaRpcClient(endpoint), { ...effectiveAddresses, generation: generationValue });
      setPlan(next);
      setTable(null);
      setStages([
        { id: 'rent-credit', label: 'Lifecycle RentCredit Create', signature: null, status: next.rentCreditState === 'created' ? 'submitted' : 'pending', detail: next.rentCreditState === 'created' ? 'Already created on this chain for this Market and generation.' : `${next.rentCreateWireBytes?.length ?? 0} bytes · unsigned` },
        { id: 'routing-table', label: `Routing table · ${next.routableAddresses.length} addresses`, signature: null, status: 'pending', detail: 'Not created. Found37 does not fit a packet without it.' },
        { id: 'found31', label: 'Found37 — Market at Founding', signature: null, status: 'pending', detail: next.foundRefusal ?? `${next.wireBytes?.length ?? 0} bytes · unsigned` },
      ]);
      setPlanStatus(`Accepted at finalized slot ${next.observedSlot}. Nothing is signed or submitted.`);
    } catch (error) {
      setPlanStatus(`Refused: ${reason(error)}`);
    }
  }

  function updateStage(target: SubmitStage['id'], patch: Partial<SubmitStage>): void {
    setStages((current) => current.map((stage) => (stage.id === target ? { ...stage, ...patch } : stage)));
  }

  /** Sign one transaction with the connected wallet and submit it, once. */
  async function signAndSubmitOne(transaction: VersionedTransaction, client: SolanaRpcClient): Promise<string> {
    const signed = await requestWalletTransactionSignatureV1(client, wallets.handoff(endpoint), transaction, addresses.payer);
    if (!signed.complete) throw new Error('the wallet did not complete the signature set');
    return submitSignedTransactionV1(client, signed.transaction);
  }

  /**
   * Wait for one submitted signature to finalize, or say why it did not.
   *
   * Submission is not landing, and every stage here is a precondition of the
   * next: the lifecycle credit must exist before Found37 names it, and a lookup
   * table is usable only strictly after the slot that last extended it.
   */
  async function awaitFinalized(client: SolanaRpcClient, signature: string): Promise<string> {
    for (let attempt = 0; attempt < 90; attempt += 1) {
      const status = await client.signatureStatus(signature);
      if (status?.err) throw new Error(`${signature} failed on chain: ${JSON.stringify(status.err)}`);
      if (status?.confirmation === 'finalized') return status.slot;
      await new Promise((resolve) => setTimeout(resolve, 1_000));
    }
    throw new Error(`${signature} did not finalize within ninety seconds`);
  }

  function compile(instructions: ReadonlyArray<TransactionInstruction>, blockhash: string): VersionedTransaction {
    return new VersionedTransaction(new TransactionMessage({
      payerKey: new PublicKey(addresses.payer),
      recentBlockhash: blockhash,
      instructions: [...instructions],
    }).compileToV0Message());
  }

  function updateStage(target: SubmitStageId, patch: Partial<SubmitStage>): void {
    setStages((current) => current.map((stage) => (stage.id === target ? { ...stage, ...patch } : stage)));
  }

  /**
   * Build the routing table Found37 rides, and read it back off the chain.
   *
   * The create and every extend page is its own transaction and its own wallet
   * signature; nothing is batched behind one click that the operator did not
   * see. The table's contents are then re-read at finalized commitment and
   * compared against the plan, because a client that compiles indexes against
   * the list it built the plan FROM rather than against the table itself hands
   * the program a permuted account frame -- which is a refusal three layers
   * away from its cause.
   */
  async function buildRoutingTable(client: SolanaRpcClient, current: CoreFoundPlanV2): Promise<AddressLookupTableAccount> {
    const recentSlot = await client.finalizedSlot();
    const routing: LookupTablePlanV1 = planLookupTableV1({
      authority: addresses.payer,
      payer: addresses.payer,
      recentSlot: BigInt(recentSlot),
      addresses: current.routableAddresses,
    });
    let lastSlot = recentSlot;
    const pages = [routing.create, ...routing.extensions];
    for (let index = 0; index < pages.length; index += 1) {
      updateStage('routing-table', { detail: `Signing ${index === 0 ? 'create' : `extend page ${index}`} of ${pages.length}…` });
      const blockhash = await client.latestMutationBlockhash();
      const signature = await signAndSubmitOne(compile([pages[index]], blockhash.blockhash), client);
      lastSlot = await awaitFinalized(client, signature);
      updateStage('routing-table', { signature, detail: `${index + 1} of ${pages.length} finalized at slot ${lastSlot}` });
    }
    // Strictly after the slot that last extended it.
    for (let attempt = 0; attempt < 60; attempt += 1) {
      if (BigInt(await client.finalizedSlot()) > BigInt(lastSlot)) break;
      await new Promise((resolve) => setTimeout(resolve, 1_000));
    }
    const account = await lookupTableAccountV1(client, routing.lookupTable, routing.addresses);
    updateStage('routing-table', { status: 'submitted', detail: `${routing.lookupTable} · ${routing.addresses.length} addresses, contents verified against the plan` });
    return account;
  }

  /**
   * Run one rung, then stop.
   *
   * Deliberately one rung per click. Chaining them would be deciding on the
   * operator's behalf that the previous one landed.
   */
  async function runStage(target: SubmitStageId): Promise<void> {
    if (plan === null) return;
    const client = new SolanaRpcClient(endpoint);
    updateStage(target, { status: 'signing', detail: 'Awaiting the wallet’s signature…' });
    try {
      if (target === 'rent-credit') {
        if (plan.rentCreateTransaction === null) throw new Error('the lifecycle RentCredit already exists for this Market and generation');
        const signature = await signAndSubmitOne(plan.rentCreateTransaction, client);
        updateStage(target, { detail: 'Submitted. Waiting for finality…', signature });
        const slot = await awaitFinalized(client, signature);
        updateStage(target, { status: 'submitted', detail: `Finalized at slot ${slot}` });
        setWalletStatus(`Lifecycle RentCredit finalized as ${signature}.`);
        return;
      }
      if (target === 'routing-table') {
        setTable(await buildRoutingTable(client, plan));
        setWalletStatus('Routing table finalized and its contents verified.');
        return;
      }
      if (table === null) throw new Error('Found37 does not fit a packet without its routing table; build that first');
      // Recompiled against the finalized table, and reauthenticated from
      // scratch: the chain has moved since the plan was built, and a stale
      // blockhash or a changed record is a refusal rather than a surprise.
      const generationValue = BigInt(generation);
      const routed = await prepareCoreFoundV2(client, { ...effectiveAddresses, generation: generationValue, lookupTable: table });
      if (routed.transaction === null) throw new Error(routed.foundRefusal ?? 'Found37 could not be compiled');
      if (routed.market !== plan.market) throw new Error('the Market address changed between planning and submission');
      const signature = await signAndSubmitOne(routed.transaction, client);
      updateStage(target, { detail: `Submitted · ${routed.wireBytes?.length ?? 0} bytes. Waiting for finality…`, signature });
      const slot = await awaitFinalized(client, signature);
      updateStage(target, { status: 'submitted', detail: `Finalized at slot ${slot} · Market ${routed.market} is at phase Founding`, signature });
      setWalletStatus(`Found37 finalized as ${signature}.`);
    } catch (error) {
      updateStage(target, { status: 'refused', detail: `Refused: ${reason(error)}` });
      setWalletStatus(`Refused: ${reason(error)}`);
    }
  }

  return <main className="product-shell wizard-shell">
    <Nav current="/create" />

    <section className="market-heading">
      <div>
        <div className="market-kicker"><span>Create · range protection</span><span>DCLTGMF2 atomic route</span></div>
        <h1>State a band.<br />Price the window.<br />See the whole ladder.</h1>
      </div>
      <p>
        Founding is not one transaction. This wizard composes the Product, the terminal window and the funding from the
        same arithmetic the chain applies, then shows the exact transaction set — including the rungs a browser cannot
        build yet, named with the reason. Nothing here fabricates a price, a market, or a release.
      </p>
    </section>

    <nav className="wizard-steps" aria-label="Founding steps">
      {STEPS.map((candidate) => <button
        type="button"
        key={candidate.id}
        className={candidate.id === step ? 'active' : ''}
        aria-current={candidate.id === step ? 'step' : undefined}
        onClick={() => setStep(candidate.id)}
      ><span>{candidate.number}</span><strong>{candidate.title}</strong><small>{candidate.blurb}</small></button>)}
    </nav>

    {step === 'product' && <section className="direct-card">
      <header className="direct-card-heading"><span>01</span><div>
        <h2>Range protection on a Pyth source</h2>
        <p>A categorical partition of one coordinate domain by two cuts, plus an explicit failure outcome. The payoff is one unit of the liability basis in either tail and nothing inside the band — the shape a holder buys as protection against the price leaving a range they can live with.</p>
      </div></header>
      <div className="direct-form-grid">
        <label><span>Coordinate label (display only)</span><input value={coordinateLabel} onChange={(event) => setCoordinateLabel(event.target.value)} /></label>
        <label><span>Ticks per whole unit · cut denominator</span><input inputMode="numeric" value={cutDenominator} onChange={(event) => setCutDenominator(event.target.value)} /></label>
        <label><span>Display precision · Mint decimals</span><input inputMode="numeric" value={decimals} onChange={(event) => setDecimals(event.target.value)} /></label>
        <label><span>Lower band edge · ticks</span><input inputMode="numeric" value={lowerEdge} onChange={(event) => setLowerEdge(event.target.value)} /></label>
        <label><span>Upper band edge · ticks</span><input inputMode="numeric" value={upperEdge} onChange={(event) => setUpperEdge(event.target.value)} /></label>
      </div>
      {product.ok ? <>
        <p className="direct-status">
          Band {formatTicksV1(product.value.cuts[0], product.value.cutDenominator)} to {formatTicksV1(product.value.cuts[1], product.value.cutDenominator)} ·
          {' '}{product.value.regions} ordinary regions + 1 explicit failure = {product.value.outcomeCount} outcomes ·
          {' '}portfolio denominator {product.value.portfolioDenominator.toString()}, gcd-normalized
        </p>
        <table className="wizard-table">
          <thead><tr><th>Outcome</th><th>Label</th><th>Coefficient</th><th>Pays</th></tr></thead>
          <tbody>{product.value.outcomes.map((outcome) => <tr key={outcome.index} className={outcome.coefficient > 0n ? 'wizard-paying' : ''}>
            <td>{outcome.index}</td><td>{outcome.label}</td><td>{outcome.coefficient.toString()}</td>
            <td>{outcome.coefficient > 0n ? 'one collateral atom per claim atom' : 'exactly zero'}</td>
          </tr>)}</tbody>
        </table>
        <p className="direct-refusal">
          The labels above are display metadata and are never decoded from a Market account. What the chain re-derives at
          Found time is the partition: cuts strictly increasing, regions exactly cuts + 1, outcomes exactly regions + 1,
          and a portfolio that is neither empty nor un-normalized.
        </p>
      </> : <p className="direct-refusal">Refused: {product.message}</p>}
    </section>}

    {step === 'window' && <section className="direct-card">
      <header className="direct-card-heading"><span>02</span><div>
        <h2>How wide the terminal window has to be</h2>
        <p>A terminal window used to be one second, and on a real cluster every terminal market walked to its failure outcome instead of resolving. Width is now the operator’s to state. There is no default, and this wizard does not invent one — it prices whatever you choose.</p>
      </div></header>
      <div className="direct-form-grid">
        <label><span>Window width · seconds</span><input inputMode="numeric" value={windowWidth} onChange={(event) => setWindowWidth(event.target.value)} /></label>
        <label><span>Window end · Unix seconds</span><input inputMode="numeric" value={windowEnd} onChange={(event) => setWindowEnd(event.target.value)} /></label>
        <label><span>Max age · seconds (a separate budget)</span><input inputMode="numeric" value={maxAge} onChange={(event) => setMaxAge(event.target.value)} /></label>
      </div>
      <table className="wizard-table">
        <thead><tr><th>W</th><th>Shape</th><th>P(at least one publication)</th><th /></tr></thead>
        <tbody>{WINDOW_CADENCE_TABLE_V1.map((row) => <tr key={row.seconds} className={digitsOrNull(windowWidth) === row.seconds ? 'wizard-paying' : ''}>
          <td>{row.seconds.toLocaleString()} s</td><td>{row.shape}</td><td>{row.publishedProbability}</td>
          <td><button type="button" className="wizard-inline" onClick={() => setWindowWidth(String(row.seconds))}>use</button></td>
        </tr>)}</tbody>
      </table>
      {window.ok ? <>
        <dl className="found-facts">
          <div><dt>Your width</dt><dd>{window.assessment.seconds.toLocaleString()} s · {window.assessment.cadences.toFixed(2)} measured cadences</dd></div>
          <div><dt>Publication probability</dt><dd>{window.assessment.headline}</dd></div>
          <div><dt>Confidence</dt><dd className={`wizard-verdict ${window.assessment.confidence}`}>{window.assessment.confidence.replaceAll('-', ' ')}</dd></div>
          <div><dt>Inclusive window</dt><dd>[{window.deadline.windowStart}, {window.deadline.windowEnd}]</dd></div>
          <div><dt>Primary deadline</dt><dd>{window.deadline.primaryDeadline} = end + max age</dd></div>
          <div><dt>Failure walk opens</dt><dd>{window.deadline.failureWalkOpensAt} · adjacent, with no gap where neither route can act</dd></div>
        </dl>
        <p className="direct-status">{window.assessment.detail}</p>
        <p className="direct-refusal">
          The probability is <em>provisional</em>: publications are modelled as a Poisson process at the measured devnet
          SOL/USD p50 of {PYTH_SOL_USD_MEASURED_P50_SECONDS_V1} s, while Pyth actually publishes on price movement and
          confidence thresholds. The operative guidance is at least {TERMINAL_WINDOW_GUIDANCE_SECONDS_V1.toLocaleString()} s
          (four cadences), and {TERMINAL_WINDOW_ROBUST_SECONDS_V1.toLocaleString()} s for a market that should not fail for
          provider reasons. Max age is a <em>different</em> budget: it covers submission latency, not publication cadence,
          and widening the window does nothing for it.
        </p>
      </> : <p className="direct-refusal">Refused: {window.message}</p>}
    </section>}

    {step === 'funding' && <section className="direct-card">
      <header className="direct-card-heading"><span>03</span><div>
        <h2>Principal, the capacity bound, and seven segregated compartments</h2>
        <p>Founding mints one complete set per collateral atom, so the founder holds every outcome. Capability funding is quoted per compartment and carries two independent totals; nothing anywhere adds a lamport to a collateral atom.</p>
      </div></header>
      <div className="direct-form-grid">
        <label><span>Founding principal · raw collateral atoms</span><input inputMode="numeric" value={principal} onChange={(event) => setPrincipal(event.target.value)} /></label>
        <label><span>Venue manipulation floor · lamports{floorRecord.kind === 'decoded' ? ' · from the record below' : ' · stated'}</span><input inputMode="numeric" disabled={floorRecord.kind === 'decoded'} value={floorRecord.kind === 'decoded' ? floorRecord.floor.floorAtoms.toString() : venueFloor} onChange={(event) => setVenueFloor(event.target.value)} /></label>
        <label><span>κ numerator / denominator</span><span className="wizard-pair">
          <input inputMode="numeric" value={kappaNumerator} onChange={(event) => setKappaNumerator(event.target.value)} />
          <input inputMode="numeric" value={kappaDenominator} onChange={(event) => setKappaDenominator(event.target.value)} />
        </span></label>
      </div>

      <label><span>ManipulationFloorV1 record · {MANIPULATION_FLOOR_V1_BYTES} bytes of hexadecimal, optional — the venue&apos;s own floor derivation, from the operator&apos;s source tooling; without it the typed floor above is what counts</span>
        <textarea spellCheck={false} value={floorRecordHex} onChange={(event) => setFloorRecordHex(event.target.value)} />
      </label>
      {floorRecord.kind === 'decoded'
        ? <dl className="found-facts">
            <div><dt>Recognized</dt><dd><code>{MANIPULATION_FLOOR_V1_MAGIC}</code> · {floorRecord.floor.basis.replace('-', ' ')} derivation</dd></div>
            <div><dt>Floor</dt><dd>{floorRecord.floor.floorAtoms.toLocaleString()} atoms of the collateral unit below</dd></div>
            <div><dt>Derived for Source</dt><dd>{floorRecord.floor.sourceSpecId}</dd></div>
            <div><dt>Venue configuration</dt><dd>{floorRecord.floor.adapterConfigId}</dd></div>
            <div><dt>Collateral unit</dt><dd>{floorRecord.floor.collateralUnitId}</dd></div>
            <div><dt>Derivation release</dt><dd>{floorRecord.floor.derivationReleaseId}{floorRecord.floor.derivationReleaseId === BONDING_CURVE_FLOOR_DERIVATION_ID_V1 ? ' · bonding-curve buyout/exit' : ''}</dd></div>
          </dl>
        : <p className="direct-refusal">
            {floorRecord.kind === 'refused'
              ? `Refused: ${floorRecord.message}`
              : 'No floor record supplied, so the number above is a stated claim about a venue rather than that venue’s own derivation. A real founding binds the floor to the Source, adapter configuration and collateral unit it was derived for; a floor derived for something else is not a weaker bound, it is an answer to a different question.'}
          </p>}

      {kappa.ok ? <div className={`wizard-kappa ${kappa.verdict.admitted ? 'admitted' : 'refused'}`}>
        <strong>{kappa.verdict.admitted ? 'Under the capacity bound' : `Over the capacity bound · ${kappa.verdict.refusal}`}</strong>
        <p>
          The predicate is <code>principal · denominator ≤ numerator · floor</code>, cross-multiplied so there is no
          division and no rounding. At κ = {formatCapacityV1(capacity)} against a floor of {BigInt(venueFloor || '0').toLocaleString()} lamports,
          {' '}the largest admitted principal is {kappa.verdict.largestAdmittedPrincipal === null ? 'none' : kappa.verdict.largestAdmittedPrincipal.toLocaleString()} atoms.
          {kappa.verdict.scaled !== null && kappa.verdict.bound !== null && <> This founding states {kappa.verdict.scaled.toLocaleString()} against a bound of {kappa.verdict.bound.toLocaleString()}.</>}
        </p>
        <p className="wizard-enforcement">
          Enforcement: <strong>{kappa.verdict.enforcement}</strong>. No on-chain route applies this predicate today — it is
          proven in Lean and implemented in <code>dclutch-source-contract</code>, and its only non-test caller is the
          off-chain gauntlet driver. Found sees the Source and not the principal; Claims FoundingV5 sees the reverse. So
          this verdict tells you what the protocol <em>intends</em>, not what a validator will refuse. And even once wired,
          a founding-only check is not a cap: principal grows on every complete-set split.
        </p>
      </div> : <p className="direct-refusal">Refused: {kappa.message}</p>}

      {backing && product.ok && <dl className="found-facts">
        <div><dt>Complete sets minted</dt><dd>{backing.completeSets.toLocaleString()} ({formatAtoms(backing.completeSets, displayDecimals)} at {displayDecimals} decimals)</dd></div>
        <div><dt>Per-outcome supply</dt><dd>{backing.perOutcomeSupplyAtoms.map((entry) => entry.toString()).join(' · ')} atoms</dd></div>
        <div><dt>Required backing while unresolved</dt><dd>{backing.requiredBackingWhileUnresolvedAtoms.toLocaleString()} atoms · max(supply)</dd></div>
        <div><dt>Paying outcomes</dt><dd>{backing.payingOutcomes.map((index) => product.value.outcomes[index].label).join(' · ')}</dd></div>
      </dl>}

      <h3 className="wizard-subhead">Capability funding · one quote per entry, {manifest.ok ? manifest.entries : 0} entries</h3>
      <table className="wizard-table">
        <thead><tr><th>Compartment</th><th>Asset policy</th><th>Per entry · lamports</th><th>Manifest total</th></tr></thead>
        <tbody>{FUNDING_COMPARTMENTS_V1.map((compartment) => {
          const total = manifest.ok ? manifest.totals.perCompartment.find((entry) => entry.name === compartment.name) : undefined;
          return <tr key={compartment.name} className={total && total.amount > 0n ? 'wizard-paying' : ''}>
            <td>{compartment.name}</td>
            <td>{compartment.assetPolicy}</td>
            <td><input inputMode="numeric" className="wizard-inline-input" value={compartments[compartment.name]} onChange={(event) => setCompartments((current) => ({ ...current, [compartment.name]: event.target.value }))} /></td>
            <td>{total ? `${total.amount.toString()} · ${total.assetClass}` : '—'}</td>
          </tr>;
        })}</tbody>
        {manifest.ok && <tfoot>
          <tr><td colSpan={3}>Native lamport total</td><td>{manifest.totals.nativeLamports.toString()}</td></tr>
          <tr><td colSpan={3}>Realm collateral total</td><td>{manifest.totals.realmCollateral.toString()}</td></tr>
        </tfoot>}
      </table>
      {manifest.ok
        ? <p className="direct-status">
            Manifest encodes to {manifest.bytes.length} bytes and passes the Found path’s own decoder.
            {manifestDigest && <> Its content identity is <code>{manifestDigest}</code> — the digest that goes into the Market PDA.</>}
            {' '}Capability identities here are wizard placeholders; a real founding names released kinds.
          </p>
        : <p className="direct-refusal">Refused: {manifest.message}</p>}
      <p className="direct-refusal">
        <code>Rent</code> and <code>Creation</code> pay for account existence and admit native lamports only. The other five
        are capability-selected. Both totals are recomputed from the compartments and never taken from a caller, and the
        Realm collateral binding is present exactly when the Realm total is nonzero.
      </p>
    </section>}

    {step === 'review' && <section className="direct-card">
      <header className="direct-card-heading"><span>04</span><div>
        <h2>The exact transaction set</h2>
        <p>{ladder.rungs} rungs. {ladder.browserBuilders} have a browser builder, {ladder.browserFrames} assemble their frame in the browser from coordinates Rust supplies, and {ladder.toolingOnly} are tooling-only. Three ride an address lookup table.</p>
      </div></header>
      <ol className="wizard-ladder">
        {FOUNDING_LADDER_V1.map((rung, index) => <li key={rung.id} className={rung.status}>
          <div className="wizard-rung-head">
            <span className="wizard-rung-index">{(index + 1).toString().padStart(2, '0')}</span>
            <strong>{rung.title}</strong>
            <span className={`wizard-badge ${rung.status}`}>{STATUS_LABEL[rung.status]}</span>
            {rung.lookupTable && <span className="wizard-badge alt">ALT</span>}
          </div>
          <p className="wizard-rung-effect">{rung.effect}</p>
          <dl>
            <div><dt>Transactions</dt><dd>{rung.transactions}</dd></div>
            <div><dt>Builder</dt><dd><code>{rung.builder}</code></dd></div>
          </dl>
          <p className="wizard-rung-reason">{rung.reason}</p>
        </li>)}
      </ol>
      <p className="direct-refusal">
        A “Create market” button that submitted one transaction would be a lie about five sixths of this. The rungs marked
        tooling-only are not missing features to be filled in by hand — each names a first-party Rust encoder or kernel
        transition that is the authority for what its bytes mean, and a browser re-implementation would be a second
        authority rather than a client.
      </p>
    </section>}

    {step === 'submit' && <section className="direct-card">
      <header className="direct-card-heading"><span>05</span><div>
        <h2>Sign and submit the rungs a browser can drive</h2>
        <p>Against a validator whose record graph and collateral Mint already exist, the two Core rungs are reachable from a wallet at a generation nothing has used. Each is signed and submitted separately: the lifecycle credit must confirm before Found37 is submitted, and chaining them would be deciding on your behalf that the first one landed.</p>
      </div></header>

      <WalletDirectory
        directory={wallets}
        purpose="founding payer and rent-refund wallet"
        onConnected={(address) => {
          // The connected wallet is the payer and, by default, the immutable
          // rent-refund beneficiary. Both remain editable: a founding that
          // refunds to a different wallet is legitimate and common.
          setAddresses((current) => ({ ...current, payer: address, refundWallet: current.refundWallet || address }));
          setWalletStatus(`${address} connected. No signature has been requested.`);
        }}
      />

      <form className="wizard-chain-form" onSubmit={buildChainPlan}>
        <div className="direct-form-grid">
          <label><span>Finalized RPC endpoint</span><input required value={endpoint} onChange={(event) => setEndpoint(event.target.value.trim())} /></label>
          <label><span>Market generation</span><input required inputMode="numeric" value={generation} onChange={(event) => setGeneration(event.target.value.trim())} /></label>
        </div>
        <div className="direct-form-grid wizard-address-grid">
          {ADDRESS_FIELDS.map(({ field, label }) => <label key={field}>
            <span>{label}</span>
            <input required spellCheck={false} value={effectiveAddresses[field]} onChange={(event) => setAddresses((current) => ({ ...current, [field]: event.target.value.trim() }))} />
          </label>)}
        </div>
        <button type="submit">Construct the unsigned lifecycle + Found37 pair</button>
        <p className="direct-status" aria-live="polite">{planStatus}</p>
      </form>

      {plan && <>
        <dl className="found-facts">
          <div><dt>Derived Market</dt><dd>{plan.market}</dd></div>
          <div><dt>Lifecycle RentCredit</dt><dd>{plan.rentCredit}</dd></div>
          <div><dt>Outcome width</dt><dd>{plan.outcomeCount.toLocaleString()}</dd></div>
          <div><dt>Product identity</dt><dd>{plan.productId}</dd></div>
          <div><dt>Rent debit</dt><dd>{plan.rentCreditRentDebit} credit + {plan.marketRentTopUp} Market lamports</dd></div>
          <div><dt>Blockhash validity</dt><dd>through block height {plan.lastValidBlockHeight}</dd></div>
        </dl>

        <div className="signing-grid">
          <article>
            <span>Wallet identity</span>
            <strong>{wallets.address ?? 'not connected'}</strong>
            <p>{walletStatus}</p>
          </article>
          <article>
            <span>Submission boundary</span>
            <strong>{stages.filter((stage) => stage.status === 'submitted').length} / {stages.length} submitted</strong>
            <p>Nothing is submitted without a separate click. Preflight is never skipped.</p>
          </article>
        </div>

        <ol className="wizard-stages">
          {stages.map((stage) => <li key={stage.id} className={stage.status}>
            <div><strong>{stage.label}</strong><span className={`wizard-badge ${stage.status}`}>{stage.status}</span></div>
            <p>{stage.detail}</p>
            {stage.signature && <code>{stage.signature}</code>}
            <button
              type="button"
              disabled={stage.status === 'signing' || stage.status === 'submitted' || wallets.address === null}
              onClick={() => void runStage(stage.id)}
            >Sign &amp; submit this transaction</button>
          </li>)}
        </ol>
        <p className="direct-refusal">
          Submitting Found37 leaves a Market in phase <em>Founding</em>: identity exists, obligations and readiness are
          still being assembled, and no liabilities or trading are admitted. Reaching <em>Open</em> is the DCLTGMF2 rung,
          which needs the projected-Custody prestate the review step marks tooling-only.
        </p>
      </>}
    </section>}
  </main>;
}
