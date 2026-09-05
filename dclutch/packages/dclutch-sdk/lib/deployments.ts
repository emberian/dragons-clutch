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
 * - `devnet` — the CURRENT cohort's substrate, redeployed whole at fresh
 *   identities each time and byte-verified dump-side per program
 *   (`docs/evidence/COHORT16_DEPLOYED_SEALED_2026_09_05.md` §2). Mutable under
 *   the retained deployer authority per decision 0012; a moved deployment slot
 *   is named `ReleaseSupersededByUpgrade` by the release layer, so baking these
 *   addresses does not assert immutability — the slot-pinned admission still
 *   decides what counts as the released artifact. Since cohort-16 there are
 *   EIGHT of them: the seven checked roles and the accelerator.
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

/**
 * The eighth program, which is DEPLOYED and is not a checked role.
 *
 * Cohort-16 is the first cohort that put an accelerator on a chain from its own
 * runbook: the simplification swarm folded `dclutch-general-accelerator-sbf`,
 * `dclutch-dealer-accelerator-sbf` and `dclutch-series-shadow-sbf` into one
 * `dclutch-accelerator-sbf`, and General batches, the Dealer's first market and
 * the whole Series family became dependent on a binary nothing had deployed.
 *
 * IT IS DELIBERATELY NOT A MEMBER OF `PROTOCOL_ROLES_V1`. The seven are the
 * checked roles: the deployment-set journal owns exactly those, every PDA in
 * this client is derived under one of them, and every owner check names one.
 * The accelerator owns no account and is never an account's owner — it is a
 * callback the Trading routes invoke — so widening the seven would have made
 * every custom deployment demand an address that answers no question, and would
 * have made the local fixed-seed layout, which deploys seven programs, unable
 * to be stated. What it IS is a program a reader can look up, whose liveness
 * decides whether a General or Series route can run at all, so it belongs in
 * the manifest and in the liveness reading beside the seven.
 */
export const ACCELERATOR_ROLE_V1 = 'accelerator' as const;

/** The seven checked roles and the accelerator: every program a cohort deploys. */
export const DEPLOYED_PROGRAM_ROLES_V1 = [...PROTOCOL_ROLES_V1, ACCELERATOR_ROLE_V1] as const;

export type DeployedProgramRoleV1 = (typeof DEPLOYED_PROGRAM_ROLES_V1)[number];

/**
 * A deployment's program addresses: the seven, and the accelerator when one is
 * deployed. `undefined` is a real answer — the gauntlet's local layout deploys
 * no accelerator — and a surface that iterates must ask
 * `deployedProgramRolesV1`, never assume eight.
 */
export type DeploymentProgramsV1 = Readonly<Record<ProtocolRoleV1, string>> & Readonly<{ accelerator?: string }>;

/** One sentence per program, for surfaces that introduce them by name. */
export const PROTOCOL_ROLE_MEANING_V1: Readonly<Record<DeployedProgramRoleV1, string>> = Object.freeze({
  registry: 'Finalized records and release activation — the content-addressed record layer.',
  rent: 'Rent credits and beneficiaries for protocol accounts.',
  custody: 'Collateral custody — the Hoards that physically back every liability.',
  resolution: 'Resolution — how a market learns its outcome, oracle receivers included.',
  claims: 'Claim liabilities and Positions — who is owed what, exactly.',
  trading: 'Trading — the routes that move claims against collateral.',
  core: 'Market roots — founding, phase, generation, and the identities a market commits to.',
  accelerator: 'The accelerator — one merged callback the Trading routes invoke to compute General batches, Dealer candidates and Series shadows inside a lifted heap frame. It owns no account.',
});

export type ClusterIdV1 = 'devnet' | 'local' | 'custom';

export type DeploymentV1 = Readonly<{
  cluster: ClusterIdV1;
  /** Short human name for the picker: "Devnet", "Local", "Custom". */
  label: string;
  endpoint: string;
  /** Expected genesis hash, or null when the chain's identity varies (local ledgers, custom). */
  genesisHash: string | null;
  /** The seven checked role programs, and the accelerator where one is deployed. */
  programs: DeploymentProgramsV1;
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

/** Per-program deployment evidence beyond the id — devnet only. */
export type ProgramEvidenceV1 = Readonly<{
  programData: string;
  deploymentSlot: string;
}>;

/**
 * Which programs this deployment actually names, in manifest order.
 *
 * The one place a surface asks "seven or eight". A deployment that carries no
 * accelerator answers seven, and nothing downstream renders an empty card or
 * reads an undefined address.
 */
export function deployedProgramRolesV1(deployment: DeploymentV1): ReadonlyArray<DeployedProgramRoleV1> {
  return deployment.programs.accelerator === undefined ? PROTOCOL_ROLES_V1 : DEPLOYED_PROGRAM_ROLES_V1;
}

export const DEVNET_DEPLOYMENT_V1: DeploymentV1 = Object.freeze({
  cluster: 'devnet',
  label: 'Devnet',
  endpoint: 'https://api.devnet.solana.com',
  genesisHash: SOLANA_DEVNET_GENESIS_HASH_V1,
  // COHORT-16, deployed 2026-09-05 from commit f2ae6bf75 on the named release
  // builder (platform-tools v1.53, Linux/x86_64, through `swarm-build`), and
  // dumped back and compared to its candidate ELF role by role before the next
  // role spent. EIGHT rows, and the eighth had never been on any chain: the
  // simplification swarm folded three accelerator links into one
  // `dclutch-accelerator-sbf`, and cohort-16 is the cohort that deployed it.
  // Devnet is disposable by ruling: each cohort is a full redeploy with fresh
  // identities and the previous one is abandoned in place and then CLOSED,
  // which returns its rent to pay for the next. These ids are not permanent and
  // nothing here should say they are.
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
  // written here at all: run against COHORT-15's own sealed plan on 2026-09-05
  // it refuses by naming all EIGHT vacant addresses, which is the reading that
  // retired the rows this table used to carry.
  programs: Object.freeze({
    registry: '6gRRiB9BtQFN6AquyLXXjuiX1GYN2xyW8nqCTc3xJzkV',
    rent: '42xN9ULoMpULmeDbdGCtyAo82FRJved6sojUun6NSKdt',
    custody: '8UkoNCPD4JuWBiHWdc7WaM3j7Fj9jbf8Fe926Q1CDceo',
    resolution: 'jrjXw2Rph15VyJB3ztbRgoHUPJrcvMSHV6svRUYtUw3',
    claims: '8JfHfBBGaoUP1yV6VzXcvWwhQSZNV8eQmDAiYmCpNQJk',
    trading: 'ESQhDyV7obS4oNp7abjn7sSYChxtGrHru4TzvPuybJi3',
    core: '4wv7JxoAad6JMQi2vHJyByLXasWS8RzJSTdvEEmpCjpe',
    accelerator: '6v1c2Go2h1rxkTN2EmzC5xGC35MTbaHPCHrKF6kTvg4y',
  }),
  // Bootstrap hint, GENERATED — do not hand-edit. Regenerate with
  // `node packages/dclutch-sdk/scripts/derive-activation-hint.mjs --write`.
  //
  // The one cache of those the Registry owns whose five pinned deployment
  // slots equalled the five live ProgramData slots in a single reading.
  // Release set 85defd75b236b191de00b48e673cdc4a4bcc2408b2248c4504895815b04cc69f,
  // pinning Core at deployment slot 493639301.
  // A session follows past this when it ages out; a reader cannot.
  activationCache: '2xVxMvfypJyo9bacGz1FFeK4L2qgqcsHaGoR9cbun6wV',
  provenance: 'Cohort-16’s devnet substrate, deployed 2026-09-05 from commit f2ae6bf75 on one named release builder, every ProgramData balance equal to (128 + 45 + elf_bytes) × 5,080 lamports exactly and every live image compared to its candidate ELF before the next role spent. Cohort-15 was closed the same morning — all eight of its ProgramData accounts read AccountNotFound while its Program stubs stayed executable and kept naming them — and the 44.42 SOL its rent returned paid for this one, which cost 36.50. It is the first cohort with EIGHT programs on the chain: the three accelerator links became one, and General batches, the Dealer’s first market and the whole Series family depend on a binary that until now nothing had deployed. It is also the first cohort to found a REFUNDING market: an oracle outage on GyD95eyERwRfwj8fSFNhWjKF2eaDg5XcREidPKex65zY pays one atom to every ordinary claim and nothing at all to the failure coordinate, which this site derives from that market’s own authenticated basis record rather than being told.',
});

/**
 * Cohort-16's ProgramData addresses and deployment slots.
 *
 * READ, not copied from a record, and not derived either: each address is the
 * 32 bytes the Program account itself names at offset 4, and each slot is the
 * u64 at offset 4 of that ProgramData account's own Loader-v3 header. Read
 * finalized at slot 493,692,510 -- hours after the deploy rather than the
 * minute after it, which is the stronger reading: these slots are what the
 * chain still says, not what the deploy reported. They run 493,638,685 through
 * 493,639,473, one per program in the order the eight were deployed, and they
 * reproduce COHORT16_DEPLOYED_SEALED_2026_09_05.md §2 without reading it --
 * including the accelerator's, which no prior cohort's table could carry
 * because no prior runbook deployed it.
 *
 * THAT LAST CHECK IS THE ONE THAT MATTERS. A closed program keeps its 36-byte
 * Program account, its executable flag and the ProgramData address it names --
 * only the ProgramData itself is gone. Cohort-8's rows survived here after its
 * close on 2026-09-01, and cohort-14's survived here after its close on
 * 2026-09-04, both because every gate asked the Program account, which was
 * alive, and none asked the account that holds the code.
 * `deployments.live.test.ts` asks, `deploymentLiveness.live.test.ts` makes the
 * refusal a gate rather than an env-gated skip, and the derivation that wrote
 * these rows refuses to emit one for a role whose ProgramData is vacant --
 * which is how cohort-15's eight rows left this table on 2026-09-05 rather than
 * being noticed missing later.
 */
export const DEVNET_PROGRAM_EVIDENCE_V1: Readonly<Record<DeployedProgramRoleV1, ProgramEvidenceV1>> = Object.freeze({
  registry: Object.freeze({ programData: '68Jh5pD42XWmYq5ViWoX3MKHMeENCRbgdxdGb8B7UY6k', deploymentSlot: '493638685' }),
  rent: Object.freeze({ programData: '8KG9NGFoMRCh4dngeAGNkP7kCmtQ68KthSbk8V883x5v', deploymentSlot: '493638731' }),
  custody: Object.freeze({ programData: 'AjYb8Ss7E3ruHppSCDcqxJLErwGhHikTcHQymKZu6BG1', deploymentSlot: '493638796' }),
  resolution: Object.freeze({ programData: 'PpzTFUiPbyj4MKbLoUzCxh4cAeLrZ52PBdvN8byxR1n', deploymentSlot: '493638882' }),
  claims: Object.freeze({ programData: '14EYxVmGJuSKX9iizPaLQQRj8ae3XiJJqWHdnAnCcv33', deploymentSlot: '493639017' }),
  trading: Object.freeze({ programData: '7RxAyfAUd3hEENzog4Faq4tqpzFfA6riM1jnYVLEgSwx', deploymentSlot: '493639190' }),
  core: Object.freeze({ programData: 'BbyZZAwbz37VwLR6zMQMm2bJAhfqbJVFAxr9HbFRQ5AU', deploymentSlot: '493639301' }),
  accelerator: Object.freeze({ programData: 'DfJLGB1W12cUYGpw3doG2DmMDe6ubR2UkmrrUsqosa9g', deploymentSlot: '493639473' }),
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

/** Labels for the programs of one deployment, keyed by address. */
export function deploymentProgramLabelsV1(deployment: DeploymentV1): Readonly<Record<string, string>> {
  const labels: Record<string, string> = {};
  for (const role of deployedProgramRolesV1(deployment)) {
    const address = deployment.programs[role];
    if (address === undefined) continue;
    labels[address] = `dClutch ${role[0].toUpperCase()}${role.slice(1)} · ${deployment.label}`;
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
  programs: DeploymentProgramsV1;
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
  // The accelerator is OPTIONAL in the plan schema and absent from every genesis
  // plan, so its absence is read as an absence and never as a missing role. A
  // document that names one has it admitted the same way the seven are.
  const acceleratorEntry = record.general_accelerator;
  const acceleratorId = acceleratorEntry !== null && typeof acceleratorEntry === 'object' && !Array.isArray(acceleratorEntry)
    ? (acceleratorEntry as Record<string, unknown>).program_id
    : undefined;
  const accelerator = typeof acceleratorId === 'string' && acceleratorId !== ''
    ? canonicalAddress(acceleratorId, 'general_accelerator program')
    : undefined;
  const rpc = record.rpc_url;
  const endpoint = typeof rpc === 'string' && rpc.trim() !== '' ? rpc.trim() : null;
  return Object.freeze({
    endpoint,
    programs: Object.freeze(accelerator === undefined ? programs : { ...programs, accelerator }),
  });
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
  // The accelerator is admitted when it is named and required never: an
  // operator running a local successor has seven programs, and a form that
  // demanded an eighth would refuse the deployment they actually have.
  const acceleratorRaw = (programsRaw as Record<string, unknown>)[ACCELERATOR_ROLE_V1];
  const accelerator = acceleratorRaw === undefined || acceleratorRaw === null || acceleratorRaw === ''
    ? undefined
    : canonicalAddress(acceleratorRaw, 'accelerator program');
  if (accelerator !== undefined && Object.values(programs).includes(accelerator)) {
    throw new Error('the accelerator program must be distinct from the seven role programs');
  }
  return Object.freeze({
    cluster: 'custom',
    label: 'Custom',
    endpoint: url.toString(),
    genesisHash: null,
    programs: Object.freeze(accelerator === undefined ? programs : { ...programs, accelerator }),
    activationCache: record.activationCache === undefined || record.activationCache === null || record.activationCache === ''
      ? null
      : canonicalAddress(record.activationCache, 'activation cache'),
    provenance: 'Your own deployment, entered through the cluster picker and stored only in this browser.',
  });
}
