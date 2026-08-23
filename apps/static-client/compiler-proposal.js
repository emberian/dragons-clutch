/*
 * Transport contract for Rust-produced Product compiler proposals.
 *
 * No payoff math is implemented here. The browser binds exact rational input
 * JSON to bytes/IDs emitted by an external operatord or CLI which calls
 * compile_production_payoff_v1 and the canonical bundle assembler.
 */
(function (root) {
  "use strict";

  const UINT = /^(0|[1-9][0-9]*)$/;
  const INT = /^(0|-?[1-9][0-9]*)$/;
  const HASH32 = /^[0-9a-f]{64}$/;
  const HEX = /^(?:[0-9a-f]{2})*$/;
  const U64_MAX = (1n << 64n) - 1n;
  const U128_MAX = (1n << 128n) - 1n;
  const STATUSES = new Set(["exact-in-span", "certified-approximation"]);
  const CLASSES = new Set(["exact-categorical", "exact-smooth", "analytic-smooth"]);
  const BOUND_NAMES = Object.freeze([
    "spline-sup-lower", "spline-sup-upper", "spline-l1-lower", "spline-l1-upper",
    "consensus-quantization-sup-upper", "consensus-sup-upper", "consensus-l1-upper",
    "coefficient-sample-sup-upper"
  ]);
  const BUNDLE_IDENTITY_NAMES = Object.freeze([
    "registryReleaseId", "capabilityProfileId", "sourceReleaseManifestId",
    "sourcePlaneContractId", "sourceSpecId", "summaryProgramId",
    "productCompilerReleaseId", "nativeClaimBasisId",
    "evidenceOnlyRecoveryPolicyId", "productTemplateId", "priceMeasurePolicyId",
    "marketGenesisProfileId", "fundingQuoteId", "attachmentPlanId",
    "seriesPlanId", "fundingTermsId"
  ]);
  const BUNDLE_INPUT_BYTES = Object.freeze({
    registryCapabilityProfileV2BytesHex: 800,
    evidenceOnlyRecoveryPolicyV1BytesHex: 208,
    productTemplateV4BytesHex: 256,
    priceMeasurePolicyV1BytesHex: 96,
    marketGenesisProfileV2BytesHex: 416,
    seriesFundingQuoteV1BytesHex: 280,
    seriesAttachmentPlanV1BytesHex: 112,
    seriesPlanV5BytesHex: 152,
    seriesFundingTermsV2BytesHex: 240
  });

  const plain = (value) => Boolean(value) && typeof value === "object" && !Array.isArray(value) && Object.getPrototypeOf(value) === Object.prototype;
  const hash = (value, name) => {
    if (typeof value !== "string" || !HASH32.test(value) || /^0+$/.test(value)) throw new Error(`${name} must be a nonzero lowercase 32-byte hexadecimal identity.`);
    return value;
  };
  const decimal = (value, name, maximum) => {
    if (typeof value !== "string" || !UINT.test(value)) throw new Error(`${name} must be a canonical unsigned decimal string.`);
    const parsed = BigInt(value);
    if (parsed > maximum) throw new Error(`${name} exceeds its exact integer width.`);
    return parsed.toString();
  };
  const positiveDecimal = (value, name, maximum) => {
    const parsed = decimal(value, name, maximum);
    if (parsed === "0") throw new Error(`${name} must be positive.`);
    return parsed;
  };
  const exactKeys = (value, expected, name) => {
    if (!plain(value)) throw new Error(`${name} must be an object.`);
    const keys = Object.keys(value);
    if (keys.length !== expected.length || expected.some((key) => !keys.includes(key))) throw new Error(`${name} must contain exactly ${expected.join(", ")}.`);
    return value;
  };
  const bytes = (value, name, exactBytes, maximumBytes) => {
    if (typeof value !== "string" || !HEX.test(value)) throw new Error(`${name} must be lowercase, even-length hexadecimal bytes.`);
    const length = value.length / 2;
    if (exactBytes !== null && length !== exactBytes) throw new Error(`${name} must contain exactly ${exactBytes} bytes.`);
    if (maximumBytes !== null && length > maximumBytes) throw new Error(`${name} exceeds ${maximumBytes} bytes.`);
    return value;
  };
  const rational = (value, name) => {
    if (!plain(value) || Object.keys(value).length !== 2 || typeof value.numerator !== "string" || value.numerator.length > 4096 || !INT.test(value.numerator) || typeof value.denominator !== "string" || value.denominator.length > 4096 || !UINT.test(value.denominator) || value.denominator === "0") {
      throw new Error(`${name} must be an exact rational {numerator, denominator} in canonical decimal strings.`);
    }
    const numerator = BigInt(value.numerator);
    const denominator = BigInt(value.denominator);
    const gcd = (left, right) => {
      left = left < 0n ? -left : left;
      while (right !== 0n) { const next = left % right; left = right; right = next; }
      return left;
    };
    if (gcd(numerator, denominator) !== 1n) throw new Error(`${name} is not reduced to lowest terms.`);
    return Object.freeze({ numerator: numerator.toString(), denominator: denominator.toString() });
  };

  const canonicalize = (value) => Array.isArray(value) ? value.map(canonicalize) : plain(value)
    ? Object.keys(value).sort().reduce((output, key) => { output[key] = canonicalize(value[key]); return output; }, {})
    : value;
  const canonicalJson = (value) => JSON.stringify(canonicalize(value));

  const smoothBasis = (raw, name) => {
    exactKeys(raw, ["degree", "coordinateDomainMin", "coordinateDomainMax", "payoutDenominator", "knots", "resolvedEdgePolicy", "ambiguityPolicyRegistryValue", "edgePolicyRegistryValue"], name);
    const degree = positiveDecimal(raw.degree, `${name}.degree`, 3n);
    if (!Array.isArray(raw.knots) || raw.knots.length < 2 || raw.knots.length > 16) throw new Error(`${name}.knots must contain 2..16 exact coordinates.`);
    if (raw.resolvedEdgePolicy !== "clamp" && raw.resolvedEdgePolicy !== "refuse") throw new Error(`${name}.resolvedEdgePolicy must be clamp or refuse.`);
    return Object.freeze({
      degree,
      coordinateDomainMin: decimal(raw.coordinateDomainMin, `${name}.coordinateDomainMin`, U128_MAX),
      coordinateDomainMax: decimal(raw.coordinateDomainMax, `${name}.coordinateDomainMax`, U128_MAX),
      payoutDenominator: positiveDecimal(raw.payoutDenominator, `${name}.payoutDenominator`, U64_MAX),
      knots: Object.freeze(raw.knots.map((value, index) => decimal(value, `${name}.knots[${index}]`, U128_MAX))),
      resolvedEdgePolicy: raw.resolvedEdgePolicy,
      ambiguityPolicyRegistryValue: positiveDecimal(raw.ambiguityPolicyRegistryValue, `${name}.ambiguityPolicyRegistryValue`, 255n),
      edgePolicyRegistryValue: positiveDecimal(raw.edgePolicyRegistryValue, `${name}.edgePolicyRegistryValue`, 255n)
    });
  };

  const analyticShape = (raw, name) => {
    if (!plain(raw)) throw new Error(`${name} must be an object.`);
    const fields = {
      "hard-range": ["low", "high", "height"],
      "upper-tail": ["strike", "height"],
      "lower-tail": ["strike", "height"],
      triangle: ["left", "peak", "right", "height"],
      "capped-call": ["low", "high", "height"],
      "capped-put": ["low", "high", "height"],
      gaussian: ["center", "sigma", "height"]
    }[raw.kind];
    if (!fields) throw new Error(`${name}.kind is not a supported bounded analytic shape.`);
    exactKeys(raw, ["kind", ...fields], name);
    const output = { kind: raw.kind };
    for (const field of fields) output[field] = field === "height"
      ? decimal(raw[field], `${name}.${field}`, U64_MAX)
      : decimal(raw[field], `${name}.${field}`, U128_MAX);
    return Object.freeze(output);
  };

  const validateDefinition = (raw) => {
    if (!plain(raw)) throw new Error("Payoff definition must be an object.");
    let nodes = 0;
    const visit = (value, path, depth) => {
      nodes += 1;
      if (nodes > 20_000 || depth > 12) throw new Error("Payoff definition exceeds browser shape bounds.");
      if (typeof value === "number" || typeof value === "bigint") throw new Error(`${path} must use decimal strings, never a JavaScript number.`);
      if (typeof value === "string") {
        if (value.length === 0 || value.length > 4096) throw new Error(`${path} contains invalid text.`);
        return;
      }
      if (typeof value === "boolean" || value === null) return;
      if (Array.isArray(value)) {
        if (value.length > 4096) throw new Error(`${path} contains too many entries.`);
        value.forEach((item, index) => visit(item, `${path}[${index}]`, depth + 1));
        return;
      }
      if (!plain(value)) throw new Error(`${path} contains an unsupported value.`);
      const keys = Object.keys(value);
      if (keys.length === 0 || keys.length > 128) throw new Error(`${path} has an invalid field count.`);
      for (const key of keys) {
        if (!/^[A-Za-z][A-Za-z0-9]*$/.test(key)) throw new Error(`${path} contains unsupported field name ${JSON.stringify(key)}.`);
        visit(value[key], `${path}.${key}`, depth + 1);
      }
    };
    visit(raw, "definition", 0);
    exactKeys(raw, ["schema", "productTermsId", "kind", "definition"], "definition");
    if (raw.schema !== "dragons-clutch/compiler/production-payoff-definition/v1" || !CLASSES.has(raw.kind)) throw new Error("Payoff definition has an unknown schema or compiler input class.");
    const productTermsId = hash(raw.productTermsId, "definition.productTermsId");
    let body;
    if (raw.kind === "exact-categorical") {
      exactKeys(raw.definition, ["coordinateDomainMin", "coordinateDomainMax", "knots", "cellPayouts", "ambiguityPolicyRegistryValue", "edgePolicyRegistryValue"], "definition.definition");
      if (!Array.isArray(raw.definition.cellPayouts) || raw.definition.cellPayouts.length === 0 || raw.definition.cellPayouts.length > 16) throw new Error("definition.definition.cellPayouts must contain 1..16 coordinate cells.");
      if (!Array.isArray(raw.definition.knots) || raw.definition.knots.length + 1 !== raw.definition.cellPayouts.length) throw new Error("Categorical knots must contain exactly one fewer entry than cellPayouts.");
      let payoutWidth = null;
      const cellPayouts = raw.definition.cellPayouts.map((row, rowIndex) => {
        if (!Array.isArray(row) || row.length === 0 || row.length > 16) throw new Error(`cellPayouts[${rowIndex}] must contain 1..16 exact rationals.`);
        if (payoutWidth === null) payoutWidth = row.length;
        if (row.length !== payoutWidth) throw new Error("All categorical payout rows must have the same width.");
        return Object.freeze(row.map((value, columnIndex) => rational(value, `cellPayouts[${rowIndex}][${columnIndex}]`)));
      });
      body = Object.freeze({
        coordinateDomainMin: decimal(raw.definition.coordinateDomainMin, "definition.definition.coordinateDomainMin", U128_MAX),
        coordinateDomainMax: decimal(raw.definition.coordinateDomainMax, "definition.definition.coordinateDomainMax", U128_MAX),
        knots: Object.freeze(raw.definition.knots.map((value, index) => decimal(value, `definition.definition.knots[${index}]`, U128_MAX))),
        cellPayouts: Object.freeze(cellPayouts),
        ambiguityPolicyRegistryValue: positiveDecimal(raw.definition.ambiguityPolicyRegistryValue, "definition.definition.ambiguityPolicyRegistryValue", 255n),
        edgePolicyRegistryValue: positiveDecimal(raw.definition.edgePolicyRegistryValue, "definition.definition.edgePolicyRegistryValue", 255n)
      });
    } else if (raw.kind === "exact-smooth") {
      exactKeys(raw.definition, ["basis", "controlValues", "maximumLiability"], "definition.definition");
      if (!Array.isArray(raw.definition.controlValues) || raw.definition.controlValues.length < 2 || raw.definition.controlValues.length > 16) throw new Error("Exact smooth controlValues must contain 2..16 exact rationals.");
      body = Object.freeze({
        basis: smoothBasis(raw.definition.basis, "definition.definition.basis"),
        controlValues: Object.freeze(raw.definition.controlValues.map((value, index) => rational(value, `definition.definition.controlValues[${index}]`))),
        maximumLiability: rational(raw.definition.maximumLiability, "definition.definition.maximumLiability")
      });
    } else {
      exactKeys(raw.definition, ["basis", "shape"], "definition.definition");
      body = Object.freeze({
        basis: smoothBasis(raw.definition.basis, "definition.definition.basis"),
        shape: analyticShape(raw.definition.shape, "definition.definition.shape")
      });
    }
    const value = Object.freeze({ schema: raw.schema, productTermsId, kind: raw.kind, definition: body });
    const encoded = canonicalJson(value);
    if (new TextEncoder().encode(encoded).byteLength > 262_144) throw new Error("Payoff definition exceeds 262144 canonical UTF-8 bytes.");
    return Object.freeze({ value, canonicalJson: encoded, productTermsId, classification: raw.kind });
  };

  const validateBundleInputs = (raw) => {
    const names = ["registryCapabilityProfileV2BytesHex", "sourceReleaseManifestId", "evidenceOnlyRecoveryPolicyV1BytesHex", "productTemplateV4BytesHex", "priceMeasurePolicyV1BytesHex", "marketGenesisProfileV2BytesHex", "seriesFundingQuoteV1BytesHex", "seriesAttachmentPlanV1BytesHex", "seriesPlanV5BytesHex", "seriesFundingTermsV2BytesHex"];
    exactKeys(raw, names, "bundle inputs");
    const output = { sourceReleaseManifestId: hash(raw.sourceReleaseManifestId, "bundleInputs.sourceReleaseManifestId") };
    for (const [name, length] of Object.entries(BUNDLE_INPUT_BYTES)) output[name] = bytes(raw[name], `bundleInputs.${name}`, length, null);
    return Object.freeze(output);
  };

  const compileRemote = async (operatorUrl, compilerReleaseSha256, definition, bundleInputs, maximumResponseBytes, timeoutMilliseconds) => {
    const expectedCompilerReleaseSha256 = hash(compilerReleaseSha256, "compiler release SHA-256");
    const request = Object.freeze({
      schema: "dragons-clutch/compiler/production-payoff-request/v1",
      expectedCompilerReleaseSha256,
      definition: definition.value,
      bundleInputs: validateBundleInputs(bundleInputs)
    });
    const encoded = new TextEncoder().encode(JSON.stringify(request));
    if (encoded.byteLength > 327_680) throw new Error("Compiler request exceeds the operatord 327680-byte request bound.");
    const maximum = BigInt(maximumResponseBytes);
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), Number(timeoutMilliseconds));
    try {
      const response = await fetch(`${operatorUrl}/v1/compiler/production-payoff`, {
        method: "POST",
        mode: "cors",
        credentials: "omit",
        cache: "no-store",
        redirect: "error",
        referrerPolicy: "no-referrer",
        headers: { Accept: "application/json", "Content-Type": "application/json" },
        body: encoded,
        signal: controller.signal
      });
      const contentType = response.headers.get("content-type") || "";
      if (!contentType.toLowerCase().startsWith("application/json")) throw new Error("Compiler endpoint did not return application/json.");
      const declared = response.headers.get("content-length");
      if (declared !== null && BigInt(decimal(declared, "compiler Content-Length", U64_MAX)) > maximum) throw new Error("Compiler response exceeds the selected response-byte budget.");
      const chunks = [];
      let length = 0n;
      if (response.body && typeof response.body.getReader === "function") {
        const reader = response.body.getReader();
        for (;;) {
          const item = await reader.read();
          if (item.done) break;
          length += BigInt(item.value.byteLength);
          if (length > maximum) {
            await reader.cancel();
            throw new Error("Compiler response exceeded the selected response-byte budget while reading.");
          }
          chunks.push(item.value);
        }
      } else {
        const body = new Uint8Array(await response.arrayBuffer());
        length = BigInt(body.byteLength);
        if (length > maximum) throw new Error("Compiler response exceeds the selected response-byte budget.");
        chunks.push(body);
      }
      const body = new Uint8Array(Number(length));
      let offset = 0;
      for (const chunk of chunks) { body.set(chunk, offset); offset += chunk.byteLength; }
      let parsed;
      try { parsed = JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(body)); } catch (_) { throw new Error("Compiler endpoint did not return valid UTF-8 JSON."); }
      if (!response.ok) {
        const detail = plain(parsed) && typeof parsed.detail === "string" && parsed.detail.length <= 4096 ? parsed.detail : `HTTP ${response.status}`;
        throw new Error(`Compiler refused: ${detail}`);
      }
      return parsed;
    } finally {
      clearTimeout(timer);
    }
  };

  const validateProposal = (raw, expectedInputSha256, expectedCompilerReleaseSha256, expectedDefinition) => {
    exactKeys(raw, ["schema", "authority", "registrationAuthority", "compilerReleaseSha256", "inputCanonicalSha256", "productTermsId", "classification", "spanStatus", "nativeClaimBasis", "certificate", "bounds", "subdivisionDepth", "compiledProductSeriesBundle"], "compiler proposal");
    if (!plain(raw) || raw.schema !== "dragons-clutch/compiler/production-payoff-proposal/v1" || raw.authority !== "untrusted-compiler-proposal" || raw.registrationAuthority !== false) {
      throw new Error("Compiler result is not an untrusted production-payoff proposal v1.");
    }
    const compilerReleaseSha256 = hash(raw.compilerReleaseSha256, "compilerReleaseSha256");
    if (compilerReleaseSha256 !== expectedCompilerReleaseSha256) throw new Error("Compiler result names a different explicit compiler release.");
    const inputCanonicalSha256 = hash(raw.inputCanonicalSha256, "inputCanonicalSha256");
    if (inputCanonicalSha256 !== expectedInputSha256) throw new Error("Compiler result is not bound to the exact rational definition shown in this page.");
    if (!CLASSES.has(raw.classification) || !STATUSES.has(raw.spanStatus)) throw new Error("Compiler classification or span status is unknown.");
    if (!expectedDefinition || raw.classification !== expectedDefinition.classification) throw new Error("Compiler classification differs from the exact definition variant.");
    if (raw.classification !== "analytic-smooth" && raw.spanStatus !== "exact-in-span") throw new Error("Only analytic smooth requests may carry a certified approximation status.");
    const nativeClaimBasis = (() => {
      exactKeys(raw.nativeClaimBasis, ["id", "bytesHex"], "nativeClaimBasis");
      return Object.freeze({
        id: hash(raw.nativeClaimBasis.id, "nativeClaimBasis.id"),
        bytesHex: bytes(raw.nativeClaimBasis.bytesHex, "nativeClaimBasis.bytesHex", 2352, null),
        byteLength: "2352"
      });
    })();
    const certificate = raw.certificate === null ? null : (() => {
      exactKeys(raw.certificate, ["id", "bytesHex"], "certificate");
      const body = bytes(raw.certificate.bytesHex, "certificate.bytesHex", null, 262_144);
      if (body.length === 0) throw new Error("certificate.bytesHex must not be empty.");
      return Object.freeze({ id: hash(raw.certificate.id, "certificate.id"), bytesHex: body, byteLength: String(body.length / 2) });
    })();
    if (raw.classification === "exact-categorical" && certificate !== null) throw new Error("Exact categorical output is owned by the basis and must not invent a second certificate.");
    if (raw.classification !== "exact-categorical" && certificate === null) throw new Error("Smooth compiler output requires its recompilable certificate.");
    if (!Array.isArray(raw.bounds)) throw new Error("bounds must be an array.");
    const bounds = raw.bounds.map((bound, index) => {
      exactKeys(bound, ["name", "value"], `bounds[${index}]`);
      if (!BOUND_NAMES.includes(bound.name)) throw new Error(`bounds[${index}] has an unknown metric name.`);
      return Object.freeze({ name: bound.name, value: rational(bound.value, `bounds[${index}].value`) });
    });
    const names = new Set(bounds.map((bound) => bound.name));
    if (names.size !== bounds.length) throw new Error("Compiler bounds contain duplicate metrics.");
    if (raw.classification === "analytic-smooth" && BOUND_NAMES.some((name) => !names.has(name))) throw new Error("Analytic smooth output must carry all eight exact rational compiler bounds.");
    if (raw.classification !== "analytic-smooth" && bounds.length !== 0) throw new Error("Exact categorical/smooth output must not carry analytic approximation bounds.");
    const subdivisionDepth = raw.classification === "analytic-smooth"
      ? decimal(raw.subdivisionDepth, "subdivisionDepth", 255n)
      : null;
    if (raw.classification !== "analytic-smooth" && raw.subdivisionDepth !== null) throw new Error("Only analytic smooth output carries a subdivisionDepth.");
    exactKeys(raw.compiledProductSeriesBundle, ["id", "bytesHex", "identities"], "compiledProductSeriesBundle");
    const bundleBytes = bytes(raw.compiledProductSeriesBundle.bytesHex, "compiledProductSeriesBundle.bytesHex", 528, null);
    if (!plain(raw.compiledProductSeriesBundle.identities)) throw new Error("compiledProductSeriesBundle.identities is required.");
    const identities = {};
    const namesInProposal = Object.keys(raw.compiledProductSeriesBundle.identities);
    if (namesInProposal.length !== BUNDLE_IDENTITY_NAMES.length || BUNDLE_IDENTITY_NAMES.some((name) => !namesInProposal.includes(name))) {
      throw new Error("Compiled Product/Series bundle must expose the exact sixteen typed identities owned by CompiledProductSeriesBundleV1.");
    }
    for (const name of BUNDLE_IDENTITY_NAMES) {
      identities[name] = hash(raw.compiledProductSeriesBundle.identities[name], `compiledProductSeriesBundle.identities.${name}`);
    }
    if (identities.nativeClaimBasisId !== nativeClaimBasis.id) throw new Error("Compiled Product/Series bundle names a different nativeClaimBasisId than the compiler output.");
    const productTermsId = hash(raw.productTermsId, "productTermsId");
    if (productTermsId !== expectedDefinition.productTermsId) throw new Error("Compiler result names a different Product Terms identity than the exact definition.");
    return Object.freeze({
      schema: raw.schema,
      authority: raw.authority,
      registrationAuthority: false,
      compilerReleaseSha256,
      inputCanonicalSha256,
      productTermsId,
      classification: raw.classification,
      spanStatus: raw.spanStatus,
      nativeClaimBasis,
      certificate,
      bounds: Object.freeze(bounds),
      subdivisionDepth,
      compiledProductSeriesBundle: Object.freeze({
        id: hash(raw.compiledProductSeriesBundle.id, "compiledProductSeriesBundle.id"),
        bytesHex: bundleBytes,
        byteLength: "528",
        identities: Object.freeze(identities)
      }),
      registration: "must reopen registry, Source release, canonical artifacts, and recompute every join"
    });
  };

  root.GlassCompilerProposal = Object.freeze({
    validateDefinition,
    validateBundleInputs,
    validateProposal,
    compileRemote,
    canonicalJson,
    boundNames: BOUND_NAMES,
    bundleIdentityNames: BUNDLE_IDENTITY_NAMES
  });
})(typeof globalThis === "object" ? globalThis : this);
