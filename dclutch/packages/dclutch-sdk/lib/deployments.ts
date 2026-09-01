import { PublicKey } from '@solana/web3.js';

import { SOLANA_DEVNET_GENESIS_HASH_V1 } from './rpc';

/**
 * The deployment manifest — the protocol addresses this app ships KNOWING.
 *
 * The product inversion this module carries: a visitor should land on content,
 * not on a form asking which chain the protocol lives on. dClutch has exactly
 * two deployments whose coordinates are public facts, so those coordinates are
 * baked here and every surface reads them from the active deployment instead
 * of asking. The one legitimate "bring your own" case — an operator pointing
 * at their own validator — lives behind the cluster picker's Custom entry, and
 * nowhere else.
 *
 * Provenance, per cluster:
 *
 * - `devnet` — DEPLOY-1's durable substrate, deployed 2026-08-27 and byte-
 *   verified buffer-side and dump-side (`docs/evidence/DEPLOY_1.md` §2, "The
 *   substrate — PERMANENT ADDRESSES"). Mutable under the retained deployer
 *   authority per decision 0012; a moved deployment slot is named
 *   `ReleaseSupersededByUpgrade` by the release layer, so baking these
 *   addresses does not assert immutability — the slot-pinned admission still
 *   decides what counts as the released artifact.
 * - `local` — the gauntlet campaign's fixed-seed layout. Tier-1 campaigns
 *   derive every signing key from the hashed preimage
 *   `dclutch/gauntlet/tier1/keypair-seed/v1` (tools/gauntlet/run.sh), so every
 *   local campaign deploys the same seven program addresses; the values here
 *   are pinned against `fixtures/live-open-market.json`, bytes captured off a
 *   real campaign validator.
 *
 * `lib/deployments.test.ts` pins both tables; `lib/deployments.live.test.ts`
 * (gated on `DCLUTCH_LIVE_DEVNET=1`) verifies each devnet address against the
 * public cluster: the account must exist, be executable, and be owned by the
 * upgradeable loader.
 */

export const PROTOCOL_ROLES_V1 = ['registry', 'rent', 'custody', 'resolution', 'claims', 'trading', 'core'] as const;

export type ProtocolRoleV1 = (typeof PROTOCOL_ROLES_V1)[number];

/** One sentence per role, for surfaces that introduce the programs by name. */
export const PROTOCOL_ROLE_MEANING_V1: Readonly<Record<ProtocolRoleV1, string>> = Object.freeze({
  registry: 'Finalized records and release activation — the content-addressed record layer.',
  rent: 'Rent credits and beneficiaries for protocol accounts.',
  custody: 'Collateral custody — the Hoards that physically back every liability.',
  resolution: 'Resolution — how a market learns its outcome, oracle receivers included.',
  claims: 'Claim liabilities and Positions — who is owed what, exactly.',
  trading: 'Trading — the routes that move claims against collateral.',
  core: 'Market roots — founding, phase, generation, and the identities a market commits to.',
});

export type ClusterIdV1 = 'devnet' | 'local' | 'custom';

export type DeploymentV1 = Readonly<{
  cluster: ClusterIdV1;
  /** Short human name for the picker: "Devnet", "Local", "Custom". */
  label: string;
  endpoint: string;
  /** Expected genesis hash, or null when the chain's identity varies (local ledgers, custom). */
  genesisHash: string | null;
  /** The seven role program ids. */
  programs: Readonly<Record<ProtocolRoleV1, string>>;
  /**
   * A BOOTSTRAP HINT for the Registry activation cache — never the answer.
   *
   * A cohort activates a new release set, which mints a new cache at a new PDA,
   * and superseded caches are never deleted. So this address ages the moment a
   * cohort lands, and it will age again. `openReleaseBoundSessionV1` treats it
   * as a hint it may follow past: it reads the hint, and when the hint's pinned
   * deployment slots no longer match the live programs it discovers the current
   * cache from the chain and binds to that instead. A stale value here costs a
   * reader accuracy, not a session.
   *
   * Keep it honest anyway, and generate it rather than typing it:
   * `node packages/dclutch-sdk/scripts/derive-activation-hint.mjs --write`.
   */
  activationCache: string | null;
  /** One sentence: where these addresses come from. */
  provenance: string;
}>;

/** Per-role deployment evidence beyond the id — devnet only, from DEPLOY_1.md §2. */
export type ProgramEvidenceV1 = Readonly<{
  programData: string;
  deploymentSlot: string;
}>;

export const DEVNET_DEPLOYMENT_V1: DeploymentV1 = Object.freeze({
  cluster: 'devnet',
  label: 'Devnet',
  endpoint: 'https://api.devnet.solana.com',
  genesisHash: SOLANA_DEVNET_GENESIS_HASH_V1,
  programs: Object.freeze({
    registry: 'Hies39GBowHUMZw9rVCfaDTAXNorkQqMGKnukY2MD4Qj',
    rent: 'DgfYeuorJUmnktxgCmUXy65f6MFBGcc1aMQoauxoJCY3',
    custody: '34dhZkSUUhhFPL98KpWXaoG9aMs3EinZo5xN5epJEgGH',
    resolution: '2GHmxBawHTmwDRzqXuqdeC9A9Gj2HzucRd29wGpfgzmd',
    claims: '85hwTeQGabwFRs71Hafvngb1UmHb6dQoumBv3VV4epNN',
    trading: '5ywjTNdo6DGTe7bC8p9CgFYWFrBNePx61xeXp8Cdhbkk',
    core: 'HezRkcMGTZ5EY2LZk3i4uJbrAjUSDcamAw9B5v68z33N',
  }),
  // Bootstrap hint, GENERATED — do not hand-edit. Regenerate with
  // `node packages/dclutch-sdk/scripts/derive-activation-hint.mjs --write`.
  //
  // The one cache of those the Registry owns whose five pinned deployment
  // slots equalled the five live ProgramData slots in a single reading.
  // Release set d5aaadea2435978604d93c0e48af0e44547ec54b69681585f47f185ef530a2fa,
  // pinning Core at deployment slot 490106442.
  // A session follows past this when it ages out; a reader cannot.
  activationCache: '69d1MKP4PaPVDFankLfnzeHBugoVBjPCDm7PEHParRF6',
  provenance: 'DEPLOY-1’s permanent devnet substrate, deployed 2026-08-27 and byte-verified (docs/evidence/DEPLOY_1.md §2).',
});

/** DEPLOY_1.md §2's ProgramData addresses and deployment slots, verbatim. */
export const DEVNET_PROGRAM_EVIDENCE_V1: Readonly<Record<ProtocolRoleV1, ProgramEvidenceV1>> = Object.freeze({
  registry: Object.freeze({ programData: 'ENRSwrUEymWaXyrNtyD4QXXXk3tsTmcTGPTUFvnpsRVz', deploymentSlot: '489100383' }),
  rent: Object.freeze({ programData: '78MW6W4iPzBVLceAwTL51CtyLcpcFM2iGVMDbzZtUFmy', deploymentSlot: '489100242' }),
  custody: Object.freeze({ programData: 'EhB7hHJ7vsCW3nCeqbxbJrn5Jsi6gbqwpVhoLMPZ8ENf', deploymentSlot: '489100460' }),
  resolution: Object.freeze({ programData: '2QFBQJdLBXAnJWTVK8KeeUtWZEFhQqqN2CbkrWjMjY6f', deploymentSlot: '489100560' }),
  claims: Object.freeze({ programData: '4La2511ddSxUcAQfdhKvEeGEasih3TStbQWVFEQKd34j', deploymentSlot: '489100803' }),
  trading: Object.freeze({ programData: 'AE1cWbCvXedE23XH3otSxvDQ7xVx7WLNMYDc8y8rqkrn', deploymentSlot: '489100942' }),
  core: Object.freeze({ programData: 'AD6mb5SP6yqc5GFexf3xhpr1wKaZQhS7Hrt41iZhKxaN', deploymentSlot: '489100672' }),
});

export const LOCAL_DEPLOYMENT_V1: DeploymentV1 = Object.freeze({
  cluster: 'local',
  label: 'Local',
  endpoint: 'http://127.0.0.1:8899',
  genesisHash: null,
  programs: Object.freeze({
    registry: '87syw3eBN6nrXYT5RkkRSBKcRbciD1sg5X3R3hu7mh7e',
    rent: '2gUDaLHEAdfs44vDWyjj3cCkJDVTEjrBPntDKHQxD9U8',
    custody: '7H6H9NabHSQtLVpiAaMKgmuRV5VVWD5VPGzpFYNGDbiD',
    resolution: 'EBH6zun6a9PRcQtAaSHjTCArqUd8ArRGMciRHmJFu34x',
    claims: '9fAcEn8fhVkmJmhx4xFfquNshTryNC6cQ9ieKwAPBMY6',
    trading: 'H3yrZV5ekNATUhhydami88bahXYtrP5fZig31LEU8UM8',
    core: '2rJGzuF2AduNJCc2td1y87ApUk8NhiCUGhsKCNRqhd8o',
  }),
  activationCache: null,
  provenance: 'The gauntlet campaign’s fixed-seed layout: every tier-1 campaign derives these same addresses from one pinned seed preimage.',
});

export const DEFAULT_DEPLOYMENT_V1: DeploymentV1 = DEVNET_DEPLOYMENT_V1;

/** Labels for the seven role programs of one deployment, keyed by address. */
export function deploymentProgramLabelsV1(deployment: DeploymentV1): Readonly<Record<string, string>> {
  const labels: Record<string, string> = {};
  for (const role of PROTOCOL_ROLES_V1) {
    labels[deployment.programs[role]] = `dClutch ${role[0].toUpperCase()}${role.slice(1)} · ${deployment.label}`;
  }
  return Object.freeze(labels);
}

function canonicalAddress(value: unknown, field: string): string {
  if (typeof value !== 'string' || value.length === 0) throw new Error(`${field} is required`);
  let canonical: string;
  try {
    canonical = new PublicKey(value).toBase58();
  } catch {
    throw new Error(`${field} is not a Solana address`);
  }
  if (canonical !== value) throw new Error(`${field} must be canonical base58 text`);
  return canonical;
}

/**
 * Read an operator document into the Custom form: the successor bootstrap's
 * run spec (`dclutch-local-successor-run-spec-v2`, which names its RPC URL)
 * or its infrastructure plan (`dclutch-local-successor-infrastructure-plan-v2`,
 * program identities only). This fills the draft; admission is still
 * `parseCustomDeploymentV1`, so an imported document cannot smuggle anything
 * the hand-typed form could not.
 */
export function importDeploymentDocumentV1(text: string): Readonly<{
  endpoint: string | null;
  programs: Readonly<Record<ProtocolRoleV1, string>>;
}> {
  let raw: unknown;
  try {
    raw = JSON.parse(text);
  } catch {
    throw new Error('the pasted document is not JSON');
  }
  if (raw === null || typeof raw !== 'object' || Array.isArray(raw)) throw new Error('the pasted document is not one JSON object');
  const record = raw as Record<string, unknown>;
  const schema = record.schema;
  if (schema !== 'dclutch-local-successor-run-spec-v2' && schema !== 'dclutch-local-successor-infrastructure-plan-v2') {
    throw new Error('the pasted document is neither a successor run spec (dclutch-local-successor-run-spec-v2) nor an infrastructure plan (dclutch-local-successor-infrastructure-plan-v2)');
  }
  const roleKey: Readonly<Record<ProtocolRoleV1, string>> = Object.freeze({
    registry: 'registry', core: 'core', claims: 'claims', trading: 'trading',
    resolution: 'resolution', custody: 'custody', rent: 'rent_credit',
  });
  const programs = {} as Record<ProtocolRoleV1, string>;
  const missing: string[] = [];
  for (const role of PROTOCOL_ROLES_V1) {
    const entry = record[roleKey[role]];
    const id = entry !== null && typeof entry === 'object' && !Array.isArray(entry)
      ? (entry as Record<string, unknown>).program_id
      : undefined;
    if (typeof id !== 'string' || id === '') {
      missing.push(roleKey[role]);
      continue;
    }
    programs[role] = canonicalAddress(id, `${roleKey[role]} program`);
  }
  if (missing.length > 0) {
    throw new Error(`the document names no program for: ${missing.join(', ')} — a browser deployment needs all seven roles`);
  }
  const rpc = record.rpc_url;
  const endpoint = typeof rpc === 'string' && rpc.trim() !== '' ? rpc.trim() : null;
  return Object.freeze({ endpoint, programs: Object.freeze(programs) });
}

/**
 * Validate one custom deployment (the picker's Custom form, or its stored
 * JSON). Every field is checked the way the RPC layer would check it, so a
 * stored deployment can never smuggle a malformed address into a derivation.
 */
export function parseCustomDeploymentV1(raw: unknown): DeploymentV1 {
  if (raw === null || typeof raw !== 'object' || Array.isArray(raw)) throw new Error('custom deployment must be one JSON object');
  const record = raw as Record<string, unknown>;
  const endpointText = typeof record.endpoint === 'string' ? record.endpoint.trim() : '';
  const url = new URL(endpointText);
  if (url.protocol !== 'http:' && url.protocol !== 'https:') throw new Error('custom endpoint must use http or https');
  const programsRaw = record.programs;
  if (programsRaw === null || typeof programsRaw !== 'object' || Array.isArray(programsRaw)) throw new Error('custom deployment must carry a programs object');
  const programs = {} as Record<ProtocolRoleV1, string>;
  for (const role of PROTOCOL_ROLES_V1) {
    programs[role] = canonicalAddress((programsRaw as Record<string, unknown>)[role], `${role} program`);
  }
  if (new Set(Object.values(programs)).size !== PROTOCOL_ROLES_V1.length) throw new Error('the seven role programs must be distinct addresses');
  return Object.freeze({
    cluster: 'custom',
    label: 'Custom',
    endpoint: url.toString(),
    genesisHash: null,
    programs: Object.freeze(programs),
    activationCache: record.activationCache === undefined || record.activationCache === null || record.activationCache === ''
      ? null
      : canonicalAddress(record.activationCache, 'activation cache'),
    provenance: 'Your own deployment, entered through the cluster picker and stored only in this browser.',
  });
}
