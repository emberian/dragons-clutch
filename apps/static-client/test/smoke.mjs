/*
 * Offline gates for the static client. No network, no install, no build step:
 * `node test/smoke.mjs` is the whole harness.
 *
 * These tests are named so the evidence ledger in
 * docs/implementation/ADVERSARIAL_REVIEW_V0.md section 6 can cite them.
 */
import { test } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import vm from "node:vm";
import { createHash } from "node:crypto";

const root = path.resolve(import.meta.dirname, "..");
const read = (name) => fs.readFileSync(path.join(root, name), "utf8");
const manifest = JSON.parse(read("manifest.json"));
const terms = JSON.parse(read("terms.json"));
const app = read("app.js");
const embeddedSource = read("embedded-data.js");
const html = read("index.html");

const canonicalize = (value) => {
  if (Array.isArray(value)) return value.map(canonicalize);
  if (value && typeof value === "object") {
    return Object.keys(value).sort().reduce((out, key) => {
      out[key] = canonicalize(value[key]);
      return out;
    }, {});
  }
  return value;
};
const canonicalJson = (value) => JSON.stringify(canonicalize(value));
const sha256Digest = (text) => `sha256:${createHash("sha256").update(text).digest("hex")}`;

// embedded-data.js is a classic browser script with no DOM dependency, so it
// can be evaluated in a bare realm. Values are re-parsed into this realm
// because cross-realm objects never satisfy deepStrictEqual prototype checks.
const evaluateEmbeddedData = () => {
  const context = vm.createContext({});
  vm.runInContext(embeddedSource, context, { filename: "embedded-data.js" });
  assert.ok(context.GlassEmbeddedData, "embedded-data.js must define GlassEmbeddedData");
  return JSON.parse(JSON.stringify(context.GlassEmbeddedData));
};

test("manifest_declares_every_chain_capability_unavailable", () => {
  assert.equal(manifest.schemaVersion, "dragon-clutch.static-release-manifest.v0");
  assert.ok(Array.isArray(manifest.clusters) && manifest.clusters.length >= 3);
  assert.ok(manifest.clusters.every((cluster) => cluster.status === "unavailable" && cluster.endpoint === null));
  assert.ok(manifest.programs.length > 0 && manifest.programs.every((program) => program.programId === null));
  assert.ok(manifest.profiles.some((profile) => profile.id === "synthetic-six-decimal"));
  assert.equal(manifest.capabilities.walletConnection, false);
  assert.equal(manifest.capabilities.rpcReads, false);
  assert.equal(manifest.capabilities.transactionSigning, false);
  assert.equal(manifest.capabilities.transactionSubmission, false);
});

test("terms_digest_recomputes_from_canonical_terms", () => {
  const digest = sha256Digest(canonicalJson(terms.canonicalTerms));
  assert.equal(terms.digest, digest);
  assert.equal(manifest.terms.digest, digest);
  assert.equal(manifest.terms.termsVersion, terms.termsVersion);
  assert.equal(manifest.terms.digestAlgorithm, "sha256");
});

test("terms_rounding_matches_kernel_refuse_on_remainder_semantics", () => {
  // crates/clutch-kernel `redeem` returns Error::RemainderRequired when
  // quantity * weight % denominator != 0; it never floors. `redeem_complete_set`
  // is the always-exact exit. A terms surface that promises flooring would
  // contradict the landed kernel (ADVERSARIAL_REVIEW_V0 P1-A).
  const rounding = terms.canonicalTerms.rounding;
  assert.doesNotMatch(rounding, /floor|truncat|round-(down|up|half)/i, "the kernel refuses a remainder rather than rounding it");
  assert.match(rounding, /exact/);
  assert.match(rounding, /refuse|remainder/);
  assert.match(terms.canonicalTerms.redemption, /complete-set/);
  assert.match(terms.semanticsNote, /refused, never floored/);
});

test("embedded_static_data_equals_reviewed_manifest_and_terms", () => {
  const embedded = evaluateEmbeddedData();
  assert.deepStrictEqual(embedded.manifest, manifest, "embedded-data.js manifest drifted from manifest.json; run `npm run embed`");
  assert.deepStrictEqual(embedded.terms, terms, "embedded-data.js terms drifted from terms.json; run `npm run embed`");
  // Key order is not load bearing, but a canonical-JSON comparison also catches
  // a mirror that agrees only up to JSON.parse coercion.
  assert.equal(canonicalJson(embedded.manifest), canonicalJson(manifest));
  assert.equal(canonicalJson(embedded.terms), canonicalJson(terms));
  // The digest the mirror carries must still be the one the terms hash to.
  assert.equal(embedded.terms.digest, sha256Digest(canonicalJson(embedded.terms.canonicalTerms)));
  assert.equal(embedded.manifest.terms.digest, embedded.terms.digest);
});

test("app_holds_no_second_copy_of_release_data_or_digest", () => {
  // Every displayed binding must come from the mirrored release data. A
  // re-stated digest, mint, program note, or commit here is exactly the drift
  // the gate above cannot see.
  assert.doesNotMatch(app, /sha256:[0-9a-f]{8}/i, "app.js must not hard-code a terms digest");
  assert.doesNotMatch(app, /UNBOUND-OFFLINE-SNAPSHOT|UNPUBLISHED-BUNDLE-DIGEST/);
  assert.doesNotMatch(app, /SYNTHETIC-MINT-NOT-ONCHAIN|XkeTXo1125vz5H9svJpGiw4JvLbN8VmMu9cmMvspump/);
  assert.doesNotMatch(app, /mainnet-beta|devnet|localnet/);
  assert.match(app, /globalThis\.GlassEmbeddedData/);
  assert.match(html, /<script src="embedded-data\.js"><\/script>[\s\S]*<script src="app\.js"><\/script>/);
});

test("browser_scripts_reject_wallet_rpc_signing_and_submission_symbols", () => {
  for (const [name, source] of [["app.js", app], ["embedded-data.js", embeddedSource]]) {
    assert.doesNotMatch(source, /window\.solana|window\.phantom|new\s+WebSocket|\bfetch\s*\(|XMLHttpRequest|EventSource|navigator\.sendBeacon|import\s*\(/, name);
    assert.doesNotMatch(source, /signTransaction|signAllTransactions|sendRawTransaction|sendTransaction|@solana\//, name);
  }
  assert.match(app, /mode:\s*["']offline-inspection-only["']/);
  assert.match(app, /submission:\s*["']disabled["']/);
});

test("meta_csp_carries_only_directives_a_meta_policy_can_enforce", () => {
  const csp = /<meta http-equiv="Content-Security-Policy" content="([^"]+)">/.exec(html);
  assert.ok(csp, "index.html must carry a meta CSP");
  const policy = csp[1];
  assert.match(policy, /default-src 'none'/);
  assert.match(policy, /script-src 'self'/);
  assert.match(policy, /connect-src 'none'/);
  // frame-ancestors, sandbox, and report-to are ignored in a meta policy.
  // Promising them from this HTML would be a false claim about the host.
  for (const headerOnly of ["frame-ancestors", "sandbox", "report-to", "report-uri"]) {
    assert.doesNotMatch(policy, new RegExp(headerOnly), `${headerOnly} is header-only and must not appear in the meta CSP`);
  }
});

test("serving_note_states_the_header_only_protections", () => {
  const serving = read("SERVING.md");
  assert.match(serving, /frame-ancestors/);
  assert.match(serving, /X-Content-Type-Options: nosniff/);
  assert.match(serving, /Referrer-Policy/);
  assert.match(serving, /meta/i);
});

test("every_shipped_asset_is_present_and_non_empty", () => {
  for (const file of ["index.html", "styles.css", "app.js", "embedded-data.js", "manifest.json", "terms.json", "SERVING.md", "README.md"]) {
    assert.ok(fs.statSync(path.join(root, file)).size > 0, `${file} should not be empty`);
  }
});

test("html_references_only_local_assets", () => {
  const urls = html.match(/(?:src|href)="([^"]+)"/g) || [];
  for (const url of urls) {
    assert.doesNotMatch(url, /^(?:src|href)="(?:https?:)?\/\//, `${url} is a remote reference`);
  }
});

test("app_only_addresses_element_ids_present_in_index_html", () => {
  const declared = new Set(Array.from(html.matchAll(/\bid="([^"]+)"/g), (match) => match[1]));
  const addressed = Array.from(app.matchAll(/\$\("([^"]+)"\)/g), (match) => match[1]);
  assert.ok(addressed.length > 10, "expected the app to address the inspection surface by id");
  for (const id of new Set(addressed)) {
    assert.ok(declared.has(id), `app.js addresses #${id}, which index.html does not declare`);
  }
});
