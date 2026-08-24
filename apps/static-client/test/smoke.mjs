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
  for (const endpoint of ["/v1/health", "/v1/acquisition", "/v1/session", "/v1/releases", "/v1/accounts?commitment=", "/v1/keeper/next?commitment=", "/v1/forks"]) assert.match(chain, new RegExp(endpoint.replace(/[?]/g, "\\?")));
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
});

test("source_and_structured_action_material_use_disjoint_current_transport_contracts", () => {
  const context = browserRealm("chain-client.js");
  const result = vm.runInContext(`(() => {
    const account = "11111111111111111111111111111112";
    const lookup = "11111111111111111111111111111113";
    const releaseKey = "checked-release";
    const manifest = "01".repeat(32);
    const profile = "02".repeat(32);
    const sessionId = "03".repeat(32);
    const state = "04".repeat(32);
    const workflow = "05".repeat(32);
    const ownerRelease = "06".repeat(32);
    const make = (familyTag, familyVersion, family, action, flow, messageVersion, addressLookupTables) => {
      const coordinate = { familyTag, familyVersion, localAction: "1", family, action };
      const cursor = { workflowId: workflow, lane: family, generation: "1", phase: "1", item: "0", observedStateSha256: state };
      const selection = { account, releaseKey, action, accountSlot: "100", observedCommitment: "finalized", effectiveCommitment: "finalized", branch: { kind: "finalized-scan" }, dependencies: [], cursor };
      const configuration = { release: { releaseKey, releaseManifestSha256: manifest, capabilityProfileId: profile, enabledIntents: [{ familyTag, familyVersion, localAction: "1" }] } };
      const session = { sessionId, release: { releaseKey }, restart: { cursors: [selection] } };
      const row = {
        coordinate,
        releaseAdmission: { enabled: true, releaseKey, capabilityProfileId: profile },
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
          authorityStateSha256: state,
          releaseManifestSha256: manifest,
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
    return { sourceFlow: sourceOutput.actions[0].transactionDraft.flows[0], structuredFlow: structuredOutput.actions[0].transactionDraft.flows[0], refused };
  })()`, context);
  assert.equal(result.sourceFlow, "source-plane-v3");
  assert.equal(result.structuredFlow, "structured-claim");
  assert.equal(result.refused, true);
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
