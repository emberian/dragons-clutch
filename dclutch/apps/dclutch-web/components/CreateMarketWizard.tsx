'use client';

import PageShell from '@/components/PageShell';
import Nav from '@/components/Nav';
import { FormEvent, useEffect, useMemo, useState } from 'react';

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
  loadPartitionQualityWasmV1,
  requireInterestingPartitionV1,
  type PartitionQualityReportV1,
  type PartitionQualityWasmV1,
} from '@/lib/founding/partitionQualityV1';
import {
  PYTH_SOL_USD_MEASURED_P50_SECONDS_V1,
  TERMINAL_WINDOW_GUIDANCE_SECONDS_V1,
  TERMINAL_WINDOW_ROBUST_SECONDS_V1,
  WINDOW_CADENCE_TABLE_V1,
  assessWindowWidthV1,
  resolutionDeadlineV1,
} from '@/lib/founding/windowCadence';
import { inspectMarketDetailV1 } from '@/lib/marketDetail';
import { readSponsoredPriceV1 } from '@/lib/sourceProviderV1';
import { inspectMarketQuestionV1 } from '@/lib/marketQuestion';
import { PUBLIC_DEVNET_CUT_V1 } from '@/lib/publicCutStaging';
import { SolanaRpcClient } from '@/lib/rpc';
import OpenerFirstCrankTerms from '@/components/OpenerFirstCrankTerms';
import { useDeploymentFieldV1, useDeploymentV1 } from '@/lib/deploymentStore';

type StepId = 'product' | 'window' | 'funding' | 'review' | 'submit';

const STEPS: ReadonlyArray<Readonly<{ id: StepId; number: string; title: string; blurb: string }>> = Object.freeze([
  { id: 'product', number: '01', title: 'Design the payout', blurb: 'Range protection on a Pyth source: what pays, what does not, and what happens if the source is silent.' },
  { id: 'window', number: '02', title: 'Choose the window', blurb: 'You state how long the source may answer; this page does not choose for you.' },
  { id: 'funding', number: '03', title: 'Backing & funding', blurb: 'Seven named funding purposes and a capacity ratio checked in exact base units.' },
  { id: 'review', number: '04', title: 'Review the plan', blurb: 'Every opening step, with a plain statement of what can build it today.' },
  { id: 'submit', number: '05', title: 'Inspect the chain', blurb: 'Recheck the partial browser preview without signing or spending.' },
]);

const STATUS_LABEL: Readonly<Record<FoundingRungStatusV1, string>> = Object.freeze({
  'browser-builder': 'browser preview',
  'browser-frame-borrowed-coordinates': 'uses operator coordinates',
  'tooling-only': 'operator tooling',
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

  /*
   * 01 -- Product. THE BAND SHIPS EMPTY AND IS THEN READ, not typed.
   *
   * These four fields carried `12000 / 18000` around an observation of
   * `15000`: a $150 SOL, the runbook's numbers from three months earlier. SOL
   * was near $100 on 2026-09-02, so a wizard opened at its defaults asked
   * whether the price finishes between $120 and $180 -- a question whose
   * answer is already known, which is exactly the defect the founding band and
   * `centred_cuts_v1` exist to refuse. Four devnet markets were founded
   * unfillable before anyone noticed a stale constant, and the belief fields
   * beside it were tuned so the gate ADMITTED the stale default.
   *
   * They now start blank and are filled from the market this deployment last
   * opened, read from its own result-domain record. Blank on a failed read, so
   * a wizard that could not reach the chain asks for a band instead of
   * suggesting one; and the provenance line beside them says what was read,
   * when, and which of the four is not a price.
   */
  const [coordinateLabel, setCoordinateLabel] = useState('SOL/USD');
  const [cutDenominator, setCutDenominator] = useState('');
  const [lowerEdge, setLowerEdge] = useState('');
  const [upperEdge, setUpperEdge] = useState('');
  // The coordinate at founding, in the band's own ticks. Not a display value:
  // it is the one input that decides whether this partition is a question.
  const [foundingObservation, setFoundingObservation] = useState('');
  /*
   * THE OBSERVATION BECOMES A PRICE.
   *
   * The band's centre was a typed number, then (as of the derived prefill) the
   * midpoint of the last opened market's band -- a shape to replace, and said
   * to be one. Neither is a price. The Source family already owns a
   * `PriceUpdateV2` decoder and already compiles to WASM for this browser, so
   * the wizard reads the feed its market will resolve against through that
   * boundary rather than growing a second Pyth reader in TypeScript: the
   * number it centres on is the one the founding will grade against, and it
   * arrives with the publish time and posted slot that say how old it is.
   */
  const [priceUpdateAddress, setPriceUpdateAddress] = useState('');
  const [priceReceiverProgram, setPriceReceiverProgram] = useState('');
  const [priceReading, setPriceReading] = useState('No feed has been read. The band below is a shape, not a price.');
  const [bandProvenance, setBandProvenance] = useState(() => PUBLIC_DEVNET_CUT_V1.market === null
    ? 'This deployment has no open market to read a band from. Enter a band centred on what your Source reports today.'
    : 'Reading the band of the market this deployment last opened…');
  /*
   * WHAT THE AUTHOR BELIEVES, which is the input the compiler's gate is
   * measured against and which this wizard did not collect at all.
   *
   * A partition is not degenerate or interesting on its own; it is one or the
   * other RELATIVE TO A BELIEF about where the coordinate goes. Without these
   * three fields there was no belief to state, so the only check the wizard
   * could run was a unit-sanity bound with a provisional constant of its own.
   * The defaults below are admitted by the real gate at the default band;
   * a volatility of 200 bp over this window is NOT, and that is the gate
   * working rather than a bad default -- a band six thousand ticks wide around
   * a coordinate that moves three hundred is a market whose middle cell takes
   * everything.
   */
  const [volatilityBps, setVolatilityBps] = useState('3000');
  const [beliefWindowSlots, setBeliefWindowSlots] = useState('10000');
  const [plausibleHalfWidths, setPlausibleHalfWidths] = useState('2');
  const [cellShareCeilingBps, setCellShareCeilingBps] = useState('9000');

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

  /**
   * Read the last opened market's band, once, and offer it as the starting
   * point. The operator's own edit always wins: the fill only ever lands in a
   * field that is still empty.
   */
  useEffect(() => {
    const featured = PUBLIC_DEVNET_CUT_V1.market;
    if (featured === null) return;
    let live = true;
    void (async () => {
      try {
        const client = new SolanaRpcClient(deployment.endpoint);
        const detail = await inspectMarketDetailV1(client, {
          coreProgramId: deployment.programs.core,
          registryProgramId: deployment.programs.registry,
          address: featured,
        });
        if (detail.card.status !== 'decoded' || detail.registryProgramId === null) throw new Error(detail.reason);
        const read = await inspectMarketQuestionV1(client, {
          registryProgramId: detail.registryProgramId,
          address: featured,
          productRecordId: detail.card.identity.productRecordId,
          resolutionPolicyId: detail.card.identity.resolutionPolicyId,
        });
        const low = read.cuts[0];
        const high = read.cuts[read.cuts.length - 1];
        if (!live || low === undefined || high === undefined || read.cuts.length < 2) return;
        setCutDenominator((current) => current === '' ? read.cutDenominator.toString() : current);
        setLowerEdge((current) => current === '' ? low.toString() : current);
        setUpperEdge((current) => current === '' ? high.toString() : current);
        // The midpoint, and SAID to be the midpoint. A band's centre is not a
        // price: the market this came from was centred on a spot measured at
        // its own founding, and that measurement is not a record anything can
        // read back. Offering the midpoint as a shape to replace is honest;
        // calling it an observation would not be.
        setFoundingObservation((current) => current === '' ? ((low + high) / 2n).toString() : current);
        setBandProvenance(`Band read from ${featured.slice(0, 5)}…${featured.slice(-4)} — its own result-domain record at finalized slot ${read.observedSlot}. The founding observation is that band’s MIDPOINT, not a price: replace it with what your Source reports for the coordinate today, and re-centre the edges on it.`);
      } catch (error) {
        if (!live) return;
        setBandProvenance(`The last opened market's band did not read (${reason(error)}). Enter a band centred on what your Source reports today.`);
      }
    })();
    return () => { live = false; };
  }, [deployment]);

  /**
   * Read the selected feed and centre the band on it.
   *
   * Exact integers throughout: ticks are `price * denominator / 10^-exponent`
   * taken in BigInt, so a spot of 10003917148 at exponent -8 over a
   * denominator of 100 is 10003 ticks and never a rounded double. The band
   * keeps the WIDTH the author already chose and moves its centre, because the
   * width is a judgement about volatility and the centre is a fact.
   */
  async function readTheFeed(): Promise<void> {
    setPriceReading('Reading the sponsored price update at finalized commitment…');
    try {
      const denominator = bigintOrNull(cutDenominator);
      if (denominator === null || denominator <= 0n) throw new Error('set the cut denominator first: a price is only ticks once it has one');
      const price = await readSponsoredPriceV1(new SolanaRpcClient(endpoint), {
        priceUpdateAddress, receiverProgram: priceReceiverProgram,
      });
      if (price.exponent > 0) throw new Error('this feed publishes a positive exponent, which this wizard does not convert');
      const ticks = (price.price * denominator) / 10n ** BigInt(-price.exponent);
      const low = bigintOrNull(lowerEdge);
      const high = bigintOrNull(upperEdge);
      setFoundingObservation(ticks.toString());
      if (low !== null && high !== null && high > low) {
        const half = (high - low) / 2n;
        setLowerEdge((ticks - half).toString());
        setUpperEdge((ticks + half).toString());
      }
      setPriceReading(`${price.decimal} at exponent ${price.exponent}, confidence ${price.confidence.toString()} — published at unix ${price.publishTimeUnixSeconds.toString()}, posted at slot ${price.postedSlot}. That is ${ticks.toString()} ticks at this denominator, and the band above keeps its width and moves its centre onto it.`);
    } catch (error) {
      setPriceReading(`Refused: ${reason(error)}. The observation is unchanged.`);
    }
  }

  const product = useMemo(() => {
    const denominator = bigintOrNull(cutDenominator);
    const low = bigintOrNull(lowerEdge);
    const high = bigintOrNull(upperEdge);
    if (denominator === null || low === null || high === null) return { ok: false as const, message: 'Band edges and the denominator must be whole numbers.' };
    try {
      return { ok: true as const, value: composeRangeProtectionV1({ coordinateLabel, cutDenominator: denominator, lowerEdgeTicks: low, upperEdgeTicks: high }) };
    } catch (error) { return { ok: false as const, message: reason(error) }; }
  }, [coordinateLabel, cutDenominator, lowerEdge, upperEdge]);

  /**
   * The compiled partition-quality gate, loaded once.
   *
   * `null` while it loads and if it fails, and the surface says which. A gate
   * that did not load is not a gate that passed, and this wizard used to have
   * no gate at all.
   */
  const [gate, setGate] = useState<PartitionQualityWasmV1 | null>(null);
  const [gateFailure, setGateFailure] = useState<string | null>(null);
  useEffect(() => {
    let live = true;
    loadPartitionQualityWasmV1()
      .then((loaded) => { if (live) setGate(loaded); })
      .catch((error: unknown) => { if (live) setGateFailure(reason(error)); });
    return () => { live = false; };
  }, []);

  /**
   * What the compiler says about this partition, under this author's belief.
   *
   * THE MEASUREMENT IS NOT MADE HERE. `require_interesting_partition_v1` runs
   * in `crates/dclutch-partition-quality-wasm`, compiled from the same
   * `dclutch-product-compiler` the founding path calls, and its refusals reach
   * the reader by the compiler's own name for them. The wizard used to run a
   * strictly weaker unit-sanity check instead, so a market authored here was
   * never measured by the gate that refuses degenerate partitions.
   */
  const quality = useMemo<
    | Readonly<{ ok: true; report: PartitionQualityReportV1 }>
    | Readonly<{ ok: false; message: string }>
    | null
  >(() => {
    if (gate === null || !product.ok) return null;
    const anchor = bigintOrNull(foundingObservation);
    const denominator = bigintOrNull(cutDenominator);
    const volatility = digitsOrNull(volatilityBps);
    const slots = bigintOrNull(beliefWindowSlots);
    const halfWidths = digitsOrNull(plausibleHalfWidths);
    const ceiling = digitsOrNull(cellShareCeilingBps);
    if (anchor === null || denominator === null || volatility === null || slots === null
        || slots < 0n || halfWidths === null || ceiling === null) return null;
    try {
      return {
        ok: true as const,
        report: requireInterestingPartitionV1(gate, product.value.cuts, {
          kind: 'spot-band',
          anchor,
          denominator,
          volatilityBps: volatility,
          windowSlots: slots,
          plausibleHalfWidths: halfWidths,
        }, ceiling),
      };
    } catch (error) { return { ok: false as const, message: reason(error) }; }
  }, [gate, product, foundingObservation, cutDenominator, volatilityBps, beliefWindowSlots, plausibleHalfWidths, cellShareCeilingBps]);

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
      setPlanStatus(`Accepted as a read-only preview at finalized slot ${next.observedSlot}. Nothing can be signed or submitted from this page.`);
    } catch (error) {
      setPlanStatus(`Refused: ${reason(error)}`);
    }
  }

  return <PageShell className="product-shell wizard-shell" header={<Nav current="/create" />}>

    <section className="market-heading">
      <div>
        <div className="market-kicker"><span>Design · range protection</span><span>Read-only opening preview</span></div>
        <h1>State a band.<br />Price the window.<br />See the whole ladder.</h1>
      </div>
      <p>
        Opening a market takes several ordered steps. This page combines the payout design, answer window, and funding
        with the same exact arithmetic the protocol uses, then shows which steps have a browser preview and which still
        require the operator tooling. Nothing here fabricates a price, a market, or a deployed release.
        And to say it plainly: anyone may design and preview here, with no wallet and no coin — founding the result on
        devnet still runs through the operator tooling today. If you want to poke the live programs, devnet SOL is free
        from the <a href="https://faucet.solana.com" rel="noreferrer">public faucet</a>.
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
        <p>You choose a lower and upper edge, plus what happens if the source is silent. A claim pays one collateral unit when the answer falls outside the band and zero inside it.</p>
      </div></header>
      <div className="direct-form-grid">
        <label><span>Coordinate label (display only)</span><input value={coordinateLabel} onChange={(event) => setCoordinateLabel(event.target.value)} /></label>
        <label><span>Ticks per whole unit · cut denominator</span><input inputMode="numeric" value={cutDenominator} onChange={(event) => setCutDenominator(event.target.value)} /></label>
        <label><span>Display precision · Mint decimals</span><input inputMode="numeric" value={decimals} onChange={(event) => setDecimals(event.target.value)} /></label>
        <label><span>Lower band edge · ticks</span><input inputMode="numeric" value={lowerEdge} onChange={(event) => setLowerEdge(event.target.value)} /></label>
        <label><span>Upper band edge · ticks</span><input inputMode="numeric" value={upperEdge} onChange={(event) => setUpperEdge(event.target.value)} /></label>
        <label><span>Founding observation · ticks</span><input inputMode="numeric" value={foundingObservation} onChange={(event) => setFoundingObservation(event.target.value)} /><small className="feed-forward">What this market&rsquo;s Source reports for the coordinate today, in the same ticks as the band. The Source returns raw provider atoms and rescales nothing, so this is where a band in the wrong units becomes visible.</small></label>
      </div>
      <p className="direct-status">{bandProvenance}</p>
      <fieldset className="direct-form-grid">
        <legend>Read the price this market will resolve against</legend>
        <label><span>Sponsored price update account</span><input value={priceUpdateAddress} onChange={(event) => setPriceUpdateAddress(event.target.value.trim())} spellCheck={false} /></label>
        <label><span>Receiver program that maintains it</span><input value={priceReceiverProgram} onChange={(event) => setPriceReceiverProgram(event.target.value.trim())} spellCheck={false} /><small className="feed-forward">Checked inside the Source family&rsquo;s own decoder: a 134-byte account with the right discriminator is not a price unless the program that maintains it says so.</small></label>
      </fieldset>
      <div className="direct-actions">
        <button type="button" onClick={() => void readTheFeed()} disabled={priceUpdateAddress === '' || priceReceiverProgram === ''}>Read the feed and centre the band</button>
      </div>
      <p className="direct-status" aria-live="polite">{priceReading}</p>
      <fieldset className="direct-form-grid wizard-belief">
        <legend>What you believe the coordinate does</legend>
        <label><span>Volatility · basis points of spot over the window</span><input inputMode="numeric" value={volatilityBps} onChange={(event) => setVolatilityBps(event.target.value)} /><small className="feed-forward">A partition is not degenerate or interesting on its own — it is one or the other <em>relative to a belief</em> about where the coordinate goes. This is that belief, and the gate below is measured against it.</small></label>
        <label><span>Window · slots from founding to deadline</span><input inputMode="numeric" value={beliefWindowSlots} onChange={(event) => setBeliefWindowSlots(event.target.value)} /><small className="feed-forward">Slots, not seconds: the band the compiler measures is quoted over this market&rsquo;s own window in the unit the chain counts in, and converting here would put a second author on the slot clock.</small></label>
        <label><span>Plausible half-widths</span><input inputMode="numeric" value={plausibleHalfWidths} onChange={(event) => setPlausibleHalfWidths(event.target.value)} /><small className="feed-forward">How many characteristic displacements the band is taken to reach each way.</small></label>
        <label><span>Largest share one outcome may take · basis points</span><input inputMode="numeric" value={cellShareCeilingBps} onChange={(event) => setCellShareCeilingBps(event.target.value)} /><small className="feed-forward">Your ceiling, stated at or below the compiler&rsquo;s own maximum{gate === null ? '' : ` of ${gate.partition_quality_maximum_ceiling_bps_v1()}`}. Above it the compiler refuses CellShareCeilingAboveMaximum.</small></label>
      </fieldset>
      {gateFailure !== null && <div className="market-refusal"><strong>The partition gate did not load.</strong> {gateFailure} Nothing below has been measured against it, and a gate that did not load is not a gate that passed.</div>}
      {gate === null && gateFailure === null && <p className="direct-status">Loading the compiled partition gate…</p>}
      {product.ok && quality !== null && (quality.ok
        ? <div className="direct-status">
          <strong>Admitted · {quality.report.model}.</strong> Measured by the compiler&rsquo;s own <code>require_interesting_partition_v1</code>: outcome {quality.report.dominantCell} holds the most ex-ante mass at {quality.report.dominantShareBps} of {quality.report.ceilingBps} permitted basis points.
          <table className="wizard-table">
            <thead><tr><th>Ordinary cell</th><th>Ex-ante share · basis points</th></tr></thead>
            <tbody>
              {quality.report.cellShareBps.map((share, index) => <tr key={index} className={index === quality.report.dominantCell ? 'wizard-paying' : ''}>
                <td>{index}</td><td>{share}</td>
              </tr>)}
            </tbody>
          </table>
          <p className="wizard-rung-reason">
            Characteristic displacement {quality.report.characteristicDisplacement === null ? 'is not measured under this model' : `${quality.report.characteristicDisplacement.toString()} ticks`},
            {' '}plausible half-width {quality.report.plausibleHalfWidth === null ? 'likewise' : `${quality.report.plausibleHalfWidth.toString()} ticks`}.
            {' '}Mass landing on no ordinary cell: {quality.report.unresolvedShareBps} basis points.
          </p>
        </div>
        : <div className="market-refusal">
          <strong>Refused · {quality.message}.</strong> This is the compiler&rsquo;s own refusal, raised by the same <code>dclutch-product-compiler</code> the founding path calls — not a client&rsquo;s guess at one. A partition where one outcome takes more than your stated ceiling is a market whose answer is already known, and it is refused before it is founded rather than after.
        </div>)}
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
          and a payout design that is neither empty nor ambiguous.
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
        <h2>Backing, the capacity ratio, and seven named funding purposes</h2>
        <p>Opening mints one complete set per collateral atom, so the founder initially holds every outcome. Each operating purpose is funded separately, and network lamports are never added to collateral atoms.</p>
      </div></header>
      <div className="direct-form-grid">
        <label><span>Founding principal · raw collateral atoms</span><input inputMode="numeric" value={principal} onChange={(event) => setPrincipal(event.target.value)} /></label>
        <label><span>Venue cost floor · lamports{floorRecord.kind === 'decoded' ? ' · from the technical proof below' : ' · stated'}</span><input inputMode="numeric" disabled={floorRecord.kind === 'decoded'} value={floorRecord.kind === 'decoded' ? floorRecord.floor.floorAtoms.toString() : venueFloor} onChange={(event) => setVenueFloor(event.target.value)} /></label>
        <label><span>Capacity ratio · numerator / denominator</span><span className="wizard-pair">
          <input inputMode="numeric" value={kappaNumerator} onChange={(event) => setKappaNumerator(event.target.value)} />
          <input inputMode="numeric" value={kappaDenominator} onChange={(event) => setKappaDenominator(event.target.value)} />
        </span></label>
      </div>

      <details className="trade-v3-bytes">
        <summary>Technical: verify a venue-derived floor</summary>
        <label><span>Venue floor proof · {MANIPULATION_FLOOR_V1_BYTES} bytes of hexadecimal, optional</span>
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
      </details>

      {kappa.ok ? <div className={`wizard-kappa ${kappa.verdict.admitted ? 'admitted' : 'refused'}`}>
        <strong>{kappa.verdict.admitted ? 'Under the capacity bound' : `Over the capacity bound · ${kappa.verdict.refusal}`}</strong>
        <p>
          The predicate is <code>principal · denominator ≤ numerator · floor</code>, cross-multiplied so there is no
          division and no rounding. At a ratio of {formatCapacityV1(capacity)} against a floor of {BigInt(venueFloor || '0').toLocaleString()} lamports,
          {' '}the largest admitted principal is {kappa.verdict.largestAdmittedPrincipal === null ? 'none' : kappa.verdict.largestAdmittedPrincipal.toLocaleString()} atoms.
          {kappa.verdict.scaled !== null && kappa.verdict.bound !== null && <> This founding states {kappa.verdict.scaled.toLocaleString()} against a bound of {kappa.verdict.bound.toLocaleString()}.</>}
        </p>
        <p className="wizard-enforcement">
          Enforcement: <strong>{kappa.verdict.enforcement}</strong>. ProjectFound authenticates the selected Source graph,
          converts the atom cap to complete-set units at one floor-division boundary, and generic Found refuses a quantity
          above that cap before mutation. Core then persists the exact <code>principal_cap_sets</code> value. This typed
          preview is not chain evidence: the opening route still has to authenticate the floor and its three bindings.
        </p>
      </div> : <p className="direct-refusal">Refused: {kappa.message}</p>}

      {backing && product.ok && <dl className="found-facts">
        <div><dt>Complete sets minted</dt><dd>{backing.completeSets.toLocaleString()} ({formatAtoms(backing.completeSets, displayDecimals)} at {displayDecimals} decimals)</dd></div>
        <div><dt>Per-outcome supply</dt><dd>{backing.perOutcomeSupplyAtoms.map((entry) => entry.toString()).join(' · ')} atoms</dd></div>
        <div><dt>Required backing while unresolved</dt><dd>{backing.requiredBackingWhileUnresolvedAtoms.toLocaleString()} atoms · max(supply)</dd></div>
        <div><dt>Paying outcomes</dt><dd>{backing.payingOutcomes.map((index) => product.value.outcomes[index].label).join(' · ')}</dd></div>
      </dl>}

      <h3 className="wizard-subhead">Operating funds · one quote per service, {manifest.ok ? manifest.entries : 0} services</h3>
      <table className="wizard-table">
        <thead><tr><th>Purpose</th><th>Asset rule</th><th>Per service · lamports</th><th>Plan total</th></tr></thead>
        <tbody>{FUNDING_COMPARTMENTS_V1.map((compartment) => {
          const total = manifest.ok ? manifest.totals.perCompartment.find((entry) => entry.name === compartment.name) : undefined;
          return <tr key={compartment.name} className={total && total.amount > 0n ? 'wizard-paying' : ''}>
            <td>{compartment.name}</td>
            <td>{compartment.assetPolicy}</td>
            {/* The row's first cell names this box on screen and to nobody
                else: a screen reader does not read the cell to the left, and
                these `<th>`s carry no `scope`, so the field announced itself
                as "edit, blank" once per compartment. The name says what the
                number is as well as which service it belongs to. */}
            <td><input inputMode="numeric" className="wizard-inline-input" aria-label={`${compartment.name} · lamports per service`} value={compartments[compartment.name]} onChange={(event) => setCompartments((current) => ({ ...current, [compartment.name]: event.target.value }))} /></td>
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
            The funding plan encodes to {manifest.bytes.length} bytes and passes the opening decoder.
            {' '}Service identities here are preview placeholders; a real opening names checked releases.
          </p>
        : <p className="direct-refusal">Refused: {manifest.message}</p>}
      <p className="direct-refusal">
        <code>Rent</code> and <code>Creation</code> pay for account existence and admit native lamports only. The other five
        are selected by the service. Both totals are recomputed from the named purposes and never taken from a caller, and the
        Realm collateral binding is present exactly when the Realm total is nonzero.
      </p>
      {/* RULING D1 item 2, on the founding surface: the terms a founder agrees
          to include who is paid first when somebody cranks this market's
          escrows, and what that leaves the opener short. It sits in the funding
          step because that is where a founder is already reading what opening
          costs, and it is the one cost on this page the seven named purposes do
          NOT cover -- it is borne by whoever opens an escrow later, which on a
          quiet market is the founder again. */}
      {product.ok && <OpenerFirstCrankTerms
        endpoint={deployment.endpoint}
        outcomeCount={product.value.outcomes.length}
        heading="And one cost the seven purposes do not cover: the first crank"
      />}
    </section>}

    {step === 'review' && <section className="direct-card">
      <header className="direct-card-heading"><span>04</span><div>
        <h2>The ordered opening plan</h2>
        <p>{ladder.rungs} steps. {ladder.browserBuilders} have a browser preview, {ladder.browserFrames} can be assembled from operator-supplied coordinates, and {ladder.toolingOnly} remain in the operator tooling. Three use an address lookup table.</p>
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
          <details><summary>Technical builder details</summary><dl>
            <div><dt>Transactions</dt><dd>{rung.transactions}</dd></div>
            <div><dt>Builder</dt><dd><code>{rung.builder}</code></dd></div>
          </dl></details>
          <p className="wizard-rung-reason">{rung.reason}</p>
        </li>)}
      </ol>
      <p className="direct-refusal">
        A one-click “Create market” button would misstate this plan. Steps marked for operator tooling must use the
        first-party implementation that owns their transaction meaning; this page does not reimplement them in the browser.
      </p>
    </section>}

    {step === 'submit' && <section className="direct-card">
      <header className="direct-card-heading"><span>05</span><div>
        <h2>Inspect the partial browser preview</h2>
        <p>This read-only check reacquires the immutable releases, Product semantics, and account frame at finalized commitment. It does not ask for a wallet, create a lookup table, sign a transaction, spend devnet SOL, or open a Market.</p>
      </div></header>

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
        <button type="submit">Inspect and construct the unsigned preview</button>
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
      </>}
      <p className="direct-refusal">
        This page deliberately has no signing or submission button. The partial browser pair would spend devnet funds
        and stop before the Market is open. The complete operator campaign owns the durable
        journal, every opening step, the final transition, and recovery after a crash. Until that complete
        caller is available here, use this page only to review the design and inspect the unsigned preview.
      </p>
    </section>}
  </PageShell>;
}
