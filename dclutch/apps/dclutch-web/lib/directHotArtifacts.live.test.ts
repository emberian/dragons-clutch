import { appendFileSync } from 'node:fs';

import { describe, expect, it } from 'vitest';

import { hex, sha256 } from './bytes';
import { decodeCapabilityManifestV1 } from './capabilityManifest';
import { DEVNET_DEPLOYMENT_V1 } from './deployments';
import {
  decodeDirectDescriptorV4,
  decodeDirectProgramSetV2,
  validateDirectSignedRequestProfileV2,
} from './directHotChain';
import { CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1 } from './generated/coreFound';
import * as DirectAbi from './generated/directInlineV3';
import { decodeMarketCoreStateV2 } from './marketCoreV2';
import { deriveFinalizedRecordAddressesV1 } from './releaseRegistry';
import { PUBLIC_DEVNET_CUT_V1 } from './publicCutStaging';
import { SolanaRpcClient } from './rpc';

const report = (line: string) => {
  const out = process.env.DCLUTCH_LIVE_REPORT;
  if (out !== undefined) appendFileSync(out, `${line}\n`);
};

/**
 * The Market whose descriptor artifacts this walks.
 *
 * This pinned cohort-8's `8Xky2yx3wB…`, and every record it reaches is derived
 * from `DEVNET_DEPLOYMENT_V1.programs.registry` -- so the coordinate and the
 * registry it is resolved against came from different cohorts, and the walk
 * refused at the first `body()` with "capability manifest record is absent".
 * A finalized record lives at a PDA of the REGISTRY that published it; naming a
 * market from one cohort and a registry from another asks for a record that was
 * never written. It reads the cut's market now, so the two always agree.
 */
const MARKET = PUBLIC_DEVNET_CUT_V1.market ?? '';

const live = process.env.DCLUTCH_LIVE_DEVNET === '1' && MARKET !== '' ? it : it.skip;

/**
 * The artifact validators the full Hot route runs, against the real records.
 *
 * `inspectDirectTradeSpineV1` stops at the descriptor, so the spine reaching
 * `tradable` does not exercise `validateDirectSignedRequestProfileV2` or the
 * per-artifact content binding that `inspectDirectHotRouteV3` performs once an
 * operator manifest exists. Those validators carry pins of their own, and one
 * of them was wrong in the same way the effect schema was: the InlineOrdinary
 * item scalar stride was written as a literal 0 while the emitter carries it as
 * `DIRECT_ORDINARY_ITEM_SCALAR_STRIDE_V3 = 2`.
 *
 * Reaching those validators needs no manifest — every artifact is addressable
 * from the descriptor's own (schema, digest) pair — so this walks there from
 * the Market alone. Reads only. Gated on `DCLUTCH_LIVE_DEVNET=1`.
 */
describe('live devnet Direct Hot artifact validators', () => {
  live('binds and validates every descriptor-named artifact the featured Market names', async () => {
    const client = new SolanaRpcClient(process.env.DCLUTCH_LIVE_ENDPOINT ?? DEVNET_DEPLOYMENT_V1.endpoint);
    const registry = DEVNET_DEPLOYMENT_V1.programs.registry;
    const floor = await client.finalizedSlot();
    const body = async (schema: Uint8Array, digest: Uint8Array, field: string) => {
      const derived = deriveFinalizedRecordAddressesV1(registry, schema, digest);
      const observation = await client.multipleAccounts([derived.record], floor);
      const account = observation.accounts[0]?.account ?? null;
      if (account === null) throw new Error(`${field} record is absent at ${derived.record}`);
      return Object.freeze({ address: derived.record, data: account.data });
    };

    const marketObservation = await client.multipleAccounts([MARKET], floor);
    const marketAccount = marketObservation.accounts[0]?.account ?? null;
    if (marketAccount === null) throw new Error(`no account at ${MARKET}`);
    const market = decodeMarketCoreStateV2(MARKET, marketAccount.data);
    const manifestDigest = Uint8Array.from((market.identity.capabilityManifestId.match(/../g) ?? []).map((v) => Number.parseInt(v, 16)));
    const manifestRecord = await body(CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1, manifestDigest, 'capability manifest');
    const entry = decodeCapabilityManifestV1(manifestRecord.data)
      .find((e) => hex(e.kind) === hex(DirectAbi.DIRECT_SUCCESSOR_KIND_ID_V3));
    if (entry === undefined) throw new Error('this Market lists no Direct successor entry');
    const setRecord = await body(DirectAbi.CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2, entry.programSet, 'CapabilityProgramSetV2');
    const selected = decodeDirectProgramSetV2(setRecord.data);
    const descriptorRecord = await body(selected.schema, selected.program, 'CapabilityProgramV4 descriptor');

    // The conjunct that refused a reader on clutch.dregg.pro.
    const descriptor = decodeDirectDescriptorV4(descriptorRecord.data);
    report(`descriptor accepted at ${descriptorRecord.address}`);

    const named = Object.freeze([
      ['accountProfile', descriptor.accountProfile],
      ['requestProfile', descriptor.requestProfile],
      ['lifecycle', descriptor.lifecycle],
      ['strategy', descriptor.strategy],
      ['transition', descriptor.transition],
      ['effect', descriptor.effect],
    ] as const);
    for (const [label, reference] of named) {
      const record = await body(reference.schema, reference.program, label);
      // Rust does not pin these program ids; it requires the record the
      // descriptor names to hash to the digest the descriptor names.
      expect(hex(await sha256(record.data)), `${label} content identity`).toBe(hex(reference.program));
      report(`${label} bound: ${record.data.length}B at ${record.address}`);
    }

    const requestProfile = await body(descriptor.requestProfile.schema, descriptor.requestProfile.program, 'RequestProfile V2');
    validateDirectSignedRequestProfileV2(requestProfile.data);
    report(`RequestProfile V2 accepted: ${requestProfile.data.length}B at ${requestProfile.address}`);
  }, 180_000);
});
