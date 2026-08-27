#!/usr/bin/env node
// Read the live successor chain independently and write the expectation set the
// browser is graded against.
//
// Inputs are a running RPC endpoint and one campaign run directory. The run
// directory supplies COORDINATES ONLY — which addresses the Rust campaign
// derived. Every VALUE below is decoded here, out of finalized account bytes
// this script fetched itself, by `chain-witness.mjs`. Nothing is copied out of
// the campaign's own reported state, so a campaign that misreported what it did
// would not survive this.

import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

import {
  Rpc,
  decodeCapabilityManifestV1,
  decodeClaimsAggregateV2,
  decodeClaimsPositionV2,
  decodeCoreMarketV2,
  decodeRealmV1,
  decodeTokenAmount,
  hex,
  magicOf,
  sha256,
} from './chain-witness.mjs';

function argument(name, fallback) {
  const index = process.argv.indexOf(`--${name}`);
  if (index < 0) {
    if (fallback === undefined) throw new Error(`missing required --${name}`);
    return fallback;
  }
  return process.argv[index + 1];
}

const endpoint = argument('endpoint', 'http://127.0.0.1:20890/');
const run = argument('run');
const outDir = argument('out-dir');
mkdirSync(outDir, { recursive: true });

const plan = JSON.parse(readFileSync(join(run, 'plan.json'), 'utf8'));
const evidence = JSON.parse(readFileSync(join(run, 'evidence.json'), 'utf8'));
const programs = {
  registry: plan.registry.program_id,
  core: plan.core.program_id,
  claims: plan.claims.program_id,
  trading: plan.trading.program_id,
  resolution: plan.resolution.program_id,
  custody: plan.custody.program_id,
  rent: plan.rent_credit.program_id,
};

const rpc = new Rpc(endpoint);
const floor = await rpc.finalizedSlot();

// ------------------------------------------------------- Core program scan
// The discovery surface enumerates Markets with one bounded getProgramAccounts
// scan. Do the same scan here, so "the browser found the Market" is checked
// against what the Core program actually holds and not against a pasted address.
const scan = await rpc.programAccounts(programs.core);
const scanned = scan.accounts.map((account) => ({
  address: account.address,
  magic: magicOf(account.data),
  bytes: account.data.length,
}));
const coreMarkets = [];
for (const account of scan.accounts) {
  if (magicOf(account.data) !== 'DCLTCOR2') continue;
  coreMarkets.push(decodeCoreMarketV2({ ...account, slot: scan.slot }));
}
coreMarkets.sort((left, right) => left.address.localeCompare(right.address));

const openMarkets = coreMarkets.filter((market) => market.phase === 'Open');
if (openMarkets.length !== 1) {
  throw new Error(`expected exactly one Open Market on this chain, found ${openMarkets.length}`);
}
const market = openMarkets[0];

// -------------------------------------------------------------- Claims side
// The founding's liabilities live in the Claims LiabilityBasisV2 aggregate,
// whose address the CAMPAIGN derived in Rust. Reading it at that coordinate and
// checking it names this Market back is what joins the two halves; nothing here
// re-derives a PDA, so nothing here can agree with the browser by construction.
const aggregateAccount = await rpc.account(evidence.accounts.claims_aggregate.address, floor);
if (aggregateAccount === null) throw new Error('Claims aggregate is absent at the finalized floor');
const aggregate = decodeClaimsAggregateV2(aggregateAccount);
if (aggregate.logicalMarket !== market.address) {
  throw new Error(`the Claims aggregate names Market ${aggregate.logicalMarket}, not the Open Market ${market.address}`);
}

const positionAccount = await rpc.account(evidence.accounts.founder_position.address, floor);
if (positionAccount === null) throw new Error('founder Position is absent at the finalized floor');
const position = decodeClaimsPositionV2(positionAccount);
if (position.aggregate !== aggregate.address) {
  throw new Error('the founder Position names a different Claims aggregate');
}

// -------------------------------------------------------- Realm and manifest
const realmRecord = await rpc.account(evidence.accounts.realm_record.address, floor);
if (realmRecord === null) throw new Error('Realm record is absent at the finalized floor');
const realm = decodeRealmV1(realmRecord.data);
if (realm.contentDigest !== market.realmId) {
  throw new Error('the Realm record body does not hash to the identity the Market committed to');
}

const manifestRecord = await rpc.account(evidence.accounts.capability_manifest_record.address, floor);
if (manifestRecord === null) throw new Error('capability manifest record is absent at the finalized floor');
const manifestDigest = hex(sha256(manifestRecord.data));
const capabilities = decodeCapabilityManifestV1(manifestRecord.data);

// ---------------------------------------------------------------- the Hoard
// The Hoard is a Custody vault. Its address is namespaced by the founding's
// action context, which is NOT a field of the Market root and NOT a protocol
// constant — the campaign's own context domain is
// `dclutch/local-campaign/founding-context/v1`, a campaign-local value. So a
// reader holding only a Market address cannot derive it. The campaign's
// coordinate is used here as a witness input and labelled as such.
const hoardAccount = await rpc.account(evidence.accounts.founding_hoard_vault_open.address, floor);
const hoard = hoardAccount === null ? null : {
  address: hoardAccount.address,
  program: hoardAccount.owner,
  ...decodeTokenAmount(hoardAccount),
  derivable: false,
  derivabilityNote: 'the Hoard vault address is namespaced by the founding action context and is not derivable from the Market root; this coordinate came from the campaign, not from the chain-readable Market',
};

const witness = {
  schema: 'dclutch-frontend-witness-expectation-v2',
  endpoint,
  run,
  floorSlot: floor,
  programs,
  founder: position.positionOwner,
  scan: {
    accounts: scan.accounts.length,
    entries: scanned,
    coreMarketAddresses: coreMarkets.map((entry) => entry.address),
    openMarketAddress: market.address,
  },
  market,
  claimsAggregate: aggregate,
  founderPosition: position,
  realm: { recordAddress: realmRecord.address, recordOwner: realmRecord.owner, ...realm },
  capabilityManifest: {
    recordAddress: manifestRecord.address,
    recordOwner: manifestRecord.owner,
    recordBytes: manifestRecord.data.length,
    contentDigest: manifestDigest,
    committedIdentity: market.capabilityManifestId,
    identityJoins: manifestDigest === market.capabilityManifestId,
    entries: capabilities,
  },
  hoard,
  economics: {
    supplyAtoms: aggregate.supplyAtoms,
    requiredBackingAtoms: aggregate.maximumSupplyAtoms,
    requiredBackingBasis: market.settled ? 'winning-claim-supply' : 'maximum-claim-supply',
    hoardAtoms: hoard?.amount ?? null,
    hoardCoversRequiredBacking: hoard === null ? null : BigInt(hoard.amount) >= BigInt(aggregate.maximumSupplyAtoms),
    completeSetsAtoms: position.completeSetsAtoms,
  },
  seedDomains: {
    corePosition: 'dclutch/position/v1 under the Core program (dclutch-realm-contract::POSITION_PDA_DOMAIN) — the Direct family Position',
    claimsAggregate: 'dclutch:lbv2:market under the Claims program (dclutch-claims-svm::CLAIMS_FOUNDING_AGGREGATE_SEED_V5)',
    claimsPosition: 'dclutch:lbv2:position under the Claims program, keyed by the aggregate (dclutch-claims-svm::PROTOCOL_POSITION_STATE_SEED_V2)',
  },
};

writeFileSync(join(outDir, 'expected.json'), `${JSON.stringify(witness, null, 2)}\n`);

// Optional: the exact finalized bytes, so a unit test can be written against
// real chain state instead of a hand-built buffer that agrees with whatever the
// decoder happens to do.
const fixtureOut = argument('fixture-out', '');
if (fixtureOut !== '') {
  const raw = (account) => ({
    address: account.address,
    owner: account.owner,
    lamports: account.lamports,
    dataHex: hex(account.data),
  });
  writeFileSync(fixtureOut, `${JSON.stringify({
    format: 'dclutch-web/live-open-market/v1',
    note: 'Finalized account bytes copied verbatim off a local successor-campaign validator. Not devnet or mainnet evidence.',
    sourceRun: 'tools/gauntlet/run.sh --mode full (successor campaign), local validator 127.0.0.1:20890',
    sourceRevision: JSON.parse(readFileSync(join(run, 'attestation', 'core.json'), 'utf8')).commit,
    observedFloorSlot: floor,
    programs,
    founder: position.positionOwner,
    accounts: {
      market: raw(scan.accounts.find((account) => account.address === market.address)),
      claimsAggregate: raw(aggregateAccount),
      founderPosition: raw(positionAccount),
      realmRecord: raw(realmRecord),
      hoardVault: hoardAccount === null ? null : raw(hoardAccount),
    },
  }, null, 2)}\n`);
  process.stderr.write(`fixture: ${fixtureOut}\n`);
}

process.stdout.write(`${JSON.stringify(witness, null, 2)}\n`);
