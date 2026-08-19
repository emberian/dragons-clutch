import { test } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import vm from "node:vm";
import { createHash, webcrypto } from "node:crypto";

const clientRoot = path.resolve(import.meta.dirname, "..");
const repositoryRoot = path.resolve(clientRoot, "../..");
const source = fs.readFileSync(path.join(clientRoot, "native-bspline-v1.js"), "utf8");
const schemaSource = fs.readFileSync(path.join(clientRoot, "native-bspline-market-creation-v1.schema.json"), "utf8");
const clientSchemaDoc = fs.readFileSync(path.join(repositoryRoot, "docs/implementation/NATIVE_BSPLINE_CLIENT_SCHEMA_V1.md"), "utf8");
const semanticsAudit = fs.readFileSync(path.join(repositoryRoot, "docs/reviews/NATIVE_SEMANTICS_AUDIT_V4.md"), "utf8");
const previewSchema = JSON.parse(
  fs.readFileSync(path.join(clientRoot, "native-bspline-market-creation-v1.schema.json"), "utf8")
);
const fixturePath = path.join(
  repositoryRoot,
  "research/bspline-shape-compiler/fixtures/native-v1-degree1.json"
);
const fixture = JSON.parse(fs.readFileSync(fixturePath, "utf8"));
const context = vm.createContext({ crypto: webcrypto });
vm.runInContext(source, context, { filename: "native-bspline-v1.js" });
const sdk = context.DragonsClutchNativeBsplineV1;

const unhex = (value) => Uint8Array.from(value.match(/../g), (pair) => Number.parseInt(pair, 16));
const cloneBytes = (value) => Uint8Array.from(value);
const sha256 = (domain, payload) => createHash("sha256").update(domain).update(payload).digest();
const setLe = (target, offset, value, width) => {
  let remaining = BigInt(value);
  for (let index = 0; index < width; index += 1) {
    target[offset + index] = Number(remaining & 255n);
    remaining >>= 8n;
  }
};
const rehashTerms = (target) => {
  const digest = sha256("dragons-clutch/terms/v2", target.slice(34, 1654));
  target.set(digest, 2);
  return target;
};
const plain = (value) => JSON.parse(JSON.stringify(value));

test("rust_fixture_fields_match_the_unsigned_offline_preview", async () => {
  assert.equal(fixture.schema, "dragon-clutch.native-bspline-cross-language.v1");
  const termsBytes = unhex(fixture.termsAccountBytes);
  const certificateBytes = unhex(fixture.shapeCertificateBytes);
  const terms = plain(await sdk.inspectTermsAccount(termsBytes));
  assert.equal(terms.termsDigest, fixture.termsDigest);
  assert.equal(terms.basisSpec.domainMax, fixture.maxValue);
  assert.equal(terms.basisSpec.semanticMode, "native-bspline");
  assert.equal(sdk.hex(sdk.encodeBasisSpec(terms.basisSpec)), fixture.basisSpecBytes);
  assert.equal(sdk.hex(await sdk.digestBasisSpec(terms.basisSpec)), fixture.basisSpecDigest);

  const certificate = plain(await sdk.inspectShapeCertificate(certificateBytes));
  assert.equal(certificate.termsDigest, fixture.termsDigest);
  assert.equal(certificate.basisDigest, fixture.basisSpecDigest);
  assert.equal(certificate.semanticMode, "native-bspline");
  assert.equal(certificate.shape.family, "capped-call");
  assert.equal(sdk.hex(await sdk.digestShapeCertificate(certificateBytes)), fixture.shapeCertificateDigest);

  const preview = plain(await sdk.buildMarketCreationPreview({
    termsAccountBytes: termsBytes,
    shapeCertificateBytes: certificateBytes,
    marketNonce: fixture.marketNonce,
    observedCurrentSlot: fixture.observedCurrentSlot,
    expiresSlot: fixture.expiresSlot
  }));
  assert.equal(preview.basisSpecBytes, fixture.basisSpecBytes);
  assert.equal(preview.basisSpecDigest, fixture.basisSpecDigest);
  assert.equal(preview.shapeCertificateDigest, fixture.shapeCertificateDigest);
  assert.deepEqual(preview.termsArtifactIntentBytes, [
    fixture.beginTermsArtifactIntentBytes,
    ...fixture.writeTermsArtifactIntentBytes,
    fixture.sealTermsArtifactIntentBytes
  ]);
  assert.equal(preview.createMarketIntentBytes, fixture.createMarketIntentBytes);
  assert.equal(preview.authorization.submission, "disabled");
});

test("preview_schema_names_the_unsigned_offline_eleven_intent_sequence", () => {
  assert.equal(previewSchema.$id, "dragon-clutch.native-bspline-market-creation.v1");
  assert.equal(previewSchema.properties.mode.const, "offline-inspection-only");
  assert.equal(previewSchema.properties.semanticMode.const, "native-bspline");
  assert.equal(previewSchema.properties.termsArtifactIntentBytes.minItems, 11);
  assert.equal(previewSchema.properties.termsArtifactIntentBytes.maxItems, 11);
  assert.ok(!JSON.stringify(previewSchema).includes("InitTerms"));
});

test("native_preview_wording_does_not_claim_execution_or_a_live_route", () => {
  for (const [name, text] of [
    ["native-bspline-v1.js", source],
    ["native-bspline-market-creation-v1.schema.json", schemaSource],
    ["NATIVE_BSPLINE_CLIENT_SCHEMA_V1.md", clientSchemaDoc]
  ]) {
    assert.doesNotMatch(text, /live(?:\s+[^\n]{0,40})?route|executed only after/i, `${name} must describe an offline preview`);
  }
  assert.match(source, /unsigned, offline runtime-shaped/i);
  assert.match(schemaSource, /offline runtime-shaped preview/i);
  assert.match(clientSchemaDoc, /unsigned, offline runtime-shaped/i);
  assert.match(semanticsAudit, /separate native B-spline inspection SDK/i);
  assert.doesNotMatch(semanticsAudit, /It has no degree, knots, denominator, native semantic identity, or native order\/compiler artifact/i);
});

test("client_claim_stops_at_structure_and_digest_not_compiler_or_full_policy", async () => {
  const certificate = plain(await sdk.inspectShapeCertificate(unhex(fixture.shapeCertificateBytes)));
  const terms = plain(await sdk.inspectTermsAccount(unhex(fixture.termsAccountBytes)));
  assert.match(certificate.verificationScope, /structure-and-digests-only/);
  assert.match(certificate.verificationScope, /Rust compiler recompile still required/);
  assert.doesNotMatch(certificate.verificationScope, /compiler-(?:verified|proved)/i);
  assert.match(terms.validationScope, /basis-projection/);
  assert.match(terms.validationScope, /runtime policy admission remains on-chain/);

  // statistic_id is deliberately outside this client's validation scope. A
  // self-certifying but runtime-unregistered value remains inspectable and is
  // explicitly not labeled runtime-admitted.
  const unsupportedPolicy = cloneBytes(unhex(fixture.termsAccountBytes));
  unsupportedPolicy[1302] = 99; // body statistic_id low byte: 34 + 1268
  rehashTerms(unsupportedPolicy);
  const inspected = plain(await sdk.inspectTermsAccount(unsupportedPolicy));
  assert.match(inspected.validationScope, /runtime policy admission remains on-chain/);
});

test("max_value_and_historical_terms_digest_domain_match_rust_owners", () => {
  const accumulator = fs.readFileSync(path.join(repositoryRoot, "crates/clutch-accumulator/src/lib.rs"), "utf8");
  const layout = fs.readFileSync(path.join(repositoryRoot, "programs/solana-layout/src/lib.rs"), "utf8");
  const match = /pub const MAX_VALUE: u128 = ([0-9_]+);/.exec(accumulator);
  assert.ok(match, "Rust MAX_VALUE owner must remain discoverable");
  assert.equal(match[1].replaceAll("_", ""), fixture.maxValue);
  assert.match(layout, /digest\(b"dragons-clutch\/terms\/v2", &\[body_bytes\]\)/);
  assert.equal(
    sha256("dragons-clutch/terms/v2", unhex(fixture.termsAccountBytes).slice(34, 1654)).toString("hex"),
    fixture.termsDigest
  );
});

test("basis_json_and_bytes_refuse_noncanonical_numbers_modes_and_padding", async () => {
  const canonical = plain(await sdk.inspectTermsAccount(unhex(fixture.termsAccountBytes))).basisSpec;
  for (const mutate of [
    (value) => { value.denominator = "08"; },
    (value) => { value.denominator = 8; },
    (value) => { value.degree = 4; },
    (value) => { value.outcomeCount = 1; },
    (value) => { value.knots = ["0"]; },
    (value) => { value.knots = [value.knots[1], value.knots[0]]; },
    (value) => { value.edgePolicy = "categorical-lowering"; },
  ]) {
    const mutant = structuredClone(canonical);
    mutate(mutant);
    assert.throws(() => sdk.encodeBasisSpec(mutant));
  }
  const basisBytes = unhex(fixture.basisSpecBytes);
  for (const offset of [0, 8, 10, 12, 13, 14, 15, 16, 17, 18, 24, 64]) {
    const mutant = cloneBytes(basisBytes);
    mutant[offset] ^= offset === 24 ? 8 : 1;
    assert.throws(() => sdk.decodeBasisSpec(mutant), undefined, `basis offset ${offset}`);
  }
  // Domain and active-knot changes can describe a different *valid* basis.
  // They must change identity rather than being mislabeled noncanonical.
  for (const offset of [32, 48, 67]) {
    const mutant = cloneBytes(basisBytes);
    mutant[offset] ^= 1;
    sdk.decodeBasisSpec(mutant);
    assert.notEqual(sdk.hex(await sdk.digestBasisSpec(mutant)), fixture.basisSpecDigest);
  }
  const trailing = Uint8Array.from([...basisBytes, 0]);
  assert.throws(() => sdk.decodeBasisSpec(trailing));
});

test("every_manually_typed_terms_offset_fails_closed_when_relevant", async () => {
  const canonical = unhex(fixture.termsAccountBytes);
  const cases = [
    ["tag", (value) => { value[0] = 9; }, false],
    ["version", (value) => { value[1] = 2; }, false],
    ["stored digest", (value) => { value[2] ^= 1; }, false],
    ["realm", (value) => { value.fill(0, 34, 66); }, true],
    ["profile", (value) => { value.fill(0, 66, 98); }, true],
    ["feed", (value) => { value.fill(0, 98, 130); }, true],
    ["outcome count", (value) => { value[162] = 1; }, true],
    ["payout count", (value) => { value[163] = 0; }, true],
    ["denominator", (value) => { value.fill(0, 164, 172); }, true],
    ["edge policy", (value) => { value[1305] = 3; }, true],
    ["degree", (value) => { value[1306] = 4; }, true],
    ["knot count", (value) => { value[1307] = 3; }, true],
    ["uniform", (value) => { value[1308] = 3; }, true],
    ["resolution reserved", (value) => { value[1310] = 1; }, true],
    ["inactive knot padding", (value) => { value[1415] = 1; }, true],
    ["body reserved suffix", (value) => { value[1647] = 1; }, true],
    ["flags", (value) => { value[1655] = 1; }, false],
  ];
  for (const [name, mutate, rehash] of cases) {
    const mutant = cloneBytes(canonical);
    mutate(mutant);
    if (rehash) rehashTerms(mutant);
    await assert.rejects(() => sdk.inspectTermsAccount(mutant), undefined, name);
  }
});

test("certificate_structure_refuses_versions_lowering_digests_padding_and_rationals", async () => {
  const canonical = unhex(fixture.shapeCertificateBytes);
  for (const offset of [0, 8, 10, 12, 14, 16, 52, 389, 454]) {
    const mutant = cloneBytes(canonical);
    mutant[offset] ^= 1;
    await assert.rejects(() => sdk.inspectShapeCertificate(mutant), undefined, `certificate offset ${offset}`);
  }
  const firstRational = 456;
  const zeroDenominator = cloneBytes(canonical);
  zeroDenominator[firstRational + 4] = 0;
  await assert.rejects(() => sdk.inspectShapeCertificate(zeroDenominator));

  const overwide = cloneBytes(canonical);
  setLe(overwide, firstRational, 4097, 2);
  await assert.rejects(() => sdk.inspectShapeCertificate(overwide));
  await assert.rejects(() => sdk.inspectShapeCertificate(Uint8Array.from([...canonical, 0])));
});

test("typed_terms_upload_matches_fixture_and_refuses_every_wire_binding", async () => {
  const terms = plain(await sdk.inspectTermsAccount(unhex(fixture.termsAccountBytes)));
  const plan = sdk.buildTermsArtifactUploadPlan({
    termsAccountBytes: unhex(fixture.termsAccountBytes),
    realm: terms.realm,
    termsDigest: terms.termsDigest,
    observedCurrentSlot: fixture.observedCurrentSlot,
    expiresSlot: fixture.expiresSlot
  });
  assert.equal(sdk.hex(plan.beginIntent), fixture.beginTermsArtifactIntentBytes);
  assert.deepEqual(Array.from(plan.writeIntents, sdk.hex), fixture.writeTermsArtifactIntentBytes);
  assert.equal(sdk.hex(plan.sealIntent), fixture.sealTermsArtifactIntentBytes);
  assert.equal(plan.writeIntents[8][69], 120);
  assert.equal(plan.writeIntents[8][70], 0);
  assert.ok(Array.from(plan.writeIntents[8].slice(71 + 120)).every((byte) => byte === 0));
  assert.equal(sdk.verifyTermsArtifactUploadPlan({
    termsAccountBytes: unhex(fixture.termsAccountBytes),
    realm: terms.realm,
    termsDigest: terms.termsDigest,
    plan
  }), true);

  const rejectMutation = (section, offset) => {
    const mutant = {
      observedCurrentSlot: plan.observedCurrentSlot,
      expiresSlot: plan.expiresSlot,
      beginIntent: cloneBytes(plan.beginIntent),
      writeIntents: Array.from(plan.writeIntents, cloneBytes),
      sealIntent: cloneBytes(plan.sealIntent)
    };
    mutant[section][offset] ^= 1;
    assert.throws(() => sdk.verifyTermsArtifactUploadPlan({
      termsAccountBytes: unhex(fixture.termsAccountBytes), realm: terms.realm, termsDigest: terms.termsDigest, plan: mutant
    }));
  };
  for (const offset of [0, 1, 2, 3, 35, 67, 69]) rejectMutation("beginIntent", offset);
  for (const offset of [0, 1, 2, 3, 35, 67, 69, 71]) {
    const mutant = {
      observedCurrentSlot: plan.observedCurrentSlot,
      expiresSlot: plan.expiresSlot,
      beginIntent: cloneBytes(plan.beginIntent),
      writeIntents: Array.from(plan.writeIntents, cloneBytes),
      sealIntent: cloneBytes(plan.sealIntent)
    };
    mutant.writeIntents[0][offset] ^= 1;
    assert.throws(() => sdk.verifyTermsArtifactUploadPlan({
      termsAccountBytes: unhex(fixture.termsAccountBytes), realm: terms.realm, termsDigest: terms.termsDigest, plan: mutant
    }), undefined, `write offset ${offset}`);
  }
  for (const offset of [0, 1, 2, 3, 35, 67]) rejectMutation("sealIntent", offset);

  const finalPadding = {
    observedCurrentSlot: plan.observedCurrentSlot,
    expiresSlot: plan.expiresSlot,
    beginIntent: cloneBytes(plan.beginIntent),
    writeIntents: Array.from(plan.writeIntents, cloneBytes),
    sealIntent: cloneBytes(plan.sealIntent)
  };
  finalPadding.writeIntents[8][262] = 1;
  assert.throws(() => sdk.verifyTermsArtifactUploadPlan({
    termsAccountBytes: unhex(fixture.termsAccountBytes), realm: terms.realm, termsDigest: terms.termsDigest, plan: finalPadding
  }));

  const swapped = {
    ...plan,
    writeIntents: Array.from(plan.writeIntents, cloneBytes)
  };
  [swapped.writeIntents[0], swapped.writeIntents[1]] = [swapped.writeIntents[1], swapped.writeIntents[0]];
  assert.throws(() => sdk.verifyTermsArtifactUploadPlan({
    termsAccountBytes: unhex(fixture.termsAccountBytes), realm: terms.realm, termsDigest: terms.termsDigest, plan: swapped
  }));

  for (const [current, expiry] of [["100", "99"], ["100", "107"], ["100", "432101"]]) {
    assert.throws(() => sdk.buildTermsArtifactUploadPlan({
      termsAccountBytes: unhex(fixture.termsAccountBytes), realm: terms.realm, termsDigest: terms.termsDigest,
      observedCurrentSlot: current, expiresSlot: expiry
    }));
  }
});

test("create_market_intent_fields_match_rust_fixture_and_are_exact", async () => {
  const terms = plain(await sdk.inspectTermsAccount(unhex(fixture.termsAccountBytes)));
  const intent = sdk.encodeCreateMarketIntent({
    realm: terms.realm,
    profile: terms.profile,
    marketNonce: fixture.marketNonce,
    outcomeCount: terms.basisSpec.outcomeCount,
    termsDigest: terms.termsDigest,
    feed: terms.feed
  });
  assert.equal(sdk.hex(intent), fixture.createMarketIntentBytes);
  assert.equal(intent.length, 139);
  assert.deepEqual(Array.from(intent.slice(0, 2)), [1, 3]);
  assert.equal(Buffer.from(intent.slice(2, 34)).toString("hex"), terms.realm);
  assert.equal(Buffer.from(intent.slice(34, 66)).toString("hex"), terms.profile);
  assert.equal(Buffer.from(intent.slice(75, 107)).toString("hex"), terms.termsDigest);
  assert.equal(Buffer.from(intent.slice(107, 139)).toString("hex"), terms.feed);
  for (const field of ["realm", "profile", "termsDigest", "feed"]) {
    const input = {
      realm: terms.realm,
      profile: terms.profile,
      marketNonce: fixture.marketNonce,
      outcomeCount: terms.basisSpec.outcomeCount,
      termsDigest: terms.termsDigest,
      feed: terms.feed
    };
    input[field] = "00".repeat(32);
    assert.throws(() => sdk.encodeCreateMarketIntent(input));
  }
  assert.throws(() => sdk.encodeCreateMarketIntent({
    realm: terms.realm, profile: terms.profile, marketNonce: "042", outcomeCount: 2, termsDigest: terms.termsDigest, feed: terms.feed
  }));
});
