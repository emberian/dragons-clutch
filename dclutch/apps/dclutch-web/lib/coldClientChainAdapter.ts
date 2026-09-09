import { SolanaRpcClient } from '@dclutch/sdk/rpc';
import {
  enumerateCoreMarketAddressesV1,
  inspectMarketDiscoveryV1,
} from '@dclutch/sdk/marketDiscovery';
import { inspectMarketDetailV1 } from '@dclutch/sdk/marketDetail';
import { inspectDirectParticipantReadinessV1 } from '@dclutch/sdk/directParticipant';
import { inspectDirectTradeSpineV1 } from '@dclutch/sdk/directTradeSpine';
import { inspectDirectMakerNonceV1 } from '@dclutch/sdk/directMakerReplay';
import {
  decodeDirectIntentTicketV1,
  planDirectCrossingV1,
} from '@dclutch/sdk/directTicket';
import { inspectClaimsCustodyReplayV1 } from '@dclutch/sdk/claimsCustodyReplay';
import {
  importRustWalletTerminalPayoutArtifactV3,
  prepareWalletTerminalPayoutV3,
} from '@dclutch/sdk/walletTerminalPayoutV3';
import { inspectAggregateRetirementV1 } from '@dclutch/sdk/aggregateRetirement';

import { hex, sha256 } from '@dclutch/sdk/bytes';
import {
  type ColdClientAdapterV1,
  type ColdClientChainStepV1,
  type ColdClientContextV1,
  type ColdClientDeploymentV1,
  type ColdClientStepResultV1,
  type ColdClientTruthV1,
} from './coldClientJourney';

/**
 * The cold-client journey, bound to the REAL public reading surface.
 *
 * `lib/coldClientJourney.ts` owns the order and acceptance rules; this adapter
 * supplies each step from the same SDK modules the app's own workspaces mount
 * — discovery, detail, participant readiness, the Direct spine, the crossing
 * planner, the Claims replay inspector, the Rust-artifact payout preparer,
 * and the retirement seam. Nothing here signs or submits; the two unsigned
 * builder steps return digests of exact bytes and stop.
 *
 * A journey run against a live chain (local successor validator or devnet)
 * therefore exercises the exact code path a person's browser runs, minus only
 * the wallet. That is the point: the journey is evidence about the public
 * surface, not about a parallel test double.
 */

function truth(subject: string, verdict: ColdClientTruthV1['verdict'], detail: string): ColdClientTruthV1 {
  return Object.freeze({ subject, verdict, detail });
}

function refused(step: ColdClientChainStepV1, reason: string): ColdClientStepResultV1 {
  return Object.freeze({ step, status: 'refused' as const, reason });
}

function unavailable(step: ColdClientChainStepV1, reason: string): ColdClientStepResultV1 {
  return Object.freeze({ step, status: 'unavailable' as const, reason });
}

async function digestText(text: string): Promise<string> {
  return hex(await sha256(new TextEncoder().encode(text)));
}

export function makeColdClientChainAdapterV1(options: Readonly<{
  deployments: Readonly<Record<string, ColdClientDeploymentV1>>;
}>): ColdClientAdapterV1 {
  const clients = new Map<string, SolanaRpcClient>();
  const client = (deployment: ColdClientDeploymentV1): SolanaRpcClient => {
    const existing = clients.get(deployment.endpoint);
    if (existing !== undefined) return existing;
    const created = new SolanaRpcClient(deployment.endpoint);
    clients.set(deployment.endpoint, created);
    return created;
  };

  return Object.freeze({
    async coldState() {
      const keysOf = (storage: Readonly<{ length: number; key(index: number): string | null }> | undefined): string[] => {
        if (storage === undefined) return [];
        const keys: string[] = [];
        for (let index = 0; index < storage.length; index += 1) {
          const key = storage.key(index);
          if (key !== null) keys.push(key);
        }
        return keys;
      };
      const globals = globalThis as Readonly<{ localStorage?: Storage; sessionStorage?: Storage }>;
      return Object.freeze({
        localStorageKeys: Object.freeze(keysOf(globals.localStorage)),
        sessionStorageKeys: Object.freeze(keysOf(globals.sessionStorage)),
        cacheKeys: Object.freeze([]),
      });
    },

    async loadBakedDeployment(deploymentKey: string) {
      const deployment = options.deployments[deploymentKey];
      if (deployment === undefined) throw new Error(`no deployment is baked under the key ${JSON.stringify(deploymentKey)}`);
      return deployment;
    },

    async runStep(step: ColdClientChainStepV1, context: ColdClientContextV1): Promise<ColdClientStepResultV1> {
      const { deployment, evidence, selectedMarket } = context;
      const rpc = client(deployment);
      const programs = deployment.programs;

      switch (step) {
        case 'market.discover': {
          const enumeration = await enumerateCoreMarketAddressesV1(rpc, programs.core);
          const discovery = await inspectMarketDiscoveryV1(rpc, {
            coreProgramId: programs.core,
            registryProgramId: programs.registry,
            claimsProgramId: programs.claims,
            custodyProgramId: programs.custody,
            addresses: enumeration.addresses,
            enumeration,
          });
          const decoded = discovery.cards.filter((card) => card.status === 'decoded');
          return Object.freeze({
            step,
            status: 'ready' as const,
            reason: discovery.reason,
            observedSlot: discovery.floorSlot,
            addresses: Object.freeze(decoded.map((card) => card.address)),
            truths: Object.freeze([
              truth('Core market scan', 'authenticated', `${decoded.length} decoded Market root(s) of ${discovery.cards.length} scanned at finalized slot ${discovery.floorSlot}`),
            ]),
          });
        }

        case 'market.inspect': {
          if (selectedMarket === null) return refused(step, 'Select a market to continue.');
          const detail = await inspectMarketDetailV1(rpc, {
            coreProgramId: programs.core,
            registryProgramId: programs.registry,
            claimsProgramId: programs.claims,
            custodyProgramId: programs.custody,
            address: selectedMarket,
          });
          if (detail.card.status !== 'decoded') return refused(step, `The selected market could not be decoded: ${detail.reason}`);
          const truths: ColdClientTruthV1[] = [
            truth('Market root', 'authenticated', `phase ${detail.card.phase}, generation ${detail.card.generation}`),
            detail.realmProvenance.kind === 'refused'
              ? truth('Realm binding', 'refused', detail.realmProvenance.reason)
              : truth('Realm binding', 'authenticated', 'Collateral record loaded and matched.'),
            detail.liabilityProvenance.kind === 'refused'
              ? truth('Claims liability', 'refused', detail.liabilityProvenance.reason)
              : truth('Claims liability', 'authenticated', 'Claim balances loaded from the aggregate.'),
            detail.capabilityProvenance.kind === 'refused'
              ? truth('capability manifest', 'refused', detail.capabilityProvenance.reason)
              : truth('capability manifest', 'authenticated', 'Capability manifest matched to the market.'),
          ];
          return Object.freeze({
            step,
            status: 'ready' as const,
            reason: detail.reason,
            observedSlot: detail.floorSlot,
            addresses: Object.freeze([detail.address]),
            truths: Object.freeze(truths),
          });
        }

        case 'participant.inspect': {
          if (selectedMarket === null) return refused(step, 'Select a market to continue.');
          if (evidence.walletAddress === undefined) return unavailable(step, 'Provide a wallet address to inspect participant accounts.');
          const readiness = await inspectDirectParticipantReadinessV1(rpc, {
            market: selectedMarket,
            owner: evidence.walletAddress,
            coreProgram: programs.core,
            registryProgram: programs.registry,
            claimsProgram: programs.claims,
            tradingProgram: programs.trading,
            custodyProgram: programs.custody,
            rentProgram: programs.rent,
          });
          if (readiness.status === 'refused') return refused(step, readiness.reason);
          const verdict = readiness.status === 'ready'
            ? truth('participant standing', 'authenticated', `Position revision ${readiness.positionRevision}, ${readiness.spendableCollateralAtoms} spendable collateral atoms`)
            : truth('participant standing', 'refused', `not a participant: missing ${readiness.missing.join(' and ')}`);
          return Object.freeze({
            step,
            status: 'ready' as const,
            reason: readiness.reason,
            observedSlot: readiness.observedSlot,
            addresses: Object.freeze([readiness.coordinates.position, readiness.coordinates.collateral]),
            truths: Object.freeze([verdict]),
          });
        }

        case 'direct.inspect': {
          if (selectedMarket === null) return refused(step, 'Select a market to continue.');
          const spine = await inspectDirectTradeSpineV1(rpc, {
            marketAddress: selectedMarket,
            coreProgramId: programs.core,
            registryProgramId: programs.registry,
            tradingProgramId: programs.trading,
            claimsProgramId: programs.claims,
            owner: evidence.walletAddress ?? null,
          });
          if (spine.status === 'refused') return refused(step, spine.reason);
          const truths: ColdClientTruthV1[] = [
            truth('Direct capability', 'authenticated', `manifest entry ${spine.entryIndex}, price scale ${spine.priceScale}, fee ${spine.feeBasisPoints} bps`),
            spine.walls.length === 0
              ? truth('trade walls', 'authenticated', 'The market-state checks found no trading blockers.')
              : truth('trade walls', 'refused', spine.walls.map((wall) => wall.name).join('; ')),
          ];
          return Object.freeze({
            step,
            status: 'ready' as const,
            reason: spine.reason,
            observedSlot: spine.observedSlot,
            addresses: Object.freeze([spine.manifestRecordAddress]),
            truths: Object.freeze(truths),
          });
        }

        case 'direct.preview-unsigned': {
          if (selectedMarket === null) return refused(step, 'Select a market to continue.');
          if (evidence.walletAddress === undefined || evidence.directTicket === undefined) {
            return unavailable(step, 'Provide a wallet address and a signed counterparty ticket to preview the trade.');
          }
          const spine = await inspectDirectTradeSpineV1(rpc, {
            marketAddress: selectedMarket,
            coreProgramId: programs.core,
            registryProgramId: programs.registry,
            tradingProgramId: programs.trading,
            claimsProgramId: programs.claims,
            owner: evidence.walletAddress,
          });
          if (spine.status === 'refused') return refused(step, spine.reason);
          if (spine.outcomeCount === null) return refused(step, 'The market’s outcome count is unavailable.');
          const readiness = await inspectDirectParticipantReadinessV1(rpc, {
            market: selectedMarket,
            owner: evidence.walletAddress,
            coreProgram: programs.core,
            registryProgram: programs.registry,
            claimsProgram: programs.claims,
            tradingProgram: programs.trading,
            custodyProgram: programs.custody,
            rentProgram: programs.rent,
          });
          if (readiness.status !== 'ready') {
            return refused(step, readiness.status === 'refused' ? readiness.reason : `Participant accounts are missing: ${readiness.missing.join(' and ')}`);
          }
          const ticket = decodeDirectIntentTicketV1(evidence.directTicket);
          const replay = await inspectDirectMakerNonceV1(rpc, {
            tradingProgram: programs.trading,
            market: selectedMarket,
            generation: BigInt(spine.generation),
            maker: evidence.walletAddress,
          });
          const plan = planDirectCrossingV1({
            route: {
              tradingProgram: programs.trading,
              market: selectedMarket,
              generation: BigInt(spine.generation),
              outcomeCount: spine.outcomeCount,
              priceScale: spine.priceScale,
              feeBasisPoints: spine.feeBasisPoints,
            },
            ticket,
            takerAddress: evidence.walletAddress,
            takerReplay: replay,
            takerCollateralAccount: readiness.coordinates.collateral,
            desiredFill: ticket.intent.maximumFill,
            clockSlot: BigInt(replay.observedSlot),
          });
          const previewText = JSON.stringify({
            fill: plan.fill.toString(),
            executionPrice: plan.executionPrice.toString(),
            takerSide: plan.takerSide,
            grossCollateral: plan.preview.grossCollateral.toString(),
            buyerFee: plan.preview.buyerFee.toString(),
            sellerFee: plan.preview.sellerFee.toString(),
          });
          return Object.freeze({
            step,
            status: 'ready' as const,
            reason: `Unsigned ${plan.takerSide} preview: ${plan.fill} claim atoms at signed price ${plan.executionPrice}.`,
            observedSlot: replay.observedSlot,
            addresses: Object.freeze([readiness.coordinates.collateral]),
            truths: Object.freeze([
              truth('crossing arithmetic', 'authenticated', 'Trade amounts and fees calculated in integer atoms.'),
            ]),
            artifact: Object.freeze({
              kind: 'unsigned-preview' as const,
              digest: await digestText(previewText),
              byteLength: previewText.length,
            }),
          });
        }

        case 'resolution.inspect': {
          if (selectedMarket === null) return refused(step, 'Select a market to continue.');
          const detail = await inspectMarketDetailV1(rpc, {
            coreProgramId: programs.core,
            registryProgramId: programs.registry,
            claimsProgramId: programs.claims,
            custodyProgramId: programs.custody,
            address: selectedMarket,
          });
          if (detail.card.status !== 'decoded') return refused(step, `The selected market could not be decoded: ${detail.reason}`);
          const settlement = detail.card.settlement;
          const verdict = settlement.status === 'terminal'
            ? truth('terminal settlement', 'authenticated', `winning claim ${settlement.winner}`)
            : truth('terminal settlement', 'refused', 'The market has no terminal settlement receipt.');
          return Object.freeze({
            step,
            status: 'ready' as const,
            reason: `phase ${detail.card.phase} at finalized slot ${detail.floorSlot}`,
            observedSlot: detail.floorSlot,
            truths: Object.freeze([verdict]),
          });
        }

        case 'redeem.inspect': {
          if (selectedMarket === null) return refused(step, 'Select a market to continue.');
          if (evidence.walletAddress === undefined) return unavailable(step, 'Provide a wallet address to inspect redemption readiness.');
          const state = await inspectClaimsCustodyReplayV1(rpc, {
            marketAddress: selectedMarket,
            claimsProgramId: programs.claims,
            custodyProgramId: programs.custody,
            registryProgramId: programs.registry,
            payer: evidence.walletAddress,
          });
          if (state.status === 'refused') return refused(step, state.reason);
          const verdict = state.status === 'exists'
            ? truth('Claims replay', 'authenticated', 'Payment record already exists.')
            : truth('Claims replay', 'authenticated', 'Payment record needs creation. The creation transaction is ready to sign.');
          return Object.freeze({
            step,
            status: 'ready' as const,
            reason: state.status === 'exists' ? 'The payment record is ready for payouts.' : state.note,
            observedSlot: state.observedSlot,
            truths: Object.freeze([verdict]),
          });
        }

        case 'redeem.prepare-unsigned': {
          if (evidence.walletAddress === undefined || evidence.redeemPlan === undefined) {
            return unavailable(step, 'Provide a wallet address and a prepared payout file.');
          }
          const manifest = importRustWalletTerminalPayoutArtifactV3(evidence.redeemPlan);
          const prepared = await prepareWalletTerminalPayoutV3(rpc, manifest, evidence.walletAddress);
          const messageBytes = prepared.transaction.message.serialize();
          return Object.freeze({
            step,
            status: 'ready' as const,
            reason: `Prepared an unsigned payout message (${messageBytes.length} bytes).`,
            observedSlot: prepared.report.observedSlot,
            truths: Object.freeze([
              truth('payout plan', 'authenticated', 'Payout plan checked against finalized account state.'),
            ]),
            artifact: Object.freeze({
              kind: 'unsigned-transaction' as const,
              digest: hex(await sha256(messageBytes)),
              byteLength: messageBytes.length,
            }),
          });
        }

        case 'retirement.inspect': {
          if (selectedMarket === null) return refused(step, 'Select a market to continue.');
          const detail = await inspectMarketDetailV1(rpc, {
            coreProgramId: programs.core,
            registryProgramId: programs.registry,
            claimsProgramId: programs.claims,
            custodyProgramId: programs.custody,
            address: selectedMarket,
          });
          if (detail.card.status !== 'decoded') return refused(step, `The selected market could not be decoded: ${detail.reason}`);
          const retirement = await inspectAggregateRetirementV1(rpc, {
            coreProgramId: programs.core,
            claimsProgramId: programs.claims,
            marketAddress: selectedMarket,
            marketGeneration: String(detail.card.generation),
            marketPhase: detail.card.phase,
            minimumContextSlot: detail.floorSlot,
          });
          const verdict = retirement.status === 'not-admitted'
            ? truth('retirement seam', 'refused', retirement.reason)
            : truth('retirement seam', retirement.status === 'refused' ? 'refused' : 'authenticated', retirement.reason);
          return Object.freeze({
            step,
            status: 'ready' as const,
            reason: retirement.reason,
            observedSlot: retirement.observedSlot,
            addresses: Object.freeze([retirement.aggregateAddress]),
            truths: Object.freeze([verdict]),
          });
        }
      }
    },
  });
}
