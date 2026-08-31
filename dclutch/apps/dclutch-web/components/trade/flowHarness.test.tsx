import { mkdirSync, writeFileSync } from 'node:fs';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import FlowRail from './FlowRail';
import FlowStep from './FlowStep';
import MarketGateCard from './MarketGateCard';
import PreviewReceipt from './PreviewReceipt';
import StepRefusal from './StepRefusal';
import TicketCard from './TicketCard';
import SignStep from './steps/SignStep';
import { assignRefusalV1 } from '@/lib/tradeFlowRefusals';
import { marketGateV1, tradeFlowStepsV1, type MarketGateV1 } from '@/lib/tradeFlowSteps';
import { type DenominationV1 } from '@/lib/quantity';
import { type SignedDirectIntentV3 } from '@/lib/directInlineV3';
import { type DirectCrossingPlanV1 } from '@dclutch/sdk/directTicket';
import { type DirectParticipantCrossingAdmissionV1 } from '@/lib/directParticipant';

/**
 * A LAYOUT HARNESS, not a behaviour test.
 *
 * The stepper only exists after a chain read, so no static render of the panel
 * can ever show it -- which means the one standard it must meet, no horizontal
 * overflow at 390px, cannot be checked by rendering the panel. This writes the
 * real components, with real props, into one page beside the real stylesheet,
 * so a browser can measure it. The measurement itself lives in the shell
 * command that opens the file; this only builds the subject.
 */

const OUT_DIR_V1 = process.env.FLOW_HARNESS_DIR ?? '';

const SIX_DECIMALS_V1: DenominationV1 = Object.freeze({ decimals: 6, unit: 'USDC', mint: 'mint' });

const TICKET_V1 = Object.freeze({
  maker: '8bcRzB3v6PxbbtkVCiX9ceW2whwakA6gX7qvSYbeMHLq',
  signature: new Uint8Array(64).fill(0xab),
  intent: Object.freeze({
    side: 0 as const, lifecycle: 0 as const, outcome: 1,
    market: '5F8wMRFMdYGMkjWQUye6WfbgRVWEo9yyKo9aFPk2TLaD',
    generation: 7n, nonce: 9n, validFrom: 11n, validThrough: 4_294_967_295n,
    maximumFill: 500_000_000n, limitPrice: 350_000n, feeBasisPoints: 0,
    collateralAccount: '7xwJ3uceuBV7KyCsdJsBs9Ljfh1bL3WB7NbGpwUNeJ2o',
  }),
}) as SignedDirectIntentV3;

const PLAN_V1 = Object.freeze({
  takerSide: 'buy', fill: 500_000_000n, executionPrice: 350_000n,
  taker: Object.freeze({ outcome: 1 }),
  note: 'Buying 500000000 claim atoms of outcome 1',
  preview: Object.freeze({
    fill: 500_000_000n, executionPrice: 350_000n, grossCollateral: 175_000_000n,
    sellerFee: 0n, buyerFee: 0n, sellerNetCollateralCredit: 175_000_000n,
    buyerCollateralDebit: 175_000_000n, totalFeeTransfer: 0n,
  }),
}) as unknown as DirectCrossingPlanV1;

const ADMISSION_V1 = Object.freeze({
  requiredAtoms: 175_000_000n, availableAtoms: 240_000_000n, resource: 'spendable collateral',
}) as unknown as DirectParticipantCrossingAdmissionV1;

const label = (index: number) => (index === 1 ? 'Above one eighty' : `claim ${index}`);

describe('the trade flow layout harness', () => {
  it('writes one page carrying every step surface, for a browser to measure', () => {
    const steps = tradeFlowStepsV1({
      participantReady: true, outcomePicked: true, outcomeCountKnown: true,
      ticketReady: true, sizeAccepted: true, previewReady: true,
      intentSigned: false, packetSigned: false, executed: false,
      operatorRequired: false, packetWallDetail: null,
    });
    const gate = marketGateV1([{
      name: 'activation',
      detail: 'this Market founded a Direct trading capability but never switched it on — no activation root exists at 7Mcu1ZT9pnLDMPqxTS9pFPqRDMbF6xhTPu2wSbc8WAC. Activation is the operator’s move, not yours.',
    }]) as Extract<MarketGateV1, { kind: 'closed' }>;

    const body = renderToStaticMarkup(<section className="trade-v3-card">
      <header><span>06</span><div><h2>Trade this market</h2><p>Pick an outcome, choose how much, and take one signed offer at the price its maker set.</p></div></header>
      <FlowRail steps={steps} />
      <FlowStep step={steps[2]!}>
        <TicketCard ticket={TICKET_V1} denomination={SIX_DECIMALS_V1} priceScale={1_000_000n} outcomeLabel={label} clock={null} nowMs={null} />
      </FlowStep>
      <FlowStep step={steps[4]!}>
        <PreviewReceipt plan={PLAN_V1} admission={ADMISSION_V1} replaySlot="490712003" denomination={SIX_DECIMALS_V1} priceScale={1_000_000n} feeBasisPoints={0} outcomeLabel={label} />
      </FlowStep>
      <FlowStep step={steps[5]!}>
        <SignStep walletPreparation={{ kind: 'idle' }} previewReady routeText="" publishedRoute={null} onRouteText={() => {}} onPrepare={() => {}} onSignPacket={() => {}} refusal={null} />
      </FlowStep>
      <StepRefusal refusal={assignRefusalV1('the ticket seller’s finalized Position does not cover this fill', 6)} />
      <MarketGateCard gate={gate} />
    </section>);

    expect(body).toContain('flow-rail');
    if (OUT_DIR_V1 === '') return;
    mkdirSync(OUT_DIR_V1, { recursive: true });
    writeFileSync(
      `${OUT_DIR_V1}/flow.html`,
      `<!doctype html><html><head><meta charset="utf-8">`
      + `<meta name="viewport" content="width=device-width,initial-scale=1">`
      + `<link rel="stylesheet" href="./globals.css"></head>`
      + `<body><main class="product-shell trade-v3-shell">${body}</main></body></html>`,
      'utf8',
    );
  });
});
