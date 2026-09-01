'use client';

import { useEffect, useMemo, useRef, useState } from 'react';

import { type WalletDirectoryHandleV1 } from '@/components/WalletDirectory';
import TicketCard from '@/components/trade/TicketCard';
import { inspectDirectMakerNonceV1 } from '@/lib/directMakerReplay';
import {
  composeDirectSellOfferV1,
  sealDirectSellOfferV1,
  type AuthoredDirectSellOfferV1,
  type DirectSellOfferDraftV1,
} from '@/lib/directOfferAuthoring';
import { inspectDirectSellerReadinessV1 } from '@/lib/directParticipant';
import {
  exactTwinV1,
  formatClaimPriceV1,
  formatQuantityV1,
  parseClaimPriceV1,
  parseQuantityV1,
  type DenominationV1,
} from '@/lib/quantity';
import { SolanaRpcClient } from '@/lib/rpc';
import { type SlotClockV1 } from '@/lib/slotClock';
import {
  postConfiguredBoardOfferV1,
  type TicketBoardConfigV1,
} from '@/lib/ticketBoard';
import { requestWalletMessageSignatureV1 } from '@/lib/walletHandoff';

const U64_MAX_V1 = BigInt('18446744073709551615');

type ComposerActivityV1 =
  | Readonly<{ kind: 'idle'; message: string }>
  | Readonly<{ kind: 'working'; message: string }>
  | Readonly<{ kind: 'preview'; message: string }>
  | Readonly<{ kind: 'signed'; message: string }>
  | Readonly<{ kind: 'refused'; message: string }>;

type BoardPostV1 =
  | Readonly<{ kind: 'idle'; message: string }>
  | Readonly<{ kind: 'working'; message: string }>
  | Readonly<{ kind: 'posted'; message: string }>
  | Readonly<{ kind: 'refused'; message: string }>;

type IssuedOfferV1 = Readonly<{
  authored: AuthoredDirectSellOfferV1;
  draft: DirectSellOfferDraftV1;
}>;

function durationSlotsV1(text: string): bigint {
  const cleaned = text.trim();
  if (!/^[0-9]+$/.test(cleaned)) {
    throw new Error('validity must be one positive whole number of slots');
  }
  const duration = BigInt(cleaned);
  if (duration <= BigInt(0) || duration > U64_MAX_V1) {
    throw new Error('validity must be one positive u64 number of slots');
  }
  return duration;
}

export default function MakerOfferComposer({
  endpoint,
  marketAddress,
  coreProgramId,
  registryProgramId,
  claimsProgramId,
  tradingProgramId,
  custodyProgramId,
  rentProgramId,
  generation,
  feeBasisPoints,
  outcomeCount,
  outcome,
  outcomeLabel,
  denomination,
  priceScale,
  clock,
  nowMs,
  wallets,
  boardConfig,
}: Readonly<{
  endpoint: string;
  marketAddress: string;
  coreProgramId: string;
  registryProgramId: string | null;
  claimsProgramId: string | null;
  tradingProgramId: string | null;
  custodyProgramId: string | null;
  rentProgramId: string | null;
  generation: bigint;
  feeBasisPoints: number;
  outcomeCount: number;
  outcome: number | null;
  outcomeLabel: (index: number) => string;
  denomination: DenominationV1;
  priceScale: bigint;
  clock: SlotClockV1 | null;
  nowMs: number | null;
  wallets: WalletDirectoryHandleV1;
  boardConfig: TicketBoardConfigV1 | null;
}>) {
  const [amount, setAmount] = useState('');
  const [priceCents, setPriceCents] = useState('');
  const [lifecycle, setLifecycle] = useState<0 | 1>(1);
  const [duration, setDuration] = useState('');
  const [review, setReview] = useState<Readonly<{ source: string; draft: DirectSellOfferDraftV1 }> | null>(null);
  const [issued, setIssued] = useState<IssuedOfferV1 | null>(null);
  const [activity, setActivity] = useState<ComposerActivityV1>({
    kind: 'idle',
    message: 'Set the exact terms, then check them against your current Claims position. Nothing is signed by checking.',
  });
  const [boardPost, setBoardPost] = useState<BoardPostV1>({
    kind: 'idle',
    message: 'This signed ticket has not been sent to a relay.',
  });
  const revision = useRef(0);
  const issuedRevision = useRef(0);

  const routeIdentity = useMemo(() => [
    endpoint, marketAddress, coreProgramId, registryProgramId ?? '', claimsProgramId ?? '',
    tradingProgramId ?? '', custodyProgramId ?? '', rentProgramId ?? '', generation.toString(),
    feeBasisPoints.toString(), outcomeCount.toString(), outcome?.toString() ?? '',
    wallets.address ?? '', priceScale.toString(),
  ].join('|'), [
    endpoint, marketAddress, coreProgramId, registryProgramId, claimsProgramId,
    tradingProgramId, custodyProgramId, rentProgramId, generation, feeBasisPoints,
    outcomeCount, outcome, wallets.address, priceScale,
  ]);
  const sourceIdentity = `${routeIdentity}|${amount}|${priceCents}|${lifecycle}|${duration}`;
  const sourceIdentityRef = useRef(sourceIdentity);
  // Synchronize the asynchronous-act guard only; no render state is derived
  // from this effect. Input handlers also bump `revision` synchronously.
  useEffect(() => { sourceIdentityRef.current = sourceIdentity; }, [sourceIdentity]);
  // Route/input changes make a review inapplicable without an effect-driven
  // state cascade. An already-authored ticket intentionally remains visible:
  // it is portable evidence, not a projection of the fields now on screen.
  const preview = review?.source === sourceIdentity ? review.draft : null;

  function edit(next: () => void): void {
    revision.current += 1;
    next();
    setReview(null);
    setActivity({
      kind: 'idle',
      message: 'Terms changed. Check the current terms before asking your wallet to sign them.',
    });
  }

  async function acquireDraft(): Promise<DirectSellOfferDraftV1> {
    if (wallets.address === null) throw new Error('connect the wallet that owns the claims before checking an offer');
    if (outcome === null) throw new Error('pick the outcome whose claims you want to sell');
    if (registryProgramId === null || claimsProgramId === null || tradingProgramId === null
        || custodyProgramId === null || rentProgramId === null) {
      throw new Error('this Market has no complete authenticated Direct program route');
    }
    const maximumFill = parseQuantityV1(amount, denomination);
    const limitPrice = parseClaimPriceV1(priceCents, priceScale);
    const durationSlots = durationSlotsV1(duration);
    const client = new SolanaRpcClient(endpoint);
    const seller = await inspectDirectSellerReadinessV1(client, {
      market: marketAddress,
      owner: wallets.address,
      coreProgram: coreProgramId,
      registryProgram: registryProgramId,
      claimsProgram: claimsProgramId,
      tradingProgram: tradingProgramId,
      custodyProgram: custodyProgramId,
      rentProgram: rentProgramId,
    });
    if (seller.status !== 'ready') throw new Error(seller.reason);
    const replay = await inspectDirectMakerNonceV1(client, {
      tradingProgram: tradingProgramId,
      market: marketAddress,
      generation,
      maker: wallets.address,
    });
    return composeDirectSellOfferV1({
      route: {
        market: marketAddress,
        generation,
        outcomeCount,
        priceScale,
        feeBasisPoints,
        tradingProgram: tradingProgramId,
      },
      maker: wallets.address,
      seller,
      replay,
      outcome,
      maximumFill,
      limitPrice,
      lifecycle,
      durationSlots,
    });
  }

  async function checkTerms(): Promise<void> {
    const token = ++revision.current;
    const source = sourceIdentity;
    setReview(null);
    setActivity({ kind: 'working', message: 'Reacquiring your Claims position and canonical next maker nonce…' });
    try {
      const next = await acquireDraft();
      if (revision.current !== token || sourceIdentityRef.current !== source) return;
      setReview(Object.freeze({ source, draft: next }));
      setActivity({
        kind: 'preview',
        message: 'These exact terms fit your current Claims balance and canonical maker nonce. Signing will reacquire both once more.',
      });
    } catch (error) {
      if (revision.current !== token || sourceIdentityRef.current !== source) return;
      setActivity({ kind: 'refused', message: error instanceof Error ? error.message : String(error) });
    }
  }

  async function signTicket(): Promise<void> {
    if (preview === null) return;
    const token = ++revision.current;
    const source = sourceIdentity;
    setActivity({ kind: 'working', message: 'Reacquiring the chain state, then asking your wallet for one detached message signature…' });
    try {
      const current = await acquireDraft();
      if (revision.current !== token || sourceIdentityRef.current !== source) return;
      const maker = wallets.address;
      if (maker === null) throw new Error('the connected wallet disappeared before signing');
      const signature = await requestWalletMessageSignatureV1(
        new SolanaRpcClient(endpoint),
        wallets.handoff(endpoint),
        maker,
        current.signingMessage,
      );
      if (revision.current !== token || sourceIdentityRef.current !== source) return;
      const authored = sealDirectSellOfferV1(maker, current, signature);
      setReview(Object.freeze({ source, draft: current }));
      issuedRevision.current += 1;
      setIssued(Object.freeze({ authored, draft: current }));
      setBoardPost({ kind: 'idle', message: 'This signed ticket has not been sent to a relay.' });
      setActivity({
        kind: 'signed',
        message: 'Portable sell ticket authored. No transaction was submitted and no claims moved.',
      });
    } catch (error) {
      if (revision.current !== token || sourceIdentityRef.current !== source) return;
      setActivity({ kind: 'refused', message: error instanceof Error ? error.message : String(error) });
    }
  }

  async function postTicket(): Promise<void> {
    if (issued === null || boardConfig === null) return;
    const exactText = issued.authored.text;
    const token = issuedRevision.current;
    setBoardPost({ kind: 'working', message: 'Sending this exact signed text to the configured relay…' });
    try {
      const report = await postConfiguredBoardOfferV1(boardConfig, exactText);
      if (issuedRevision.current !== token) return;
      setBoardPost({
        kind: 'posted',
        message: report.duplicate
          ? `The relay already held this exact ticket (${report.digest}).`
          : `The relay accepted this exact ticket (${report.digest}).`,
      });
    } catch (error) {
      if (issuedRevision.current !== token) return;
      setBoardPost({
        kind: 'refused',
        message: `${error instanceof Error ? error.message : String(error)} The signed ticket below is unchanged and still works without the relay.`,
      });
    }
  }

  const previewAmount = preview === null ? null : formatQuantityV1(preview.intent.maximumFill, denomination);
  const previewPrice = preview === null ? null : formatClaimPriceV1(preview.intent.limitPrice, priceScale);

  return <details className="maker-offer">
    <summary>Make your own sell offer</summary>
    <div className="maker-offer-body">
      <p className="direct-status">You are authoring an offer, not making a transaction. Your wallet signs the terms; your claims move only if someone later submits a matching trade that the chain accepts.</p>

      <div className="direct-form-grid maker-offer-fields">
        <label>
          <span>Claims to sell</span>
          <input inputMode="decimal" placeholder="for example 25" value={amount} onChange={(event) => edit(() => setAmount(event.target.value))} />
          <small>{outcome === null ? 'Pick an outcome above first.' : outcomeLabel(outcome)}</small>
        </label>
        <label>
          <span>Price for each claim</span>
          <input inputMode="decimal" placeholder="cents, for example 35" value={priceCents} onChange={(event) => edit(() => setPriceCents(event.target.value))} />
          <small>More than 0 through 100 cents on one full collateral payout; exact ticks only.</small>
        </label>
        <label>
          <span>Fill rule</span>
          <select value={lifecycle} onChange={(event) => edit(() => setLifecycle(event.target.value === '0' ? 0 : 1))}>
            <option value="1">Allow one smaller fill</option>
            <option value="0">All or nothing</option>
          </select>
          <small>{lifecycle === 0
            ? 'The whole signed size must cross in one execution.'
            : 'One smaller fill may cross; no remainder rests onchain.'}</small>
        </label>
        <label>
          <span>Valid for how many slots</span>
          <input inputMode="numeric" placeholder="choose explicitly" value={duration} onChange={(event) => edit(() => setDuration(event.target.value))} />
          <small>No default lifetime is chosen for you. The signed deadline starts at the reacquired finalized slot.</small>
        </label>
      </div>

      <p className="direct-status">The Market&apos;s immutable fee is {feeBasisPoints} basis points. It is copied into the signed terms; this form cannot change it.</p>
      <div className="direct-actions">
        <button type="button" disabled={activity.kind === 'working'} onClick={() => void checkTerms()}>Check these exact terms</button>
        <button type="button" disabled={preview === null || activity.kind === 'working'} onClick={() => void signTicket()}>Sign portable ticket</button>
      </div>
      <p className={activity.kind === 'refused' ? 'market-refusal' : 'direct-status'} aria-live="polite">{activity.kind === 'refused' ? `Refused: ${activity.message}` : activity.message}</p>

      {preview !== null && previewAmount !== null && previewPrice !== null && <div className="maker-offer-review">
        <strong>Exact terms ready for signature</strong>
        <p>{previewAmount.display} {outcomeLabel(preview.intent.outcome)} claims at {previewPrice.display} each · {preview.intent.lifecycle === 0 ? 'all or nothing' : 'one smaller fill allowed'}.</p>
        <small>{exactTwinV1(previewAmount, 'claim')} · price {previewPrice.fraction} of one payout · nonce {preview.intent.nonce.toString()} · slots {preview.intent.validFrom.toString()} through {preview.intent.validThrough.toString()}.</small>
        <small>{preview.collateralPrestate === 'vacant'
          ? 'Your canonical Direct destination token account is vacant; the execution route creates it permissionlessly if this ticket crosses.'
          : 'Your canonical Direct destination token account is already initialized.'}</small>
      </div>}

      {issued !== null && <div className="maker-offer-issued">
        <TicketCard
          ticket={issued.authored.ticket}
          denomination={denomination}
          priceScale={priceScale}
          outcomeLabel={outcomeLabel}
          clock={clock}
          nowMs={nowMs}
        />
        <label>
          <span>Portable ticket JSON</span>
          <textarea readOnly rows={8} spellCheck={false} value={issued.authored.text} />
        </label>
        <div className="direct-actions">
          <a
            className="secondary-action"
            download={`dclutch-direct-sell-${issued.authored.ticket.intent.nonce.toString()}.json`}
            href={`data:application/json;charset=utf-8,${encodeURIComponent(issued.authored.text)}`}
          >Download exact ticket</a>
          <button type="button" onClick={() => {
            void navigator.clipboard?.writeText(issued.authored.text)
              .then(() => setBoardPost({ kind: 'idle', message: 'Copied the exact ticket text.' }))
              .catch(() => setBoardPost({ kind: 'refused', message: 'This browser refused clipboard access. The download and text remain available.' }));
          }}>Copy ticket</button>
          {boardConfig !== null && <button type="button" disabled={boardPost.kind === 'working'} onClick={() => void postTicket()}>Post to configured relay</button>}
        </div>
        <p className={boardPost.kind === 'refused' ? 'market-refusal' : 'direct-status'} aria-live="polite">{boardPost.kind === 'refused' ? `Relay refused: ${boardPost.message}` : boardPost.message}</p>
        {boardConfig === null && <p className="board-honesty">No relay is configured. That does not weaken this ticket: download it or send its exact text directly to a taker.</p>}
        <button type="button" className="secondary-action" onClick={() => {
          issuedRevision.current += 1;
          setIssued(null);
          setBoardPost({ kind: 'idle', message: 'This signed ticket has not been sent to a relay.' });
        }}>Clear this ticket from the screen</button>
      </div>}
    </div>
  </details>;
}
