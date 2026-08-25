'use client';

import { PublicKey } from '@solana/web3.js';
import Link from 'next/link';
import { FormEvent, useMemo, useState } from 'react';

import {
  ECONOMIC_HOARD_SEED,
  ECONOMIC_PROJECTION_BYTES,
  authenticateEconomicRelease,
  buildEconomicFoundingTransaction,
  buildEconomicOperationTransaction,
  decodeEconomicProjectionV1,
  decodeExecutionReleaseSetV1,
  deriveEconomicFoundingCoordinates,
  inspectEconomicTokenRoute,
  scanEconomicProjections,
  type EconomicAction,
  type EconomicHolder,
  type EconomicProjectionObservationV1,
  type EconomicSnapshotV1,
  type EconomicVacancyObservationV1,
} from '@/lib/economicSuccessor';
import { LEGACY_TOKEN_PROGRAM_ID, decodeLegacyTokenObservationV1 } from '@/lib/registeredDirect';
import { SolanaRpcClient, type ConnectionFacts, type RpcAccount } from '@/lib/rpc';

const SYSTEM_PROGRAM = '11111111111111111111111111111111';
const PLACEHOLDER_BLOCKHASH = SYSTEM_PROGRAM;

type Discovery =
  | Readonly<{ kind: 'idle' | 'loading' | 'error'; message?: string }>
  | Readonly<{ kind: 'ready'; facts: ConnectionFacts; snapshot: EconomicSnapshotV1 }>;

type OperationArtifact = Readonly<{
  action: string;
  projection: string;
  revision: string;
  nextRevision: string;
  phase: string;
  releaseRole: string;
  claimEffects: ReadonlyArray<string>;
  custody: string;
  tokenBalances: string;
  wireBytes: number;
  signers: ReadonlyArray<string>;
  blockhashSlot: string;
  lastValidBlockHeight: string;
  base64: string;
}>;

type FoundingArtifact = Readonly<{
  projection: string;
  marketId: string;
  releaseId: string;
  outcomeCount: number;
  collateralMint: string;
  hoard: string;
  projectionLamports: string;
  minimumRent: string;
  wireBytes: number;
  signers: ReadonlyArray<string>;
  blockhashSlot: string;
  lastValidBlockHeight: string;
  base64: string;
}>;

function message(error: unknown): string {
  return error instanceof Error ? error.message : 'The operation failed without a usable refusal reason.';
}

function canonical(value: string, field: string): string {
  const parsed = new PublicKey(value);
  if (parsed.toBase58() !== value) throw new Error(`${field} must be canonical base58 text`);
  return value;
}

function quantity(value: string): bigint {
  if (!/^[1-9][0-9]*$/.test(value)) throw new Error('quantity must be a positive canonical integer');
  const parsed = BigInt(value);
  if (parsed > 18_446_744_073_709_551_615n) throw new Error('quantity exceeds u64');
  return parsed;
}

function byte(value: string, field: string): number {
  if (!/^(0|[1-9][0-9]*)$/.test(value)) throw new Error(`${field} must be a canonical byte`);
  const parsed = Number(value);
  if (!Number.isInteger(parsed) || parsed > 255) throw new Error(`${field} exceeds one byte`);
  return parsed;
}

function equal(left: Uint8Array, right: Uint8Array): boolean {
  return left.length === right.length && left.every((value, index) => value === right[index]);
}

function hex(bytes: Uint8Array): string {
  return Array.from(bytes, (value) => value.toString(16).padStart(2, '0')).join('');
}

function base64(bytes: Uint8Array): string {
  let binary = '';
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary);
}

function compact(address: string): string {
  return `${address.slice(0, 6)}…${address.slice(-6)}`;
}

function accountMap(addresses: ReadonlyArray<string>, accounts: ReadonlyArray<Readonly<{ address: string; account: RpcAccount | null }>>): Map<string, RpcAccount | null> {
  if (addresses.length !== accounts.length) throw new Error('RPC account vector changed width');
  return new Map(accounts.map((entry) => [entry.address, entry.account]));
}

function required(accounts: Map<string, RpcAccount | null>, address: string, field: string): RpcAccount {
  const account = accounts.get(address);
  if (account === undefined || account === null) throw new Error(`${field} is absent at the finalized construction floor`);
  return account;
}

function StateCard({ observation }: Readonly<{ observation: EconomicProjectionObservationV1 }>) {
  const state = observation.projection.state;
  return <article className="registered-state-card">
    <span className="eyebrow">{state.phase} · revision {observation.projection.revision.toString()}</span>
    <h3>{compact(observation.address)}</h3>
    <dl className="registered-facts">
      <div><dt>Outcomes</dt><dd>{state.outcomeCount}</dd></div><div><dt>Hoard principal</dt><dd>{state.hoard.toString()}</dd></div>
      <div><dt>Source holder</dt><dd>{compact(observation.projection.sourceHolder)}</dd></div><div><dt>Destination holder</dt><dd>{compact(observation.projection.destinationHolder)}</dd></div>
      <div><dt>Conservative supply</dt><dd>{state.supply.map(String).join(' · ')}</dd></div><div><dt>Native / materialized</dt><dd>{state.nativeSupply.map(String).join(' · ')} / {state.materializedSupply.map(String).join(' · ')}</dd></div>
    </dl>
  </article>;
}

export default function EconomicWorkspace() {
  const [endpoint, setEndpoint] = useState('http://127.0.0.1:8899');
  const [economicProgram, setEconomicProgram] = useState('');
  const [discovery, setDiscovery] = useState<Discovery>({ kind: 'idle' });
  const [projectionAddress, setProjectionAddress] = useState('');
  const [releaseSet, setReleaseSet] = useState('');
  const [authority, setAuthority] = useState('');
  const [payer, setPayer] = useState('');
  const [action, setAction] = useState<EconomicAction>('split');
  const [holder, setHolder] = useState<EconomicHolder>('source');
  const [representation, setRepresentation] = useState<'native' | 'materialized'>('native');
  const [outcome, setOutcome] = useState('0');
  const [amount, setAmount] = useState('1');
  const [holderToken, setHolderToken] = useState('');
  const [operationStatus, setOperationStatus] = useState('');
  const [operationArtifact, setOperationArtifact] = useState<OperationArtifact | null>(null);
  const [vacancyAddress, setVacancyAddress] = useState('');
  const [foundingReleaseSet, setFoundingReleaseSet] = useState('');
  const [foundingAuthority, setFoundingAuthority] = useState('');
  const [foundingPayer, setFoundingPayer] = useState('');
  const [foundingMarket, setFoundingMarket] = useState('');
  const [foundingRealm, setFoundingRealm] = useState('');
  const [sourceHolder, setSourceHolder] = useState('');
  const [destinationHolder, setDestinationHolder] = useState('');
  const [hoardAccount, setHoardAccount] = useState('');
  const [foundingStatus, setFoundingStatus] = useState('');
  const [foundingArtifact, setFoundingArtifact] = useState<FoundingArtifact | null>(null);

  const founded = useMemo(() => discovery.kind === 'ready' ? discovery.snapshot.founded : [], [discovery]);
  const vacant = useMemo(() => discovery.kind === 'ready' ? discovery.snapshot.vacant : [], [discovery]);

  async function discover(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setDiscovery({ kind: 'loading', message: 'Reading the selected program’s finalized 1,136-byte projection accounts…' });
    setOperationArtifact(null);
    setFoundingArtifact(null);
    try {
      canonical(economicProgram, 'economic program');
      const client = new SolanaRpcClient(endpoint);
      const [facts, snapshot] = await Promise.all([client.probe(), scanEconomicProjections(client, economicProgram)]);
      setProjectionAddress(snapshot.founded[0]?.address ?? '');
      setVacancyAddress(snapshot.vacant[0]?.address ?? '');
      setDiscovery({ kind: 'ready', facts, snapshot });
    } catch (error) {
      setDiscovery({ kind: 'error', message: message(error) });
    }
  }

  async function buildOperation(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setOperationArtifact(null);
    setOperationStatus('Reacquiring projection, release authority, payer, programs, custody accounts, and blockhash…');
    try {
      if (discovery.kind !== 'ready') throw new Error('run finalized economic discovery first');
      const selected = founded.find((candidate) => candidate.address === projectionAddress);
      if (selected === undefined) throw new Error('selected projection is not in the finalized discovered set');
      canonical(releaseSet, 'release set'); canonical(authority, 'semantic authority'); canonical(payer, 'fee payer');
      const client = new SolanaRpcClient(endpoint);
      const floor = await client.finalizedSlot();
      const currentRead = await client.accountInfo(projectionAddress, floor);
      if (currentRead.account === null || currentRead.account.owner !== economicProgram || currentRead.account.executable || currentRead.account.data.length !== ECONOMIC_PROJECTION_BYTES) throw new Error('projection is absent or no longer an exact economic-program account');
      const currentProjection = decodeEconomicProjectionV1(currentRead.account.data);
      const observation = Object.freeze({ status: 'founded' as const, address: projectionAddress, observedSlot: currentRead.slot, lamports: currentRead.account.lamports, projection: currentProjection });
      const operation = Object.freeze({ action, holder, representation, outcome: byte(outcome, 'outcome'), quantity: quantity(amount), expectedRevision: currentProjection.revision });
      const preliminary = buildEconomicOperationTransaction({
        economicProgram, payer, recentBlockhash: PLACEHOLDER_BLOCKHASH, authority, projection: observation,
        releaseSet, operation, holderToken: holderToken === '' ? undefined : canonical(holderToken, 'holder token account'),
      });
      const addresses = [...new Set([economicProgram, payer, ...preliminary.instruction.keys.map((meta) => meta.pubkey.toBase58())])];
      const reacquired = await client.multipleAccounts(addresses, currentRead.slot);
      const accounts = accountMap(addresses, reacquired.accounts);
      if (!required(accounts, economicProgram, 'economic program').executable) throw new Error('economic program is not executable');
      const payerAccount = required(accounts, payer, 'fee payer');
      if (payerAccount.owner !== SYSTEM_PROGRAM || payerAccount.executable) throw new Error('fee payer is not a funded system account');
      const exactProjectionAccount = required(accounts, projectionAddress, 'economic projection');
      if (exactProjectionAccount.owner !== economicProgram || exactProjectionAccount.executable || !equal(exactProjectionAccount.data, currentRead.account.data)) throw new Error('projection changed during one-floor reacquisition');
      const releaseAccount = required(accounts, releaseSet, 'execution release set');
      const release = await decodeExecutionReleaseSetV1(releaseAccount);
      authenticateEconomicRelease(release, economicProgram);
      if (!equal(release.digest, currentProjection.releaseSetId)) throw new Error('projection does not bind this exact execution release set');
      const role = preliminary.simulation.admissionRole;
      const authorityAccount = required(accounts, authority, `${role} authority`);
      if (authorityAccount.owner !== release.roles[role].program || authorityAccount.executable) throw new Error(`semantic authority is not owned by the selected ${role} program`);
      let custody = 'none · claim-only operation';
      let tokenBalances = 'not applicable';
      if (preliminary.simulation.custody !== null) {
        const holderIdentity = preliminary.simulation.custody.source === 'hoard' ? preliminary.simulation.custody.destination : preliminary.simulation.custody.source;
        if (holderIdentity === 'hoard') throw new Error('custody route omitted a concrete holder');
        const tokenAddress = canonical(holderToken, 'holder token account');
        const tokenProgramAccount = required(accounts, LEGACY_TOKEN_PROGRAM_ID.toBase58(), 'legacy Token Program');
        if (!tokenProgramAccount.executable) throw new Error('legacy Token Program is not executable');
        const route = inspectEconomicTokenRoute({
          projectionAddress, economicProgram, projection: currentProjection, holder: holderIdentity,
          holderToken: required(accounts, tokenAddress, 'holder token account'),
          hoardToken: required(accounts, currentProjection.hoardAccount, 'Hoard token account'),
          mint: required(accounts, currentProjection.collateralMint, 'collateral Mint'),
          custody: preliminary.simulation.custody,
        });
        custody = `${preliminary.simulation.custody.amount} atoms · ${preliminary.simulation.custody.source} → ${preliminary.simulation.custody.destination}`;
        tokenBalances = `holder ${route.holderBefore} → ${route.holderAfter}; Hoard ${route.hoardBefore} → ${route.hoardAfter}; decimals ${route.decimals}`;
      }
      const blockhash = await client.latestBlockhash(reacquired.slot);
      const plan = buildEconomicOperationTransaction({ economicProgram, payer, recentBlockhash: blockhash.blockhash, authority, projection: observation, releaseSet, operation, holderToken: holderToken || undefined });
      setOperationArtifact(Object.freeze({
        action, projection: projectionAddress, revision: currentProjection.revision.toString(), nextRevision: (currentProjection.revision + 1n).toString(),
        phase: `${currentProjection.state.phase} → ${plan.simulation.nextState.phase}`, releaseRole: role,
        claimEffects: Object.freeze(plan.simulation.claims.map((effect) => `${effect.operation} ${effect.amount} · ${effect.holder} outcome ${effect.outcome}`)),
        custody, tokenBalances, wireBytes: plan.wireBytes.length, signers: plan.requiredSignerKeys,
        blockhashSlot: blockhash.slot, lastValidBlockHeight: blockhash.lastValidBlockHeight, base64: base64(plan.wireBytes),
      }));
      setOperationStatus('Exact unsigned economic transaction constructed. No claim, custody, signature, or submission has occurred.');
    } catch (error) {
      setOperationStatus(`Refused: ${message(error)}`);
    }
  }

  async function buildFounding(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setFoundingArtifact(null);
    setFoundingStatus('Reacquiring the vacant projection, Market/Realm, release roles, Hoard, payer, rent, and blockhash…');
    try {
      if (discovery.kind !== 'ready') throw new Error('run finalized economic discovery first');
      const selected = vacant.find((candidate) => candidate.address === vacancyAddress);
      if (selected === undefined) throw new Error('selected projection is not a discovered exact vacant account');
      canonical(foundingReleaseSet, 'release set'); canonical(foundingAuthority, 'Core authority'); canonical(foundingPayer, 'fee payer');
      canonical(foundingMarket, 'Market'); canonical(foundingRealm, 'Realm'); canonical(sourceHolder, 'source holder');
      canonical(destinationHolder, 'destination holder'); canonical(hoardAccount, 'Hoard token account');
      const client = new SolanaRpcClient(endpoint);
      const floor = await client.finalizedSlot();
      const firstAddresses = [vacancyAddress, foundingReleaseSet, foundingMarket, foundingRealm];
      if (new Set(firstAddresses).size !== firstAddresses.length) throw new Error('founding aliases projection, release, Market, or Realm roles');
      const first = await client.multipleAccounts(firstAddresses, floor);
      const firstMap = accountMap(firstAddresses, first.accounts);
      const projectionAccount = required(firstMap, vacancyAddress, 'vacant projection');
      if (projectionAccount.owner !== economicProgram || projectionAccount.executable || projectionAccount.data.length !== ECONOMIC_PROJECTION_BYTES || projectionAccount.data.some((value) => value !== 0)) throw new Error('founding requires one preallocated, program-owned, wholly zero 1,136-byte projection');
      const releaseAccount = required(firstMap, foundingReleaseSet, 'execution release set');
      const release = await decodeExecutionReleaseSetV1(releaseAccount);
      authenticateEconomicRelease(release, economicProgram);
      const marketAccount = required(firstMap, foundingMarket, 'Market');
      const realmAccount = required(firstMap, foundingRealm, 'Realm');
      const coordinates = await deriveEconomicFoundingCoordinates(release.roles.core.program, foundingMarket, marketAccount, foundingRealm, realmAccount, first.slot);
      if (coordinates.tokenProgram !== LEGACY_TOKEN_PROGRAM_ID.toBase58()) throw new Error('physical economic founding supports only a Realm selecting the exact legacy Token Program');
      const vacancy: EconomicVacancyObservationV1 = Object.freeze({ status: 'vacant', address: vacancyAddress, observedSlot: first.slot, lamports: projectionAccount.lamports });
      const preliminary = buildEconomicFoundingTransaction({
        economicProgram, payer: foundingPayer, recentBlockhash: PLACEHOLDER_BLOCKHASH, authority: foundingAuthority, projection: vacancy,
        releaseSet: foundingReleaseSet, coordinates, releaseSetId: release.digest, sourceHolder, destinationHolder, hoardAccount,
      });
      const [hoardAuthority] = PublicKey.findProgramAddressSync([ECONOMIC_HOARD_SEED, new PublicKey(vacancyAddress).toBytes()], new PublicKey(economicProgram));
      const addresses = [...new Set([
        economicProgram, foundingPayer, ...preliminary.instruction.keys.map((meta) => meta.pubkey.toBase58()), foundingMarket, foundingRealm,
        coordinates.collateralMint, hoardAccount, hoardAuthority.toBase58(), LEGACY_TOKEN_PROGRAM_ID.toBase58(),
      ])];
      const reacquired = await client.multipleAccounts(addresses, first.slot);
      const accounts = accountMap(addresses, reacquired.accounts);
      if (!required(accounts, economicProgram, 'economic program').executable || !required(accounts, LEGACY_TOKEN_PROGRAM_ID.toBase58(), 'legacy Token Program').executable) throw new Error('one physical execution program is not executable');
      const payerAccount = required(accounts, foundingPayer, 'fee payer');
      if (payerAccount.owner !== SYSTEM_PROGRAM || payerAccount.executable) throw new Error('fee payer is not a funded system account');
      const authorityAccount = required(accounts, foundingAuthority, 'Core authority');
      if (authorityAccount.owner !== release.roles.core.program || authorityAccount.executable) throw new Error('founding authority is not owned by the selected Core program');
      const exactVacancy = required(accounts, vacancyAddress, 'vacant projection');
      if (exactVacancy.owner !== economicProgram || exactVacancy.executable || !equal(exactVacancy.data, projectionAccount.data)) throw new Error('vacant projection changed during one-floor reacquisition');
      const exactRelease = required(accounts, foundingReleaseSet, 'execution release set');
      const currentRelease = await decodeExecutionReleaseSetV1(exactRelease);
      authenticateEconomicRelease(currentRelease, economicProgram);
      if (!equal(currentRelease.digest, release.digest)) throw new Error('execution release set changed during founding construction');
      if (!equal(required(accounts, foundingMarket, 'Market').data, marketAccount.data) || !equal(required(accounts, foundingRealm, 'Realm').data, realmAccount.data)) throw new Error('Market or Realm changed during founding construction');
      const mintAccount = required(accounts, coordinates.collateralMint, 'collateral Mint');
      if (mintAccount.owner !== LEGACY_TOKEN_PROGRAM_ID.toBase58() || mintAccount.executable || mintAccount.data.length !== 82 || mintAccount.data[45] !== 1) throw new Error('Realm collateral is not one initialized exact legacy Mint');
      const hoard = decodeLegacyTokenObservationV1(required(accounts, hoardAccount, 'Hoard token account'));
      if (hoard.mint !== coordinates.collateralMint || hoard.owner !== hoardAuthority.toBase58() || hoard.amount !== 0n || hoard.frozen) throw new Error('Hoard token account is not the empty unfrozen projection-PDA custody account');
      const rent = await client.minimumBalanceForRentExemption(ECONOMIC_PROJECTION_BYTES);
      if (BigInt(exactVacancy.lamports) < BigInt(rent.lamports)) throw new Error(`preallocated projection has ${exactVacancy.lamports} lamports, below exact rent exemption ${rent.lamports}`);
      const blockhash = await client.latestBlockhash(reacquired.slot);
      const plan = buildEconomicFoundingTransaction({ economicProgram, payer: foundingPayer, recentBlockhash: blockhash.blockhash, authority: foundingAuthority, projection: vacancy, releaseSet: foundingReleaseSet, coordinates, releaseSetId: release.digest, sourceHolder, destinationHolder, hoardAccount });
      setFoundingArtifact(Object.freeze({
        projection: vacancyAddress, marketId: hex(coordinates.marketId), releaseId: hex(release.digest), outcomeCount: coordinates.outcomeCount,
        collateralMint: coordinates.collateralMint, hoard: hoardAccount, projectionLamports: exactVacancy.lamports, minimumRent: rent.lamports,
        wireBytes: plan.wireBytes.length, signers: plan.requiredSignerKeys, blockhashSlot: blockhash.slot,
        lastValidBlockHeight: blockhash.lastValidBlockHeight, base64: base64(plan.wireBytes),
      }));
      setFoundingStatus('Exact unsigned founding transaction constructed. The browser did not allocate, fund, sign, or submit anything.');
    } catch (error) {
      setFoundingStatus(`Refused: ${message(error)}`);
    }
  }

  return <main className="product-shell direct-workspace">
    <header className="product-nav"><Link className="brand" href="/">dClutch</Link><nav><Link href="/direct">Direct</Link><Link className="active" href="/economic">Economic</Link><Link href="/explorer">Explorer</Link></nav><span className="preview-control"><i className="preview-dot" />unsigned operator</span></header>
    <section className="market-heading"><div><div className="market-kicker"><span>physical custody</span><span>finalized RPC</span><span>no wallet</span></div><h1>Conservative claims, tied to real collateral.</h1><p>Found a projection or construct split, merge, representation conversion, and redemption transactions from exact chain state. This page never invents a market and never signs or submits.</p></div></section>
    <section className="direct-card" aria-labelledby="economic-discovery"><div className="direct-card-heading"><span>01</span><div><h2 id="economic-discovery">Discover the economic program</h2><p>Only exact program-owned 1,136-byte accounts are reacquired. Wholly zero accounts are separately reported as founding vacancies.</p></div></div>
      <form className="direct-form-grid" onSubmit={discover}><label><span>Finalized RPC endpoint</span><input value={endpoint} onChange={(event) => setEndpoint(event.target.value.trim())} /></label><label><span>Economic adapter program</span><input required value={economicProgram} onChange={(event) => setEconomicProgram(event.target.value.trim())} /></label><button type="submit" disabled={discovery.kind === 'loading'}>{discovery.kind === 'loading' ? 'Reading finalized state…' : 'Connect & discover projections'}</button></form>
      <p className="direct-status" aria-live="polite">{discovery.kind === 'ready' ? `${founded.length} founded · ${vacant.length} vacant · ${discovery.snapshot.refused.length} refused at slot ${discovery.snapshot.scanSlot} · genesis ${compact(discovery.facts.genesisHash)}` : discovery.message ?? 'No RPC request has been made.'}</p>
      {discovery.kind === 'ready' && founded.length > 0 && <div className="registered-state-grid">{founded.map((entry) => <StateCard key={entry.address} observation={entry} />)}</div>}
      {discovery.kind === 'ready' && founded.length === 0 && <p className="direct-refusal">No canonical founded projection exists. This workspace does not synthesize one.</p>}
      {discovery.kind === 'ready' && discovery.snapshot.refused.length > 0 && <details className="registered-refusals"><summary>{discovery.snapshot.refused.length} candidate(s) refused</summary>{discovery.snapshot.refused.map((entry) => <p key={entry.address}>{compact(entry.address)} · {entry.reason}</p>)}</details>}
    </section>
    <form className="direct-card" onSubmit={buildOperation} aria-labelledby="economic-operation"><div className="direct-card-heading"><span>02</span><div><h2 id="economic-operation">Operate one founded projection</h2><p>Every build authenticates the exact release digest and role-owned authority, advances one persisted revision, and shows all conservative claim and SPL custody effects before external signing.</p></div></div>
      <div className="direct-form-grid"><label><span>Founded projection</span><select value={projectionAddress} onChange={(event) => setProjectionAddress(event.target.value)}>{founded.map((entry) => <option key={entry.address} value={entry.address}>{compact(entry.address)} · r{entry.projection.revision.toString()}</option>)}</select></label><label><span>Execution release set</span><input required value={releaseSet} onChange={(event) => setReleaseSet(event.target.value.trim())} /></label><label><span>Trading / resolution authority signer</span><input required value={authority} onChange={(event) => setAuthority(event.target.value.trim())} /></label><label><span>Fee payer signer</span><input required value={payer} onChange={(event) => setPayer(event.target.value.trim())} /></label><label><span>Operation</span><select value={action} onChange={(event) => setAction(event.target.value as EconomicAction)}><option value="split">Split complete set</option><option value="merge">Merge complete set</option><option value="materialize">Materialize source → destination</option><option value="dematerialize">Dematerialize source → destination</option><option value="redeem">Redeem terminal claim</option></select></label><label><span>Holder</span><select value={holder} onChange={(event) => setHolder(event.target.value as EconomicHolder)}><option value="source">Source</option><option value="destination">Destination</option></select></label><label><span>Representation</span><select value={representation} onChange={(event) => setRepresentation(event.target.value as 'native' | 'materialized')}><option value="native">Native</option><option value="materialized">Materialized</option></select></label><label><span>Outcome index</span><input inputMode="numeric" value={outcome} onChange={(event) => setOutcome(event.target.value)} /></label><label><span>Quantity · collateral atoms</span><input inputMode="numeric" value={amount} onChange={(event) => setAmount(event.target.value)} /></label><label><span>Holder legacy-token account · custody routes</span><input value={holderToken} onChange={(event) => setHolderToken(event.target.value.trim())} /></label></div>
      <div className="registered-facts creation-boundary"><p><strong>Conservation</strong> Split/merge affect every outcome. Representation conversion keeps total supply fixed. Redemption burns only the named claim.</p><p><strong>Custody</strong> SPL transfers occur only for split, merge, or winning redemption. Losing redemption is claim-only.</p><p><strong>Release authority</strong> Trading admits split/merge/conversion; Resolution admits redemption. Same-width substitute releases are refused.</p></div>
      <button type="submit" disabled={founded.length === 0}>Reacquire & build unsigned operation</button><p className="direct-status" aria-live="polite">{founded.length === 0 ? 'No founded economic projection is available.' : operationStatus || 'No operation state, token account, or blockhash has been read.'}</p>
      {operationArtifact && <div className="direct-output"><dl><div><dt>Action / projection</dt><dd>{operationArtifact.action} · {operationArtifact.projection}</dd></div><div><dt>Revision / phase</dt><dd>{operationArtifact.revision} → {operationArtifact.nextRevision} · {operationArtifact.phase}</dd></div><div><dt>Authenticated role</dt><dd>{operationArtifact.releaseRole}</dd></div><div><dt>Claim effects</dt><dd>{operationArtifact.claimEffects.join(' | ')}</dd></div><div><dt>Custody effect</dt><dd>{operationArtifact.custody}</dd></div><div><dt>Token balances</dt><dd>{operationArtifact.tokenBalances}</dd></div><div><dt>Wire profile</dt><dd>{operationArtifact.wireBytes} / 1232 bytes</dd></div><div><dt>Required external signers</dt><dd>{operationArtifact.signers.join(' · ')}</dd></div><div><dt>Blockhash lifetime</dt><dd>slot {operationArtifact.blockhashSlot} · height {operationArtifact.lastValidBlockHeight}</dd></div></dl><label><span>Unsigned v0 transaction · base64</span><textarea readOnly value={operationArtifact.base64} /></label><p className="direct-refusal">All signature slots remain zero. A separate wallet boundary must inspect, sign, and submit this exact artifact.</p></div>}
    </form>
    <form className="direct-card" onSubmit={buildFounding} aria-labelledby="economic-founding"><div className="direct-card-heading"><span>03</span><div><h2 id="economic-founding">Found one preallocated projection</h2><p>Founding derives market identity, width, Realm token program, and collateral Mint from binding-clean finalized Core state. The current ABI deliberately does not allocate or fund projection accounts.</p></div></div>
      <div className="direct-form-grid"><label><span>Vacant projection</span><select value={vacancyAddress} onChange={(event) => setVacancyAddress(event.target.value)}>{vacant.map((entry) => <option key={entry.address} value={entry.address}>{compact(entry.address)} · {entry.lamports} lamports</option>)}</select></label><label><span>Execution release set</span><input required value={foundingReleaseSet} onChange={(event) => setFoundingReleaseSet(event.target.value.trim())} /></label><label><span>Core authority signer</span><input required value={foundingAuthority} onChange={(event) => setFoundingAuthority(event.target.value.trim())} /></label><label><span>Fee payer signer</span><input required value={foundingPayer} onChange={(event) => setFoundingPayer(event.target.value.trim())} /></label><label><span>Open Market account</span><input required value={foundingMarket} onChange={(event) => setFoundingMarket(event.target.value.trim())} /></label><label><span>Market Realm account</span><input required value={foundingRealm} onChange={(event) => setFoundingRealm(event.target.value.trim())} /></label><label><span>Source holder</span><input required value={sourceHolder} onChange={(event) => setSourceHolder(event.target.value.trim())} /></label><label><span>Destination holder</span><input required value={destinationHolder} onChange={(event) => setDestinationHolder(event.target.value.trim())} /></label><label><span>Empty Hoard token account</span><input required value={hoardAccount} onChange={(event) => setHoardAccount(event.target.value.trim())} /></label></div>
      <div className="registered-facts creation-boundary"><p><strong>Preallocation</strong> The projection must already be program-owned, wholly zero, exactly 1,136 bytes, and rent exempt. No hidden System allocation is inserted.</p><p><strong>Market derivation</strong> Market identity is SHA-256 of canonical Market identity bytes; Realm selects the Mint and exact legacy Token Program.</p><p><strong>External boundary</strong> Core authority and fee-payer signatures remain external. This page never creates, funds, signs, or submits.</p></div>
      <button type="submit" disabled={vacant.length === 0}>Reacquire & build unsigned founding</button><p className="direct-status" aria-live="polite">{vacant.length === 0 ? 'No exact preallocated vacant projection is available.' : foundingStatus || 'No Market, Realm, release, rent, or Hoard state has been read.'}</p>
      {foundingArtifact && <div className="direct-output"><dl><div><dt>Projection</dt><dd>{foundingArtifact.projection}</dd></div><div><dt>Market identity</dt><dd>{foundingArtifact.marketId}</dd></div><div><dt>Execution release</dt><dd>{foundingArtifact.releaseId}</dd></div><div><dt>Outcome width</dt><dd>{foundingArtifact.outcomeCount}</dd></div><div><dt>Collateral / Hoard</dt><dd>{foundingArtifact.collateralMint} / {foundingArtifact.hoard}</dd></div><div><dt>Preallocated rent</dt><dd>{foundingArtifact.projectionLamports} observed · {foundingArtifact.minimumRent} minimum</dd></div><div><dt>Wire profile</dt><dd>{foundingArtifact.wireBytes} / 1232 bytes</dd></div><div><dt>Required external signers</dt><dd>{foundingArtifact.signers.join(' · ')}</dd></div><div><dt>Blockhash lifetime</dt><dd>slot {foundingArtifact.blockhashSlot} · height {foundingArtifact.lastValidBlockHeight}</dd></div></dl><label><span>Unsigned v0 transaction · base64</span><textarea readOnly value={foundingArtifact.base64} /></label><p className="direct-refusal">The artifact contains zero signatures. Projection allocation and wallet submission are explicitly outside this ABI and page.</p></div>}
    </form>
  </main>;
}
