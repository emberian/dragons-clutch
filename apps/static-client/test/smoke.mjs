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
const registry = read("successor-registry.js");
const builder = read("successor-builder.js");
const compiler = read("compiler-proposal.js");

const browserRealm = (...scripts) => {
  const context = vm.createContext({ URL, TextEncoder, TextDecoder, AbortController, Uint8Array, BigInt });
  for (const name of scripts) vm.runInContext(read(name), context, { filename: name });
  return context;
};

test("startup_has_no_embedded_chain_program_release_or_fixture_truth", () => {
  for (const id of ["operator-url", "cluster-name", "genesis-hash", "rpc-http-url", "rpc-websocket-url", "program-id", "program-data", "deployment-slot", "elf-sha256", "release-manifest-sha256", "source-commit", "capability-profile-id"]) {
    assert.match(html, new RegExp(`id="${id}"`));
  }
  assert.doesNotMatch(html, /<script src="(?:embedded-data|protocol-client|protocol-contracts|native-bspline-v1)\.js"/);
  assert.doesNotMatch(html, /value="(?:https?:\/\/|wss?:\/\/|[1-9][0-9]*|[1-9A-HJ-NP-Za-km-z]{32,44}|[0-9a-f]{40,64})"/);
  assert.match(app, /Nothing is inferred from fixtures or defaults/);
  assert.doesNotMatch(app, /manifest\.json|terms\.json|GlassEmbeddedData/);
});

test("operatord_transport_is_bounded_get_only_and_rpc_urls_are_configuration_only", () => {
  assert.match(chain, /method:\s*"GET"/);
  assert.match(chain, /credentials:\s*"omit"/);
  assert.match(chain, /redirect:\s*"error"/);
  assert.match(chain, /remainingResponseBytes/);
  assert.match(chain, /operatord-only; browser does not call validator RPC/);
  assert.doesNotMatch(chain, /method:\s*"(?:POST|PUT|PATCH|DELETE)"/);
  for (const endpoint of ["/v1/health", "/v1/acquisition", "/v1/releases", "/v1/accounts?commitment=", "/v1/keeper/next?commitment=", "/v1/forks"]) assert.match(chain, new RegExp(endpoint.replace(/[?]/g, "\\?")));
});

test("explicit_configuration_preserves_full_width_fields_as_decimal_strings", () => {
  const context = browserRealm("successor-registry.js", "successor-builder.js", "chain-client.js");
  const output = vm.runInContext(`(() => {
    const bytes = (fill) => GlassSuccessorBuilder.encodeBase58(new Uint8Array(32).fill(fill));
    return GlassChainClient.validateConfiguration({
      operatorUrl: "http://127.0.0.1:9898",
      clusterName: "private-local",
      genesisHash: bytes(4),
      rpcHttpUrl: "http://127.0.0.1:8899",
      rpcWebsocketUrl: "ws://127.0.0.1:8900",
      commitment: "processed",
      release: {
        programId: bytes(2), programData: bytes(3), deploymentSlot: "18446744073709551615",
        elfSha256: "01".repeat(32), releaseManifestSha256: "02".repeat(32),
        sourceCommit: "03".repeat(20), capabilityProfileId: "04".repeat(32)
      },
      bounds: { maximumAccounts: "4096", maximumResponseBytes: "8388608", timeoutMilliseconds: "10000", maximumSlotLag: "150" }
    });
  })()`, context);
  assert.equal(output.release.deploymentSlot, "18446744073709551615");
  assert.equal(output.bounds.maximumResponseBytes, "8388608");
  assert.equal(output.commitment, "processed");
  assert.match(output.release.releaseKey, /:18446744073709551615:/);
});

test("outer_builder_emits_zero_signature_blockhash_free_reserved_disabled_transaction", () => {
  const context = browserRealm("successor-registry.js", "successor-builder.js");
  const output = vm.runInContext(`(() => {
    const bytes = (fill) => GlassSuccessorBuilder.encodeBase58(new Uint8Array(32).fill(fill));
    const programId = bytes(2);
    return GlassSuccessorBuilder.build({
      payer: bytes(1),
      instructions: [{
        flow: "market-epoch-creation", family: "general", localAction: "1", actionName: "CreateMarket",
        payloadHex: "aabb", semanticOwner: { package: "clutch-market", schema: "create-market/v1", releaseSha256: "09".repeat(32) },
        accounts: [], requiredSigners: [], equations: [{ name: "exact conservation", unit: { kind: "collateral-atoms", mint: bytes(7) }, left: "340282366920938463463374607431768211455", right: "340282366920938463463374607431768211455" }]
      }]
    }, {
      clusterKey: "private:genesis", release: { programId, programData: bytes(3), deploymentSlot: "7", elfSha256: "01".repeat(32), releaseManifestSha256: "02".repeat(32), sourceCommit: "03".repeat(20), capabilityProfileId: "04".repeat(32) }
    }, "1232");
  })()`, context);
  assert.equal(output.schema, "dragons-clutch/operator/unsigned-protocol-transaction/v3");
  assert.equal(output.message.recentBlockhash, "11111111111111111111111111111111");
  assert.equal(output.hasRecentBlockhash, false);
  assert.equal(output.signed, false);
  assert.equal(output.submitted, false);
  assert.deepEqual([...output.runtimeAdmissions], ["reserved-disabled"]);
  assert.match(output.serializedTransactionHex, /^01(?:00){64}010001/);
  assert.equal(output.exactEquations[0].left, "340282366920938463463374607431768211455");
});

test("outer_builder_refuses_unbalanced_or_caller_enabled_material", () => {
  assert.doesNotMatch(builder, /runtimeAdmission\s*=|raw\.runtimeAdmission|enabled\s*:\s*raw/);
  assert.match(builder, /Unbalanced exact equation/);
  assert.match(builder, /central allocation ledger is ReservedDisabled/);
});

test("compiler_boundary_names_rust_owner_and_does_not_reimplement_payoff_math", () => {
  assert.match(html, /compile_production_payoff_v1/);
  assert.match(html, /canonical Product\/Series bundle assembler/);
  assert.match(compiler, /production-payoff-definition\/v1/);
  assert.match(compiler, /production-payoff-proposal\/v1/);
  assert.match(compiler, /exact-categorical.*exact-smooth.*analytic-smooth/s);
  assert.match(compiler, /Compiled Product\/Series bundle must expose the exact sixteen typed identities/);
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
    return GlassCompilerProposal.validateProposal({
      schema: "dragons-clutch/compiler/production-payoff-proposal/v1",
      authority: "untrusted-compiler-proposal", registrationAuthority: false,
      compilerReleaseSha256: "03".repeat(32), inputCanonicalSha256: "02".repeat(32),
      productTermsId, classification: "exact-categorical", spanStatus: "exact-in-span",
      nativeClaimBasis: { id: "05".repeat(32), bytesHex: "00".repeat(2352) },
      certificate: null, bounds: [], subdivisionDepth: null,
      compiledProductSeriesBundle: { id: "06".repeat(32), bytesHex: "00".repeat(528), identities }
    }, "02".repeat(32), "03".repeat(32), definition);
  })()`, context);
  assert.equal(output.productTermsId, "01".repeat(32));
  assert.equal(output.classification, "exact-categorical");
  assert.equal(Object.keys(output.compiledProductSeriesBundle.identities).length, 16);
  assert.equal(output.nativeClaimBasis.byteLength, "2352");
  assert.equal(output.compiledProductSeriesBundle.byteLength, "528");
});

test("no_shipped_script_contains_wallet_sign_or_submit_capability", () => {
  for (const [name, source] of [["app.js", app], ["chain-client.js", chain], ["successor-registry.js", registry], ["successor-builder.js", builder], ["compiler-proposal.js", compiler]]) {
    assert.doesNotMatch(source, /window\.(?:solana|phantom)|signTransaction|signAllTransactions|sendRawTransaction|sendTransaction|@solana\//, name);
    assert.doesNotMatch(source, /new\s+WebSocket|XMLHttpRequest|EventSource|navigator\.sendBeacon|serviceWorker/, name);
  }
});
