/**
 * `dclutch markets ls` / `dclutch markets show <address>`.
 *
 * Both are pure reads through the SDK's discovery projection: enumerate the
 * Core program's market headers at one finalized floor, decode each into a
 * card, and say plainly which decoded and which the browser-grade decoder
 * refused (a refused card is the client working, not a rendering gap).
 */
import { inspectMarketDetailV1 } from '@dclutch/sdk/marketDetail';
import {
  enumerateCoreMarketAddressesV1,
  inspectMarketDiscoveryV1,
  provenanceChipV1,
  type MarketDiscoveryCardV1,
} from '@dclutch/sdk/marketDiscovery';

import { bindDeploymentIdentity, optionalProgramId, programId, rpcClient, type CliContext } from '../context';
import { deploymentProvenanceLineV1 } from '../deployment';
import { block, type Io } from '../output';

function cardLine(card: MarketDiscoveryCardV1): string {
  if (card.status === 'refused') return `${card.address}  REFUSED  ${card.refusal}`;
  const chip = provenanceChipV1(card.provenance);
  return `${card.address}  gen ${card.generation}  ${card.phase}  ${chip}`;
}

export async function marketsLs(context: CliContext, io: Io): Promise<number> {
  const client = rpcClient(context);
  const admission = await bindDeploymentIdentity(context, client, 'markets ls');
  const coreProgramId = programId(context, 'core');
  const enumeration = await enumerateCoreMarketAddressesV1(client, coreProgramId);
  if (enumeration.mode === 'refused') {
    io.err(`market enumeration refused: ${enumeration.reason}`);
    return 1;
  }
  const known = [...new Set([...enumeration.addresses, ...context.session.markets])];
  const discovery = await inspectMarketDiscoveryV1(client, {
    coreProgramId,
    registryProgramId: optionalProgramId(context, 'registry'),
    claimsProgramId: optionalProgramId(context, 'claims'),
    custodyProgramId: optionalProgramId(context, 'custody'),
    addresses: known,
    enumeration,
  });
  if (context.json) {
    io.out(JSON.stringify(discovery, null, 2));
    return 0;
  }
  io.out(`markets under Core ${coreProgramId} at finalized slot ${discovery.floorSlot} (${enumeration.note})`);
  if (admission !== null && context.deployment !== null) {
    io.out(`  ${deploymentProvenanceLineV1(context.deployment)}; endpoint proved genesis ${admission.genesisHash}`);
  }
  if (discovery.cards.length === 0) io.out('  no market accounts found');
  for (const card of discovery.cards) io.out(`  ${cardLine(card)}`);
  return 0;
}

export async function marketsShow(context: CliContext, io: Io, address: string): Promise<number> {
  const client = rpcClient(context);
  await bindDeploymentIdentity(context, client, 'markets show');
  const detail = await inspectMarketDetailV1(client, {
    address,
    coreProgramId: programId(context, 'core'),
    registryProgramId: optionalProgramId(context, 'registry'),
    claimsProgramId: optionalProgramId(context, 'claims'),
    custodyProgramId: optionalProgramId(context, 'custody'),
  });
  if (context.json) {
    io.out(JSON.stringify(detail, null, 2));
    return 0;
  }
  const card = detail.card;
  if (card.status === 'refused') {
    io.out(`${card.address} REFUSED at slot ${card.observedSlot}: ${card.refusal}`);
    return 1;
  }
  io.out(`market ${card.address} at finalized slot ${card.observedSlot}`);
  block(io, [
    ['phase', detail.phaseMeaning === null ? card.phase : `${card.phase} — ${detail.phaseMeaning}`],
    ['generation', card.generation],
    ['provenance', provenanceChipV1(card.provenance)],
    ['outstanding capabilities', card.outstandingCapabilities],
    ['market id', card.identity.marketId],
    ['registry program', card.identity.registryProgram],
    ['collateral', describeSection(card.collateral)],
    ['liability', describeSection(card.liability)],
    ['hoard', describeSection(card.hoard)],
  ]);
  if (card.bindings.length > 0) {
    io.out('  bindings:');
    for (const binding of card.bindings) io.out(`    ${JSON.stringify(binding)}`);
  }
  return 0;
}

function describeSection(section: unknown): string {
  if (typeof section === 'object' && section !== null && 'kind' in section && (section as { kind: unknown }).kind === 'refused') {
    return `refused: ${String((section as { reason?: unknown }).reason ?? 'unstated')}`;
  }
  return JSON.stringify(section);
}
