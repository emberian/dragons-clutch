'use client';

import { PublicKey } from '@solana/web3.js';
import { FormEvent, useMemo, useState } from 'react';

import { inspectDirectFeePolicy } from '@/lib/directChain';
import { CLAIM_PROGRAM_ID } from '@/lib/directTransaction';
import {
  LEGACY_TOKEN_PROGRAM_ID,
  REPLAY_STATE_BYTES,
  buildRegisteredCreateTransaction,
  buildRegisteredFillTransaction,
  buildRegisteredTerminalTransaction,
  decodeLegacyTokenObservationV1,
  decodeMakerReplayObservationV1,
  deriveRegisteredCreateAddresses,
  encodeRegisteredIntentStateV1,
  observeRegisteredDirectState,
  projectRegisteredDirectState,
  registeredPhase,
  scanRegisteredDirectStates,
  type RegisteredDirectSnapshot,
  type RegisteredDirectStateObservation,
  type RegisteredFillRouteV1,
} from '@/lib/registeredDirect';
import { SolanaRpcClient } from '@/lib/rpc';

type Props = Readonly<{
  endpoint: string;
  protocolProgram: string;
  controllerProgram: string;
  scanSlot: string;
  markets: ReadonlyArray<RegisteredCreateMarketChoiceV1>;
}>;

export type RegisteredCreateMarketChoiceV1 = Readonly<{
  address: string;
  generation: string;
  outcomeCount: number;
  realm: string;
  mint: string;
  tokenProgram: string;
}>;

type ScanState = Readonly<{ kind: 'idle' | 'loading' | 'error'; message?: string }> | Readonly<{ kind: 'ready'; snapshot: RegisteredDirectSnapshot }>;
type Artifact = Readonly<{ action: string; base64: string; wireBytes: number; signers: ReadonlyArray<string>; blockhashSlot: string; lastValidBlockHeight: string }>;
type CreateArtifact = Artifact & Readonly<{
  nonce: string;
  replay: string;
  registration: string;
  rentLamports: string;
  payerLamports: string;
  approval: string;
  delegation: string;
}>;

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : 'The registered workflow failed without a usable error message.';
}

function canonicalUnsigned(value: string, field: string): bigint {
  if (!/^(0|[1-9][0-9]*)$/.test(value)) throw new Error(`${field} must be canonical unsigned integer text`);
  const parsed = BigInt(value);
  if (parsed > 18_446_744_073_709_551_615n) throw new Error(`${field} exceeds u64`);
  return parsed;
}

function base64(bytes: Uint8Array): string {
  let binary = '';
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary);
}

function routePlaceholder(): string {
  return JSON.stringify({
    journal: '', realm: '', feePolicy: '', capabilityManifest: '', mint: '', source: '',
    sellerDestination: '', feeDestination: '', tokenProgram: '',
  }, null, 2);
}

function compact(value: string, edge = 8): string {
  return value.length <= edge * 2 + 1 ? value : `${value.slice(0, edge)}…${value.slice(-edge)}`;
}

function StateCard({ observation }: Readonly<{ observation: RegisteredDirectStateObservation }>) {
  const state = observation.state;
  return <article className="registered-state-card">
    <div className="card-topline"><p className="account-kind">{state.intent.side === 0 ? 'Seller' : 'Buyer'} · {registeredPhase(state.phase)}</p><span className={`status-chip ${state.phase === 0 ? 'pass' : 'caution'}`}>sequence {state.sequence.toString()}</span></div>
    <h3 title={observation.address}>{compact(observation.address, 10)}</h3>
    <p className="observation">Finalized slot {observation.observedSlot} · {observation.lamports} lamports</p>
    <dl className="registered-facts">
      <div><dt>Maker</dt><dd title={state.maker}>{compact(state.maker)}</dd></div>
      <div><dt>Market</dt><dd title={new PublicKey(state.intent.market).toBase58()}>{compact(new PublicKey(state.intent.market).toBase58())}</dd></div>
      <div><dt>Outcome</dt><dd>{state.intent.outcome}</dd></div>
      <div><dt>Remaining</dt><dd>{state.remaining.toString()} / {state.intent.maximumFill.toString()}</dd></div>
      <div><dt>Limit · 1e6</dt><dd>{state.intent.limitPrice.toString()}</dd></div>
      <div><dt>Valid through</dt><dd>slot {state.intent.validThrough.toString()}</dd></div>
    </dl>
  </article>;
}

export default function RegisteredOrdersWorkspace({ endpoint, protocolProgram, controllerProgram, scanSlot, markets }: Props) {
  const [createMarket, setCreateMarket] = useState(markets[0]?.address ?? '');
  const [createMaker, setCreateMaker] = useState('');
  const [createPayer, setCreatePayer] = useState('');
  const [createCollateral, setCreateCollateral] = useState('');
  const [createFeePolicy, setCreateFeePolicy] = useState('');
  const [createManifest, setCreateManifest] = useState('');
  const [createSide, setCreateSide] = useState('1');
  const [createOutcome, setCreateOutcome] = useState('0');
  const [createValidFrom, setCreateValidFrom] = useState('0');
  const [createValidThrough, setCreateValidThrough] = useState('18446744073709551615');
  const [createMaximum, setCreateMaximum] = useState('0');
  const [createLimit, setCreateLimit] = useState('0');
  const [createStatus, setCreateStatus] = useState('');
  const [createArtifact, setCreateArtifact] = useState<CreateArtifact | null>(null);
  const [scan, setScan] = useState<ScanState>({ kind: 'idle' });
  const [sellerAddress, setSellerAddress] = useState('');
  const [buyerAddress, setBuyerAddress] = useState('');
  const [terminalAddress, setTerminalAddress] = useState('');
  const [payer, setPayer] = useState('');
  const [fill, setFill] = useState('0');
  const [executionPrice, setExecutionPrice] = useState('0');
  const [routeText, setRouteText] = useState(routePlaceholder);
  const [actionStatus, setActionStatus] = useState('');
  const [artifact, setArtifact] = useState<Artifact | null>(null);

  const states = useMemo(() => scan.kind === 'ready' ? scan.snapshot.states : [], [scan]);
  const openStates = useMemo(() => states.filter((state) => state.state.phase === 0), [states]);
  const sellers = useMemo(() => openStates.filter((state) => state.state.intent.side === 0), [openStates]);
  const buyers = useMemo(() => openStates.filter((state) => state.state.intent.side === 1), [openStates]);

  async function buildCreate(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setCreateArtifact(null);
    setCreateStatus('Reacquiring the replay nonce, authority route, token state, payer funding, rent, and blockhash…');
    try {
      const market = markets.find((candidate) => candidate.address === createMarket);
      if (market === undefined) throw new Error('selected Market is not in the finalized binding-clean scan');
      const maker = new PublicKey(createMaker);
      const payerKey = new PublicKey(createPayer);
      const collateral = new PublicKey(createCollateral);
      if (maker.toBase58() !== createMaker || payerKey.toBase58() !== createPayer || collateral.toBase58() !== createCollateral) throw new Error('maker, payer, and collateral must be canonical base58 text');
      const client = new SolanaRpcClient(endpoint);
      const policy = await inspectDirectFeePolicy(client, protocolProgram, createFeePolicy, scanSlot);
      const generation = BigInt(market.generation);
      const initial = deriveRegisteredCreateAddresses(controllerProgram, market.address, generation, createMaker, 0n);
      const replayRead = await client.accountInfo(initial.replay.toBase58(), policy.observedSlot);
      const replay = decodeMakerReplayObservationV1(replayRead.account, initial.controller);
      const derived = deriveRegisteredCreateAddresses(controllerProgram, market.address, generation, createMaker, replay.nextNonce);
      if (!derived.replay.equals(initial.replay)) throw new Error('replay derivation changed across nonce projection');
      const side = Number(canonicalUnsigned(createSide, 'side'));
      const outcome = Number(canonicalUnsigned(createOutcome, 'outcome'));
      if (side > 1) throw new Error('side must be seller 0 or buyer 1');
      if (outcome >= market.outcomeCount) throw new Error(`outcome must be below the chain-derived width ${market.outcomeCount}`);
      const intent = {
        side, outcome, lifecycle: 2, market: new PublicKey(market.address).toBytes(), generation,
        nonce: replay.nextNonce, validFrom: canonicalUnsigned(createValidFrom, 'valid-from slot'),
        validThrough: canonicalUnsigned(createValidThrough, 'valid-through slot'),
        maximumFill: canonicalUnsigned(createMaximum, 'maximum fill'), limitPrice: canonicalUnsigned(createLimit, 'limit price'),
        feeBasisPoints: policy.feeBasisPoints, collateralAccount: collateral.toBytes(),
      };
      const route = {
        realm: market.realm, feePolicy: policy.address, capabilityManifest: createManifest,
        mint: market.mint, collateral: createCollateral, venue: policy.recipient, tokenProgram: market.tokenProgram,
      };
      const preliminary = buildRegisteredCreateTransaction({
        controllerProgram, payer: createPayer, maker: createMaker, market: market.address,
        recentBlockhash: '11111111111111111111111111111111', intent, expectedNonce: replay.nextNonce, route,
      });
      const addresses = [...new Set([
        controllerProgram, preliminary.derived.controller.toBase58(), createMaker, createPayer,
        preliminary.derived.replay.toBase58(), preliminary.derived.registration.toBase58(), CLAIM_PROGRAM_ID.toBase58(),
        '11111111111111111111111111111111', market.address, market.realm, policy.address, createManifest,
        market.mint, createCollateral, policy.recipient, market.tokenProgram,
      ])];
      const observation = await client.multipleAccounts(addresses, replayRead.slot);
      const accounts = new Map(observation.accounts.map((entry) => [entry.address, entry.account]));
      const requiredAccount = (address: string, field: string) => {
        const account = accounts.get(address);
        if (account === undefined || account === null) throw new Error(`${field} is absent at the finalized construction floor`);
        return account;
      };
      const programAccount = requiredAccount(controllerProgram, 'controller program');
      const claimAccount = requiredAccount(CLAIM_PROGRAM_ID.toBase58(), 'claim program');
      const systemAccount = requiredAccount('11111111111111111111111111111111', 'system program');
      const tokenProgramAccount = requiredAccount(market.tokenProgram, 'token program');
      if (!programAccount.executable || !claimAccount.executable || !systemAccount.executable || !tokenProgramAccount.executable) throw new Error('one required program account is not executable');
      const marketAccount = requiredAccount(market.address, 'Market');
      const realmAccount = requiredAccount(market.realm, 'Realm');
      const policyAccount = requiredAccount(policy.address, 'fee policy');
      const manifestAccount = requiredAccount(createManifest, 'capability manifest');
      if ([marketAccount, realmAccount, policyAccount, manifestAccount].some((account) => account.owner !== protocolProgram || account.executable)) throw new Error('one Market authority account has the wrong protocol owner or executable flag');
      if (marketAccount.data.length < 201 || new TextDecoder().decode(marketAccount.data.slice(0, 8)) !== 'DCLTCAT1'
          || marketAccount.data[200] !== 1 || new DataView(marketAccount.data.buffer, marketAccount.data.byteOffset + 192, 8).getBigUint64(0, true) !== generation
          || marketAccount.data[10] !== market.outcomeCount) throw new Error('Market phase, generation, width, or exact layout changed after discovery');
      if (realmAccount.data.length !== 112 || new TextDecoder().decode(realmAccount.data.slice(0, 8)) !== 'DCLTRLM1'
          || new PublicKey(realmAccount.data.slice(16, 48)).toBase58() !== market.tokenProgram
          || new PublicKey(realmAccount.data.slice(48, 80)).toBase58() !== market.mint) throw new Error('Realm no longer selects the discovered token program and mint');
      if (market.tokenProgram !== LEGACY_TOKEN_PROGRAM_ID.toBase58()) throw new Error('this experimental registered controller requires the exact legacy-token Realm profile');
      if (requiredAccount(market.mint, 'collateral mint').owner !== market.tokenProgram) throw new Error('Realm collateral mint has the wrong token-program owner');
      const token = decodeLegacyTokenObservationV1(requiredAccount(createCollateral, 'collateral token account'));
      if (token.mint !== market.mint) throw new Error('collateral token account has the wrong Realm mint');
      if (side === 1 && token.owner !== createMaker) throw new Error('buyer collateral is not owned by the required maker signer');
      if (preliminary.approvalAmount !== null && token.amount < preliminary.approvalAmount) throw new Error(`buyer collateral ${token.amount} is below the exact ${preliminary.approvalAmount} worst-case reserve`);
      if (accounts.get(preliminary.derived.registration.toBase58()) !== null) throw new Error('next registration PDA already exists; replay nonce and state are inconsistent');
      const replayAgain = decodeMakerReplayObservationV1(accounts.get(preliminary.derived.replay.toBase58()) ?? null, preliminary.derived.controller);
      if (replayAgain.exists !== replay.exists || replayAgain.nextNonce !== replay.nextNonce) throw new Error('maker replay root changed while constructing the transaction');
      const payerAccount = requiredAccount(createPayer, 'payer');
      if (payerAccount.owner !== '11111111111111111111111111111111' || payerAccount.executable) throw new Error('payer is not a funded system account');
      const [replayRent, registrationRent] = await Promise.all([
        client.minimumBalanceForRentExemption(REPLAY_STATE_BYTES), client.minimumBalanceForRentExemption(232),
      ]);
      const requiredRent = BigInt(registrationRent.lamports) + (replay.exists ? 0n : BigInt(replayRent.lamports));
      if (BigInt(payerAccount.lamports) < requiredRent) throw new Error(`payer has ${payerAccount.lamports} lamports but account creation needs at least ${requiredRent}, excluding transaction fees`);
      const blockhash = await client.latestBlockhash(observation.slot);
      const plan = buildRegisteredCreateTransaction({
        controllerProgram, payer: createPayer, maker: createMaker, market: market.address,
        recentBlockhash: blockhash.blockhash, intent, expectedNonce: replay.nextNonce, route,
      });
      const oldDelegation = token.delegate === null ? 'none' : `${token.delegatedAmount} to ${token.delegate}`;
      setCreateArtifact({
        action: side === 1 ? 'Buyer registered creation' : 'Seller registered creation', base64: base64(plan.wireBytes),
        wireBytes: plan.wireBytes.length, signers: plan.requiredSignerKeys, blockhashSlot: blockhash.slot,
        lastValidBlockHeight: blockhash.lastValidBlockHeight, nonce: replay.nextNonce.toString(),
        replay: plan.derived.replay.toBase58(), registration: plan.derived.registration.toBase58(),
        rentLamports: requiredRent.toString(), payerLamports: payerAccount.lamports,
        approval: plan.approvalAmount === null ? 'not required for a seller destination' : `${plan.approvalAmount} collateral atoms`,
        delegation: plan.approvalAmount === null ? 'unchanged' : `${oldDelegation} → registration PDA`,
      });
      setCreateStatus('Exact unsigned creation transaction constructed. Maker/payer signatures remain external; nothing was signed or submitted.');
    } catch (error) {
      setCreateStatus(`Refused: ${errorMessage(error)}`);
    }
  }

  async function discover() {
    setScan({ kind: 'loading', message: 'Scanning the pinned claim owner and reacquiring 232-byte candidates…' });
    setArtifact(null);
    try {
      const snapshot = await scanRegisteredDirectStates(new SolanaRpcClient(endpoint), controllerProgram);
      setSellerAddress(snapshot.states.find((state) => state.state.phase === 0 && state.state.intent.side === 0)?.address ?? '');
      setBuyerAddress(snapshot.states.find((state) => state.state.phase === 0 && state.state.intent.side === 1)?.address ?? '');
      setTerminalAddress(snapshot.states.find((state) => state.state.phase === 0)?.address ?? '');
      setScan({ kind: 'ready', snapshot });
    } catch (error) {
      setScan({ kind: 'error', message: errorMessage(error) });
    }
  }

  async function validateFillRoute(
    client: SolanaRpcClient,
    plan: ReturnType<typeof buildRegisteredFillTransaction>,
    route: RegisteredFillRouteV1,
    seller: RegisteredDirectStateObservation,
    buyer: RegisteredDirectStateObservation,
    floor: string,
  ): Promise<string> {
    if (new Set(plan.instruction.keys.map((meta) => meta.pubkey.toBase58())).size !== 17) throw new Error('registered fill route aliases two exact account roles');
    const addresses = [controllerProgram, payer, ...plan.instruction.keys.map((meta) => meta.pubkey.toBase58())];
    if (new Set(addresses).size !== addresses.length) throw new Error('payer/program alias a registered fill account role');
    const observation = await client.multipleAccounts(addresses, floor);
    if (observation.accounts.some((entry) => entry.account === null)) throw new Error('one or more registered fill accounts are absent at the finalized construction floor');
    const [program, payerAccount, ...routeAccounts] = observation.accounts.map((entry) => entry.account!);
    if (!program.executable || payerAccount.executable) throw new Error('controller program is not executable or payer is executable');
    const accounts = routeAccounts;
    if (accounts[1].owner !== CLAIM_PROGRAM_ID.toBase58() || accounts[2].owner !== CLAIM_PROGRAM_ID.toBase58()
        || accounts[3].owner !== controllerProgram || accounts[4].owner !== CLAIM_PROGRAM_ID.toBase58() || accounts[5].owner !== CLAIM_PROGRAM_ID.toBase58()
        || !accounts[6].executable || !accounts[7].executable || accounts.slice(8, 12).some((account) => account.owner !== protocolProgram)
        || accounts.slice(12, 16).some((account) => account.owner !== route.tokenProgram) || !accounts[16].executable) {
      throw new Error('registered fill account owners/executable flags do not match exact controller roles');
    }
    const sellerBytes = encodeRegisteredIntentStateV1(seller.state);
    const buyerBytes = encodeRegisteredIntentStateV1(buyer.state);
    if (accounts[1].data.length !== sellerBytes.length || accounts[2].data.length !== buyerBytes.length
        || !accounts[1].data.every((byte, index) => byte === sellerBytes[index])
        || !accounts[2].data.every((byte, index) => byte === buyerBytes[index])) throw new Error('registered residual changed while constructing the route');
    return observation.slot;
  }

  async function buildFill(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (scan.kind !== 'ready') return;
    setArtifact(null);
    setActionStatus('Reacquiring both states, policy, route, payer, Clock, and blockhash…');
    try {
      const client = new SolanaRpcClient(endpoint);
      const slotText = await client.finalizedSlot();
      const statePair = await client.multipleAccounts([sellerAddress, buyerAddress], slotText);
      if (statePair.accounts[0].account === null || statePair.accounts[1].account === null) throw new Error('one selected registered state disappeared');
      const seller = projectRegisteredDirectState(controllerProgram, sellerAddress, statePair.slot, statePair.accounts[0].account);
      const buyer = projectRegisteredDirectState(controllerProgram, buyerAddress, statePair.slot, statePair.accounts[1].account);
      const slot = BigInt(slotText);
      for (const state of [seller.state, buyer.state]) {
        if (slot < state.intent.validFrom || slot > state.intent.validThrough) throw new Error('finalized Clock slot is outside one signed validity window');
      }
      const route = JSON.parse(routeText) as RegisteredFillRouteV1;
      const policy = await inspectDirectFeePolicy(client, protocolProgram, route.feePolicy, scan.snapshot.scanSlot);
      if (seller.state.intent.feeBasisPoints !== policy.feeBasisPoints || buyer.state.intent.feeBasisPoints !== policy.feeBasisPoints || route.feeDestination !== policy.recipient) throw new Error('registered states or route do not bind the authenticated fee policy');
      const preliminaryBlockhash = await client.latestBlockhash(statePair.slot);
      const preliminaryPlan = buildRegisteredFillTransaction({
        controllerProgram, payer, recentBlockhash: preliminaryBlockhash.blockhash, seller, buyer,
        fill: canonicalUnsigned(fill, 'fill'), executionPrice: canonicalUnsigned(executionPrice, 'execution price'), route,
      });
      const routeSlot = await validateFillRoute(client, preliminaryPlan, route, seller, buyer, statePair.slot);
      const blockhash = await client.latestBlockhash(routeSlot);
      const plan = buildRegisteredFillTransaction({
        controllerProgram, payer, recentBlockhash: blockhash.blockhash, seller, buyer,
        fill: canonicalUnsigned(fill, 'fill'), executionPrice: canonicalUnsigned(executionPrice, 'execution price'), route,
      });
      setArtifact({ action: 'Registered residual fill', base64: base64(plan.wireBytes), wireBytes: plan.wireBytes.length, signers: plan.requiredSignerKeys, blockhashSlot: blockhash.slot, lastValidBlockHeight: blockhash.lastValidBlockHeight });
      setActionStatus('Unsigned registered fill transaction constructed. Nothing was signed or submitted.');
    } catch (error) {
      setActionStatus(`Refused: ${errorMessage(error)}`);
    }
  }

  async function buildTerminal(action: 'cancel' | 'expire') {
    if (scan.kind !== 'ready') return;
    setArtifact(null);
    setActionStatus(`Reacquiring current sequence and finalized Clock for ${action}…`);
    try {
      const client = new SolanaRpcClient(endpoint);
      const floor = await client.finalizedSlot();
      const state = await observeRegisteredDirectState(client, controllerProgram, terminalAddress, floor);
      const preliminaryBlockhash = await client.latestBlockhash(state.observedSlot);
      const preliminaryPlan = buildRegisteredTerminalTransaction({ controllerProgram, payer, recentBlockhash: preliminaryBlockhash.blockhash, state, action, finalizedSlot: BigInt(floor) });
      const addresses = [...new Set([controllerProgram, payer, ...preliminaryPlan.instruction.keys.map((meta) => meta.pubkey.toBase58())])];
      const observation = await client.multipleAccounts(addresses, state.observedSlot);
      const accounts = new Map(observation.accounts.map((entry) => [entry.address, entry.account]));
      const programAccount = accounts.get(controllerProgram);
      const payerAccount = accounts.get(payer);
      const controllerAccount = accounts.get(preliminaryPlan.instruction.keys[0].pubkey.toBase58());
      const registrationAccount = accounts.get(state.address);
      const claimAccount = accounts.get(CLAIM_PROGRAM_ID.toBase58());
      const stateBytes = encodeRegisteredIntentStateV1(state.state);
      if (programAccount === null || !programAccount?.executable || payerAccount === null || payerAccount?.executable
          || controllerAccount === null || controllerAccount?.executable || registrationAccount === null || registrationAccount?.owner !== CLAIM_PROGRAM_ID.toBase58()
          || registrationAccount.data.length !== stateBytes.length || !registrationAccount.data.every((byte, index) => byte === stateBytes[index])
          || claimAccount === null || !claimAccount?.executable) throw new Error('terminal route changed state/owner/executable facts during one-context reacquisition');
      const blockhash = await client.latestBlockhash(observation.slot);
      const plan = buildRegisteredTerminalTransaction({ controllerProgram, payer, recentBlockhash: blockhash.blockhash, state, action, finalizedSlot: BigInt(observation.slot) });
      setArtifact({ action: action === 'cancel' ? 'Maker cancellation' : 'Permissionless expiry', base64: base64(plan.wireBytes), wireBytes: plan.wireBytes.length, signers: plan.requiredSignerKeys, blockhashSlot: blockhash.slot, lastValidBlockHeight: blockhash.lastValidBlockHeight });
      setActionStatus(`Unsigned ${action} transaction constructed. Nothing was signed or submitted.`);
    } catch (error) {
      setActionStatus(`Refused: ${errorMessage(error)}`);
    }
  }

  return <section className="direct-card registered-workspace" aria-labelledby="registered-heading">
    <div className="direct-card-heading"><span>04</span><div><h2 id="registered-heading">Registered orders on chain</h2><p>Create from the maker’s finalized replay nonce, or discover the pinned claim owner’s exact 232-byte states. Every transaction remains unsigned.</p></div></div>
    <form className="registered-action" onSubmit={buildCreate}>
      <div className="direct-card-heading"><span>C</span><div><h2>Create one registered order</h2><p>The browser derives the 48-byte replay root and next 232-byte registration from finalized state. A buyer transaction atomically replaces its legacy-token delegation, then asks the experimental controller to persist the exact 152-byte request.</p></div></div>
      <div className="direct-form-grid">
        <label><span>Observed open Market</span><select value={createMarket} onChange={(event) => setCreateMarket(event.target.value)}>{markets.map((market) => <option key={market.address} value={market.address}>{compact(market.address)} · generation {market.generation}</option>)}</select></label>
        <label><span>Maker signer public key</span><input required value={createMaker} onChange={(event) => setCreateMaker(event.target.value.trim())} /></label>
        <label><span>Rent + transaction payer</span><input required value={createPayer} onChange={(event) => setCreatePayer(event.target.value.trim())} /></label>
        <label><span>Collateral token account</span><input required value={createCollateral} onChange={(event) => setCreateCollateral(event.target.value.trim())} /></label>
        <label><span>Canonical fee-policy record</span><input required value={createFeePolicy} onChange={(event) => setCreateFeePolicy(event.target.value.trim())} /></label>
        <label><span>Canonical capability manifest</span><input required value={createManifest} onChange={(event) => setCreateManifest(event.target.value.trim())} /></label>
        <label><span>Role</span><select value={createSide} onChange={(event) => setCreateSide(event.target.value)}><option value="1">Buyer · approve reserve</option><option value="0">Seller · destination only</option></select></label>
        <label><span>Outcome index</span><input inputMode="numeric" value={createOutcome} onChange={(event) => setCreateOutcome(event.target.value)} /></label>
        <label><span>Valid from slot</span><input inputMode="numeric" value={createValidFrom} onChange={(event) => setCreateValidFrom(event.target.value)} /></label>
        <label><span>Valid through slot</span><input inputMode="numeric" value={createValidThrough} onChange={(event) => setCreateValidThrough(event.target.value)} /></label>
        <label><span>Maximum fill atoms</span><input inputMode="numeric" value={createMaximum} onChange={(event) => setCreateMaximum(event.target.value)} /></label>
        <label><span>Limit price · 1e6 scale</span><input inputMode="numeric" value={createLimit} onChange={(event) => setCreateLimit(event.target.value)} /></label>
      </div>
      <div className="registered-facts creation-boundary"><p><strong>Funding</strong> Payer funds the missing replay root plus every registration. Transaction fees are reported separately from rent.</p><p><strong>Delegation</strong> Buyer approval is bounded to maximum-fill × limit-price plus the signed fee. Seller collateral is a destination and is not delegated.</p><p><strong>Experimental boundary</strong> Claim and custody/controller identities are pinned successor programs, not an official deployed release.</p></div>
      <button type="submit" disabled={markets.length === 0}>Reacquire nonce & build unsigned creation</button>
      <p className="direct-status" aria-live="polite">{markets.length === 0 ? 'No binding-clean Open Market is available for registered creation.' : createStatus || 'No replay root, token delegation, rent, or blockhash has been read yet.'}</p>
      {createArtifact && <div className="direct-output registered-artifact"><dl><div><dt>Exact nonce</dt><dd>{createArtifact.nonce}</dd></div><div><dt>Replay root</dt><dd>{createArtifact.replay}</dd></div><div><dt>New registration</dt><dd>{createArtifact.registration}</dd></div><div><dt>Rent funding</dt><dd>{createArtifact.rentLamports} required · {createArtifact.payerLamports} observed</dd></div><div><dt>Approval</dt><dd>{createArtifact.approval}</dd></div><div><dt>Delegation change</dt><dd>{createArtifact.delegation}</dd></div><div><dt>Wire profile</dt><dd>{createArtifact.wireBytes} / 1232 bytes</dd></div><div><dt>Required external signers</dt><dd>{createArtifact.signers.join(' · ')}</dd></div><div><dt>Blockhash lifetime</dt><dd>slot {createArtifact.blockhashSlot} · height {createArtifact.lastValidBlockHeight}</dd></div></dl><label><span>Unsigned v0 transaction · base64</span><textarea readOnly value={createArtifact.base64} /></label><p className="direct-refusal">All signature slots are zero. A separate wallet boundary must inspect, sign, and submit this exact artifact.</p></div>}
    </form>
    <div className="registered-action"><div className="direct-card-heading"><span>D</span><div><h2>Discover existing residuals</h2><p>Fill, cancellation, and expiry start only from reacquired persisted authority.</p></div></div>
    <button type="button" onClick={discover} disabled={scan.kind === 'loading'}>{scan.kind === 'loading' ? 'Reading registered states…' : 'Discover registered Direct states'}</button>
    <p className="direct-status">{scan.kind === 'ready' ? `${scan.snapshot.states.length} accepted · ${scan.snapshot.refused.length} refused at slot ${scan.snapshot.scanSlot}` : scan.message ?? 'No claim-owner scan has run.'}</p>
    </div>
    {scan.kind === 'ready' && <>
      {scan.snapshot.states.length === 0 ? <p className="direct-refusal">No canonical registered state was found. This workspace does not synthesize one.</p> : <div className="registered-state-grid">{scan.snapshot.states.map((state) => <StateCard key={state.address} observation={state} />)}</div>}
      {scan.snapshot.refused.length > 0 && <details className="registered-refusals"><summary>{scan.snapshot.refused.length} candidate account(s) refused</summary>{scan.snapshot.refused.map((state) => <p key={state.address}>{compact(state.address)} · {state.reason}</p>)}</details>}
      {openStates.length > 0 && <>
        <label><span>Fee payer public key</span><input required value={payer} onChange={(event) => setPayer(event.target.value.trim())} /></label>
        <form className="registered-action" onSubmit={buildFill}>
          <div className="direct-card-heading"><span>F</span><div><h2>Fill two residuals</h2><p>Matcher action; only the fee payer signs. Both maker authorizations already live in the claim-owned states.</p></div></div>
          <div className="direct-form-grid"><label><span>Seller state</span><select value={sellerAddress} onChange={(event) => setSellerAddress(event.target.value)}>{sellers.map((state) => <option key={state.address} value={state.address}>{compact(state.address)} · {state.state.remaining.toString()}</option>)}</select></label><label><span>Buyer state</span><select value={buyerAddress} onChange={(event) => setBuyerAddress(event.target.value)}>{buyers.map((state) => <option key={state.address} value={state.address}>{compact(state.address)} · {state.state.remaining.toString()}</option>)}</select></label><label><span>Fill atoms</span><input inputMode="numeric" value={fill} onChange={(event) => setFill(event.target.value)} /></label><label><span>Execution price · 1e6</span><input inputMode="numeric" value={executionPrice} onChange={(event) => setExecutionPrice(event.target.value)} /></label></div>
          <label><span>Chain route · no signatures or private keys</span><textarea className="match-envelope" value={routeText} onChange={(event) => setRouteText(event.target.value)} spellCheck={false} /></label>
          <button type="submit" disabled={sellers.length === 0 || buyers.length === 0}>Reacquire & build unsigned fill</button>
        </form>
        <div className="registered-action">
          <div className="direct-card-heading"><span>T</span><div><h2>Close one residual</h2><p>Cancellation requires the persisted maker signature. Expiry is payer-only and refuses until finalized Clock is strictly after valid-through.</p></div></div>
          <label><span>Open registered state</span><select value={terminalAddress} onChange={(event) => setTerminalAddress(event.target.value)}>{openStates.map((state) => <option key={state.address} value={state.address}>{compact(state.address)} · {state.state.maker === payer ? 'payer is maker' : registeredPhase(state.state.phase)}</option>)}</select></label>
          <div className="registered-buttons"><button type="button" onClick={() => buildTerminal('cancel')}>Build unsigned cancellation</button><button type="button" onClick={() => buildTerminal('expire')}>Build unsigned expiry</button></div>
        </div>
        <p className="direct-status" aria-live="polite">{actionStatus}</p>
        {artifact && <div className="direct-output registered-artifact"><dl><div><dt>Action</dt><dd>{artifact.action}</dd></div><div><dt>Wire profile</dt><dd>{artifact.wireBytes} / 1232 bytes</dd></div><div><dt>Required external signers</dt><dd>{artifact.signers.join(' · ')}</dd></div><div><dt>Blockhash lifetime</dt><dd>slot {artifact.blockhashSlot} · height {artifact.lastValidBlockHeight}</dd></div></dl><label><span>Unsigned v0 transaction · base64</span><textarea readOnly value={artifact.base64} /></label><p className="direct-refusal">This artifact still contains zero signature slots. The browser did not sign or submit it.</p></div>}
      </>}
    </>}
  </section>;
}
