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
  // COHORT-15, deployed 2026-09-04 from commit 1cae26fd6 and byte-identical on
  // read-back three ways: the dump compares equal per role, the byte count
  // matches, and a hostile decode of each live ProgramData answers MATCH for
  // all seven. Devnet is disposable by ruling: each cohort is a full redeploy
  // with fresh identities and the previous one is abandoned in place and then
  // CLOSED, which returns its rent to pay for the next. These ids are not
  // permanent and nothing here should say they are.
  //
  // NOT TYPED, AND NOT MOVED BY HAND EITHER. Each id is the `program_id` the
  // sealed plan `plan-seal.json` names for its role, and the two facts beside
  // it below are read off the chain; `scripts/derive-deployment-manifest.mjs`
  // is what performs both and what wrote these lines. That script exists
  // because this table has now shipped a CLOSED cohort twice -- `0f1d75b27`
  // fixed it for cohort-8 and the second C-16 walk found cohort-14's seven
  // ProgramData accounts reading AccountNotFound the morning after cohort-15
  // landed -- and both times the derivation that could have said so had been
  // performed in a scratch directory and thrown away. It refuses to emit a row
  // for a role whose ProgramData is vacant, so a closed cohort cannot be
  // written here at all: proven against cohort-14's own sealed plan, which it
  // refuses by naming all seven vacant addresses.
  programs: Object.freeze({
    registry: '8ci2LojrUPtVZoByaAmoNCj8244DTdxkUkHRVA9JiHdV',
    rent: 'EQ61nrN71xaTsDMSrrH9FzMeqc27ghRex9abyjKu8vC4',
    custody: '4sCbUV6f8iZaaVDkp3qAJtxiLY1cfBcKrVc5wpAFjpzM',
    resolution: '24AkUjtXg61La45u7KTge8u4dKpVqkzirmzycVyckFgn',
    claims: '595PExHje1GnbryQtXvpxSfxK72snpMcrG1Sj8VpPLEe',
    trading: '3gBSSjYwSC4phutpGKRkMhrnCDVzHu6kfQ3L4jLf2UmG',
    core: '7hGerMC6Wj742FVTyiF9PhRnGSBzbee7TMZ6sUytsmFr',
  }),
  // Bootstrap hint, GENERATED — do not hand-edit. Regenerate with
  // `node packages/dclutch-sdk/scripts/derive-activation-hint.mjs --write`.
  //
  // The one cache of those the Registry owns whose five pinned deployment
  // slots equalled the five live ProgramData slots in a single reading.
  // Release set 9895faee8f7f6a1926df18302f1b003afcf4b6c56518ba7bba2614c86eea8a22,
  // pinning Core at deployment slot 492746271.
  // A session follows past this when it ages out; a reader cannot.
  activationCache: '3hFTU9ka7fryKrVY7s8Lm5ZMCHsnq5bxEGcgSCd6TSiu',
  provenance: 'Cohort-15’s devnet substrate, deployed 2026-09-04 from commit 1cae26fd6 and byte-identical on read-back; cohort-14 was closed the same day and the 42.20 SOL its rent returned paid for this one, which cost 42.55 against a 42.55 projection. Its founding, its checked seal and the Market account itself all carry one release-set identity — and the third of those is the one a browser can check for itself, because it is 32 bytes at offset 208 of a Market this site reads. It is also the first cohort whose statistic carries the source-to-result scale factor, which is why a market page here can now reproduce the cell the chain committed instead of assuming the shift is zero.',
});

/**
 * Cohort-15's ProgramData addresses and deployment slots.
 *
 * READ, not copied from a record, and not derived either: each address is the
 * 32 bytes the Program account itself names at offset 4, and each slot is the
 * u64 at offset 4 of that ProgramData account's own Loader-v3 header. Read
 * finalized at slot 492,944,410 -- hours after the deploy rather than the
 * minute after it, which is the stronger reading: these slots are what the
 * chain still says, not what the deploy reported. They run 492,745,516 through
 * 492,746,271, one per role in the order the seven were deployed, and they
 * reproduce COHORT15_DEPLOYED_SEALED_FOUNDED_CAPTURED_2026_09_04.md §3 without
 * reading it.
 *
 * THAT LAST CHECK IS THE ONE THAT MATTERS. A closed program keeps its 36-byte
 * Program account, its executable flag and the ProgramData address it names --
 * only the ProgramData itself is gone. Cohort-8's rows survived here after its
 * close on 2026-09-01, and cohort-14's survived here after its close on
 * 2026-09-04, both because every gate asked the Program account, which was
 * alive, and none asked the account that holds the code.
 * `deployments.live.test.ts` asks, `deploymentLiveness.live.test.ts` makes the
 * refusal a gate rather than an env-gated skip, and the derivation that wrote
 * these rows refuses to emit one for a role whose ProgramData is vacant.
 */
export const DEVNET_PROGRAM_EVIDENCE_V1: Readonly<Record<ProtocolRoleV1, ProgramEvidenceV1>> = Object.freeze({
  registry: Object.freeze({ programData: '2Xvmiz5kBseLAeCCgJsuJmK7UHQ1fzzbaSm7FqHF8Eat', deploymentSlot: '492745516' }),
  rent: Object.freeze({ programData: '6jqwvw8jPSqomLLir3rNohLHAm5awrkcx4b3WvjxVmSm', deploymentSlot: '492745574' }),
  custody: Object.freeze({ programData: 'D7X4Wximtu9BwHXdrWgWpXkjMCFS7xPtV3UevgMVXhKB', deploymentSlot: '492745658' }),
  resolution: Object.freeze({ programData: 'B8N72QJgwKypTfgr9TsLDtKWjrCFeJ9n7FjsjV1xgB71', deploymentSlot: '492745773' }),
  claims: Object.freeze({ programData: '7RLeuS5vDph1VpgG3NGYwZnjnTxvwmwJE2FfgWNUDWPG', deploymentSlot: '492745957' }),
  trading: Object.freeze({ programData: 'CnER99tNi7gz24g7sFEzoUoxa9Q3cZe8FMJTdu9iEfxG', deploymentSlot: '492746142' }),
  core: Object.freeze({ programData: 'DrT1XF1qDgbTs8WCkhk1yjxFyehPSBM1VQrrowipmDyt', deploymentSlot: '492746271' }),
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
 * or its infrastructure plan (`dclutch-local-successor-infrastructure-plan-v3`,
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
  // The plan schema moved to v3 when initialization began committing both
  // infrastructure profiles in one instruction. A v2 plan is named rather than
  // lumped in with a foreign document: what an operator holds is a stale plan,
  // not the wrong kind of file, and the remedy is to re-run `prepare`.
  if (schema === 'dclutch-local-successor-infrastructure-plan-v2') {
    throw new Error('this is a retired dclutch-local-successor-infrastructure-plan-v2 document; re-run `prepare` to emit a dclutch-local-successor-infrastructure-plan-v3 plan');
  }
  if (schema !== 'dclutch-local-successor-run-spec-v2' && schema !== 'dclutch-local-successor-infrastructure-plan-v3') {
    throw new Error('the pasted document is neither a successor run spec (dclutch-local-successor-run-spec-v2) nor an infrastructure plan (dclutch-local-successor-infrastructure-plan-v3)');
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
