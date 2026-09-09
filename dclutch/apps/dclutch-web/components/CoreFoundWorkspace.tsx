'use client';

import PageShell from '@/components/PageShell';
import Anchor from '@/components/Anchor';
import ConsoleHeader from '@/components/ConsoleHeader';
import { FormEvent, useState } from 'react';

import { deriveCoreFoundRecordsV2, prepareCoreFoundV2, type CoreFoundInputV2, type CoreFoundPlanV2 } from '@dclutch/sdk/coreFound';
import { CORE_FOUND_ACCOUNT_LABELS_V3, CORE_FOUND_ACCOUNT_ROLES_V3 } from '@dclutch/sdk/generated/coreFound';
import { useDeploymentFieldV1, useDeploymentV1 } from '@/lib/deploymentStore';
import { SolanaRpcClient } from '@dclutch/sdk/rpc';
import {
  DerivedProvenance,
  EndpointField,
  OperatorRefusal,
  PubkeyField,
  U64Field,
} from '@/components/operator/OperatorFields';
import { assignFoundRefusalV1 } from '@/components/operator/foundRefusals';
import CommandRunbook from '@/components/operator/CommandRunbook';
import WalletDirectory, { useWalletDirectoryV1 } from '@/components/WalletDirectory';

type AddressField = Exclude<keyof CoreFoundInputV2, 'generation' | 'lookupTable'>;
type AddressValues = Record<AddressField, string>;
type BuildState =
  | Readonly<{ kind: 'idle' | 'loading' | 'error'; message: string }>
  | Readonly<{ kind: 'ready'; plan: CoreFoundPlanV2; rentBase64: string | null; foundBase64: string | null }>;

export const CURRENT_FOUND_RUNBOOK_V1 = `dclutch-terminal --rpc "$DEVNET_RPC" \\
  --i-mean-devnet EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG \\
  --bootstrap-bin "$SUCCESSOR" found \\
  --found-operation "$FOUND_OPERATION" \\
  --found-journal "$FOUND_JOURNAL"

# Review the authored Market input and read-only journal, then authorize the
# exact same operation. Rerun this line unchanged after an interruption.
dclutch-terminal --rpc "$DEVNET_RPC" \\
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
const ADDRESS_FIELDS: ReadonlyArray<Readonly<{ field: AddressField; label: string; group: 'authority' | 'deployment' | 'records'; provenance: string; derived?: 'product' | 'source' | 'wallet' }>> = Object.freeze([
  { field: 'payer', label: 'Payer', group: 'authority', derived: 'wallet',
    provenance: 'The wallet that pays transaction fees and account deposits. Connect a wallet to fill its address, or enter another payer.' },
  { field: 'refundWallet', label: 'Immutable rent refund wallet', group: 'authority',
    provenance: 'Account deposits return to this wallet. It may differ from the payer and cannot be changed after the RentCredit is created.' },
  { field: 'registryProgram', label: 'Registry program', group: 'deployment', provenance: '' },
  { field: 'activationCache', label: 'Release activation cache', group: 'deployment', provenance: '' },
  { field: 'realmRecord', label: 'Realm raw record', group: 'records',
    provenance: 'The finalized Registry raw record holding this market\u2019s Realm. Its address is the PDA of the Realm schema and the record\u2019s own content digest.' },
  { field: 'productRecord', label: 'Product Runtime V2 raw', group: 'records',
    provenance: 'The finalized record holding the Product Runtime V2 root this market pays by. It names the result domain and the portfolio below.' },
  { field: 'resultDomainRecord', label: 'Result domain raw', group: 'records', derived: 'product',
    provenance: 'The result domain selected by the Product. Use “Read the four dependent records” to fill this address.' },
  { field: 'portfolioRecord', label: 'Portfolio raw', group: 'records', derived: 'product',
    provenance: 'The portfolio selected by the Product. Use “Read the four dependent records” to fill this address.' },
  { field: 'linkedBasisRecord', label: 'Linked basis raw', group: 'records',
    provenance: 'The Registry address of the graded basis record required by Found37.' },
  { field: 'sourceMaterialRecord', label: 'SourceMaterialV3 raw', group: 'records',
    provenance: 'The source material record naming the Product, source specification and manipulation floor.' },
  { field: 'sourceSpecRecord', label: 'Source spec raw', group: 'records', derived: 'source',
    provenance: 'The specification selected by SourceMaterialV3. Use “Read the four dependent records” to fill this address.' },
  { field: 'capacityProfileRecord', label: 'Source capacity profile raw', group: 'records',
    provenance: 'Enter the capacity profile address selected by the source specification.' },
  { field: 'manipulationFloorRecord', label: 'Manipulation floor raw', group: 'records', derived: 'source',
    provenance: 'The manipulation floor selected by SourceMaterialV3. Use “Read the four dependent records” to fill this address.' },
  { field: 'capabilityManifestRecord', label: 'Capability manifest raw', group: 'records',
    provenance: 'The Registry record listing the market’s trading and lifecycle services.' },
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
  const wallets = useWalletDirectoryV1();
  const [addresses, setAddresses] = useState<AddressValues>(emptyAddresses);
  // Deployment-derivable fields arrive filled; an operator's edit overrides.
  const effective: AddressValues = {
    ...addresses,
    registryProgram: addresses.registryProgram !== '' ? addresses.registryProgram : deployment.programs.registry,
    activationCache: addresses.activationCache !== '' ? addresses.activationCache : deployment.activationCache ?? '',
    // The one address on this form whose answer is already in the reader's
    // browser. Asking somebody to transcribe their own public key is the
    // purest case of OPERATOR_FORMS_V1 §0, and an edit still overrides it —
    // the payer does not have to be the wallet that reads this page.
    payer: addresses.payer !== '' ? addresses.payer : wallets.address ?? '',
  };
  const [generation, setGeneration] = useState('1');
  const [state, setState] = useState<BuildState>({
    kind: 'idle',
    message: 'Enter the market record addresses to prepare the unsigned transactions.',
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
  const [derivation, setDerivation] = useState('Enter Product and SourceMaterialV3 addresses, then read their dependent records.');
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
        // A Source material with an explicitly unbounded principal policy
        // names no floor, and the console must not paste an empty box over a
        // reader's own value: it leaves the field exactly as it found it and
        // says so in the provenance line beside it.
        manipulationFloorRecord: read.manipulationFloorRecord ?? current.manipulationFloorRecord,
      }));
      setDerivedFrom({
        resultDomainRecord: read.provenance.resultDomainRecord,
        portfolioRecord: read.provenance.portfolioRecord,
        sourceSpecRecord: read.provenance.sourceSpecRecord,
        manipulationFloorRecord: read.provenance.manipulationFloorRecord,
      });
      setDerivation(`${read.manipulationFloorRecord === null ? 'Three' : 'Four'} addresses read from their parent records at finalized slot ${read.observedSlot}. Enter the capacity profile separately.`);
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
      : entry.field === 'activationCache' ? deployment.activationCache ?? ''
      : entry.derived === 'wallet' ? wallets.address ?? '' : null;
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
            absent={entry.field === 'registryProgram'
              ? 'Pick a cluster in the header to fill this, or paste the Registry program address.'
              : entry.field === 'activationCache'
              ? 'Pick a cluster in the header to fill this, or paste the activation cache this release derives.'
              : 'Connect a wallet above to fill this, or paste the payer address.'}
            source={entry.derived === 'wallet' ? 'the wallet connected above' : 'the deployment this browser is pointed at'} />}
      />
      {routed === null ? null : <OperatorRefusal remedy={routed.remedy} detail={routed.detail} />}
    </div>;
  }

  const group = (name: 'authority' | 'deployment' | 'records') => ADDRESS_FIELDS.filter((entry) => entry.group === name);
  return <PageShell className="product-shell direct-workspace found-workspace" header={<ConsoleHeader path="/found" title="Found a market" purpose="Prepare a devnet market, open it and admit its first participant with the operator tools." />}>

    <section className="market-heading found-heading"><div><h1>Create a market.<br />Add a participant.</h1></div><p>Prepare an operation document, preview its funding and market terms, then run the founding command. Keep the document and journal to resume an interrupted operation.</p></section>

    <section className="found-current-campaign" id="current-founding"><header className="direct-card-heading"><span>Current</span><div><h2>Found a devnet market</h2><p>Use the command-line workflow below to create and open the market, then admit its first participant.</p></div></header><div className="found-current-contract"><article><span>Input</span><strong>One operation document</strong><p>A <code>dclutch-devnet-market-participant-operation-v1</code> document contains the market plan, funding inputs, output paths and signing-key paths.</p></article><article><span>Action</span><strong>Preview, then execute</strong><p>The preview reads the inputs. Add <code>--execute</code> to sign and submit the transactions, paying the required devnet fees and deposits.</p></article><article><span>Result</span><strong>Market + first participant + session</strong><p>The command returns market and participant details. Save the optional session file for later trading and redemption commands.</p></article><article><span>Recovery</span><strong>Rerun the same operation and journal</strong><p>After an interruption, rerun the same command with the same operation document and journal to continue.</p></article></div><details><summary>Show preview and execute commands</summary><p>Set every shell variable to an absolute path. Review the operation document and preview outputs before adding <code>--execute</code>.</p><CommandRunbook label="Preview, then authorized execution" command={CURRENT_FOUND_RUNBOOK_V1} /></details></section>

    <details className="found-legacy-inspector"><summary>Open the legacy Found37 packet inspector</summary><p>This inspector prepares older unsigned Found37 transactions. Use the command-line workflow above to open a market.</p>

    <section className="found-boundaries" aria-label="Construction boundaries">
      <article><span>01</span><strong>Select execution</strong><p>The activation cache must select immutable Core, Registry, and Rent artifacts whose Loader observations still match.</p></article>
      <article><span>02</span><strong>Check market records</strong><p>Product, domain, portfolio, Source, Realm, capabilities, and releases are decoded from finalized Registry bytes.</p></article>
      <article><span>03</span><strong>Prepare account deposits</strong><p>Create the RentCredit with your chosen refund wallet before submitting Found37.</p></article>
    </section>

    <form className="direct-card found-form" onSubmit={construct}>
      <header className="direct-card-heading"><span>01</span><div><h2>Chain authority and record coordinates</h2><p>Enter the Registry record addresses used by the market.</p></div></header>
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
        <p>Connect a wallet to fill the payer address. Choose the rent refund wallet carefully: it cannot change after the RentCredit is created. This inspector exports unsigned transactions.</p>
        <WalletDirectory directory={wallets} onConnected={(address) => update('payer', address)} />
        <div className="operator-act-grid">{group('authority').map(addressField)}</div>
      </fieldset>

      <fieldset className="operator-act">
        <legend>The deployment this founds against</legend>
        <p>These addresses come from the selected deployment. You can edit them.</p>
        <div className="operator-act-grid">{group('deployment').map(addressField)}</div>
      </fieldset>

      <fieldset className="operator-act">
        <legend>The ten finalized records this market is built from</legend>
        <p>Enter the Product and SourceMaterialV3 addresses, then read the four dependent records. Supply the remaining record addresses separately.</p>
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
        ? <p className="direct-status" aria-live="polite">Check the highlighted field above.</p>
        : <p className="direct-status" aria-live="polite">{state.kind === 'ready' ? `Unsigned transactions prepared from finalized slot ${state.plan.observedSlot}.` : state.message}</p>}
      {refusal !== null && !refusal.routed
        ? <OperatorRefusal remedy={refusal.remedy} detail={refusal.detail} />
        : null}
    </form>

    {ready === null ? <section className="direct-card found-empty"><div className="radar"><span /></div><div><p className="eyebrow">Prepare the records</p><h2>Enter the records to inspect the transactions.</h2><p>The inspector checks the records, required account deposits and transaction size.</p></div></section> : <>
      <section className="direct-card found-result">
        <header className="direct-card-heading"><span>02</span><div><h2>Two legacy unsigned packets inspected</h2><p>Use the command-line workflow above to complete market opening.</p></div></header>
        <div className="found-verdict"><span>{ready.plan.infrastructureRecognition.kind}</span><strong>{ready.plan.outcomeCount.toLocaleString()} outcomes · Rent {ready.plan.rentCreateWireBytes === null ? 'already created' : `${ready.plan.rentCreateWireBytes.length} bytes`} / Found {ready.plan.wireBytes === null ? 'unroutable' : `${ready.plan.wireBytes.length} bytes`}</strong><p>Compare the selected release with your deployment manifest.</p></div>
        <dl className="found-facts"><div><dt>Derived Market</dt><dd>{ready.plan.market}</dd></div><div><dt>Lifecycle RentCredit</dt><dd>{ready.plan.rentCredit}</dd></div><div><dt>Product identity</dt><dd>{ready.plan.productId}</dd></div><div><dt>Product record digest</dt><dd>{ready.plan.productRecordDigest}</dd></div><div><dt>Execution release set</dt><dd>{ready.plan.executionReleaseSetId}</dd></div><div><dt>Infrastructure profile</dt><dd>{ready.plan.infrastructureProfile}</dd></div><div><dt>Core / Registry / Rent</dt><dd>{compact(ready.plan.coreProgram)} · {compact(ready.plan.registryProgram)} · {compact(ready.plan.rentProgram)}</dd></div><div><dt>Rent debit</dt><dd>{ready.plan.rentCreditRentDebit} credit + {ready.plan.marketRentTopUp} Market lamports</dd></div><div><dt>Blockhash validity</dt><dd>through block height {ready.plan.lastValidBlockHeight}</dd></div></dl>
        {ready.rentBase64 === null
          ? <p className="direct-refusal">RentCredit already exists at {ready.plan.rentCredit} for this generation; its setup can be skipped.</p>
          : <>
              <label><span>1 · unsigned lifecycle RentCredit Create · base64</span><textarea className="found-packet" readOnly value={ready.rentBase64} /></label>
              <div className="found-export"><a download={`dclutch-rent-create-${ready.plan.market}.tx`} href={`data:application/octet-stream;base64,${ready.rentBase64}`}>Download Rent Create packet</a><span>Confirm this packet before Found.</span></div>
            </>}
        {ready.foundBase64 === null
          ? <div className="direct-refusal">
              <strong>Found37 is not downloadable from this route.</strong> {ready.plan.foundRefusal} Its {ready.plan.routableAddresses.length} account addresses are available for inspection. Use the founding commands above to open the market, or return to <Anchor href="/create">market design</Anchor>.
            </div>
          : <>
              <label><span>2 · unsigned Core Found · base64</span><textarea className="found-packet" readOnly value={ready.foundBase64} /></label>
              <div className="found-export"><a download={`dclutch-found-${ready.plan.market}.tx`} href={`data:application/octet-stream;base64,${ready.foundBase64}`}>Download Found packet</a><span>Unsigned transaction</span></div>
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
