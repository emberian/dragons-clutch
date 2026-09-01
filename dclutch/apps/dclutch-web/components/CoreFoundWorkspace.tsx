'use client';

import PageShell from '@/components/PageShell';
import Anchor from '@/components/Anchor';
import ConsoleHeader from '@/components/ConsoleHeader';
import { FormEvent, useState } from 'react';

import { deriveCoreFoundRecordsV2, prepareCoreFoundV2, type CoreFoundInputV2, type CoreFoundPlanV2 } from '@/lib/coreFound';
import { CORE_FOUND_ACCOUNT_LABELS_V3, CORE_FOUND_ACCOUNT_ROLES_V3 } from '@/lib/generated/coreFound';
import { useDeploymentFieldV1, useDeploymentV1 } from '@/lib/deploymentStore';
import { SolanaRpcClient } from '@/lib/rpc';
import {
  DerivedProvenance,
  EndpointField,
  OperatorRefusal,
  PubkeyField,
  U64Field,
} from '@/components/operator/OperatorFields';
import { assignFoundRefusalV1 } from '@/components/operator/foundRefusals';
import CommandRunbook from '@/components/operator/CommandRunbook';

type AddressField = Exclude<keyof CoreFoundInputV2, 'generation' | 'lookupTable'>;
type AddressValues = Record<AddressField, string>;
type BuildState =
  | Readonly<{ kind: 'idle' | 'loading' | 'error'; message: string }>
  | Readonly<{ kind: 'ready'; plan: CoreFoundPlanV2; rentBase64: string | null; foundBase64: string | null }>;

export const CURRENT_FOUND_RUNBOOK_V1 = `dclutch --rpc "$DEVNET_RPC" \\
  --i-mean-devnet EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG \\
  --bootstrap-bin "$SUCCESSOR" found \\
  --found-operation "$FOUND_OPERATION" \\
  --found-journal "$FOUND_JOURNAL"

# Review the authored Market input and read-only journal, then authorize the
# exact same operation. Rerun this line unchanged after an interruption.
dclutch --rpc "$DEVNET_RPC" \\
  --i-mean-devnet EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG \\
  --bootstrap-bin "$SUCCESSOR" found \\
  --found-operation "$FOUND_OPERATION" \\
  --found-journal "$FOUND_JOURNAL" \\
  --session-out "$CLI_SESSION" --execute`;

/**
 * Every address this console asks for, with the one concrete sentence saying
 * where it comes from.
 *
 * OPERATOR_FORMS_V1 §0, which is `ArtifactInput`'s own rule generalised: "if a
 * console asks you to paste something and you don't know where it comes from,
 * that's a bug in the console." Fourteen addresses arrived here with a label
 * and nothing else.
 *
 * `derived` marks the four this console now READS rather than asks for. Each
 * is a digest a parent record on this same list already carries at a named
 * coordinate, and digest plus schema is the whole input to the Registry's raw
 * PDA -- so asking a reader for it was asking them to be an oracle for a chain
 * read this console can make itself. §3.2 recorded five of these as named
 * debt; four are paid, and the fifth says in its own provenance line why it is
 * not (`SourceSpecV1` writes that coordinate as a bare number, so there is no
 * constant for the ABI generator to emit and nothing to import instead of
 * restating it).
 */
const ADDRESS_FIELDS: ReadonlyArray<Readonly<{ field: AddressField; label: string; group: 'authority' | 'deployment' | 'records'; provenance: string; derived?: 'product' | 'source' }>> = Object.freeze([
  { field: 'payer', label: 'Payer', group: 'authority',
    provenance: 'The wallet that funds both packets and signs them elsewhere. It must be a plain System-owned wallet holding no account data.' },
  { field: 'refundWallet', label: 'Immutable rent refund wallet', group: 'authority',
    provenance: 'Embedded once in the Market-bound RentCredit and immutable afterwards, so rent returns here rather than to the payer. Often the payer, and it does not have to be.' },
  { field: 'registryProgram', label: 'Registry program', group: 'deployment', provenance: '' },
  { field: 'activationCache', label: 'Release activation cache', group: 'deployment', provenance: '' },
  { field: 'realmRecord', label: 'Realm raw record', group: 'records',
    provenance: 'The finalized Registry raw record holding this market\u2019s Realm. Its address is the PDA of the Realm schema and the record\u2019s own content digest.' },
  { field: 'productRecord', label: 'Product Runtime V2 raw', group: 'records',
    provenance: 'The finalized record holding the Product Runtime V2 root this market pays by. It names the result domain and the portfolio below.' },
  { field: 'resultDomainRecord', label: 'Result domain raw', group: 'records', derived: 'product',
    provenance: 'The result domain the Product root selects. Read it out of the Product record above rather than finding it: the digest is at a named coordinate and the address is that digest under the result-domain schema.' },
  { field: 'portfolioRecord', label: 'Portfolio raw', group: 'records', derived: 'product',
    provenance: 'The portfolio the Product root selects. Read it out of the Product record above rather than finding it: the digest is at a named coordinate and the address is that digest under the portfolio schema.' },
  { field: 'linkedBasisRecord', label: 'Linked basis raw', group: 'records',
    provenance: 'A graded basis record. It is authenticated for PDA, owner and rent and placed in the Found37 frame \u2014 and unlike the other nine, none of its bytes are joined to the semantic graph.' },
  { field: 'sourceMaterialRecord', label: 'SourceMaterialV3 raw', group: 'records',
    provenance: 'The SourceMaterialV3 record. It names the Product digest, the source spec, and the manipulation floor, so three of the fields below are answers it already contains.' },
  { field: 'sourceSpecRecord', label: 'Source spec raw', group: 'records', derived: 'source',
    provenance: 'The source spec SourceMaterialV3 selects. Read it out of the SourceMaterialV3 record above rather than finding it: the digest is at a named coordinate and the address is that digest under the source-spec schema.' },
  { field: 'capacityProfileRecord', label: 'Source capacity profile raw', group: 'records',
    provenance: 'The capacity profile the source spec selects. It is the one address on this list that stays typed: SourceSpecV1 writes that coordinate as a bare number with no named constant, so there is nothing this browser could import instead of restating it. Derivable from that record once this console reads it; today it is typed and then checked.' },
  { field: 'manipulationFloorRecord', label: 'Manipulation floor raw', group: 'records', derived: 'source',
    provenance: 'The manipulation floor SourceMaterialV3 selects. Read it out of the SourceMaterialV3 record above rather than finding it: the digest is at a named coordinate and the address is that digest under the manipulation-floor schema.' },
  { field: 'capabilityManifestRecord', label: 'Capability manifest raw', group: 'records',
    provenance: 'The capability manifest this market founds with. Its dependency graph must terminate; a cycle is refused.' },
]);

function emptyAddresses(): AddressValues {
  return Object.fromEntries(ADDRESS_FIELDS.map(({ field }) => [field, ''])) as AddressValues;
}

function canonicalU64(value: string): bigint {
  if (!/^(0|[1-9][0-9]*)$/.test(value)) throw new Error('generation must be a canonical unsigned integer');
  const parsed = BigInt(value);
  if (parsed > 0xffff_ffff_ffff_ffffn) throw new Error('generation exceeds u64');
  return parsed;
}

function failure(error: unknown): string {
  return error instanceof Error ? error.message : 'construction failed without a usable refusal reason';
}

function encodeBase64(bytes: Uint8Array): string {
  let binary = '';
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary);
}

function accountRole(index: number): string {
  const role = CORE_FOUND_ACCOUNT_ROLES_V3[index];
  if (role === undefined) return 'unknown role';
  if (role.signer && role.writable) return 'writable · signer';
  if (role.signer) return 'signer';
  return role.writable ? 'writable' : 'read only';
}

function compact(value: string): string {
  return value.length > 24 ? `${value.slice(0, 10)}…${value.slice(-9)}` : value;
}

export default function CoreFoundWorkspace() {
  const deployment = useDeploymentV1();
  const [endpoint, setEndpoint] = useDeploymentFieldV1((d) => d.endpoint);
  const [addresses, setAddresses] = useState<AddressValues>(emptyAddresses);
  // Deployment-derivable fields arrive filled; an operator's edit overrides.
  const effective: AddressValues = {
    ...addresses,
    registryProgram: addresses.registryProgram !== '' ? addresses.registryProgram : deployment.programs.registry,
    activationCache: addresses.activationCache !== '' ? addresses.activationCache : deployment.activationCache ?? '',
  };
  const [generation, setGeneration] = useState('1');
  const [state, setState] = useState<BuildState>({
    kind: 'idle',
    message: 'No transaction has been constructed. Enter chain-derived record addresses to begin.',
  });
  /**
   * What the last dependent read produced, per field, or nothing yet.
   *
   * Held separately from the values so a field can say WHERE its value came
   * from. A filled box that cannot say how it was filled is the same defect as
   * an empty one somebody has to go and research; and a field whose provenance
   * line claims a chain read before any read has run is a status typed in
   * advance, which is why this starts empty and only the act fills it.
   */
  const [derivedFrom, setDerivedFrom] = useState<Partial<Record<AddressField, string>>>({});
  const [derivation, setDerivation] = useState('No dependent record has been read.');
  const [deriving, setDeriving] = useState(false);

  function update(field: AddressField, value: string): void {
    setAddresses((current) => ({ ...current, [field]: value.trim() }));
    // An edited field is no longer what the chain said it was.
    setDerivedFrom((current) => { const next = { ...current }; delete next[field]; return next; });
  }

  async function deriveDependents(): Promise<void> {
    setDeriving(true);
    setDerivation('Reading the Product and SourceMaterialV3 records at finalized commitment…');
    try {
      const read = await deriveCoreFoundRecordsV2(new SolanaRpcClient(endpoint), {
        registryProgram: effective.registryProgram,
        productRecord: effective.productRecord,
        sourceMaterialRecord: effective.sourceMaterialRecord,
      });
      setAddresses((current) => ({
        ...current,
        resultDomainRecord: read.resultDomainRecord,
        portfolioRecord: read.portfolioRecord,
        sourceSpecRecord: read.sourceSpecRecord,
        manipulationFloorRecord: read.manipulationFloorRecord,
      }));
      setDerivedFrom({
        resultDomainRecord: read.provenance.resultDomainRecord,
        portfolioRecord: read.provenance.portfolioRecord,
        sourceSpecRecord: read.provenance.sourceSpecRecord,
        manipulationFloorRecord: read.provenance.manipulationFloorRecord,
      });
      setDerivation(`Four addresses read from their parent records at finalized slot ${read.observedSlot}. The capacity profile is still yours to supply.`);
    } catch (error) {
      // A failed derivation must not leave four boxes holding a previous
      // read's answers: the fields it owns are cleared with it.
      setDerivedFrom({});
      setDerivation(`Refused: ${failure(error)}`);
    } finally {
      setDeriving(false);
    }
  }

  async function construct(event: FormEvent<HTMLFormElement>): Promise<void> {
    event.preventDefault();
    setState({ kind: 'loading', message: 'Reading finalized state…' });
    try {
      const plan = await prepareCoreFoundV2(new SolanaRpcClient(endpoint), {
        ...effective,
        generation: canonicalU64(generation),
      });
      setState({ kind: 'ready', plan, rentBase64: plan.rentCreateWireBytes === null ? null : encodeBase64(plan.rentCreateWireBytes), foundBase64: plan.wireBytes === null ? null : encodeBase64(plan.wireBytes) });
    } catch (error) {
      setState({ kind: 'error', message: `Refused: ${failure(error)}` });
    }
  }

  const ready = state.kind === 'ready' ? state : null;
  /**
   * OPERATOR_FORMS_V1 §6. Sixteen fields shared one `aria-live` line, and the
   * ten record refusals named their field by POSITION while the screen named
   * it by role. This routes the refusal to the field that owns it; anything
   * whose owner is ambiguous stays at form level rather than being guessed.
   */
  const refusal = state.kind === 'error' ? assignFoundRefusalV1(state.message) : null;
  const refusalFor = (field: string) => refusal !== null && refusal.field === field ? refusal : null;

  function addressField(entry: (typeof ADDRESS_FIELDS)[number]) {
    const routed = refusalFor(entry.field);
    const derived = entry.field === 'registryProgram' ? deployment.programs.registry
      : entry.field === 'activationCache' ? deployment.activationCache ?? '' : null;
    const read = derivedFrom[entry.field];
    return <div className="operator-field-slot" key={entry.field}>
      <PubkeyField
        label={entry.label}
        value={effective[entry.field]}
        onChange={(next) => update(entry.field, next)}
        required
        provenance={read !== undefined
          ? read
          : derived === null
          ? entry.provenance
          : <DerivedProvenance
            derived={derived === '' ? null : derived}
            value={effective[entry.field]}
            source="the deployment this browser is pointed at"
            absent={entry.field === 'registryProgram'
              ? 'Pick a cluster in the header to fill this, or paste the Registry program address.'
              : 'Pick a cluster in the header to fill this, or paste the activation cache this release derives.'} />}
      />
      {routed === null ? null : <OperatorRefusal remedy={routed.remedy} detail={routed.detail} />}
    </div>;
  }

  const group = (name: 'authority' | 'deployment' | 'records') => ADDRESS_FIELDS.filter((entry) => entry.group === name);
  return <PageShell className="product-shell direct-workspace found-workspace" header={<ConsoleHeader path="/found" title="Found a market" purpose="Run the current journaled devnet founding campaign. The legacy packet inspector remains below for diagnosis only." />}>

    <section className="market-heading found-heading"><div><h1>Found, then<br />admit.</h1></div><p>One operation document drives current Market founding and first-participant admission. Preparation is read-only. Execution is explicit, journaled before any key-owning child runs, and resumes only that same operation.</p></section>

    <section className="found-current-campaign" id="current-founding"><header className="direct-card-heading"><span>Current</span><div><h2>Found one current devnet Market</h2><p>The CLI delegates every authored request, signature, transaction, and poststate report to the current Rust successor. The browser neither reconstructs its frame nor asks for a wallet.</p></div></header><div className="found-current-contract"><article><span>Input</span><strong>One operation document</strong><p>A <code>dclutch-devnet-market-participant-operation-v1</code> names the checked plan, Market producer arguments, evidence outputs, and explicit key files used only by their owning Rust children.</p></article><article><span>Authority</span><strong>Preview first; execute explicitly</strong><p>The first command is read-only. <code>--execute</code> records devnet authorization in the durable journal before the campaign can read a signing key or submit.</p></article><article><span>Result</span><strong>Market + first participant + session</strong><p>Founding, compact opening, and admission emit machine reports. The optional CLI session then feeds discovery, route export, joining, portfolio, offer, and redemption commands.</p></article><article><span>Recovery</span><strong>Rerun the same operation and journal</strong><p>Each stage rereads its exact chain checkpoint. A submitted child report is reconciled; the driver does not start over against a different plan or Market.</p></article></div><details><summary>Show preview and execute commands</summary><p>Set every shell variable to an absolute path. Review the operation document and preview outputs before adding <code>--execute</code>.</p><CommandRunbook label="Preview, then authorized execution" command={CURRENT_FOUND_RUNBOOK_V1} /></details></section>

    <details className="found-legacy-inspector"><summary>Open the legacy Found37 packet inspector</summary><p>This older two-packet reader is useful for diagnosing record and release joins. It cannot perform the current atomic opening and is not the founding path above.</p>

    <section className="found-boundaries" aria-label="Construction boundaries">
      <article><span>01</span><strong>Select execution</strong><p>The activation cache must select immutable Core, Registry, and Rent artifacts whose Loader observations still match.</p></article>
      <article><span>02</span><strong>Join one semantic graph</strong><p>Product, domain, portfolio, Source, Realm, capabilities, and releases are decoded from finalized Registry bytes.</p></article>
      <article><span>03</span><strong>Reacquire &amp; compile</strong><p>The refund wallet is embedded once in a Market-bound RentCredit. Create must confirm before the exact Found37 packet is submitted.</p></article>
    </section>

    <form className="direct-card found-form" onSubmit={construct}>
      <header className="direct-card-heading"><span>01</span><div><h2>Chain authority and record coordinates</h2><p>No program, balance, Product, or release identity is supplied here. Every address is reauthenticated against the chain.</p></div></header>
      <fieldset className="operator-act">
        <legend>The chain this founds against</legend>
        <div className="operator-act-grid">
          <div className="operator-field-slot">
            <EndpointField label="Finalized RPC endpoint" value={endpoint} onChange={setEndpoint} required
              provenance={<DerivedProvenance derived={deployment.endpoint === '' ? null : deployment.endpoint} value={endpoint}
                source="the cluster picked in the header"
                absent="Pick a cluster in the header, or paste the endpoint to read finalized state from." />} />
            {refusalFor('endpoint') === null ? null : <OperatorRefusal remedy={refusalFor('endpoint')!.remedy} detail={refusalFor('endpoint')!.detail} />}
          </div>
          <div className="operator-field-slot">
            <U64Field label="Market generation" value={generation} onChange={setGeneration} noun="generation" min={1n} required
              provenance="Which generation of this market is being founded. The first is 1; a later one reuses the same records under a new lifecycle." />
            {refusalFor('generation') === null ? null : <OperatorRefusal remedy={refusalFor('generation')!.remedy} detail={refusalFor('generation')!.detail} />}
          </div>
        </div>
      </fieldset>

      <fieldset className="operator-act">
        <legend>Who pays, and who is refunded</legend>
        <div className="operator-act-grid">{group('authority').map(addressField)}</div>
      </fieldset>

      <fieldset className="operator-act">
        <legend>The deployment this founds against</legend>
        <p>Both arrive filled from the cluster picked in the header. An edit overrides them and says so.</p>
        <div className="operator-act-grid">{group('deployment').map(addressField)}</div>
      </fieldset>

      <fieldset className="operator-act">
        <legend>The ten finalized records this market is built from</legend>
        <p>Every one is a Registry raw record, reauthenticated against the chain for owner, PDA, rent and exact ABI. Four of them are values the Product and SourceMaterialV3 records above already carry — supply those two and this console reads the other four rather than asking you to find them.</p>
        <div className="direct-actions">
          <button type="button" disabled={deriving} onClick={() => void deriveDependents()}>
            {deriving ? 'Reading the parent records…' : 'Read the four dependent records'}
          </button>
        </div>
        <p className="direct-status" aria-live="polite">{derivation}</p>
        <div className="operator-act-grid">{group('records').map(addressField)}</div>
      </fieldset>

      <button type="submit" disabled={state.kind === 'loading'}>{state.kind === 'loading' ? 'Reacquiring Found37 authority…' : 'Construct unsigned lifecycle + Found transactions'}</button>
      {refusal !== null && refusal.routed
        ? <p className="direct-status" aria-live="polite">This construction refused at one field. Its remedy is with that field, above.</p>
        : <p className="direct-status" aria-live="polite">{state.kind === 'ready' ? `Accepted at finalized slot ${state.plan.observedSlot}. Both transactions remain unsigned and unsubmitted.` : state.message}</p>}
      {refusal !== null && !refusal.routed
        ? <OperatorRefusal remedy={refusal.remedy} detail={refusal.detail} />
        : null}
    </form>

    {ready === null ? <section className="direct-card found-empty"><div className="radar"><span /></div><div><p className="eyebrow">No inferred authority</p><h2>Construction stops at the first broken join.</h2><p>Missing records, stale ELF bytes, mutable infrastructure, same-width Product substitution, account aliases, insufficient rent, and packet overflow are refusals, not warnings. No signing or submission here.</p></div></section> : <>
      <section className="direct-card found-result">
        <header className="direct-card-heading"><span>02</span><div><h2>Two legacy unsigned packets inspected</h2><p>Neither has been signed, funded, simulated, or submitted. The pair is incomplete for current devnet opening.</p></div></header>
        <div className="found-verdict"><span>{ready.plan.infrastructureRecognition.kind}</span><strong>{ready.plan.outcomeCount.toLocaleString()} outcomes · Rent {ready.plan.rentCreateWireBytes === null ? 'already created' : `${ready.plan.rentCreateWireBytes.length} bytes`} / Found {ready.plan.wireBytes === null ? 'unroutable' : `${ready.plan.wireBytes.length} bytes`}</strong><p>An internally consistent release is an official dClutch release only when it matches a separately supplied checked manifest.</p></div>
        <dl className="found-facts"><div><dt>Derived Market</dt><dd>{ready.plan.market}</dd></div><div><dt>Lifecycle RentCredit</dt><dd>{ready.plan.rentCredit}</dd></div><div><dt>Product identity</dt><dd>{ready.plan.productId}</dd></div><div><dt>Product record digest</dt><dd>{ready.plan.productRecordDigest}</dd></div><div><dt>Execution release set</dt><dd>{ready.plan.executionReleaseSetId}</dd></div><div><dt>Infrastructure profile</dt><dd>{ready.plan.infrastructureProfile}</dd></div><div><dt>Core / Registry / Rent</dt><dd>{compact(ready.plan.coreProgram)} · {compact(ready.plan.registryProgram)} · {compact(ready.plan.rentProgram)}</dd></div><div><dt>Rent debit</dt><dd>{ready.plan.rentCreditRentDebit} credit + {ready.plan.marketRentTopUp} Market lamports</dd></div><div><dt>Blockhash validity</dt><dd>through block height {ready.plan.lastValidBlockHeight}</dd></div></dl>
        {ready.rentBase64 === null
          ? <p className="direct-refusal">The Market-scoped lifecycle RentCredit already exists at {ready.plan.rentCredit} for this generation, so there is nothing to create. Found37 names it as a precondition.</p>
          : <>
              <label><span>1 · unsigned lifecycle RentCredit Create · base64</span><textarea className="found-packet" readOnly value={ready.rentBase64} /></label>
              <div className="found-export"><a download={`dclutch-rent-create-${ready.plan.market}.tx`} href={`data:application/octet-stream;base64,${ready.rentBase64}`}>Download Rent Create packet</a><span>Confirm this packet before Found.</span></div>
            </>}
        {ready.foundBase64 === null
          ? <div className="direct-refusal">
              <strong>Found37 is not downloadable from this route.</strong> {ready.plan.foundRefusal} Its {ready.plan.routableAddresses.length} routable
              addresses are derived and available. Building the lookup table and submitting through a wallet is what the
              <Anchor href="/create"> read-only design preview</Anchor> does.
            </div>
          : <>
              <label><span>2 · unsigned Core Found · base64</span><textarea className="found-packet" readOnly value={ready.foundBase64} /></label>
              <div className="found-export"><a download={`dclutch-found-${ready.plan.market}.tx`} href={`data:application/octet-stream;base64,${ready.foundBase64}`}>Download Found packet</a><span>No signing or submission here.</span></div>
            </>}
      </section>

      <section className="direct-card found-accounts">
        <header className="direct-card-heading"><span>03</span><div><h2>Exact account projection</h2><p>This order is the instruction ABI. Only payer and the new Market are writable; only payer signs.</p></div></header>
        <ol>{ready.plan.accountAddresses.map((address, index) => <li key={address}><span>{index.toString().padStart(2, '0')}</span><strong>{CORE_FOUND_ACCOUNT_LABELS_V3[index]}</strong><code>{address}</code><small>{accountRole(index)}</small></li>)}</ol>
      </section>
    </>}
    </details>
  </PageShell>;
}
