import { test } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import vm from "node:vm";

const root = path.resolve(import.meta.dirname, "..");
const read = (name) => fs.readFileSync(path.join(root, name), "utf8");
const html = read("index.html");
const app = read("app.js");
const chain = read("chain-client.js");
const compiler = read("compiler-proposal.js");

const browserRealm = (...scripts) => {
  const context = vm.createContext({ URL, TextEncoder, TextDecoder, AbortController, Uint8Array, BigInt });
  for (const name of scripts) vm.runInContext(read(name), context, { filename: name });
  return context;
};

test("startup_has_no_embedded_chain_program_release_or_fixture_truth", () => {
  assert.match(html, /id="operator-url"/);
  for (const id of ["cluster-name", "genesis-hash", "rpc-http-url", "rpc-websocket-url", "decoder-set", "program-id", "program-data", "deployment-slot", "elf-sha256", "release-manifest-sha256", "source-commit", "capability-profile-id"]) assert.doesNotMatch(html, new RegExp(`id="${id}"`));
  assert.doesNotMatch(html, /<script src="(?:embedded-data|protocol-client|protocol-contracts|native-bspline-v1)\.js"/);
  assert.doesNotMatch(html, /value="(?:https?:\/\/|wss?:\/\/|[1-9][0-9]*|[1-9A-HJ-NP-Za-km-z]{32,44}|[0-9a-f]{40,64})"/);
  assert.match(app, /Nothing is inferred from fixtures or defaults/);
  assert.doesNotMatch(app, /manifest\.json|terms\.json|GlassEmbeddedData/);
  for (const obsolete of ["manifest.json", "terms.json", "successor-registry.js"]) assert.equal(fs.existsSync(path.join(root, obsolete)), false, obsolete);
});

test("operatord_transport_is_bounded_get_only_and_rpc_urls_are_daemon_projection_only", () => {
  assert.match(chain, /method:\s*"GET"/);
  assert.match(chain, /credentials:\s*"omit"/);
  assert.match(chain, /redirect:\s*"error"/);
  assert.match(chain, /remainingResponseBytes/);
  assert.match(chain, /operatord-only; browser does not call validator RPC/);
  assert.doesNotMatch(chain, /method:\s*"(?:POST|PUT|PATCH|DELETE)"/);
  for (const endpoint of ["/v1/health", "/v1/acquisition", "/v1/session", "/v1/actions", "/v1/releases", "/v1/accounts?commitment=", "/v1/keeper/next?commitment=", "/v1/forks"]) assert.match(chain, new RegExp(endpoint.replace(/[?]/g, "\\?")));
  assert.doesNotMatch(chain, /configuration\.(?:rpcHttpUrl|rpcWebsocketUrl)/);
  assert.match(chain, /transportBinding must expose exactly one composed release/);
  assert.match(chain, /release key does not bind its exact coordinates and manifest/);
  assert.match(chain, /canonical-account-decoders\/v4-source-work-schedule/);
  assert.doesNotMatch(chain, /general-selected-candidate/);
});

test("canonical session brackets acquisition and admits only onchain-owned restart identities", () => {
  assert.match(chain, /operator-read-only-session-manifest\/v1/);
  assert.match(chain, /Finalized canonical session identity changed during acquisition/);
  assert.match(chain, /restart cursor names an identity not owned by a finalized canonical account decode/);
  assert.match(chain, /session checked release\/profile identity differs from acquisition/);
  assert.match(chain, /session canonical account identities contain duplicate addresses/);
  assert.match(chain, /differs from finalized canonical account field/);
  assert.match(chain, /accounts\?commitment=finalized/);
  assert.doesNotMatch(chain, /localStorage|sessionStorage|fixture|mock-source/i);
});

test("browser_target_contains_only_operatord_commitment_and_local_bounds", () => {
  const context = browserRealm("chain-client.js");
  const output = vm.runInContext(`(() => {
    return GlassChainClient.validateConfiguration({
      operatorUrl: "http://127.0.0.1:9898",
      commitment: "processed",
      bounds: { maximumAccounts: "4096", maximumResponseBytes: "8388608", timeoutMilliseconds: "10000", maximumSlotLag: "150" }
    });
  })()`, context);
  assert.equal(output.bounds.maximumResponseBytes, "8388608");
  assert.equal(output.commitment, "processed");
  assert.equal(output.authority, "explicit-user-selected-operatord-only");
  assert.equal("release" in output, false);
  assert.equal("genesisHash" in output, false);
  assert.equal("rpcHttpUrl" in output, false);
});

test("browser_refuses_caller_shaped_transaction_truth", () => {
  assert.equal(fs.existsSync(path.join(root, "successor-builder.js")), false);
  assert.doesNotMatch(html, /draft-json|payloadHex|requiredSigners|packet-limit/);
  assert.doesNotMatch(app, /GlassSuccessorBuilder|\.build\(draft|JSON\.parse\(\$\("draft-json"\)/);
  assert.match(chain, /operator-action-capability-set\/v1/);
  assert.match(chain, /action verdict is absent from, or duplicated within, the checked release enabled-intent set/);
  assert.match(chain, /unavailable action carries executable-looking transaction or signer material/);
  assert.match(chain, /operator-canonical-action-material\/v1/);
  assert.match(chain, /callable signer requirements differ from exact signer roles/);
  assert.match(chain, /serialized transaction encoding or byte count is invalid/);
  assert.match(chain, /discard this draft regardless of outcome|freshnessDisposition/);
  assert.match(app, /Browser-authored protocol material is forbidden/);
  assert.match(app, /canonical unsigned draft joined to a fresh finalized exact tuple/);
  assert.match(chain, /Fractional internal-credit material is not one exact live\/fresh\/reopen role shape/);
  assert.match(chain, /Fractional lifecycle material does not have the exact 32\+2\*N Product-foundation geometry/);
  assert.match(chain, /Fractional lifecycle selection does not carry its exact scheduler lane, phase, and sequence/);
  assert.match(chain, /complete canonical Product-foundation dependency index/);
  assert.match(chain, /fractional-redemption\/79\/1\/1\/initialize/);
  assert.match(chain, /fractional-redemption\/79\/1\/10\/close-empty-ledger/);
  assert.match(chain, /Fractional exact tuple contains an unowned account alias/);
  assert.match(app, /raw account IDs and metas are forbidden/);
});

test("source_and_structured_action_material_use_disjoint_current_transport_contracts", () => {
  const context = browserRealm("chain-client.js");
  const result = vm.runInContext(`(() => {
    const account = "11111111111111111111111111111112";
    const lookup = "11111111111111111111111111111113";
    const manifest = "01".repeat(32);
    const profile = "02".repeat(32);
    const sessionId = "03".repeat(32);
    const state = "04".repeat(32);
    const workflow = "05".repeat(32);
    const ownerRelease = "06".repeat(32);
    const make = (familyTag, familyVersion, family, action, flow, messageVersion, addressLookupTables) => {
      const releaseKey = account + ":1:" + ownerRelease + ":" + manifest;
      const executionManifest = family === "structured-claim" ? "09".repeat(32) : manifest;
      const executionReleaseKey = family === "structured-claim" ? lookup + ":2:" + ownerRelease + ":" + executionManifest : releaseKey;
      const coordinate = { familyTag, familyVersion, localAction: "1", family, action };
      const cursor = { workflowId: workflow, lane: family, generation: "1", phase: "1", item: "0", observedStateSha256: state };
      const selection = { account, releaseKey, action, accountSlot: "100", observedCommitment: "finalized", effectiveCommitment: "finalized", branch: { kind: "finalized-scan" }, dependencies: [], cursor };
      const configuration = { release: { releaseKey, releaseManifestSha256: manifest, capabilityProfileId: profile, enabledIntents: [{ familyTag, familyVersion, localAction: "1" }], enabledIntentVariants: [] } };
      const session = { sessionId, release: { releaseKey }, restart: { cursors: [selection] } };
      const row = {
        coordinate,
        releaseAdmission: { enabled: true, scope: family === "structured-claim" ? "structured-composite-wrapper-execution-base-driver-v1" : "single-release-execution-and-driver-v1", releaseKey, executionReleaseKey, driverReleaseKey: releaseKey, executionReleaseManifestSha256: executionManifest, capabilityProfileId: profile },
        stateSelection: selection,
        semanticOwnerConstructor: "closed-rust-owner",
        accountRoles: [{ index: "0", role: "payer", writable: true, signer: true, address, identityDisposition: "semantic-owner-derived-and-bound-to-draft" }],
        callable: true,
        verdict: "callable-unsigned-draft",
        reason: "exact current tuple",
        transactionDraft: {
          schema: "dragons-clutch/operator-canonical-action-material/v1",
          draftId: "07".repeat(32),
          constructionSchema: "dragons-clutch/operator/unsigned-protocol-transaction/v3",
          driverAccount: account,
          driverAccountSlot: "100",
          driverReleaseKey: releaseKey,
          executionReleaseKey,
          authorityStateSha256: state,
          releaseManifestSha256: executionManifest,
          capabilityProfileId: profile,
          feePayer: account,
          messageVersion,
          addressLookupTables,
          recentBlockhash: null,
          hasRecentBlockhash: false,
          signed: false,
          submitted: false,
          serializedTransactionHex: "00",
          serializedBytes: "1",
          actions: [action],
          flows: [flow],
          semanticOwners: [{ package: "owner", schema: "schema", releaseSha256: ownerRelease }],
          registryBindings: [{ familyTag, familyVersion, localAction: "1", allocationStatus: "frozen", centralAction: family === "source" ? "1" : null }],
          runtimeAdmissions: ["release-bound-enabled"],
          exactEquations: [{ name: "exact", unit: { kind: "lamports" }, left: "1", right: "1" }],
          reloadAuthoritativeAccounts: true
        },
        signerRequirements: [{ address, semanticRoles: ["payer", "transaction-fee-payer"], signaturePresent: false, keyAccess: false }],
        freshnessDisposition: { observedSlot: "100", validBeforeSlot: "110", maximumValiditySlots: "10", recentBlockhash: "absent; a launcher must reacquire state before adding one", beforeSigning: "reload", afterSubmission: "discard" }
      };
      const raw = { schema: "dragons-clutch/operator-action-capability-set/v1", status: "ready", commitment: "finalized", projectionAuthority: "untrusted-release-and-canonical-codec-projection", signing: false, submission: false, sessionId, releaseKey, capabilityProfileId: profile, freshness: { recentBlockhash: "absent-by-contract", feePayer: "must-be-explicit-in-server-constructed-draft", validBeforeSlot: "must-be-derived-from-a-fresh-clock-observation", beforeSigning: "reload", afterSubmission: "discard" }, actions: [row] };
      return { raw, configuration, session };
    };
    const source = make("77", "2", "source", "initialize-source-head", "source-plane-v3", "legacy", []);
    const structured = make("75", "1", "structured-claim", "create-structured-descriptor", "structured-claim", "v0", [{ account: lookup, observedSlot: "99", stateSha256: "08".repeat(32), writableAddresses: "1", readonlyAddresses: "1" }]);
    const sourceOutput = GlassChainClient.validateActionCapabilities(source.raw, source.configuration, source.session);
    const structuredOutput = GlassChainClient.validateActionCapabilities(structured.raw, structured.configuration, structured.session);
    structured.raw.actions[0].transactionDraft.addressLookupTables = [];
    let refused = false;
    try { GlassChainClient.validateActionCapabilities(structured.raw, structured.configuration, structured.session); } catch (_) { refused = true; }
    const dealerVariant = { familyTag: "76", familyVersion: "1", localAction: "25", payloadDiscriminator: "8", name: "dealer-retire-active-facility-credit" };
    const dealerConfiguration = { release: { releaseKey: source.configuration.release.releaseKey, releaseManifestSha256: manifest, capabilityProfileId: profile, elfSha256: ownerRelease, enabledIntents: [], enabledIntentVariants: [dealerVariant] } };
    const dealerSession = { sessionId, release: { releaseKey: dealerConfiguration.release.releaseKey }, restart: { cursors: [] } };
    const dealerRaw = { schema: "dragons-clutch/operator-action-capability-set/v1", status: "ready", commitment: "finalized", projectionAuthority: "untrusted-release-and-canonical-codec-projection", signing: false, submission: false, sessionId, releaseKey: dealerConfiguration.release.releaseKey, capabilityProfileId: profile, freshness: { recentBlockhash: "absent-by-contract", feePayer: "must-be-explicit-in-server-constructed-draft", validBeforeSlot: "must-be-derived-from-a-fresh-clock-observation", beforeSigning: "reload", afterSubmission: "discard" }, actions: [{ coordinate: { familyTag: "76", familyVersion: "1", localAction: "25", family: "dealer", action: "retire" }, payloadVariant: { discriminator: "8", name: dealerVariant.name }, releaseAdmission: { enabled: true, scope: "payload-discriminator-only", coarseCoordinateEnabled: false, releaseKey: dealerConfiguration.release.releaseKey, capabilityProfileId: profile }, stateSelection: null, semanticOwnerConstructor: "chain-derived-dealer-terminal-v1", accountRoles: [], callable: false, verdict: "unavailable", reason: "exact frame absent", transactionDraft: null, signerRequirements: [], freshnessDisposition: "no draft" }] };
    const dealerOutput = GlassChainClient.validateActionCapabilities(dealerRaw, dealerConfiguration, dealerSession);
    dealerConfiguration.release.enabledIntents = [{ familyTag: "76", familyVersion: "1", localAction: "25" }];
    let coarseRefused = false;
    try { GlassChainClient.validateActionCapabilities(dealerRaw, dealerConfiguration, dealerSession); } catch (_) { coarseRefused = true; }
    dealerConfiguration.release.enabledIntents = [];
    const dealerLabels = ["actor","policy","state-v3","facility-position-v3","facility-replay-v3","funded-dependencies-v2","dealer-liveness-schedule-v1","liveness-policy","liveness-source","liveness-candidate","liveness-clearing","liveness-settlement","liveness-resolution","liveness-retirement","liveness-recovery","liveness-receipt","liveness-payer","position-rent-payer","replay-rent-payer","obligation-rent-payer","neutral-lamport-sink","clock-sysvar","rent-sysvar","system-program","dealer-series-obligation-v2","product-market-root-v2","series-registry-v3","current-program","current-programdata","registry-release-v2","capability-profile-v4","series-market-link-v2","compiler-bundle-v6","attachment-v5","realm","collateral-profile-v2","collateral-policy-v2","collateral-token-program","collateral-token-programdata","market-binding-v2","market-runtime-v3","market-instance-v2","hoard-v2","claim-ledger-v3","dealer-future-credit-funding-v1"];
    const dealerWritable = new Set([0,2,3,4,13,15,16,17,18,19,20,24,31,40,44]);
    const dealerAddresses = dealerLabels.map((_, index) => GlassChainClient.encodeBase58(new Uint8Array(32).fill(index + 1)));
    dealerRaw.actions[0] = {
      ...dealerRaw.actions[0],
      payloadVariant: { discriminator: "9", name: "dealer-retire-unused-future-credit" },
      accountRoles: dealerLabels.map((role, index) => ({ index: String(index), role, writable: dealerWritable.has(index), signer: index === 0, address: dealerAddresses[index], identityDisposition: "semantic-owner-derived-and-bound-to-draft" })),
      callable: true,
      verdict: "callable-unsigned-draft",
      reason: "exact target-9 frame",
      observationSet: { schema: "dragons-clutch/operator/dealer-terminal-observation-set/v1", payloadDiscriminator: "9", authorityStateSha256: "15".repeat(32), chainStateSha256: "11".repeat(32), collateralCatalogReceiptId: "12".repeat(32), lookupTableStateSha256: "16".repeat(32), accounts: dealerAddresses.map((address, index) => index === 15 ? { index: String(index), address, observedSlot: "100", disposition: "finalized-absent", owner: null, lamports: null, executable: null, rentEpoch: null, dataSha256: null, releaseKey: null } : { index: String(index), address, observedSlot: "100", disposition: "finalized-present", owner: dealerAddresses[0], lamports: "1", executable: false, rentEpoch: "0", dataSha256: "13".repeat(32), releaseKey: dealerConfiguration.release.releaseKey }) },
      transactionDraft: { schema: "dragons-clutch/operator-canonical-action-material/v1", draftId: "14".repeat(32), constructionSchema: "dragons-clutch/operator/unsigned-protocol-transaction/v3", driverAccount: dealerAddresses[2], driverAccountSlot: "100", driverReleaseKey: dealerConfiguration.release.releaseKey, executionReleaseKey: dealerConfiguration.release.releaseKey, authorityStateSha256: "15".repeat(32), releaseManifestSha256: manifest, capabilityProfileId: profile, feePayer: dealerAddresses[0], messageVersion: "v0", addressLookupTables: [{ account: GlassChainClient.encodeBase58(new Uint8Array(32).fill(60)), observedSlot: "100", stateSha256: "16".repeat(32), writableAddresses: "1", readonlyAddresses: "1" }], recentBlockhash: null, hasRecentBlockhash: false, serializedTransactionHex: "00", serializedBytes: "1", actions: ["dealer-retire-unused-future-credit"], flows: ["dealer-facility-terminal"], semanticOwners: [{ package: "clutch-dealer-runtime-contract", schema: "dragons-clutch/dealer-terminal-retire/action25/targets8-9/v1", releaseSha256: ownerRelease }], registryBindings: [{ familyTag: "76", familyVersion: "1", localAction: "25", allocationStatus: "frozen", centralAction: "25" }], runtimeAdmissions: ["payload-variant-release-bound-enabled"], exactEquations: [{ name: "keeper payment", unit: { kind: "lamports" }, left: "1", right: "1" }], signed: false, submitted: false, reloadAuthoritativeAccounts: true },
      signerRequirements: [{ address: dealerAddresses[0], semanticRoles: ["actor", "transaction-fee-payer"], signaturePresent: false, keyAccess: false }],
      freshnessDisposition: { observedSlot: "100", validBeforeSlot: "110", maximumValiditySlots: "10", recentBlockhash: "absent; a launcher must reacquire state before adding one", beforeSigning: "reload", afterSubmission: "discard" }
    };
    dealerConfiguration.release.enabledIntentVariants = [{ ...dealerVariant, payloadDiscriminator: "9", name: "dealer-retire-unused-future-credit" }];
    const callableDealer = GlassChainClient.validateActionCapabilities(dealerRaw, dealerConfiguration, dealerSession);
    dealerRaw.actions[0].observationSet.accounts.pop();
    let dealerTupleRefused = false;
    try { GlassChainClient.validateActionCapabilities(dealerRaw, dealerConfiguration, dealerSession); } catch (_) { dealerTupleRefused = true; }
    return { sourceFlow: sourceOutput.actions[0].transactionDraft.flows[0], structuredFlow: structuredOutput.actions[0].transactionDraft.flows[0], refused, dealerDiscriminator: dealerOutput.actions[0].payloadVariant.payloadDiscriminator, coarseRefused, dealerCallable: callableDealer.actions[0].callable, dealerTupleRefused };
  })()`, context);
  assert.equal(result.sourceFlow, "source-plane-v3");
  assert.equal(result.structuredFlow, "structured-claim");
  assert.equal(result.refused, true);
  assert.equal(result.dealerDiscriminator, "8");
  assert.equal(result.coarseRefused, true);
  assert.equal(result.dealerCallable, true);
  assert.equal(result.dealerTupleRefused, true);
});

test("direct_actions_accept_only_exact_current_chain_material", () => {
  const context = browserRealm("chain-client.js");
  const result = vm.runInContext(`(() => {
    const address = (value) => GlassChainClient.encodeBase58(new Uint8Array(32).fill(value));
    const addresses = Array.from({ length: 7 }, (_, index) => address(index + 1));
    const manifest = "21".repeat(32);
    const profile = "22".repeat(32);
    const ownerRelease = "23".repeat(32);
    const state = "24".repeat(32);
    const releaseKey = addresses[0] + ":1:" + ownerRelease + ":" + manifest;
    const coordinate = { familyTag: "80", familyVersion: "1", localAction: "5", family: "direct", action: "submit-direct-candidate" };
    const cursor = { workflowId: "25".repeat(32), lane: "candidate", generation: "1", phase: "1", item: "0", observedStateSha256: state };
    const selection = { account: addresses[0], releaseKey, action: coordinate.action, accountSlot: "100", observedCommitment: "finalized", effectiveCommitment: "finalized", branch: { kind: "finalized-scan" }, dependencies: [], cursor };
    const configuration = { release: { releaseKey, releaseManifestSha256: manifest, capabilityProfileId: profile, enabledIntents: [{ familyTag: "80", familyVersion: "1", localAction: "5" }], enabledIntentVariants: [] } };
    const session = { sessionId: "26".repeat(32), release: { releaseKey }, restart: { cursors: [selection] } };
    const roleContract = [
      ["direct-root", false, true], ["direct-replay", false, true], ["direct-selection", false, true],
      ["clock-sysvar", false, false], ["candidate-submitter", true, true], ["system-program", false, false]
    ];
    const row = {
      coordinate,
      releaseAdmission: { enabled: true, scope: "single-release-execution-and-driver-v1", releaseKey, executionReleaseKey: releaseKey, driverReleaseKey: releaseKey, executionReleaseManifestSha256: manifest, capabilityProfileId: profile },
      stateSelection: selection,
      semanticOwnerConstructor: "clutch-direct-market-runtime/current-v1",
      accountRoles: roleContract.map(([role, signer, writable], index) => ({ index: String(index), role, signer, writable, address: addresses[index], identityDisposition: "semantic-owner-derived-and-bound-to-draft" })),
      callable: true,
      verdict: "callable-unsigned-draft",
      reason: "current chain-derived submit material",
      transactionDraft: {
        schema: "dragons-clutch/operator-canonical-action-material/v1", draftId: "27".repeat(32), constructionSchema: "dragons-clutch/operator/unsigned-protocol-transaction/v3",
        driverAccount: addresses[0], driverAccountSlot: "100", driverReleaseKey: releaseKey, executionReleaseKey: releaseKey, authorityStateSha256: state,
        releaseManifestSha256: manifest, capabilityProfileId: profile, feePayer: addresses[4], messageVersion: "legacy", addressLookupTables: [],
        recentBlockhash: null, hasRecentBlockhash: false, signed: false, submitted: false, serializedTransactionHex: "00", serializedBytes: "1",
        actions: [coordinate.action], flows: ["direct-market-v1"], semanticOwners: [{ package: "clutch-direct-market-runtime", schema: "current-v1", releaseSha256: ownerRelease }],
        registryBindings: [{ familyTag: "80", familyVersion: "1", localAction: "5", allocationStatus: "frozen", centralAction: "5" }], runtimeAdmissions: ["release-bound-enabled"],
        exactEquations: [{ name: "selection retained-bond principal conservation", unit: { kind: "lamports" }, left: "1", right: "1" }], reloadAuthoritativeAccounts: true
      },
      symbolicPostcondition: null,
      signerRequirements: [{ address: addresses[4], semanticRoles: ["candidate-submitter", "transaction-fee-payer"], signaturePresent: false, keyAccess: false }],
      freshnessDisposition: { observedSlot: "100", validBeforeSlot: "110", maximumValiditySlots: "10", recentBlockhash: "absent; a launcher must reacquire state before adding one", beforeSigning: "reload", afterSubmission: "discard" }
    };
    const raw = { schema: "dragons-clutch/operator-action-capability-set/v1", status: "ready", commitment: "finalized", projectionAuthority: "untrusted-release-and-canonical-codec-projection", signing: false, submission: false, sessionId: session.sessionId, releaseKey, capabilityProfileId: profile, freshness: { recentBlockhash: "absent-by-contract", feePayer: "must-be-explicit-in-server-constructed-draft", validBeforeSlot: "must-be-derived-from-a-fresh-clock-observation", beforeSigning: "reload", afterSubmission: "discard" }, actions: [row] };
    const accepted = GlassChainClient.validateActionCapabilities(raw, configuration, session);
    row.coordinate.action = "finalize-direct-selection";
    row.transactionDraft.actions = [row.coordinate.action];
    row.stateSelection.action = row.coordinate.action;
    session.restart.cursors[0].action = row.coordinate.action;
    let legacyRefused = false;
    try { GlassChainClient.validateActionCapabilities(raw, configuration, session); } catch (_) { legacyRefused = true; }
    return { branch: accepted.actions[0].directContract.branch, legacyRefused };
  })()`, context);
  assert.equal(result.branch, "submit-without-eviction");
  assert.equal(result.legacyRefused, true);
});

test("fractional_terminal_material_keeps_authority_chain_derived", () => {
  const context = browserRealm("chain-client.js");
  const result = vm.runInContext(`(() => {
    const address = (value) => GlassChainClient.encodeBase58(new Uint8Array(32).fill(value));
    const roles = ["realm", "profile", "collateral-policy", "collateral-token-program", "market-binding-v2", "market-runtime-v3", "market-instance-preimage-v2", "hoard-v2", "claim-ledger-v3", "resolution-v5", "fractional-policy-v3", "fractional-ledger-v1"];
    const addresses = roles.map((_, index) => address(index + 1));
    const feePayer = address(40);
    const manifest = "31".repeat(32);
    const profile = "32".repeat(32);
    const elf = "33".repeat(32);
    const state = "34".repeat(32);
    const releaseKey = address(50) + ":1:" + elf + ":" + manifest;
    const coordinate = { familyTag: "79", familyVersion: "1", localAction: "9", family: "fractional", action: "seal-fractional-claims-exhausted" };
    const cursor = { workflowId: "35".repeat(32), lane: "fractional-redemption", generation: "1", phase: "9", item: "1", observedStateSha256: state };
    const selection = { account: addresses[11], releaseKey, action: coordinate.action, accountSlot: "100", observedCommitment: "finalized", effectiveCommitment: "finalized", branch: { kind: "finalized-scan" }, dependencies: [], cursor };
    const configuration = { release: { releaseKey, releaseManifestSha256: manifest, capabilityProfileId: profile, elfSha256: elf, enabledIntents: [{ familyTag: "79", familyVersion: "1", localAction: "9" }], enabledIntentVariants: [] } };
    const session = { sessionId: "36".repeat(32), release: { releaseKey }, restart: { cursors: [selection] } };
    const row = {
      coordinate,
      releaseAdmission: { enabled: true, scope: "single-release-execution-and-driver-v1", releaseKey, executionReleaseKey: releaseKey, driverReleaseKey: releaseKey, executionReleaseManifestSha256: manifest, capabilityProfileId: profile },
      stateSelection: selection,
      semanticOwnerConstructor: "clutch-fractional-redemption-runtime/fractional-redemption/79/1/9/seal-claims-exhausted",
      accountRoles: roles.map((role, index) => ({ index: String(index), role, signer: false, writable: index === 8 || index === 11, address: addresses[index], identityDisposition: "semantic-owner-derived-and-bound-to-draft" })),
      callable: true,
      verdict: "callable-unsigned-draft",
      reason: "exact claims-exhausted frame",
      transactionDraft: {
        schema: "dragons-clutch/operator-canonical-action-material/v1", draftId: "37".repeat(32), constructionSchema: "dragons-clutch/operator/unsigned-protocol-transaction/v3",
        driverAccount: addresses[11], driverAccountSlot: "100", driverReleaseKey: releaseKey, executionReleaseKey: releaseKey, authorityStateSha256: state,
        releaseManifestSha256: manifest, capabilityProfileId: profile, feePayer, messageVersion: "legacy", addressLookupTables: [], recentBlockhash: null,
        hasRecentBlockhash: false, signed: false, submitted: false, serializedTransactionHex: "00", serializedBytes: "1", actions: [coordinate.action], flows: ["fractional-redemption"],
        semanticOwners: [{ package: "clutch-fractional-redemption-runtime", schema: "fractional-redemption/79/1/9/seal-claims-exhausted", releaseSha256: elf }],
        registryBindings: [{ familyTag: "79", familyVersion: "1", localAction: "9", allocationStatus: "frozen", centralAction: "9" }], runtimeAdmissions: ["release-bound-enabled"],
        exactEquations: [{ name: "chain-derived zero native claim supply", unit: { kind: "egg-atoms" }, left: "0", right: "0" }], reloadAuthoritativeAccounts: true
      },
      symbolicPostcondition: null,
      signerRequirements: [{ address: feePayer, semanticRoles: ["transaction-fee-payer"], signaturePresent: false, keyAccess: false }],
      freshnessDisposition: { observedSlot: "100", validBeforeSlot: "110", maximumValiditySlots: "10", recentBlockhash: "absent; a launcher must reacquire state before adding one", beforeSigning: "reload", afterSubmission: "discard" }
    };
    const raw = { schema: "dragons-clutch/operator-action-capability-set/v1", status: "ready", commitment: "finalized", projectionAuthority: "untrusted-release-and-canonical-codec-projection", signing: false, submission: false, sessionId: session.sessionId, releaseKey, capabilityProfileId: profile, freshness: { recentBlockhash: "absent-by-contract", feePayer: "must-be-explicit-in-server-constructed-draft", validBeforeSlot: "must-be-derived-from-a-fresh-clock-observation", beforeSigning: "reload", afterSubmission: "discard" }, actions: [row] };
    const accepted = GlassChainClient.validateActionCapabilities(raw, configuration, session);
    row.accountRoles[8].writable = false;
    let weakenedRefused = false;
    try { GlassChainClient.validateActionCapabilities(raw, configuration, session); } catch (_) { weakenedRefused = true; }
    return { holderChoice: accepted.actions[0].fractionalContract.holderChoice, weakenedRefused };
  })()`, context);
  assert.equal(result.holderChoice, null);
  assert.equal(result.weakenedRefused, true);
});

test("fractional_bearer_material_exposes_choices_without_account_meta_authority", () => {
  const context = browserRealm("chain-client.js");
  const result = vm.runInContext(`(() => {
    const address = (value) => GlassChainClient.encodeBase58(new Uint8Array(32).fill(value));
    const labels = ["claimant", "realm", "profile", "collateral-policy", "collateral-token-program", "market-binding-v2", "market-runtime-v3", "market-instance-preimage-v2", "hoard-v2", "claim-ledger-v3", "resolution-v5", "fractional-policy-v3", "fractional-ledger-v1", "collateral-mint", "collateral-destination", "hoard-authority", "hoard-token", "outcome-token-program", "outcome-token-programdata", "bearer-source", "collateral-token-programdata", "outcome-mint", "outcome-mint"];
    const writable = new Set([8, 9, 12, 14, 16, 19, 22]);
    const addresses = labels.map((_, index) => address(index + 1));
    const manifest = "41".repeat(32), profile = "42".repeat(32), elf = "43".repeat(32), state = "44".repeat(32);
    const releaseKey = address(50) + ":1:" + elf + ":" + manifest;
    const coordinate = { familyTag: "79", familyVersion: "1", localAction: "3", family: "fractional", action: "redeem-fractional-bearer-exact" };
    const cursor = { workflowId: "45".repeat(32), lane: "fractional-redemption", generation: "1", phase: "3", item: "1", observedStateSha256: state };
    const selection = { account: addresses[12], releaseKey, action: coordinate.action, accountSlot: "100", observedCommitment: "finalized", effectiveCommitment: "finalized", branch: { kind: "finalized-scan" }, dependencies: [], cursor };
    const configuration = { release: { releaseKey, releaseManifestSha256: manifest, capabilityProfileId: profile, elfSha256: elf, enabledIntents: [{ familyTag: "79", familyVersion: "1", localAction: "3" }], enabledIntentVariants: [] } };
    const session = { sessionId: "46".repeat(32), release: { releaseKey }, restart: { cursors: [selection] } };
    const row = {
      coordinate, releaseAdmission: { enabled: true, scope: "single-release-execution-and-driver-v1", releaseKey, executionReleaseKey: releaseKey, driverReleaseKey: releaseKey, executionReleaseManifestSha256: manifest, capabilityProfileId: profile }, stateSelection: selection,
      semanticOwnerConstructor: "clutch-fractional-redemption-runtime/fractional-redemption/79/1/3/redeem-bearer-exact",
      accountRoles: labels.map((role, index) => ({ index: String(index), role, signer: index === 0, writable: writable.has(index), address: addresses[index], identityDisposition: "semantic-owner-derived-and-bound-to-draft" })),
      callable: true, verdict: "callable-unsigned-draft", reason: "holder intent joined to exact frame", symbolicPostcondition: null,
      transactionDraft: { schema: "dragons-clutch/operator-canonical-action-material/v1", draftId: "47".repeat(32), constructionSchema: "dragons-clutch/operator/unsigned-protocol-transaction/v3", driverAccount: addresses[12], driverAccountSlot: "100", driverReleaseKey: releaseKey, executionReleaseKey: releaseKey, authorityStateSha256: state, releaseManifestSha256: manifest, capabilityProfileId: profile, feePayer: addresses[0], messageVersion: "legacy", addressLookupTables: [], recentBlockhash: null, hasRecentBlockhash: false, signed: false, submitted: false, serializedTransactionHex: "00", serializedBytes: "1", actions: [coordinate.action], flows: ["fractional-redemption"], semanticOwners: [{ package: "clutch-fractional-redemption-runtime", schema: "fractional-redemption/79/1/3/redeem-bearer-exact", releaseSha256: elf }], registryBindings: [{ familyTag: "79", familyVersion: "1", localAction: "3", allocationStatus: "frozen", centralAction: "3" }], runtimeAdmissions: ["release-bound-enabled"], exactEquations: [{ name: "holder-approved bearer Eggs burned", unit: { kind: "egg-atoms", market: "48".repeat(32), outcome: "1" }, left: "7", right: "7" }, { name: "chain-derived whole collateral payout", unit: { kind: "collateral-atoms", mint: addresses[13] }, left: "5", right: "5" }, { name: "chain-derived retained payout numerator", unit: { kind: "price-units", scale: "10" }, left: "0", right: "0" }], reloadAuthoritativeAccounts: true },
      signerRequirements: [{ address: addresses[0], semanticRoles: ["claimant", "transaction-fee-payer"], signaturePresent: false, keyAccess: false }], freshnessDisposition: { observedSlot: "100", validBeforeSlot: "110", maximumValiditySlots: "10", recentBlockhash: "absent; a launcher must reacquire state before adding one", beforeSigning: "reload", afterSubmission: "discard" }
    };
    const raw = { schema: "dragons-clutch/operator-action-capability-set/v1", status: "ready", commitment: "finalized", projectionAuthority: "untrusted-release-and-canonical-codec-projection", signing: false, submission: false, sessionId: session.sessionId, releaseKey, capabilityProfileId: profile, freshness: { recentBlockhash: "absent-by-contract", feePayer: "must-be-explicit-in-server-constructed-draft", validBeforeSlot: "must-be-derived-from-a-fresh-clock-observation", beforeSigning: "reload", afterSubmission: "discard" }, actions: [row] };
    const accepted = GlassChainClient.validateActionCapabilities(raw, configuration, session);
    row.accountRoles[21].writable = true;
    let metaRefused = false;
    try { GlassChainClient.validateActionCapabilities(raw, configuration, session); } catch (_) { metaRefused = true; }
    return { choice: accepted.actions[0].fractionalContract.holderChoice, geometry: accepted.actions[0].fractionalContract.geometry, metaRefused };
  })()`, context);
  assert.equal(result.choice.outcome, "1");
  assert.equal(result.choice.quantity, "7");
  assert.equal(result.geometry, "bearer-exact-21+2");
  assert.equal(result.metaRefused, true);
});

test("fractional_credit_transfer_binds_holder_route_to_exact_geometry", () => {
  const context = browserRealm("chain-client.js");
  const result = vm.runInContext(`(() => {
    const address = (value) => GlassChainClient.encodeBase58(new Uint8Array(32).fill(value));
    const labels = ["source-claimant", "destination-claimant", "realm", "profile", "collateral-policy", "collateral-token-program", "market-binding-v2", "market-runtime-v3", "market-instance-preimage-v2", "hoard-v2", "claim-ledger-v3", "resolution-v5", "fractional-policy-v3", "fractional-ledger-v1", "source-credit-v2", "destination-credit-v2", "payout-and-lifecycle-role", "payout-and-lifecycle-role", "payout-and-lifecycle-role", "payout-and-lifecycle-role", "payout-and-lifecycle-role"];
    const writable = new Set([9, 10, 13, 14, 15, 16, 17]);
    const addresses = labels.map((_, index) => address(index + 1));
    const manifest = "51".repeat(32), profile = "52".repeat(32), elf = "53".repeat(32), state = "54".repeat(32);
    const releaseKey = address(50) + ":1:" + elf + ":" + manifest;
    const coordinate = { familyTag: "79", familyVersion: "1", localAction: "6", family: "fractional", action: "transfer-fractional-credit" };
    const cursor = { workflowId: "55".repeat(32), lane: "fractional-redemption", generation: "1", phase: "6", item: "1", observedStateSha256: state };
    const selection = { account: addresses[13], releaseKey, action: coordinate.action, accountSlot: "100", observedCommitment: "finalized", effectiveCommitment: "finalized", branch: { kind: "finalized-scan" }, dependencies: [], cursor };
    const configuration = { release: { releaseKey, releaseManifestSha256: manifest, capabilityProfileId: profile, elfSha256: elf, enabledIntents: [{ familyTag: "79", familyVersion: "1", localAction: "6" }], enabledIntentVariants: [] } };
    const session = { sessionId: "56".repeat(32), release: { releaseKey }, restart: { cursors: [selection] } };
    const row = { coordinate, releaseAdmission: { enabled: true, scope: "single-release-execution-and-driver-v1", releaseKey, executionReleaseKey: releaseKey, driverReleaseKey: releaseKey, executionReleaseManifestSha256: manifest, capabilityProfileId: profile }, stateSelection: selection, semanticOwnerConstructor: "clutch-fractional-redemption-runtime/fractional-redemption/79/1/6/transfer-credit", accountRoles: labels.map((role, index) => ({ index: String(index), role, signer: index < 2, writable: writable.has(index), address: addresses[index], identityDisposition: "semantic-owner-derived-and-bound-to-draft" })), callable: true, verdict: "callable-unsigned-draft", reason: "two holder consents joined to finalized credits", symbolicPostcondition: null,
      transactionDraft: { schema: "dragons-clutch/operator-canonical-action-material/v1", draftId: "57".repeat(32), constructionSchema: "dragons-clutch/operator/unsigned-protocol-transaction/v3", driverAccount: addresses[13], driverAccountSlot: "100", driverReleaseKey: releaseKey, executionReleaseKey: releaseKey, authorityStateSha256: state, releaseManifestSha256: manifest, capabilityProfileId: profile, feePayer: addresses[0], messageVersion: "legacy", addressLookupTables: [], recentBlockhash: null, hasRecentBlockhash: false, signed: false, submitted: false, serializedTransactionHex: "00", serializedBytes: "1", actions: [coordinate.action], flows: ["fractional-redemption"], semanticOwners: [{ package: "clutch-fractional-redemption-runtime", schema: "fractional-redemption/79/1/6/transfer-credit", releaseSha256: elf }], registryBindings: [{ familyTag: "79", familyVersion: "1", localAction: "6", allocationStatus: "frozen", centralAction: "6" }], runtimeAdmissions: ["release-bound-enabled"], exactEquations: [{ name: "holder-approved credit numerator moved", unit: { kind: "price-units", scale: "10" }, left: "3", right: "3" }, { name: "chain-derived whole credit payout", unit: { kind: "collateral-atoms", mint: address(40) }, left: "1", right: "1" }], reloadAuthoritativeAccounts: true },
      signerRequirements: [{ address: addresses[0], semanticRoles: ["source-claimant", "transaction-fee-payer"], signaturePresent: false, keyAccess: false }, { address: addresses[1], semanticRoles: ["destination-claimant"], signaturePresent: false, keyAccess: false }], freshnessDisposition: { observedSlot: "100", validBeforeSlot: "110", maximumValiditySlots: "10", recentBlockhash: "absent; a launcher must reacquire state before adding one", beforeSigning: "reload", afterSubmission: "discard" } };
    const raw = { schema: "dragons-clutch/operator-action-capability-set/v1", status: "ready", commitment: "finalized", projectionAuthority: "untrusted-release-and-canonical-codec-projection", signing: false, submission: false, sessionId: session.sessionId, releaseKey, capabilityProfileId: profile, freshness: { recentBlockhash: "absent-by-contract", feePayer: "must-be-explicit-in-server-constructed-draft", validBeforeSlot: "must-be-derived-from-a-fresh-clock-observation", beforeSigning: "reload", afterSubmission: "discard" }, actions: [row] };
    const accepted = GlassChainClient.validateActionCapabilities(raw, configuration, session);
    row.accountRoles[16].writable = false;
    let routeRefused = false;
    try { GlassChainClient.validateActionCapabilities(raw, configuration, session); } catch (_) { routeRefused = true; }
    return { choice: accepted.actions[0].fractionalContract.holderChoice, geometry: accepted.actions[0].fractionalContract.geometry, routeRefused };
  })()`, context);
  assert.equal(result.choice.numerator, "3");
  assert.equal(result.choice.payout.kind, "internal-position");
  assert.equal(result.geometry, "transfer-internal-live-credit-21-roles");
  assert.equal(result.routeRefused, true);
});

test("compiler_boundary_names_rust_owner_and_does_not_reimplement_payoff_math", () => {
  assert.match(html, /compile_production_payoff_v1/);
  assert.match(html, /current BundleV7 assembler/);
  assert.match(compiler, /production-payoff-definition\/v1/);
  assert.match(compiler, /product-exact-market-request\/v2/);
  assert.match(compiler, /product-exact-market-proposal\/v3/);
  assert.match(compiler, /exact-categorical.*exact-smooth.*analytic-smooth/s);
  assert.match(compiler, /Compiled Product\/Series bundle must expose the exact sixteen typed identities owned by CompiledProductSeriesBundleV7/);
  assert.match(compiler, /completeFullDomainNegative/);
  assert.match(compiler, /bundleArtifactKind/);
  assert.doesNotMatch(compiler, /Math\.(?:exp|pow|sqrt)|parseFloat|Number\([^)]*(?:numerator|denominator|coordinate|payout)/);
});

test("compiler_transport_joins_definition_class_terms_bytes_and_sixteen_bundle_ids", () => {
  const context = browserRealm("compiler-proposal.js");
  const output = vm.runInContext(`(() => {
    const productTermsId = "01".repeat(32);
    const definition = GlassCompilerProposal.validateDefinition({
      schema: "dragons-clutch/compiler/production-payoff-definition/v1",
      productTermsId,
      kind: "exact-categorical",
      definition: {
        coordinateDomainMin: "0", coordinateDomainMax: "9", knots: [],
        cellPayouts: [[{ numerator: "1", denominator: "1" }]],
        ambiguityPolicyRegistryValue: "1", edgePolicyRegistryValue: "1"
      }
    });
    const identities = {};
    for (const name of GlassCompilerProposal.bundleIdentityNames) identities[name] = "04".repeat(32);
    identities.nativeClaimBasisId = "05".repeat(32);
    identities.marketGenesisProfileId = productTermsId;
    const programId = "11111111111111111111111111111112";
    const exactMarketSearch = {
      marketId: "08".repeat(32), priceId: "09".repeat(32), prices: ["1"],
      coordinates: ["0"], maximumSubsetEvaluationsPerSupport: "1"
    };
    return GlassCompilerProposal.validateProposal({
      schema: "dragons-clutch/compiler/product-exact-market-proposal/v3",
      authority: "untrusted-compiler-proposal", registrationAuthority: false,
      compilerReleaseSha256: "03".repeat(32), programId,
      requestCanonicalSha256: "02".repeat(32), inputCanonicalSha256: "07".repeat(32),
      productTermsId, classification: "exact-categorical", spanStatus: "exact-in-span",
      nativeClaimBasis: { id: "05".repeat(32), bytesHex: "00".repeat(2352) },
      certificate: null, bounds: [], subdivisionDepth: null,
      compiledProductSeriesBundleV7: {
        id: "06".repeat(32), bytesHex: "00".repeat(528),
        artifact: { kind: "68", context: "0".repeat(64), exactBodyBytes: "528", programId, pda: "11111111111111111111111111111113", bump: "254" },
        identities
      },
      exactMarket: {
        authority: "untrusted-compiler-sidecar", registrationAuthority: false,
        outcome: "unsupported", coverage: "declared-coordinate-subset", completeFullDomainNegative: false,
        claims: { uniquePrice: false, fairValue: false, optimalClearing: false },
        bindings: { marketId: exactMarketSearch.marketId, productTermsId, nativeClaimBasisId: "05".repeat(32), priceId: exactMarketSearch.priceId, bundleV7Id: "06".repeat(32) },
        target: { outcomeCount: "1", payoutDenominator: "1", prices: ["1"] },
        search: { coordinateDomainMin: "0", coordinateDomainMax: "9", coordinates: ["0"], maximumSubsetEvaluationsPerSupport: "1", exhaustedThroughSupport: "1", truncatedSupport: "0", workBySupport: [{ support: "1", evaluations: "1", exactButUnrepresentable: "0" }] },
        workManifest: { id: "10".repeat(32), bytesHex: "00".repeat(1640) }, certificate: null,
        bundleV7Sidecar: { id: "11".repeat(32), bytesHex: "00".repeat(176), bundleArtifactKind: "68", bundleArtifactContext: "0".repeat(64) }
      }
    }, "02".repeat(32), "07".repeat(32), "03".repeat(32), definition, { programId, exactMarketSearch });
  })()`, context);
  assert.equal(output.productTermsId, "01".repeat(32));
  assert.equal(output.classification, "exact-categorical");
  assert.equal(Object.keys(output.compiledProductSeriesBundleV7.identities).length, 16);
  assert.equal(output.nativeClaimBasis.byteLength, "2352");
  assert.equal(output.compiledProductSeriesBundleV7.byteLength, "528");
  assert.equal(output.exactMarket.coverage, "declared-coordinate-subset");
  assert.equal(output.exactMarket.completeFullDomainNegative, false);
  assert.equal(output.exactMarket.bundleV7Sidecar.bundleArtifactKind, "68");
});

test("no_shipped_script_contains_wallet_sign_or_submit_capability", () => {
  for (const [name, source] of [["app.js", app], ["chain-client.js", chain], ["compiler-proposal.js", compiler]]) {
    assert.doesNotMatch(source, /window\.(?:solana|phantom)|signTransaction|signAllTransactions|sendRawTransaction|sendTransaction|@solana\//, name);
    assert.doesNotMatch(source, /new\s+WebSocket|XMLHttpRequest|EventSource|navigator\.sendBeacon|serviceWorker/, name);
  }
});
