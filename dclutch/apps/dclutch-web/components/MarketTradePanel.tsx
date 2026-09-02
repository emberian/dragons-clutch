'use client';

import Anchor from '@/components/Anchor';
import { useEffect, useMemo, useRef, useState } from 'react';

import WalletDirectory, { useWalletDirectoryV1 } from '@/components/WalletDirectory';
import { admissionRequestV1, JoinStanding } from '@/components/JoinPanel';
import { useDeploymentV1 } from '@/lib/deploymentStore';
import FlowRail from '@/components/trade/FlowRail';
import FlowStep from '@/components/trade/FlowStep';
import MarketGateCard from '@/components/trade/MarketGateCard';
import PreviewReceipt from '@/components/trade/PreviewReceipt';
import StepRefusal from '@/components/trade/StepRefusal';
import TicketBoard from '@/components/trade/TicketBoard';
import SignStep from '@/components/trade/steps/SignStep';
import SendStep from '@/components/trade/steps/SendStep';
import { type DirectParticipantReadinessV1 } from '@/lib/directParticipant';
import { type DirectTradeSpineV1 } from '@/lib/directTradeSpine';
import { type MarketLiabilityV1 } from '@/lib/marketDiscovery';
import { checkedReleaseSetIdsV1 } from '@/lib/publicCutStaging';
import { publishedDirectRouteManifestV1 } from '@/lib/publishedRouteManifests';
import { type SlotClockV1 } from '@/lib/slotClock';
import {
  denominationUnitV1,
  exactTwinV1,
  formatQuantityV1,
  type DenominationV1,
} from '@/lib/quantity';
import { assignRefusalV1, type FlowStepIndexV1, type StepRefusalV1 } from '@/lib/tradeFlowRefusals';
import { marketGateV1, outcomeShareV1, sizeDecisionV1, tradeFlowStepsV1 } from '@/lib/tradeFlowSteps';
import {
  createDirectTradeFlowMachineV1,
  directTicketStateV1,
  type ExecutionState,
  type TicketState,
  type WalletPreparationState,
} from '@/lib/tradeFlowMachine';

/**
 * The trader's face of one Market, as a flow you can see the whole of.
 *
 * What changed here is PRESENTATION. The orchestration lives in
 * `lib/tradeFlowMachine.ts` and is called, not reimplemented: durable intent
 * before key access, signature match on resume, never a second send, and five
 * separate re-checks that the chain under the flow is still the chain the flow
 * started on. That discipline is the product. This file decides what a reader
 * sees; it decides nothing about what the protocol does.
 *
 * The panel this replaces was eleven sibling blocks in one section -- two
 * buttons, three status paragraphs, two evidence grids, an outcome list, a
 * textarea, a size input, a wallet directory, a walls list, and one `<details>`
 * holding the entire second half of the trade. Every part of it was correct.
 * None of it told a reader where they were, what was left, or which of the
 * things on screen was the one they were supposed to touch next.
 *
 * Seven steps, always all seven visible. Two market-level walls resolved BEFORE
 * the stepper, because six greyed steps under "this market can never trade" is
 * the same failure in a new costume. And every named refusal routed to the step
 * that owns it, remedy first -- which is why `lib/tradeFlowRefusals.ts` exists:
 * the machine reports through two state slots, and those two slots are not two
 * steps.
 */
export default function MarketTradePanel({
  endpoint,
  marketAddress,
  coreProgramId,
  registryProgramId,
  claimsProgramId,
  tradingProgramId,
  custodyProgramId,
  rentProgramId,
  liability,
  denomination,
  outcomes: outcomeNames,
  clock,
  nowMs,
}: Readonly<{
  endpoint: string;
  marketAddress: string;
  coreProgramId: string;
  registryProgramId: string | null;
  claimsProgramId: string | null;
  tradingProgramId: string | null;
  custodyProgramId: string | null;
  rentProgramId: string | null;
  liability: MarketLiabilityV1 | null;
  /**
   * The collateral's display denomination, resolved once by the market page.
   * The panel never read this before, so ember's 500000000 rendered as
   * 500000000 when the mint's own decimals byte said it was 500.
   */
  denomination: DenominationV1;
  /**
   * This market's outcome names, index-ordered, or null.
   *
   * The market page resolves them once -- from its editorial row where it has
   * one and from the market's own result-domain record where it does not --
   * and hands the answer down. This panel used to take the editorial ENTRY and
   * read `.outcomes` off it, which meant the one place a person CHOSE an
   * outcome was the one place an unregistered market said `claim 0`.
   */
  outcomes: ReadonlyArray<string> | null;
  /**
   * The measured slot clock the detail page already reads, so a ticket's
   * deadline can be a time rather than a slot number. Null renders the exact
   * slot instead: an estimated countdown with no measured rate behind it is a
   * guess wearing a clock's clothes.
   */
  clock: SlotClockV1 | null;
  nowMs: number | null;
}>) {
  const wallets = useWalletDirectoryV1();
  const deployment = useDeploymentV1();
  const [spine, setSpine] = useState<DirectTradeSpineV1 | null>(null);
  const [spineStatus, setSpineStatus] = useState('The chain has not been asked about trading this Market yet.');
  const [participant, setParticipant] = useState<DirectParticipantReadinessV1 | null>(null);
  const [participantStatus, setParticipantStatus] = useState('Connect your wallet, then ask the chain to check your Position, admission, and collateral account.');
  const [outcome, setOutcome] = useState<number | null>(null);
  const [desired, setDesired] = useState('');
  const [ticketText, setTicketText] = useState('');
  const publishedRoute = publishedDirectRouteManifestV1(marketAddress);
  const [routeText, setRouteText] = useState(publishedRoute ?? '');
  const [execution, setExecution] = useState<ExecutionState>({ kind: 'idle' });
  const [walletPreparation, setWalletPreparation] = useState<WalletPreparationState>({ kind: 'idle' });

  const inspected = spine !== null && spine.status === 'inspected' ? spine : null;

  const ticketState: TicketState = useMemo(
    () => directTicketStateV1({ inspected, ticketText, wallets, claimsProgramId }),
    [inspected, ticketText, wallets, claimsProgramId],
  );

  // Rebuilt every render, which is exactly what React did with these closures
  // when they lived in this function body. The orchestration itself is in
  // lib/tradeFlowMachine.ts and is unchanged by this panel's redesign.
  const {
    invalidatePreview, invalidateWalletState, inspect, previewIntent,
    prepareWalletIntent, signPreparedTransaction, submitDirectPacket,
  } = createDirectTradeFlowMachineV1({
    endpoint, marketAddress, coreProgramId, registryProgramId, claimsProgramId,
    tradingProgramId, custodyProgramId, rentProgramId, denomination, wallets,
    checkedReleaseSetIds: checkedReleaseSetIdsV1(),
    inspected, participant, outcome, desired, routeText,
    ticketState, execution, walletPreparation,
    setSpine, setSpineStatus, setParticipant, setParticipantStatus,
    setExecution, setWalletPreparation,
  });

  const supplies = liability !== null && liability.status === 'bound' ? liability.supplyAtoms : null;
  const unit = denominationUnitV1(denomination);
  /**
   * The registry's name for one outcome, falling back to the index the chain
   * knows it by. Claims are denominated at the COLLATERAL's decimals on
   * purpose: one claim pays one unit of collateral if it wins, so the two
   * scales are the same scale, and that is the fully-backed invariant showing
   * through the units.
   */
  const outcomeLabel = (index: number): string => outcomeNames?.[index] ?? `claim ${index}`;

  // Taking a new ticket resets the size, because a size is only ever a size OF
  // something: carrying 500 across from one maker's offer to another's is how
  // a reader ends up signing a number they last thought about two offers ago.
  const takeTicket = (next: string): void => {
    setTicketText(next);
    setDesired('');
    invalidatePreview();
  };

  /**
   * Ask the chain about trading on load, once per market.
   *
   * It used to take a button. So the market-level wall -- "founded, but never
   * switched on", the single most consequential thing this page can say --
   * appeared only to a reader who had already clicked a control labelled "Ask
   * the chain about trading here", and step 1 with it. Everything else on this
   * page reads on mount; there is no reason this was the exception, and the
   * button stays for re-reading.
   *
   * Guarded by the market it read rather than by a dependency array: the
   * machine's closures are rebuilt every render, so `[inspect]` would loop.
   */
  const inspectedFor = useRef<string | null>(null);
  useEffect(() => {
    const key = `${endpoint}|${marketAddress}`;
    if (inspectedFor.current === key) return;
    inspectedFor.current = key;
    void inspect();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [endpoint, marketAddress]);

  const gate = inspected === null ? null : marketGateV1(inspected.walls);
  const packetWall = inspected?.walls.find((wall) => wall.name === 'packet') ?? null;
  const prestateWall = inspected?.walls.find((wall) => wall.name === 'prestate') ?? null;
  const size = sizeDecisionV1(desired, denomination);
  const ticket = ticketState.kind === 'ready' ? ticketState.ticket : null;
  const fillOrKill = ticket !== null && ticket.intent.lifecycle === 0;

  const steps = tradeFlowStepsV1({
    participantReady: participant !== null && participant.status === 'ready',
    outcomePicked: outcome !== null,
    outcomeCountKnown: inspected?.outcomeCount != null,
    ticketReady: ticket !== null,
    sizeAccepted: size.ok,
    previewReady: execution.kind === 'ready',
    intentSigned: walletPreparation.kind !== 'idle' && walletPreparation.kind !== 'working' && walletPreparation.kind !== 'refused',
    packetSigned: walletPreparation.kind === 'wallet-signed' || walletPreparation.kind === 'submitted' || walletPreparation.kind === 'executed',
    executed: walletPreparation.kind === 'executed',
    operatorRequired: walletPreparation.kind === 'operator-required',
    packetWallDetail: packetWall?.detail ?? null,
  });

  /**
   * Every live refusal, routed to its owner.
   *
   * The machine reports through two slots and the ticket decoder through a
   * third; between them they raise refusals belonging to five different steps.
   * Routing happens once, here, so that each step below can ask only "which of
   * these is mine" and no step has to know what any other step's refusals look
   * like.
   */
  const routed: ReadonlyArray<StepRefusalV1> = [
    ticketState.kind === 'refused' ? assignRefusalV1(ticketState.reason, 3) : null,
    execution.kind === 'refused' ? assignRefusalV1(execution.reason, 5) : null,
    walletPreparation.kind === 'refused' ? assignRefusalV1(walletPreparation.reason, 6) : null,
    prestateWall !== null ? assignRefusalV1(prestateWall.detail, 1) : null,
  ].filter((entry): entry is StepRefusalV1 => entry !== null);
  const refusalFor = (step: FlowStepIndexV1): StepRefusalV1 | null =>
    routed.find((entry) => entry.step === step) ?? null;

  const stepAt = (index: FlowStepIndexV1) => steps[index - 1]!;

  return <section className="trade-v3-card">
    <header><span>06</span><div><h2>Trade this market</h2><p>Pick an outcome, choose how much, and take one signed offer at the price its maker set.</p></div></header>

    <div className="direct-actions">
      <button type="button" onClick={() => void inspect()}>Ask the chain about trading here</button>
    </div>
    <p className="direct-status" aria-live="polite">{spineStatus}</p>

    {spine !== null && spine.status === 'refused' && <p className="market-refusal">Refused: {spine.reason}</p>}

    {gate !== null && gate.kind === 'closed' && <MarketGateCard gate={gate} />}

    {inspected !== null && gate !== null && gate.kind === 'open' && <FlowRail steps={steps} />}

    {/*
      STEP 1 STANDS OUTSIDE THE GATE, because the chain does.

      `phase` and `activation` are walls in front of a FILL. They are not walls
      in front of joining: a Market admits participants and takes collateral
      while Direct execution is closed, and cohort-12 admitted two strangers on
      a market whose Direct capability was not switched on yet. The panel used
      to render step 1 inside the same block as steps 2-7, so a reader with a
      wallet met a market page with no wallet control anywhere on it, could not
      see their own Position, and could not join -- while `/console` was
      advertising exactly that act, and the chain was accepting it.

      That was the one row in the whole browser where it refused something the
      chain accepts. Steps 2-7 stay gated, because those really are the fill.
    */}
    {inspected !== null && <>
      {gate !== null && gate.kind === 'closed' && <p className="direct-status">
        Trading is closed here, and joining is not. You can connect a wallet, read
        your standing, and join this market now; the steps that trade appear when
        the wall above is gone.
      </p>}

      <FlowStep step={stepAt(1)}>
        <p className="direct-status" aria-live="polite">{participantStatus}</p>
        <WalletDirectory directory={wallets} onConnected={invalidateWalletState} />
        {wallets.address === null
          ? <p className="direct-status">Connect a wallet to see where you stand.</p>
          : participant !== null && participant.status === 'ready'
            ? (() => {
              const spendable = formatQuantityV1(participant.spendableCollateralAtoms, denomination);
              const balance = outcome === null ? null : formatQuantityV1(participant.positionBalances[outcome] ?? 0n, denomination);
              return <>
                <div className="trade-v3-evidence">
                  <article>
                    <span>Your collateral, spendable</span>
                    <strong title={spendable.title}>{spendable.display} {unit}</strong>
                    <small>{exactTwinV1(spendable, 'collateral')} — a buy needs this to cover its debit exactly</small>
                  </article>
                  <article>
                    <span>Your claim balance</span>
                    <strong title={balance === null ? undefined : balance.title}>{balance === null ? 'pick a claim' : `${balance.display} claims`}</strong>
                    <small>{balance === null
                      ? `finalized Position revision ${participant.positionRevision.toString()}`
                      : `${exactTwinV1(balance, 'claim')} · finalized Position revision ${participant.positionRevision.toString()}`}</small>
                  </article>
                </div>
                <details className="trade-v3-bytes">
                  <summary>Your accounts, exactly as the chain has them</summary>
                  <JoinStanding
                    readiness={participant}
                    marketPhase={inspected.phase}
                    walletAddress={wallets.address}
                    endpoint={endpoint}
                    admission={admissionRequestV1({
                      market: marketAddress, owner: wallets.address, coreProgramId,
                      registryProgramId, claimsProgramId, tradingProgramId, rentProgramId,
                      activationCache: deployment.activationCache,
                    })}
                    directory={wallets}
                  />
                </details>
              </>;
            })()
            : null}
        {refusalFor(1) !== null && <StepRefusal refusal={refusalFor(1)!} />}
        {participant !== null && participant.status === 'incomplete' && prestateWall === null && <p className="market-refusal">Not ready: {participant.reason}</p>}
        {participant !== null && participant.status === 'refused' && <p className="market-refusal">Participant state refused: {participant.reason}</p>}
      </FlowStep>
    </>}

    {inspected !== null && gate !== null && gate.kind === 'open' && <>
      <FlowStep step={stepAt(2)}>
        {refusalFor(2) !== null && <StepRefusal refusal={refusalFor(2)!} />}
        {inspected.outcomeCount !== null && <ol className="outcome-vector">
          {Array.from({ length: inspected.outcomeCount }, (_, index) => {
            const issued = supplies === null ? null : formatQuantityV1(supplies[index] ?? '0', denomination);
            const share = supplies === null ? null : outcomeShareV1(supplies[index] ?? '0', supplies);
            return <li key={index} className={outcome === index ? 'winning-outcome' : ''}>
              <button type="button" className="secondary-action" onClick={() => { setOutcome(index); invalidatePreview(); }}>{outcomeLabel(index)}</button>
              {issued !== null && <strong title={issued.title}>{issued.display}</strong>}
              {issued !== null && <small>{issued.humanized ? `claims issued · ${issued.atoms} atoms` : 'claim atoms issued'}{share === null ? '' : ` · ${share} of all claims issued`}</small>}
            </li>;
          })}
        </ol>}
      </FlowStep>

      <FlowStep step={stepAt(3)}>
        <TicketBoard
          endpoint={endpoint}
          marketAddress={marketAddress}
          coreProgramId={coreProgramId}
          registryProgramId={registryProgramId}
          claimsProgramId={claimsProgramId}
          tradingProgramId={tradingProgramId}
          custodyProgramId={custodyProgramId}
          rentProgramId={rentProgramId}
          outcome={outcome}
          outcomeLabel={outcomeLabel}
          screenContext={{
            connectedWallet: wallets.address,
            generation: BigInt(inspected.generation),
            feeBasisPoints: inspected.feeBasisPoints,
            outcomeCount: inspected.outcomeCount ?? 0,
            outcome,
            finalizedSlot: null,
          }}
          denomination={denomination}
          priceScale={inspected.priceScale}
          clock={clock}
          nowMs={nowMs}
          ticketText={ticketText}
          ticketState={ticketState}
          onTicketText={takeTicket}
          refusal={refusalFor(3)}
          wallets={wallets}
        />
      </FlowStep>

      <FlowStep step={stepAt(4)}>
        {ticket !== null && (() => {
          const most = formatQuantityV1(ticket.intent.maximumFill, denomination);
          // THE CEILING, NOT A BALANCE. A signed sell offer sets aside
          // nothing: its maximum fill is a bound on what this offer may ever
          // trade, and the maker's claims are moved — or the whole transaction
          // is rolled back — only when the fill executes. Saying "up to X" and
          // stopping there invites a reader to hear "X are waiting for you",
          // which no part of this protocol promises.
          const ceiling = <small className="direct-note">This is the offer&rsquo;s ceiling, not a balance. Nothing
          is set aside when a sell is signed; the chain moves the maker&rsquo;s claims when the
          trade executes, or refuses the whole trade.</small>;
          return fillOrKill
            ? <div className="size-fixed">
              <span>All or nothing — this offer is for exactly {most.display} claims.</span>
              <small title={most.title}>{exactTwinV1(most, 'claim')}. Its maker signed it fill-or-kill, so a smaller size is not a smaller trade — it is no trade.</small>
              {ceiling}
            </div>
            : <>
              <div className="direct-form-grid">
                <label>
                  <span>How much</span>
                  <input inputMode="decimal" placeholder="all of it" value={desired} onChange={(event) => { setDesired(event.target.value.trim()); invalidatePreview(); }} />
                  <small className="block text-sm text-muted-foreground">claims — blank takes the offer in full</small>
                </label>
              </div>
              <p className="direct-status" title={most.title}>You can take up to {most.display} claims from this offer · {exactTwinV1(most, 'claim')}</p>
              {ceiling}
            </>;
        })()}
        {!size.ok && <StepRefusal refusal={assignRefusalV1(size.reason, 4)} />}
        {refusalFor(4) !== null && <StepRefusal refusal={refusalFor(4)!} />}
      </FlowStep>

      <FlowStep step={stepAt(5)}>
        {execution.kind === 'ready'
          ? <PreviewReceipt
            plan={execution.plan}
            admission={execution.admission}
            replaySlot={execution.replaySlot}
            denomination={denomination}
            priceScale={inspected.priceScale}
            feeBasisPoints={inspected.feeBasisPoints}
            outcomeLabel={outcomeLabel}
          />
          : <>
            <p className="direct-status">Nothing is signed by previewing. This asks the chain what this exact crossing would do, and checks it against what you hold.</p>
            <div className="direct-actions">
              <button type="button" disabled={execution.kind === 'working'} onClick={() => void previewIntent()}>Preview this exact crossing</button>
            </div>
            {execution.kind === 'working' && <p className="direct-status" aria-live="polite">{execution.message}</p>}
          </>}
        {refusalFor(5) !== null && <StepRefusal refusal={refusalFor(5)!} />}
      </FlowStep>

      <FlowStep step={stepAt(6)}>
        <SignStep
          walletPreparation={walletPreparation}
          previewReady={execution.kind === 'ready'}
          routeText={routeText}
          publishedRoute={publishedRoute}
          onRouteText={(next) => { setRouteText(next); setWalletPreparation({ kind: 'idle' }); }}
          onPrepare={() => void prepareWalletIntent()}
          onSignPacket={() => void signPreparedTransaction()}
          refusal={refusalFor(6)}
        />
      </FlowStep>

      <FlowStep step={stepAt(7)}>
        <SendStep
          walletPreparation={walletPreparation}
          onSubmit={() => void submitDirectPacket()}
          refusal={refusalFor(7)}
        />
      </FlowStep>
    </>}

    <footer className="flow-footer">
      <Anchor className="secondary-action" href="/trade">Advanced: full route workbench →</Anchor>
      <Anchor className="secondary-action" href={`/explorer?view=market&q=${encodeURIComponent(marketAddress)}`}>See this market in the explorer →</Anchor>
    </footer>
  </section>;
}
